use std::ffi::c_void;

use squirrels_sys::{
    HSQOBJECT, sq_addref, sq_getrefcount, sq_getstackobj, sq_pushobject, sq_release,
    sq_resetobject, tagSQObjectType_OT_ARRAY, tagSQObjectType_OT_BOOL, tagSQObjectType_OT_CLASS,
    tagSQObjectType_OT_CLOSURE, tagSQObjectType_OT_FLOAT, tagSQObjectType_OT_GENERATOR,
    tagSQObjectType_OT_INSTANCE, tagSQObjectType_OT_INTEGER, tagSQObjectType_OT_NATIVECLOSURE,
    tagSQObjectType_OT_NULL, tagSQObjectType_OT_STRING, tagSQObjectType_OT_TABLE,
    tagSQObjectType_OT_THREAD, tagSQObjectType_OT_USERDATA, tagSQObjectType_OT_USERPOINTER,
    tagSQObjectType_OT_WEAKREF,
};

use crate::{
    Array, Class, Closure, Generator, Instance, Integer, NativeClosure, Squirrel, String, Table,
    Thread, UnsignedInteger, UserData, UserPointer, Value, WeakRef,
};

/// Handle to a Squirrel ref-counted object.
pub struct Object<'vm> {
    pub(crate) sq: &'vm Squirrel,
    pub(crate) obj: HSQOBJECT,
}

impl<'vm> Object<'vm> {
    /// Gets an object from stack index `idx`.
    pub(crate) fn from_stack(sq: &'vm Squirrel, idx: Integer) -> Self {
        sq.assert_valid_idx(idx);

        // Initialize the object
        let mut obj: HSQOBJECT = unsafe { std::mem::zeroed() };
        unsafe { sq_resetobject(&mut obj) };

        // Get it from the stack
        let ret = unsafe { sq_getstackobj(sq.vm, idx, &mut obj) };
        assert!(!ret.is_error(), "sq_getstackobj failed for idx {idx}");

        // Increment the refcount
        unsafe { sq_addref(sq.vm, &mut obj) };

        Self { sq, obj }
    }

    /// Pushes the object to the top of the stack.
    pub(crate) fn push(&self) {
        unsafe { sq_pushobject(self.sq.vm, self.obj) };
    }

    /// Returns the [`ObjectType`] of this object.
    pub fn kind(&self) -> ObjectType {
        #[allow(non_upper_case_globals)]
        match self.obj._type {
            tagSQObjectType_OT_NULL => ObjectType::Null,
            tagSQObjectType_OT_INTEGER => ObjectType::Integer,
            tagSQObjectType_OT_FLOAT => ObjectType::Float,
            tagSQObjectType_OT_BOOL => ObjectType::Bool,
            tagSQObjectType_OT_STRING => ObjectType::String,
            tagSQObjectType_OT_TABLE => ObjectType::Table,
            tagSQObjectType_OT_ARRAY => ObjectType::Array,
            tagSQObjectType_OT_USERDATA => ObjectType::UserData,
            tagSQObjectType_OT_CLOSURE => ObjectType::Closure,
            tagSQObjectType_OT_NATIVECLOSURE => ObjectType::NativeClosure,
            tagSQObjectType_OT_GENERATOR => ObjectType::Generator,
            tagSQObjectType_OT_USERPOINTER => ObjectType::Generator,
            tagSQObjectType_OT_THREAD => ObjectType::Thread,
            tagSQObjectType_OT_CLASS => ObjectType::Class,
            tagSQObjectType_OT_INSTANCE => ObjectType::Instance,
            tagSQObjectType_OT_WEAKREF => ObjectType::WeakRef,
            t => panic!("invalid object type: {t:?}"),
        }
    }

    /// Gets the number of references to this object.
    pub fn ref_count(&self) -> UnsignedInteger {
        let mut obj = self.obj;
        unsafe { sq_getrefcount(self.sq.vm, &mut obj) }
    }

    /// Converts this object into a [`Value`].
    pub fn into_value(self) -> Value<'vm> {
        match self.kind() {
            ObjectType::Null => Value::Null,
            ObjectType::Integer => Value::Integer(unsafe { self.obj._unVal.nInteger }),
            ObjectType::Float => Value::Float(unsafe { self.obj._unVal.fFloat }),
            ObjectType::Bool => Value::Bool(unsafe { self.obj._unVal.nInteger } != 0),
            ObjectType::String => Value::String(
                String::from_object(self).expect("OT_STRING object materializes as String"),
            ),
            ObjectType::Table => Value::Table(Table(self)),
            ObjectType::Array => Value::Array(Array(self)),
            ObjectType::UserData => Value::UserData(UserData(self)),
            ObjectType::Closure => Value::Closure(Closure(self)),
            ObjectType::NativeClosure => Value::NativeClosure(NativeClosure(self)),
            ObjectType::Generator => Value::Generator(Generator(self)),
            ObjectType::UserPointer => {
                Value::UserPointer(UserPointer(unsafe { self.obj._unVal.pUserPointer }))
            }
            ObjectType::Thread => Value::Thread(Thread(self)),
            ObjectType::Class => Value::Class(Class(self)),
            ObjectType::Instance => Value::Instance(Instance(self)),
            ObjectType::WeakRef => Value::WeakRef(WeakRef(self)),
        }
    }

    pub fn as_pointer(&self) -> *const c_void {
        match self.kind() {
            ObjectType::Null | ObjectType::Integer | ObjectType::Float | ObjectType::Bool => {
                std::ptr::null()
            }
            ObjectType::UserPointer => unsafe { self.obj._unVal.pUserPointer },
            ObjectType::String
            | ObjectType::Table
            | ObjectType::Array
            | ObjectType::UserData
            | ObjectType::Closure
            | ObjectType::NativeClosure
            | ObjectType::Generator
            | ObjectType::Thread
            | ObjectType::Class
            | ObjectType::Instance
            | ObjectType::WeakRef => unsafe { self.obj._unVal.pRefCounted as *const c_void },
        }
    }
}

impl Clone for Object<'_> {
    fn clone(&self) -> Self {
        let mut obj = self.obj;
        unsafe { sq_addref(self.sq.vm, &mut obj) };
        Self { sq: self.sq, obj }
    }
}

impl Drop for Object<'_> {
    fn drop(&mut self) {
        unsafe { sq_release(self.sq.vm, &mut self.obj) };
    }
}

impl PartialEq for Object<'_> {
    fn eq(&self, other: &Self) -> bool {
        if self.sq.vm != other.sq.vm {
            return false;
        }

        if self.obj._type != other.obj._type {
            return false;
        }

        match self.kind() {
            ObjectType::Null => true,
            ObjectType::Integer => unsafe { self.obj._unVal.nInteger == other.obj._unVal.nInteger },
            ObjectType::Float => unsafe { self.obj._unVal.fFloat == other.obj._unVal.fFloat },
            ObjectType::Bool => unsafe {
                (self.obj._unVal.nInteger != 0) == (other.obj._unVal.nInteger != 0)
            },
            ObjectType::UserPointer => unsafe {
                self.obj._unVal.pUserPointer == other.obj._unVal.pUserPointer
            },
            ObjectType::String
            | ObjectType::Table
            | ObjectType::Array
            | ObjectType::UserData
            | ObjectType::Closure
            | ObjectType::NativeClosure
            | ObjectType::Generator
            | ObjectType::Thread
            | ObjectType::Class
            | ObjectType::Instance
            | ObjectType::WeakRef => unsafe {
                self.obj._unVal.pRefCounted == other.obj._unVal.pRefCounted
            },
        }
    }
}

impl std::fmt::Debug for Object<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind() {
            ObjectType::Null => write!(f, "Object(Null)"),
            ObjectType::Integer => write!(f, "Object({})", unsafe { self.obj._unVal.nInteger }),
            ObjectType::Float => write!(f, "Object({})", unsafe { self.obj._unVal.fFloat }),
            ObjectType::Bool => write!(f, "Object({})", unsafe { self.obj._unVal.nInteger } != 0),
            ObjectType::UserPointer
            | ObjectType::String
            | ObjectType::Table
            | ObjectType::Array
            | ObjectType::UserData
            | ObjectType::Closure
            | ObjectType::NativeClosure
            | ObjectType::Generator
            | ObjectType::Thread
            | ObjectType::Class
            | ObjectType::Instance
            | ObjectType::WeakRef => write!(f, "Object({:p})", self.as_pointer()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectType {
    Null,
    Integer,
    Float,
    Bool,
    String,
    Table,
    Array,
    UserData,
    Closure,
    NativeClosure,
    Generator,
    UserPointer,
    Thread,
    Class,
    Instance,
    WeakRef,
}
