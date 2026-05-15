use squirrels_sys::{SQChar, sq_getstringandsize, sq_pushstring, tagSQObjectType_OT_STRING};

use crate::{
    Error, FromSquirrel, Integer, IntoSquirrel, Object, ObjectType, PushIntoStack, Result,
    Squirrel, Value,
};

pub struct String<'vm> {
    pub(crate) obj: Object<'vm>,
    pub(crate) ptr: *const SQChar,
    pub(crate) len: usize,
}

impl<'vm> String<'vm> {
    pub(crate) fn from_object(object: Object<'vm>) -> Result<Self> {
        if object.kind() != ObjectType::String {
            return Err(Error::Type { expected: "string" });
        }

        // First we must push the string onto the stack because we can't get its stack index
        // from its object handle, if it has any.
        object.push_into_stack();

        let mut ptr: *const SQChar = std::ptr::null();
        let mut len: Integer = 0;
        let ret = unsafe { sq_getstringandsize(object.sq.vm, -1, &mut ptr, &mut len) };

        // Pop before we check for an error to avoid leaving the stack in an invalid state.
        object.sq.pop(1);

        assert!(
            !ret.is_error(),
            "sq_getstringandsize failed on a verified OT_STRING"
        );

        Ok(Self {
            obj: object,
            ptr,
            len: len as usize,
        })
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr as *const u8, self.len) }
    }

    pub fn to_str(&self) -> std::result::Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(self.as_bytes())
    }

    pub fn to_string_lossy(&self) -> std::string::String {
        std::string::String::from_utf8_lossy(self.as_bytes()).into_owned()
    }

    pub fn from_str(sq: &'vm Squirrel, str: &str) -> Self {
        Self::from_bytes(sq, str.as_bytes())
    }

    pub fn from_bytes(sq: &'vm Squirrel, bytes: &[u8]) -> Self {
        unsafe { sq_pushstring(sq.vm, bytes.as_ptr() as *const i8, bytes.len() as _) };
        let obj =
            unsafe { String::from_stack(-1, sq) }.expect("expecting the string we just pushed");
        sq.pop(1);
        obj
    }
}

impl Clone for String<'_> {
    fn clone(&self) -> Self {
        Self {
            obj: self.obj.clone(),
            ptr: self.ptr,
            len: self.len,
        }
    }
}

impl PartialEq for String<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.ptr == other.ptr
    }
}

impl<T> PartialEq<T> for String<'_>
where
    T: AsRef<[u8]> + ?Sized,
{
    fn eq(&self, other: &T) -> bool {
        self.as_bytes() == other.as_ref()
    }
}

impl Eq for String<'_> {}

impl<T> PartialOrd<T> for String<'_>
where
    T: AsRef<[u8]> + ?Sized,
{
    fn partial_cmp(&self, other: &T) -> Option<std::cmp::Ordering> {
        self.as_bytes().partial_cmp(other.as_ref())
    }
}

impl PartialOrd for String<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for String<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_bytes().cmp(other.as_bytes())
    }
}

impl std::hash::Hash for String<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ptr.hash(state);
    }
}

impl std::fmt::Debug for String<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bytes = self.as_bytes();
        if let Ok(s) = str::from_utf8(bytes) {
            return s.fmt(f);
        }

        write!(f, "b")?;
        <bstr::BStr as std::fmt::Debug>::fmt(bstr::BStr::new(&bytes), f)
    }
}

impl<'vm> FromSquirrel<'vm> for String<'vm> {
    fn from_squirrel(value: crate::Value<'vm>, _sq: &'vm Squirrel) -> Result<Self> {
        if let Value::String(s) = value {
            Ok(s)
        } else {
            Err(Error::Type { expected: "string" })
        }
    }

    unsafe fn from_stack(idx: Integer, sq: &'vm Squirrel) -> Result<Self> {
        let object = Object::from_stack(idx, sq);
        if object.obj._type != tagSQObjectType_OT_STRING {
            return Err(Error::Type { expected: "string" });
        }

        let mut ptr: *const SQChar = std::ptr::null();
        let mut len: Integer = 0;
        let ret = unsafe { sq_getstringandsize(sq.vm, idx, &mut ptr, &mut len) };
        assert!(
            !ret.is_error(),
            "sq_getstringandsize failed on a verified OT_STRING"
        );

        Ok(Self {
            obj: object,
            ptr,
            len: len as usize,
        })
    }
}

impl<'vm> IntoSquirrel<'vm> for String<'vm> {
    fn into_squirrel(self, sq: &'vm Squirrel) -> Value<'vm> {
        self.obj.sq.assert_same_vm(sq);
        Value::String(self)
    }
}

impl IntoSquirrel<'_> for &str {
    fn into_squirrel(self, sq: &'_ Squirrel) -> Value<'_> {
        Value::String(String::from_str(sq, self))
    }
}

impl IntoSquirrel<'_> for std::string::String {
    fn into_squirrel(self, sq: &'_ Squirrel) -> Value<'_> {
        Value::String(String::from_str(sq, self.as_str()))
    }
}

impl IntoSquirrel<'_> for &[u8] {
    fn into_squirrel(self, sq: &'_ Squirrel) -> Value<'_> {
        Value::String(String::from_bytes(sq, self))
    }
}

unsafe impl<'vm> PushIntoStack for String<'vm> {
    fn push_into_stack(self, sq: &Squirrel) {
        self.obj.sq.assert_same_vm(sq);
        self.obj.push_into_stack();
    }
}

#[test]
fn test_string_from_stack() {
    let sq = Squirrel::new(1024);
    let str = sq.eval::<String>("return \"test\"").unwrap();
    assert_eq!(str.to_str().unwrap(), "test");
}

#[test]
fn test_value_from_object() {
    use crate::Value;

    let sq = Squirrel::new(1024);
    let v = sq.eval::<Value>("return 123").unwrap();
    assert!(matches!(v, Value::Integer(123)));
}
