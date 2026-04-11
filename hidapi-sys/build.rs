use std::env;
use std::path::PathBuf;

fn main() {
    let bindings = bindgen::builder()
        .header("../external/hidapi/hidapi/hidapi.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    cc::Build::new()
        .file("../external/hidapi/linux/hid.c")
        .include("../external/hidapi/hidapi")
        .compile("libhidapi.a");

    pkg_config::probe_library("libudev").unwrap();
}
