use filesystem_macro::fuse_main;
use fuse_rs::*;
use std::{env, fs, process};

pub struct Passthrough<T>(T);

#[fuse_main]
impl<T> Into<u8> for Passthrough<T> {
    fn into(self) -> u8 {
        todo!()
    }
}

fn main() {
    let a: u8 = Passthrough(1).into();
    // let path = format!("/tmp/fsmnt{}", process::id());
    // fs::create_dir(&path).unwrap();

    // println!("Mouning to {path}...");

    // run_filesystem(
    //     Passthrough,
    //     &[&env::args().next().unwrap(), &path, "-s", "-f"],
    // )
    // .unwrap();
}
