use std::{
    any::TypeId,
    cell::{Ref, RefCell, RefMut},
    ffi::c_void,
    panic::{AssertUnwindSafe, catch_unwind},
};

use squirrels_sys::{
    SQInteger, SQUserPointer, sq_getuserdata, sq_newuserdata, sq_setreleasehook, sq_settypetag,
    tagSQObjectType_OT_USERDATA,
};

use crate::{
    Error, FromSquirrel as _, Integer, Object, Result, Squirrel, errors::SqResultExt as _,
    traits::impl_object_traits,
};

/// A ref-counted handle to a Squirrel userdata object.
#[derive(Debug, Clone, PartialEq)]
pub struct UserData<'vm>(pub(crate) Object<'vm>);

impl_object_traits!(UserData, tagSQObjectType_OT_USERDATA, "userdata");

pub(crate) struct Payload<T: 'static> {
    type_id: TypeId,
    cell: RefCell<T>,
}

impl<T: 'static> Payload<T> {
    pub(crate) fn cell(&self) -> &RefCell<T> {
        &self.cell
    }
}

fn userdata_tag() -> SQUserPointer {
    static TAG: u8 = 0;
    &TAG as *const u8 as *mut c_void
}

extern "C" fn release_hook<T: 'static>(slot: SQUserPointer, _size: SQInteger) -> SQInteger {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let ptr: *mut Payload<T> = unsafe { *(slot as *mut *mut Payload<T>) };
        if !ptr.is_null() {
            drop(unsafe { Box::from_raw(ptr) });
        }
    }));
    1
}

impl<'vm> UserData<'vm> {
    pub fn new<T: Send + 'static>(sq: &'vm Squirrel, value: T) -> Self {
        // Box the payload so the layout of Payload is abstracted away from Squirrel.
        let boxed: *mut Payload<T> = Box::into_raw(Box::new(Payload {
            type_id: TypeId::of::<T>(),
            cell: RefCell::new(value),
        }));

        // Allocate the slot as a Squirrel userdata object.
        let slot = unsafe { sq_newuserdata(sq.vm, size_of::<*mut Payload<T>>() as _) };
        unsafe { *(slot as *mut *mut Payload<T>) = boxed };

        // Set a stable type tag on the userdata so we know it's been allocated by us.
        unsafe { sq_settypetag(sq.vm, -1, userdata_tag()) }
            .expect(format_args!("sq_settypetag failed on fresh userdata"));
        unsafe { sq_setreleasehook(sq.vm, -1, Some(release_hook::<T>)) };

        let user_data =
            unsafe { Self::from_stack(-1, sq) }.expect("expecting the userdata we just pushed");
        sq.pop(1);

        user_data
    }

    pub(crate) unsafe fn payload_from_stack<T: 'static>(
        sq: &Squirrel,
        idx: Integer,
    ) -> Option<&Payload<T>> {
        let mut buf: SQUserPointer = std::ptr::null_mut();
        let mut tag: SQUserPointer = std::ptr::null_mut();
        if unsafe { sq_getuserdata(sq.vm, -1, &mut buf, &mut tag) }.is_error() {
            return None;
        }

        if tag != userdata_tag() {
            return None;
        }

        let payload = unsafe { &*(*(buf as *const *const Payload<T>)) };
        if payload.type_id != TypeId::of::<T>() {
            return None;
        }
        Some(payload)
    }

    fn payload<T: 'static>(&self) -> Result<&Payload<T>> {
        self.0.push_into_stack();

        let mut buf: SQUserPointer = std::ptr::null_mut();
        let mut tag: SQUserPointer = std::ptr::null_mut();
        unsafe { sq_getuserdata(self.0.sq.vm, -1, &mut buf, &mut tag) }
            .expect(format_args!("sq_getuserdata failed on {:?}", self));
        self.0.sq.pop(1);

        if tag != userdata_tag() {
            return Err(Error::Type {
                expected: "a userdata object allocated by us",
            });
        }

        let payload = unsafe { &*(*(buf as *const *const Payload<T>)) };
        if payload.type_id != TypeId::of::<T>() {
            return Err(Error::Type {
                expected: "userdata of matching type",
            });
        }
        Ok(payload)
    }

    pub fn borrow<T: 'static>(&self) -> Result<Ref<'_, T>> {
        let payload = self.payload::<T>()?;
        // TODO these should probably be their own Error case
        let t = payload.cell.try_borrow().map_err(|_| Error::Type {
            expected: "userdata not currently mutably borrowed",
        })?;
        Ok(t)
    }

    pub fn borrow_mut<T: 'static>(&self) -> Result<RefMut<'_, T>> {
        let payload = self.payload::<T>()?;
        let t = payload.cell.try_borrow_mut().map_err(|_| Error::Type {
            expected: "userdata not currently immutably borrowed",
        })?;
        Ok(t)
    }
}
