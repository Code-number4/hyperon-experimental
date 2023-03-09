use hyperon::*;

use crate::util::*;

use std::os::raw::*;
use std::fmt::Display;
use std::sync::atomic::{AtomicPtr, Ordering};

use hyperon::matcher::Bindings;
use std::mem;


// Atom

#[repr(C)]
pub enum atom_type_t {
    SYMBOL,
    VARIABLE,
    EXPR,
    GROUNDED,
}

pub struct atom_t {
    pub atom: Atom,
}

pub struct exec_error_t {
    pub error: ExecError,
}

#[repr(C)]
pub struct var_atom_t {
    pub var: *const c_char,
    pub atom: *mut atom_t,
}

pub struct bindings_t {
    pub bindings: Bindings,
}

#[repr(C)]
pub struct gnd_api_t {
    // TODO: replace args by C array and ret by callback
    // One can assign NULL to this field, it means the atom is not executable
    execute: Option<extern "C" fn(*const gnd_t, *mut vec_atom_t, *mut vec_atom_t) -> *mut exec_error_t>,
    match_: Option<extern "C" fn(*const gnd_t, *const atom_t, lambda_t<* const bindings_t>, *mut c_void)>,
    eq: extern "C" fn(*const gnd_t, *const gnd_t) -> bool,
    clone: extern "C" fn(*const gnd_t) -> *mut gnd_t,
    display: extern "C" fn(*const gnd_t, *mut c_char, usize) -> usize,
    free: extern "C" fn(*mut gnd_t),
}

#[repr(C)]
pub struct gnd_t {
    api: *const gnd_api_t,
    typ: *mut atom_t,
}

#[no_mangle]
pub extern "C" fn exec_error_runtime(message: *const c_char) -> *mut exec_error_t {
    Box::into_raw(Box::new(exec_error_t{ error: ExecError::Runtime(cstr_into_string(message)) }))
}

#[no_mangle]
pub extern "C" fn exec_error_no_reduce() -> *mut exec_error_t {
    Box::into_raw(Box::new(exec_error_t{ error: ExecError::NoReduce }))
}

#[no_mangle]
pub extern "C" fn exec_error_free(error: *mut exec_error_t) {
    unsafe{ drop(Box::from_raw(error)); }
}

// bindings
#[no_mangle]
pub unsafe extern "C" fn bindings_new() -> *mut bindings_t {
    // cstr_as_str() keeps pointer ownership, but Atom::sym() copies resulting
    // String into Atom::Symbol::symbol field. atom_into_ptr() moves value to the
    // heap and gives ownership to the caller.
    bindings_into_ptr(Bindings::new())
}

#[no_mangle]
pub unsafe extern "C" fn bindings_traverse(bindings: * const bindings_t, callback: lambda_t<* const var_atom_t>, context: *mut c_void) {
    (*bindings).bindings.iter().for_each(|(var, atom)|  {
            let name = string_as_cstr(var.name());
            let var_atom = var_atom_t {
                var: name.as_ptr(),
                atom: atom_into_ptr(atom)
            };
            callback(&var_atom, context);
        }
    )
}

#[no_mangle]
pub unsafe extern "C" fn bindings_add_var_binding(bindings: * mut bindings_t, atom: *const var_atom_t) -> bool {
    let bindings = &mut(*bindings).bindings;
    let var = VariableAtom::new(cstr_into_string ((*atom).var));
    let atom = (*(*atom).atom).atom.clone();
    bindings.add_var_binding(var, atom)
}

// end of bindings

#[no_mangle]
pub unsafe extern "C" fn atom_sym(name: *const c_char) -> *mut atom_t {
    // cstr_as_str() keeps pointer ownership, but Atom::sym() copies resulting
    // String into Atom::Symbol::symbol field. atom_into_ptr() moves value to the
    // heap and gives ownership to the caller.
    atom_into_ptr(Atom::sym(cstr_as_str(name)))
}

#[no_mangle]
pub unsafe extern "C" fn atom_expr(children: *const *mut atom_t, size: usize) -> *mut atom_t {
    let c_arr = std::slice::from_raw_parts(children, size);
    let children: Vec<Atom> = c_arr.into_iter().map(|atom| {
        ptr_into_atom(*atom)
    }).collect();
    atom_into_ptr(Atom::expr(children))
}

#[no_mangle]
pub unsafe extern "C" fn atom_var(name: *const c_char) -> *mut atom_t {
    atom_into_ptr(Atom::var(cstr_as_str(name)))
}

#[no_mangle]
pub extern "C" fn atom_gnd(gnd: *mut gnd_t) -> *mut atom_t {
    atom_into_ptr(Atom::gnd(CGrounded(AtomicPtr::new(gnd))))
}

#[no_mangle]
pub unsafe extern "C" fn atom_get_type(atom: *const atom_t) -> atom_type_t {
    match (*atom).atom {
        Atom::Symbol(_) => atom_type_t::SYMBOL,
        Atom::Variable(_) => atom_type_t::VARIABLE,
        Atom::Expression(_) => atom_type_t::EXPR,
        Atom::Grounded(_) => atom_type_t::GROUNDED,
    }
}

#[no_mangle]
pub extern "C" fn atom_to_str(atom: *const atom_t, callback: c_str_callback_t, context: *mut c_void) {
    println!("atom_to_str rs\n");
    let atom = unsafe{ &(*atom).atom };
    println!("atom_to_str atom rs\n");
    let s = str_as_cstr(atom.to_string().as_str());
    println!("atom_to_str s rs\n");
    callback(s.as_ptr(), context);
    println!("atom_to_str rs end \n");
}


#[no_mangle]
pub extern "C" fn atom_get_name(atom: *const atom_t, callback: c_str_callback_t, context: *mut c_void) {
    let atom = unsafe{ &(*atom).atom };
    match atom {
        Atom::Symbol(s) => callback(str_as_cstr(s.name()).as_ptr(), context),
        Atom::Variable(v) => callback(string_as_cstr(v.name()).as_ptr(), context),
        _ => panic!("Only Symbol and Variable has name attribute!"),
    }
}

#[no_mangle]
pub unsafe extern "C" fn atom_get_object(atom: *const atom_t) -> *mut gnd_t {
    if let Atom::Grounded(ref g) = (*atom).atom {
        match (*g).as_any_ref().downcast_ref::<CGrounded>() {
            Some(g) => g.get_mut_ptr(),
            None => panic!("Returning non C grounded objects is not implemented yet!"),
        }
    } else {
        panic!("Only Grounded has object attribute!");
    }
}

#[no_mangle]
pub extern "C" fn atom_get_grounded_type(atom: *const atom_t) -> *mut atom_t {
    if let Atom::Grounded(ref g) = unsafe{ &(*atom) }.atom {
        atom_into_ptr(g.type_())
    } else {
        panic!("Only Grounded atoms has grounded type attribute!");
    }
}

#[no_mangle]
pub unsafe extern "C" fn atom_get_children(atom: *const atom_t,
        callback: c_atoms_callback_t, context: *mut c_void) {
    if let Atom::Expression(ref e) = (*atom).atom {
        return_atoms(e.children(), callback, context);
    } else {
        panic!("Only Expression has children!");
    }
}

#[no_mangle]
pub unsafe extern "C" fn atom_free(atom: *mut atom_t) {
    // drop() does nothing actually, but it is used here for clarity
    drop(Box::from_raw(atom));
}

#[no_mangle]
pub extern "C" fn atom_clone(atom: *const atom_t) -> *mut atom_t {
    atom_into_ptr(unsafe{ &(*atom) }.atom.clone())
}

#[no_mangle]
pub unsafe extern "C" fn atom_eq(atoma: *const atom_t, atomb: *const atom_t) -> bool {
    (*atoma).atom == (*atomb).atom
}

// TODO: make a macros to generate Vec<T> definitions for C API
pub struct vec_atom_t(Vec<Atom>);

#[no_mangle]
pub extern "C" fn vec_atom_new() -> *mut vec_atom_t {
    vec_atom_into_ptr(Vec::new()) 
}

#[no_mangle]
pub unsafe extern "C" fn vec_atom_free(vec: *mut vec_atom_t) {
    drop(Box::from_raw(vec));
}

#[no_mangle]
pub unsafe extern "C" fn vec_atom_size(vec: *mut vec_atom_t) -> usize {
    (*vec).0.len()
}

#[no_mangle]
pub unsafe extern "C" fn vec_atom_pop(vec: *mut vec_atom_t) -> *mut atom_t {
    atom_into_ptr((*vec).0.pop().expect("Vector is empty"))
}

#[no_mangle]
pub unsafe extern "C" fn vec_atom_push(vec: *mut vec_atom_t, atom: *mut atom_t) {
    (*vec).0.push(ptr_into_atom(atom));
}

#[no_mangle]
pub unsafe extern "C" fn vec_atom_len(vec: *const vec_atom_t) -> usize {
    (*vec).0.len()
}

#[no_mangle]
pub unsafe extern "C" fn vec_atom_get(vec: *mut vec_atom_t, idx: usize) -> *mut atom_t {
    atom_into_ptr((*vec).0[idx].clone())
}

pub type atom_array_t = array_t<*const atom_t>;
pub type c_atoms_callback_t = lambda_t<atom_array_t>;

#[no_mangle]
pub extern "C" fn atoms_are_equivalent(first: *const atom_t, second: *const atom_t) -> bool {
    crate::atom::matcher::atoms_are_equivalent(&unsafe{ &*first }.atom, &unsafe{ &*second }.atom)
}

/////////////////////////////////////////////////////////////////
// Code below is a boilerplate code to implement C API correctly.
// It is not a part of C API.

pub fn atom_into_ptr(atom: Atom) -> *mut atom_t {
    Box::into_raw(Box::new(atom_t{ atom }))
}

pub fn bindings_into_ptr(bindings: Bindings) -> *mut bindings_t {
    Box::into_raw(Box::new(bindings_t{bindings}))
}

pub fn ptr_into_atom(atom: *mut atom_t) -> Atom {
    unsafe{ Box::from_raw(atom) }.atom
}

pub fn ptr_into_bindings(bindings: *mut bindings_t) -> Bindings {
    unsafe {Box::from_raw(bindings)}.bindings
}

fn vec_atom_into_ptr(vec: Vec<Atom>) -> *mut vec_atom_t {
    Box::into_raw(Box::new(vec_atom_t(vec)))
}

pub fn return_atoms(atoms: &Vec<Atom>, callback: c_atoms_callback_t, context: *mut c_void) {
    let results: Vec<*const atom_t> = atoms.iter()
        .map(|atom| (atom as *const Atom).cast::<atom_t>()).collect();
    callback((&results).into(), context);
}

// C grounded atom wrapper

#[derive(Debug)]
struct CGrounded(AtomicPtr<gnd_t>);

impl CGrounded {
    fn get_mut_ptr(&self) -> *mut gnd_t {
        self.0.load(Ordering::Acquire)
    }

    fn get_ptr(&self) -> *const gnd_t {
        self.0.load(Ordering::Acquire)
    }

    fn api(&self) -> &gnd_api_t {
        unsafe {
            &*(*self.get_ptr()).api
        }
    }

    fn free(&mut self) {
        (self.api().free)(self.get_mut_ptr());
    }

    extern "C" fn match_callback(cbindings: *const bindings_t, context: *mut c_void) {
        // todo: check for optimality
        let bindings = unsafe{ (*cbindings).bindings.clone() };
        mem::forget(cbindings);

        let vec_bnd = unsafe{ &mut *context.cast::<Vec<Bindings>>() };
        vec_bnd.push(bindings);
    }

}


impl Grounded for CGrounded {
    fn type_(&self) -> Atom {
        unsafe{ &*(*self.get_ptr()).typ }.atom.clone()
    }

    fn execute(&self, args: &mut Vec<Atom>) -> Result<Vec<Atom>, ExecError> {
        let mut ret = Vec::new();
        match self.api().execute {
            Some(func) => {
                let error = func(self.get_ptr(), (args as *mut Vec<Atom>).cast::<vec_atom_t>(),
                (&mut ret as *mut Vec<Atom>).cast::<vec_atom_t>());
                let ret = if error.is_null() {
                    Ok(ret)
                } else {
                    let error = unsafe{ Box::from_raw(error) };
                    Err(error.error)
                };
                log::trace!("CGrounded::execute: atom: {:?}, args: {:?}, ret: {:?}", self, args, ret);
                ret
            },
            None => execute_not_executable(self)
        }
    }

    fn match_(&self, other: &Atom) -> matcher::MatchResultIter {
        match self.api().match_ {
            Some(func) => {
                let mut results: Vec<Bindings> = Vec::new();
                let context = (&mut results as *mut Vec<Bindings>).cast::<c_void>();
                func(self.get_ptr(), (other as *const Atom).cast::<atom_t>(),
                    CGrounded::match_callback, context);
                Box::new(results.into_iter())
            },
            None => match_by_equality(self, other)
        }
    }
}

impl PartialEq for CGrounded {
    fn eq(&self, other: &CGrounded) -> bool {
        (self.api().eq)(self.get_ptr(), other.get_ptr())
    }
}

impl Clone for CGrounded {
    fn clone(&self) -> Self {
        CGrounded(AtomicPtr::new((self.api().clone)(self.get_ptr())))
    }
}

impl Display for CGrounded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut buffer = [0u8; 4096];
        (self.api().display)(self.get_ptr(), buffer.as_mut_ptr().cast::<c_char>(), 4096);
        let text = cstr_into_string(buffer.as_ptr().cast::<c_char>());
        write!(f, "{}", text)
    }
}

impl Drop for CGrounded {
    fn drop(&mut self) {
        self.free();
    }
}

#[cfg(test)]
mod tests {
use super::*;
use std::ptr;

    #[test]
    pub fn test_match_callback() {
        let var = VariableAtom::new_id ("var", 1);
        let atom = Atom::sym("atom_test");
        let mut bindings= Bindings::new();
        bindings.add_var_binding(var, atom);
        let cbindings = bindings_t{bindings };

        let mut results: Vec<Bindings> = Vec::new();
        let context = ptr::addr_of_mut!(results).cast::<c_void>();

        CGrounded::match_callback(&cbindings, context);

        assert_eq!(results, vec![Bindings::from(vec![
                (VariableAtom::new_id("var", 1), Atom::sym("atom_test"))])]);
    }

}
