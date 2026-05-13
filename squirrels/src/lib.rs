mod compiler_error_handler;

use std::ffi::{CStr, c_char, c_void};

use squirrels_sys::{
    HSQOBJECT, HSQUIRRELVM, SQ_VMSTATE_IDLE, SQ_VMSTATE_RUNNING, SQ_VMSTATE_SUSPENDED, SQBool,
    SQChar, SQFalse, SQFloat, SQInteger, SQTrue, SQUnsignedInteger, sq_addref, sq_call, sq_close,
    sq_compilebuffer, sq_getbool, sq_getfloat, sq_getinteger, sq_getrefcount, sq_getstackobj,
    sq_getstringandsize, sq_gettop, sq_getvmstate, sq_open, sq_pop, sq_push, sq_pushobject,
    sq_pushroottable, sq_release, sq_resetobject, tagSQObjectType_OT_ARRAY,
    tagSQObjectType_OT_BOOL, tagSQObjectType_OT_CLASS, tagSQObjectType_OT_CLOSURE,
    tagSQObjectType_OT_FLOAT, tagSQObjectType_OT_GENERATOR, tagSQObjectType_OT_INSTANCE,
    tagSQObjectType_OT_INTEGER, tagSQObjectType_OT_NATIVECLOSURE, tagSQObjectType_OT_NULL,
    tagSQObjectType_OT_STRING, tagSQObjectType_OT_TABLE, tagSQObjectType_OT_THREAD,
    tagSQObjectType_OT_USERDATA, tagSQObjectType_OT_USERPOINTER, tagSQObjectType_OT_WEAKREF,
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
type UnsignedInteger = SQUnsignedInteger;
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
    pub fn new(initial_stack_size: Integer) -> Self {
        let vm = unsafe { sq_open(initial_stack_size) };
        assert!(!vm.is_null(), "sq_open returned a null vm");

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
        unsafe { sq_close(self.vm) };
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

        if ret.is_error() {
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

    // TODO: decide whether to leave the stack empty when `exec` is done.
    // Right now it does not pop [roottable, compiled_closure].
    pub fn exec(&self, src: &str) -> Result<()> {
        unsafe { sq_pushroottable(self.vm) };

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
        if ret.is_error() {
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

fn assert_valid_stack_idx(vm: HSQUIRRELVM, idx: SQInteger) {
    let top = unsafe { sq_gettop(vm) };
    let valid = idx != 0 && if idx > 0 { idx <= top } else { idx >= -top };
    assert!(valid, "invalid stack index {idx} (top={top})")
}

/// A handle to a Squirrel ref-counted object.
pub struct ObjectHandle<'vm> {
    vm: &'vm Squirrel,
    obj: HSQOBJECT,
}

impl<'vm> ObjectHandle<'vm> {
    pub(crate) fn from_stack(sq: &'vm Squirrel, idx: SQInteger) -> Self {
        assert_valid_stack_idx(sq.vm, idx);

        // Initialize the object
        let mut obj: HSQOBJECT = unsafe { std::mem::zeroed() };
        unsafe { sq_resetobject(&mut obj) };

        // Get it from the stack
        let ret = unsafe { sq_getstackobj(sq.vm, idx, &mut obj) };
        assert!(!ret.is_error(), "sq_getstackobj failed for idx {idx}");

        // Increment the refcount
        unsafe { sq_addref(sq.vm, &mut obj) };

        Self { vm: sq, obj }
    }

    pub(crate) fn push(&self) {
        unsafe { sq_pushobject(self.vm.vm, self.obj) };
    }

    /// Get the ref count of this object.
    pub fn ref_count(&self) -> UnsignedInteger {
        let mut obj = self.obj;
        unsafe { sq_getrefcount(self.vm.vm, &mut obj) }
    }

    #[allow(non_upper_case_globals)]
    pub fn to_value(self) -> Value<'vm> {
        match self.obj._type {
            tagSQObjectType_OT_NULL => Value::Null,
            tagSQObjectType_OT_INTEGER => Value::Integer(unsafe { self.obj._unVal.nInteger }),
            tagSQObjectType_OT_FLOAT => Value::Float(unsafe { self.obj._unVal.fFloat }),
            tagSQObjectType_OT_BOOL => Value::Bool(unsafe { self.obj._unVal.nInteger } != 0),
            tagSQObjectType_OT_STRING => Value::String(
                SqString::from_handle(self).expect("OT_STRING handle materializes as SqString"),
            ),
            tagSQObjectType_OT_TABLE => Value::Table(Table(self)),
            tagSQObjectType_OT_ARRAY => Value::Array(Array(self)),
            tagSQObjectType_OT_USERDATA => Value::UserData(UserData(self)),
            tagSQObjectType_OT_CLOSURE => Value::Closure(Closure(self)),
            tagSQObjectType_OT_NATIVECLOSURE => Value::NativeClosure(NativeClosure(self)),
            tagSQObjectType_OT_GENERATOR => Value::Generator(Generator(self)),
            tagSQObjectType_OT_USERPOINTER => {
                Value::UserPointer(UserPointer(unsafe { self.obj._unVal.pUserPointer }))
            }
            tagSQObjectType_OT_THREAD => Value::Thread(Thread(self)),
            tagSQObjectType_OT_CLASS => Value::Class(Class(self)),
            tagSQObjectType_OT_INSTANCE => Value::Instance(Instance(self)),
            tagSQObjectType_OT_WEAKREF => Value::WeakRef(WeakRef(self)),
            t => panic!("Squirrel VM returned an invalid object type: {t:?}"),
        }
    }
}

impl Drop for ObjectHandle<'_> {
    fn drop(&mut self) {
        unsafe { sq_release(self.vm.vm, &mut self.obj) };
    }
}

pub struct SqString<'vm> {
    handle: ObjectHandle<'vm>,
    ptr: *const SQChar,
    len: usize,
}

impl<'vm> SqString<'vm> {
    pub(crate) fn from_handle(handle: ObjectHandle<'vm>) -> Result<Self> {
        if handle.obj._type != tagSQObjectType_OT_STRING {
            return Err(Error::Type { expected: "string" });
        }

        // First we must push the string onto the stack because we can't get its stack index
        // from its handle, if it has any.
        handle.push();

        let mut ptr: *const SQChar = std::ptr::null();
        let mut len: SQInteger = 0;
        let ret = unsafe { sq_getstringandsize(handle.vm.vm, -1, &mut ptr, &mut len) };

        // Pop before we check for an error to avoid leaving the stack in an invalid state.
        unsafe { sq_pop(handle.vm.vm, 1) };

        assert!(
            !ret.is_error(),
            "sq_getstringandsize failed on a verified OT_STRING"
        );

        Ok(Self {
            handle,
            ptr,
            len: len as usize,
        })
    }

    pub(crate) fn from_stack(sq: &'vm Squirrel, idx: SQInteger) -> Result<Self> {
        let handle = ObjectHandle::from_stack(sq, idx);
        if handle.obj._type != tagSQObjectType_OT_STRING {
            return Err(Error::Type { expected: "string" });
        }

        let mut ptr: *const SQChar = std::ptr::null();
        let mut len: SQInteger = 0;
        let ret = unsafe { sq_getstringandsize(sq.vm, idx, &mut ptr, &mut len) };
        assert!(
            !ret.is_error(),
            "sq_getstringandsize failed on a verified OT_STRING"
        );

        Ok(Self {
            handle,
            ptr,
            len: len as usize,
        })
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr as *const u8, self.len) }
    }

    pub fn to_str(&self) -> std::result::Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(self.as_bytes())
    }
}

#[test]
fn test_string_from_stack() {
    let sq = Squirrel::new(1024);
    sq.exec("return \"test\"").unwrap();

    let str = SqString::from_top(&sq).unwrap();
    assert_eq!(str.to_str().unwrap(), "test");
}

#[test]
fn test_value_from_object_handle() {
    let sq = Squirrel::new(1024);
    sq.exec("return 123").unwrap();

    let v = ObjectHandle::from_stack(&sq, -1).to_value();
    assert!(matches!(v, Value::Integer(123)));
}

pub struct Table<'vm>(ObjectHandle<'vm>);

pub struct Array<'vm>(ObjectHandle<'vm>);

pub struct UserData<'vm>(ObjectHandle<'vm>);

pub struct Closure<'vm>(ObjectHandle<'vm>);

pub struct NativeClosure<'vm>(ObjectHandle<'vm>);

pub struct Generator<'vm>(ObjectHandle<'vm>);

pub struct UserPointer(*mut c_void);

pub struct Thread<'vm>(ObjectHandle<'vm>);

pub struct Class<'vm>(ObjectHandle<'vm>);

pub struct Instance<'vm>(ObjectHandle<'vm>);

pub struct WeakRef<'vm>(ObjectHandle<'vm>);

pub enum Value<'vm> {
    Null,
    Integer(Integer),
    Float(Float),
    Bool(bool),
    String(SqString<'vm>),
    Table(Table<'vm>),
    Array(Array<'vm>),
    UserData(UserData<'vm>),
    Closure(Closure<'vm>),
    NativeClosure(NativeClosure<'vm>),
    Generator(Generator<'vm>),
    UserPointer(UserPointer),
    Thread(Thread<'vm>),
    Class(Class<'vm>),
    Instance(Instance<'vm>),
    WeakRef(WeakRef<'vm>),
}

// TODO should this trait be public?
// If we call `from_top` on an empty stack we panic.
pub trait FromSquirrel<'vm>: Sized {
    fn from_top(sq: &'vm Squirrel) -> Result<Self>;
}

impl FromSquirrel<'_> for bool {
    fn from_top(sq: &Squirrel) -> Result<Self> {
        assert_valid_stack_idx(sq.vm, -1);

        let mut out: SQBool = 0;
        if unsafe { sq_getbool(sq.vm, -1, &mut out) }.is_error() {
            return Err(Error::Type { expected: "bool" });
        }
        Ok(out != 0)
    }
}

impl FromSquirrel<'_> for Integer {
    fn from_top(sq: &Squirrel) -> Result<Self> {
        assert_valid_stack_idx(sq.vm, -1);

        let mut out: SQInteger = 0;
        if unsafe { sq_getinteger(sq.vm, -1, &mut out) }.is_error() {
            return Err(Error::Type {
                expected: "integer",
            });
        }
        Ok(out)
    }
}

impl FromSquirrel<'_> for Float {
    fn from_top(sq: &Squirrel) -> Result<Self> {
        assert_valid_stack_idx(sq.vm, -1);

        let mut out: SQFloat = 0.0;
        if unsafe { sq_getfloat(sq.vm, -1, &mut out) }.is_error() {
            return Err(Error::Type { expected: "float" });
        }
        Ok(out)
    }
}

impl<'vm> FromSquirrel<'vm> for SqString<'vm> {
    fn from_top(sq: &'vm Squirrel) -> Result<Self> {
        SqString::from_stack(sq, -1)
    }
}

impl<'vm> FromSquirrel<'vm> for Value<'vm> {
    fn from_top(sq: &'vm Squirrel) -> Result<Self> {
        Ok(ObjectHandle::from_stack(sq, -1).to_value())
    }
}
