use squirrels_sys::{
    SQFalse, SQTrue, sq_clear, sq_deleteslot, sq_get, sq_getsize, sq_newslot, sq_newtable,
    sq_newtableex, sq_rawdeleteslot, sq_rawget, sq_rawset, sq_set, tagSQObjectType_OT_TABLE,
};

use crate::{
    CallError, CallResult, FromSquirrel, Integer, IntoSquirrel, Object, Result, Squirrel,
    traits::impl_object_traits,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Table<'vm>(pub(crate) Object<'vm>);

impl_object_traits!(Table, tagSQObjectType_OT_TABLE, "table");

impl<'vm> Table<'vm> {
    /// Creates a new empty `Table`.
    pub fn new(sq: &'vm Squirrel) -> Table<'vm> {
        unsafe { sq_newtable(sq.vm) };
        let obj = unsafe { Self::from_stack(-1, sq) };
        sq.pop(1);
        obj.expect("expecting the table we just created")
    }

    /// Creates a new empty `Table` with an initial capacity.
    ///
    /// This prevents unnecessary rehashing when the number of slots required is known
    /// at creation time.
    ///
    /// # Panics
    ///
    /// Panics if `initial_capacity` is negative.
    pub fn with_capacity(sq: &'vm Squirrel, initial_capacity: Integer) -> Table<'vm> {
        assert!(
            initial_capacity >= 0,
            "Table::with_capacity: initial_capacity must be non-negative, got {initial_capacity}"
        );
        unsafe { sq_newtableex(sq.vm, initial_capacity) };
        let obj = unsafe { Self::from_stack(-1, sq) };
        sq.pop(1);
        obj.expect("expecting the table we just created")
    }

    /// Gets the value associated to `key` from the table.
    ///
    /// This might invoke the `_get` delegate on the table if `key` is not
    /// already present in the table.
    /// Use the [`raw_get`](Self::raw_get) method if that is not desired.
    ///
    /// Fails if no value is associated to `key`, the `_get` delegate is not present TODO
    pub fn get<K, V>(&self, key: K) -> CallResult<'vm, V>
    where
        K: IntoSquirrel<'vm>,
        V: FromSquirrel<'vm>,
    {
        self.0.push_into_stack();
        key.push_into_stack(self.0.sq);

        let ret = unsafe { sq_get(self.0.sq.vm, -2) };
        if ret.is_error() {
            self.0.sq.pop(1);
            return Err(CallError::get_runtime_error(self.0.sq));
        }

        let val = unsafe { V::from_stack(-1, self.0.sq) };
        self.0.sq.pop(2);
        Ok(val?)
    }

    /// Creates or overwrites the value associated to `key` with `value` in the table.
    ///
    /// Equivalent to `table.key <- value` in Squirrel scripts.
    ///
    /// Can only fail if `key` is `null`.
    ///
    /// This might invoke the `_newslot` delegate on the table if `key` is not
    /// already present in the table.
    pub fn set<K, V>(&self, key: K, value: V) -> CallResult<'vm, ()>
    where
        K: IntoSquirrel<'vm>,
        V: IntoSquirrel<'vm>,
    {
        self.0.push_into_stack();
        key.push_into_stack(self.0.sq);
        value.push_into_stack(self.0.sq);

        let ret = unsafe { sq_newslot(self.0.sq.vm, -3, SQFalse as _) };
        if ret.is_error() {
            // sq_newslot only pops k+v on success
            self.0.sq.pop(3);

            return Err(CallError::get_runtime_error(self.0.sq));
        }

        self.0.sq.pop(1);
        Ok(())
    }

    /// Sets the value associated to an already existing `key` to `value` in the table.
    ///
    /// Equivalent to `table.key = value` in Squirrel scripts.
    ///
    /// Fails if:
    /// * `key` is `null`
    /// * There is no `key` slot in the table.
    pub fn assign<K, V>(&self, key: K, value: K) -> CallResult<'vm, ()>
    where
        K: IntoSquirrel<'vm>,
        V: IntoSquirrel<'vm>,
    {
        self.0.push_into_stack();
        key.push_into_stack(self.0.sq);
        value.push_into_stack(self.0.sq);

        let ret = unsafe { sq_set(self.0.sq.vm, -3) };
        if ret.is_error() {
            // sq_set only pops k+v on success
            self.0.sq.pop(3);

            return Err(CallError::get_runtime_error(self.0.sq));
        }

        self.0.sq.pop(1);
        Ok(())
    }

    /// Removes the slot associated with `key` in the table and returns its value.
    ///
    /// Fails if:
    /// * `key` is `null`.
    /// * There is no `key` slot in the table.
    /// * The conversion from `V` fails.
    pub fn delete<K, V>(&self, key: K) -> CallResult<'vm, V>
    where
        K: IntoSquirrel<'vm>,
        V: FromSquirrel<'vm>,
    {
        let prev_top = self.0.sq.stack_depth();
        self.0.push_into_stack();
        key.push_into_stack(self.0.sq);

        let ret = unsafe { sq_deleteslot(self.0.sq.vm, -2, SQTrue as _) };
        if ret.is_error() {
            // sq_deleteslot leaves the stack in an inconsistent state based on
            // the error it raises.
            self.0.sq.resize_stack(prev_top);
            return Err(CallError::get_runtime_error(self.0.sq));
        }

        let val = unsafe { V::from_stack(-1, self.0.sq) };
        self.0.sq.pop(2);
        Ok(val?)
    }

    /// Deletes all the slots in the table, leaving it empty.
    pub fn clear(&self) {
        self.0.push_into_stack();

        let ret = unsafe { sq_clear(self.0.sq.vm, -1) };
        self.0.sq.pop(1);
        assert!(!ret.is_error(), "sq_clear failed on {:?}", self);
    }

    pub fn raw_get<K, V>(&self, key: K) -> CallResult<'vm, V>
    where
        K: IntoSquirrel<'vm>,
        V: FromSquirrel<'vm>,
    {
        self.0.push_into_stack();
        key.push_into_stack(self.0.sq);

        let ret = unsafe { sq_rawget(self.0.sq.vm, -2) };
        if ret.is_error() {
            self.0.sq.pop(1);
            return Err(CallError::get_runtime_error(self.0.sq));
        }

        let val = unsafe { V::from_stack(-1, self.0.sq) };
        self.0.sq.pop(2);
        Ok(val?)
    }

    pub fn raw_set<K, V>(&self, key: K, value: V) -> CallResult<'vm, ()>
    where
        K: IntoSquirrel<'vm>,
        V: IntoSquirrel<'vm>,
    {
        self.0.push_into_stack();
        key.push_into_stack(self.0.sq);
        value.push_into_stack(self.0.sq);

        let ret = unsafe { sq_rawset(self.0.sq.vm, -3) };
        self.0.sq.pop(1);
        if ret.is_error() {
            return Err(CallError::get_runtime_error(self.0.sq));
        }

        Ok(())
    }

    pub fn raw_delete<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: IntoSquirrel<'vm>,
        V: FromSquirrel<'vm>,
    {
        self.0.push_into_stack();
        key.push_into_stack(self.0.sq);

        let ret = unsafe { sq_rawdeleteslot(self.0.sq.vm, -2, SQTrue as _) };
        assert!(!ret.is_error(), "sq_rawdeleteslot failed for {:?}", self);

        let val = unsafe { Option::<V>::from_stack(-1, self.0.sq) };
        self.0.sq.pop(2);
        Ok(val?)
    }

    /// Returns the number of slots in the table.
    pub fn len(&self) -> Integer {
        self.0.push_into_stack();
        let len = unsafe { sq_getsize(self.0.sq.vm, -1) };
        self.0.sq.pop(1);
        len
    }

    /// Returns `true` if the table is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use crate::{CallError, Integer, Squirrel};

    #[test]
    fn table_get() {
        let sq = Squirrel::new(1024);
        sq.eval::<()>("a <- 1").unwrap();
        let v: Integer = sq.root_table().get("a").unwrap();
        assert_eq!(v, 1)
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
        let y: Integer = sq.root_table().get("y").unwrap();
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
}
