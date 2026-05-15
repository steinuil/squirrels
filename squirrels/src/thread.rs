use squirrels_sys::tagSQObjectType_OT_THREAD;

use crate::{Object, traits::impl_object_traits};

#[derive(Debug, Clone, PartialEq)]
pub struct Thread<'vm>(pub(crate) Object<'vm>);

impl_object_traits!(Thread, tagSQObjectType_OT_THREAD, "thread");
