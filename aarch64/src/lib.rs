#![doc = include_str!("../README.md")]
//!
//! ## Examples
//!
//! ### Decoding
//!
//! [`disassemble`] returns an iterator over the instructions in a byte buffer
//! and their addresses. [`decode_bytes`] decodes one instruction from a byte
//! slice, and [`decode_word`] one from a 32-bit word. For a word that is not an
//! instruction, each returns a [`DecodeError`] that says why.
//!
//! ```
//! use exarmo_aarch64::{DecodeError, decode_word, disassemble};
//!
//! // str x0, [sp, #-16]!
//! // ldr x0, [sp], #16
//! let code = [0xe0, 0x0f, 0x1f, 0xf8, 0xe0, 0x07, 0x41, 0xf8];
//! let text: Vec<_> = disassemble(&code, 0x1000)
//!     .map(|(address, instruction)| instruction.unwrap().at(address).to_string())
//!     .collect();
//! assert_eq!(text, ["str\tx0, [sp, #-0x10]!", "ldr\tx0, [sp], #0x10"]);
//!
//! // No row of the exception generation class allocates this word.
//! assert_eq!(decode_word(0xd478da60), Err(DecodeError::Unallocated));
//! ```
//!
//! ### What an instruction is
//!
//! An [`Instruction`] is an enum with a variant for every encoding and alias
//! the specification describes. Each variant holds the fields of its
//! encoding. For example, `cset x0, eq` decodes to `CsetCsinc64Condsel`.
//! Its `Xd` field is an [`XRegZr`], a 64-bit register where 31 is the zero
//! register. Its `invcond` field holds `ne`, the inverse of the condition
//! the assembly writes.
//!
//! There are thousands of variants, so most code does not match on them.
//! [`Instruction::encoding`] returns the variant as an [`Encoding`], a
//! fieldless enum with a name for each. [`Instruction::mnemonic`] returns a
//! [`Mnemonic`], which many encodings share. [`Instruction::operands`],
//! below, reads the operands of any variant in one shape.
//!
//! ```
//! use exarmo_aarch64::{Encoding, Instruction, Mnemonic, decode_word};
//!
//! // cset x0, eq
//! let cset = decode_word(0x9a9f17e0).unwrap();
//! let Instruction::CsetCsinc64Condsel { Xd, .. } = cset else {
//!     unreachable!()
//! };
//! assert_eq!(Xd.num(), 0);
//!
//! assert_eq!(cset.encoding(), Encoding::CsetCsinc64Condsel);
//! assert_eq!(cset.encoding().name(), "CsetCsinc64Condsel");
//! assert_eq!(cset.mnemonic(), Mnemonic::Cset);
//! assert_eq!(cset.mnemonic().name(), "cset");
//! ```
//!
//! ### Operands
//!
//! [`Instruction::operands`] gives the operands in the order the assembly
//! writes them, each an [`Operand`]. A memory operand is a single value that
//! holds its base, offset and writeback.
//!
//! ```
//! use exarmo_aarch64::{
//!     Mem, Offset, Operand, Reg, RegOperand, Writeback, XRegSp, XRegZr, decode_word,
//! };
//!
//! // str x0, [sp, #-16]!
//! let str = decode_word(0xf81f0fe0).unwrap();
//! assert_eq!(
//!     str.operands().as_slice(),
//!     [
//!         Operand::Reg(RegOperand::plain(Reg::X(XRegZr::new(0)))),
//!         Operand::Mem(Mem {
//!             base: Reg::XSp(XRegSp::new(31)),
//!             base_arrangement: None,
//!             offset: Offset::Imm { value: -16, mul_vl: false },
//!             writeback: Writeback::Pre,
//!         }),
//!     ]
//! );
//! ```
//!
//! A condition operand holds the condition as the assembly writes it. The
//! field of `cset x0, eq` holds `ne`, and its operand is `eq`.
//!
//! ```
//! use exarmo_aarch64::{Cond, Operand, decode_word};
//!
//! // cset x0, eq
//! let cset = decode_word(0x9a9f17e0).unwrap();
//! assert_eq!(cset.operands()[1], Operand::Cond(Cond::Eq));
//! ```
//!
//! ### Tokens
//!
//! [`InstructionAt::write_tokens`] writes the text to a [`TokenSink`] as
//! tokens, each with its [`TokenKind`] and the index of the operand it is
//! part of. Number tokens carry their values, so you never need to parse the
//! text.
//! The [`tokens`] module shows how to write your own sink.
//!
//! ```
//! use exarmo_aarch64::{Token, TokenKind, decode_word};
//!
//! // ldr x0, [x1, #8]
//! let mut tokens: Vec<Token> = Vec::new();
//! decode_word(0xf9400420).unwrap().at(0).write_tokens(&mut tokens).unwrap();
//!
//! let kinds: Vec<_> = tokens.iter().map(|t| (t.kind, t.operand, t.text.as_str())).collect();
//! assert_eq!(
//!     kinds,
//!     [
//!         (TokenKind::Mnemonic, None, "ldr"),
//!         (TokenKind::Text, None, "\t"),
//!         (TokenKind::Register, Some(0), "x0"),
//!         (TokenKind::Separator, None, ", "),
//!         (TokenKind::Bracket, Some(1), "["),
//!         (TokenKind::Register, Some(1), "x1"),
//!         (TokenKind::Text, Some(1), ", "),
//!         (TokenKind::Text, Some(1), "#"),
//!         (TokenKind::Immediate(8), Some(1), "0x8"),
//!         (TokenKind::Bracket, Some(1), "]"),
//!     ]
//! );
//! ```
//!
//! ### Flags and control flow
//!
//! [`Instruction::flags`] returns the condition flags an instruction writes
//! and reads. [`InstructionAt::branch`] returns what it does to the flow of
//! control, and for a direct branch, the address it branches to.
//!
//! ```
//! use exarmo_aarch64::{BranchKind, Flags, decode_word};
//!
//! // cmp x0, #0x41
//! let cmp = decode_word(0xf101041f).unwrap();
//! assert_eq!(cmp.flags().writes, Flags::NZCV);
//! assert!(cmp.flags().reads.is_empty());
//!
//! // b.ne #12
//! let branch = decode_word(0x54000061).unwrap().at(0x1000).branch();
//! assert_eq!(branch.kind, BranchKind::Direct);
//! assert!(branch.conditional);
//! assert_eq!(branch.target, Some(0x100c));
//!
//! // bl #64
//! let call = decode_word(0x94000010).unwrap().at(0x1000).branch();
//! assert_eq!(call.kind, BranchKind::DirectCall);
//! assert_eq!(call.target, Some(0x1040));
//!
//! // ret
//! let ret = decode_word(0xd65f03c0).unwrap().at(0x1000).branch();
//! assert_eq!(ret.kind, BranchKind::Return);
//! assert_eq!(ret.target, None);
//! ```
//!
//! ### CONSTRAINED UNPREDICTABLE words
//!
//! For some words, the architecture permits the processor to execute the
//! instruction or to do something else. Decoding one of these words returns
//! the instruction, and [`Instruction::unpredictable`] returns true for it.
//!
//! ```
//! use exarmo_aarch64::decode_word;
//!
//! // rcwsetp x0, x0, [sp], which names x0 for both registers of its pair
//! let rcwsetp = decode_word(0x1920b3e0).unwrap();
//! assert_eq!(rcwsetp.at(0).to_string(), "rcwsetp\tx0, x0, [sp]");
//! assert!(rcwsetp.unpredictable());
//! ```
//!
//! ### System registers and operations
//!
//! MRS and MSR take a [`SysReg`] operand, which gives the register's name
//! where the architecture has one. AT, DC, IC and TLBI take a system
//! operation, and MSR (immediate) a PSTATE field. Their operands refer to
//! rows of [`SysOp::all`].
//!
//! ```
//! use exarmo_aarch64::{Mnemonic, Operand, decode_word};
//!
//! // msr vbar_el3, x0
//! let Operand::SysReg(register) = decode_word(0xd51ec000).unwrap().operands()[0] else {
//!     unreachable!()
//! };
//! assert_eq!(register.name(), Some("vbar_el3"));
//! assert!(register.is_write());
//!
//! // ic iallu
//! let Operand::SysOp(operation) = decode_word(0xd508751f).unwrap().operands()[0] else {
//!     unreachable!()
//! };
//! assert_eq!(operation.def().name, "iallu");
//! assert_eq!(operation.def().instruction, Mnemonic::Ic);
//! ```
//!
//! ### Intrinsics
//!
//! [`Instruction::intrinsics`] lists the ACLE Advanced SIMD intrinsics an
//! instruction implements, and binds each argument to one of the
//! instruction's operands.
//!
//! ```
//! use exarmo_aarch64::{Output, Source, decode_word};
//!
//! // addp v0.8b, v1.8b, v0.8b
//! let addp = decode_word(0x0e20bc20).unwrap();
//! let vpadd = addp.intrinsics().iter().find(|i| i.name() == "vpadd_s8").unwrap();
//! assert_eq!(vpadd.arguments, [Source::Operand(1), Source::Operand(2)]);
//! assert_eq!(vpadd.result, Output::Operand(0));
//! ```
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
