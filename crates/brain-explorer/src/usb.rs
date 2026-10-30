use super::{Device, source::Source};
use alloc::{format, vec};
use brain_explorer::Explorer;
use core::{ffi::c_void, ptr};
use reflex::{
    Action, Application, Event, TouchPhase, Window, display,
    peg::{self, key, raw},
    runtime, text,
};

const TIMER: u32 = 0x5553;
const MAGIC: &[u8; 8] = b"BRNFILE3";
const CHUNK: usize = 64 * 1024;
const MAX_ENCODED: usize = CHUNK + 1024;

// V690_H1's experimental PCConnect ABI; see docs/usb.md.
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
fn request(command: u32) -> bool {
    unsafe { control(command, ptr::null(), 0, ptr::null_mut(), 0, ptr::null_mut()) != 0 }
}
fn command4(command: u32) -> bool {
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
fn connected() -> bool {
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
fn word(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes.try_into().unwrap())
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Browsing,
    Detaching,
    Connecting,
    Ready,
    Busy,
    Done,
    Error,
    Stopping,
}

struct Transfer {
    file: Source,
    size: u64,
    next: u64,
    token: Option<[u8; 8]>,
    last: Option<[u8; 28]>,
    input: [u8; 28],
    received: usize,
}
impl Transfer {
    fn poll(&mut self) -> Result<Option<bool>, &'static str> {
        self.received += transfer(0x20034, &mut self.input[self.received..])?;
        if self.received != self.input.len() {
            return Ok(None);
        }
        self.received = 0;
        let input = self.input;
        if &input[..8] != MAGIC {
            return Err("Invalid PC request");
        }
        if self.last == Some(input) {
            return Ok(None);
        }
        let token: [u8; 8] = input[8..16].try_into().unwrap();
        if let Some(current) = self.token
            && token != current
        {
            return Err("PC session changed. Cancel and retry");
        }
        let offset = u64::from_le_bytes(input[16..24].try_into().unwrap());
        let command = word(&input[24..28]);
        if offset != self.next || !matches!(command, 1 | 2) {
            return Err("Invalid file request offset");
        }
        if command == 2 && offset != self.size {
            return Err("File is not fully received");
        }
        let mut response = vec![0u8; 44 + MAX_ENCODED];
        response[..8].copy_from_slice(MAGIC);
        response[8..16].copy_from_slice(&token);
        response[16..24].copy_from_slice(&offset.to_le_bytes());
        response[24..32].copy_from_slice(&self.size.to_le_bytes());
        response[36..40].copy_from_slice(&command.to_le_bytes());
        let length = if command == 1 {
            (self.size - offset).min(CHUNK as u64) as usize
        } else {
            0
        };
        let mut raw = vec![0u8; length];
        let mut read = 0;
        while read < length {
            let count = self
                .file
                .read(&mut raw[read..])
                .map_err(|_| "Cannot read file")?;
            if count == 0 {
                return Err("File size changed. Cancel and retry");
            }
            read += count;
        }
        if offset + length as u64 == self.size {
            let mut extra = [0];
            if self.file.read(&mut extra).map_err(|_| "Cannot read file")? != 0 {
                return Err("File size changed. Cancel and retry");
            }
        }
        let encoded = if command == 1 {
            unsafe { super::compression::encode_block(&raw, &mut response[44..]) }
        } else {
            0
        };
        response[32..36].copy_from_slice(&(encoded as u32).to_le_bytes());
        response[40..44].copy_from_slice(&(length as u32).to_le_bytes());
        response.truncate(44 + encoded);

        // Record the request before firmware calls that could dispatch another PEG message.
        self.last = Some(input);
        self.token = Some(token);
        if transfer(0x20035, &mut response)? != response.len() {
            return Err("USB send incomplete");
        }
        self.next += length as u64;
        Ok(Some(command == 2))
    }
}

pub struct Sender {
    explorer: Explorer<Device>,
    window: *mut c_void,
    state: State,
    since: u64,
    message: &'static str,
    transfer: Option<Transfer>,
    exit: bool,
    cancel_pressed: bool,
}
impl Sender {
    pub fn new(explorer: Explorer<Device>) -> Self {
        Self {
            explorer,
            window: ptr::null_mut(),
            state: State::Browsing,
            since: 0,
            message: "",
            transfer: None,
            exit: false,
            cancel_pressed: false,
        }
    }
    fn set(&mut self, state: State, message: &'static str) {
        self.state = state;
        self.message = message;
        self.since = unsafe { runtime::micros() };
    }
    fn start(&mut self, name: &[u8], size: u32, directory: bool) -> Result<(), &'static str> {
        let file = Source::new(name, size, directory)?;
        let size = file.size;
        self.transfer = Some(Transfer {
            file,
            size,
            next: 0,
            token: None,
            last: None,
            input: [0; 28],
            received: 0,
        });
        self.set(State::Detaching, "Switching USB...");
        if !request(0x2002c) {
            return Err("USB disconnect failed");
        }
        Ok(())
    }
    fn stop(&mut self) {
        if self.transfer.is_none() {
            self.set(State::Browsing, "");
            return;
        }
        self.set(State::Stopping, "Disconnecting...");
        let disabled = command4(0x2003b);
        let detached = request(0x2002c);
        if !disabled || !detached {
            self.set(State::Error, "Disconnect failed. Esc to retry");
        }
    }
    fn tick(&mut self) -> Result<Action, &'static str> {
        let elapsed = unsafe { runtime::micros() }.wrapping_sub(self.since);
        if matches!(
            self.state,
            State::Detaching | State::Connecting | State::Stopping
        ) && elapsed > 120_000_000
        {
            return Err("USB timeout. Esc to disconnect");
        }
        match self.state {
            State::Detaching if elapsed >= 2_000_000 && !connected() => {
                self.set(State::Busy, "Switching USB...");
                let selected = unsafe {
                    control(
                        0x2002d,
                        c"DRM:".as_ptr().cast(),
                        4,
                        ptr::null_mut(),
                        0,
                        ptr::null_mut(),
                    )
                };
                if selected == 0 || !request(0x2002b) {
                    return Err("DRM connection failed");
                }
                self.set(State::Connecting, "Connect USB to PC");
            }
            State::Connecting if elapsed >= 2_000_000 && connected() => {
                self.set(State::Busy, "Connecting...");
                if !command4(0x2003a) || !command4(0x20036) {
                    return Err("USB enable failed");
                }
                self.set(State::Ready, "Waiting for PC receiver...");
            }
            State::Ready => {
                if !connected() {
                    return Err("USB disconnected");
                }
                self.state = State::Busy;
                let result = self.transfer.as_mut().unwrap().poll()?;
                self.state = State::Ready;
                match result {
                    Some(true) => self.set(State::Done, "Received on PC"),
                    Some(false) => self.set(State::Ready, "Sending..."),
                    None if elapsed > 120_000_000 => {
                        return Err("PC receiver timeout. Esc to disconnect");
                    }
                    None => return Ok(Action::None),
                }
            }
            State::Stopping if elapsed >= 2_000_000 && !connected() => {
                self.transfer = None;
                self.set(State::Browsing, "");
                if self.exit {
                    return Ok(Action::Exit);
                }
            }
            _ => return Ok(Action::None),
        }
        Ok(Action::Redraw)
    }
}
impl Application for Sender {
    fn mounted(&mut self, window: Window) -> Action {
        self.window = window.raw;
        unsafe {
            raw::set_timer(self.window, TIMER, 1, 1);
        }
        self.explorer.mounted(window)
    }
    fn raw_event(&mut self, message: &peg::Message) -> Option<Action> {
        if message.kind != peg::PM_TIMER || message.param != TIMER as u16 {
            return None;
        }
        Some(match self.tick() {
            Ok(action) => action,
            Err(error) => {
                self.set(State::Error, error);
                Action::Redraw
            }
        })
    }
    fn event(&mut self, event: Event) -> Action {
        if self.state != State::Browsing {
            let cancel = match event {
                Event::Key {
                    code: key::ESCAPE,
                    down: true,
                    ..
                } => true,
                Event::Touch { x, y, phase } => {
                    let inside = (16..=350).contains(&x) && (400..=474).contains(&y);
                    match phase {
                        TouchPhase::Down => {
                            self.cancel_pressed = inside;
                            false
                        }
                        TouchPhase::Move => {
                            self.cancel_pressed &= inside;
                            false
                        }
                        TouchPhase::Up => core::mem::take(&mut self.cancel_pressed) && inside,
                    }
                }
                _ => false,
            };
            if cancel && self.state != State::Stopping {
                self.stop();
                return Action::Redraw;
            }
            return Action::None;
        }
        let action = self.explorer.event(event);
        if let Some((name, size, directory)) = self.explorer.take_send_target() {
            if let Err(error) = self.start(&name, size, directory) {
                self.set(State::Error, error);
            }
            return Action::Redraw;
        }
        action
    }
    fn can_exit(&mut self) -> bool {
        if self.state == State::Browsing || self.transfer.is_none() {
            unsafe {
                raw::kill_timer(self.window, TIMER);
            }
            true
        } else {
            self.exit = true;
            if self.state != State::Stopping {
                self.stop();
            }
            false
        }
    }
    fn draw(&self, pixels: &mut [u16]) {
        if self.state == State::Browsing {
            self.explorer.draw(pixels);
            return;
        }
        pixels.fill(0xffff);
        for y in 400..475 {
            pixels[y * display::WIDTH + 16..y * display::WIDTH + 351].fill(0xdedb);
        }
        let progress = match &self.transfer {
            Some(transfer) => format!("{} / {} bytes", transfer.next, transfer.size),
            None => "0 bytes".into(),
        };
        for (y, line) in [
            (18, "Send to PC"),
            (72, self.message),
            (118, &progress),
            (418, "Esc  Disconnect / Back"),
        ] {
            text::draw(
                |x, y, color| pixels[y * display::WIDTH + x] = color,
                16,
                y,
                line,
                0,
                if y == 418 { 0xdedb } else { 0xffff },
                1,
            );
        }
    }
}
