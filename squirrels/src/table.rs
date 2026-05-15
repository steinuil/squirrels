use squirrels_sys::{SQFalse, sq_get, sq_newslot, tagSQObjectType_OT_TABLE};

use crate::{
    CallError, CallResult, Error, FromSquirrel, IntoSquirrel, Object, PushIntoStack, Result,
    Squirrel, Value, get_runtime_error,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Table<'vm>(pub(crate) Object<'vm>);

impl<'vm> Table<'vm> {
    pub fn get<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: IntoSquirrel<'vm>,
        V: FromSquirrel<'vm>,
    {
        self.0.push_into_stack();
        key.push_into_stack(self.0.sq);

        let ret = unsafe { sq_get(self.0.sq.vm, -2) };
        if ret.is_error() {
            self.0.sq.pop(1);
            return Ok(None);
        }

        let val = unsafe { V::from_stack(-1, self.0.sq) };
        self.0.sq.pop(2);
        val.map(Some)
    }

    pub fn set<K, V>(&self, key: K, value: V) -> CallResult<'_, ()>
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

            return Err(CallError::Runtime(get_runtime_error(self.0.sq)));
        }

        // Pop the table
        self.0.sq.pop(1);

        Ok(())
    }
}

impl Eq for Table<'_> {}

impl<'vm> FromSquirrel<'vm> for Table<'vm> {
    fn from_squirrel(value: crate::Value<'vm>, _sq: &'vm Squirrel) -> Result<Self> {
        if let Value::Table(t) = value {
            Ok(t)
        } else {
            Err(Error::Type { expected: "table" })
        }
    }

    unsafe fn from_stack(idx: crate::Integer, sq: &'vm Squirrel) -> Result<Self> {
        let object = Object::from_stack(idx, sq);

        if object.obj._type == tagSQObjectType_OT_TABLE {
            Ok(Table(object))
        } else {
            Err(Error::Type { expected: "table" })
        }
    }
}

impl<'vm> IntoSquirrel<'vm> for Table<'vm> {
    fn into_squirrel(self, _sq: &'vm Squirrel) -> Value<'vm> {
        // TODO check for VM equality
        Value::Table(self)
    }
}

unsafe impl<'vm> PushIntoStack for Table<'vm> {
    fn push_into_stack(self, _sq: &Squirrel) {
        self.0.push_into_stack();
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
