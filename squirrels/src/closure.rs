use squirrels_sys::{
    SQFalse, SQTrue, sq_call, tagSQObjectType_OT_CLOSURE, tagSQObjectType_OT_NATIVECLOSURE,
};

use crate::{
    CallResult, FromSquirrel, IntoArgs, IntoSquirrel, Object, errors::SqResultExt as _,
    traits::impl_object_traits,
};

/// A ref-counted handle to a Squirrel function.
#[derive(Debug, Clone, PartialEq)]
pub struct Closure<'vm>(pub(crate) Object<'vm>);

impl_object_traits!(Closure, tagSQObjectType_OT_CLOSURE, "closure");

impl<'vm> Closure<'vm> {
    /// Calls the function with the global environment bound as `this`.
    pub fn call<Args, T>(&self, args: Args) -> CallResult<'vm, T>
    where
        Args: IntoArgs<'vm>,
        T: FromSquirrel<'vm>,
    {
        self.0.push_into_stack();
        self.0.sq.push_root_table();
        call_closure(&self.0, args)
    }

    /// Calls the function with the given `Env` value bound as `this`.
    pub fn call_with<Env, Args, T>(&self, this: Env, args: Args) -> CallResult<'vm, T>
    where
        Env: IntoSquirrel<'vm>,
        Args: IntoArgs<'vm>,
        T: FromSquirrel<'vm>,
    {
        self.0.push_into_stack();
        unsafe { this.push_into_stack(self.0.sq) };
        call_closure(&self.0, args)
    }
}

/// A ref-counted handle to a native function defined using
/// [`Squirrel::create_function`](crate::Squirrel::create_function).
#[derive(Debug, Clone, PartialEq)]
pub struct NativeClosure<'vm>(pub(crate) Object<'vm>);

impl_object_traits!(
    NativeClosure,
    tagSQObjectType_OT_NATIVECLOSURE,
    "nativeclosure"
);

impl<'vm> NativeClosure<'vm> {
    /// Calls the native function with the global environment bound as `this`.
    pub fn call<Args, T>(&self, args: Args) -> CallResult<'vm, T>
    where
        Args: IntoArgs<'vm>,
        T: FromSquirrel<'vm>,
    {
        self.0.push_into_stack();
        self.0.sq.push_root_table();
        call_closure(&self.0, args)
    }

    /// Calls the native function with the given `Env` value bound `this`.
    pub fn call_with<Env, Args, T>(&self, this: Env, args: Args) -> CallResult<'vm, T>
    where
        Env: IntoSquirrel<'vm>,
        Args: IntoArgs<'vm>,
        T: FromSquirrel<'vm>,
    {
        self.0.push_into_stack();
        unsafe { this.push_into_stack(self.0.sq) };
        call_closure(&self.0, args)
    }
}

/// Expects the closure and the environment to already be pushed.
pub(crate) fn call_closure<'vm, A: IntoArgs<'vm>, T: FromSquirrel<'vm>>(
    obj: &Object<'vm>,
    args: A,
) -> CallResult<'vm, T> {
    let arg_count = args.push_args(obj.sq) + 1;

    unsafe { sq_call(obj.sq.vm, arg_count, SQTrue as _, SQFalse as _) }
        .to_runtime_error(obj.sq, 1)?;

    let val = unsafe { T::from_stack(-1, obj.sq) };
    obj.sq.pop(2);
    Ok(val?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CallError, Integer, Squirrel, String, Value};

    #[test]
    fn closure_call_no_args() {
        let sq = Squirrel::new(1024);
        let f: Closure = sq.eval("return function() { return 123 }").unwrap();
        let val: Integer = f.call(()).unwrap();
        assert_eq!(val, 123);
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn closure_call_single_arg() {
        let sq = Squirrel::new(1024);
        let f: Closure = sq.eval("return function(n) { return n + 1 }").unwrap();
        let val: Integer = f.call((9000,)).unwrap();
        assert_eq!(val, 9001);
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn closure_call_multiple_args() {
        let sq = Squirrel::new(1024);
        let f: Closure = sq.eval("return function(n, m) { return n + m }").unwrap();
        let val: Integer = f.call((3, 4)).unwrap();
        assert_eq!(val, 7);
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn closure_call_mixed_types() {
        let sq = Squirrel::new(1024);
        let f: Closure = sq
            .eval("return function(s, n) { return s + n.tostring() }")
            .unwrap();
        let val: String = f.call(("count: ", 9001)).unwrap();
        assert_eq!(val.to_str().unwrap(), "count: 9001");
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn closure_call_error() {
        let sq = Squirrel::new(1024);
        let f: Closure = sq.eval("return function(x) { throw \"error\" }").unwrap();
        let err = f.call::<_, ()>((1,)).unwrap_err();
        assert!(matches!(err, CallError::Runtime(Value::String(_))));
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn closure_outlives_other_evals() {
        let sq = Squirrel::new(1024);
        let f: Closure = sq.eval("return function(x) { return x + 1 }").unwrap();
        let _: Integer = sq.eval("return 0").unwrap();
        let val: Integer = f.call((10,)).unwrap();
        assert_eq!(val, 11);
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn closure_call_no_stack_leak() {
        let sq = Squirrel::new(1024);
        let f: Closure = sq.eval("return function(x) { return x + 1 }").unwrap();
        let _: Integer = f.call((10,)).unwrap();
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn closure_call_with() {
        let sq = Squirrel::new(1024);
        let f: Closure = sq.eval("return function() { return this + 1 }").unwrap();
        let v: Integer = f.call_with(5, ()).unwrap();
        assert_eq!(v, 6);
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn nativeclosure_call_with() {
        let sq = Squirrel::new(1024);
        let f = sq.create_function(|(x, y): (Integer, Integer)| Ok(x + y));
        let v: Integer = f.call((4, 5)).unwrap();
        assert_eq!(v, 9);

        let v: Integer = f.call_with((), (4, 5)).unwrap();
        assert_eq!(v, 9);
        assert_eq!(sq.stack_depth(), 0);
    }
}
