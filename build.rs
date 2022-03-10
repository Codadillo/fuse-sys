use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rustc-link-lib=fuse3");

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .derive_default(true)
        .generate()
        .expect("Could not generate bindings");

    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("bindings.rs");
    bindings
        .write_to_file(&out)
        .expect("Couldn't write bindings!");

    #[cfg(feature = "auto")]
    {
        use std::fs;

        let mut bindings_raw = fs::read_to_string(&out).unwrap();
        let operations_loc = bindings_raw
            .find("pub struct fuse_operations")
            .expect("Could not find struct fuse_operations");
        bindings_raw.insert_str(operations_loc, "#[filesystem_macro::fuse_operations]\n");
        fs::write(out, bindings_raw).unwrap();
    }
}
