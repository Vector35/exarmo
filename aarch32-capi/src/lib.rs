//! The C face of the exarmo-aarch32 disassembler, declared in
//! `include/exarmo/aarch32.h`.
//!
//! An instruction is decoded once into an opaque value the caller holds,
//! and then read through functions: what it is, what it does to the flags,
//! its operands as `include/exarmo/aarch32.h` lays them out, and its text as spans
//! of a buffer the caller owns. Nothing allocates on the caller's behalf,
//! and no panic crosses the boundary. Each entry point catches one and
//! reports a failure.
//!
//! A32 and T32 are two decode entry points into one instruction. A T32
//! decode is told the IT state, since a 16-bit encoding takes its
//! condition from a preceding IT instruction, and a caller that tracks the
//! blocks advances the state through `exarmo_aarch32_it_state_after`.

#![warn(missing_docs)]

use exarmo_aarch32::{Cond, Encoding, Instruction, ItState, Mnemonic};
pub use exarmo_core::capi::{Branch, FlagEffect, Status, Str, TextSize, Token};
use exarmo_core::capi::{Storage, deliver, fill, guarded, with_held, write_tokens};

pub mod model;
pub mod tables;

use model::{COperand, SysReg};

/// Opaque storage for an `Instruction`, `exarmo_aarch32_instruction` in C.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CInstruction {
    opaque: [u64; 5],
}

// SAFETY: the assertions below hold the storage to an `Instruction`'s size
// and alignment.
unsafe impl Storage for CInstruction {
    type Instruction = Instruction;
}

const _: () = assert!(std::mem::size_of::<Instruction>() <= std::mem::size_of::<CInstruction>());
const _: () = assert!(std::mem::align_of::<Instruction>() <= std::mem::align_of::<CInstruction>());

/// How many bytes of storage a decode writes, as the library was built.
///
/// The header fixes the size of the `exarmo_aarch32_instruction` a caller
/// reserves. A caller linked against a library from another release checks
/// this against it.
#[unsafe(no_mangle)]
pub extern "C" fn exarmo_aarch32_instruction_size() -> usize {
    std::mem::size_of::<CInstruction>()
}

/// Whether a T32 instruction is inside an IT block, `exarmo_aarch32_it_kind`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItKind {
    /// The caller has not tracked the blocks. Read as outside one.
    Unknown = 0,
    /// Outside any block.
    Outside = 1,
    /// Inside a block, under `cond`, with `mask` of it left.
    Inside = 2,
}

/// The IT state a T32 decode is told, `exarmo_aarch32_it_state`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CItState {
    /// Which of the three, an `ItKind`.
    pub kind: u8,
    /// Inside a block, the condition this instruction runs under, an
    /// `exarmo_aarch32_cond`.
    pub cond: u8,
    /// Inside a block, the low four bits of ITSTATE, which say what is left
    /// of it.
    pub mask: u8,
}

impl From<CItState> for ItState {
    fn from(state: CItState) -> Self {
        match state.kind {
            1 => ItState::Outside,
            2 => ItState::Inside {
                cond: Cond::from_bits(state.cond).unwrap_or(Cond::Al),
                mask: state.mask & 0xf,
            },
            _ => ItState::Unknown,
        }
    }
}

impl From<ItState> for CItState {
    fn from(state: ItState) -> Self {
        match state {
            ItState::Unknown => CItState {
                kind: ItKind::Unknown as u8,
                cond: 0,
                mask: 0,
            },
            ItState::Outside => CItState {
                kind: ItKind::Outside as u8,
                cond: 0,
                mask: 0,
            },
            ItState::Inside { cond, mask } => CItState {
                kind: ItKind::Inside as u8,
                cond: cond as u8,
                mask,
            },
        }
    }
}

/// Decode the A32 word `bits` into `out`, which is written only on
/// `Status::Ok`.
///
/// # Safety
///
/// A non-null `out` must point to writable storage for a `CInstruction`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch32_decode_a32(bits: u32, out: *mut CInstruction) -> Status {
    if out.is_null() {
        return Status::Failed;
    }
    unsafe { deliver(|| exarmo_aarch32::a32::decode(bits), out) }
}

/// Decode the T32 word `bits`, its first halfword in the high half, under
/// the IT state `state`, into `out`, which is written only on `Status::Ok`.
///
/// # Safety
///
/// A non-null `out` must point to writable storage for a `CInstruction`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch32_decode_t32(
    bits: u32,
    state: CItState,
    out: *mut CInstruction,
) -> Status {
    if out.is_null() {
        return Status::Failed;
    }
    unsafe { deliver(|| exarmo_aarch32::t32::decode(bits, state.into()), out) }
}

/// How many bytes the T32 instruction beginning with the halfword `hw1`
/// takes, 2 or 4.
#[unsafe(no_mangle)]
pub extern "C" fn exarmo_aarch32_t32_length(hw1: u16) -> u8 {
    guarded(0, || exarmo_aarch32::t32::length(hw1))
}

/// The IT state the instruction after `inst` is in, on the straight line,
/// given that `inst` was decoded under `state`. Returns `state` itself for a
/// null pointer.
///
/// # Safety
///
/// See [`exarmo_aarch32_decode_t32`] for what `inst` must point to.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch32_it_state_after(
    state: CItState,
    inst: *const CInstruction,
) -> CItState {
    unsafe { with_held(inst, state, |inst| ItState::from(state).after(inst).into()) }
}

/// How many bytes of the instruction stream the instruction is, 4 for an
/// A32 instruction and 2 or 4 for a T32 one. Returns 0 for a null pointer.
///
/// # Safety
///
/// See [`exarmo_aarch32_decode_a32`] for what `inst` must point to.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch32_instruction_length(inst: *const CInstruction) -> u8 {
    unsafe { with_held(inst, 0, |inst| inst.encoding().length()) }
}

/// Whether the word `inst` was decoded from is CONSTRAINED UNPREDICTABLE,
/// through its decode's tests or a bit its diagram writes `(0)` or `(1)`.
/// Returns false for a null pointer.
///
/// # Safety
///
/// See [`exarmo_aarch32_decode_a32`] for what `inst` must point to.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch32_instruction_unpredictable(
    inst: *const CInstruction,
) -> bool {
    unsafe { with_held(inst, false, |inst| inst.unpredictable()) }
}

/// Which encoding the instruction is. Returns `EXARMO_AARCH32_ENCODING_COUNT`
/// for a null pointer.
///
/// # Safety
///
/// See [`exarmo_aarch32_decode_a32`] for what `inst` must point to.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch32_instruction_encoding(inst: *const CInstruction) -> u32 {
    let none = Encoding::COUNT as u32;
    unsafe { with_held(inst, none, |inst| inst.encoding() as u32) }
}

/// The mnemonic the instruction is written with. Returns
/// `EXARMO_AARCH32_MNEMONIC_COUNT` for a null pointer.
///
/// # Safety
///
/// See [`exarmo_aarch32_decode_a32`] for what `inst` must point to.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch32_instruction_mnemonic(inst: *const CInstruction) -> u32 {
    let none = Mnemonic::COUNT as u32;
    unsafe { with_held(inst, none, |inst| inst.mnemonic() as u32) }
}

/// The mnemonic an encoding is written with. Returns
/// `EXARMO_AARCH32_MNEMONIC_COUNT` for a value that is no encoding.
#[unsafe(no_mangle)]
pub extern "C" fn exarmo_aarch32_encoding_mnemonic(encoding: u32) -> u32 {
    Encoding::from_index(encoding as usize).map_or(Mnemonic::COUNT as u32, |e| e.mnemonic() as u32)
}

/// An encoding's name, or empty for a value that is no encoding.
#[unsafe(no_mangle)]
pub extern "C" fn exarmo_aarch32_encoding_name(encoding: u32) -> Str {
    Encoding::from_index(encoding as usize).map_or(Str::EMPTY, |e| Str::of(e.name()))
}

/// How many bytes an encoding is, or 0 for a value that is no encoding.
#[unsafe(no_mangle)]
pub extern "C" fn exarmo_aarch32_encoding_length(encoding: u32) -> u8 {
    Encoding::from_index(encoding as usize).map_or(0, |e| e.length())
}

/// A mnemonic's text, or empty for a value that is no mnemonic.
#[unsafe(no_mangle)]
pub extern "C" fn exarmo_aarch32_mnemonic_name(mnemonic: u32) -> Str {
    Mnemonic::from_index(mnemonic as usize).map_or(Str::EMPTY, |m| Str::of(m.name()))
}

/// What the instruction does with the condition flags. Nothing for a null
/// pointer.
///
/// # Safety
///
/// See [`exarmo_aarch32_decode_a32`] for what `inst` must point to.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch32_instruction_flags(inst: *const CInstruction) -> FlagEffect {
    let none = FlagEffect::from(exarmo_aarch32::FlagEffect::NONE);
    unsafe { with_held(inst, none, |inst| inst.flags().into()) }
}

/// What the instruction does to the flow of control, as it reads at
/// `address`. Nothing for a null pointer.
///
/// # Safety
///
/// See [`exarmo_aarch32_decode_a32`] for what `inst` must point to.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch32_instruction_branch(
    inst: *const CInstruction,
    address: u64,
) -> Branch {
    unsafe { with_held(inst, Branch::NONE, |inst| inst.at(address).branch().into()) }
}

/// Write the system register the instruction accesses into `out`, and
/// return true. Returns false, leaving `out` untouched, for an instruction
/// that reaches none or a null pointer.
///
/// # Safety
///
/// See [`exarmo_aarch32_decode_a32`] for what `inst` must point to. A
/// non-null `out` must point to a writable `exarmo_aarch32_sysreg`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch32_instruction_sysreg(
    inst: *const CInstruction,
    out: *mut SysReg,
) -> bool {
    match unsafe { with_held(inst, None, |inst| inst.system_register()) } {
        Some(reg) if !out.is_null() => {
            unsafe { out.write(reg.into()) };
            true
        }
        _ => false,
    }
}

/// The name of the register with this key in this space, accessed in this
/// direction, or empty where the architecture names none.
#[unsafe(no_mangle)]
pub extern "C" fn exarmo_aarch32_sysreg_name(space: u8, encoding: u32, write: bool) -> Str {
    exarmo_aarch32::SysRegSpace::from_bits(space)
        .and_then(|space| exarmo_aarch32::SysReg::new(space, encoding, write).name())
        .map_or(Str::EMPTY, Str::of)
}

/// Write the instruction's operands into `out`, up to `capacity`. Returns
/// how many the instruction has.
///
/// # Safety
///
/// See [`exarmo_aarch32_decode_a32`] for what `inst` must point to. A
/// non-null `out` must point to `capacity` writable operands.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch32_instruction_operands(
    inst: *const CInstruction,
    out: *mut COperand,
    capacity: usize,
) -> usize {
    unsafe {
        with_held(inst, 0, |inst| {
            fill(
                out,
                capacity,
                inst.operands().iter().map(|op| COperand::from(*op)),
            )
        })
    }
}

/// Write the instruction's text and tokens into the caller's buffers.
///
/// # Safety
///
/// See [`exarmo_aarch32_decode_a32`] for what `inst` must point to. A
/// non-null `text` must point to `text_capacity` writable bytes and a
/// non-null `tokens` to `token_capacity` writable tokens.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch32_instruction_tokens(
    inst: *const CInstruction,
    address: u64,
    text: *mut u8,
    text_capacity: usize,
    tokens: *mut Token,
    token_capacity: usize,
) -> TextSize {
    unsafe { write_tokens(inst, address, text, text_capacity, tokens, token_capacity) }
}

/// Write the instruction's text into the caller's buffer. Returns the
/// length of the whole text.
///
/// # Safety
///
/// See [`exarmo_aarch32_instruction_tokens`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch32_instruction_text(
    inst: *const CInstruction,
    address: u64,
    text: *mut u8,
    capacity: usize,
) -> usize {
    unsafe {
        exarmo_aarch32_instruction_tokens(inst, address, text, capacity, std::ptr::null_mut(), 0)
    }
    .length
}
