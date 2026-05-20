use std::marker::PhantomData;

use squirrels_sys::{
    SQTrue, sq_arrayappend, sq_arrayinsert, sq_arraypop, sq_arrayremove, sq_arrayresize,
    sq_arrayreverse, sq_clear, sq_clone, sq_get, sq_getsize, sq_newarray, sq_set,
    tagSQObjectType_OT_ARRAY,
};

use crate::{
    CallResult, FromSquirrel, Integer, IntoSquirrel, Object, Squirrel, Value, errors::SqResultExt,
    traits::impl_object_traits,
};

/// A ref-counted handle to a Squirrel array.
///
/// [`Clone::clone`]ing this handle will create a new reference
/// to the underlying object.
/// To create a new `Array` object use [`clone_value`](Array::clone_value).
#[derive(Debug, Clone, PartialEq)]
pub struct Array<'vm>(pub(crate) Object<'vm>);

impl_object_traits!(Array, tagSQObjectType_OT_ARRAY, "array");

impl<'vm> Array<'vm> {
    /// Creates a new array of the specified `size` filled with `null`s.
    ///
    /// # Panics
    ///
    /// Panics if `size` is negative.
    pub fn new(sq: &'vm Squirrel, size: Integer) -> Array<'vm> {
        assert!(
            size >= 0,
            "Array::new: size must be non-negative, got {size}"
        );
        unsafe { sq_newarray(sq.vm, size) };

        let arr = unsafe { Self::from_stack(-1, sq) }
            .unwrap_or_else(|_| panic!("sq_newarray did not push an Array"));
        sq.pop(1);
        arr
    }

    /// Gets the value at position `idx` of the array.
    ///
    /// Fails if `idx` is out of range or the conversion from `T` failed.
    pub fn get<T: FromSquirrel<'vm>>(&self, idx: Integer) -> CallResult<'vm, T> {
        self.0.push_into_stack();
        unsafe { idx.push_into_stack(self.0.sq) };

        unsafe { sq_get(self.0.sq.vm, -2) }.to_runtime_error(self.0.sq, 1)?;

        let val = unsafe { T::from_stack(-1, self.0.sq) };
        self.0.sq.pop(2);
        Ok(val?)
    }

    /// Replaces the value at position `idx` of the array.
    ///
    /// Fails if `idx` is out of range.
    pub fn set<T: IntoSquirrel<'vm>>(&self, key: Integer, value: T) -> CallResult<'vm, ()> {
        self.0.push_into_stack();
        unsafe { key.push_into_stack(self.0.sq) };
        unsafe { value.push_into_stack(self.0.sq) };

        unsafe { sq_set(self.0.sq.vm, -3) }.to_runtime_error(self.0.sq, 3)?;

        self.0.sq.pop(1);

        Ok(())
    }

    /// Pushes a value to the back of the array.
    pub fn append<T: IntoSquirrel<'vm>>(&self, value: T) {
        self.0.push_into_stack();
        unsafe { value.push_into_stack(self.0.sq) };

        unsafe { sq_arrayappend(self.0.sq.vm, -2) }
            .expect(format_args!("sq_arrayappend failed on {:?}", self));
        self.0.sq.pop(1);
    }

    /// Removes the last element from the array and returns it.
    ///
    /// Fails if the array is empty.
    pub fn pop<T: FromSquirrel<'vm>>(&self) -> CallResult<'vm, T> {
        self.0.push_into_stack();

        unsafe { sq_arraypop(self.0.sq.vm, -1, SQTrue as _) }.to_runtime_error(self.0.sq, 1)?;

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
        unsafe { value.push_into_stack(self.0.sq) };

        unsafe { sq_arrayinsert(self.0.sq.vm, -2, idx) }.to_runtime_error(self.0.sq, 2)?;
        self.0.sq.pop(1);

        Ok(())
    }

    /// Removes the item at position `idx` in the array,
    /// shifting down the elements from `array[idx+1]`.
    ///
    /// Fails if `idx` is out of range.
    pub fn remove(&self, idx: Integer) -> CallResult<'vm, ()> {
        self.0.push_into_stack();

        unsafe { sq_arrayremove(self.0.sq.vm, -1, idx) }.to_runtime_error(self.0.sq, 1)?;
        self.0.sq.pop(1);

        Ok(())
    }

    /// Reverses all the items of the array in place.
    pub fn reverse(&self) {
        self.0.push_into_stack();

        unsafe { sq_arrayreverse(self.0.sq.vm, -1) }
            .expect(format_args!("sq_arrayreverse failed on {:?}", self));
        self.0.sq.pop(1);
    }

    /// Clears all items from the array.
    pub fn clear(&self) {
        self.0.push_into_stack();

        unsafe { sq_clear(self.0.sq.vm, -1) }.expect(format_args!("sq_clear failed on {:?}", self));
        self.0.sq.pop(1);
    }

    /// Grows or shrinks the array to the `new_size`.
    ///
    /// If `new_size` is > `array.len()`, the new slots will be filled
    /// with `null`s.
    ///
    /// Fails if `new_size` is negative.
    pub fn resize(&self, new_size: Integer) -> CallResult<'vm, ()> {
        self.0.push_into_stack();

        unsafe { sq_arrayresize(self.0.sq.vm, -1, new_size) }.to_runtime_error(self.0.sq, 1)?;
        self.0.sq.pop(1);

        Ok(())
    }

    /// Returns an iterator over the items of the array.
    ///
    /// The items are wrapped in a [`CallResult`], since they are lazily converted
    /// to the `V` type. If `V` is [`Value`], the iterator will always yield `Ok(Value)`
    /// unless the array is shrunk during iteration.
    ///
    /// Mutating the array length while iterating over it is safe, but it may cause
    /// the iterator to skip elements or return an error.
    /// If you need to mutate the length, collect first.
    pub fn iter<V: FromSquirrel<'vm>>(&self) -> ArrayItems<'vm, V> {
        ArrayItems {
            array: self.clone(),
            idx: 0,
            len: self.len(),
            _v: PhantomData,
        }
    }

    /// Create a shallow copy of this `Array` object.
    pub fn clone_value(&self) -> Array<'vm> {
        self.0.push_into_stack();

        unsafe { sq_clone(self.0.sq.vm, -1) }.expect(format_args!("sq_clone failed on {:?}", self));

        let new_arr = unsafe { Self::from_stack(-1, self.0.sq) };
        self.0.sq.pop(2);

        new_arr.expect("expected array after sq_clone")
    }
}

/// An iterator over the items of an [`Array`].
pub struct ArrayItems<'vm, V: FromSquirrel<'vm>> {
    array: Array<'vm>,
    idx: Integer,
    len: Integer,
    _v: PhantomData<V>,
}

impl<'vm, V: FromSquirrel<'vm>> Iterator for ArrayItems<'vm, V> {
    type Item = CallResult<'vm, V>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.len {
            return None;
        }
        let v: V = match self.array.get(self.idx) {
            Ok(v) => v,
            Err(e) => return Some(Err(e)),
        };
        self.idx += 1;
        Some(Ok(v))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len as _, Some(self.len as _))
    }
}

impl<'vm> IntoIterator for Array<'vm> {
    type Item = CallResult<'vm, Value<'vm>>;

    type IntoIter = ArrayItems<'vm, Value<'vm>>;

    fn into_iter(self) -> Self::IntoIter {
        let len = self.len();

        ArrayItems {
            array: self,
            idx: 0,
            len,
            _v: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Array;
    use crate::{CallError, CallResult, Integer, Squirrel, Value};

    #[test]
    fn array_new() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = Array::new(&sq, 0);
        assert_eq!(arr.len(), 0);
        assert_eq!(sq.stack_depth(), 0);
    }

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
    fn array_get_oob() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = sq.eval("return [123]").unwrap();
        let err = arr.get::<()>(1).unwrap_err();
        assert!(matches!(err, CallError::Runtime(_)));
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
        assert_eq!(sq.stack_depth(), 0);
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
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn array_remove() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = sq.eval("return [1, 2, 3]").unwrap();
        arr.remove(1).unwrap();
        let v: Integer = arr.get(1).unwrap();
        assert_eq!(v, 3);
        assert_eq!(arr.len(), 2);
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn array_reverse() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = sq.eval("return [1, 2, 3]").unwrap();
        arr.reverse();
        let v: Integer = arr.get(0).unwrap();
        assert_eq!(v, 3);
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn array_clear() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = sq.eval("return [1, 2, 3]").unwrap();
        arr.clear();
        assert_eq!(arr.len(), 0);
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn array_resize_grow() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = sq.eval("return [1, 2, 3]").unwrap();
        arr.resize(5).unwrap();
        assert_eq!(arr.len(), 5);
        let v: Value<'_> = arr.get(4).unwrap();
        assert_eq!(v, Value::Null);
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn array_resize_shrink() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = sq.eval("return [1, 2, 3]").unwrap();
        arr.resize(2).unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn array_iter() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = sq.eval("return [1, 2, 3]").unwrap();
        let vals = arr
            .iter()
            .collect::<CallResult<'_, Vec<Integer>>>()
            .unwrap();
        assert_eq!(&vals, &[1, 2, 3]);
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn array_into_iter() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = sq.eval("return [1, 2, 3]").unwrap();
        let vals = arr
            .into_iter()
            .collect::<CallResult<'_, Vec<Value<'_>>>>()
            .unwrap();
        assert_eq!(
            &vals,
            &[Value::Integer(1), Value::Integer(2), Value::Integer(3)]
        );
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn array_iter_err() {
        let sq = Squirrel::new(1024);
        let arr: Array<'_> = sq.eval("return [1, \"test\", 3]").unwrap();
        let err = arr
            .iter()
            .collect::<CallResult<'_, Vec<Integer>>>()
            .unwrap_err();
        assert!(matches!(err, CallError::Other(_)));
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn array_clone_value() {
        let sq = Squirrel::new(1024);
        let arr1: Array<'_> = sq.eval("return [1, 2, 3]").unwrap();
        let arr2 = arr1.clone_value();
        arr1.clear();

        assert_eq!(arr1.len(), 0);
        assert_eq!(arr2.len(), 3);
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn array_clone() {
        let sq = Squirrel::new(1024);
        let arr = Array::new(&sq, 0);
        let arr_clone = arr.clone();

        arr.clear();
        assert_eq!(arr_clone.len(), 0);
        assert_eq!(arr.0.ref_count(), 2);
        assert_eq!(sq.stack_depth(), 0);
    }
}
