#![doc = include_str!("../README.md")]
#![deny(unsafe_code)]
#![warn(missing_docs)]

/// The decoder, instruction model and tables generated from ARM's XML.
pub mod generated;
pub mod intrinsics;
pub mod operands;
pub mod rendering;
pub mod tokens;
mod types;

pub mod sysops;
pub use sysops::{RegisterAccess, RegisterUse, SysOp, SysOpRef};

pub use exarmo_core::DecodeError;
pub use exarmo_core::branch::{Branch, BranchEffect, BranchKind};
pub use exarmo_core::flags::{FlagEffect, Flags};
pub use generated::enums::*;
pub use generated::{
    Encoding, Instruction, InstructionAt, Mnemonic, ModifierKind, PROVENANCE, Provenance, decode,
};
pub use intrinsics::{
    Intrinsic, IntrinsicDef, IntrinsicType, IntrinsicTypeDef, Output, Source, TypeKind,
};
pub use operands::{
    Mem, Modifier, Offset, Operand, Operands, PcRead, Reg, RegList, RegOperand, Symbol, Writeback,
    ZaArray, ZaSlice,
};
pub use tokens::{Decoded, Hex, Token, TokenKind, TokenSink};
pub use types::{
    Arrangement, BReg, DReg, DynRegSp, DynRegZr, DynScalarSimd, ElementWidth, GpReg, HReg, PNReg,
    PReg, QReg, SReg, SysReg, SysRegDef, VReg, WRegSp, WRegZr, XRegSp, XRegZr, ZATile, ZReg,
    ZaTileMask, ZaTileName,
};
