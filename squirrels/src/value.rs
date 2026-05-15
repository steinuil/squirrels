use crate::{
    Array, Class, Closure, Float, FromSquirrel, Generator, Instance, Integer, IntoSquirrel,
    NativeClosure, Object, PushIntoStack, Result, Squirrel, String, Table, Thread, UserData,
    UserPointer, WeakRef,
};

/// A dynamically typed Squirrel value.
///
/// The non-primitive variants (e.g. string/table/closure/userdata) contain handle types
/// into the internal Squirrel state.
///
/// It is a logic error to mix handle types between separate `Squirrel` instances,
/// and doing so will result in a panic.
#[derive(Clone, PartialEq)]
pub enum Value<'vm> {
    Null,
    Integer(Integer),
    Float(Float),
    Bool(bool),
    String(String<'vm>),
    Table(Table<'vm>),
    Array(Array<'vm>),
    UserData(UserData<'vm>),
    Closure(Closure<'vm>),
    NativeClosure(NativeClosure<'vm>),
    Generator(Generator<'vm>),
    UserPointer(UserPointer),
    Thread(Thread<'vm>),
    Class(Class<'vm>),
    Instance(Instance<'vm>),
    WeakRef(WeakRef<'vm>),
}

impl<'vm> Value<'vm> {
    pub(crate) fn as_object(&self) -> Option<&Object<'vm>> {
        match self {
            Value::String(String { obj, .. })
            | Value::Table(Table(obj))
            | Value::Array(Array(obj))
            | Value::UserData(UserData(obj))
            | Value::Closure(Closure(obj))
            | Value::NativeClosure(NativeClosure(obj))
            | Value::Generator(Generator(obj))
            | Value::Thread(Thread(obj))
            | Value::Class(Class(obj))
            | Value::Instance(Instance(obj))
            | Value::WeakRef(WeakRef(obj)) => Some(obj),
            Value::Null
            | Value::Integer(_)
            | Value::Float(_)
            | Value::Bool(_)
            | Value::UserPointer(_) => None,
        }
    }
}

impl std::fmt::Debug for Value<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Null => write!(f, "Null"),
            Self::Integer(n) => write!(f, "Integer({})", n),
            Self::Float(n) => write!(f, "Float({})", n),
            Self::Bool(b) => write!(f, "Bool({})", b),
            Self::String(s) => f.debug_tuple("String").field(s).finish(),
            Self::Table(o) => write!(f, "Table({:?})", o.0),
            Self::Array(o) => write!(f, "Array({:?})", o.0),
            Self::UserData(o) => write!(f, "UserData({:?})", o.0),
            Self::Closure(o) => write!(f, "Closure({:?})", o.0),
            Self::NativeClosure(o) => write!(f, "NativeClosure({:?})", o.0),
            Self::Generator(o) => write!(f, "Generator({:?})", o.0),
            Self::UserPointer(o) => write!(f, "UserPointer({:p})", o.0),
            Self::Thread(o) => write!(f, "Thread({:?})", o.0),
            Self::Class(o) => write!(f, "Class({:?})", o.0),
            Self::Instance(o) => write!(f, "Instance({:?})", o.0),
            Self::WeakRef(o) => write!(f, "WeakRef({:?})", o.0),
        }
    }
}

impl<'vm> FromSquirrel<'vm> for Value<'vm> {
    fn from_squirrel(value: Value<'vm>, sq: &'vm Squirrel) -> Result<Self> {
        if let Some(obj) = value.as_object() {
            obj.sq.assert_same_vm(sq);
        }

        Ok(value)
    }

    unsafe fn from_stack(idx: Integer, sq: &'vm Squirrel) -> Result<Self> {
        Ok(Object::from_stack(idx, sq).into_value())
    }
}

impl<'vm> IntoSquirrel<'vm> for Value<'vm> {
    fn into_squirrel(self, sq: &'vm Squirrel) -> Value<'vm> {
        if let Some(obj) = self.as_object() {
            obj.sq.assert_same_vm(sq);
        }

        self
    }
}

unsafe impl<'vm> PushIntoStack for Value<'vm> {
    fn push_into_stack(self, sq: &Squirrel) {
        sq.push_value(&self);
    }
}
