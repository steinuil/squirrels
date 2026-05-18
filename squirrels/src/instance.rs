use squirrels_sys::{
    sq_get, sq_getclass, sq_instanceof, sq_rawget, sq_rawset, sq_set, tagSQObjectType_OT_INSTANCE,
};

use crate::{
    CallError, CallResult, Class, Error, FromSquirrel, IntoArgs, IntoSquirrel, Object, Value,
    errors::SqResultExt as _, traits::impl_object_traits,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Instance<'vm>(pub(crate) Object<'vm>);

impl_object_traits!(Instance, tagSQObjectType_OT_INSTANCE, "instance");

impl<'vm> Instance<'vm> {
    /// Returns the class this instance was instantiated from.
    pub fn class(&self) -> Class<'vm> {
        self.0.push_into_stack();

        unsafe { sq_getclass(self.0.sq.vm, -1) }
            .expect(format_args!("sq_getclass failed on {:?}", self));

        let class = unsafe { Class::from_stack(-1, self.0.sq) }
            .expect("expecting the class we just pushed");

        self.0.sq.pop(2);

        class
    }

    /// Returns `true` if this instance is an instance of `class`.
    pub fn instance_of(&self, class: Class<'vm>) -> bool {
        self.0.sq.assert_same_vm(class.0.sq);

        self.0.push_into_stack();
        class.0.push_into_stack();

        let b = unsafe { sq_instanceof(self.0.sq.vm) };
        self.0.sq.pop(2);
        b != 0
    }

    /// Call the method associated with `key` with `args`.
    pub fn call_method<K, A, T>(&self, key: K, args: A) -> CallResult<'vm, T>
    where
        K: IntoSquirrel<'vm>,
        A: IntoArgs<'vm>,
        T: FromSquirrel<'vm>,
    {
        let method: Value<'vm> = self.get(key)?;
        match method {
            Value::Closure(c) => c.call_with(self.clone(), args),
            Value::NativeClosure(c) => c.call_with(self.clone(), args),
            _ => Err(CallError::Other(Error::Type {
                expected: "closure",
            })),
        }
    }

    /// Gets the value associated to `key` on this instance.
    ///
    /// Lookup falls back to the instance's class and its base classes.
    ///
    /// Fails if `key` is not found or a `_get` metamethod throws an error.
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

    /// Sets the value associated to an already existing `key` on this instance.
    ///
    /// Fails if `key` is `null`, not found, or a `_set` metamethod throws
    /// an error.
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

    /// Gets the value associated to `key` on this instance, bypassing metamethods.
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

    /// Sets the value associated to an already existing `key` on this instance,
    /// bypassing metamethods.
    ///
    /// Fails if `key` is `null` or not found.
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

    // TODO:
    // * sq_setinstanceup
    // * sq_getinstanceup
    // * sq_getbyhandle
    // * sq_setbyhandle
    // * sq_setreleasehook
    // * sq_getreleasehook
}
