//! The C face of the exarmo-aarch64 disassembler, declared in
//! `include/exarmo/aarch64.h`.
//!
//! An instruction is decoded once into an opaque value the caller holds,
//! and then read through functions: what it is, what it does to the flags,
//! its operands as `include/exarmo/aarch64.h` lays them out, and its text as spans
//! of a buffer the caller owns. Nothing allocates on the caller's behalf,
//! and no panic crosses the boundary. Each entry point catches one and
//! reports a failure.

#![warn(missing_docs)]

use std::fmt::Write;

use exarmo_aarch64::{Encoding, Instruction, Mnemonic};
pub use exarmo_core::capi::{Branch, FlagEffect, Status, Str, TextSize, Token};
use exarmo_core::capi::{
    Spans, Storage, buffer, bytes, deliver, fill, guarded, with_held, write_tokens,
};

pub mod model;
pub mod tables;

use model::{CIntrinsic, CIntrinsicDef, CIntrinsicTypeDef, COperand};

/// Opaque storage for an `Instruction`, `exarmo_aarch64_instruction` in C.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CInstruction {
    opaque: [u64; 3],
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
/// The header fixes the size of the `exarmo_aarch64_instruction` a caller
/// reserves. A caller linked against a library from another release checks
/// this against it.
#[unsafe(no_mangle)]
pub extern "C" fn exarmo_aarch64_instruction_size() -> usize {
    std::mem::size_of::<CInstruction>()
}

/// Decode `bits` into `out`, which is written only on `Status::Ok`.
///
/// # Safety
///
/// A non-null `out` must point to writable storage for a `CInstruction`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch64_decode_word(bits: u32, out: *mut CInstruction) -> Status {
    if out.is_null() {
        return Status::Failed;
    }
    unsafe { deliver(|| exarmo_aarch64::decode_word(bits), out) }
}

/// The former name of [`exarmo_aarch64_decode_word`], which the header marks
/// deprecated.
///
/// # Safety
///
/// See [`exarmo_aarch64_decode_word`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch64_decode(bits: u32, out: *mut CInstruction) -> Status {
    unsafe { exarmo_aarch64_decode_word(bits, out) }
}

/// Decode the instruction at the start of the `len` bytes at `bytes` into
/// `out`, which is written only on `Status::Ok`.
///
/// # Safety
///
/// A non-null `bytes` must point to `len` readable bytes, and a non-null
/// `out` to writable storage for a `CInstruction`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch64_decode_bytes(
    bytes: *const u8,
    len: usize,
    out: *mut CInstruction,
) -> Status {
    match (unsafe { self::bytes(bytes, len) }, out.is_null()) {
        (Some(code), false) => unsafe { deliver(|| exarmo_aarch64::decode_bytes(code), out) },
        _ => Status::Failed,
    }
}

/// Which encoding the instruction is. Returns `EXARMO_AARCH64_ENCODING_COUNT`
/// for a null pointer.
///
/// # Safety
///
/// A non-null `inst` must point to an instruction written by a successful
/// decode.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch64_instruction_encoding(inst: *const CInstruction) -> u32 {
    let none = Encoding::COUNT as u32;
    unsafe { with_held(inst, none, |inst| inst.encoding() as u32) }
}

/// The mnemonic the instruction is written with. Returns
/// `EXARMO_AARCH64_MNEMONIC_COUNT` for a null pointer.
///
/// # Safety
///
/// A non-null `inst` must point to an instruction written by a successful
/// decode.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch64_instruction_mnemonic(inst: *const CInstruction) -> u32 {
    let none = Mnemonic::COUNT as u32;
    unsafe { with_held(inst, none, |inst| inst.mnemonic() as u32) }
}

/// The mnemonic an encoding is written with. Returns
/// `EXARMO_AARCH64_MNEMONIC_COUNT` for a value that is no encoding.
#[unsafe(no_mangle)]
pub extern "C" fn exarmo_aarch64_encoding_mnemonic(encoding: u32) -> u32 {
    Encoding::from_index(encoding as usize).map_or(Mnemonic::COUNT as u32, |e| e.mnemonic() as u32)
}

/// An encoding's name, or empty for a value that is no encoding.
#[unsafe(no_mangle)]
pub extern "C" fn exarmo_aarch64_encoding_name(encoding: u32) -> Str {
    Encoding::from_index(encoding as usize).map_or(Str::EMPTY, |e| Str::of(e.name()))
}

/// How many bytes an encoding is, or 0 for a value that is no encoding.
///
/// Every A64 encoding is four. The AArch32 face answers 2 or 4, and a
/// caller stepping either instruction stream asks the same question.
#[unsafe(no_mangle)]
pub extern "C" fn exarmo_aarch64_encoding_length(encoding: u32) -> u8 {
    Encoding::from_index(encoding as usize).map_or(0, |e| e.length())
}

/// How many bytes of the instruction stream the instruction is, or 0 for a
/// null pointer.
///
/// # Safety
///
/// A non-null `inst` must point to an instruction written by a successful
/// decode.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch64_instruction_length(inst: *const CInstruction) -> u8 {
    unsafe { with_held(inst, 0, |inst| inst.encoding().length()) }
}

/// Whether the word `inst` was decoded from is CONSTRAINED UNPREDICTABLE,
/// through its decode's tests or a bit its diagram writes `(0)` or `(1)`.
/// Returns false for a null pointer.
///
/// # Safety
///
/// A non-null `inst` must point to an instruction written by a successful
/// decode.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch64_instruction_unpredictable(
    inst: *const CInstruction,
) -> bool {
    unsafe { with_held(inst, false, |inst| inst.unpredictable()) }
}

/// A mnemonic's text, or empty for a value that is no mnemonic.
#[unsafe(no_mangle)]
pub extern "C" fn exarmo_aarch64_mnemonic_name(mnemonic: u32) -> Str {
    Mnemonic::from_index(mnemonic as usize).map_or(Str::EMPTY, |m| Str::of(m.name()))
}

/// What the instruction does with the condition flags. Nothing for a null
/// pointer.
///
/// # Safety
///
/// A non-null `inst` must point to an instruction written by a successful
/// decode.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch64_instruction_flags(inst: *const CInstruction) -> FlagEffect {
    let none = FlagEffect::from(exarmo_aarch64::FlagEffect::NONE);
    unsafe { with_held(inst, none, |inst| inst.flags().into()) }
}

/// What the instruction does to the flow of control, as it reads at
/// `address`. Nothing for a null pointer.
///
/// # Safety
///
/// A non-null `inst` must point to an instruction written by a successful
/// decode.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch64_instruction_branch(
    inst: *const CInstruction,
    address: u64,
) -> Branch {
    unsafe { with_held(inst, Branch::NONE, |inst| inst.at(address).branch().into()) }
}

/// Write the instruction's operands into `out`, up to `capacity`. Returns
/// how many the instruction has.
///
/// # Safety
///
/// A non-null `inst` must point to an instruction written by a successful
/// decode. A non-null `out` must point to `capacity` writable operands.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch64_instruction_operands(
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
/// A non-null `inst` must point to an instruction written by a successful
/// decode. A non-null `text` must point to `text_capacity` writable bytes and a
/// non-null `tokens` to `token_capacity` writable tokens.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch64_instruction_tokens(
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
/// See [`exarmo_aarch64_instruction_tokens`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch64_instruction_text(
    inst: *const CInstruction,
    address: u64,
    text: *mut u8,
    capacity: usize,
) -> usize {
    unsafe {
        exarmo_aarch64_instruction_tokens(inst, address, text, capacity, std::ptr::null_mut(), 0)
    }
    .length
}

/// Write a system register's name into the caller's buffer. Returns the
/// length of the whole name.
///
/// # Safety
///
/// A non-null `name` must point to `capacity` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch64_sysreg_name(
    encoding: u16,
    write: bool,
    name: *mut u8,
    capacity: usize,
) -> usize {
    let name = unsafe { buffer(name, capacity) };
    guarded(0, || {
        let reg = if write {
            exarmo_aarch64::SysReg::write(encoding)
        } else {
            exarmo_aarch64::SysReg::read(encoding)
        };
        let mut spans = Spans::new(name, &mut []);
        match write!(spans, "{reg}") {
            Ok(()) => spans.finish().length,
            Err(_) => 0,
        }
    })
}

/// Write the ACLE intrinsics the instruction realises into `out`, up to
/// `capacity` of them. Returns how many it has.
///
/// The instruction cannot always tell them apart, since signed and unsigned
/// bytes are the same bytes. Where several are listed they are ordered by
/// what a reader would write first, and `out[0]` is the one to show.
///
/// The list is every intrinsic the instruction's own operands allow, so a
/// list of one is the only one it can be. Each carries its own arguments,
/// and they differ. CMGT is vcgt with its operands one way round and vclt
/// with them the other, so a caller showing an intrinsic other than
/// `out[0]` reads that one's arguments.
///
/// Each argument says where the instruction holds it, as an index into
/// `exarmo_aarch64_instruction_operands`. `exarmo_aarch64_intrinsic_at`
/// gives the name, signature and types for an `id`.
///
/// # Safety
///
/// `inst` must be a pointer `exarmo_aarch64_decode_word` filled, and `out` must be
/// writable for `capacity` elements, or null when `capacity` is zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch64_instruction_intrinsics(
    inst: *const CInstruction,
    out: *mut CIntrinsic,
    capacity: usize,
) -> usize {
    unsafe {
        with_held(inst, 0, |inst| {
            fill(
                out,
                capacity,
                inst.intrinsics().iter().map(CIntrinsic::from),
            )
        })
    }
}

/// Write the intrinsic `id` names into `out`. Returns false, leaving `out`
/// untouched, if `id` names none.
///
/// Every string points into the library and lives as long as it does.
///
/// # Safety
///
/// `out` must be writable, and is written only when this returns true.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch64_intrinsic_at(id: u32, out: *mut CIntrinsicDef) -> bool {
    if out.is_null() {
        return false;
    }
    guarded(false, || {
        let Some(def) = exarmo_aarch64::intrinsics::DEFS.get(id as usize) else {
            return false;
        };
        unsafe { out.write(CIntrinsicDef::from(def)) };
        true
    })
}

/// Write what the C type `ty` is into `out`. Returns false, leaving `out`
/// untouched, if `ty` is not one.
///
/// A definition's `result` and `parameters` are these indexes. There are
/// far fewer types than parameters, so a caller builds its own type for
/// each of these once.
///
/// The name points into the library and lives as long as it does.
///
/// # Safety
///
/// `out` must be writable, and is written only when this returns true.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn exarmo_aarch64_intrinsic_type_at(
    ty: u32,
    out: *mut CIntrinsicTypeDef,
) -> bool {
    if out.is_null() {
        return false;
    }
    guarded(false, || {
        let Some(def) = exarmo_aarch64::intrinsics::TYPES.get(ty as usize) else {
            return false;
        };
        unsafe { out.write(CIntrinsicTypeDef::from(def)) };
        true
    })
}
