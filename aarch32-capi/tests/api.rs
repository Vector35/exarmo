//! The C entry points, called as C would call them.

use std::ffi::CStr;
use std::mem::MaybeUninit;

use exarmo_aarch32_capi::model::{COperand, OffsetKind, OperandKind, RegClass, Writeback};
use exarmo_aarch32_capi::*;

const OUTSIDE: CItState = CItState {
    kind: ItKind::Outside as u8,
    cond: 0,
    mask: 0,
};

fn a32(bits: u32) -> CInstruction {
    let mut out = MaybeUninit::<CInstruction>::uninit();
    let status = unsafe { exarmo_aarch32_decode_a32_word(bits, out.as_mut_ptr()) };
    assert_eq!(status, Status::Ok, "{bits:08x}");
    unsafe { out.assume_init() }
}

fn t32(bits: u32, state: CItState) -> CInstruction {
    let mut out = MaybeUninit::<CInstruction>::uninit();
    let status = unsafe { exarmo_aarch32_decode_t32_word(bits, state, out.as_mut_ptr()) };
    assert_eq!(status, Status::Ok, "{bits:08x}");
    unsafe { out.assume_init() }
}

fn text_of(s: Str) -> &'static str {
    std::str::from_utf8(unsafe { std::slice::from_raw_parts(s.data, s.length) }).unwrap()
}

fn text(inst: &CInstruction, address: u64) -> String {
    let mut buf = [0u8; 64];
    let n = unsafe { exarmo_aarch32_instruction_text(inst, address, buf.as_mut_ptr(), buf.len()) };
    let written = CStr::from_bytes_until_nul(&buf).unwrap().to_str().unwrap();
    assert_eq!(n, written.len());
    written.to_string()
}

const BLANK_TOKEN: Token = Token {
    kind: 0,
    operand: Token::NO_OPERAND,
    offset: 0,
    length: 0,
    value: 0,
};

fn operands_of(inst: &CInstruction) -> Vec<COperand> {
    let mut out = vec![COperand::from(exarmo_aarch32::Operand::Other); 8];
    let n = unsafe { exarmo_aarch32_instruction_operands(inst, out.as_mut_ptr(), out.len()) };
    out.truncate(n);
    out
}

#[test]
fn a_decode_that_fails_says_how() {
    let mut out = MaybeUninit::<CInstruction>::uninit();
    let mut a32 = |bits| unsafe { exarmo_aarch32_decode_a32_word(bits, out.as_mut_ptr()) };
    assert_eq!(a32(0xf1200000), Status::Unallocated);
    assert_eq!(a32(0xf1200070), Status::Unpredictable);
    assert_eq!(a32(0xe320f020), Status::ReservedHint);
    assert_eq!(
        unsafe { exarmo_aarch32_decode_a32_word(0xe2800001, std::ptr::null_mut()) },
        Status::Failed
    );
    assert_eq!(
        unsafe { exarmo_aarch32_decode_t32_word(0xf9bf_f000, OUTSIDE, out.as_mut_ptr()) },
        Status::ReservedHint
    );
}

/// Every outcome an AArch32 decode can reach, found by a stride over the
/// whole encoding space of each set. `Status::Nop` is left out because no row
/// of either index decodes to one.
#[test]
fn every_outcome_the_decoder_reaches() {
    let mut out = MaybeUninit::<CInstruction>::uninit();
    let mut a32 = |bits| unsafe { exarmo_aarch32_decode_a32_word(bits, out.as_mut_ptr()) };
    assert_eq!(a32(0xe2800001), Status::Ok);
    assert_eq!(a32(0x002000d8), Status::Unallocated);
    // AESMC with a size it reserves, which is UNDEFINED by the word alone
    assert_eq!(a32(0xf3b4f3af), Status::Undefined);
    assert_eq!(a32(0xf1200076), Status::Unpredictable);
    assert_eq!(a32(0x0320001f), Status::ReservedHint);

    let mut t32 = |bits| unsafe { exarmo_aarch32_decode_t32_word(bits, OUTSIDE, out.as_mut_ptr()) };
    assert_eq!(t32(0x46080000), Status::Ok);
    assert_eq!(t32(0xb6200008), Status::Unallocated);
    assert_eq!(t32(0xffb4f3af), Status::Undefined);
    assert_eq!(t32(0xbf600002), Status::ReservedHint);
}

/// The bytes of an A32 `bx lr`.
#[test]
fn an_a32_decode_from_bytes_reads_them_as_memory_holds_them() {
    let bx = [0x1e, 0xff, 0x2f, 0xe1];
    let mut out = MaybeUninit::<CInstruction>::uninit();
    let null = std::ptr::null();
    unsafe {
        let decode = |bytes, len, out| exarmo_aarch32_decode_a32_bytes(bytes, len, out);
        assert_eq!(decode(bx.as_ptr(), bx.len(), out.as_mut_ptr()), Status::Ok);
        assert_eq!(text(out.assume_init_ref(), 0), "bx\tlr");
        assert_eq!(decode(bx.as_ptr(), 3, out.as_mut_ptr()), Status::Truncated);
        assert_eq!(decode(null, 4, out.as_mut_ptr()), Status::Failed);
        assert_eq!(decode(bx.as_ptr(), 4, std::ptr::null_mut()), Status::Failed);
    }
}

/// `bx lr` is a 16-bit instruction and `bl` a 32-bit one, so the decode reads
/// two bytes for the first and four for the second.
#[test]
fn a_t32_decode_from_bytes_reads_as_many_as_the_instruction_takes() {
    let bx = [0x70, 0x47];
    let bl = [0x00, 0xf0, 0x00, 0xf8];
    let mut out = MaybeUninit::<CInstruction>::uninit();
    unsafe {
        let decode = |bytes: &[u8], len, out| {
            exarmo_aarch32_decode_t32_bytes(bytes.as_ptr(), len, OUTSIDE, out)
        };
        assert_eq!(decode(&bx, bx.len(), out.as_mut_ptr()), Status::Ok);
        assert_eq!(exarmo_aarch32_instruction_length(out.as_ptr()), 2);
        assert_eq!(decode(&bl, bl.len(), out.as_mut_ptr()), Status::Ok);
        assert_eq!(exarmo_aarch32_instruction_length(out.as_ptr()), 4);
        assert_eq!(decode(&bl, 2, out.as_mut_ptr()), Status::Truncated);
        assert_eq!(decode(&bl, 1, out.as_mut_ptr()), Status::Truncated);
        assert_eq!(
            exarmo_aarch32_decode_t32_bytes(std::ptr::null(), 2, OUTSIDE, out.as_mut_ptr()),
            Status::Failed
        );
    }
}

#[test]
fn a_null_instruction_is_answered() {
    let null: *const CInstruction = std::ptr::null();
    unsafe {
        assert_eq!(
            exarmo_aarch32_instruction_encoding(null),
            exarmo_aarch32::Encoding::COUNT as u32
        );
        assert_eq!(
            exarmo_aarch32_instruction_mnemonic(null),
            exarmo_aarch32::Mnemonic::COUNT as u32
        );
        assert_eq!(exarmo_aarch32_instruction_length(null), 0);
        assert!(!exarmo_aarch32_instruction_unpredictable(null));
        let branch = exarmo_aarch32_instruction_branch(null, 0);
        assert_eq!(branch.kind, exarmo_aarch32::BranchKind::None as u8);
        let flags = exarmo_aarch32_instruction_flags(null);
        assert_eq!((flags.writes, flags.reads), (0, 0));
        let mut operands = [COperand::from(exarmo_aarch32::Operand::Other); 4];
        assert_eq!(
            exarmo_aarch32_instruction_operands(null, operands.as_mut_ptr(), operands.len()),
            0
        );
        let mut text = [0u8; 64];
        assert_eq!(
            exarmo_aarch32_instruction_text(null, 0, text.as_mut_ptr(), text.len()),
            0
        );
        let mut tokens = [BLANK_TOKEN; 8];
        let size = exarmo_aarch32_instruction_tokens(
            null,
            0,
            text.as_mut_ptr(),
            text.len(),
            tokens.as_mut_ptr(),
            tokens.len(),
        );
        assert_eq!((size.length, size.tokens), (0, 0));
    }
}

#[test]
fn an_index_past_the_end_names_nothing() {
    assert_eq!(text_of(exarmo_aarch32_encoding_name(u32::MAX)), "");
    assert_eq!(text_of(exarmo_aarch32_mnemonic_name(u32::MAX)), "");
    assert_eq!(
        exarmo_aarch32_encoding_mnemonic(u32::MAX),
        exarmo_aarch32::Mnemonic::COUNT as u32
    );
    assert_eq!(exarmo_aarch32_encoding_length(u32::MAX), 0);
}

#[test]
fn the_text_is_written_and_measured() {
    let ldr = a32(0xe5901000);
    assert_eq!(text(&ldr, 0), "ldr\tr1, [r0]");

    let mut small = [0xAAu8; 8];
    let n = unsafe { exarmo_aarch32_instruction_text(&ldr, 0, small.as_mut_ptr(), small.len()) };
    assert_eq!(n, "ldr\tr1, [r0]".len());
    assert_eq!(&small[..8], b"ldr\tr1,\0");

    let n = unsafe { exarmo_aarch32_instruction_text(&ldr, 0, std::ptr::null_mut(), 0) };
    assert_eq!(n, "ldr\tr1, [r0]".len());
}

#[test]
fn the_tokens_span_the_text() {
    // beq at 0x1000 reaches 0x1000 + 8 + 0
    let b = a32(0x0a000000);
    let mut text = [0u8; 64];
    let mut tokens = [Token {
        kind: 0,
        operand: Token::NO_OPERAND,
        offset: 0,
        length: 0,
        value: 0,
    }; 16];
    let size = unsafe {
        exarmo_aarch32_instruction_tokens(
            &b,
            0x1000,
            text.as_mut_ptr(),
            text.len(),
            tokens.as_mut_ptr(),
            tokens.len(),
        )
    };
    let written = std::str::from_utf8(&text[..size.length]).unwrap();
    assert_eq!(written, "beq\t0x1008");
    let spans: Vec<(u32, &str, u64)> = tokens[..size.tokens]
        .iter()
        .map(|t| {
            (
                t.kind,
                &written[t.offset as usize..(t.offset + t.length) as usize],
                t.value,
            )
        })
        .collect();
    assert_eq!(
        spans,
        [
            (0, "b", 0),
            (0, "eq", 0),
            (1, "\t", 0),
            (8, "0x1008", 0x1008)
        ]
    );

    let size = unsafe {
        exarmo_aarch32_instruction_tokens(
            &b,
            0x1000,
            text.as_mut_ptr(),
            text.len(),
            tokens.as_mut_ptr(),
            2,
        )
    };
    assert_eq!(size.tokens, 4);
}

#[test]
fn the_operands_are_laid_out_as_the_header_says() {
    // ldr r1, [r0], whose view begins with its condition
    let ldr = a32(0xe5901000);
    let ops = operands_of(&ldr);
    assert_eq!(ops.len(), 3);
    assert_eq!(ops[0].kind, OperandKind::Cond);
    assert_eq!(unsafe { ops[0].value.cond }, exarmo_aarch32::Cond::Al as u8);
    assert_eq!(ops[1].kind, OperandKind::Reg);
    let reg = unsafe { ops[1].value.reg };
    assert_eq!((reg.reg.class, reg.reg.num), (RegClass::Core as u8, 1));
    assert_eq!(reg.index, -1);
    assert!(!reg.modifier.present && !reg.writeback);
    assert_eq!(ops[2].kind, OperandKind::Mem);
    let mem = unsafe { ops[2].value.mem };
    assert_eq!((mem.base.class, mem.base.num), (RegClass::Core as u8, 0));
    // The word holds a zero offset, which the text does not write
    assert_eq!(mem.offset.kind, OffsetKind::Imm);
    assert_eq!((mem.offset.imm, mem.offset.subtract), (0, false));
    // str r1, [r0], #-0 holds the same zero, subtracted
    let str = a32(0xe4001000);
    let subtracted = unsafe { operands_of(&str)[2].value.mem };
    assert_eq!(
        (subtracted.offset.imm, subtracted.offset.subtract),
        (0, true)
    );
    assert_eq!(mem.writeback, Writeback::None);
    assert_eq!(mem.align, 0);

    // add r0, r0, r1, lsl #3. The shift is folded onto the register it
    // applies to.
    let add = a32(0xe0800181);
    let ops = operands_of(&add);
    assert_eq!(ops.len(), 4);
    assert_eq!(ops[3].kind, OperandKind::Reg);
    let shift = unsafe { ops[3].value.reg.modifier };
    assert!(shift.present);
    assert_eq!(shift.kind, exarmo_aarch32::ModifierKind::Lsl as u8);
    assert_eq!(shift.amount, 3);
    assert!(!shift.has_by);

    // push {r4, lr}
    let push = a32(0xe92d4010);
    let ops = operands_of(&push);
    assert_eq!(ops[1].kind, OperandKind::List);
    let list = unsafe { ops[1].value.list };
    assert_eq!(list.mask, 1 << 4 | 1 << 14);
    assert_eq!(list.file, exarmo_aarch32_capi::model::CListFile::Core as u8);

    // beq, whose label is an offset from a PC 8 ahead
    let b = a32(0x0a000000);
    let ops = operands_of(&b);
    assert_eq!(ops[1].kind, OperandKind::Label);
    let label = unsafe { ops[1].value.label };
    assert_eq!((label.offset, label.pc_ahead, label.pc_align), (0, 8, 1));

    let mut one = [COperand::from(exarmo_aarch32::Operand::Other)];
    let n = unsafe { exarmo_aarch32_instruction_operands(&add, one.as_mut_ptr(), 1) };
    assert_eq!(n, 4);
    assert_eq!(one[0].kind, OperandKind::Cond);
    assert!(n <= exarmo_aarch32::Instruction::MAX_OPERANDS);
}

#[test]
fn a_t32_instruction_takes_its_condition_from_the_it_state() {
    // 1840 is `adds r0, r0, r1` outside a block and `addeq r0, r0, r1` inside
    let word = 0x1840_0000;
    assert_eq!(text(&t32(word, OUTSIDE), 0), "adds\tr0, r0, r1");
    let inside = CItState {
        kind: ItKind::Inside as u8,
        cond: 0,
        mask: 0b1000,
    };
    assert_eq!(text(&t32(word, inside), 0), "addeq\tr0, r0, r1");
    let unknown = CItState {
        kind: ItKind::Unknown as u8,
        cond: 0,
        mask: 0,
    };
    assert_eq!(text(&t32(word, unknown), 0), "adds\tr0, r0, r1");
}

#[test]
fn the_it_state_advances_through_a_block() {
    // itett eq, so the next four run under eq, ne, eq and eq
    let it = t32(0xbf09_0000, OUTSIDE);
    assert_eq!(text(&it, 0), "itett\teq");
    let add = t32(0x4408_0000, OUTSIDE);
    let mut state = unsafe { exarmo_aarch32_it_state_after(OUTSIDE, &it) };
    for (cond, mask) in [(0, 0b1001), (1, 0b0010), (0, 0b0100), (0, 0b1000)] {
        assert_eq!(
            state,
            CItState {
                kind: ItKind::Inside as u8,
                cond,
                mask
            }
        );
        state = unsafe { exarmo_aarch32_it_state_after(state, &add) };
    }
    assert_eq!(state, OUTSIDE);
    assert_eq!(
        unsafe { exarmo_aarch32_it_state_after(OUTSIDE, std::ptr::null()) },
        OUTSIDE
    );
}

#[test]
fn the_header_agrees_with_the_enums() {
    let header = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/include/exarmo/aarch32_generated.h"
    ))
    .expect("the generator wrote include/exarmo/aarch32_generated.h");
    let mut encodings = 0;
    let mut mnemonics = 0;
    let mut modifier_kinds: Vec<(usize, String)> = Vec::new();
    let mut conditions = 0;
    let mut conditions_counted = None;
    for (name, value) in exarmo_testing::capi::header_numbers(&header) {
        let name = name.as_str();
        if let Some(variant) = name.strip_prefix("EXARMO_AARCH32_ENC_") {
            assert_eq!(
                exarmo_aarch32::Encoding::from_index(value).unwrap().name(),
                variant
            );
            encodings += 1;
        } else if name == "EXARMO_AARCH32_ENCODING_COUNT" {
            assert_eq!(value, exarmo_aarch32::Encoding::COUNT);
        } else if name == "EXARMO_AARCH32_MAX_OPERANDS" {
            assert_eq!(value, exarmo_aarch32::Instruction::MAX_OPERANDS);
        } else if name == "EXARMO_AARCH32_MAX_TOKENS" {
            assert_eq!(value, exarmo_aarch32::Instruction::MAX_TOKENS);
        } else if name == "EXARMO_AARCH32_MAX_TEXT" {
            assert_eq!(value, exarmo_aarch32::Instruction::MAX_TEXT);
        } else if name == "EXARMO_AARCH32_SYSREG_COUNT" {
            assert_eq!(value, exarmo_aarch32_capi::tables::SYSREGS.len());
        } else if name == "EXARMO_AARCH32_MNEMONIC_COUNT" {
            assert_eq!(value, exarmo_aarch32::Mnemonic::COUNT);
        } else if name == "EXARMO_AARCH32_MOD_COUNT" {
            assert_eq!(value, modifier_kinds.len());
        } else if let Some(kind) = name.strip_prefix("EXARMO_AARCH32_MOD_") {
            modifier_kinds.push((value, kind.to_lowercase()));
        } else if name == "EXARMO_AARCH32_SYSREG_SPACE_COUNT" {
            assert!(exarmo_aarch32::SysRegSpace::from_bits(value as u8).is_none());
            assert!(value > 0 && exarmo_aarch32::SysRegSpace::from_bits(value as u8 - 1).is_some());
        } else if let Some(space) = name.strip_prefix("EXARMO_AARCH32_SYSREG_SPACE_") {
            let named = exarmo_aarch32::SysRegSpace::from_bits(value as u8)
                .unwrap_or_else(|| panic!("{name} is numbered {value}"));
            assert_eq!(format!("{named:?}").to_uppercase(), space);
        } else if name == "EXARMO_AARCH32_COND_COUNT" {
            conditions_counted = Some(value);
        } else if let Some(cond) = name.strip_prefix("EXARMO_AARCH32_COND_") {
            // The discriminant must equal the `cond` field's bits, so a value
            // read from an operand or put in an IT state means the same on
            // both sides.
            let named = exarmo_aarch32::Cond::from_bits(value as u8)
                .filter(|c| *c as usize == value)
                .unwrap_or_else(|| panic!("{name} is numbered {value}"));
            assert_eq!(named.name(), cond.to_lowercase());
            conditions += 1;
        } else if let Some(mnemonic) = name.strip_prefix("EXARMO_AARCH32_") {
            assert_eq!(
                exarmo_aarch32::Mnemonic::from_index(value).unwrap().name(),
                mnemonic.to_lowercase()
            );
            mnemonics += 1;
        }
    }
    assert_eq!(encodings, exarmo_aarch32::Encoding::COUNT);
    assert_eq!(mnemonics, exarmo_aarch32::Mnemonic::COUNT);
    assert_eq!(conditions, 15);
    assert_eq!(conditions_counted, Some(conditions));
    assert!(modifier_kinds.contains(&(
        exarmo_aarch32::ModifierKind::Lsl as usize,
        "lsl".to_string()
    )));
    assert!(modifier_kinds.contains(&(
        exarmo_aarch32::ModifierKind::Rrx as usize,
        "rrx".to_string()
    )));
}
