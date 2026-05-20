use squirrels_sys::{
    SQBool, sq_get, sq_getclass, sq_instanceof, sq_rawget, sq_rawset, sq_set,
    tagSQObjectType_OT_INSTANCE,
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
    pub fn instance_of(&self, class: &Class<'vm>) -> bool {
        self.0.sq.assert_same_vm(class.0.sq);

        class.0.push_into_stack();
        self.0.push_into_stack();

        let b = unsafe { sq_instanceof(self.0.sq.vm) };
        assert!(
            b != (0 as SQBool).wrapping_sub(1),
            "sq_instanceof failed on {self:?} (class: {class:?}"
        );
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

#[cfg(test)]
mod tests {
    use crate::{CallError, Class, Error, Integer, Squirrel};

    fn class_with_field<'vm>(sq: &'vm Squirrel, key: &str, value: Integer) -> Class<'vm> {
        let class = Class::new(sq);
        class.new_slot(key, value, false).unwrap();
        class
    }

    #[test]
    fn instance_class() {
        let sq = Squirrel::new(1024);
        let class = Class::new(&sq);
        let instance = class.raw_instantiate();

        assert_eq!(instance.class(), class);
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn instance_instance_of() {
        let sq = Squirrel::new(1024);
        let class = Class::new(&sq);
        let instance = class.raw_instantiate();

        assert!(instance.instance_of(&class));
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn instance_instance_of_other_class() {
        let sq = Squirrel::new(1024);
        let class_a = Class::new(&sq);
        let class_b = Class::new(&sq);
        let instance = class_a.raw_instantiate();

        assert!(!instance.instance_of(&class_b));
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn instance_get() {
        let sq = Squirrel::new(1024);
        let class = class_with_field(&sq, "x", 7);
        let instance = class.raw_instantiate();

        let v: Integer = instance.get("x").unwrap();
        assert_eq!(v, 7);
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn instance_get_missing_key() {
        let sq = Squirrel::new(1024);
        let class = Class::new(&sq);
        let instance = class.raw_instantiate();

        let err = instance.get::<_, Integer>("nope").unwrap_err();
        assert!(matches!(err, CallError::Runtime(_)));
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn instance_set() {
        let sq = Squirrel::new(1024);
        let class = class_with_field(&sq, "x", 1);
        let instance = class.raw_instantiate();

        instance.set("x", 42).unwrap();
        let v: Integer = instance.get("x").unwrap();
        assert_eq!(v, 42);
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn instance_set_missing_key() {
        let sq = Squirrel::new(1024);
        let class = Class::new(&sq);
        let instance = class.raw_instantiate();

        let err = instance.set("nope", 1).unwrap_err();
        assert!(matches!(err, CallError::Runtime(_)));
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn instance_raw_get() {
        let sq = Squirrel::new(1024);
        let class = class_with_field(&sq, "x", 7);
        let instance = class.raw_instantiate();

        let v: Integer = instance.raw_get("x").unwrap();
        assert_eq!(v, 7);
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn instance_raw_get_missing_key() {
        let sq = Squirrel::new(1024);
        let class = Class::new(&sq);
        let instance = class.raw_instantiate();

        let err = instance.raw_get::<_, Integer>("nope").unwrap_err();
        assert!(matches!(err, CallError::Runtime(_)));
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn instance_raw_set() {
        let sq = Squirrel::new(1024);
        let class = class_with_field(&sq, "x", 1);
        let instance = class.raw_instantiate();

        instance.raw_set("x", 99).unwrap();
        let v: Integer = instance.raw_get("x").unwrap();
        assert_eq!(v, 99);
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn instance_call_method() {
        let sq = Squirrel::new(1024);
        let class: Class = sq
            .eval("return class { function double(n) { return n * 2 } }")
            .unwrap();
        let instance = class.raw_instantiate();

        let v: Integer = instance.call_method("double", (21,)).unwrap();
        assert_eq!(v, 42);
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn instance_call_method_not_a_closure() {
        let sq = Squirrel::new(1024);
        let class = class_with_field(&sq, "x", 1);
        let instance = class.raw_instantiate();

        let err = instance.call_method::<_, _, ()>("x", ()).unwrap_err();
        assert!(matches!(
            err,
            CallError::Other(Error::Type {
                expected: "closure"
            })
        ));
        assert_eq!(sq.stack_depth(), 0);
    }
}
