use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rustc-link-lib=fuse");

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

        // The attributes on the fuse_operations macro correspond
        // to the fuse operations that should by default return Ok (0)
        // instead of Not Supported (-38)
        // See https://github.com/libfuse/libfuse/blob/48ae2e72b39b6a31cb2194f6f11786b7ca06aac6/include/fuse.h#L1135
        bindings_raw.insert_str(
            operations_loc,
            "#[filesystem_macro::fuse_operations[open, release, opendir, releasedir, statfs]]\n",
        );

        fs::write(out, bindings_raw).unwrap();
    }
}
