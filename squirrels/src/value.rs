use crate::{
    Array, Class, Closure, Float, Generator, Instance, Integer, NativeClosure, String, Table,
    Thread, UserData, UserPointer, WeakRef,
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
            Self::Integer(n) => f.debug_tuple("Integer").field(n).finish(),
            Self::Float(n) => f.debug_tuple("Float").field(n).finish(),
            Self::Bool(b) => f.debug_tuple("Bool").field(b).finish(),
            Self::String(s) => f.debug_tuple("String").field(s).finish(),
            Self::Table(_) => write!(f, "Table"),
            Self::Array(_) => write!(f, "Array"),
            Self::UserData(_) => write!(f, "UserData"),
            Self::Closure(_) => write!(f, "Closure"),
            Self::NativeClosure(_) => write!(f, "NativeClosure"),
            Self::Generator(_) => write!(f, "Generator"),
            Self::UserPointer(_) => write!(f, "UserPointer"),
            Self::Thread(_) => write!(f, "Thread"),
            Self::Class(_) => write!(f, "Class"),
            Self::Instance(_) => write!(f, "Instance"),
            Self::WeakRef(_) => write!(f, "WeakRef"),
        }
    }
}
