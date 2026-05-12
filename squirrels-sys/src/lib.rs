#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::{
    collections::HashMap,
    ffi::CStr,
    sync::{Arc, Mutex, OnceLock},
};

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

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

unsafe extern "C" {
    pub fn squirrels_shim_print(v: HSQUIRRELVM, fmt: *const SQChar, ...);
    pub fn squirrels_shim_error(v: HSQUIRRELVM, fmt: *const SQChar, ...);
}

type PrintCallback = Arc<dyn Fn(&str) + Send + Sync>;

#[derive(Default)]
struct PrintFns {
    print: Option<PrintCallback>,
    error: Option<PrintCallback>,
}

fn registry() -> &'static Mutex<HashMap<usize, PrintFns>> {
    static REG: OnceLock<Mutex<HashMap<usize, PrintFns>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn install_print_shims(v: HSQUIRRELVM) {
    unsafe {
        sq_setprintfunc(v, Some(squirrels_shim_print), Some(squirrels_shim_error));
    }
}

// pub fn set_print_fns<P, E>(v: HSQUIRRELVM, print: Option<P>, error: Option<E>)
// where
//     P: Fn(&str) + Send + Sync + 'static,
//     E: Fn(&str) + Send + Sync + 'static,
// {
//     {
//         let mut reg = registry().lock().unwrap();
//         let fns = reg.entry(v as usize).or_default();
//         fns.print = print.map(|f| Arc::new(f) as PrintCallback);
//         fns.error = error.map(|f| Arc::new(f) as PrintCallback);
//     }
// }

pub fn set_print_fn<F>(v: HSQUIRRELVM, f: F)
where
    F: Fn(&str) + Send + Sync + 'static,
{
    registry()
        .lock()
        .unwrap()
        .entry(v as usize)
        .or_default()
        .print = Some(Arc::new(f) as PrintCallback)
}

pub fn set_error_fn<F>(v: HSQUIRRELVM, f: F)
where
    F: Fn(&str) + Send + Sync + 'static,
{
    registry()
        .lock()
        .unwrap()
        .entry(v as usize)
        .or_default()
        .error = Some(Arc::new(f) as PrintCallback)
}

pub fn clear_print_fns(v: HSQUIRRELVM) {
    registry().lock().unwrap().remove(&(v as usize));
}

#[derive(Debug, Clone, Copy)]
enum PrintFnType {
    Print,
    Error,
}

fn dispatch(v: HSQUIRRELVM, msg: *const SQChar, t: PrintFnType) {
    let f = registry()
        .lock()
        .unwrap()
        .get(&(v as usize))
        .and_then(|fns| match t {
            PrintFnType::Print => fns.print.clone(),
            PrintFnType::Error => fns.error.clone(),
        });

    if let Some(f) = f {
        let str = unsafe { CStr::from_ptr(msg) }.to_string_lossy();
        f(&str)
    }
}

#[unsafe(no_mangle)]
extern "C" fn squirrels_dispatch_print(v: HSQUIRRELVM, msg: *const SQChar) {
    dispatch(v, msg, PrintFnType::Print)
}

#[unsafe(no_mangle)]
extern "C" fn squirrels_dispatch_error(v: HSQUIRRELVM, msg: *const SQChar) {
    dispatch(v, msg, PrintFnType::Error)
}
