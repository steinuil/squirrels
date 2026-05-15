use squirrels_sys::{sq_get, tagSQObjectType_OT_ARRAY};

use crate::{
    FromSquirrel, Integer, Object, PushIntoStack as _, Result, traits::impl_object_traits,
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
}
