#![doc = include_str!("../README.md")]
#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(unsafe_code)]
#![warn(missing_docs)]

#[cfg(test)]
extern crate alloc;

mod bytes;
/// The decoder, instruction model and tables generated from ARM's XML.
pub mod generated;
pub mod intrinsics;
pub mod operands;
pub mod rendering;
pub mod tokens;
mod types;

pub mod sysops;
pub use sysops::{RegisterAccess, RegisterUse, SysOp, SysOpRef};

pub use bytes::{decode_bytes, disassemble};
pub use exarmo_core::DecodeError;
pub use exarmo_core::branch::{Branch, BranchEffect, BranchKind};
pub use exarmo_core::flags::{FlagEffect, Flags};
pub use generated::enums::*;
pub use generated::{
    Encoding, Instruction, InstructionAt, Mnemonic, ModifierKind, PROVENANCE, Provenance,
    decode_word,
};
pub use intrinsics::{
    Intrinsic, IntrinsicDef, IntrinsicType, IntrinsicTypeDef, Output, Source, TypeKind,
};
pub use operands::{
    Mem, Modifier, Offset, Operand, Operands, PcRead, Reg, RegList, RegOperand, Symbol, Writeback,
    ZaArray, ZaSlice,
};
pub use tokens::{Decoded, Hex, TokenKind, TokenSink};
pub use types::{
    Arrangement, BReg, DReg, DynRegSp, DynRegZr, DynScalarSimd, ElementWidth, GpReg, HReg, PNReg,
    PReg, QReg, SReg, SysReg, SysRegDef, VReg, WRegSp, WRegZr, XRegSp, XRegZr, ZATile, ZReg,
    ZaTileMask, ZaTileName,
};

#[cfg(feature = "alloc")]
pub use tokens::Token;

/// Decode the 32-bit AArch64 instruction word `bits`.
#[deprecated(
    since = "0.4.0",
    note = "renamed to `decode_word`. Use `decode_bytes` to decode from a byte slice"
)]
#[inline]
pub fn decode(bits: u32) -> Result<Instruction, DecodeError> {
    decode_word(bits)
}
