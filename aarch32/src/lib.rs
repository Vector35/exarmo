#![doc = include_str!("../README.md")]
//!
//! A32 and T32 decode to the same [`Instruction`] type, so code that reads
//! instructions handles both instruction sets at once. The decoders in [`a32`]
//! only return A32 encodings, and those in [`t32`] only return T32 encodings.
//!
//! ## Examples
//!
//! ### Decoding A32
//!
//! [`a32::disassemble`] returns an iterator over the instructions in a buffer
//! of A32 code and their addresses. [`a32::decode_bytes`] decodes one
//! instruction from a byte slice, and [`a32::decode_word`] one from a 32-bit
//! word. For a word that is not an instruction, each returns a [`DecodeError`]
//! that says why.
//!
//! ```
//! use exarmo_aarch32::a32;
//!
//! // push {r4, lr}
//! // bx lr
//! let code = [0x10, 0x40, 0x2d, 0xe9, 0x1e, 0xff, 0x2f, 0xe1];
//! let text: Vec<_> = a32::disassemble(&code, 0x1000)
//!     .map(|(address, instruction)| instruction.unwrap().at(address).to_string())
//!     .collect();
//! assert_eq!(text, ["push\t{r4, lr}", "bx\tlr"]);
//! ```
//!
//! ### Decoding T32
//!
//! A T32 instruction is 16 or 32 bits long. [`t32::disassemble`] and
//! [`t32::decode_bytes`] read as many bytes as each instruction takes.
//!
//! A T32 decode also takes the IT state, as an instruction inside an IT block
//! takes its condition from the block. [`t32::disassemble`] advances the state
//! from one instruction to the next with [`ItState::after`], which assumes no
//! branch intervenes.
//!
//! ```
//! use exarmo_aarch32::{ItState, t32};
//!
//! // ite eq
//! // moveq r0, #1
//! // movne r0, #0
//! // bx lr
//! let code = [0x0c, 0xbf, 0x01, 0x20, 0x00, 0x20, 0x70, 0x47];
//! let text: Vec<_> = t32::disassemble(&code, 0x1000, ItState::Outside)
//!     .map(|(address, instruction)| instruction.unwrap().at(address).to_string())
//!     .collect();
//! assert_eq!(text, ["ite\teq", "moveq\tr0, #0x1", "movne\tr0, #0x0", "bx\tlr"]);
//!
//! // Outside a block, the same 16 bits decode as the flag-setting form.
//! let movs = t32::decode_bytes(&[0x01, 0x20], ItState::Outside).unwrap();
//! assert_eq!(movs.at(0).to_string(), "movs\tr0, #0x1");
//! ```
//!
//! ### What an instruction is
//!
//! An [`Instruction`] is an enum with a variant for every encoding and alias
//! the specification describes. Each variant holds the fields of its
//! encoding. For example, `push {r4, lr}` decodes to `PushStmdbA1`. Its
//! `cond` field is a [`Cond`], and its `registers` field is a [`RegList`].
//!
//! There are thousands of variants, so most code does not match on them.
//! [`Instruction::encoding`] returns the variant as an [`Encoding`], a
//! fieldless enum with a name for each. [`Instruction::mnemonic`] returns a
//! [`Mnemonic`], which many encodings share. [`Instruction::operands`],
//! below, reads the operands of any variant in one shape.
//!
//! ```
//! use exarmo_aarch32::{Encoding, GpReg, Instruction, Mnemonic, Reg, a32};
//!
//! // push {r4, lr}
//! let push = a32::decode_word(0xe92d4010).unwrap();
//! let Instruction::PushStmdbA1 { registers, .. } = push else {
//!     unreachable!()
//! };
//! assert_eq!(
//!     registers.regs().collect::<Vec<_>>(),
//!     [Reg::Core(GpReg::new(4)), Reg::Core(GpReg::new(14))]
//! );
//!
//! assert_eq!(push.encoding(), Encoding::PushStmdbA1);
//! assert_eq!(push.encoding().name(), "PushStmdbA1");
//! assert_eq!(push.mnemonic(), Mnemonic::Push);
//! assert_eq!(push.mnemonic().name(), "push");
//! ```
//!
//! ### Operands
//!
//! [`Instruction::operands`] returns the operands in the order the assembly
//! writes them, each an [`Operand`]. For an instruction that can be
//! conditional, the first operand is its condition. It will be `al` when none
//! is written. An instruction that is always unconditional, such as `bkpt` or
//! an A32 Advanced SIMD instruction, has no condition operand. A register list
//! is one operand, and a memory operand is a single value that holds its base,
//! offset and writeback.
//!
//! ```
//! use exarmo_aarch32::{Cond, GpReg, Mem, Offset, Operand, Reg, RegOperand, Writeback, a32};
//!
//! // push {r4, lr}
//! let operands = a32::decode_word(0xe92d4010).unwrap().operands();
//! let [Operand::Cond(Cond::Al), Operand::List(list)] = operands.as_slice() else {
//!     unreachable!()
//! };
//! assert_eq!(
//!     list.regs().collect::<Vec<_>>(),
//!     [Reg::Core(GpReg::new(4)), Reg::Core(GpReg::new(14))]
//! );
//!
//! // ldrne r0, [r1, #-8]
//! let ldr = a32::decode_word(0x15110008).unwrap();
//! assert_eq!(
//!     ldr.operands().as_slice(),
//!     [
//!         Operand::Cond(Cond::Ne),
//!         Operand::Reg(RegOperand::plain(Reg::Core(GpReg::new(0)))),
//!         Operand::Mem(Mem {
//!             base: Reg::Core(GpReg::new(1)),
//!             offset: Offset::Imm { value: -8, subtract: true },
//!             writeback: Writeback::None,
//!             align: None,
//!         }),
//!     ]
//! );
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
//! use exarmo_aarch32::{Token, TokenKind, a32};
//!
//! // addeq r2, r1, #4
//! let mut tokens: Vec<Token> = Vec::new();
//! a32::decode_word(0x02812004).unwrap().at(0).write_tokens(&mut tokens).unwrap();
//!
//! let kinds: Vec<_> = tokens.iter().map(|t| (t.kind, t.operand, t.text.as_str())).collect();
//! assert_eq!(
//!     kinds,
//!     [
//!         (TokenKind::Mnemonic, None, "add"),
//!         (TokenKind::Mnemonic, Some(0), "eq"),
//!         (TokenKind::Text, None, "\t"),
//!         (TokenKind::Register, Some(1), "r2"),
//!         (TokenKind::Separator, None, ", "),
//!         (TokenKind::Register, Some(2), "r1"),
//!         (TokenKind::Separator, None, ", "),
//!         (TokenKind::Text, Some(3), "#"),
//!         (TokenKind::Immediate(4), Some(3), "0x4"),
//!     ]
//! );
//! ```
//!
//! ### Flags and control flow
//!
//! [`Instruction::flags`] returns the condition flags an instruction writes
//! and reads. [`InstructionAt::branch`] returns what it does to the flow of
//! control, and for a direct branch, the address it branches to. Any
//! instruction that writes the PC is a branch, including a load such as
//! `ldr pc, [r1]`.
//!
//! ```
//! use exarmo_aarch32::{BranchKind, Flags, a32};
//!
//! // adds r2, r1, r3
//! let adds = a32::decode_word(0xe0912003).unwrap();
//! assert_eq!(adds.flags().writes, Flags::NZCV);
//!
//! // beq #16, which A32 reads from 8 bytes ahead
//! let beq = a32::decode_word(0x0a000002).unwrap().at(0x1000).branch();
//! assert_eq!(beq.kind, BranchKind::Direct);
//! assert!(beq.conditional);
//! assert_eq!(beq.target, Some(0x1010));
//!
//! // ldr pc, [r1]
//! let jump = a32::decode_word(0xe591f000).unwrap().at(0x1000).branch();
//! assert_eq!(jump.kind, BranchKind::Indirect);
//!
//! // ldr r0, [r1]
//! let load = a32::decode_word(0xe5910000).unwrap().at(0x1000).branch();
//! assert_eq!(load.kind, BranchKind::None);
//! ```
//!
//! ### CONSTRAINED UNPREDICTABLE words
//!
//! For some words, the architecture permits the processor to execute the
//! instruction or to do something else. Decoding one of these words returns
//! the instruction, and [`Instruction::unpredictable`] returns true for it.
//!
//! ```
//! use exarmo_aarch32::a32;
//!
//! // ldr r0, [r0], #4, which loads into the base it writes back
//! let ldr = a32::decode_word(0xe4900004).unwrap();
//! assert_eq!(ldr.at(0).to_string(), "ldr\tr0, [r0], #0x4");
//! assert!(ldr.unpredictable());
//! ```
//!
//! ### System registers
//!
//! [`Instruction::system_register`] returns the system register an MRC, MCR,
//! MRRC or MCRR accesses. [`SysReg::name`] returns the architecture's name for
//! it, where there is one.
//!
//! ```
//! use exarmo_aarch32::a32;
//!
//! // mrc p15, #0, r0, c1, c0, #0
//! let mrc = a32::decode_word(0xee110f10).unwrap();
//! let register = mrc.system_register().unwrap();
//! assert_eq!(register.name(), Some("sctlr"));
//! assert!(!register.is_write());
//! ```

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
