#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[test]
fn squirrel_test() {
    use std::ffi::c_char;

    unsafe {
        let vm = sq_open(1024);

        sq_setprintfunc(vm, None, None);

        sq_pushroottable(vm);

        let script = "return 1 + 2";

        sq_compilebuffer(
            vm,
            script.as_ptr() as *const c_char,
            script.len() as SQInteger,
            c"embedded".as_ptr(),
            1,
        );

        sq_push(vm, -2);

        sq_call(vm, 1, SQTrue as _, SQTrue as _);

        let mut out: SQInteger = 0;
        sq_getinteger(vm, -1, &mut out);
        assert_eq!(out, 3);

        sq_pop(vm, 3);

        sq_close(vm);
    }
}
