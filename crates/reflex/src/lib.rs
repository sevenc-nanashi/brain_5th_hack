//! Reflex: fifth-generation Sharp Brain application APIs.
//!
//! Device calls require privileged ARM execution and the documented firmware.
//! See the crate README for verification status and startup requirements.
#![no_std]

pub mod app;
pub mod display;
pub mod peg;

#[cfg(all(
    feature = "experimental-runtime",
    target_arch = "arm",
    target_os = "none"
))]
pub mod runtime;
