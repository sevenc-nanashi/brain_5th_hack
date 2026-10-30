//! V690_H1 filesystem services. See docs/filesystem.md for the ABI evidence.
use core::{
    ffi::{CStr, c_char, c_void},
    ptr::NonNull,
};

/// Kernel directory entry, borrowed until the next read or close.
#[repr(C)]
pub struct Entry {
    pub kind: u32,
    pub size: u32,
    pub name_length: u32,
    pub name: [u8; 260],
}

pub struct Directory(NonNull<c_void>);
impl Directory {
    /// # Safety
    /// Requires V690_H1's filesystem runtime and serialized calls on the PEG thread.
    pub unsafe fn open(path: &CStr) -> Option<Self> {
        let call: unsafe extern "C" fn(*const c_char) -> *mut c_void =
            unsafe { core::mem::transmute(0x6000_6454usize) };
        NonNull::new(unsafe { call(path.as_ptr()) }).map(Self)
    }

    /// Null means end of enumeration or a runtime error; the ABI does not distinguish them.
    pub fn read(&mut self) -> Option<&Entry> {
        let call: unsafe extern "C" fn(*mut c_void) -> *const Entry =
            unsafe { core::mem::transmute(0x6000_6450usize) };
        unsafe { call(self.0.as_ptr()).as_ref() }
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let call: unsafe extern "C" fn(*mut c_void) -> i32 =
            unsafe { core::mem::transmute(0x6000_644cusize) };
        unsafe {
            call(self.0.as_ptr());
        }
    }
}

pub struct File(NonNull<c_void>);
impl File {
    /// Opens an existing file in read-only mode.
    ///
    /// # Safety
    /// Requires V690_H1's filesystem runtime and serialized calls on the PEG thread.
    pub unsafe fn open(path: &CStr) -> Option<Self> {
        let call: unsafe extern "C" fn(*const c_char, *const c_char) -> *mut c_void =
            unsafe { core::mem::transmute(0x6000_6400usize) };
        NonNull::new(unsafe { call(path.as_ptr(), c"r".as_ptr()) }).map(Self)
    }

    /// Creates or truncates a file. The caller must own the destination path.
    ///
    /// # Safety
    /// Requires V690_H1's runtime, serialized calls and an exclusively owned destination.
    pub unsafe fn create(path: &CStr) -> Option<Self> {
        let call: unsafe extern "C" fn(*const c_char, *const c_char) -> *mut c_void =
            unsafe { core::mem::transmute(0x6000_6400usize) };
        NonNull::new(unsafe { call(path.as_ptr(), c"w".as_ptr()) }).map(Self)
    }

    /// Opens an existing file for reading and writing without truncation.
    ///
    /// # Safety
    /// Requires V690_H1's runtime and serialized calls on the PEG thread.
    pub unsafe fn update(path: &CStr) -> Option<Self> {
        let call: unsafe extern "C" fn(*const c_char, *const c_char) -> *mut c_void =
            unsafe { core::mem::transmute(0x6000_6400usize) };
        NonNull::new(unsafe { call(path.as_ptr(), c"r+".as_ptr()) }).map(Self)
    }

    pub fn write(&mut self, buffer: &[u8]) -> Result<(), i32> {
        if buffer.is_empty() {
            return Ok(());
        }
        let call: unsafe extern "C" fn(*const u8, i32, i32, *mut c_void) -> i32 =
            unsafe { core::mem::transmute(0x6000_6410usize) };
        let length = i32::try_from(buffer.len()).expect("write buffer exceeds i32::MAX");
        let result = unsafe { call(buffer.as_ptr(), 1, length, self.0.as_ptr()) };
        if result == length {
            Ok(())
        } else {
            Err(result)
        }
    }

    pub fn flush(&mut self) -> Result<(), i32> {
        let call: unsafe extern "C" fn(*mut c_void) -> i32 =
            unsafe { core::mem::transmute(0x6000_641cusize) };
        let result = unsafe { call(self.0.as_ptr()) };
        if result == 0 { Ok(()) } else { Err(result) }
    }

    pub fn seek(&mut self, offset: i32) -> Result<(), i32> {
        let call: unsafe extern "C" fn(*mut c_void, i32, i32) -> i32 =
            unsafe { core::mem::transmute(0x6000_6414usize) };
        let result = unsafe { call(self.0.as_ptr(), offset, 0) };
        if result < 0 { Err(result) } else { Ok(()) }
    }

    pub fn read(&mut self, buffer: &mut [u8]) -> Result<usize, i32> {
        let call: unsafe extern "C" fn(*mut u8, i32, i32, *mut c_void) -> i32 =
            unsafe { core::mem::transmute(0x6000_640cusize) };
        let length = i32::try_from(buffer.len()).expect("read buffer exceeds i32::MAX");
        let result = unsafe { call(buffer.as_mut_ptr(), 1, length, self.0.as_ptr()) };
        if result < 0 {
            Err(result)
        } else {
            Ok(result as usize)
        }
    }

    pub fn close(self) -> Result<(), i32> {
        let result = self.close_raw();
        core::mem::forget(self);
        if result < 0 { Err(result) } else { Ok(()) }
    }

    fn close_raw(&self) -> i32 {
        let call: unsafe extern "C" fn(*mut c_void) -> i32 =
            unsafe { core::mem::transmute(0x6000_6404usize) };
        unsafe { call(self.0.as_ptr()) }
    }
}
impl Drop for File {
    fn drop(&mut self) {
        self.close_raw();
    }
}

const _: () = {
    assert!(core::mem::size_of::<Entry>() == 272);
    assert!(core::mem::offset_of!(Entry, name) == 12);
};

/// File metadata returned by the public stat wrapper.
#[repr(C)]
pub struct Metadata {
    pub mode: u32,
    timestamps: [u32; 3],
    pub size: u32,
}

/// # Safety
/// Requires V690_H1's runtime and serialized calls on the PEG thread.
pub unsafe fn metadata(path: &CStr) -> Result<Metadata, i32> {
    let call: unsafe extern "C" fn(*const c_char, *mut Metadata) -> i32 =
        unsafe { core::mem::transmute(0x6000_6444usize) };
    let mut metadata = Metadata {
        mode: 0,
        size: 0,
        timestamps: [0; 3],
    };
    let result = unsafe { call(path.as_ptr(), &mut metadata) };
    if result == 0 {
        Ok(metadata)
    } else {
        Err(result)
    }
}

/// # Safety
/// Requires V690_H1's runtime and serialized calls. Destination replacement is caller-controlled.
pub unsafe fn rename(source: &CStr, destination: &CStr) -> Result<(), i32> {
    let call: unsafe extern "C" fn(*const c_char, *const c_char) -> i32 =
        unsafe { core::mem::transmute(0x6000_6438usize) };
    let result = unsafe { call(source.as_ptr(), destination.as_ptr()) };
    if result == 0 { Ok(()) } else { Err(result) }
}

/// # Safety
/// Requires V690_H1's runtime and serialized calls. Removes a file or empty directory.
pub unsafe fn remove(path: &CStr) -> Result<(), i32> {
    let call: unsafe extern "C" fn(*const c_char) -> i32 =
        unsafe { core::mem::transmute(0x6000_6434usize) };
    let result = unsafe { call(path.as_ptr()) };
    if result == 0 { Ok(()) } else { Err(result) }
}

/// # Safety
/// Requires V690_H1's runtime and serialized calls on the PEG thread.
pub unsafe fn create_dir(path: &CStr) -> Result<(), i32> {
    let call: unsafe extern "C" fn(*const c_char) -> i32 =
        unsafe { core::mem::transmute(0x6000_643cusize) };
    let result = unsafe { call(path.as_ptr()) };
    if result == 0 { Ok(()) } else { Err(result) }
}

const _: () = {
    assert!(core::mem::size_of::<Metadata>() == 20);
    assert!(core::mem::offset_of!(Metadata, size) == 16);
};
