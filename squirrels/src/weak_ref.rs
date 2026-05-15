use squirrels_sys::tagSQObjectType_OT_WEAKREF;

use crate::{Object, traits::impl_object_traits};

#[derive(Debug, Clone, PartialEq)]
pub struct WeakRef<'vm>(pub(crate) Object<'vm>);

impl_object_traits!(WeakRef, tagSQObjectType_OT_WEAKREF, "weakref");
