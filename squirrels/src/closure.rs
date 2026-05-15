use squirrels_sys::{
    SQFalse, SQTrue, sq_call, tagSQObjectType_OT_CLOSURE, tagSQObjectType_OT_NATIVECLOSURE,
};

use crate::{
    CallError, CallResult, Error, FromSquirrel, IntoArgs, Object, Value, get_runtime_error,
    traits::impl_object_traits,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Closure<'vm>(pub(crate) Object<'vm>);

impl Eq for Closure<'_> {}

impl_object_traits!(Closure, tagSQObjectType_OT_CLOSURE, "closure");

impl<'vm> Closure<'vm> {
    pub fn call<A: IntoArgs, T: FromSquirrel<'vm>>(&self, args: A) -> CallResult<'vm, T> {
        call_closure(&self.0, args)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeClosure<'vm>(pub(crate) Object<'vm>);

impl Eq for NativeClosure<'_> {}

impl_object_traits!(
    NativeClosure,
    tagSQObjectType_OT_NATIVECLOSURE,
    "nativeclosure"
);

impl<'vm> NativeClosure<'vm> {
    pub fn call<A: IntoArgs, T: FromSquirrel<'vm>>(&self, args: A) -> CallResult<'vm, T> {
        call_closure(&self.0, args)
    }
}

fn call_closure<'vm, A: IntoArgs, T: FromSquirrel<'vm>>(
    obj: &Object<'vm>,
    args: A,
) -> CallResult<'vm, T> {
    obj.push_into_stack();
    obj.sq.push_root_table();
    let arg_count = args.push_args(obj.sq) + 1;

    let ret = unsafe { sq_call(obj.sq.vm, arg_count, SQTrue as _, SQFalse as _) };
    if ret.is_error() {
        obj.sq.pop(1);

        return Err(CallError::Runtime(get_runtime_error(obj.sq)));
    }

    let val = unsafe { T::from_stack(-1, obj.sq) };
    obj.sq.pop(2);
    Ok(val?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Integer, Squirrel, String, Value};

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
        let val: String = f.call(("count: ", 9001)).unwrap();
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
}
