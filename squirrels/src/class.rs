use squirrels_sys::{
    HSQMEMBERHANDLE, SQFalse, SQTrue, sq_createinstance, sq_get, sq_getbase, sq_getmemberhandle,
    sq_newclass, sq_newmember, sq_newslot, sq_rawget, sq_rawnewmember, sq_rawset, sq_set,
    sq_setclassudsize, tagSQObjectType_OT_CLASS,
};

use crate::{
    CallError, CallResult, FromSquirrel, Instance, Integer, IntoArgs, IntoSquirrel, Object,
    Squirrel, String, closure::call_closure, errors::SqResultExt as _, traits::impl_object_traits,
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
    pub fn with_base(base: &Class<'vm>) -> Self {
        let sq = base.0.sq;
        base.0.push_into_stack();

        unsafe { sq_newclass(sq.vm, SQTrue as _) }
            .expect(format_args!("sq_newclass failed on {:?}", base));

        let k = unsafe { Self::from_stack(-1, sq) };
        sq.pop(1);
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

    /// Creates a new member on this class with `key`, `value`, and `attr`.
    ///
    /// If `is_static` is `true`, the member is created as a static member.
    ///
    /// Fails if `key` is `null` or already exists on the class.
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
        let prev_top = self.0.sq.stack_depth();
        self.0.push_into_stack();
        unsafe { key.push_into_stack(self.0.sq) };
        unsafe { value.push_into_stack(self.0.sq) };
        unsafe { attr.push_into_stack(self.0.sq) };

        let ret = unsafe {
            sq_newmember(
                self.0.sq.vm,
                -4,
                if is_static { SQTrue } else { SQFalse } as _,
            )
        };
        if ret.is_error() {
            // sq_newmember leaves the stack in an inconsistent state depending
            // on the error path it took.
            self.0.sq.resize_stack(prev_top);
            return Err(CallError::get_runtime_error(self.0.sq));
        }

        self.0.sq.pop(1);
        Ok(())
    }

    /// Creates a new member on this class with `key`, `value`, and `attr`,
    /// bypassing the `_newmember` metamethod.
    ///
    /// If `is_static` is `true`, the member is created as a static member.
    ///
    /// Fails if `key` is `null` or already exists on the class.
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
        let prev_top = self.0.sq.stack_depth();
        self.0.push_into_stack();
        unsafe { key.push_into_stack(self.0.sq) };
        unsafe { value.push_into_stack(self.0.sq) };
        unsafe { attr.push_into_stack(self.0.sq) };

        let ret = unsafe {
            sq_rawnewmember(
                self.0.sq.vm,
                -4,
                if is_static { SQTrue } else { SQFalse } as _,
            )
        };
        if ret.is_error() {
            self.0.sq.resize_stack(prev_top);
            return Err(CallError::get_runtime_error(self.0.sq));
        }

        self.0.sq.pop(1);
        Ok(())
    }

    /// Creates or overwrites a slot on this class with `key` and `value`.
    ///
    /// If `is_static` is `true`, the slot is created as a static member.
    ///
    /// Fails if `key` is `null`.
    pub fn new_slot<K, V>(&self, key: K, value: V, is_static: bool) -> CallResult<'vm, ()>
    where
        K: IntoSquirrel<'vm>,
        V: IntoSquirrel<'vm>,
    {
        self.0.push_into_stack();
        unsafe { key.push_into_stack(self.0.sq) };
        unsafe { value.push_into_stack(self.0.sq) };

        // sq_newslot only pops k+v on success
        unsafe {
            sq_newslot(
                self.0.sq.vm,
                -3,
                if is_static { SQTrue } else { SQFalse } as _,
            )
        }
        .to_runtime_error(self.0.sq, 3)?;

        self.0.sq.pop(1);
        Ok(())
    }

    /// Gets the value associated to `key` on this class.
    ///
    /// Fails if `key` is not found.
    pub fn get<K, V>(&self, key: K) -> CallResult<'vm, V>
    where
        K: IntoSquirrel<'vm>,
        V: FromSquirrel<'vm>,
    {
        self.0.push_into_stack();
        unsafe { key.push_into_stack(self.0.sq) };

        unsafe { sq_get(self.0.sq.vm, -2) }.to_runtime_error(self.0.sq, 1)?;

        let val = unsafe { V::from_stack(-1, self.0.sq) };
        self.0.sq.pop(2);
        Ok(val?)
    }

    /// Sets the value associated to an already existing `key` on this class.
    ///
    /// Fails if `key` is `null` or not found.
    pub fn set<K, V>(&self, key: K, value: V) -> CallResult<'vm, ()>
    where
        K: IntoSquirrel<'vm>,
        V: IntoSquirrel<'vm>,
    {
        self.0.push_into_stack();
        unsafe { key.push_into_stack(self.0.sq) };
        unsafe { value.push_into_stack(self.0.sq) };

        // sq_set only pops k+v on success
        unsafe { sq_set(self.0.sq.vm, -3) }.to_runtime_error(self.0.sq, 3)?;

        self.0.sq.pop(1);
        Ok(())
    }

    /// Gets the value associated to `key` on this class, bypassing metamethods.
    ///
    /// Fails if `key` is `null` or not found.
    pub fn raw_get<K, V>(&self, key: K) -> CallResult<'vm, V>
    where
        K: IntoSquirrel<'vm>,
        V: FromSquirrel<'vm>,
    {
        self.0.push_into_stack();
        unsafe { key.push_into_stack(self.0.sq) };

        unsafe { sq_rawget(self.0.sq.vm, -2) }.to_runtime_error(self.0.sq, 1)?;

        let val = unsafe { V::from_stack(-1, self.0.sq) };
        self.0.sq.pop(2);
        Ok(val?)
    }

    /// Sets the value associated to an already existing `key` on this class,
    /// bypassing metamethods.
    ///
    /// Fails if `key` is `null`.
    pub fn raw_set<K, V>(&self, key: K, value: V) -> CallResult<'vm, ()>
    where
        K: IntoSquirrel<'vm>,
        V: IntoSquirrel<'vm>,
    {
        self.0.push_into_stack();
        unsafe { key.push_into_stack(self.0.sq) };
        unsafe { value.push_into_stack(self.0.sq) };

        unsafe { sq_rawset(self.0.sq.vm, -3) }.to_runtime_error(self.0.sq, 1)?;
        self.0.sq.pop(1);
        Ok(())
    }

    /// Returns a [`MemberHandle`] for the member of this class named `name`.
    ///
    /// Fails if no member named `name` exists on this class.
    pub fn member_handle(&self, name: String<'vm>) -> CallResult<'vm, MemberHandle<'vm>> {
        self.0.push_into_stack();
        unsafe { name.push_into_stack(self.0.sq) };

        let mut handle: HSQMEMBERHANDLE = unsafe { std::mem::zeroed() };
        // sq_getmemberhandle pops the key on success, leaves the stack
        // untouched on error.
        unsafe { sq_getmemberhandle(self.0.sq.vm, -2, &mut handle) }
            .to_runtime_error(self.0.sq, 2)?;

        self.0.sq.pop(1);
        Ok(MemberHandle {
            ptr: handle,
            class: self.clone(),
        })
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

    // TODO:
    // * sq_setattributes
    // * sq_getattributes
    // * sq_settypetag
    // * sq_gettypetag
    // * sq_setreleasehook
    // * sq_getreleasehook
}

pub struct MemberHandle<'vm> {
    ptr: HSQMEMBERHANDLE,
    class: Class<'vm>,
}

#[test]
fn class_new() {
    let sq = Squirrel::new(1024);
    Class::new(&sq);
    assert_eq!(sq.stack_depth(), 0);
}

#[test]
fn class_with_base() {
    let sq = Squirrel::new(1024);
    let base = Class::new(&sq);
    let class = Class::with_base(&base);

    assert_eq!(base, class.base().unwrap());
    assert_eq!(sq.stack_depth(), 0);
}

#[test]
fn class_instantiate() {
    let sq = Squirrel::new(1024);
    let class = Class::new(&sq);
    let instance = class.instantiate(()).unwrap();

    assert!(instance.instance_of(&class));
    assert_eq!(sq.stack_depth(), 0);
}

#[test]
fn class_raw_instantiate() {
    let sq = Squirrel::new(1024);
    let class = Class::new(&sq);
    let instance = class.raw_instantiate();

    assert!(instance.instance_of(&class));
    assert_eq!(sq.stack_depth(), 0);
}
