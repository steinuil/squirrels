use std::marker::PhantomData;

use squirrels_sys::{
    SQFalse, SQTrue, sq_clear, sq_clone, sq_deleteslot, sq_get, sq_getdelegate, sq_getsize,
    sq_newslot, sq_newtable, sq_newtableex, sq_next, sq_pushnull, sq_rawdeleteslot, sq_rawget,
    sq_rawset, sq_set, sq_setdelegate, tagSQObjectType_OT_TABLE,
};

use crate::{
    CallError, CallResult, FromSquirrel, Integer, IntoSquirrel, Object, Result, Squirrel,
    traits::impl_object_traits,
};

/// A ref-counted handle to a Squirrel table.
///
/// Tables are associative containers implemented as pairs of key/value called *slots*.
/// Keys can be any type except `null`.
///
/// # Delegates and metamethods
///
/// A delegate is a parent table that allow defining special behaviors for its child in a
/// similar manner to Lua's metatables.
///
/// A table can only have one delegate, but a delegate table can have its own delegate
/// forming a chain.
///
/// ```rust
/// # use squirrels::{Squirrel, Table, Integer};
/// # let sq = Squirrel::new(1024);
/// let root = Table::new(&sq);
/// root.set("key", 1).unwrap();
///
/// let child = Table::new(&sq);
/// child.set_delegate(Some(root.clone())).unwrap();
///
/// // Fields not defined on `child` are looked up on `root`.
/// assert_eq!(child.get::<_, Integer>("key").unwrap(), 1);
///
/// let grandchild = Table::new(&sq);
/// grandchild.set_delegate(Some(child)).unwrap();
///
/// // Lookup is performed transitively on each parent of the delegate chain.
/// assert_eq!(grandchild.get::<_, Integer>("key").unwrap(), 1);
///
/// grandchild.set("key", 3).unwrap();
/// assert_eq!(grandchild.get::<_, Integer>("key").unwrap(), 3);
/// ```
///
/// Delegates are consulted when calling [`get`](Table::get), [`set`](Table::set),
/// [`assign`](Table::assign), [`delete`](Table::delete), and [`clone`](Table::clone).
///
/// Each of these methods' behaviors can be customized by defining their respective
/// **metamethod**, which is invoked in certain situations specific to the method.
/// Refer to these methods' doc comments for details.
///
/// Metamethods are functions bound to specially-named keys in a table.
///
/// [Squirrel documentation on delegates and metamethods](http://www.squirrel-lang.org/squirreldoc/reference/language/delegation.html).
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
    /// # Delegates and metamethods
    ///
    /// If `key` is not set directly set on this table, the lookup walks the table's delegate
    /// chain until it finds a delegate that defines `key`.
    ///
    /// If no ancestor delegate define `key`, the deepest `_get` metamethod found in the chain
    /// is invoked with `key` as its argument and `this` bound to its immediate child in the
    /// chain. If `_get` throws `null`, the second deepest `_get` metamethod found in the chain
    /// is invoked until the chain returns to the original table.
    ///
    /// Use the [`raw_get`](Self::raw_get) method to bypass delegate lookup.
    ///
    /// # Errors
    ///
    /// Fails if `key` is not found in the table nor in its delegate chain, or
    /// if a `_get` metamethod throws an error.
    pub fn get<K, V>(&self, key: K) -> CallResult<'vm, V>
    where
        K: IntoSquirrel<'vm>,
        V: FromSquirrel<'vm>,
    {
        self.0.push_into_stack();
        unsafe { key.push_into_stack(self.0.sq) };

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
    /// # Delegates and metamethods
    ///
    /// If `key` is not set directly on this table, the table has a delegate, and the
    /// *immediate* delegate defines a `_newslot` metamethod, `_newslot` is called with
    /// this `key` and `value` as arguments and `this` bound to this table. Otherwise,
    /// the slot is created directly on the table.
    ///
    /// # Errors
    ///
    /// Fails if `key` is `null` or if the delegates's `_newslot` metamethod throws
    /// an error.
    pub fn set<K, V>(&self, key: K, value: V) -> CallResult<'vm, ()>
    where
        K: IntoSquirrel<'vm>,
        V: IntoSquirrel<'vm>,
    {
        self.0.push_into_stack();
        unsafe { key.push_into_stack(self.0.sq) };
        unsafe { value.push_into_stack(self.0.sq) };

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
    /// # Delegates and metamethods
    ///
    /// If `key` is not set directly set on this table, the lookup walks the table's delegate
    /// chain until it finds a delegate that defines `key` and sets that slot's value.
    ///
    /// If no ancestor delegate define `key`, the deepest `_set` metamethod found in the chain
    /// is invoked with `key` and `value` as its arguments and `this` bound to its immediate child
    /// in the chain. If `_set` throws `null`, the second deepest `_set` metamethod found in the chain
    /// is invoked until the chain returns to the original table.
    ///
    /// Use the [`raw_set`](Self::raw_set) method to bypass delegate lookup.
    ///
    /// # Errors
    ///
    /// Fails if:
    /// * `key` is `null`.
    /// * There is no `key` slot in the table or its ancestor delegates, and no `_set`
    ///   metamethod handled the assignment.
    /// * A `_set` metamethod throws an error.
    pub fn assign<K, V>(&self, key: K, value: V) -> CallResult<'vm, ()>
    where
        K: IntoSquirrel<'vm>,
        V: IntoSquirrel<'vm>,
    {
        self.0.push_into_stack();
        unsafe { key.push_into_stack(self.0.sq) };
        unsafe { value.push_into_stack(self.0.sq) };

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
    /// # Delegates and metamethods
    ///
    /// If this table's delegate defines the `_delslot` metamethod, it is invoked
    /// instead of removing the slot from the table directly, regardless of whether
    /// `key` is present in the table, and its return value is returned in place of
    /// the removed value.
    ///
    /// Use the [`raw_delete`](Self::raw_delete) method to bypass delegate lookup.
    ///
    /// # Errors
    ///
    /// Fails if:
    /// * `key` is `null`.
    /// * There is no `key` slot in the table and no `_delslot` metamethod is defined.
    /// * The `_delslot` metamethod throws an error.
    /// * The conversion from `V` fails.
    pub fn delete<K, V>(&self, key: K) -> CallResult<'vm, V>
    where
        K: IntoSquirrel<'vm>,
        V: FromSquirrel<'vm>,
    {
        let prev_top = self.0.sq.stack_depth();
        self.0.push_into_stack();
        unsafe { key.push_into_stack(self.0.sq) };

        let ret = unsafe { sq_deleteslot(self.0.sq.vm, -2, SQTrue as _) };
        if ret.is_error() {
            // sq_deleteslot leaves the stack in an inconsistent state based on
            // the error it throws.
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

    /// Gets the value associated to `key` from the table, bypassing the delegate lookup.
    ///
    /// Fails if `key` is `null` or no value is associated to `key`.
    pub fn raw_get<K, V>(&self, key: K) -> CallResult<'vm, V>
    where
        K: IntoSquirrel<'vm>,
        V: FromSquirrel<'vm>,
    {
        self.0.push_into_stack();
        unsafe { key.push_into_stack(self.0.sq) };

        let ret = unsafe { sq_rawget(self.0.sq.vm, -2) };
        if ret.is_error() {
            self.0.sq.pop(1);
            return Err(CallError::get_runtime_error(self.0.sq));
        }

        let val = unsafe { V::from_stack(-1, self.0.sq) };
        self.0.sq.pop(2);
        Ok(val?)
    }

    /// Sets the value associated to an already existing `key` in the table to `value`,
    /// bypassing delegate lookup.
    ///
    /// Fails if `key` is `null` or no value is associated to `key`.
    pub fn raw_set<K, V>(&self, key: K, value: V) -> CallResult<'vm, ()>
    where
        K: IntoSquirrel<'vm>,
        V: IntoSquirrel<'vm>,
    {
        self.0.push_into_stack();
        unsafe { key.push_into_stack(self.0.sq) };
        unsafe { value.push_into_stack(self.0.sq) };

        let ret = unsafe { sq_rawset(self.0.sq.vm, -3) };
        self.0.sq.pop(1);
        if ret.is_error() {
            return Err(CallError::get_runtime_error(self.0.sq));
        }

        Ok(())
    }

    /// Removes the slot associated with `key` in the table and returns its value,
    /// bypassing the delegate lookup.
    ///
    /// Returns `None` both when `key` did not exist in the table, and when
    /// `key` existed and was associated to `null`.
    ///
    /// Fails if the conversion from `V` failed.
    pub fn raw_delete<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: IntoSquirrel<'vm>,
        V: FromSquirrel<'vm>,
    {
        self.0.push_into_stack();
        unsafe { key.push_into_stack(self.0.sq) };

        let ret = unsafe { sq_rawdeleteslot(self.0.sq.vm, -2, SQTrue as _) };
        assert!(!ret.is_error(), "sq_rawdeleteslot failed for {:?}", self);

        let val = unsafe { Option::<V>::from_stack(-1, self.0.sq) };
        self.0.sq.pop(2);
        Ok(val?)
    }

    /// Check whether a slot associated to `key` exists in the table.
    pub fn contains_key<K>(&self, key: K) -> bool
    where
        K: IntoSquirrel<'vm>,
    {
        self.0.push_into_stack();
        unsafe { key.push_into_stack(self.0.sq) };

        !unsafe { sq_rawget(self.0.sq.vm, -2) }.is_error()
    }

    /// Iterate on the table's slots.
    pub fn iter<K, V>(&self) -> TableSlots<'vm, K, V> {
        self.0.push_into_stack();
        unsafe { sq_pushnull(self.0.sq.vm) };
        TableSlots {
            sq: self.0.sq,
            _kv: PhantomData,
        }
    }

    /// Sets or clears this table's delegate.
    ///
    /// Fails if assigning this delegate would create a reference cycle.
    pub fn set_delegate(&self, delegate: Option<Table<'vm>>) -> CallResult<'vm, ()> {
        self.0.push_into_stack();
        unsafe { delegate.push_into_stack(self.0.sq) };

        let ret = unsafe { sq_setdelegate(self.0.sq.vm, -2) };
        if ret.is_error() {
            // sq_setdelegate does not pop on error
            self.0.sq.pop(2);
            return Err(CallError::get_runtime_error(self.0.sq));
        }

        self.0.sq.pop(1);
        Ok(())
    }

    /// Returns this table's delegate or `None` if it doesn't have one.
    pub fn get_delegate(&self) -> Option<Table<'vm>> {
        self.0.push_into_stack();
        let ret = unsafe { sq_getdelegate(self.0.sq.vm, -1) };
        assert!(!ret.is_error(), "sq_getdelegate failed on {:?}", self);

        let delegate = unsafe { Option::<Table<'_>>::from_stack(-1, self.0.sq) };
        self.0.sq.pop(2);
        delegate.expect("expected table or null")
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

    /// Create a shallow copy of this `Table` object.
    ///
    /// # Delegates and metamethods
    ///
    /// If this table's delegate defines the `_cloned` metamethod, it is invoked on
    /// the newly cloned table after it has been created; see [`Table::set_delegate`]
    /// for details.
    ///
    /// # Errors
    ///
    /// Fails if the `_cloned` metamethod throws an error.
    pub fn clone_value(&self) -> CallResult<'vm, Self> {
        self.0.push_into_stack();

        let ret = unsafe { sq_clone(self.0.sq.vm, -1) };
        if ret.is_error() {
            self.0.sq.pop(1);
            return Err(CallError::get_runtime_error(self.0.sq));
        }

        let new_table = unsafe { Self::from_stack(-1, self.0.sq) };
        self.0.sq.pop(2);

        Ok(new_table.expect("expected table after sq_clone"))
    }
}

/// An iterator over the slots of a [`Table`].
pub struct TableSlots<'vm, K, V> {
    sq: &'vm Squirrel,
    _kv: PhantomData<(K, V)>,
}

impl<K, V> Drop for TableSlots<'_, K, V> {
    fn drop(&mut self) {
        // Pop the table and the generator at the end
        // of the iteration.
        self.sq.pop(2);
    }
}

impl<'vm, K, V> Iterator for TableSlots<'vm, K, V>
where
    K: FromSquirrel<'vm>,
    V: FromSquirrel<'vm>,
{
    type Item = Result<(K, V)>;

    fn next(&mut self) -> Option<Self::Item> {
        let ret = unsafe { sq_next(self.sq.vm, -2) };
        if ret.is_error() {
            return None;
        }

        let key = unsafe { K::from_stack(-2, self.sq) };
        let val = unsafe { V::from_stack(-1, self.sq) };
        self.sq.pop(2);

        match (key, val) {
            (Ok(key), Ok(val)) => Some(Ok((key, val))),
            (Err(e), _) => Some(Err(e)),
            (_, Err(e)) => Some(Err(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Table;
    use crate::{CallError, Integer, Squirrel, String};

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

    #[test]
    fn table_iter() {
        let sq = Squirrel::new(1024);
        let t: Table = sq.eval("local t = {a=1, b=2, c=3}; return t").unwrap();
        let mut collected = t
            .iter::<String<'_>, Integer>()
            .map(|r| {
                let (k, v) = r.unwrap();

                (k.to_string_lossy(), v)
            })
            .collect::<Vec<_>>();

        collected.sort();

        assert_eq!(
            &collected,
            &[
                ("a".to_string(), 1),
                ("b".to_string(), 2),
                ("c".to_string(), 3),
            ]
        );
        assert_eq!(sq.stack_depth(), 0);
    }
}
