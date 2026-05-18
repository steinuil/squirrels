use squirrels_sys::{
    SQFalse, SQTrue, sq_createinstance, sq_getbase, sq_newclass, tagSQObjectType_OT_CLASS,
};

use crate::{
    CallResult, FromSquirrel as _, Instance, IntoArgs, IntoSquirrel, Object, Squirrel,
    closure::call_closure, traits::impl_object_traits,
};

/// A ref-counted handle to a Squirrel class.
#[derive(Debug, Clone, PartialEq)]
pub struct Class<'vm>(pub(crate) Object<'vm>);

impl_object_traits!(Class, tagSQObjectType_OT_CLASS, "class");

impl<'vm> Class<'vm> {
    /// Creates a new class object.
    pub fn new(sq: &'vm Squirrel) -> Self {
        let ret = unsafe { sq_newclass(sq.vm, SQFalse as _) };
        assert!(!ret.is_error(), "sq_newclass failed");

        let k = unsafe { Self::from_stack(-1, sq) };
        sq.pop(1);
        k.expect("expecting the class we just pushed")
    }

    /// Creates a new class object that inherits from `base`.
    pub fn with_base(base: Class<'vm>) -> Self {
        let sq = base.0.sq;
        unsafe { base.push_into_stack(sq) };

        let ret = unsafe { sq_newclass(sq.vm, SQTrue as _) };
        assert!(!ret.is_error(), "sq_newclass failed");

        let k = unsafe { Self::from_stack(-1, sq) };
        sq.pop(2);
        k.expect("expecting the class we just pushed")
    }

    /// Returns the base class of this class or `None`.
    pub fn base(&self) -> Option<Class<'vm>> {
        self.0.push_into_stack();

        let ret = unsafe { sq_getbase(self.0.sq.vm, -1) };
        assert!(!ret.is_error(), "sq_getbase failed on {:?}", self);

        let k = unsafe { Option::<Self>::from_stack(-1, self.0.sq) };
        self.0.sq.pop(2);
        k.expect("expecting the class we just pushed or null")
    }

    /// Creates an instance of this class *without calling its constructor*.
    ///
    /// The constructor can be called manually with [`Instance::call_method`]
    /// on `"constructor"`.
    pub fn raw_instantiate(&self) -> Instance<'vm> {
        self.0.push_into_stack();

        let ret = unsafe { sq_createinstance(self.0.sq.vm, -1) };
        assert!(!ret.is_error(), "sq_createinstance failed on {:?}", self);

        let i = unsafe { Instance::from_stack(-1, self.0.sq) };
        self.0.sq.pop(2);
        i.expect("expecting the instance we just created")
    }

    /// Creates an instance of this class and calls its constructor.
    ///
    /// Fails if the constructor throws an error.
    pub fn instantiate<Args>(&self, args: Args) -> CallResult<'vm, Instance<'vm>>
    where
        Args: IntoArgs<'vm>,
    {
        self.0.push_into_stack();

        // env placeholder
        unsafe { ().push_into_stack(self.0.sq) };

        call_closure(&self.0, args)
    }
}
