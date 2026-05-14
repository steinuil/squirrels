use squirrels_sys::{SQFalse, sq_get, sq_newslot};

use crate::{CallError, CallResult, FromSquirrel, IntoSquirrel, Object, Result, get_runtime_error};

pub struct Table<'vm>(pub(crate) Object<'vm>);

impl<'vm> Table<'vm> {
    pub fn get<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: IntoSquirrel,
        V: FromSquirrel<'vm>,
    {
        self.0.push();
        key.push_to(self.0.sq);

        let ret = unsafe { sq_get(self.0.sq.vm, -2) };
        if ret.is_error() {
            self.0.sq.pop(1);
            return Ok(None);
        }

        let val = V::from_stack(self.0.sq, -1);
        self.0.sq.pop(2);
        val.map(Some)
    }

    pub fn set<K, V>(&self, key: K, value: V) -> CallResult<'_, ()>
    where
        K: IntoSquirrel,
        V: IntoSquirrel,
    {
        self.0.push();
        key.push_to(self.0.sq);
        value.push_to(self.0.sq);

        let ret = unsafe { sq_newslot(self.0.sq.vm, -3, SQFalse as _) };
        if ret.is_error() {
            // sq_newslot only pops k+v on success
            self.0.sq.pop(3);

            return Err(CallError::Runtime(get_runtime_error(self.0.sq)));
        }

        // Pop the table
        self.0.sq.pop(1);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{CallError, Integer, Squirrel};

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
}
