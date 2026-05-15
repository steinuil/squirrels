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
    fn from_squirrel(value: Value<'vm>, _sq: &'vm Squirrel) -> Result<Self> {
        Ok(value)
    }

    unsafe fn from_stack(idx: Integer, sq: &'vm Squirrel) -> Result<Self> {
        Ok(Object::from_stack(idx, sq).into_value())
    }
}

impl<'vm> IntoSquirrel<'vm> for Value<'vm> {
    fn into_squirrel(self, _sq: &'vm Squirrel) -> Value<'vm> {
        self
    }
}

unsafe impl<'vm> PushIntoStack for Value<'vm> {
    fn push_into_stack(self, sq: &Squirrel) {
        match self {
            Value::Null => ().push_into_stack(sq),
            Value::Integer(n) => n.push_into_stack(sq),
            Value::Float(f) => f.push_into_stack(sq),
            Value::Bool(b) => b.push_into_stack(sq),
            Value::String(s) => s.push_into_stack(sq),
            Value::Table(table) => table.push_into_stack(sq),
            Value::Array(array) => array.push_into_stack(sq),
            Value::UserData(user_data) => user_data.push_into_stack(sq),
            Value::Closure(closure) => closure.push_into_stack(sq),
            Value::NativeClosure(native_closure) => native_closure.push_into_stack(sq),
            Value::Generator(generator) => generator.push_into_stack(sq),
            Value::UserPointer(user_pointer) => user_pointer.push_into_stack(sq),
            Value::Thread(thread) => thread.push_into_stack(sq),
            Value::Class(class) => class.push_into_stack(sq),
            Value::Instance(instance) => instance.push_into_stack(sq),
            Value::WeakRef(weak_ref) => weak_ref.push_into_stack(sq),
        }
    }
}
