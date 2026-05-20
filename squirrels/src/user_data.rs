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
        if unsafe { sq_getuserdata(sq.vm, idx, &mut buf, &mut tag) }.is_error() {
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

#[cfg(test)]
mod tests {
    use super::UserData;
    use crate::{Error, Squirrel};

    #[test]
    fn user_data_new_and_borrow() {
        let sq = Squirrel::new(1024);
        let ud = UserData::new(&sq, 1_i32);
        assert_eq!(*ud.borrow::<i32>().unwrap(), 1);
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn user_data_borrow_mut() {
        let sq = Squirrel::new(1024);
        let ud = UserData::new(&sq, 1_i32);
        *ud.borrow_mut::<i32>().unwrap() = 99;
        assert_eq!(*ud.borrow::<i32>().unwrap(), 99);
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn user_data_borrow_wrong_type() {
        let sq = Squirrel::new(1024);
        let ud = UserData::new(&sq, 1_i32);
        let err = ud.borrow::<u64>().unwrap_err();
        assert!(matches!(err, Error::Type { .. }));
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn user_data_borrow_mut_wrong_type() {
        let sq = Squirrel::new(1024);
        let ud = UserData::new(&sq, 1_i32);
        let err = ud.borrow_mut::<u64>().unwrap_err();
        assert!(matches!(err, Error::Type { .. }));
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn user_data_borrow_mut_while_borrowed() {
        let sq = Squirrel::new(1024);
        let ud = UserData::new(&sq, 1_i32);
        let _b = ud.borrow::<i32>().unwrap();
        let err = ud.borrow_mut::<i32>().unwrap_err();
        assert!(matches!(err, Error::Type { .. }));
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn user_data_borrow_while_borrow_mut() {
        let sq = Squirrel::new(1024);
        let ud = UserData::new(&sq, 1_i32);
        let _b = ud.borrow_mut::<i32>().unwrap();
        let err = ud.borrow::<i32>().unwrap_err();
        assert!(matches!(err, Error::Type { .. }));
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn user_data_clone_shares_payload() {
        let sq = Squirrel::new(1024);
        let ud = UserData::new(&sq, 7_i32);
        let ud2 = ud.clone();
        *ud.borrow_mut::<i32>().unwrap() = 123;
        assert_eq!(*ud2.borrow::<i32>().unwrap(), 123);
        assert_eq!(sq.stack_depth(), 0);
    }

    #[test]
    fn user_data_release_hook_drops_payload() {
        use std::sync::Arc;

        let sq = Squirrel::new(1024);
        let payload = Arc::new(());
        {
            let _ud = UserData::new(&sq, payload.clone());
            assert_eq!(Arc::strong_count(&payload), 2);
        }
        // Dropping the VM runs the release hook, which drops the boxed payload.
        drop(sq);
        assert_eq!(Arc::strong_count(&payload), 1);
    }
}
