use squirrels_sys::{SQChar, sq_getstringandsize, sq_pushstring, tagSQObjectType_OT_STRING};

use crate::{
    Error, FromSquirrel, Integer, IntoSquirrel, Object, ObjectType, PushIntoStack, Result,
    Squirrel, Value,
};

/// A ref-counted handle to a Squirrel string.
///
/// Strings in Squirrel are immutable and interned until they are GC'd.
/// Two different string objects that are alive at the same time will share
/// the same pointer.
///
/// Unlike Rust strings, Squirrel strings may not be valid UTF-8.
///
/// Squirrel strings may contain `NUL` bytes, but standard Squirrel functions
/// like `format` and `print` will truncate at the first `NUL` byte when rendering.
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

    /// Creates and returns an interned string.
    ///
    /// Squirrel strings can be arbitrary `[u8]` data including embedded `NUL` bytes,
    /// so in addition to `&str` and `&String`, you can also pass a plain `&[u8]` here.
    pub fn new(sq: &'vm Squirrel, bytes: impl AsRef<[u8]>) -> Self {
        let bytes = bytes.as_ref();
        unsafe { sq_pushstring(sq.vm, bytes.as_ptr() as *const _, bytes.len() as _) };
        let obj = unsafe { String::from_stack(-1, sq) };
        sq.pop(1);
        obj.expect("expecting the string we just pushed")
    }

    /// Gets the bytes that make up this string.
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr as *const _, self.len) }
    }

    /// Gets a `&str` if the Squirrel string is valid UTF-8.
    pub fn to_str(&self) -> std::result::Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(self.as_bytes())
    }

    /// Converts this string to an owned [`std::string::String`].
    ///
    /// Any non-Unicode sequences are replaced with [`char::REPLACEMENT_CHARACTER`].
    pub fn to_string_lossy(&self) -> std::string::String {
        std::string::String::from_utf8_lossy(self.as_bytes()).into_owned()
    }

    /// Gets the length of the string.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the string is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
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

impl AsRef<[u8]> for String<'_> {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
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
    fn from_squirrel(value: crate::Value<'vm>, sq: &'vm Squirrel) -> Result<Self> {
        if let Value::String(s) = value {
            s.obj.sq.assert_same_vm(sq);
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
        Value::String(String::new(sq, self))
    }
}

impl IntoSquirrel<'_> for std::string::String {
    fn into_squirrel(self, sq: &'_ Squirrel) -> Value<'_> {
        Value::String(String::new(sq, &self))
    }
}

impl IntoSquirrel<'_> for &[u8] {
    fn into_squirrel(self, sq: &'_ Squirrel) -> Value<'_> {
        Value::String(String::new(sq, self))
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
    let str: String = sq.eval("return \"test\"").unwrap();
    assert_eq!(str.to_str().unwrap(), "test");
}

#[test]
fn test_value_from_object() {
    use crate::Value;

    let sq = Squirrel::new(1024);
    let v: Value = sq.eval("return 123").unwrap();
    assert_eq!(v, Value::Integer(123));
}

#[test]
fn test_string_equality() {
    let sq = Squirrel::new(1024);
    let s1: String = sq.eval("return \"test\"").unwrap();
    let s2: String = sq.eval("return \"test\"").unwrap();
    assert_eq!(s1, s2);
}
