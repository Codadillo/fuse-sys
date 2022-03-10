use fuse_sys::prelude::*;
use std::{env, fs, process};

pub struct Passthrough;

impl FileSystem for Passthrough {
    fn getattr(&mut self, arg1: &str, _arg2: Option<&mut stat>) -> Result<(), i32> {
        println!("GET ATTR {arg1}");
        Ok(())
    }
}

fn main() {
    let path = format!("/tmp/fsmnt{}", process::id());
    fs::create_dir(&path).unwrap();

    println!("Mouning to {path}...");

    let fs = Passthrough;

    fs.run(&[&env::args().next().unwrap(), &path, "-f", "-s"])
        .unwrap();
}
