use crate::*;
use crate::common::shared::Shared;

use super::*;
use super::space::grounding::GroundingSpace;
use super::text::{Tokenizer, SExprParser};
use super::types::validate_atom;
use super::interpreter::interpret;

use std::path::PathBuf;
use std::collections::HashMap;

mod stdlib;

mod arithmetics;

const EXEC_SYMBOL : Atom = sym!("!");

pub struct Metta {
    space: Shared<GroundingSpace>,
    tokenizer: Shared<Tokenizer>,
    settings: Shared<HashMap<String, String>>,
    modules: Shared<HashMap<PathBuf, Shared<GroundingSpace>>>,
}

enum Mode {
    ADD,
    INTERPRET,
}

impl Metta {
    pub fn new(space: Shared<GroundingSpace>, tokenizer: Shared<Tokenizer>) -> Self {
        Metta::from_space_cwd(space, tokenizer, PathBuf::from("."))
    }

    pub fn from_space_cwd(space: Shared<GroundingSpace>, tokenizer: Shared<Tokenizer>, cwd: PathBuf) -> Self {
        let settings = Shared::new(HashMap::new());
        let modules = Shared::new(HashMap::new());
        let metta = Self{ space, tokenizer, settings, modules };
        stdlib::register_common_tokens(&metta, cwd);
        metta
    }

    pub fn load_module(&self, path: PathBuf) {
        let mut my_modules = self.modules.borrow_mut();
        // Loading the module only once
        // TODO? force_reload?
        let space =
            match my_modules.get(&path) {
                Some(module_space) => module_space.cloned(),
                None => {
                    let space = Shared::new(GroundingSpace::new());
                    let tokenizer = self.tokenizer.clone();
                    let settings = self.settings.clone();
                    let modules = self.modules.clone();
                    // We don't use Metta::[new|from_space_cwd] in order to use the right tokenizer
                    // (and to avoid overriding it with Rust tokens)
                    let runner = Self { space, tokenizer, settings, modules };
                    let program = match path.to_str() {
                        Some("stdlib") => stdlib::metta_code().to_string(),
                        _ => {
                            let prog = std::fs::read_to_string(&path);
                            let prog = match prog {
                                Err(err) => format!("Could not read file {}: {}", path.display(), err),
                                Ok(str) => str,
                            };
                            prog
                        },
                    };
                    runner.run(&mut SExprParser::new(program.as_str())).expect("Cannot import stdlib code");
                    my_modules.insert(path, runner.space.clone());
                    runner.space.clone()
                }
            };
        // REM: loading the module to &self now
        let space_atom = Atom::gnd(space);
        // TODO: Should we register the module name?
        // self.tokenizer.borrow_mut().register_token(stdlib::regex(name), move |_| { space_atom.clone() });
        // TODO: check if it is already there (if the module is newly loaded)
        self.space.borrow_mut().add(space_atom);
    }

    pub fn space(&self) -> Shared<GroundingSpace> {
        self.space.clone()
    }

    pub fn tokenizer(&self) -> Shared<Tokenizer> {
        self.tokenizer.clone()
    }

    #[cfg(test)]
    fn set_setting(&self, key: String, value: String) {
        self.settings.borrow_mut().insert(key, value);
    }

    fn get_setting(&self, key: &str) -> Option<String> {
        self.settings.borrow().get(key.into()).cloned()
    }

    pub fn run(&self, parser: &mut SExprParser) -> Result<Vec<Vec<Atom>>, String> {
        let mut mode = Mode::ADD;
        let mut results: Vec<Vec<Atom>> = Vec::new();

        loop {
            let atom = parser.parse(&self.tokenizer.borrow());
            match atom {
                Some(atom) => {
                    if atom == EXEC_SYMBOL {
                        mode = Mode::INTERPRET;
                        continue;
                    }
                    match mode {
                        Mode::ADD => match self.add_atom(atom) {
                            Err(atom) => {
                                results.push(vec![atom]);
                                break
                            }
                            Ok(()) => {},
                        }
                        Mode::INTERPRET => match self.evaluate_atom(atom) {
                            Err(msg) => return Err(msg),
                            Ok(result) => {
                                fn is_error(atom: &Atom) -> bool {
                                    match atom {
                                        Atom::Expression(expr) => expr.children()[0] == ERROR_SYMBOL,
                                        _ => false,
                                    }
                                }
                                let error = result.iter()
                                    .map(|atom| is_error(atom))
                                    .fold(false, |a, b| a | b);
                                results.push(result);
                                if error {
                                    break
                                }
                            }
                        },
                    }
                    mode = Mode::ADD;
                },
                None => break,
            }
        }
        Ok(results)
    }

    pub fn evaluate_atom(&self, atom: Atom) -> Result<Vec<Atom>, String> {
        match self.type_check(atom) {
            Err(atom) => Ok(vec![atom]),
            Ok(atom) => interpret(self.space.clone(), &atom),
        }
    }

    fn add_atom(&self, atom: Atom) -> Result<(), Atom>{
        let atom = self.type_check(atom)?;
        self.space.borrow_mut().add(atom);
        Ok(())
    }

    fn type_check(&self, atom: Atom) -> Result<Atom, Atom> {
        let is_type_check_enabled = self.get_setting("type-check").map_or(false, |val| val == "auto");
        if  is_type_check_enabled && !validate_atom(&self.space.borrow(), &atom) {
            Err(Atom::expr([ERROR_SYMBOL, atom, BAD_TYPE_SYMBOL]))
        } else {
            Ok(atom)
        }
    }

}

pub fn new_metta_rust() -> Metta {
    let metta = Metta::new(Shared::new(GroundingSpace::new()),
        Shared::new(Tokenizer::new()));
    stdlib::register_rust_tokens(&metta);
    metta.load_module(PathBuf::from("stdlib"));
    metta
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_space() {
        let program = "
            (= (And T T) T)
            (= (frog $x)
                (And (croaks $x)
                     (eat_flies $x)))
            (= (croaks Fritz) T)
            (= (eat_flies Fritz) T)
            (= (green $x) (frog $x))
            !(green Fritz)
        ";

        let metta = new_metta_rust();
        let result = metta.run(&mut SExprParser::new(program));
        assert_eq!(result, Ok(vec![vec![Atom::sym("T")]]));
    }

    #[test]
    fn metta_add_type_check() {
        let program = "
            (: foo (-> A B))
            (: b B)
            (foo b)
        ";

        let metta = Metta::new(Shared::new(GroundingSpace::new()), Shared::new(Tokenizer::new()));
        metta.set_setting("type-check".into(), "auto".into());
        let result = metta.run(&mut SExprParser::new(program));
        assert_eq!(result, Ok(vec![vec![expr!("Error" ("foo" "b") "BadType")]]));
    }

    #[test]
    fn metta_interpret_type_check() {
        let program = "
            (: foo (-> A B))
            (: b B)
            !(foo b)
        ";

        let metta = Metta::new(Shared::new(GroundingSpace::new()), Shared::new(Tokenizer::new()));
        metta.set_setting("type-check".into(), "auto".into());
        let result = metta.run(&mut SExprParser::new(program));
        assert_eq!(result, Ok(vec![vec![expr!("Error" ("foo" "b") "BadType")]]));
    }

    #[derive(Clone, PartialEq, Debug)]
    struct ErrorOp{}

    impl std::fmt::Display for ErrorOp {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "error")
        }
    }

    impl Grounded for ErrorOp {
        fn type_(&self) -> Atom {
            Atom::expr([ARROW_SYMBOL, ATOM_TYPE_UNDEFINED])
        }
        fn execute(&self, _args: &mut Vec<Atom>) -> Result<Vec<Atom>, ExecError> {
            //FIXME: why next two lines led to not equal results?
            Ok(vec![expr!("Error" ("error") "TestError")])
            //Err("TestError".into())
        }
        fn match_(&self, other: &Atom) -> crate::matcher::MatchResultIter {
            match_by_equality(self, other)
        }
    }

    #[test]
    fn metta_stop_run_after_error() {
        let program = "
            (= (foo) ok)
            !(error)
            !(foo)
        ";

        let metta = Metta::new(Shared::new(GroundingSpace::new()), Shared::new(Tokenizer::new()));
        metta.tokenizer().borrow_mut().register_token(Regex::new("error").unwrap(),
            |_| Atom::gnd(ErrorOp{}));
        let result = metta.run(&mut SExprParser::new(program));

        assert_eq!(result, Ok(vec![vec![expr!("Error" ("error") "TestError")]]));
    }

    #[test]
    fn metta_stop_after_type_check_fails_on_add() {
        let program = "
            (: foo (-> A B))
            (: a A)
            (: b B)
            (foo b)
            !(foo a)
        ";

        let metta = Metta::new(Shared::new(GroundingSpace::new()), Shared::new(Tokenizer::new()));
        metta.set_setting("type-check".into(), "auto".into());
        let result = metta.run(&mut SExprParser::new(program));
        assert_eq!(result, Ok(vec![vec![expr!("Error" ("foo" "b") "BadType")]]));
    }
}
