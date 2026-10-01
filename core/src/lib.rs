//! The runtime support shared by the AArch64 and AArch32 disassemblers.
//!
//! Register, arrangement and data type names belong to each instruction set
//! and stay out of this crate.

#![warn(missing_docs)]

pub mod address;
pub mod branch;
pub mod capi;
pub mod condition;
pub mod decode;
pub mod flags;
pub mod names;
pub mod register;
pub mod tokens;
pub mod view;
pub mod write;

pub use decode::{DecodeError, Decoded};
