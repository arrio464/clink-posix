//! The only Windows FFI in the prototype: borrow then duplicate standard handles.
use std::{
    ffi::c_void,
    fs::File,
    io,
    os::windows::io::{BorrowedHandle, OwnedHandle},
};
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetStdHandle(n: u32) -> *mut c_void;
}
pub fn standard(n: u32) -> io::Result<File> {
    // GetStdHandle returns process-owned handles. Never close the borrowed value;
    // try_clone_to_owned duplicates it and File owns only that duplicate.
    let raw = unsafe { GetStdHandle(n) };
    if raw.is_null() || raw as isize == -1 {
        return Err(io::Error::last_os_error());
    }
    let borrowed = unsafe { BorrowedHandle::borrow_raw(raw) };
    Ok(borrowed.try_clone_to_owned()?.into())
}
pub fn pipe() -> io::Result<(File, File)> {
    let (r, w) = io::pipe()?;
    Ok((OwnedHandle::from(r).into(), OwnedHandle::from(w).into()))
}
pub struct Streams {
    pub input: File,
    pub output: File,
    pub error: File,
}
impl Streams {
    pub fn standard() -> io::Result<Self> {
        Ok(Self {
            input: standard(-10_i32 as u32)?,
            output: standard(-11_i32 as u32)?,
            error: standard(-12_i32 as u32)?,
        })
    }
    pub fn duplicate(&self) -> io::Result<Self> {
        Ok(Self {
            input: self.input.try_clone()?,
            output: self.output.try_clone()?,
            error: self.error.try_clone()?,
        })
    }
}
