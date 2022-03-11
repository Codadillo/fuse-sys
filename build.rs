use std::{env, path::PathBuf};

fn main() {
    // When building with fuse3, we get an outdated version warning message
    // and (*fuse_get_conext()).private_data gets mangled
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
        // to the fuse operations that are blacklisted
        // for versioning issues. In theory these operations
        // shouldn't show up on the struct at all, but whatever
        // I'm not mad or anything like that's totally fine I'm fine.
        bindings_raw.insert_str(
            operations_loc,
            "#[filesystem_macro::fuse_operations[getdir, utime]]\n",
        );

        fs::write(out, bindings_raw).unwrap();
    }
}
