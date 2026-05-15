use squirrels_sys::tagSQObjectType_OT_GENERATOR;

use crate::{Object, traits::impl_object_traits};

#[derive(Debug, Clone, PartialEq)]
pub struct Generator<'vm>(pub(crate) Object<'vm>);

impl_object_traits!(Generator, tagSQObjectType_OT_GENERATOR, "generator");
