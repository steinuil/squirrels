use squirrels_sys::tagSQObjectType_OT_INSTANCE;

use crate::{Object, traits::impl_object_traits};

#[derive(Debug, Clone, PartialEq)]
pub struct Instance<'vm>(pub(crate) Object<'vm>);

impl_object_traits!(Instance, tagSQObjectType_OT_INSTANCE, "instance");
