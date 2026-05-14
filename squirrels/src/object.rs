use squirrels_sys::{
    HSQOBJECT, sq_addref, sq_release, tagSQObjectType_OT_ARRAY, tagSQObjectType_OT_BOOL,
    tagSQObjectType_OT_CLASS, tagSQObjectType_OT_CLOSURE, tagSQObjectType_OT_FLOAT,
    tagSQObjectType_OT_GENERATOR, tagSQObjectType_OT_INSTANCE, tagSQObjectType_OT_INTEGER,
    tagSQObjectType_OT_NATIVECLOSURE, tagSQObjectType_OT_NULL, tagSQObjectType_OT_STRING,
    tagSQObjectType_OT_TABLE, tagSQObjectType_OT_THREAD, tagSQObjectType_OT_USERDATA,
    tagSQObjectType_OT_USERPOINTER, tagSQObjectType_OT_WEAKREF,
};

use crate::Squirrel;

pub struct Object<'vm> {
    sq: &'vm Squirrel,
    obj: HSQOBJECT,
}

impl<'vm> Object<'vm> {
    /// Return the [`ObjectType`] of this object.
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
