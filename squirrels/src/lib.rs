mod compiler_error_handler;

use std::ffi::{CStr, c_char};

use squirrels_sys::{
    HSQUIRRELVM, SQ_VMSTATE_IDLE, SQ_VMSTATE_RUNNING, SQ_VMSTATE_SUSPENDED, SQBool, SQFalse,
    SQFloat, SQInteger, SQTrue, sq_call, sq_close, sq_compilebuffer, sq_getbool, sq_getfloat,
    sq_getinteger, sq_getvmstate, sq_open, sq_pop, sq_push, sq_pushroottable,
};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("compile error at {source_name}:{line}:{column}: description")]
    Compile {
        description: String,
        source_name: String,
        line: SQInteger,
        column: SQInteger,
    },

    #[error("expected {expected}")]
    Type { expected: &'static str },

    #[error("runtime error")]
    Runtime,
}

type Integer = SQInteger;
type Float = SQFloat;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionState {
    Idle,
    Running,
    Suspended,
}

#[derive(Debug)]
pub struct Squirrel {
    vm: HSQUIRRELVM,
}

unsafe impl Send for Squirrel {}

impl Squirrel {
    /// Initialize a new Squirrel VM.
    ///
    /// `initial_stack_size` controls the size of the stack in slots,
    /// or number of objects.
    pub fn new(initial_stack_size: usize) -> Self {
        let vm = unsafe { sq_open(initial_stack_size as _) };
        compiler_error_handler::register_vm(vm);
        squirrels_sys::install_print_shims(vm);
        Self { vm }
    }

    /// Set the print function of the virtual machine.
    ///
    /// This function is used by the builtin function `::print()`
    /// to output text.
    pub fn set_print_fn<F>(&self, f: F)
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        squirrels_sys::set_print_fn(self.vm, f);
    }

    /// Set the print error function of the virtual machine.
    pub fn set_eprint_fn<F>(&self, f: F)
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        squirrels_sys::set_error_fn(self.vm, f);
    }
}

impl Drop for Squirrel {
    fn drop(&mut self) {
        squirrels_sys::clear_print_fns(self.vm);
        unsafe {
            sq_close(self.vm);
        }
        compiler_error_handler::unregister_vm(self.vm);
    }
}

impl Squirrel {
    /// Get the execution state of this virtual machine.
    pub fn state(&self) -> ExecutionState {
        match unsafe { sq_getvmstate(self.vm) } as _ {
            SQ_VMSTATE_IDLE => ExecutionState::Idle,
            SQ_VMSTATE_RUNNING => ExecutionState::Running,
            SQ_VMSTATE_SUSPENDED => ExecutionState::Suspended,
            s => unreachable!("invalid vmstate: {s}"),
        }
    }

    /// Compile the Squirrel program in `src` and push it as a function in the stack.
    pub fn compile_str(&self, src: &str, source_name: &CStr) -> Result<()> {
        compiler_error_handler::clear_error(self.vm);

        let ret = unsafe {
            sq_compilebuffer(
                self.vm,
                src.as_ptr() as *const c_char,
                src.len() as Integer,
                source_name.as_ptr(),
                SQTrue as _,
            )
        };

        if ret < 0 {
            let e = compiler_error_handler::take_error(self.vm)
                .expect("sq_compilebuffer failed but no compile error was captured");
            Err(Error::Compile {
                description: e.description,
                source_name: e.source_name,
                line: e.line,
                column: e.column,
            })
        } else {
            Ok(())
        }
    }

    pub fn exec(&self, src: &str) -> Result<()> {
        unsafe {
            sq_pushroottable(self.vm);
        }

        if let Err(e) = self.compile_str(src, c"=eval") {
            unsafe {
                sq_pop(self.vm, 1);
            }
            return Err(e);
        }

        unsafe {
            sq_push(self.vm, -2);
        }

        let ret = unsafe { sq_call(self.vm, 1, SQTrue as _, SQFalse as _) };
        if ret < 0 {
            unsafe {
                sq_pop(self.vm, 2);
                return Err(Error::Runtime);
            }
        }

        Ok(())
    }
}

#[test]
fn compile_error_test() {
    let sq = Squirrel::new(1024);
    let err = sq.compile_str("return 1 +", c"=eval").unwrap_err();
    assert!(matches!(err, Error::Compile { .. }), "got {err:?}")
}

#[test]
fn arithmetic_test() {
    let sq = Squirrel::new(1024);
    sq.exec("return 5 + 8").unwrap();

    let val = Integer::from_top(&sq).unwrap();
    assert_eq!(val, 13);
}

#[test]
fn arithmetic_float_test() {
    let sq = Squirrel::new(1024);
    sq.exec("return 1.0 + 5.0").unwrap();

    let val = Float::from_top(&sq).unwrap();
    assert_eq!(val, 6.0)
}

#[test]
fn print_fn_test() {
    use std::sync::{Arc, Mutex};

    let str = Arc::new(Mutex::new("".to_string()));

    let sq = Squirrel::new(1024);
    sq.set_print_fn({
        let str = str.clone();
        move |s: &str| *str.lock().unwrap() = s.to_string()
    });
    sq.exec("print(\"hello\")").unwrap();

    let s = str.lock().unwrap().to_string();

    assert_eq!(&s, "hello")
}

pub trait FromSquirrel: Sized {
    fn from_top(sq: &Squirrel) -> Result<Self>;
}

impl FromSquirrel for bool {
    fn from_top(sq: &Squirrel) -> Result<Self> {
        let mut out: SQBool = 0;
        if unsafe { sq_getbool(sq.vm, -1, &mut out) } < 0 {
            return Err(Error::Type { expected: "bool" });
        }
        Ok(out != 0)
    }
}

impl FromSquirrel for Integer {
    fn from_top(sq: &Squirrel) -> Result<Self> {
        let mut out: SQInteger = 0;
        if unsafe { sq_getinteger(sq.vm, -1, &mut out) } < 0 {
            return Err(Error::Type {
                expected: "integer",
            });
        }
        Ok(out)
    }
}

impl FromSquirrel for Float {
    fn from_top(sq: &Squirrel) -> Result<Self> {
        let mut out: SQFloat = 0.0;
        if unsafe { sq_getfloat(sq.vm, -1, &mut out) } < 0 {
            return Err(Error::Type { expected: "float" });
        }
        Ok(out)
    }
}
