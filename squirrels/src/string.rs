use squirrels_sys::{SQChar, sq_getstringandsize, sq_pushstring};

use crate::{Error, Integer, Object, ObjectType, Result, Squirrel};

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
        object.push();

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

    pub(crate) fn from_stack(sq: &'vm Squirrel, idx: Integer) -> Result<Self> {
        let object = Object::from_stack(sq, idx);
        if object.kind() != ObjectType::String {
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

    pub fn as_bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr as *const u8, self.len) }
    }

    pub fn to_str(&self) -> std::result::Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(self.as_bytes())
    }

    pub fn from_str(sq: &'vm Squirrel, str: &str) -> Self {
        unsafe { sq_pushstring(sq.vm, str.as_bytes().as_ptr() as *const i8, str.len() as _) };
        let obj = String::from_stack(sq, -1).expect("expecting the string we just pushed");
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

impl Eq for String<'_> {}

impl std::hash::Hash for String<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ptr.hash(state);
    }
}

impl std::fmt::Debug for String<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("String")
            .field(&std::string::String::from_utf8_lossy(self.as_bytes()))
            .finish()
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
