mod array;
mod closure;
mod compiler_error_handler;
mod object;
mod string;
mod table;
mod traits;
mod value;

use std::{
    ffi::{CStr, CString, c_char, c_void},
    panic::{AssertUnwindSafe, catch_unwind},
};

use squirrels_sys::{
    HSQOBJECT, HSQUIRRELVM, SQ_VMSTATE_IDLE, SQ_VMSTATE_RUNNING, SQ_VMSTATE_SUSPENDED, SQFalse,
    SQFloat, SQInteger, SQTrue, SQUnsignedInteger, SQUserPointer, sq_addref, sq_call, sq_close,
    sq_compilebuffer, sq_getlasterror, sq_getstackobj, sq_gettop, sq_getuserdata, sq_getvmstate,
    sq_newclosure, sq_newuserdata, sq_open, sq_pop, sq_push, sq_pushbool, sq_pushfloat,
    sq_pushinteger, sq_pushnull, sq_pushobject, sq_pushroottable, sq_pushuserpointer, sq_release,
    sq_resetobject, sq_setreleasehook, sq_throwerror, sq_throwobject, tagSQObjectType_OT_CLASS,
    tagSQObjectType_OT_GENERATOR, tagSQObjectType_OT_INSTANCE, tagSQObjectType_OT_THREAD,
    tagSQObjectType_OT_USERDATA, tagSQObjectType_OT_WEAKREF,
};

pub use crate::array::Array;
pub use crate::closure::{Closure, NativeClosure};
pub use crate::string::String;
pub use crate::table::Table;
pub use crate::traits::{FromArgs, FromSquirrel, IntoArgs, IntoSquirrel, PushIntoStack};
pub use crate::value::Value;

pub(crate) use crate::object::{Object, ObjectType};

pub type Result<T> = std::result::Result<T, Error>;

pub type CallResult<'vm, T> = std::result::Result<T, CallError<'vm>>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("compile error at {source_name}:{line}:{column}: description")]
    Compile {
        description: std::string::String,
        source_name: std::string::String,
        line: SQInteger,
        column: SQInteger,
    },

    #[error("expected {expected}")]
    Type { expected: &'static str },
}

#[derive(Debug, thiserror::Error)]
pub enum CallError<'vm> {
    #[error("runtime error: {0:?}")]
    Runtime(Value<'vm>),

    #[error(transparent)]
    Other(#[from] Error),
}

pub type Integer = SQInteger;
pub type Float = SQFloat;

type UnsignedInteger = SQUnsignedInteger;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionState {
    Idle,
    Running,
    Suspended,
}

pub struct Squirrel {
    vm: HSQUIRRELVM,
    root: HSQOBJECT,
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

        // Get the root table so we can cache it.
        let mut root: HSQOBJECT = unsafe { std::mem::zeroed() };
        unsafe {
            sq_resetobject(&mut root);
            sq_pushroottable(vm);
            let ret = sq_getstackobj(vm, -1, &mut root);
            assert!(
                !ret.is_error(),
                "failed to get the root table right after pushing it"
            );
            sq_addref(vm, &mut root);
            sq_pop(vm, 1);
        };

        compiler_error_handler::register_vm(vm);
        squirrels_sys::install_print_shims(vm);
        Self { vm, root }
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

    // TODO replace this hack with a SquirrelRef type or something similar
    pub(crate) unsafe fn from_raw_borrowed(vm: HSQUIRRELVM) -> std::mem::ManuallyDrop<Self> {
        let mut root: HSQOBJECT = unsafe { std::mem::zeroed() };
        unsafe {
            sq_resetobject(&mut root);
            sq_pushroottable(vm);
            let _ = sq_getstackobj(vm, -1, &mut root);
            sq_pop(vm, 1);
            // not calling sq_addref intentionally
        }
        std::mem::ManuallyDrop::new(Squirrel { vm, root })
    }
}

impl Drop for Squirrel {
    fn drop(&mut self) {
        squirrels_sys::clear_print_fns(self.vm);
        unsafe { sq_release(self.vm, &mut self.root) };
        unsafe { sq_close(self.vm) };
        compiler_error_handler::unregister_vm(self.vm);
    }
}

pub(crate) fn get_runtime_error(sq: &Squirrel) -> Value<'_> {
    unsafe { sq_getlasterror(sq.vm) };
    let err = Object::from_stack(-1, sq);
    sq.pop(1);
    err.into_value()
}

extern "C" fn closure_release_hook<F>(payload: SQUserPointer, _size: SQInteger) -> SQInteger {
    let raw_f: *mut F = unsafe { *(payload as *mut *mut F) };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        drop(unsafe { Box::from_raw(raw_f) });
    }));
    1
}

extern "C" fn closure_trampoline<'vm, F, A, R>(v: HSQUIRRELVM) -> SQInteger
where
    F: Fn(A) -> std::result::Result<R, Value<'vm>> + Send + Sync + 'static,
    A: for<'a> FromArgs<'a>,
    R: IntoSquirrel<'vm>,
{
    let result = catch_unwind(AssertUnwindSafe(|| {
        let top = unsafe { sq_gettop(v) };

        let mut user_data: SQUserPointer = std::ptr::null_mut();
        let ret = unsafe { sq_getuserdata(v, top, &mut user_data, std::ptr::null_mut()) };
        if ret.is_error() {
            let msg = c"expected userdata on the top of the stack";
            return unsafe { sq_throwerror(v, msg.as_ptr()) }.0;
        }
        let f: &F = unsafe { &*(*(user_data as *const *const F)) };

        let sq = unsafe { Squirrel::from_raw_borrowed(v) };
        let sq: &Squirrel = &*sq;

        let args = match A::from_args(top - 2, sq) {
            Ok(a) => a,
            Err(e) => {
                let msg = CString::new(e.to_string())
                    .unwrap_or_else(|_| c"native function arg extraction failed".to_owned());
                return unsafe { sq_throwerror(v, msg.as_ptr()) }.0;
            }
        };

        match f(args) {
            Ok(r) => {
                r.push_into_stack(sq);
                1
            }
            Err(value) => {
                value.push_into_stack(sq);
                unsafe { sq_throwobject(v) }.0
            }
        }
    }));

    match result {
        Ok(ret) => ret,
        Err(_) => {
            let msg = c"panic in native function";
            unsafe { sq_throwerror(v, msg.as_ptr()) }.0
        }
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

    pub fn stack_depth(&self) -> Integer {
        unsafe { sq_gettop(self.vm) }
    }

    /// Compile the Squirrel program in `src` and push it as a closure on the stack.
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

    /// Evaluate the Squirrel program in `src` and return its output value.
    pub fn eval<'vm, T: FromSquirrel<'vm>>(&'vm self, src: &str) -> CallResult<'vm, T> {
        self.push_root_table();

        if let Err(e) = self.compile_str(src, c"=eval") {
            self.pop(1);
            return Err(e.into());
        }

        // Push the root table again to use as the argument
        // for the compiled closure.
        unsafe { sq_push(self.vm, -2) };

        let ret = unsafe { sq_call(self.vm, 1, SQTrue as _, SQFalse as _) };
        if ret.is_error() {
            self.pop(2);

            return Err(CallError::Runtime(get_runtime_error(self)));
        }

        let val = unsafe { T::from_stack(-1, self) };
        self.pop(3);
        Ok(val?)
    }

    pub fn root_table(&self) -> Table<'_> {
        let mut root = self.root;
        unsafe { sq_addref(self.vm, &mut root) };
        Table(Object {
            sq: self,
            obj: root,
        })
    }

    pub(crate) fn push_root_table(&self) {
        unsafe { sq_pushobject(self.vm, self.root) };
    }

    pub(crate) fn pop(&self, count: Integer) {
        let stack_depth = self.stack_depth();
        assert!(
            count <= stack_depth,
            "attempted to pop {count} elements but the stack has {stack_depth}"
        );
        assert!(count > 0);
        unsafe { sq_pop(self.vm, count) };
    }

    pub(crate) fn assert_valid_idx(&self, idx: Integer) {
        let top = self.stack_depth();
        let valid = idx != 0 && if idx > 0 { idx <= top } else { idx >= -top };
        assert!(valid, "invalid stack index {idx} (top={top})")
    }

    pub(crate) fn assert_same_vm(&self, other: &Self) {
        let equal = std::ptr::eq(self.vm, other.vm);
        assert!(equal, "attempted to use a value from another VM")
    }

    // NOTE: closures created by this function do not allow Squirrel objects to be
    // `move`d inside them because that implies `Object` can be `Send` and, in turn,
    // `Squirrel` to be `Sync`, because dropping a reference to an object has to
    // pass through `sq_release(&vm)` which cannot be called across threads.
    //
    // TODO figure out a way around this. Using Squirrel's registry seems like
    // the best answer.
    pub fn create_function<'vm, F, A, R>(&'vm self, f: F) -> NativeClosure<'vm>
    where
        F: Fn(A) -> std::result::Result<R, Value<'vm>> + Send + Sync + 'static,
        A: for<'a> FromArgs<'a>,
        R: IntoSquirrel<'vm>,
    {
        let fn_ptr: *mut F = Box::into_raw(Box::new(f));

        let user_data = unsafe { sq_newuserdata(self.vm, size_of::<*mut F>() as _) };

        unsafe { *(user_data as *mut *mut F) = fn_ptr };

        unsafe { sq_setreleasehook(self.vm, -1, Some(closure_release_hook::<F>)) };

        unsafe { sq_newclosure(self.vm, Some(closure_trampoline::<'vm, F, A, R>), 1) };

        let nc = NativeClosure(Object::from_stack(-1, self));
        self.pop(1);
        nc
    }

    pub fn push_value(&self, value: &Value<'_>) {
        match value {
            Value::Null => unsafe { sq_pushnull(self.vm) },
            Value::Integer(n) => unsafe { sq_pushinteger(self.vm, *n) },
            Value::Float(n) => unsafe { sq_pushfloat(self.vm, *n) },
            Value::Bool(b) => unsafe { sq_pushbool(self.vm, if *b { 1 } else { 0 }) },
            Value::UserPointer(p) => unsafe { sq_pushuserpointer(self.vm, p.0) },
            Value::String(String { obj, .. })
            | Value::Table(Table(obj))
            | Value::Array(Array(obj))
            | Value::UserData(UserData(obj))
            | Value::Closure(Closure(obj))
            | Value::NativeClosure(NativeClosure(obj))
            | Value::Generator(Generator(obj))
            | Value::Thread(Thread(obj))
            | Value::Class(Class(obj))
            | Value::Instance(Instance(obj))
            | Value::WeakRef(WeakRef(obj)) => {
                self.assert_same_vm(obj.sq);
                obj.push_into_stack()
            }
        }
    }
}

#[test]
fn compile_error_test() {
    let sq = Squirrel::new(1024);
    let err = sq.compile_str("return 1 +", c"=eval").unwrap_err();
    assert!(matches!(err, Error::Compile { .. }), "got {err:?}");
}

#[test]
fn arithmetic_test() {
    let sq = Squirrel::new(1024);
    let val = sq.eval::<Integer>("return 5 + 8").unwrap();

    assert_eq!(val, 13);
}

#[test]
fn arithmetic_float_test() {
    let sq = Squirrel::new(1024);
    let val = sq.eval::<Float>("return 1.0 + 5.0").unwrap();
    assert_eq!(val, 6.0);
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

    sq.eval::<()>("print(\"hello\")").unwrap();

    let s = str.lock().unwrap().to_string();
    assert_eq!(&s, "hello");
}

#[test]
fn runtime_error_test() {
    let sq = Squirrel::new(1024);
    let err = sq.eval::<()>("throw 42").unwrap_err();
    assert!(matches!(err, CallError::Runtime(Value::Integer(42))));
}

#[test]
fn create_function_test() {
    let sq = Squirrel::new(1024);
    let closure = sq.create_function(|(v,): (Integer,)| Ok(v + 1));
    let result: Integer = closure.call((30,)).unwrap();
    assert_eq!(result, 31);
}

#[test]
fn call_native_function() {
    let sq = Squirrel::new(1024);
    let closure = sq.create_function(|(v,): (Integer,)| Ok(v + 1));
    sq.root_table().set("add_one", closure).unwrap();
    let result: Integer = sq.eval("return add_one(30)").unwrap();
    assert_eq!(result, 31);
}

#[test]
fn native_function_panic() {
    let sq = Squirrel::new(1024);
    let closure = sq.create_function::<'_, _, (), ()>(|()| panic!("bad native function"));
    let err = closure.call::<_, ()>(()).unwrap_err();
    assert!(matches!(err, CallError::Runtime(Value::String(_))));
}

#[test]
fn native_function_error() {
    let sq = Squirrel::new(1024);
    let closure = sq.create_function::<'_, _, (), ()>(move |()| Err(Value::Integer(123)));
    let err = closure.call::<_, ()>(()).unwrap_err();
    assert!(matches!(err, CallError::Runtime(Value::Integer(123))));
}

pub struct UserData<'vm>(Object<'vm>);

pub struct Generator<'vm>(Object<'vm>);

pub struct UserPointer(*mut c_void);

unsafe impl PushIntoStack for UserPointer {
    fn push_into_stack(self, sq: &Squirrel) {
        unsafe { sq_pushuserpointer(sq.vm, self.0) };
    }
}

pub struct Thread<'vm>(Object<'vm>);

pub struct Class<'vm>(Object<'vm>);

pub struct Instance<'vm>(Object<'vm>);

pub struct WeakRef<'vm>(Object<'vm>);

macro_rules! object_from_squirrel {
    ($(($t:ident, $tag:ident, $name:literal)),*) => {
        $(
            impl<'vm> FromSquirrel<'vm> for $t<'vm> {
                fn from_squirrel(value: Value<'vm>, _sq: &'vm Squirrel) -> Result<Self> {
                    if let Value::$t(o) = value {
                        Ok(o)
                    } else {
                        Err(Error::Type { expected: $name })
                    }
                }

                unsafe fn from_stack(idx: Integer, sq: &'vm Squirrel) -> Result<Self> {
                    let object = Object::from_stack(idx, sq);
                    if object.obj._type != $tag {
                        return Err(Error::Type { expected: $name });
                    }
                    Ok($t(object))
                }
            }
        )*
    };
}

object_from_squirrel!(
    (UserData, tagSQObjectType_OT_USERDATA, "userdata"),
    (Generator, tagSQObjectType_OT_GENERATOR, "generator"),
    (Thread, tagSQObjectType_OT_THREAD, "thread"),
    (Class, tagSQObjectType_OT_CLASS, "class"),
    (Instance, tagSQObjectType_OT_INSTANCE, "instance"),
    (WeakRef, tagSQObjectType_OT_WEAKREF, "weakref")
);

macro_rules! object_into_squirrel {
    ($($t:ident),*) => {
        $(
            impl<'vm> IntoSquirrel<'vm> for $t<'vm> {
                fn into_squirrel(self, sq: &'vm Squirrel) -> Value<'vm> {
                    self.0.sq.assert_same_vm(sq);
                    Value::$t(self)
                }
            }

            unsafe impl<'vm> PushIntoStack for $t<'vm> {
                fn push_into_stack(self, sq: &Squirrel) {
                    self.0.sq.assert_same_vm(sq);
                    self.0.push_into_stack();
                }
            }
        )*
    }
}

object_into_squirrel!(UserData, Generator, Thread, Class, Instance, WeakRef);
