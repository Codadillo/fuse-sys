use fuse_sys::prelude::*;
use std::{env, fs::*, os::unix::fs::*, process};

pub struct Passthrough;

impl FileSystem for Passthrough {
    fn chmod(&mut self, path: &str, mode: mode_t) -> Result<(), i32> {
        let perm = Permissions::from_mode(mode);
        set_permissions(path, perm).map_err(|e| e.raw_os_error().unwrap_or(1))
    }

//     fn create(
//         &mut self,
//         path: &str,
//         mode: mode_t,
//         info: Option<&mut fuse_file_info>
//     ) -> Result<(), i32> {
//         info.unwrap().fh = 
//     }
}

fn main() {
    let path = format!("/tmp/fsmnt{}", process::id());
    create_dir(&path).unwrap();

    println!("Mouning to {path}...");

    let fs = Passthrough;
    fs.run(&[&env::args().next().unwrap(), &path, "-f", "-s"])
        .unwrap();
}
