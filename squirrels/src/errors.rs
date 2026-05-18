use std::fmt;

use squirrels_sys::SQRESULT;

use crate::{CallError, CallResult, Squirrel};

pub trait SqResultExt {
    fn expect(self, msg: fmt::Arguments<'_>);

    fn to_runtime_error<'vm>(self, sq: &'vm Squirrel, args_to_pop: usize) -> CallResult<'vm, ()>;
}

impl SqResultExt for SQRESULT {
    fn expect(self, msg: fmt::Arguments<'_>) {
        if self.is_error() {
            panic!("{}", msg);
        }
    }

    fn to_runtime_error<'vm>(self, sq: &'vm Squirrel, args_to_pop: usize) -> CallResult<'vm, ()> {
        if self.is_error() {
            if args_to_pop > 0 {
                sq.pop(args_to_pop as _);
            }
            Err(CallError::get_runtime_error(sq))
        } else {
            Ok(())
        }
    }
}
