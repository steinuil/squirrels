use squirrels_sys::tagSQObjectType_OT_USERDATA;

use crate::{Object, traits::impl_object_traits};

#[derive(Debug, Clone, PartialEq)]
pub struct UserData<'vm>(pub(crate) Object<'vm>);

impl_object_traits!(UserData, tagSQObjectType_OT_USERDATA, "userdata");
