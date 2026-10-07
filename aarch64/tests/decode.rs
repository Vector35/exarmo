//! Which encoding a word is, and the fields it carries.
//!
//! One case per decision the generator makes, so that a change to any of
//! them fails here by name rather than somewhere in the 63,000 corpus
//! lines. Each word was checked against llvm-mc by hand.

// `Shift_Lsl0_Lsl12` is the left shift an ADD immediate takes. The
// architecture names no value-table enum, so each is named after what it
// holds.
use exarmo_aarch64::{
    Arrangement, ElementWidth, Encoding, Instruction, Shift_Lsl0_Lsl12, decode_word,
};

/// The encoding a word decodes to, for each shape of instruction.
#[test]
fn a_word_decodes_to_its_encoding() {
    let cases: &[(u32, Encoding, &str)] = &[
        (0xF9400420, Encoding::Ldr64LdstPos, "ldr x0, [x1, #8]"),
        (0x91000420, Encoding::Add64AddsubImm, "add x0, x1, #1"),
        (0x14000002, Encoding::BOnlyBranchImm, "b .+8"),
        (0x54000020, Encoding::BOnlyCondbranch, "b.eq .+4"),
        (0x90000000, Encoding::AdrpOnlyPcreladdr, "adrp x0, 0"),
        (0xB2400400, Encoding::Orr64LogImm, "orr x0, x0, #3"),
        (
            0xD5330000,
            Encoding::MrsRsSystemmove,
            "mrs x0, s2_3_c0_c0_0",
        ),
        (0x4E0C0420, Encoding::DupAsimdinsDvV, "dup v0.4s, v1.s[1]"),
        (0x9AC02C20, Encoding::RorRorv64Dp2src, "ror x0, x1, x0"),
        // An alias is a form of the encoding it spells, and has an
        // `Encoding` of its own.
        (0xD2800020, Encoding::MovMovz64Movewide, "mov x0, #1"),
        (0xAA0203E0, Encoding::MovOrr64LogShift, "mov x0, x2"),
        (0xD503201F, Encoding::NopHiHints, "nop"),
    ];
    for (word, encoding, disassembly) in cases {
        assert_eq!(
            decode_word(*word).map(|i| i.encoding()),
            Ok(*encoding),
            "{word:08x} is {disassembly}"
        );
    }
}

/// ORR with ZR as the first source is MOV, and with any other register it
/// is ORR.
#[test]
fn an_alias_is_taken_only_where_its_condition_holds() {
    // orr x0, xzr, x2, which is mov x0, x2
    let Ok(Instruction::MovOrr64LogShift { Xd, Xm }) = decode_word(0xAA0203E0) else {
        panic!("mov");
    };
    assert_eq!((Xd.num(), Xm.num()), (0, 2));
    // orr x0, x1, x2, whose first source is not ZR
    assert_eq!(
        decode_word(0xAA020020).map(|i| i.encoding()),
        Ok(Encoding::Orr64LogShift)
    );
}

/// An immediate the decode reads as two's complement is held signed, and
/// keeps its sign.
#[test]
fn a_signed_immediate_is_held_signed() {
    // stur x0, [x1, #-16]
    let Ok(Instruction::Stur64LdstUnscaled { Xt, Xn, simm }) = decode_word(0xF81F0020) else {
        panic!("stur");
    };
    assert_eq!((Xt.num(), Xn.num(), simm), (0, 1, -16));
}

/// A bitmask immediate is the value DecodeBitMasks makes of its fields,
/// not the fields.
#[test]
fn a_bitmask_is_the_value_it_decodes_to() {
    // orr x0, x0, #3, whose N:immr:imms is 0:000000:000001
    let Ok(Instruction::Orr64LogImm { imm, .. }) = decode_word(0xB2400400) else {
        panic!("orr");
    };
    assert_eq!(imm, 3);
}

/// A label is held as the offset from the PC. The rendering works out the
/// address it names, given one.
#[test]
fn a_label_is_held_as_its_offset() {
    let Ok(Instruction::BOnlyBranchImm { offset }) = decode_word(0x14000002) else {
        panic!("b");
    };
    assert_eq!(offset, 8);
    assert_eq!(
        decode_word(0x14000002).unwrap().at(0x1000).to_string(),
        "b\t0x1008"
    );
}

/// An element index is the number the assembly writes, and the arrangement
/// beside it says what a lane is.
#[test]
fn a_lane_is_its_arrangement_and_its_index() {
    let Ok(Instruction::DupAsimdinsDvV {
        Vd,
        T,
        Vn,
        Ts,
        index,
    }) = decode_word(0x4E0C0420)
    else {
        panic!("dup");
    };
    assert_eq!((Vd.num(), Vn.num()), (0, 1));
    assert_eq!(T, Arrangement::vector(4, ElementWidth::S));
    assert_eq!(Ts, Arrangement::element(ElementWidth::S));
    assert_eq!(index, 1);
}

/// A system register is held as its encoding, which the name lookup reads.
#[test]
fn a_system_register_is_held_as_its_encoding() {
    let Ok(Instruction::MrsRsSystemmove { Xt, systemreg, .. }) = decode_word(0xD5330000) else {
        panic!("mrs");
    };
    assert_eq!(Xt.num(), 0);
    assert!(!systemreg.is_write());
    // The architecture names none at this encoding, so it is spelt by it.
    assert_eq!(systemreg.to_string(), "s2_3_c0_c0_0");
}

/// A shift an encoding chooses between is held as the enum of what it
/// chose, not as the bits that chose it.
#[test]
fn a_shift_is_held_as_what_it_selects() {
    let Ok(Instruction::Add64AddsubImm { shift, imm, .. }) = decode_word(0x91000420) else {
        panic!("add");
    };
    assert_eq!((shift, imm), (Shift_Lsl0_Lsl12::Lsl0, 1));
    // add x0, x1, #1, lsl #12
    let Ok(Instruction::Add64AddsubImm { shift, .. }) = decode_word(0x91400420) else {
        panic!("add lsl 12");
    };
    assert_eq!(shift, Shift_Lsl0_Lsl12::Lsl12);
}

/// A 32-bit form and a 64-bit one are different encodings, and the
/// register type says which.
#[test]
fn the_register_width_is_in_the_type() {
    assert!(matches!(
        decode_word(0x11000420),
        Ok(Instruction::Add32AddsubImm { .. })
    ));
    let Ok(Instruction::Add32AddsubImm { Wn, .. }) = decode_word(0x11000420) else {
        panic!("add w");
    };
    assert_eq!(Wn.num(), 1);
}

/// A word the architecture allocates to nothing says so, rather than
/// decoding to something near it.
#[test]
fn an_unallocated_word_says_so() {
    assert!(decode_word(0xC0200400).is_err());
    assert!(decode_word(0x00010005).is_err());
}
