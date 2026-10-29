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
pub mod audio;

#[cfg(all(
    feature = "experimental-runtime",
    target_arch = "arm",
    target_os = "none"
))]
pub mod runtime;

#[cfg(feature = "application")]
pub use reflex_macros::main;

pub mod application;
pub use application::{Action, Application, Event, TouchPhase};

#[cfg(feature = "panic-screen")]
pub mod panic_screen;
#[cfg(feature = "text")]
pub mod text;

#[cfg(all(feature = "alloc", target_arch = "arm", target_os = "none"))]
pub mod allocator;
#[cfg(all(feature = "application", target_arch = "arm", target_os = "none"))]
#[doc(hidden)]
pub mod lifecycle;
