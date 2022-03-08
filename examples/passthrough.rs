use fuse_rs::*;
use std::{env, process, fs};

pub struct Passthrough;

impl FileSystem for Passthrough {
}

fn main() {
    let path = format!("/tmp/fsmnt{}", process::id());
    fs::create_dir(&path).unwrap();
    
    println!("Mouning to {path}...");

    run_filesystem(
        Passthrough,
        &[&env::args().next().unwrap(), &path, "-s", "-f"],
    )
    .unwrap();
}
