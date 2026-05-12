#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

unsafe extern "C" {
    pub fn sq_shim_print(v: HSQUIRRELVM, fmt: *const SQChar, ...);
    pub fn sq_shim_error(v: HSQUIRRELVM, fmt: *const SQChar, ...);
}

pub type SqPrintFn = unsafe extern "C" fn(v: HSQUIRRELVM, msg: *const SQChar);

#[derive(Default, Clone, Copy)]
struct PrintFns {
    print: Option<SqPrintFn>,
    error: Option<SqPrintFn>,
}

/// Registry of Squirrel VM -> print functions.
fn print_fn_registry() -> &'static Mutex<HashMap<usize, PrintFns>> {
    static REG: OnceLock<Mutex<HashMap<usize, PrintFns>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

pub unsafe fn set_print_fns(v: HSQUIRRELVM, print: Option<SqPrintFn>, error: Option<SqPrintFn>) {
    print_fn_registry()
        .lock()
        .unwrap()
        .insert(v as usize, PrintFns { print, error });

    unsafe {
        sq_setprintfunc(v, Some(sq_shim_print), Some(sq_shim_error));
    }
}

pub fn clear_print_fns(v: HSQUIRRELVM) {
    print_fn_registry().lock().unwrap().remove(&(v as usize));
}

#[unsafe(no_mangle)]
extern "C" fn ffi_sq_get_print(v: HSQUIRRELVM) -> Option<SqPrintFn> {
    print_fn_registry()
        .lock()
        .unwrap()
        .get(&(v as usize))
        .and_then(|e| e.print)
}

#[unsafe(no_mangle)]
extern "C" fn ffi_sq_get_error(v: HSQUIRRELVM) -> Option<SqPrintFn> {
    print_fn_registry()
        .lock()
        .unwrap()
        .get(&(v as usize))
        .and_then(|e| e.error)
}

#[test]
fn squirrel_test() {
    use std::ffi::c_char;

    unsafe {
        let vm = sq_open(1024);

        sq_setprintfunc(vm, None, None);

        sq_pushroottable(vm);

        let script = "return 1 + 2";

        sq_compilebuffer(
            vm,
            script.as_ptr() as *const c_char,
            script.len() as SQInteger,
            c"embedded".as_ptr(),
            1,
        );

        sq_push(vm, -2);

        sq_call(vm, 1, SQTrue as _, SQTrue as _);

        let mut out: SQInteger = 0;
        sq_getinteger(vm, -1, &mut out);
        assert_eq!(out, 3);

        sq_pop(vm, 3);

        sq_close(vm);
    }
}
