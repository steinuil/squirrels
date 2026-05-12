use std::{env, path::PathBuf};

fn main() {
    let dst = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    cc::Build::new()
        .cpp(true)
        .include("squirrel-3.2/include")
        .include("squirrel-3.2/squirrel")
        .flag("-fno-rtti")
        .flag("-fno-exceptions")
        .flag("-fno-strict-aliasing")
        .flag("-Wcast-qual")
        .flag("-O3")
        .warnings(false)
        .files(&[
            "squirrel-3.2/squirrel/sqapi.cpp",
            "squirrel-3.2/squirrel/sqbaselib.cpp",
            "squirrel-3.2/squirrel/sqclass.cpp",
            "squirrel-3.2/squirrel/sqcompiler.cpp",
            "squirrel-3.2/squirrel/sqdebug.cpp",
            "squirrel-3.2/squirrel/sqfuncstate.cpp",
            "squirrel-3.2/squirrel/sqlexer.cpp",
            "squirrel-3.2/squirrel/sqmem.cpp",
            "squirrel-3.2/squirrel/sqobject.cpp",
            "squirrel-3.2/squirrel/sqstate.cpp",
            "squirrel-3.2/squirrel/sqtable.cpp",
            "squirrel-3.2/squirrel/sqvm.cpp",
        ])
        .out_dir(dst.join("lib"))
        .compile("squirrel");

    cc::Build::new()
        .include("squirrel-3.2/include")
        .file("squirrel_print_shim.c")
        .out_dir(dst.join("lib"))
        .compile("squirrel_shim");

    bindgen::Builder::default()
        .header("squirrel-3.2/include/squirrel.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(dst.join("bindings.rs"))
        .expect("Unable to write bindings");
}
