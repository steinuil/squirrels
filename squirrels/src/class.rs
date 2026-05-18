use squirrels_sys::{
    HSQMEMBERHANDLE, SQFalse, SQTrue, sq_createinstance, sq_getbase, sq_newclass,
    sq_setclassudsize, tagSQObjectType_OT_CLASS,
};

use crate::{
    CallResult, FromSquirrel, Instance, Integer, IntoArgs, IntoSquirrel, Object, Squirrel, String,
    closure::call_closure, errors::SqResultExt as _, traits::impl_object_traits,
};

/// A ref-counted handle to a Squirrel class.
#[derive(Debug, Clone, PartialEq)]
pub struct Class<'vm>(pub(crate) Object<'vm>);

impl_object_traits!(Class, tagSQObjectType_OT_CLASS, "class");

impl<'vm> Class<'vm> {
    /// Creates a new class object.
    pub fn new(sq: &'vm Squirrel) -> Self {
        unsafe { sq_newclass(sq.vm, SQFalse as _) }.expect(format_args!("sq_newclass failed"));

        let k = unsafe { Self::from_stack(-1, sq) };
        sq.pop(1);
        k.expect("expecting the class we just pushed")
    }

    /// Creates a new class object that inherits from `base`.
    pub fn with_base(base: Class<'vm>) -> Self {
        let sq = base.0.sq;
        base.0.push_into_stack();

        unsafe { sq_newclass(sq.vm, SQTrue as _) }
            .expect(format_args!("sq_newclass failed on {:?}", base));

        let k = unsafe { Self::from_stack(-1, sq) };
        sq.pop(2);
        k.expect("expecting the class we just pushed")
    }

    /// Returns the base class of this class or `None`.
    pub fn base(&self) -> Option<Class<'vm>> {
        self.0.push_into_stack();

        unsafe { sq_getbase(self.0.sq.vm, -1) }
            .expect(format_args!("sq_getbase failed on {:?}", self));

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

        unsafe { sq_createinstance(self.0.sq.vm, -1) }
            .expect(format_args!("sq_createinstance failed on {:?}", self));

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

        // Placeholder. This is replaced with the instance when
        // the constructor is called.
        unsafe { ().push_into_stack(self.0.sq) };

        call_closure(&self.0, args)
    }

    pub fn new_member<K, V, A>(
        &self,
        key: K,
        value: V,
        attr: A,
        is_static: bool,
    ) -> CallResult<'vm, ()>
    where
        K: IntoSquirrel<'vm>,
        V: IntoSquirrel<'vm>,
        A: IntoSquirrel<'vm>,
    {
        todo!()
    }

    pub fn raw_new_member<K, V, A>(
        &self,
        key: K,
        value: V,
        attr: A,
        is_static: bool,
    ) -> CallResult<'vm, ()>
    where
        K: IntoSquirrel<'vm>,
        V: IntoSquirrel<'vm>,
        A: IntoSquirrel<'vm>,
    {
        todo!()
    }

    pub fn new_slot<K, V>(&self, key: K, value: V, is_static: bool) -> CallResult<'vm, ()>
    where
        K: IntoSquirrel<'vm>,
        V: IntoSquirrel<'vm>,
    {
        todo!()
    }

    pub fn get<K, V>(&self, key: K) -> CallResult<'vm, V>
    where
        K: IntoSquirrel<'vm>,
        V: FromSquirrel<'vm>,
    {
        todo!()
    }
    pub fn set<K, V>(&self, key: K, value: V) -> CallResult<'vm, ()>
    where
        K: IntoSquirrel<'vm>,
        V: IntoSquirrel<'vm>,
    {
        todo!()
    }
    pub fn raw_get<K, V>(&self, key: K) -> CallResult<'vm, V>
    where
        K: IntoSquirrel<'vm>,
        V: FromSquirrel<'vm>,
    {
        todo!()
    }

    pub fn raw_set<K, V>(&self, key: K, value: V) -> CallResult<'vm, ()>
    where
        K: IntoSquirrel<'vm>,
        V: IntoSquirrel<'vm>,
    {
        todo!()
    }

    pub fn member_handle(&self, name: String<'vm>) -> CallResult<'vm, MemberHandle<'vm>> {
        todo!()
    }

    /// Sets the user data size of a class.
    ///
    /// # Panics
    ///
    /// Panics if `size` is negative.
    pub fn set_instance_user_data_size(&self, size: Integer) -> CallResult<'vm, ()> {
        assert!(
            size >= 0,
            "Class::set_instance_user_data_size: size must be non-negative, got {size}"
        );

        self.0.push_into_stack();
        unsafe { sq_setclassudsize(self.0.sq.vm, -1, size) }.to_runtime_error(self.0.sq, 1)?;
        self.0.sq.pop(1);
        Ok(())
    }
}

pub struct MemberHandle<'vm> {
    ptr: HSQMEMBERHANDLE,
    class: Class<'vm>,
}
