//! The C entry points, called as C would call them.

use std::ffi::CStr;
use std::mem::MaybeUninit;

use exarmo_aarch64_capi::Str;
use exarmo_aarch64_capi::model::{
    CIntrinsicDef, CIntrinsicTypeDef, COperand, OffsetKind, OperandKind, RegClass, Writeback,
};
use exarmo_aarch64_capi::*;

fn decoded(bits: u32) -> CInstruction {
    let mut out = MaybeUninit::<CInstruction>::uninit();
    let status = unsafe { exarmo_aarch64_decode(bits, out.as_mut_ptr()) };
    assert_eq!(status, Status::Ok, "{bits:08x}");
    unsafe { out.assume_init() }
}

fn text_of(s: Str) -> &'static str {
    std::str::from_utf8(unsafe { std::slice::from_raw_parts(s.data, s.length) }).unwrap()
}

const BLANK_TOKEN: Token = Token {
    kind: 0,
    operand: Token::NO_OPERAND,
    offset: 0,
    length: 0,
    value: 0,
};

fn operands_of(inst: &CInstruction) -> Vec<COperand> {
    let mut out = vec![COperand::from(exarmo_aarch64::Operand::Other); 8];
    let n = unsafe { exarmo_aarch64_instruction_operands(inst, out.as_mut_ptr(), out.len()) };
    out.truncate(n);
    out
}

#[test]
fn a_decode_that_fails_says_how() {
    let mut out = MaybeUninit::<CInstruction>::uninit();
    assert_eq!(
        unsafe { exarmo_aarch64_decode(0x00010000, out.as_mut_ptr()) },
        Status::Unallocated
    );
    assert_eq!(
        unsafe { exarmo_aarch64_decode(0xF9400420, std::ptr::null_mut()) },
        Status::Failed
    );
}

/// Every outcome an A64 decode can reach, found by a stride over the whole
/// encoding space.
///
/// `Status::Nop` is left out because no row of the A64 index decodes to one.
/// `Unpredictable` and `ReservedHint` are left out because only the AArch32
/// index marks them.
#[test]
fn every_outcome_the_decoder_reaches() {
    let mut out = MaybeUninit::<CInstruction>::uninit();
    let mut decode = |bits| unsafe { exarmo_aarch64_decode(bits, out.as_mut_ptr()) };
    assert_eq!(decode(0xF9400420), Status::Ok);
    assert_eq!(decode(0x00010005), Status::Unallocated);
    assert_eq!(decode(0x04008002), Status::Undefined);
}

#[test]
fn a_null_instruction_is_answered() {
    let null: *const CInstruction = std::ptr::null();
    unsafe {
        assert_eq!(
            exarmo_aarch64_instruction_encoding(null),
            exarmo_aarch64::Encoding::COUNT as u32
        );
        assert_eq!(
            exarmo_aarch64_instruction_mnemonic(null),
            exarmo_aarch64::Mnemonic::COUNT as u32
        );
        let flags = exarmo_aarch64_instruction_flags(null);
        assert_eq!((flags.writes, flags.reads), (0, 0));
        let branch = exarmo_aarch64_instruction_branch(null, 0);
        assert_eq!(branch, Branch::NONE);
        let mut operands = [COperand::from(exarmo_aarch64::Operand::Other); 4];
        assert_eq!(
            exarmo_aarch64_instruction_operands(null, operands.as_mut_ptr(), operands.len()),
            0
        );
        let mut text = [0u8; 64];
        assert_eq!(
            exarmo_aarch64_instruction_text(null, 0, text.as_mut_ptr(), text.len()),
            0
        );
        let mut tokens = [BLANK_TOKEN; 8];
        let size = exarmo_aarch64_instruction_tokens(
            null,
            0,
            text.as_mut_ptr(),
            text.len(),
            tokens.as_mut_ptr(),
            tokens.len(),
        );
        assert_eq!((size.length, size.tokens), (0, 0));
        assert_eq!(
            exarmo_aarch64_instruction_intrinsics(null, std::ptr::null_mut(), 0),
            0
        );
        assert_eq!(exarmo_aarch64_instruction_length(null), 0);
        assert!(!exarmo_aarch64_instruction_unpredictable(null));
    }
}

#[test]
fn an_index_past_the_end_names_nothing() {
    assert_eq!(text_of(exarmo_aarch64_encoding_name(u32::MAX)), "");
    assert_eq!(text_of(exarmo_aarch64_mnemonic_name(u32::MAX)), "");
    assert_eq!(
        exarmo_aarch64_encoding_mnemonic(u32::MAX),
        exarmo_aarch64::Mnemonic::COUNT as u32
    );
    assert_eq!(exarmo_aarch64_encoding_length(u32::MAX), 0);
    let mut intrinsic = MaybeUninit::<CIntrinsicDef>::uninit();
    assert!(!unsafe { exarmo_aarch64_intrinsic_at(u32::MAX, intrinsic.as_mut_ptr()) });
    let mut ty = MaybeUninit::<CIntrinsicTypeDef>::uninit();
    assert!(!unsafe { exarmo_aarch64_intrinsic_type_at(u32::MAX, ty.as_mut_ptr()) });
    // A system register encoding the architecture names nothing for is
    // still spelt, by the encoding itself, as the assembly writes one
    let mut name = [0u8; 32];
    let length =
        unsafe { exarmo_aarch64_sysreg_name(u16::MAX, false, name.as_mut_ptr(), name.len()) };
    assert_eq!(&name[..length], b"s3_7_c15_c15_7");
}

#[test]
fn the_text_is_written_and_measured() {
    let ldr = decoded(0xF9400420);
    let mut buf = [0xAAu8; 64];
    let n = unsafe { exarmo_aarch64_instruction_text(&ldr, 0, buf.as_mut_ptr(), buf.len()) };
    let expected = "ldr\tx0, [x1, #0x8]";
    assert_eq!(n, expected.len());
    assert_eq!(
        CStr::from_bytes_until_nul(&buf).unwrap().to_str().unwrap(),
        expected
    );

    let mut small = [0xAAu8; 8];
    let n = unsafe { exarmo_aarch64_instruction_text(&ldr, 0, small.as_mut_ptr(), small.len()) };
    assert_eq!(n, expected.len());
    assert_eq!(&small[..8], b"ldr\tx0,\0");

    let n = unsafe { exarmo_aarch64_instruction_text(&ldr, 0, std::ptr::null_mut(), 0) };
    assert_eq!(n, expected.len());
}

#[test]
fn the_tokens_span_the_text() {
    // B.EQ #4 at 0x1000
    let b = decoded(0x54000020);
    let mut text = [0u8; 64];
    let mut tokens = [Token {
        kind: 0,
        operand: Token::NO_OPERAND,
        offset: 0,
        length: 0,
        value: 0,
    }; 16];
    let size = unsafe {
        exarmo_aarch64_instruction_tokens(
            &b,
            0x1000,
            text.as_mut_ptr(),
            text.len(),
            tokens.as_mut_ptr(),
            tokens.len(),
        )
    };
    let written = std::str::from_utf8(&text[..size.length]).unwrap();
    assert_eq!(written, "b.eq\t0x1004");
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
            (0, ".", 0),
            (0, "eq", 0),
            (1, "\t", 0),
            (8, "0x1004", 0x1004)
        ]
    );

    let size = unsafe {
        exarmo_aarch64_instruction_tokens(
            &b,
            0x1000,
            text.as_mut_ptr(),
            text.len(),
            tokens.as_mut_ptr(),
            2,
        )
    };
    assert_eq!(size.tokens, 5);
}

#[test]
fn the_operands_are_laid_out_as_the_header_says() {
    let ldr = decoded(0xF9400420);
    let ops = operands_of(&ldr);
    assert_eq!(ops.len(), 2);
    assert_eq!(ops[0].kind, OperandKind::Reg);
    let reg = unsafe { ops[0].value.reg };
    assert_eq!((reg.reg.class, reg.reg.num), (RegClass::X as u8, 0));
    assert_eq!(reg.index, -1);
    assert!(!reg.modifier.present);
    assert_eq!(ops[1].kind, OperandKind::Mem);
    let mem = unsafe { ops[1].value.mem };
    assert_eq!((mem.base.class, mem.base.num), (RegClass::XSp as u8, 1));
    assert_eq!(mem.offset.kind, OffsetKind::Imm);
    assert_eq!(mem.offset.imm, 8);
    assert_eq!(mem.writeback, Writeback::None);

    // ADD x0, x1, x2, lsl #3
    let add = decoded(0x8B020C20);
    let ops = operands_of(&add);
    assert_eq!(ops.len(), 3);
    let rm = unsafe { ops[2].value.reg };
    assert!(rm.modifier.present);
    assert_eq!(rm.modifier.kind, exarmo_aarch64::ModifierKind::Lsl as u8);
    assert_eq!(rm.modifier.amount, 3);

    // LD1 { v0.16b }, [x1]
    let ld1 = decoded(0x4C407020);
    let ops = operands_of(&ld1);
    assert_eq!(ops[0].kind, OperandKind::List);
    let list = unsafe { ops[0].value.list };
    assert_eq!(list.len, 1);
    assert_eq!(
        (list.regs[0].class, list.regs[0].num),
        (RegClass::V as u8, 0)
    );
    assert_eq!((list.arrangement.element, list.arrangement.lanes), (8, 16));

    let mut one = [COperand::from(exarmo_aarch64::Operand::Other)];
    let n = unsafe { exarmo_aarch64_instruction_operands(&add, one.as_mut_ptr(), 1) };
    assert_eq!(n, 3);
    assert_eq!(one[0].kind, OperandKind::Reg);
    assert!(n <= exarmo_aarch64::Instruction::MAX_OPERANDS);
}

#[test]
fn an_operand_names_a_row_of_the_system_operation_table() {
    let table = &exarmo_aarch64_capi::tables::SYSOPS;
    // at s1e1wp, x10 and two writes of the same PSTATE field.
    let mut rows = Vec::new();
    for (word, name) in [
        (0xD508792Au32, "s1e1wp"),
        (0xD50343DFu32, "daifset"),
        (0xD50345DFu32, "daifset"),
    ] {
        let ops = operands_of(&decoded(word));
        assert!(!ops.is_empty());
        assert_eq!(ops[0].kind as u8, OperandKind::SysOp as u8, "{word:08X}");
        let sysop = unsafe { ops[0].value.sysop };
        let def = &table[sysop.index as usize];
        assert_eq!(text_of(def.name), name, "{word:08X}");
        rows.push((sysop.index, sysop.field));
    }
    // The two PSTATE writes are one row with different field bits, which is
    // the whole reason the row is carried rather than the bits.
    assert_eq!(rows[1].0, rows[2].0);
    assert_ne!(rows[1].1, rows[2].1);
}

/// The five encoding fields come apart, which a consumer numbering the whole
/// system instruction space alongside the registers needs.
#[test]
fn the_system_operations_are_listed() {
    let out = &exarmo_aarch64_capi::tables::SYSOPS;
    let find = |instruction: exarmo_aarch64::Mnemonic, name: &str| {
        out.iter()
            .find(|op| op.instruction == instruction as u32 && text_of(op.name) == name)
            .copied()
            .unwrap_or_else(|| panic!("{instruction:?} names {name}"))
    };

    // at s1e1r is op0 1, op1 0, CRn 7, CRm 8, op2 0, and every bit of CRm
    // names it
    let at = find(exarmo_aarch64::Mnemonic::At, "s1e1r");
    assert_eq!((at.op0, at.op1, at.crn, at.crm, at.op2), (1, 0, 7, 8, 0));
    assert_eq!(at.crm_names, 0xF);

    // gicr's result is written to its register, and tlbip reads a pair as
    // one value
    let cdia = find(exarmo_aarch64::Mnemonic::Gicr, "cdia");
    assert_eq!((cdia.reg_use, cdia.reg_access, cdia.reg_bits), (2, 1, 64));
    let vae1is = find(exarmo_aarch64::Mnemonic::Tlbip, "vae1is");
    assert_eq!(
        (vae1is.reg_use, vae1is.reg_access, vae1is.reg_bits),
        (2, 0, 128)
    );
    let ialluis = find(exarmo_aarch64::Mnemonic::Ic, "ialluis");
    assert_eq!((ialluis.reg_use, ialluis.reg_bits), (0, 0));

    // msr daifset takes its immediate in CRm, so none of it names the field
    let daifset = find(exarmo_aarch64::Mnemonic::Msr, "daifset");
    assert_eq!(
        (daifset.op0, daifset.op1, daifset.crn, daifset.op2),
        (0, 3, 4, 6)
    );
    assert_eq!(daifset.crm_names, 0);

    // The table is what the Rust side names, in the same order.
    for (packed, op) in out.iter().zip(exarmo_aarch64::SysOp::all()) {
        assert_eq!(
            (packed.encoding, text_of(packed.name)),
            (op.encoding, op.name)
        );
    }
}

#[test]
fn the_system_registers_are_listed() {
    let defs = &exarmo_aarch64_capi::tables::SYSREGS;
    assert!(defs.len() > 1000);
    let nzcv = defs.iter().find(|d| text_of(d.name) == "nzcv").unwrap();
    assert_eq!(nzcv.encoding, 0xda10);
    assert!(nzcv.readable && nzcv.writable);
    let mut buf = [0u8; 64];
    for def in defs {
        let n = unsafe {
            exarmo_aarch64_sysreg_name(def.encoding, def.writable, buf.as_mut_ptr(), buf.len())
        };
        assert_eq!(&buf[..n], text_of(def.name).as_bytes());
    }
}

#[test]
fn the_header_agrees_with_the_enums() {
    let header = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/include/exarmo/aarch64_generated.h"
    ))
    .expect("the generator wrote include/exarmo/aarch64_generated.h");
    let mut encodings = 0;
    let mut mnemonics = 0;
    let mut modifier_kinds: Vec<(usize, String)> = Vec::new();
    let mut intrinsic_types = 0;
    let mut conditions = 0;
    let mut conditions_counted = None;
    for (name, value) in exarmo_testing::capi::header_numbers(&header) {
        let name = name.as_str();
        if let Some(variant) = name.strip_prefix("EXARMO_AARCH64_ENC_") {
            assert_eq!(
                exarmo_aarch64::Encoding::from_index(value).unwrap().name(),
                variant
            );
            encodings += 1;
        } else if name == "EXARMO_AARCH64_ENCODING_COUNT" {
            assert_eq!(value, exarmo_aarch64::Encoding::COUNT);
        } else if let Some(variant) = name.strip_prefix("EXARMO_AARCH64_INTRINSIC_TYPE_") {
            // The kinds share the prefix, and only the types are numbered here
            if let Some(ty) = exarmo_aarch64::intrinsics::TYPES.get(value) {
                let expected: String = ty
                    .name
                    .replace(" const *", "_CONST_PTR")
                    .replace(" *", "_PTR")
                    .replace("_t", "")
                    .replace(' ', "_")
                    .to_uppercase();
                if variant == expected {
                    intrinsic_types += 1;
                }
            }
        } else if name == "EXARMO_AARCH64_INTRINSIC_TYPE_COUNT" {
            assert_eq!(value, exarmo_aarch64::intrinsics::TYPES.len());
        } else if name == "EXARMO_AARCH64_INTRINSIC_COUNT" {
            assert_eq!(value, exarmo_aarch64::intrinsics::DEFS.len());
        } else if name == "EXARMO_AARCH64_MNEMONIC_COUNT" {
            assert_eq!(value, exarmo_aarch64::Mnemonic::COUNT);
        } else if name == "EXARMO_AARCH64_SYSREG_COUNT" {
            assert_eq!(value, exarmo_aarch64_capi::tables::SYSREGS.len());
        } else if name == "EXARMO_AARCH64_SYSOP_COUNT" {
            assert_eq!(value, exarmo_aarch64_capi::tables::SYSOPS.len());
        } else if name == "EXARMO_AARCH64_SYSOP_INSTRUCTION_COUNT" {
            // The instructions naming an operation, which is what a consumer
            // laying out a range per instruction sizes itself on. A PSTATE
            // field is written by MSR and is not one of them.
            let naming: std::collections::HashSet<exarmo_aarch64::Mnemonic> =
                exarmo_aarch64::SysOp::all()
                    .filter(|op| op.op0 == 1)
                    .map(|op| op.instruction)
                    .collect();
            assert_eq!(value, naming.len());
        } else if name == "EXARMO_AARCH64_MAX_OPERANDS" {
            assert_eq!(value, exarmo_aarch64::Instruction::MAX_OPERANDS);
        } else if name == "EXARMO_AARCH64_MAX_TOKENS" {
            assert_eq!(value, exarmo_aarch64::Instruction::MAX_TOKENS);
        } else if name == "EXARMO_AARCH64_MAX_TEXT" {
            assert_eq!(value, exarmo_aarch64::Instruction::MAX_TEXT);
        } else if name == "EXARMO_AARCH64_MAX_INTRINSIC_ARGUMENTS" {
            assert_eq!(value, exarmo_aarch64_capi::model::MAX_INTRINSIC_ARGUMENTS);
        } else if name == "EXARMO_AARCH64_MOD_RESERVED" {
            assert_eq!(value, exarmo_aarch64::ModifierKind::Reserved as usize);
        } else if name == "EXARMO_AARCH64_MOD_COUNT" {
            assert_eq!(value, exarmo_aarch64::ModifierKind::Reserved as usize + 1);
        } else if let Some(kind) = name.strip_prefix("EXARMO_AARCH64_MOD_") {
            modifier_kinds.push((value, kind.to_lowercase()));
        } else if name == "EXARMO_AARCH64_COND_COUNT" {
            conditions_counted = Some(value);
        } else if let Some(cond) = name.strip_prefix("EXARMO_AARCH64_COND_") {
            // C numbers the conditions by the Rust enum's discriminants, so
            // a value read from an operand means the same on both sides. This
            // checks `as u8` rather than `bits`, because an alias reading the
            // condition field inverted holds bits that spell another
            // condition.
            let named = exarmo_aarch64::Cond::from_bits(value as u8)
                .filter(|c| *c as usize == value)
                .unwrap_or_else(|| panic!("{name} is numbered {value}"));
            assert_eq!(named.name(), cond.to_lowercase());
            conditions += 1;
        } else if let Some(mnemonic) = name.strip_prefix("EXARMO_AARCH64_") {
            assert_eq!(
                exarmo_aarch64::Mnemonic::from_index(value).unwrap().name(),
                mnemonic.to_lowercase()
            );
            mnemonics += 1;
        }
    }
    assert_eq!(encodings, exarmo_aarch64::Encoding::COUNT);
    assert_eq!(mnemonics, exarmo_aarch64::Mnemonic::COUNT);
    // The modifier kinds are numbered from zero up to Reserved, and named alike
    assert_eq!(
        modifier_kinds.len(),
        exarmo_aarch64::ModifierKind::Reserved as usize
    );
    assert!(modifier_kinds.contains(&(
        exarmo_aarch64::ModifierKind::Lsl as usize,
        "lsl".to_string()
    )));
    assert!(modifier_kinds.contains(&(
        exarmo_aarch64::ModifierKind::Uxtw as usize,
        "uxtw".to_string()
    )));
    assert_eq!(intrinsic_types, exarmo_aarch64::intrinsics::TYPES.len());
    assert_eq!(conditions_counted, Some(conditions));
    assert!(conditions >= 16, "the architecture writes sixteen");
}
