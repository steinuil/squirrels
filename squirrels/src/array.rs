use squirrels_sys::{sq_get, tagSQObjectType_OT_ARRAY};

use crate::{
    Error, FromSquirrel, Integer, IntoSquirrel, Object, PushIntoStack, Result, Squirrel, Value,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Array<'vm>(pub(crate) Object<'vm>);

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
}

impl Eq for Array<'_> {}

impl<'vm> FromSquirrel<'vm> for Array<'vm> {
    fn from_squirrel(value: Value<'vm>, _sq: &'vm Squirrel) -> Result<Self> {
        if let Value::Array(a) = value {
            Ok(a)
        } else {
            Err(Error::Type { expected: "array" })
        }
    }

    unsafe fn from_stack(idx: Integer, sq: &'vm Squirrel) -> Result<Self> {
        let object = Object::from_stack(idx, sq);

        if object.obj._type == tagSQObjectType_OT_ARRAY {
            Ok(Array(object))
        } else {
            Err(Error::Type { expected: "array" })
        }
    }
}

impl<'vm> IntoSquirrel<'vm> for Array<'vm> {
    fn into_squirrel(self, sq: &'vm Squirrel) -> Value<'vm> {
        self.0.sq.assert_same_vm(sq);
        Value::Array(self)
    }
}

unsafe impl<'vm> PushIntoStack for Array<'vm> {
    fn push_into_stack(self, sq: &Squirrel) {
        self.0.sq.assert_same_vm(sq);
        self.0.push_into_stack();
    }
}
