use crate::*;
use crate::atom::*;
use crate::atom::matcher::{Bindings, Unifications, WithMatch};
use crate::atom::subexpr::split_expr;

use std::fmt::{Display, Debug};
use std::rc::{Rc, Weak};
use std::cell::{RefCell, Ref};
use std::collections::HashSet;

// Grounding space

const COMMA_SYMBOL : Atom = sym!(",");

#[derive(Clone, Debug, PartialEq)]
pub enum SpaceEvent {
    Add(Atom),
    Remove(Atom),
    Replace(Atom, Atom),
}

pub trait SpaceObserver {
    fn notify(&mut self, event: &SpaceEvent);
}

// TODO: Clone is required by C API
#[derive(Clone)]
pub struct GroundingSpace {
    content: Rc<RefCell<Vec<Atom>>>,
    observers: Rc<RefCell<Vec<Weak<RefCell<dyn SpaceObserver>>>>>,
}

impl GroundingSpace {

    pub fn new() -> Self {
        Self{
            content: Rc::new(RefCell::new(Vec::new())),
            observers: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn register_observer<T>(&mut self, observer: Rc<RefCell<T>>)
        where T: SpaceObserver + 'static
    {
        self.observers.borrow_mut().push(Rc::downgrade(&observer) as Weak<RefCell<dyn SpaceObserver>>);
    }

    fn notify(&self, event: &SpaceEvent) {
        let mut cleanup = false;
        for observer in self.observers.borrow_mut().iter() {
            if let Some(observer) = observer.upgrade() {
                observer.borrow_mut().notify(event);
            } else {
                cleanup = true;
            }
        }
        if cleanup {
            self.observers.borrow_mut().retain(|w| w.strong_count() > 0);
        }
    }

    pub fn add(&mut self, atom: Atom) {
        self.content.borrow_mut().push(atom.clone());
        self.notify(&SpaceEvent::Add(atom));
    }

    pub fn remove(&mut self, atom: &Atom) -> bool {
        let position = self.borrow_vec().iter().position(|other| other == atom);
        match position {
            Some(position) => {
                self.content.borrow_mut().remove(position);
                self.notify(&SpaceEvent::Remove(atom.clone()));
                true
            },
            None => false, 
        }
    }

    pub fn replace(&mut self, from: &Atom, to: Atom) -> bool {
        let position = self.borrow_vec().iter().position(|other| other == from);
        match position {
            Some(position) => {
                self.content.borrow_mut().as_mut_slice()[position] = to.clone();
                self.notify(&SpaceEvent::Replace(from.clone(), to));
                true
            },
            None => false, 
        }
    }

    pub fn query(&self, pattern: &Atom) -> Vec<Bindings> {
        match split_expr(pattern) {
            // Cannot match with COMMA_SYMBOL here, because Rust allows
            // it only when Atom has PartialEq and Eq derived.
            Some((sym @ Atom::Symbol(_), args)) if *sym == COMMA_SYMBOL => {
                let vars = collect_variables(&pattern);
                let mut result = args.fold(vec![bind!{}],
                    |mut acc, pattern| {
                        let result = if acc.is_empty() {
                            acc
                        } else {
                            acc.drain(0..).flat_map(|prev| -> Vec<Bindings> {
                                let pattern = matcher::apply_bindings_to_atom(&pattern, &prev);
                                let mut res = self.query(&pattern);
                                res.drain(0..)
                                    .map(|next| Bindings::merge(&prev, &next))
                                    .filter(Option::is_some).map(Option::unwrap)
                                    .map(|next| matcher::apply_bindings_to_bindings(&next, &next)
                                        .expect("Self consistent bindings are expected"))
                                    .collect()
                            }).collect()
                        };
                        log::debug!("query: current result: {:?}", result);
                        result
                    });
                result.iter_mut().for_each(|bindings| bindings.filter(|k, _v| vars.contains(k)));
                result
            },
            _ => self.single_query(pattern),
        }
    }

    fn single_query(&self, pattern: &Atom) -> Vec<Bindings> {
        log::debug!("single_query: pattern: {}", pattern);
        let mut result = Vec::new();
        for next in &(*self.borrow_vec()) {
            let next = replace_variables(next);
            log::trace!("single_query: match next: {}", next);
            for bindings in next.match_(pattern) {
                log::trace!("single_query: push result: {}", bindings);
                result.push(bindings);
            }
        }
        log::debug!("single_query: result: {:?}", result);
        result
    }

    pub fn subst(&self, pattern: &Atom, template: &Atom) -> Vec<Atom> {
        self.query(pattern).drain(0..)
            .map(| bindings | matcher::apply_bindings_to_atom(template, &bindings))
            .collect()
    }

    // TODO: for now we have separate methods query() and unify() but
    // they probably can be merged. One way of doing it is designating
    // in the query which part of query should be unified and which matched.
    // For example for the typical query in a form (= (+ a b) $X) the
    // (= (...) $X) level should not be unified otherwise we will recursively
    // infer that we need calculating (+ a b) again which is equal to original
    // query. Another option is designating this in the data itself.
    // After combining match and unification we could leave only single
    // universal method.
    pub fn unify(&self, pattern: &Atom) -> Vec<(Bindings, Unifications)> {
        log::debug!("unify: pattern: {}", pattern);
        let mut result = Vec::new();
        for next in &(*self.borrow_vec()) {
            match matcher::unify_atoms(next, pattern) {
                Some(res) => {
                    let bindings = matcher::apply_bindings_to_bindings(&res.data_bindings, &res.pattern_bindings);
                    if let Ok(bindings) = bindings {
                        // TODO: implement Display for bindings
                        log::debug!("unify: push result: {}, bindings: {:?}", next, bindings);
                        result.push((bindings, res.unifications));
                    }
                },
                None => continue,
            }
        }
        result
    }

    pub fn borrow_vec(&self) -> Ref<Vec<Atom>> {
        self.content.borrow()
    }

    pub fn leak(self) -> Vec<Atom> {
        self.content.take()
    }
}

impl PartialEq for GroundingSpace {
    fn eq(&self, other: &Self) -> bool {
        self.content == other.content
    }
}

impl Debug for GroundingSpace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GroundingSpace")
    }
}

impl Display for GroundingSpace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GroundingSpace")
    }
}

fn collect_variables(atom: &Atom) -> HashSet<VariableAtom> {
    fn recursion(atom: &Atom, vars: &mut HashSet<VariableAtom>) {
        match atom {
            Atom::Variable(var) => { vars.insert(var.clone()); },
            Atom::Expression(expr) => {
                expr.children().iter().for_each(|child| recursion(child, vars));
            }
            _ => {},
        }
    }
    let mut vars = HashSet::new();
    recursion(atom, &mut vars);
    vars
}


#[cfg(test)]
mod test {
    use super::*;

    struct SpaceEventCollector {
        events: Vec<SpaceEvent>,
    }

    impl SpaceEventCollector {
        fn new() -> Self {
            Self{ events: Vec::new() }
        }
    }

    impl SpaceObserver for SpaceEventCollector {
        fn notify(&mut self, event: &SpaceEvent) {
            self.events.push(event.clone());
        }
    }

    #[test]
    fn add_atom() {
        let mut space = GroundingSpace::new();
        let observer = Rc::new(RefCell::new(SpaceEventCollector::new()));
        space.register_observer(Rc::clone(&observer));

        space.add(expr!("a"));
        space.add(expr!("b"));
        space.add(expr!("c"));

        assert_eq!(*space.borrow_vec(), vec![expr!("a"), expr!("b"), expr!("c")]);
        assert_eq!(observer.borrow().events, vec![SpaceEvent::Add(sym!("a")),
            SpaceEvent::Add(sym!("b")), SpaceEvent::Add(sym!("c"))]);
    }

    #[test]
    fn remove_atom() {
        let mut space = GroundingSpace::new();
        let observer = Rc::new(RefCell::new(SpaceEventCollector::new()));
        space.register_observer(Rc::clone(&observer));

        space.add(expr!("a"));
        space.add(expr!("b"));
        space.add(expr!("c"));
        assert_eq!(space.remove(&expr!("b")), true);

        assert_eq!(*space.borrow_vec(), vec![expr!("a"), expr!("c")]);
        assert_eq!(observer.borrow().events, vec![SpaceEvent::Add(sym!("a")),
            SpaceEvent::Add(sym!("b")), SpaceEvent::Add(sym!("c")),
            SpaceEvent::Remove(sym!("b"))]);
    }

    #[test]
    fn remove_atom_not_found() {
        let mut space = GroundingSpace::new();
        let observer = Rc::new(RefCell::new(SpaceEventCollector::new()));
        space.register_observer(Rc::clone(&observer));

        space.add(expr!("a"));
        assert_eq!(space.remove(&expr!("b")), false);

        assert_eq!(*space.borrow_vec(), vec![expr!("a")]);
        assert_eq!(observer.borrow().events, vec![SpaceEvent::Add(sym!("a"))]);
    }

    #[test]
    fn replace_atom() {
        let mut space = GroundingSpace::new();
        let observer = Rc::new(RefCell::new(SpaceEventCollector::new()));
        space.register_observer(Rc::clone(&observer));

        space.add(expr!("a"));
        space.add(expr!("b"));
        space.add(expr!("c"));
        assert_eq!(space.replace(&expr!("b"), expr!("d")), true);

        assert_eq!(*space.borrow_vec(), vec![expr!("a"), expr!("d"), expr!("c")]);
        assert_eq!(observer.borrow().events, vec![SpaceEvent::Add(sym!("a")),
            SpaceEvent::Add(sym!("b")), SpaceEvent::Add(sym!("c")),
            SpaceEvent::Replace(sym!("b"), sym!("d"))]);
    }

    #[test]
    fn replace_atom_not_found() {
        let mut space = GroundingSpace::new();
        let observer = Rc::new(RefCell::new(SpaceEventCollector::new()));
        space.register_observer(Rc::clone(&observer));

        space.add(expr!("a"));
        assert_eq!(space.replace(&expr!("b"), expr!("d")), false);

        assert_eq!(*space.borrow_vec(), vec![expr!("a")]);
        assert_eq!(observer.borrow().events, vec![SpaceEvent::Add(sym!("a"))]);
    }

    #[test]
    fn mut_shared_atomspace() {
        let mut first = GroundingSpace::new();
        let mut second = first.clone(); 

        first.add(expr!("b"));
        second.replace(&expr!("b"), expr!("d"));

        assert_eq!(*first.borrow_vec(), vec![expr!("d")]);
        assert_eq!(*second.borrow_vec(), vec![expr!("d")]);
    }

    #[test]
    fn test_match_symbol() {
        let mut space = GroundingSpace::new();
        space.add(expr!("foo"));
        assert_eq!(space.query(&expr!("foo")), vec![bind!{}]);
    }

    #[test]
    fn test_match_variable() {
        let mut space = GroundingSpace::new();
        space.add(expr!("foo"));
        assert_eq!(space.query(&expr!(x)), vec![bind!{x: expr!("foo")}]);
    }

    #[test]
    fn test_match_expression() {
        let mut space = GroundingSpace::new();
        space.add(expr!("+" "a" ("*" "b" "c")));
        assert_eq!(space.query(&expr!("+" "a" ("*" "b" "c"))), vec![bind!{}]);
    }

    #[test]
    fn test_match_expression_with_variables() {
        let mut space = GroundingSpace::new();
        space.add(expr!("+" "A" ("*" "B" "C")));
        assert_eq!(space.query(&expr!("+" a ("*" b c))),
        vec![bind!{a: expr!("A"), b: expr!("B"), c: expr!("C") }]);
    }

    #[test]
    fn test_match_different_value_for_variable() {
        let mut space = GroundingSpace::new();
        space.add(expr!("+" "A" ("*" "B" "C")));
        assert_eq!(space.query(&expr!("+" a ("*" a c))), vec![]);
    }

    fn get_var<'a>(bindings: &'a Bindings, name: &str) -> &'a Atom {
        bindings.get(&VariableAtom::new(name)).unwrap()
    }

    #[test]
    fn test_match_query_variable_has_priority() {
        let mut space = GroundingSpace::new();
        space.add(expr!("equals" x x));
        
        let result = space.query(&expr!("equals" y z));
        assert_eq!(result.len(), 1);
        assert!(matches!(get_var(&result[0], "y"), Atom::Variable(_)));
        assert!(matches!(get_var(&result[0], "z"), Atom::Variable(_)));
    }

    #[test]
    fn test_match_query_variable_via_data_variable() {
        let mut space = GroundingSpace::new();
        space.add(expr!(x x));
        assert_eq!(space.query(&expr!(y (z))), vec![bind!{y: expr!((z))}]);
    }

    #[test]
    fn test_match_if_then_with_x() {
        let mut space = GroundingSpace::new();
        space.add(expr!("=" ("if" "True" then) then));
        assert_eq!(space.query(&expr!("=" ("if" "True" "42") X)),
        vec![bind!{X: expr!("42")}]);
    }

    #[test]
    fn test_match_combined_query() {
        let mut space = GroundingSpace::new();
        space.add(expr!("posesses" "Sam" "baloon"));
        space.add(expr!("likes" "Sam" ("blue" "stuff")));
        space.add(expr!("has-color" "baloon" "blue"));

        let result = space.query(&expr!("," ("posesses" "Sam" object)
        ("likes" "Sam" (color "stuff"))
        ("has-color" object color)));
        assert_eq!(result, vec![bind!{object: expr!("baloon"), color: expr!("blue")}]);
    }

    #[test]
    fn test_unify_variables_inside_conjunction_query() {
        let mut space = GroundingSpace::new();
        space.add(expr!("lst1" ("Cons" "a1" ("Cons" "b2" "b3"))));
        space.add(expr!("lst2" ("Cons" "a2" ("Cons" "b3" "b4"))));
        space.add(expr!("Concat" x1 x2 x3));

        let result = space.subst(
            &expr!("," ("lst1" l1) ("lst2" l2) ("Concat" l1 "a2" "a3")),
            &expr!(l1));
        assert_eq!(result, vec![expr!("Cons" "a1" ("Cons" "b2" "b3"))]);
    }

    #[test]
    fn test_type_check_in_query() {
        let mut space = GroundingSpace::new();
        space.add(expr!(":" "Human" "Type"));
        space.add(expr!(":" "Socrates" "Human"));
        space.add(expr!("Cons" "Socrates" "Nil"));

        let result = space.query(&expr!("," (":" h "Human") ("Cons" h t)));
        assert_eq!(result, vec![bind!{h: expr!("Socrates"), t: expr!("Nil")}]);
    }

    #[test]
    fn cleanup_observer() {
        let mut space = GroundingSpace::new();
        {
            let observer = Rc::new(RefCell::new(SpaceEventCollector::new()));
            space.register_observer(Rc::clone(&observer));
            assert_eq!(space.observers.borrow().len(), 1);
        }

        space.add(expr!("a"));

        assert_eq!(*space.borrow_vec(), vec![expr!("a")]);
        assert_eq!(space.observers.borrow().len(), 0);
    }

    #[test]
    fn complex_query_applying_bindings_to_next_pattern() {
        let mut space = GroundingSpace::new();
        space.add(expr!(":=" ("sum" a b) ("+" a b)));
        space.add(expr!(":=" "a" {4}));

        let result = space.query(&expr!("," (":=" "a" b) (":=" ("sum" {3} b) W)));

        assert_eq!(result, vec![bind!{b: expr!({4}), W: expr!("+" {3} {4})}]);
    }

    #[test]
    fn complex_query_chain_of_bindings() {
        let mut space = GroundingSpace::new();
        space.add(expr!("implies" ("B" x) ("C" x)));
        space.add(expr!("implies" ("A" x) ("B" x)));
        space.add(expr!("A" "Sam"));

        let result = space.query(&expr!("," ("implies" ("B" x) z) ("implies" ("A" x) y) ("A" x)));
        assert_eq!(result, vec![bind!{x: sym!("Sam"), y: expr!("B" "Sam"), z: expr!("C" "Sam")}]);
    }
}
