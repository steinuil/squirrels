use squirrels_sys::{
    SQTrue, sq_arrayappend, sq_arraypop, sq_get, sq_getsize, sq_set, tagSQObjectType_OT_ARRAY,
};

use crate::{
    CallResult, FromSquirrel, Integer, IntoSquirrel, Object, PushIntoStack as _, Result,
    get_runtime_error, traits::impl_object_traits,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Array<'vm>(pub(crate) Object<'vm>);

impl_object_traits!(Array, tagSQObjectType_OT_ARRAY, "array");

impl<'vm> Array<'vm> {
    pub fn get<V: FromSquirrel<'vm>>(&self, idx: Integer) -> Result<Option<V>> {
        self.0.push_into_stack();
        idx.push_into_stack(self.0.sq);

        let ret = unsafe { sq_get(self.0.sq.vm, -2) };
        if ret.is_error() {
            self.0.sq.pop(1);

            return Ok(None);
        }

        let val = unsafe { V::from_stack(-1, self.0.sq) };
        self.0.sq.pop(2);
        val.map(Some)
    }

    pub fn set<T: IntoSquirrel<'vm>>(&self, key: Integer, value: T) -> CallResult<'vm, ()> {
        self.0.push_into_stack();
        key.push_into_stack(self.0.sq);
        value.push_into_stack(self.0.sq);

        let ret = unsafe { sq_set(self.0.sq.vm, -3) };
        if ret.is_error() {
            self.0.sq.pop(3);

            return Err(crate::CallError::Runtime(get_runtime_error(self.0.sq)));
        }

        self.0.sq.pop(1);

        Ok(())
    }

    pub fn append<T: IntoSquirrel<'vm>>(&self, value: T) -> CallResult<'vm, ()> {
        self.0.push_into_stack();
        value.push_into_stack(self.0.sq);

        let ret = unsafe { sq_arrayappend(self.0.sq.vm, -2) };
        if ret.is_error() {
            self.0.sq.pop(3);

            return Err(crate::CallError::Runtime(get_runtime_error(self.0.sq)));
        }

        self.0.sq.pop(1);

        Ok(())
    }

    pub fn pop<T: FromSquirrel<'vm>>(&self) -> CallResult<'vm, T> {
        self.0.push_into_stack();

        let ret = unsafe { sq_arraypop(self.0.sq.vm, -1, SQTrue as _) };
        if ret.is_error() {
            self.0.sq.pop(1);

            return Err(crate::CallError::Runtime(get_runtime_error(self.0.sq)));
        }

        let v = unsafe { T::from_stack(-1, self.0.sq) };
        self.0.sq.pop(2);
        Ok(v?)
    }

    pub fn len(&self) -> Integer {
        self.0.push_into_stack();
        let len = unsafe { sq_getsize(self.0.sq.vm, -1) };
        self.0.sq.pop(1);
        len
    }
}

#[cfg(test)]
mod tests {
    use super::Array;
    use crate::{CallError, Integer, Squirrel, Value};

    /// Arrays in Squirrel are 0-based and not 1-cringed.
    #[test]
    fn array_get() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = sq.eval("return [123, 456, 789]").unwrap();
        let v: Integer = arr.get(1).unwrap().unwrap();
        assert_eq!(v, 456);
    }

    #[test]
    fn array_set() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = sq.eval("return [123, 456, 789]").unwrap();
        arr.set(1, 444).unwrap();
        let v: Integer = arr.get(1).unwrap().unwrap();
        assert_eq!(v, 444);
    }

    /// Arrays in Squirrel have to be grown manually.
    /// Setting an array index out of bounds will cause
    /// a runtime error.
    #[test]
    fn array_set_oob() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = sq.eval("return [123, 456, 789]").unwrap();
        let err = arr.set(4, 444).unwrap_err();
        assert!(matches!(err, CallError::Runtime(Value::String(_))))
    }

    #[test]
    fn array_append() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = sq.eval("return [123, 456, 789]").unwrap();
        arr.append(321).unwrap();
        let v: Integer = arr.get(3).unwrap().unwrap();
        assert_eq!(v, 321);
    }

    #[test]
    fn array_pop() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = sq.eval("return [123, 456, 789]").unwrap();
        let v: Integer = arr.pop().unwrap();
        assert_eq!(v, 789);
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn array_len() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = sq.eval("return [123, 456, 789]").unwrap();
        assert_eq!(arr.len(), 3);
    }
}
