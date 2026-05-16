use squirrels_sys::{SQFalse, sq_get, sq_newslot, tagSQObjectType_OT_ARRAY};

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

        let ret = unsafe { sq_newslot(self.0.sq.vm, -3, SQFalse as _) };
        if ret.is_error() {
            self.0.sq.pop(3);

            return Err(crate::CallError::Runtime(get_runtime_error(self.0.sq)));
        }

        self.0.sq.pop(1);

        Ok(())
    }
}
