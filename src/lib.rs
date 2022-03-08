use std::{
    ffi::{CStr, CString, c_void},
    os::raw::{c_char, c_int},
    time::Duration, ptr,
};

use fuse_sys::*;
use std::mem;

pub use fuse_sys::{dev_t, fuse_file_info, mode_t, off_t, size_t, stat, statvfs};

pub trait FileSystem {
    fn chmod(&mut self, path: &str, mode: mode_t) -> Result<(), i32>;
    fn create(&mut self, path: &str, mode: mode_t, info: &mut fuse_file_info) -> Result<(), i32>;
    fn fsync(&mut self, path: &str, datasync: i32, info: &mut fuse_file_info) -> Result<(), i32>;
    fn getattr(&mut self, path: &str, stat: &stat) -> Result<(), i32>;
    fn mkdir(&mut self, path: &str, mode: mode_t) -> Result<(), i32>;
    fn mknod(&mut self, path: &str, mode: mode_t, dev: dev_t) -> Result<(), i32>;
    fn open(&mut self, path: &str, info: &mut fuse_file_info) -> Result<(), i32>;
    fn read(
        &mut self,
        path: &str,
        buf: &mut String,
        count: size_t,
        offset: off_t,
        info: &mut fuse_file_info,
    ) -> Result<(), i32>;
    fn read_dir(
        &mut self,
        path: &str,
        buf: &mut String,
        filler: fn(buf: &mut String, name: &str, stat: &stat, off: off_t),
        offset: off_t,
        info: &mut fuse_file_info,
    ) -> Result<(), i32>;
    fn readlink(&mut self, path: &str, buf: &mut String, size: size_t) -> Result<(), i32>;
    fn release(&mut self, path: &str, info: &mut fuse_file_info) -> Result<(), i32>;
    fn rename(&mut self, old_path: &str, old_path: &str) -> Result<(), i32>;
    fn rmdir(&mut self, path: &str) -> Result<(), i32>;
    fn statfs(&mut self, path: &str, buf: &mut statvfs) -> Result<(), i32>;
    fn truncate(&mut self, path: &str, size: off_t) -> Result<(), i32>;
    fn unlink(&mut self, path: &str) -> Result<(), i32>;
    fn utimens(&mut self, path: &str, time: Duration) -> Result<(), i32>;
    fn write(
        &mut self,
        path: &str,
        buf: &str,
        offset: off_t,
        info: &mut fuse_file_info,
    ) -> Result<(), i32>;
}

pub trait RawFileSystem {
    unsafe extern "C" fn chmod(path: *const c_char, mode: mode_t) -> c_int;
    unsafe extern "C" fn create(path: *const c_char, mode: mode_t, info: *mut fuse_file_info) -> c_int;
    unsafe extern "C" fn fsync(path: *const c_char, datasync: c_int, info: *mut fuse_file_info) -> c_int;
    unsafe extern "C" fn getattr(path: *const c_char, stat: *mut stat) -> c_int;
    unsafe extern "C" fn mkdir(path: *const c_char, mode: mode_t) -> c_int;
    unsafe extern "C" fn open(path: *const c_char, info: *mut fuse_file_info) -> c_int;
    unsafe extern "C" fn read(path: *const c_char, buf: *mut c_char, count: size_t, offset: off_t, info: *mut fuse_file_info) -> c_int;
    unsafe extern "C" fn readdir(path: *const c_char, buf: *mut c_void, filler: fuse_fill_dir_t, offset: off_t, info: *mut fuse_file_info) -> c_int;
    unsafe extern "C" fn readlink(path: *const c_char, buf: *mut c_char, count: size_t) -> c_int;
    unsafe extern "C" fn release(path: *const c_char, info: *mut fuse_file_info) -> c_int;
    unsafe extern "C" fn rename(old_path: *const c_char, new_path: *const c_char) -> c_int;
    unsafe extern "C" fn rmdir(path: *const c_char) -> c_int;
    unsafe extern "C" fn statfs(path: *const c_char, stat: *mut statvfs) -> c_int;
    unsafe extern "C" fn truncate(path: *const c_char, size: off_t) -> c_int;
    unsafe extern "C" fn unlink(path: *const c_char) -> c_int;
    unsafe extern "C" fn utimens(path: *const c_char, tv: *const timespec) -> c_int;
    unsafe extern "C" fn write(path: *const c_char, buf: *const c_char, size: size_t, offset: off_t, info: *mut fuse_file_info) -> c_int;
}

macro_rules! call_fs_safe {
    ($fn:ident, $( $args:expr, )*) => {
        {
            let this: *mut Self = mem::transmute((*fuse_get_context()).private_data);
            let out = this.as_mut().unwrap().$fn( $( $args, )*);
            match out {
                Ok(()) => 0,
                Err(e) => e,
            }
        }
    };
}

macro_rules! cstr {
    ($raw:ident) => {
        CStr::from_ptr($raw).to_str().unwrap()
    }
}

impl<F: FileSystem> RawFileSystem for F {
    unsafe extern "C" fn chmod(path: *const c_char, mode: mode_t) -> c_int {
        call_fs_safe!(chmod, cstr!(path), mode, )
    }

    unsafe extern "C" fn create(path: *const c_char, mode: mode_t, info: *mut fuse_file_info) -> c_int {
        call_fs_safe!(create, cstr!(path), mode, info.as_mut().unwrap(), )
    }

    unsafe extern "C" fn fsync(path: *const c_char, datasync: c_int, info: *mut fuse_file_info) -> c_int {
        call_fs_safe!(fsync, cstr!(path), datasync, info.as_mut().unwrap(), )
    }

    unsafe extern "C" fn getattr(path: *const c_char, stat: *mut stat) -> c_int {
        call_fs_safe!(getattr, cstr!(path), stat.as_ref().unwrap(), )
    }

    unsafe extern "C" fn mkdir(path: *const c_char, mode: mode_t) -> c_int {
        call_fs_safe!(mkdir, cstr!(path), mode, )
    }

    unsafe extern "C" fn open(path: *const c_char, info: *mut fuse_file_info) -> c_int {
        call_fs_safe!(open, cstr!(path), info.as_mut().unwrap(), )
    }

    unsafe extern "C" fn read(path: *const c_char, buf: *mut c_char, count: size_t, offset: off_t, info: *mut fuse_file_info) -> c_int {
        let mut r_buf = String::with_capacity(count as usize);
        let out = call_fs_safe!(read, cstr!(path), &mut r_buf, count, offset, info.as_mut().unwrap(), );
        ptr::copy_nonoverlapping(r_buf.as_ptr(), buf as *mut u8, count as usize);
        out
    }

    unsafe extern "C" fn readdir(path: *const c_char, buf: *mut c_void, filler: fuse_fill_dir_t, offset: off_t, info: *mut fuse_file_info) -> c_int {
        todo!()
    }

    unsafe extern "C" fn readlink(path: *const c_char, buf: *mut c_char, count: size_t) -> c_int {
        todo!()
    }

    unsafe extern "C" fn release(path: *const c_char, info: *mut fuse_file_info) -> c_int {
        todo!()
    }

    unsafe extern "C" fn rename(old_path: *const c_char, new_path: *const c_char) -> c_int {
        todo!()
    }

    unsafe extern "C" fn rmdir(path: *const c_char) -> c_int {
        todo!()
    }

    unsafe extern "C" fn statfs(path: *const c_char, stat: *mut statvfs) -> c_int {
        todo!()
    }

    unsafe extern "C" fn truncate(path: *const c_char, size: off_t) -> c_int {
        todo!()
    }

    unsafe extern "C" fn unlink(path: *const c_char) -> c_int {
        todo!()
    }

    unsafe extern "C" fn utimens(path: *const c_char, tv: *const timespec) -> c_int {
        todo!()
    }

    unsafe extern "C" fn write(path: *const c_char, buf: *const c_char, size: size_t, offset: off_t, info: *mut fuse_file_info) -> c_int {
        todo!()
    }

}

pub fn run_filesystem<F: RawFileSystem + 'static>(
    filesystem: F,
    fuse_args: &[&str],
) -> Result<(), i32> {
    static mut ops: Option<fuse_operations> = None;
    unsafe {
        ops = Some(fuse_operations {
            getattr: Some(F::getattr),
            readlink: Some(F::readlink),
            getdir: None,
            mkdir: Some(F::mkdir),
            mknod: None,
            unlink: Some(F::unlink),
            rmdir: Some(F::rmdir),
            symlink: None,
            rename: Some(F::rename),
            link: None,
            chmod: Some(F::chmod),
            chown: None,
            truncate: Some(F::truncate),
            utime: None,
            open: Some(F::open),
            read: Some(F::read),
            write: Some(F::write),
            statfs: Some(F::statfs),
            flush: None,
            release: Some(F::release),
            fsync: Some(F::fsync),
            setxattr: None,
            getxattr: None,
            listxattr: None,
            removexattr: None,
            opendir: None,
            readdir: Some(F::readdir),
            releasedir: None,
            fsyncdir: None,
            init: None,
            destroy: None,
            access: None,
            create: Some(F::create),
            ftruncate: None,
            fgetattr: None,
            lock: None,
            utimens: Some(F::utimens),
            bmap: None,
            _bitfield_align_1: [0; 0],
            _bitfield_1: __BindgenBitfieldUnit::new([0; 4]),
            ioctl: None,
            poll: None,
            write_buf: None,
            read_buf: None,
            flock: None,
            fallocate: None,
        });
    }

    let args: Vec<CString> = fuse_args
        .iter()
        .map(|&s| CString::new(s).unwrap())
        .collect();
    let mut args_raw: Vec<*mut c_char> = args.iter().map(|s| s.as_ptr() as *mut c_char).collect();

    let mut this = Box::new(filesystem);

    let status = unsafe {
        fuse_main_real(
            args.len() as i32,
            args_raw.as_mut_ptr(),
            ops.as_ref().unwrap() as *const fuse_operations,
            mem::size_of::<fuse_operations>() as u64,
            mem::transmute(this.as_mut()),
        )
    };

    match status {
        0 => Ok(()),
        e => Err(e),
    }
}
