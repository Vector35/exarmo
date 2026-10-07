#![doc = include_str!("../README.md")]
//!
//! The two instruction sets share one [`Instruction`], because the bundle does.
//! One set of instruction files holds both, and its aliases and operand
//! descriptions span both. No encoding is shared, so each entry point returns
//! only its own instruction set's variants.

#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(unsafe_code)]
#![warn(missing_docs)]

#[cfg(test)]
extern crate alloc;

pub mod a32;
pub mod generated;
pub mod operands;
pub mod rendering;
pub mod t32;
pub mod tokens;
mod types;

pub use exarmo_core::DecodeError;
pub use exarmo_core::branch::{Branch, BranchEffect, BranchKind};
pub use exarmo_core::flags::{FlagEffect, Flags};
pub use generated::enums::*;
pub use generated::{
    Cond, Encoding, Instruction, InstructionAt, Mnemonic, ModifierKind, PROVENANCE, Provenance,
    SysRegSpace,
};
pub use operands::{
    Lane, ListFile, Mem, Modifier, Offset, Operand, Operands, PcRead, Reg, RegList, RegOperand,
    Symbol, Writeback,
};
pub use tokens::{Decoded, Hex, TokenKind, TokenSink};
pub use types::{DReg, GpReg, ItState, QReg, SReg, Scalar, SysReg, SysRegDef};

#[cfg(feature = "alloc")]
pub use tokens::Token;
