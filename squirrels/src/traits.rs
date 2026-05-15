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

/// Trait for types that can be pushed into the Squirrel stack.
pub unsafe trait PushIntoStack {
    /// Pushes the value to the top of the Squirrel stack.
    fn push_into_stack(self, sq: &Squirrel);
}

/// Trait for types convertible to [`Value`].
pub trait IntoSquirrel<'vm>: PushIntoStack + Sized {
    /// Performs the conversion.
    fn into_squirrel(self, sq: &'vm Squirrel) -> Value<'vm>;
}

// Types that do not have a lifetime bound on `'vm` can have
// a blanket impl of `PushIntoStack`.
unsafe impl<T> PushIntoStack for T
where
    T: for<'vm> IntoSquirrel<'vm>,
{
    fn push_into_stack(self, sq: &Squirrel) {
        let v = self.into_squirrel(sq);
        sq.push_value(&v);
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

impl IntoSquirrel<'_> for () {
    fn into_squirrel(self, _sq: &'_ Squirrel) -> Value<'_> {
        Value::Null
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

impl IntoSquirrel<'_> for Integer {
    fn into_squirrel(self, _sq: &'_ Squirrel) -> Value<'_> {
        Value::Integer(self)
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

impl IntoSquirrel<'_> for Float {
    fn into_squirrel(self, _sq: &'_ Squirrel) -> Value<'_> {
        Value::Float(self)
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

impl IntoSquirrel<'_> for bool {
    fn into_squirrel(self, _sq: &'_ Squirrel) -> Value<'_> {
        Value::Bool(self)
    }
}

/// Implement object traits on object newtype wrappers.
macro_rules! impl_object_traits {
    ($type:ident, $tag:expr, $name:literal) => {
        impl Eq for $type<'_> {}

        impl<'vm> $crate::FromSquirrel<'vm> for $type<'vm> {
            fn from_squirrel(
                value: $crate::Value<'vm>,
                sq: &'vm $crate::Squirrel,
            ) -> $crate::Result<Self> {
                if let $crate::Value::$type(o) = value {
                    o.0.sq.assert_same_vm(sq);
                    Ok(o)
                } else {
                    Err($crate::Error::Type { expected: $name })
                }
            }

            unsafe fn from_stack(
                idx: $crate::Integer,
                sq: &'vm $crate::Squirrel,
            ) -> $crate::Result<Self> {
                let object = $crate::Object::from_stack(idx, sq);

                if object.obj._type == $tag {
                    Ok($type(object))
                } else {
                    Err($crate::Error::Type { expected: "array" })
                }
            }
        }

        impl<'vm> $crate::IntoSquirrel<'vm> for $type<'vm> {
            fn into_squirrel(self, sq: &'vm $crate::Squirrel) -> $crate::Value<'vm> {
                self.0.sq.assert_same_vm(sq);
                $crate::Value::$type(self)
            }
        }

        unsafe impl<'vm> $crate::PushIntoStack for $type<'vm> {
            fn push_into_stack(self, sq: &$crate::Squirrel) {
                self.0.sq.assert_same_vm(sq);
                self.0.push_into_stack();
            }
        }
    };
}

pub(crate) use impl_object_traits;

mod sealed {
    pub trait Sealed {}
}

pub trait IntoArgs: sealed::Sealed {
    fn push_args(self, sq: &Squirrel) -> Integer;
}

pub trait FromArgs<'vm>: sealed::Sealed + Sized {
    fn from_args(count: Integer, sq: &'vm Squirrel) -> Result<Self>;
}

impl sealed::Sealed for () {}

impl IntoArgs for () {
    fn push_args(self, _sq: &Squirrel) -> Integer {
        0
    }
}

impl FromArgs<'_> for () {
    fn from_args(count: Integer, _sq: &Squirrel) -> Result<Self> {
        if count == 0 {
            Ok(())
        } else {
            Err(Error::Type {
                expected: "0 arguments",
            })
        }
    }
}

impl<T> sealed::Sealed for Vec<T> {}

impl<'vm, T: IntoSquirrel<'vm>> IntoArgs for Vec<T> {
    fn push_args(self, sq: &Squirrel) -> Integer {
        let len = self.len() as Integer;

        for item in self {
            item.push_into_stack(sq);
        }

        len
    }
}

impl<'vm, T: FromSquirrel<'vm>> FromArgs<'vm> for Vec<T> {
    fn from_args(count: Integer, sq: &'vm Squirrel) -> Result<Self> {
        let mut args = Vec::new();

        for i in 0..count {
            args.push(unsafe { T::from_stack(2 + i, sq) }?);
        }

        Ok(args)
    }
}

macro_rules! count_args {
    ($($_:tt),+) => {
        <[()]>::len(&[$(count_args!(@unit $_)),+])
    };
    (@unit $_:tt) => { () };
}

macro_rules! impl_args_tuple {
    ( $( $field:tt = $name:ident ),+ $( , )? ) => {
        impl< $( $name ),+ > sealed::Sealed for ( $( $name, )+ ) {}

        impl<'vm, $( $name: IntoSquirrel<'vm> ),+ > IntoArgs for ( $( $name, )+ ) {
            fn push_args(self, sq: &Squirrel) -> Integer {
                $( self.$field.push_into_stack(sq); )+
                count_args!( $( $name ),+ ) as Integer
            }
        }

        impl<'vm, $( $name: FromSquirrel<'vm> ),+ > FromArgs<'vm> for ( $( $name, )+ ) {
            fn from_args(count: Integer, sq: &'vm Squirrel) -> Result<Self> {
                if count == (count_args!( $( $name ),+ ) as Integer) {
                    Ok(( $( unsafe { $name::from_stack($field + 2, sq) }?, )+ ))
                } else {
                    return Err(Error::Type {
                        expected: concat!(stringify!(count_args!( $($name),+ )), " arguments"),
                    })
                }
            }
        }
    }
}

impl_args_tuple!(0 = T0);
impl_args_tuple!(0 = T0, 1 = T1);
impl_args_tuple!(0 = T0, 1 = T1, 2 = T2);
impl_args_tuple!(0 = T0, 1 = T1, 2 = T2, 3 = T3);
impl_args_tuple!(0 = T0, 1 = T1, 2 = T2, 3 = T3, 4 = T4);
impl_args_tuple!(0 = T0, 1 = T1, 2 = T2, 3 = T3, 4 = T4, 5 = T5);
impl_args_tuple!(0 = T0, 1 = T1, 2 = T2, 3 = T3, 4 = T4, 5 = T5, 6 = T6);
impl_args_tuple!(
    0 = T0,
    1 = T1,
    2 = T2,
    3 = T3,
    4 = T4,
    5 = T5,
    6 = T6,
    7 = T7,
);
impl_args_tuple!(
    0 = T0,
    1 = T1,
    2 = T2,
    3 = T3,
    4 = T4,
    5 = T5,
    6 = T6,
    7 = T7,
    8 = T8,
);
impl_args_tuple!(
    0 = T0,
    1 = T1,
    2 = T2,
    3 = T3,
    4 = T4,
    5 = T5,
    6 = T6,
    7 = T7,
    8 = T8,
    9 = T9,
);
impl_args_tuple!(
    0 = T0,
    1 = T1,
    2 = T2,
    3 = T3,
    4 = T4,
    5 = T5,
    6 = T6,
    7 = T7,
    8 = T8,
    9 = T9,
    10 = T10,
);
impl_args_tuple!(
    0 = T0,
    1 = T1,
    2 = T2,
    3 = T3,
    4 = T4,
    5 = T5,
    6 = T6,
    7 = T7,
    8 = T8,
    9 = T9,
    10 = T10,
    11 = T11,
);
impl_args_tuple!(
    0 = T0,
    1 = T1,
    2 = T2,
    3 = T3,
    4 = T4,
    5 = T5,
    6 = T6,
    7 = T7,
    8 = T8,
    9 = T9,
    10 = T10,
    11 = T11,
    12 = T12,
);
impl_args_tuple!(
    0 = T0,
    1 = T1,
    2 = T2,
    3 = T3,
    4 = T4,
    5 = T5,
    6 = T6,
    7 = T7,
    8 = T8,
    9 = T9,
    10 = T10,
    11 = T11,
    12 = T12,
    13 = T13,
);
impl_args_tuple!(
    0 = T0,
    1 = T1,
    2 = T2,
    3 = T3,
    4 = T4,
    5 = T5,
    6 = T6,
    7 = T7,
    8 = T8,
    9 = T9,
    10 = T10,
    11 = T11,
    12 = T12,
    13 = T13,
    14 = T14,
);
impl_args_tuple!(
    0 = T0,
    1 = T1,
    2 = T2,
    3 = T3,
    4 = T4,
    5 = T5,
    6 = T6,
    7 = T7,
    8 = T8,
    9 = T9,
    10 = T10,
    11 = T11,
    12 = T12,
    13 = T13,
    14 = T14,
    15 = T15,
);
