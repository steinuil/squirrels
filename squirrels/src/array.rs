use squirrels_sys::sq_get;

use crate::{FromSquirrel, Integer, IntoSquirrel, Object, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct Array<'vm>(pub(crate) Object<'vm>);

impl<'vm> Array<'vm> {
    pub fn get<V: FromSquirrel<'vm>>(&self, idx: Integer) -> Result<Option<V>> {
        self.0.push();
        idx.push_to(self.0.sq);

        let ret = unsafe { sq_get(self.0.sq.vm, -2) };
        if ret.is_error() {
            self.0.sq.pop(1);

            return Ok(None);
        }

        let val = V::from_stack(self.0.sq, -1);
        self.0.sq.pop(2);
        val.map(Some)
    }
}

impl Eq for Array<'_> {}
