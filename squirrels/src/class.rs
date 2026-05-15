use squirrels_sys::tagSQObjectType_OT_CLASS;

use crate::{Object, traits::impl_object_traits};

#[derive(Debug, Clone, PartialEq)]
pub struct Class<'vm>(pub(crate) Object<'vm>);

impl_object_traits!(Class, tagSQObjectType_OT_CLASS, "class");
