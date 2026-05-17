use squirrels_sys::{
    SQTrue, sq_arrayappend, sq_arrayinsert, sq_arraypop, sq_arrayremove, sq_arrayresize,
    sq_arrayreverse, sq_clear, sq_get, sq_getsize, sq_set, tagSQObjectType_OT_ARRAY,
};

use crate::{
    CallError, CallResult, FromSquirrel, Integer, IntoSquirrel, Object, PushIntoStack as _,
    traits::impl_object_traits,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Array<'vm>(pub(crate) Object<'vm>);

impl_object_traits!(Array, tagSQObjectType_OT_ARRAY, "array");

// TODO check if any of these methods can actually fail
impl<'vm> Array<'vm> {
    /// Gets the value at position `idx` of the array.
    ///
    /// Fails if `idx` is out of range.
    pub fn get<V: FromSquirrel<'vm>>(&self, idx: Integer) -> CallResult<'vm, V> {
        self.0.push_into_stack();
        idx.push_into_stack(self.0.sq);

        let ret = unsafe { sq_get(self.0.sq.vm, -2) };
        if ret.is_error() {
            self.0.sq.pop(1);

            return Err(CallError::get_runtime_error(self.0.sq));
        }

        let val = unsafe { V::from_stack(-1, self.0.sq) };
        self.0.sq.pop(2);
        Ok(val?)
    }

    /// Replaces the value at position `idx` of the array.
    ///
    /// Fails if `idx` is out of range.
    pub fn set<T: IntoSquirrel<'vm>>(&self, key: Integer, value: T) -> CallResult<'vm, ()> {
        self.0.push_into_stack();
        key.push_into_stack(self.0.sq);
        value.push_into_stack(self.0.sq);

        let ret = unsafe { sq_set(self.0.sq.vm, -3) };
        if ret.is_error() {
            self.0.sq.pop(3);

            return Err(CallError::get_runtime_error(self.0.sq));
        }

        self.0.sq.pop(1);

        Ok(())
    }

    /// Pushes a value to the back of the array.
    pub fn append<T: IntoSquirrel<'vm>>(&self, value: T) {
        self.0.push_into_stack();
        value.push_into_stack(self.0.sq);

        let ret = unsafe { sq_arrayappend(self.0.sq.vm, -2) };
        assert!(!ret.is_error(), "sq_arrayappend failed on {:?}", self);
        self.0.sq.pop(1);
    }

    /// Removes the last element from the array and returns it.
    ///
    /// Fails if the array is empty.
    pub fn pop<T: FromSquirrel<'vm>>(&self) -> CallResult<'vm, T> {
        self.0.push_into_stack();

        let ret = unsafe { sq_arraypop(self.0.sq.vm, -1, SQTrue as _) };
        if ret.is_error() {
            self.0.sq.pop(1);

            return Err(CallError::get_runtime_error(self.0.sq));
        }

        let v = unsafe { T::from_stack(-1, self.0.sq) };
        self.0.sq.pop(2);

        Ok(v?)
    }

    /// Returns the length of the array.
    pub fn len(&self) -> Integer {
        self.0.push_into_stack();
        let len = unsafe { sq_getsize(self.0.sq.vm, -1) };
        self.0.sq.pop(1);
        len
    }

    /// Inserts a value at position `idx` in the array,
    /// shifting up the elements from `array[idx]`.
    ///
    /// Fails if `idx` is out of range.
    pub fn insert<T: IntoSquirrel<'vm>>(&self, idx: Integer, value: T) -> CallResult<'vm, ()> {
        self.0.push_into_stack();
        value.push_into_stack(self.0.sq);

        let ret = unsafe { sq_arrayinsert(self.0.sq.vm, -2, idx) };
        self.0.sq.pop(1);
        if ret.is_error() {
            return Err(CallError::get_runtime_error(self.0.sq));
        }

        Ok(())
    }

    /// Removes the item at position `idx` in the array,
    /// shifting down the elements from `array[idx+1]`.
    ///
    /// Fails if `idx` is out of range.
    pub fn remove(&self, idx: Integer) -> CallResult<'vm, ()> {
        self.0.push_into_stack();

        let ret = unsafe { sq_arrayremove(self.0.sq.vm, -1, idx) };
        self.0.sq.pop(1);
        if ret.is_error() {
            return Err(CallError::get_runtime_error(self.0.sq));
        }

        Ok(())
    }

    /// Reverses all the items of the array in place.
    pub fn reverse(&self) {
        self.0.push_into_stack();

        let ret = unsafe { sq_arrayreverse(self.0.sq.vm, -1) };
        self.0.sq.pop(1);
        assert!(!ret.is_error(), "sq_arrayreverse failed on {:?}", self);
    }

    /// Clears all items from the array.
    pub fn clear(&self) {
        self.0.push_into_stack();

        let ret = unsafe { sq_clear(self.0.sq.vm, -1) };
        self.0.sq.pop(1);
        assert!(!ret.is_error(), "sq_clear failed on {:?}", self);
    }

    /// Grows or shrinks the array to the `new_size`.
    ///
    /// If `new_size` is > `array.len()`, the new slots will be filled
    /// with `null`s.
    ///
    /// Fails if `new_size` is negative.
    pub fn resize(&self, new_size: Integer) -> CallResult<'vm, ()> {
        self.0.push_into_stack();

        let ret = unsafe { sq_arrayresize(self.0.sq.vm, -1, new_size) };
        self.0.sq.pop(1);
        if ret.is_error() {
            return Err(CallError::get_runtime_error(self.0.sq));
        }

        Ok(())
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
        let v: Integer = arr.get(1).unwrap();
        assert_eq!(v, 456);
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn array_set() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = sq.eval("return [123, 456, 789]").unwrap();
        arr.set(1, 444).unwrap();
        let v: Integer = arr.get(1).unwrap();
        assert_eq!(v, 444);
        assert_eq!(sq.stack_depth(), 0);
    }

    /// Arrays in Squirrel have to be grown manually.
    /// Setting an array index out of range will cause
    /// a runtime error.
    #[test]
    fn array_set_oob() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = sq.eval("return [123, 456, 789]").unwrap();
        let err = arr.set(4, 444).unwrap_err();
        assert!(matches!(err, CallError::Runtime(Value::String(_))));
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn array_append() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = sq.eval("return [123, 456, 789]").unwrap();
        arr.append(321);
        let v: Integer = arr.get(3).unwrap();
        assert_eq!(v, 321);
        assert_eq!(arr.len(), 4);
    }

    #[test]
    fn array_pop() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = sq.eval("return [123, 456, 789]").unwrap();
        let v: Integer = arr.pop().unwrap();
        assert_eq!(v, 789);
        assert_eq!(arr.len(), 2);
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn array_pop_empty() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = sq.eval("return []").unwrap();
        let err = arr.pop::<()>().unwrap_err();
        assert!(matches!(err, CallError::Runtime(_)));
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn array_len() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = sq.eval("return [123, 456, 789]").unwrap();
        assert_eq!(arr.len(), 3);
    }

    #[test]
    fn array_insert() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = sq.eval("return [1, 2, 3]").unwrap();
        arr.insert(1, 50).unwrap();
        let v1: Integer = arr.get(1).unwrap();
        let v2: Integer = arr.get(2).unwrap();
        assert_eq!(v1, 50);
        assert_eq!(v2, 2);
        assert_eq!(arr.len(), 4);
    }

    #[test]
    fn array_remove() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = sq.eval("return [1, 2, 3]").unwrap();
        arr.remove(1).unwrap();
        let v: Integer = arr.get(1).unwrap();
        assert_eq!(v, 3);
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn array_reverse() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = sq.eval("return [1, 2, 3]").unwrap();
        arr.reverse();
        let v: Integer = arr.get(0).unwrap();
        assert_eq!(v, 3);
    }

    #[test]
    fn array_clear() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = sq.eval("return [1, 2, 3]").unwrap();
        arr.clear();
        assert_eq!(arr.len(), 0);
    }

    #[test]
    fn array_resize_grow() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = sq.eval("return [1, 2, 3]").unwrap();
        arr.resize(5).unwrap();
        assert_eq!(arr.len(), 5);
        let v: Value<'_> = arr.get(4).unwrap();
        assert_eq!(v, Value::Null);
    }

    #[test]
    fn array_resize_shrink() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = sq.eval("return [1, 2, 3]").unwrap();
        arr.resize(2).unwrap();
        assert_eq!(arr.len(), 2);
    }
}
