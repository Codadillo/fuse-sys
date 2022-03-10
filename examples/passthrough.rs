use fuse_sys::prelude::*;
use std::{
    env,
    ffi::CString,
    fs::*,
    io::ErrorKind,
    mem,
    os::{raw::c_void, unix::fs::*},
};

macro_rules! libc_err {
    ($val:expr) => {
        match $val {
            e if e.is_negative() => Err(-*libc::__errno_location()),
            o => Ok(o),
        }
    };
}

macro_rules! io_err {
    ($val:expr) => {
        $val.map_err(|e| -e.raw_os_error().unwrap_or(1))
    };
}

struct Passthrough {
    root: String,
}

impl Passthrough {
    fn new(root: String) -> Self {
        Self { root }
    }

    fn source(&self, relative: &str) -> String {
        format!("{}/{relative}", self.root)
    }

    fn source_raw(&self, relative: &str) -> CString {
        CString::new(self.source(relative)).unwrap()
    }
}

impl FileSystem for Passthrough {
    fn chmod(&mut self, path: &str, mode: mode_t) -> Result<i32, i32> {
        io_err!(set_permissions(
            self.source(path),
            Permissions::from_mode(mode)
        ))
        .map(|_| 0)
    }

    fn create(
        &mut self,
        path: &str,
        mode: mode_t,
        info: Option<&mut fuse_file_info>,
    ) -> Result<i32, i32> {
        let path = self.source_raw(path);
        let info = info.unwrap();

        let fd = unsafe { libc_err!(libc::open(path.as_ptr(), info.flags, mode))? };
        info.fh = fd as u64;

        Ok(0)
    }

    fn fsync(
        &mut self,
        _path: &str,
        _datasync: i32,
        info: Option<&mut fuse_file_info>,
    ) -> Result<i32, i32> {
        match info {
            Some(info) => unsafe { libc_err!(libc::fsync(info.fh as i32)) },
            None => Ok(0),
        }
    }

    fn getattr(&mut self, path: &str, stat: Option<&mut stat>) -> Result<i32, i32> {
        let path = self.source_raw(path);
        unsafe {
            libc_err!(libc::stat(
                path.as_ptr(),
                stat.unwrap() as *mut stat as *mut libc::stat
            ))
        }
    }

    fn mkdir(&mut self, path: &str, mode: mode_t) -> Result<i32, i32> {
        let path = self.source(path);
        io_err!(create_dir(&path))?;
        io_err!(set_permissions(path, Permissions::from_mode(mode))).map(|_| 0)
    }

    fn mknod(&mut self, path: &str, mode: mode_t, dev: dev_t) -> Result<i32, i32> {
        let path = self.source_raw(path);
        unsafe { libc_err!(libc::mknod(path.as_ptr(), mode, dev)) }
    }

    fn open(&mut self, path: &str, info: Option<&mut fuse_file_info>) -> Result<i32, i32> {
        let path = self.source_raw(path);
        let info = info.unwrap();

        let fd = unsafe { libc_err!(libc::open(path.as_ptr(), info.flags))? };
        info.fh = fd as u64;

        Ok(0)
    }

    fn read(
        &mut self,
        path: &str,
        buf: &mut [u8],
        off: off_t,
        _info: Option<&mut fuse_file_info>,
    ) -> Result<i32, i32> {
        let f = io_err!(File::open(self.source(path)))?;
        io_err!(f.read_at(buf, off as u64).map(|n| n as i32))
    }

    fn readdir(
        &mut self,
        path: &str,
        buf: Option<&mut c_void>,
        filler: fuse_fill_dir_t,
        _off: off_t,
        _info: Option<&mut fuse_file_info>,
    ) -> Result<i32, i32> {
        let filler = filler.unwrap();
        let buf = buf
            .map(|buf| buf as *mut c_void)
            .unwrap_or(0 as *mut c_void);

        for entry in io_err!(read_dir(self.source(path)))? {
            let entry = io_err!(entry)?;

            let stat = stat {
                st_ino: entry.ino(),
                ..Default::default()
            };

            let name_raw = CString::new(entry.file_name().to_str().unwrap()).unwrap();
            unsafe {
                if filler(buf, name_raw.as_ptr(), &stat as *const stat, 0) != 0 {
                    break;
                }
            }
        }

        Ok(0)
    }

    fn readlink(&mut self, path: &str, buf: &mut [u8]) -> Result<i32, i32> {
        if buf.len() == 0 {
            return Ok(0);
        }

        let link_buf = io_err!(read_link(self.source(path)))?;
        let link = link_buf.to_str().unwrap().as_bytes();

        let length = buf.len().min(link.len());
        (&mut buf[..length]).copy_from_slice(&link[..length]);

        let null = length.min(buf.len() - 1);
        buf[null] = 0;

        Ok(0)
    }

    fn release(&mut self, _path: &str, info: Option<&mut fuse_file_info>) -> Result<i32, i32> {
        match info {
            Some(info) => unsafe { libc_err!(libc::close(info.fh as i32)) },
            None => Ok(0),
        }
    }

    fn rename(&mut self, old: &str, new: &str) -> Result<i32, i32> {
        io_err!(rename(self.source(old), self.source(new))).map(|_| 0)
    }

    fn rmdir(&mut self, path: &str) -> Result<i32, i32> {
        io_err!(remove_dir(self.source(path))).map(|_| 0)
    }

    fn statfs(&mut self, path: &str, stat: Option<&mut statvfs>) -> Result<i32, i32> {
        let path = self.source_raw(path);
        unsafe { libc_err!(libc::statvfs(path.as_ptr(), mem::transmute(stat.unwrap()))) }
    }

    fn truncate(&mut self, path: &str, size: off_t) -> Result<i32, i32> {
        let f = io_err!(File::open(self.source(path)))?;
        io_err!(f.set_len(size as u64)).map(|_| 0)
    }

    fn unlink(&mut self, path: &str) -> Result<i32, i32> {
        io_err!(remove_file(self.source(path))).map(|_| 0)
    }

    fn utimens(&mut self, _path: &str, _tv: Option<&timespec>) -> Result<i32, i32> {
        Ok(0)
    }

    fn write(
        &mut self,
        path: &str,
        buf: &[u8],
        off: off_t,
        _info: Option<&mut fuse_file_info>,
    ) -> Result<i32, i32> {
        let f = io_err!(OpenOptions::new().write(true).open(self.source(path)))?;
        io_err!(f.write_at(buf, off as u64)).map(|n| n as i32)
    }
}

fn main() {
    let mount = "/tmp/fsmnt";
    let data = "/tmp/fsdata";

    match read_dir(&mount) {
        Err(e) if e.kind() == ErrorKind::NotFound => create_dir(&mount).unwrap(),
        r => {
            r.unwrap();
        }
    }
    match read_dir(&data) {
        Err(e) if e.kind() == ErrorKind::NotFound => create_dir(&data).unwrap(),
        r => {
            r.unwrap();
        }
    }

    println!("Mounting {mount} as mirror of {data}...");

    let fs = Passthrough::new(data.to_owned());

    let bin = &env::args().next().unwrap();
    fs.run(&[bin, &mount, "-f", "-s"]).unwrap();
}
