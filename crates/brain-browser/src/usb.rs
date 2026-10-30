use crate::engine;
use core::{
    ffi::{CStr, c_void},
    ptr,
};

pub const FRAME: usize = 468;
const HEADER: usize = 44;
const MAGIC: &[u8; 8] = b"BRNWEB01";
const MAX_BODY: u32 = 2 * 1024 * 1024;

// V690_H1 PCConnect ABI, shared with the experimental Explorer transport.
unsafe fn control(
    command: u32,
    input: *const c_void,
    size: u32,
    output: *mut c_void,
    length: u32,
    count: *mut u32,
) -> u32 {
    let call: unsafe extern "C" fn(u32, *const c_void, u32, *mut c_void, u32, *mut u32) -> u32 =
        unsafe { core::mem::transmute(0x6000_63c0usize) };
    unsafe { call(command, input, size, output, length, count) }
}
pub fn command(command: u32) -> bool {
    unsafe { control(command, ptr::null(), 0, ptr::null_mut(), 0, ptr::null_mut()) != 0 }
}
pub fn command4(command: u32) -> bool {
    let dummy = 0u32;
    unsafe {
        control(
            command,
            (&dummy as *const u32).cast(),
            4,
            ptr::null_mut(),
            0,
            ptr::null_mut(),
        ) != 0
    }
}
pub fn connect() -> bool {
    unsafe {
        control(
            0x2002d,
            c"DRM:".as_ptr().cast(),
            4,
            ptr::null_mut(),
            0,
            ptr::null_mut(),
        ) != 0
            && command(0x2002b)
    }
}
pub fn connected() -> bool {
    let mut value = 0u8;
    unsafe {
        control(
            0x20039,
            ptr::null(),
            0,
            (&mut value as *mut u8).cast(),
            1,
            ptr::null_mut(),
        );
    }
    value == 1
}
fn transfer(command: u32, bytes: &mut [u8]) -> Result<usize, &'static str> {
    let mut count = 0;
    let ok = unsafe {
        control(
            command,
            ptr::null(),
            0,
            bytes.as_mut_ptr().cast(),
            bytes.len() as u32,
            &mut count,
        )
    };
    if ok == 0 || count as usize > bytes.len() {
        return Err("USB transfer failed");
    }
    Ok(count as usize)
}
fn word(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}
fn put(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

pub struct Transport {
    input: [u8; FRAME],
    received: usize,
    last: Option<[u8; FRAME]>,
    current: u32,
    offset: u32,
}
impl Transport {
    pub fn new() -> Self {
        Self {
            input: [0; FRAME],
            received: 0,
            last: None,
            current: 0,
            offset: 0,
        }
    }
    pub fn poll(&mut self) -> Result<bool, &'static str> {
        self.received += transfer(0x20034, &mut self.input[self.received..])?;
        if self.received != FRAME {
            return Ok(false);
        }
        self.received = 0;
        let input = self.input;
        if &input[..8] != MAGIC {
            return Err("Invalid web packet");
        }
        if self.last.as_ref() == Some(&input) {
            return Ok(true);
        }
        if let Some(last) = self.last {
            if input[8..16] != last[8..16] || word(&input, 16) != word(&last, 16).wrapping_add(1) {
                return Err("PC session changed. Restart Browser");
            }
        } else {
            if word(&input, 16) != 0 || word(&input, 24) != 0 {
                return Err("Expected PC handshake");
            }
            crate::runtime::set_clock(word(&input, 32));
        }
        let id = word(&input, 20);
        let kind = word(&input, 24);
        let offset = word(&input, 28);
        let total = word(&input, 32);
        let status = word(&input, 36);
        let length = word(&input, 40) as usize;
        if length > FRAME - HEADER || kind > 5 || status > 599 {
            return Err("Invalid HTTP response");
        }
        let mut response = [0; FRAME];
        response[..HEADER].copy_from_slice(&input[..HEADER]);
        put(&mut response, 24, 11);
        put(&mut response, 36, 0);
        put(&mut response, 40, 0);
        if kind == 0 {
            if length != 0 {
                return Err("Invalid URL request");
            }
            let mut url = ptr::null();
            let request = unsafe { engine::run(|| engine::brain_request(&mut url)) };
            put(&mut response, 20, request);
            put(&mut response, 24, 10);
            put(&mut response, 32, 0);
            if request != 0 {
                let url = unsafe { CStr::from_ptr(url) }.to_bytes();
                if url.len() > 2048 {
                    return Err("URL exceeds 2048 bytes");
                }
                let start = if id == request { offset as usize } else { 0 };
                if start > url.len() {
                    return Err("Invalid URL offset");
                }
                let count = (url.len() - start).min(FRAME - HEADER);
                response[HEADER..HEADER + count].copy_from_slice(&url[start..start + count]);
                put(&mut response, 28, start as u32);
                put(&mut response, 32, url.len() as u32);
                put(&mut response, 40, count as u32);
                if self.current != request {
                    self.current = request;
                    self.offset = 0;
                }
            }
        } else {
            if id != self.current {
                put(&mut response, 36, 1);
            } else {
                if offset != self.offset
                    || (kind == 2 && length as u32 > MAX_BODY - self.offset)
                    || (kind == 3 && (total != self.offset || length != 0))
                {
                    return Err("Invalid HTTP body offset/size");
                }
                let result = unsafe {
                    engine::run(|| {
                        engine::brain_response(
                            id,
                            kind as i32,
                            status as i32,
                            input[HEADER..].as_ptr(),
                            length,
                        )
                    })
                };
                if result < 0 {
                    return Err("NetSurf rejected HTTP response");
                }
                put(&mut response, 36, result as u32);
                if kind == 2 && result == 0 {
                    self.offset += length as u32;
                }
            }
        }
        self.last = Some(input);
        if transfer(0x20035, &mut response)? != FRAME {
            return Err("USB send incomplete");
        }
        Ok(true)
    }
}
