use std::{
    collections::HashMap,
    ffi::{CStr, c_char},
    sync::{Mutex, OnceLock},
};

use squirrels_sys::{HSQUIRRELVM, SQInteger, sq_setcompilererrorhandler};

#[derive(Debug)]
pub struct CompileError {
    pub description: String,
    pub source_name: String,
    pub line: SQInteger,
    pub column: SQInteger,
}

#[derive(Default)]
struct ErrorSlot {
    compile: Option<CompileError>,
}

fn registry() -> &'static Mutex<HashMap<usize, ErrorSlot>> {
    static REG: OnceLock<Mutex<HashMap<usize, ErrorSlot>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn register_vm(v: HSQUIRRELVM) {
    registry()
        .lock()
        .unwrap()
        .insert(v as usize, ErrorSlot::default());

    unsafe {
        sq_setcompilererrorhandler(v, Some(compiler_error_handler));
    }
}

pub fn unregister_vm(v: HSQUIRRELVM) {
    registry().lock().unwrap().remove(&(v as usize));
}

pub fn take_error(v: HSQUIRRELVM) -> Option<CompileError> {
    registry()
        .lock()
        .unwrap()
        .get_mut(&(v as usize))?
        .compile
        .take()
}

pub fn clear_error(v: HSQUIRRELVM) {
    if let Some(s) = registry().lock().unwrap().get_mut(&(v as usize)) {
        s.compile = None;
    }
}

unsafe extern "C" fn compiler_error_handler(
    v: HSQUIRRELVM,
    description: *const c_char,
    source: *const c_char,
    line: SQInteger,
    column: SQInteger,
) {
    let description = unsafe { CStr::from_ptr(description) }
        .to_string_lossy()
        .into_owned();
    let source = unsafe { CStr::from_ptr(source) }
        .to_string_lossy()
        .into_owned();
    if let Some(s) = registry().lock().unwrap().get_mut(&(v as usize)) {
        s.compile = Some(CompileError {
            description,
            source_name: source,
            line,
            column,
        })
    }
}
