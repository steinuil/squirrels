use squirrels_sys::{sq_getclass, sq_instanceof, tagSQObjectType_OT_INSTANCE};

use crate::{Class, FromSquirrel, Object, errors::SqResultExt as _, traits::impl_object_traits};

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
}
