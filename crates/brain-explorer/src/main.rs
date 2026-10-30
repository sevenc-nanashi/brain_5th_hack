#![no_std]
#![no_main]
extern crate alloc;

use brain_explorer::Explorer;
use brain_explorer::device::{Device, path};

mod compression;
mod source;
mod usb;

#[reflex::main]
fn main() -> usb::Sender {
    usb::Sender::new(Explorer::new(Device))
}
