mod compiler_error_handler;

use std::{
    ffi::{CStr, CString, c_char, c_void},
    panic::{AssertUnwindSafe, catch_unwind},
};

use squirrels_sys::{
    HSQOBJECT, HSQUIRRELVM, SQ_VMSTATE_IDLE, SQ_VMSTATE_RUNNING, SQ_VMSTATE_SUSPENDED, SQBool,
    SQChar, SQFalse, SQFloat, SQInteger, SQTrue, SQUnsignedInteger, SQUserPointer, sq_addref,
    sq_call, sq_close, sq_compilebuffer, sq_get, sq_getbool, sq_getfloat, sq_getinteger,
    sq_getlasterror, sq_getrefcount, sq_getstackobj, sq_getstringandsize, sq_gettop,
    sq_getuserdata, sq_getvmstate, sq_newclosure, sq_newslot, sq_newuserdata, sq_open, sq_pop,
    sq_push, sq_pushbool, sq_pushfloat, sq_pushinteger, sq_pushnull, sq_pushobject,
    sq_pushroottable, sq_pushstring, sq_release, sq_resetobject, sq_setreleasehook, sq_throwerror,
    sq_throwobject, tagSQObjectType_OT_ARRAY, tagSQObjectType_OT_BOOL, tagSQObjectType_OT_CLASS,
    tagSQObjectType_OT_CLOSURE, tagSQObjectType_OT_FLOAT, tagSQObjectType_OT_GENERATOR,
    tagSQObjectType_OT_INSTANCE, tagSQObjectType_OT_INTEGER, tagSQObjectType_OT_NATIVECLOSURE,
    tagSQObjectType_OT_NULL, tagSQObjectType_OT_STRING, tagSQObjectType_OT_TABLE,
    tagSQObjectType_OT_THREAD, tagSQObjectType_OT_USERDATA, tagSQObjectType_OT_USERPOINTER,
    tagSQObjectType_OT_WEAKREF,
};

pub type Result<T> = std::result::Result<T, Error>;

pub type CallResult<'vm, T> = std::result::Result<T, CallError<'vm>>;

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
}

#[derive(Debug, thiserror::Error)]
pub enum CallError<'vm> {
    #[error("runtime error: {0:?}")]
    Runtime(Value<'vm>),

    #[error(transparent)]
    Other(#[from] Error),
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

fn get_runtime_error(sq: &Squirrel) -> Value<'_> {
    unsafe { sq_getlasterror(sq.vm) };
    let err = Object::from_stack(sq, -1);
    sq.pop(1);
    err.to_value()
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
    R: IntoSquirrel,
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

        let args = match A::from_args(sq, top - 2) {
            Ok(a) => a,
            Err(e) => {
                let msg = CString::new(e.to_string())
                    .unwrap_or_else(|_| c"native function arg extraction failed".to_owned());
                return unsafe { sq_throwerror(v, msg.as_ptr()) }.0;
            }
        };

        match f(args) {
            Ok(r) => {
                r.push_to(sq);
                1
            }
            Err(value) => {
                value.push_to(sq);
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

        let val = T::from_stack(self, -1);
        self.pop(3);
        Ok(val?)
    }

    pub fn root_table(&self) -> Table<'_> {
        let mut root = self.root;
        unsafe { sq_addref(self.vm, &mut root) };
        Table(Object {
            vm: self,
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
            "attempted to pop {count} elements but the stack is {stack_depth}"
        );
        assert!(count > 0);
        unsafe { sq_pop(self.vm, count) };
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
        R: IntoSquirrel,
    {
        let fn_ptr: *mut F = Box::into_raw(Box::new(f));

        let user_data = unsafe { sq_newuserdata(self.vm, size_of::<*mut F>() as _) };

        unsafe { *(user_data as *mut *mut F) = fn_ptr };

        unsafe { sq_setreleasehook(self.vm, -1, Some(closure_release_hook::<F>)) };

        unsafe { sq_newclosure(self.vm, Some(closure_trampoline::<'vm, F, A, R>), 1) };

        let nc = NativeClosure(Object::from_stack(self, -1));
        self.pop(1);
        nc
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

fn assert_valid_stack_idx(vm: HSQUIRRELVM, idx: SQInteger) {
    let top = unsafe { sq_gettop(vm) };
    let valid = idx != 0 && if idx > 0 { idx <= top } else { idx >= -top };
    assert!(valid, "invalid stack index {idx} (top={top})")
}

/// A handle to a Squirrel ref-counted object.
pub struct Object<'vm> {
    vm: &'vm Squirrel,
    obj: HSQOBJECT,
}

impl<'vm> Object<'vm> {
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
                SqString::from_object(self).expect("OT_STRING object materializes as SqString"),
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

impl Drop for Object<'_> {
    fn drop(&mut self) {
        unsafe { sq_release(self.vm.vm, &mut self.obj) };
    }
}

pub struct SqString<'vm> {
    object: Object<'vm>,
    ptr: *const SQChar,
    len: usize,
}

impl<'vm> SqString<'vm> {
    pub(crate) fn from_object(object: Object<'vm>) -> Result<Self> {
        if object.obj._type != tagSQObjectType_OT_STRING {
            return Err(Error::Type { expected: "string" });
        }

        // First we must push the string onto the stack because we can't get its stack index
        // from its object handle, if it has any.
        object.push();

        let mut ptr: *const SQChar = std::ptr::null();
        let mut len: SQInteger = 0;
        let ret = unsafe { sq_getstringandsize(object.vm.vm, -1, &mut ptr, &mut len) };

        // Pop before we check for an error to avoid leaving the stack in an invalid state.
        object.vm.pop(1);

        assert!(
            !ret.is_error(),
            "sq_getstringandsize failed on a verified OT_STRING"
        );

        Ok(Self {
            object,
            ptr,
            len: len as usize,
        })
    }

    pub(crate) fn from_stack(sq: &'vm Squirrel, idx: SQInteger) -> Result<Self> {
        let object = Object::from_stack(sq, idx);
        if object.obj._type != tagSQObjectType_OT_STRING {
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
            object,
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

    pub fn from_str(sq: &'vm Squirrel, str: &str) -> Self {
        unsafe { sq_pushstring(sq.vm, str.as_bytes().as_ptr() as *const i8, str.len() as _) };
        SqString::from_stack(sq, -1).expect("expecting the string we just pushed")
    }
}

#[test]
fn test_string_from_stack() {
    let sq = Squirrel::new(1024);
    let str = sq.eval::<SqString>("return \"test\"").unwrap();
    assert_eq!(str.to_str().unwrap(), "test");
}

#[test]
fn test_value_from_object() {
    let sq = Squirrel::new(1024);
    let v = sq.eval::<Value>("return 123").unwrap();
    assert!(matches!(v, Value::Integer(123)));
}

pub struct Table<'vm>(Object<'vm>);

impl<'vm> Table<'vm> {
    pub fn get<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: IntoSquirrel,
        V: FromSquirrel<'vm>,
    {
        self.0.push();
        key.push_to(self.0.vm);

        let ret = unsafe { sq_get(self.0.vm.vm, -2) };
        if ret.is_error() {
            self.0.vm.pop(1);
            return Ok(None);
        }

        let val = V::from_stack(self.0.vm, -1);
        self.0.vm.pop(2);
        val.map(Some)
    }

    pub fn set<K, V>(&self, key: K, value: V) -> CallResult<'_, ()>
    where
        K: IntoSquirrel,
        V: IntoSquirrel,
    {
        self.0.push();
        key.push_to(self.0.vm);
        value.push_to(self.0.vm);

        let ret = unsafe { sq_newslot(self.0.vm.vm, -3, SQFalse as _) };
        if ret.is_error() {
            // sq_newslot only pops k+v on success
            self.0.vm.pop(3);

            return Err(CallError::Runtime(get_runtime_error(self.0.vm)));
        }

        // Pop the table
        self.0.vm.pop(1);

        Ok(())
    }
}

#[test]
fn table_get() {
    let sq = Squirrel::new(1024);
    sq.eval::<()>("a <- 1").unwrap();
    let v = sq.root_table().get::<_, Integer>("a").unwrap();
    assert!(matches!(v, Some(1)))
}

#[test]
fn table_set() {
    let sq = Squirrel::new(1024);
    sq.root_table().set("a", 24).unwrap();
    let v: Integer = sq.eval("return a").unwrap();
    assert_eq!(v, 24);
}

#[test]
fn table_roundtrip() {
    let sq = Squirrel::new(1024);
    sq.root_table().set("x", 10).unwrap();
    sq.eval::<()>("y <- x * 2").unwrap();
    let y: Integer = sq.root_table().get("y").unwrap().unwrap();
    assert_eq!(y, 20);
}

#[test]
fn table_set_error() {
    let sq = Squirrel::new(1024);
    let root_table = sq.root_table();

    // null is not a valid key, so this should fail.
    let err = root_table.set((), 1).unwrap_err();
    assert!(matches!(err, CallError::Runtime(_)));
}

pub struct Array<'vm>(Object<'vm>);

impl<'vm> Array<'vm> {
    pub fn get<V: FromSquirrel<'vm>>(&self, idx: Integer) -> Result<Option<V>> {
        self.0.push();
        idx.push_to(self.0.vm);

        let ret = unsafe { sq_get(self.0.vm.vm, -2) };
        if ret.is_error() {
            self.0.vm.pop(1);

            return Ok(None);
        }

        let val = V::from_stack(self.0.vm, -1);
        self.0.vm.pop(2);
        val.map(Some)
    }
}

pub struct UserData<'vm>(Object<'vm>);

pub struct Closure<'vm>(Object<'vm>);

impl<'vm> Closure<'vm> {
    pub fn call<A: IntoArgs, T: FromSquirrel<'vm>>(&self, args: A) -> CallResult<'vm, T> {
        self.push_to(self.0.vm);
        self.0.vm.push_root_table();
        let arg_count = args.push_args(self.0.vm) + 1;

        let ret = unsafe { sq_call(self.0.vm.vm, arg_count, SQTrue as _, SQFalse as _) };
        if ret.is_error() {
            self.0.vm.pop(1);

            return Err(CallError::Runtime(get_runtime_error(self.0.vm)));
        }

        let val = T::from_stack(self.0.vm, -1);
        self.0.vm.pop(2);
        Ok(val?)
    }
}

#[test]
fn closure_call_no_args() {
    let sq = Squirrel::new(1024);
    let f: Closure = sq.eval("return function() { return 123 }").unwrap();
    let val: Integer = f.call(()).unwrap();
    assert_eq!(val, 123);
}

#[test]
fn closure_call_single_arg() {
    let sq = Squirrel::new(1024);
    let f: Closure = sq.eval("return function(n) { return n + 1 }").unwrap();
    let val: Integer = f.call((9000,)).unwrap();
    assert_eq!(val, 9001);
}

#[test]
fn closure_call_multiple_args() {
    let sq = Squirrel::new(1024);
    let f: Closure = sq.eval("return function(n, m) { return n + m }").unwrap();
    let val: Integer = f.call((3, 4)).unwrap();
    assert_eq!(val, 7);
}

#[test]
fn closure_call_mixed_types() {
    let sq = Squirrel::new(1024);
    let f: Closure = sq
        .eval("return function(s, n) { return s + n.tostring() }")
        .unwrap();
    let val: SqString = f.call(("count: ", 9001)).unwrap();
    assert_eq!(val.to_str().unwrap(), "count: 9001");
}

#[test]
fn closure_call_error() {
    let sq = Squirrel::new(1024);
    let f: Closure = sq.eval("return function(x) { throw \"error\" }").unwrap();
    let err = f.call::<_, ()>((1,)).unwrap_err();
    assert!(matches!(err, CallError::Runtime(Value::String(_))));
}

#[test]
fn closure_outlives_other_evals() {
    let sq = Squirrel::new(1024);
    let f: Closure = sq.eval("return function(x) { return x + 1 }").unwrap();
    let _: Integer = sq.eval("return 0").unwrap();
    let val: Integer = f.call((10,)).unwrap();
    assert_eq!(val, 11);
}

#[test]
fn closure_call_no_stack_leak() {
    let sq = Squirrel::new(1024);
    let f: Closure = sq.eval("return function(x) { return x + 1 }").unwrap();
    let _: Integer = f.call((10,)).unwrap();
    assert_eq!(sq.stack_depth(), 0);
}

pub struct NativeClosure<'vm>(Object<'vm>);

impl<'vm> NativeClosure<'vm> {
    pub fn call<A: IntoArgs, T: FromSquirrel<'vm>>(&self, args: A) -> CallResult<'vm, T> {
        self.push_to(self.0.vm);
        self.0.vm.push_root_table();
        let arg_count = args.push_args(self.0.vm) + 1;

        let ret = unsafe { sq_call(self.0.vm.vm, arg_count, SQTrue as _, SQFalse as _) };
        if ret.is_error() {
            self.0.vm.pop(1);

            return Err(CallError::Runtime(get_runtime_error(self.0.vm)));
        }

        let val = T::from_stack(self.0.vm, -1);
        self.0.vm.pop(2);
        Ok(val?)
    }
}

pub struct Generator<'vm>(Object<'vm>);

pub struct UserPointer(*mut c_void);

pub struct Thread<'vm>(Object<'vm>);

pub struct Class<'vm>(Object<'vm>);

pub struct Instance<'vm>(Object<'vm>);

pub struct WeakRef<'vm>(Object<'vm>);

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

impl std::fmt::Debug for Value<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Null => write!(f, "Null"),
            Self::Integer(n) => f.debug_tuple("Integer").field(n).finish(),
            Self::Float(n) => f.debug_tuple("Float").field(n).finish(),
            Self::Bool(b) => f.debug_tuple("Bool").field(b).finish(),
            Self::String(sqstr) => f
                .debug_tuple("String")
                .field(&String::from_utf8_lossy(sqstr.as_bytes()))
                .finish(),
            Self::Table(_) => f.debug_tuple("Table").finish(),
            Self::Array(_) => f.debug_tuple("Array").finish(),
            Self::UserData(_) => f.debug_tuple("UserData").finish(),
            Self::Closure(_) => f.debug_tuple("Closure").finish(),
            Self::NativeClosure(_) => f.debug_tuple("NativeClosure").finish(),
            Self::Generator(_) => f.debug_tuple("Generator").finish(),
            Self::UserPointer(_) => f.debug_tuple("UserPointer").finish(),
            Self::Thread(_) => f.debug_tuple("Thread").finish(),
            Self::Class(_) => f.debug_tuple("Class").finish(),
            Self::Instance(_) => f.debug_tuple("Instance").finish(),
            Self::WeakRef(_) => f.debug_tuple("WeakRef").finish(),
        }
    }
}

// TODO should this trait be public?
// If we call `from_top` on an empty stack we panic.
pub trait FromSquirrel<'vm>: Sized {
    fn from_stack(sq: &'vm Squirrel, idx: Integer) -> Result<Self>;
}

impl FromSquirrel<'_> for () {
    fn from_stack(sq: &'_ Squirrel, idx: Integer) -> Result<Self> {
        let obj = Object::from_stack(sq, idx);
        if obj.obj._type == tagSQObjectType_OT_NULL {
            Ok(())
        } else {
            Err(Error::Type { expected: "null" })
        }
    }
}

impl FromSquirrel<'_> for bool {
    fn from_stack(sq: &Squirrel, idx: Integer) -> Result<Self> {
        assert_valid_stack_idx(sq.vm, idx);

        let mut out: SQBool = 0;
        if unsafe { sq_getbool(sq.vm, idx, &mut out) }.is_error() {
            return Err(Error::Type { expected: "bool" });
        }
        Ok(out != 0)
    }
}

impl FromSquirrel<'_> for Integer {
    fn from_stack(sq: &Squirrel, idx: Integer) -> Result<Self> {
        assert_valid_stack_idx(sq.vm, idx);

        let mut out: SQInteger = 0;
        if unsafe { sq_getinteger(sq.vm, idx, &mut out) }.is_error() {
            return Err(Error::Type {
                expected: "integer",
            });
        }
        Ok(out)
    }
}

impl FromSquirrel<'_> for Float {
    fn from_stack(sq: &Squirrel, idx: Integer) -> Result<Self> {
        assert_valid_stack_idx(sq.vm, idx);

        let mut out: SQFloat = 0.0;
        if unsafe { sq_getfloat(sq.vm, idx, &mut out) }.is_error() {
            return Err(Error::Type { expected: "float" });
        }
        Ok(out)
    }
}

impl<'vm> FromSquirrel<'vm> for SqString<'vm> {
    fn from_stack(sq: &'vm Squirrel, idx: Integer) -> Result<Self> {
        SqString::from_stack(sq, idx)
    }
}

macro_rules! object_from_squirrel {
    ($(($t:ident, $tag:ident, $name:literal)),*) => {
        $(
            impl<'vm> FromSquirrel<'vm> for $t<'vm> {
                fn from_stack(sq: &'vm Squirrel, idx: Integer) -> Result<Self> {
                    let object = Object::from_stack(sq, idx);
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
    (Table, tagSQObjectType_OT_TABLE, "table"),
    (Array, tagSQObjectType_OT_ARRAY, "array"),
    (UserData, tagSQObjectType_OT_USERDATA, "userdata"),
    (Closure, tagSQObjectType_OT_CLOSURE, "closure"),
    (
        NativeClosure,
        tagSQObjectType_OT_NATIVECLOSURE,
        "nativeclosure"
    ),
    (Generator, tagSQObjectType_OT_GENERATOR, "generator"),
    (Thread, tagSQObjectType_OT_THREAD, "thread"),
    (Class, tagSQObjectType_OT_CLASS, "class"),
    (Instance, tagSQObjectType_OT_INSTANCE, "instance"),
    (WeakRef, tagSQObjectType_OT_WEAKREF, "weakref")
);

impl<'vm> FromSquirrel<'vm> for Value<'vm> {
    fn from_stack(sq: &'vm Squirrel, idx: Integer) -> Result<Self> {
        Ok(Object::from_stack(sq, idx).to_value())
    }
}

pub trait IntoSquirrel {
    fn push_to(&self, sq: &Squirrel);
}

impl IntoSquirrel for () {
    fn push_to(&self, sq: &Squirrel) {
        unsafe { sq_pushnull(sq.vm) };
    }
}

impl IntoSquirrel for Integer {
    fn push_to(&self, sq: &Squirrel) {
        unsafe { sq_pushinteger(sq.vm, *self) };
    }
}

impl IntoSquirrel for Float {
    fn push_to(&self, sq: &Squirrel) {
        unsafe { sq_pushfloat(sq.vm, *self) };
    }
}

impl IntoSquirrel for bool {
    fn push_to(&self, sq: &Squirrel) {
        unsafe { sq_pushbool(sq.vm, if *self { 1 } else { 0 }) };
    }
}

impl IntoSquirrel for SqString<'_> {
    fn push_to(&self, sq: &Squirrel) {
        assert!(
            std::ptr::eq(self.object.vm as *const _, sq.vm as *const _),
            "pushing object to a different VM"
        );
        self.object.push();
    }
}

impl IntoSquirrel for &str {
    fn push_to(&self, sq: &Squirrel) {
        SqString::from_str(sq, self);
    }
}

impl IntoSquirrel for String {
    fn push_to(&self, sq: &Squirrel) {
        self.as_str().push_to(sq);
    }
}

macro_rules! object_into_squirrel {
    ($($t:ident),*) => {
        $(
            impl IntoSquirrel for $t<'_> {
                fn push_to(&self, sq: &Squirrel) {
                    assert!(
                        std::ptr::eq(self.0.vm.vm as *const _, sq.vm as *const _),
                        "pushing object to a different VM"
                    );
                    self.0.push();
                }
            }
        )*
    }
}

object_into_squirrel!(
    Table,
    Array,
    UserData,
    Closure,
    NativeClosure,
    Generator,
    Thread,
    Class,
    Instance,
    WeakRef
);

impl IntoSquirrel for Value<'_> {
    fn push_to(&self, sq: &Squirrel) {
        match self {
            Value::Null => <() as IntoSquirrel>::push_to(&(), sq),
            Value::Integer(n) => n.push_to(sq),
            Value::Float(n) => n.push_to(sq),
            Value::Bool(b) => b.push_to(sq),
            Value::String(sq_string) => sq_string.push_to(sq),
            Value::Table(table) => table.push_to(sq),
            Value::Array(array) => array.push_to(sq),
            Value::UserData(user_data) => user_data.push_to(sq),
            Value::Closure(closure) => closure.push_to(sq),
            Value::NativeClosure(native_closure) => native_closure.push_to(sq),
            Value::Generator(generator) => generator.push_to(sq),
            Value::UserPointer(_) => todo!(),
            Value::Thread(thread) => thread.push_to(sq),
            Value::Class(class) => class.push_to(sq),
            Value::Instance(instance) => instance.push_to(sq),
            Value::WeakRef(weak_ref) => weak_ref.push_to(sq),
        }
    }
}

pub trait IntoArgs {
    fn push_args(self, sq: &Squirrel) -> SQInteger;
}

impl IntoArgs for () {
    fn push_args(self, _: &Squirrel) -> SQInteger {
        0
    }
}

pub trait FromArgs<'vm>: Sized {
    fn from_args(sq: &'vm Squirrel, count: SQInteger) -> Result<Self>;
}

impl<'vm> FromArgs<'vm> for () {
    fn from_args(_sq: &'vm Squirrel, count: SQInteger) -> Result<Self> {
        if count != 0 {
            return Err(Error::Type {
                expected: "0 arguments",
            });
        }
        Ok(())
    }
}

macro_rules! count_args {
    ($($_:tt),+) => {
        <[()]>::len(&[$( count_args!(@unit $_) ),+])
    };
    (@unit $_:tt) => { () };
}

macro_rules! impl_into_args_tuple {
    ($($field:tt = $name:ident),+ $(,)?) => {
        impl<$($name: IntoSquirrel),+> IntoArgs for ($($name,)+) {
            fn push_args(self, sq: &Squirrel) -> SQInteger {
                $( self.$field.push_to(sq); )+
                count_args!($($name),+) as SQInteger
            }
        }
    }
}

macro_rules! impl_from_args_tuple {
    ($($field:tt = $name:ident),+ $(,)?) => {
        impl<'vm, $($name: FromSquirrel<'vm>),+> FromArgs<'vm> for ($($name,)+) {
            fn from_args(sq: &'vm Squirrel, count: SQInteger) -> Result<Self> {
                if count != (count_args!($($name),+) as SQInteger) {
                    return Err(Error::Type {
                        expected: concat!(stringify!(count_args!($($name),+)), " arguments"),
                    })
                }

                Ok(( $($name::from_stack(sq, $field + 2)?,)+ ))
            }
        }
    }
}

impl_into_args_tuple!(0 = T0);
impl_from_args_tuple!(0 = T0);
impl_into_args_tuple!(0 = T0, 1 = T1);
impl_from_args_tuple!(0 = T0, 1 = T1);
impl_into_args_tuple!(0 = T0, 1 = T1, 2 = T2);
impl_from_args_tuple!(0 = T0, 1 = T1, 2 = T2);
impl_into_args_tuple!(0 = T0, 1 = T1, 2 = T2, 3 = T3);
impl_from_args_tuple!(0 = T0, 1 = T1, 2 = T2, 3 = T3);
impl_into_args_tuple!(0 = T0, 1 = T1, 2 = T2, 3 = T3, 4 = T4);
impl_from_args_tuple!(0 = T0, 1 = T1, 2 = T2, 3 = T3, 4 = T4);
impl_into_args_tuple!(0 = T0, 1 = T1, 2 = T2, 3 = T3, 4 = T4, 5 = T5);
impl_from_args_tuple!(0 = T0, 1 = T1, 2 = T2, 3 = T3, 4 = T4, 5 = T5);
impl_into_args_tuple!(0 = T0, 1 = T1, 2 = T2, 3 = T3, 4 = T4, 5 = T5, 6 = T6);
impl_from_args_tuple!(0 = T0, 1 = T1, 2 = T2, 3 = T3, 4 = T4, 5 = T5, 6 = T6);
impl_into_args_tuple!(
    0 = T0,
    1 = T1,
    2 = T2,
    3 = T3,
    4 = T4,
    5 = T5,
    6 = T6,
    7 = T7,
);
impl_from_args_tuple!(
    0 = T0,
    1 = T1,
    2 = T2,
    3 = T3,
    4 = T4,
    5 = T5,
    6 = T6,
    7 = T7,
);
impl_into_args_tuple!(
    0 = T0,
    1 = T1,
    2 = T2,
    3 = T3,
    4 = T4,
    5 = T5,
    6 = T6,
    7 = T7,
    8 = T8,
);
impl_from_args_tuple!(
    0 = T0,
    1 = T1,
    2 = T2,
    3 = T3,
    4 = T4,
    5 = T5,
    6 = T6,
    7 = T7,
    8 = T8,
);
impl_into_args_tuple!(
    0 = T0,
    1 = T1,
    2 = T2,
    3 = T3,
    4 = T4,
    5 = T5,
    6 = T6,
    7 = T7,
    8 = T8,
    9 = T9,
);
impl_from_args_tuple!(
    0 = T0,
    1 = T1,
    2 = T2,
    3 = T3,
    4 = T4,
    5 = T5,
    6 = T6,
    7 = T7,
    8 = T8,
    9 = T9,
);
impl_into_args_tuple!(
    0 = T0,
    1 = T1,
    2 = T2,
    3 = T3,
    4 = T4,
    5 = T5,
    6 = T6,
    7 = T7,
    8 = T8,
    9 = T9,
    10 = T10,
);
impl_from_args_tuple!(
    0 = T0,
    1 = T1,
    2 = T2,
    3 = T3,
    4 = T4,
    5 = T5,
    6 = T6,
    7 = T7,
    8 = T8,
    9 = T9,
    10 = T10,
);
impl_into_args_tuple!(
    0 = T0,
    1 = T1,
    2 = T2,
    3 = T3,
    4 = T4,
    5 = T5,
    6 = T6,
    7 = T7,
    8 = T8,
    9 = T9,
    10 = T10,
    11 = T11,
);
impl_from_args_tuple!(
    0 = T0,
    1 = T1,
    2 = T2,
    3 = T3,
    4 = T4,
    5 = T5,
    6 = T6,
    7 = T7,
    8 = T8,
    9 = T9,
    10 = T10,
    11 = T11,
);
impl_into_args_tuple!(
    0 = T0,
    1 = T1,
    2 = T2,
    3 = T3,
    4 = T4,
    5 = T5,
    6 = T6,
    7 = T7,
    8 = T8,
    9 = T9,
    10 = T10,
    11 = T11,
    12 = T12,
);
impl_from_args_tuple!(
    0 = T0,
    1 = T1,
    2 = T2,
    3 = T3,
    4 = T4,
    5 = T5,
    6 = T6,
    7 = T7,
    8 = T8,
    9 = T9,
    10 = T10,
    11 = T11,
    12 = T12,
);
impl_into_args_tuple!(
    0 = T0,
    1 = T1,
    2 = T2,
    3 = T3,
    4 = T4,
    5 = T5,
    6 = T6,
    7 = T7,
    8 = T8,
    9 = T9,
    10 = T10,
    11 = T11,
    12 = T12,
    13 = T13,
);
impl_from_args_tuple!(
    0 = T0,
    1 = T1,
    2 = T2,
    3 = T3,
    4 = T4,
    5 = T5,
    6 = T6,
    7 = T7,
    8 = T8,
    9 = T9,
    10 = T10,
    11 = T11,
    12 = T12,
    13 = T13,
);
impl_into_args_tuple!(
    0 = T0,
    1 = T1,
    2 = T2,
    3 = T3,
    4 = T4,
    5 = T5,
    6 = T6,
    7 = T7,
    8 = T8,
    9 = T9,
    10 = T10,
    11 = T11,
    12 = T12,
    13 = T13,
    14 = T14,
);
impl_from_args_tuple!(
    0 = T0,
    1 = T1,
    2 = T2,
    3 = T3,
    4 = T4,
    5 = T5,
    6 = T6,
    7 = T7,
    8 = T8,
    9 = T9,
    10 = T10,
    11 = T11,
    12 = T12,
    13 = T13,
    14 = T14,
);
impl_into_args_tuple!(
    0 = T0,
    1 = T1,
    2 = T2,
    3 = T3,
    4 = T4,
    5 = T5,
    6 = T6,
    7 = T7,
    8 = T8,
    9 = T9,
    10 = T10,
    11 = T11,
    12 = T12,
    13 = T13,
    14 = T14,
    15 = T15,
);
impl_from_args_tuple!(
    0 = T0,
    1 = T1,
    2 = T2,
    3 = T3,
    4 = T4,
    5 = T5,
    6 = T6,
    7 = T7,
    8 = T8,
    9 = T9,
    10 = T10,
    11 = T11,
    12 = T12,
    13 = T13,
    14 = T14,
    15 = T15,
);
