use std::{
    ffi::{c_void, CStr, CString},
    os::raw::{c_char, c_int},
    ptr,
    time::Duration,
};

use fuse_sys::*;
use std::mem;

pub mod __private {
    pub use fuse_sys::fuse_operations;
}

pub trait FuseMain {
    fn run(fuse_args: &[&str]) -> Result<(), i32>;
}
