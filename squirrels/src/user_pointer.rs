use std::ffi::c_void;

use squirrels_sys::sq_getuserpointer;

use crate::{Error, FromSquirrel, Integer, IntoSquirrel, Result, Squirrel, Value};

#[derive(Debug, Clone, Copy, Hash)]
pub struct UserPointer(pub(crate) *mut c_void);

impl UserPointer {
    pub fn as_ptr(self) -> *mut c_void {
        self.0
    }
}

impl PartialEq for UserPointer {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.0 as *const _, other.0 as *const _)
    }
}

impl Eq for UserPointer {}

impl FromSquirrel<'_> for UserPointer {
    fn from_squirrel(value: Value<'_>, _sq: &'_ Squirrel) -> Result<Self> {
        if let Value::UserPointer(v) = value {
            Ok(v)
        } else {
            Err(Error::Type {
                expected: "userpointer",
            })
        }
    }

    unsafe fn from_stack(idx: Integer, sq: &'_ Squirrel) -> Result<Self> {
        sq.assert_valid_idx(idx);

        let mut p: *mut c_void = std::ptr::null_mut();
        if unsafe { sq_getuserpointer(sq.vm, idx, &mut p) }.is_error() {
            Err(Error::Type {
                expected: "userpointer",
            })
        } else {
            Ok(UserPointer(p))
        }
    }
}

impl IntoSquirrel<'_> for UserPointer {
    fn into_squirrel(self, _sq: &'_ Squirrel) -> Value<'_> {
        Value::UserPointer(self)
    }
}
