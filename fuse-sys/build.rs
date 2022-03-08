use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rustc-link-lib=fuse3");

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .generate()
        .expect("Could not generate bindings");

    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("bindings.rs");
    bindings
        .write_to_file(out)
        .expect("Couldn't write bindings!");
}
