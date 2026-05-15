use squirrels_sys::{SQBool, sq_getbool, sq_getfloat, sq_getinteger, tagSQObjectType_OT_NULL};

use crate::{Error, Float, Integer, Object, Result, Squirrel, Value};

/// Trait for types convertible from [`Value`].
pub trait FromSquirrel<'vm>: Sized {
    /// Performs the conversion.
    fn from_squirrel(value: Value<'vm>, sq: &'vm Squirrel) -> Result<Self>;

    #[doc(hidden)]
    #[inline]
    unsafe fn from_stack(idx: Integer, sq: &'vm Squirrel) -> Result<Self> {
        let value = Object::from_stack(idx, sq).into_value();
        Self::from_squirrel(value, sq)
    }
}

impl FromSquirrel<'_> for () {
    fn from_squirrel(value: Value<'_>, _sq: &'_ Squirrel) -> Result<Self> {
        if let Value::Null = value {
            Ok(())
        } else {
            Err(Error::Type { expected: "null" })
        }
    }

    unsafe fn from_stack(idx: Integer, sq: &'_ Squirrel) -> Result<Self> {
        let obj = Object::from_stack(idx, sq);

        if obj.obj._type == tagSQObjectType_OT_NULL {
            Ok(())
        } else {
            Err(Error::Type { expected: "null" })
        }
    }
}

impl FromSquirrel<'_> for Integer {
    fn from_squirrel(value: Value<'_>, _sq: &'_ Squirrel) -> Result<Self> {
        if let Value::Integer(n) = value {
            Ok(n)
        } else {
            Err(Error::Type {
                expected: "integer",
            })
        }
    }

    unsafe fn from_stack(idx: Integer, sq: &'_ Squirrel) -> Result<Self> {
        sq.assert_valid_idx(idx);

        let mut n: Integer = 0;
        if unsafe { sq_getinteger(sq.vm, idx, &mut n) }.is_error() {
            Err(Error::Type {
                expected: "integer",
            })
        } else {
            Ok(n)
        }
    }
}

impl FromSquirrel<'_> for Float {
    fn from_squirrel(value: Value<'_>, _sq: &'_ Squirrel) -> Result<Self> {
        if let Value::Float(n) = value {
            Ok(n)
        } else {
            Err(Error::Type { expected: "float" })
        }
    }

    unsafe fn from_stack(idx: Integer, sq: &'_ Squirrel) -> Result<Self> {
        sq.assert_valid_idx(idx);

        let mut n: Float = 0.0;
        if unsafe { sq_getfloat(sq.vm, idx, &mut n) }.is_error() {
            Err(Error::Type { expected: "float" })
        } else {
            Ok(n)
        }
    }
}

impl FromSquirrel<'_> for bool {
    fn from_squirrel(value: Value<'_>, _sq: &'_ Squirrel) -> Result<Self> {
        if let Value::Bool(b) = value {
            Ok(b)
        } else {
            Err(Error::Type { expected: "bool" })
        }
    }

    unsafe fn from_stack(idx: Integer, sq: &'_ Squirrel) -> Result<Self> {
        sq.assert_valid_idx(idx);

        let mut b: SQBool = 0;
        if unsafe { sq_getbool(sq.vm, idx, &mut b) }.is_error() {
            Err(Error::Type { expected: "bool" })
        } else {
            Ok(b != 0)
        }
    }
}
