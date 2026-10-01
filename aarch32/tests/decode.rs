//! Which encoding a word is, and the fields it carries, each case checked
//! against what llvm-mc disassembles it to.

// The architecture names none of the value-table enums, so each is named
// after the symbols it holds. Which shifts the field can spell is what these
// cases are about, so the name is not shortened.
use exarmo_aarch32::{
    Cond, DReg, DecodeError, Encoding, GpReg, Instruction, ItState, QReg, SReg,
    Shift_Lsl_Lsr_Asr_Ror, a32, t32,
};

#[test]
fn a32_words_decode_to_their_encoding() {
    let cases: &[(u32, Encoding, &str)] = &[
        (0xe2800001, Encoding::AddIA1, "add r0, r0, #1"),
        (0xe0800001, Encoding::AddRA1, "add r0, r0, r1"),
        (0xe5901000, Encoding::LdrIA1Off, "ldr r1, [r0]"),
        // PUSH is the alias of STMDB with writeback onto the stack pointer,
        // and an alias is a form of the encoding it spells.
        (0xe92d4010, Encoding::PushStmdbA1, "push {r4, lr}"),
        (0xeb000000, Encoding::BlIA1, "bl"),
        (0x0a000000, Encoding::BA1, "beq"),
        (0xee070f9a, Encoding::McrA1, "mcr p15, 0, r0, c7, c10, 4"),
    ];
    for (word, encoding, disassembly) in cases {
        assert_eq!(
            a32::decode(*word).map(|i| i.encoding()),
            Ok(*encoding),
            "{word:08x} is {disassembly}"
        );
    }
}

/// A 16-bit T32 instruction is the high halfword of the word, which is how
/// ARM's own hierarchy numbers it before it narrows.
#[test]
fn halfword_t32_instructions_decode_to_their_encoding() {
    let cases: &[(u16, Encoding, &str)] = &[
        (0x4408, Encoding::AddRT2, "add r0, r1"),
        (0x4608, Encoding::MovRT1, "mov r0, r1"),
        // An unknown IT state reads as outside a block, so this is the MOVS form.
        (0x2001, Encoding::T1bMovIT1, "movs r0, #1"),
        // The one 16-bit encoding the hierarchy reaches without a group.
        (0xe000, Encoding::BT2, "b .+4"),
        (0xbf00, Encoding::NopT1, "nop"),
        (0xb580, Encoding::PushT1, "push {r7, lr}"),
    ];
    for (halfword, encoding, disassembly) in cases {
        let word = u32::from(*halfword) << 16;
        assert_eq!(
            t32::decode(word, ItState::Unknown).map(|i| i.encoding()),
            Ok(*encoding),
            "{halfword:04x} is {disassembly}"
        );
    }
}

/// A 32-bit T32 instruction is both halfwords, the first above the second.
#[test]
fn wide_t32_instructions_decode_to_their_encoding() {
    let cases: &[(u32, Encoding, &str)] = &[
        // Written MOVW, the second template of MOV_i_T3.
        (0xf2400000, Encoding::T3bMovIT3, "movw r0, #0"),
        (0xf8d00000, Encoding::LdrIT3, "ldr.w r0, [r0]"),
        (0xe92d4800, Encoding::PushStmdbT1, "push.w {r11, lr}"),
    ];
    for (word, encoding, disassembly) in cases {
        assert_eq!(
            t32::decode(*word, ItState::Unknown).map(|i| i.encoding()),
            Ok(*encoding),
            "{word:08x} is {disassembly}"
        );
    }
}

#[test]
fn an_unallocated_word_says_so() {
    // The unconditional miscellaneous space at op0 = 10010, op1 = 0000.
    assert_eq!(a32::decode(0xf1200000), Err(DecodeError::Unallocated));
    // The A32 hint space, hint = 0x20, which llvm-mc writes as `hint #32`.
    assert_eq!(a32::decode(0xe320f020), Err(DecodeError::ReservedHint));
    // The signed literal loads at size 01 with Rt = 1111, beside PLI.
    assert_eq!(
        t32::decode(0xf9bf_f000, ItState::Outside),
        Err(DecodeError::ReservedHint)
    );
    // The same space at op1 = 0111, and the unconditional hints at op0 =
    // 1xxx1 with bit 4 clear, which the index marks UNPREDICTABLE.
    assert_eq!(a32::decode(0xf1200070), Err(DecodeError::Unpredictable));
    assert_eq!(a32::decode(0xf7f0a000), Err(DecodeError::Unpredictable));
    // A hint row the table does allocate is still the instruction.
    assert_eq!(
        a32::decode(0xe320f010).map(|i| i.encoding()),
        Ok(Encoding::EsbA1)
    );
}

#[test]
fn an_encoding_carries_the_fields_the_model_reads() {
    // ADD (register) A1: ADD{<c>}{<q>} {<Rd>, }<Rn>, <Rm>{, <shift>}
    // e0812003 is `add r2, r1, r3`.
    assert_eq!(
        a32::decode(0xe0812003),
        Ok(Instruction::AddRA1 {
            cond: Cond::Al,
            Rd: GpReg::new(2),
            Rn: GpReg::new(1),
            Rm: GpReg::new(3),
            shift: Shift_Lsl_Lsr_Asr_Ror::Lsl,
            amount: 0,
        })
    );
    // LDR (immediate) A1, offset form, `ldr r1, [r0]`
    assert_eq!(
        a32::decode(0xe5901000),
        Ok(Instruction::LdrIA1Off {
            cond: Cond::Al,
            Rt: GpReg::new(1),
            Rn: GpReg::new(0),
            add: true,
            imm: 0,
        })
    );
}

#[test]
fn a32_carries_the_condition_its_field_holds() {
    // 0a000000 is `beq`, 2a000000 is `bcs`, ea000000 is `b`.
    let cond = |word: u32| match a32::decode(word) {
        Ok(Instruction::BA1 { cond, .. }) => cond,
        other => panic!("{word:08x} decoded to {other:?}"),
    };
    assert_eq!(cond(0x0a000000), Cond::Eq);
    assert_eq!(cond(0x2a000000), Cond::Cs);
    assert_eq!(cond(0xea000000), Cond::Al);
    // CS and CC, as the architecture spells them, not HS and LO.
    assert_eq!(Cond::Cs.to_string(), "cs");
    assert_eq!(Cond::Cc.to_string(), "cc");
    // 1111 names no condition. The hierarchy sends it elsewhere.
    assert_eq!(Cond::from_bits(0b1111), None);
    assert_eq!(Cond::from_bits(0b1110), Some(Cond::Al));
}

/// A T32 instruction takes its condition from the IT state, because no field
/// of a 16-bit encoding holds one.
#[test]
fn t32_takes_its_condition_from_the_it_state() {
    // 0x4408 is `add r0, r1`, whose T2 encoding has no condition field.
    let word = 0x4408_u32 << 16;
    let cond = |state| match t32::decode(word, state) {
        Ok(Instruction::AddRT2 { cond, .. }) => cond,
        other => panic!("decoded to {other:?}"),
    };
    assert_eq!(cond(ItState::Outside), Cond::Al);
    assert_eq!(
        cond(ItState::Inside {
            cond: Cond::Eq,
            mask: 0b1000
        }),
        Cond::Eq
    );
    // A caller that has not tracked the blocks reads as being outside one,
    // which is what Binary Ninja's own plugin does with IFTHEN_UNKNOWN.
    assert_eq!(cond(ItState::Unknown), Cond::Al);
}

#[test]
fn the_it_state_answers_what_the_pseudocode_asks() {
    let inside_last = ItState::Inside {
        cond: Cond::Ne,
        mask: 0b1000,
    };
    let inside_more = ItState::Inside {
        cond: Cond::Ne,
        mask: 0b0100,
    };
    assert!(inside_last.in_block() && inside_last.last_in_block());
    assert!(inside_more.in_block() && !inside_more.last_in_block());
    for outside in [ItState::Outside, ItState::Unknown] {
        assert!(!outside.in_block() && !outside.last_in_block());
        assert_eq!(outside.condition(), Cond::Al);
    }
}

/// The state advances as the architecture's ITAdvance has it. `itett eq`
/// runs the next four instructions under EQ, NE, EQ and EQ, and the block
/// ends after them.
#[test]
fn the_it_state_advances_through_the_block_it_began() {
    let it = t32::decode(0xbf09_0000, ItState::Outside).unwrap();
    assert_eq!(it.at(0).to_string(), "itett\teq");
    let add = t32::decode(0x4408_0000, ItState::Outside).unwrap();
    assert_eq!(add.it_state_set(), None);
    let block = [
        (Cond::Eq, 0b1001),
        (Cond::Ne, 0b0010),
        (Cond::Eq, 0b0100),
        (Cond::Eq, 0b1000),
    ];
    let mut state = ItState::Outside.after(&it);
    for (cond, mask) in block {
        assert_eq!(state, ItState::Inside { cond, mask });
        assert_eq!(state.condition(), cond);
        state = state.after(&add);
    }
    assert_eq!(state, ItState::Outside);
    // A caller that has not tracked the blocks learns nothing from an
    // instruction that is not IT, and everything from one that is.
    assert_eq!(ItState::Unknown.after(&add), ItState::Unknown);
    assert_eq!(
        ItState::Unknown.after(&it),
        ItState::Inside {
            cond: Cond::Eq,
            mask: 0b1001
        }
    );
    // The bits are ITSTATE's.
    assert_eq!(ItState::from_bits(0x09).bits(), 0x09);
    assert_eq!(ItState::from_bits(0xe0), ItState::Outside);
}

/// The top split of the hierarchy decides the length. A halfword below the
/// 32-bit prefixes begins a 16-bit instruction, as does the unconditional
/// branch that sits among them.
#[test]
fn a_halfword_says_how_long_its_instruction_is() {
    assert_eq!(t32::length(0x4408), 2, "add r0, r1");
    assert_eq!(t32::length(0xbf09), 2, "itett eq");
    assert_eq!(t32::length(0xe000), 2, "b");
    assert_eq!(t32::length(0xe800), 4, "stm.w");
    assert_eq!(t32::length(0xf000), 4, "and.w");
    assert_eq!(t32::length(0xf800), 4, "strb.w");
    assert_eq!(t32::length(0xffff), 4);
}

/// The length answers what an assembler's `<q>` would have said, which the
/// text never writes.
#[test]
fn an_encoding_knows_how_long_it_is() {
    // A 16-bit T32 encoding, its 32-bit sibling, and an A32 word.
    assert_eq!(Encoding::AddRT2.length(), 2);
    assert_eq!(Encoding::AddRT3.length(), 4);
    assert_eq!(Encoding::AddRA1.length(), 4);
    assert_eq!(Encoding::BA1.length(), 4);
    // `push {r7, lr}` is narrow, `push.w {r11, lr}` is wide.
    assert_eq!(Encoding::PushT1.length(), 2);
    assert_eq!(Encoding::StmdbT1.length(), 4);
}

/// A SIMD register's number is split across the word. ARM puts its top bit
/// elsewhere and writes the whole as `(D :: Vd)`. A Q register's field holds
/// twice its number.
#[test]
fn a_simd_register_is_gathered_from_the_bits_that_hold_it() {
    // vsub.f64 d16, d1, d0. Only the D bit puts the destination above d15.
    assert_eq!(
        a32::decode(0xee710b40),
        Ok(Instruction::VsubFA2D {
            cond: Cond::Al,
            Dd: DReg::new(16),
            Dn: DReg::new(1),
            Dm: DReg::new(0),
        })
    );
    // vadd.f32 s0, s1, s0, whose single-precision names are written `Vd:D`,
    // with the odd bit at the bottom rather than the top.
    assert_eq!(
        a32::decode(0xee300a80),
        Ok(Instruction::VaddFA2S {
            cond: Cond::Al,
            Sd: SReg::new(0),
            Sn: SReg::new(1),
            Sm: SReg::new(0),
        })
    );
    // vadd.f32 q0, q0, q1. Advanced SIMD is in the unconditional space, so no
    // condition, and q1 is the field holding 2.
    match a32::decode(0xf2000d42) {
        Ok(Instruction::VaddFA1Q { dt, Qd, Qn, Qm }) => {
            assert_eq!(dt.to_string(), "f32");
            assert_eq!((Qd, Qn, Qm), (QReg::new(0), QReg::new(0), QReg::new(1)));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_tabulated_operand_carries_the_symbol_it_spells() {
    // and r0, r0, r1, lsl #1. The shift type is a table of four names. The
    // amount beside it is a number and not one of these.
    assert_eq!(
        a32::decode(0xe0000081),
        Ok(Instruction::AndRA1 {
            cond: Cond::Al,
            Rd: GpReg::new(0),
            Rn: GpReg::new(0),
            Rm: GpReg::new(1),
            shift: Shift_Lsl_Lsr_Asr_Ror::Lsl,
            amount: 1,
        })
    );
    assert_eq!(Shift_Lsl_Lsr_Asr_Ror::Lsl.to_string(), "lsl");
    assert_eq!(Shift_Lsl_Lsr_Asr_Ror::Ror.to_string(), "ror");
    // vqdmulh.s32 d0, d2, d2, whose datatype is one of its own table's.
    match a32::decode(0xf2220b02) {
        Ok(Instruction::VqdmulhA1D { dt, Dd, Dn, Dm }) => {
            assert_eq!(dt.to_string(), "s32");
            assert_eq!((Dd, Dn, Dm), (DReg::new(0), DReg::new(2), DReg::new(2)));
        }
        other => panic!("{other:?}"),
    }
}

/// A table whose rows are numbers says what number the field spells, and a row
/// spelling nothing says the operand is not written for that value.
#[test]
fn a_tabulated_number_is_the_number_not_the_bits() {
    let rotation = |word: u32| match a32::decode(word) {
        Ok(Instruction::SxtbA1 { amount, .. }) => amount,
        other => panic!("{word:08x} decoded to {other:?}"),
    };
    // sxtb r0, r0, with no rotation written.
    assert_eq!(rotation(0xe6af0070), None);
    // sxtb r0, r0, ror #8 and ror #16. The field holds 1 and 2.
    assert_eq!(rotation(0xe6af0470), Some(8));
    assert_eq!(rotation(0xe6af0870), Some(16));
}

/// A quad register is half of what its field spells, and the halving applies to
/// the whole of a concatenation rather than to its last part.
///
/// `D:Vd` of `1:0000` is 16, and the register is q8. Halving only `Vd` would
/// name q16, which no register file has.
#[test]
fn a_quad_register_halves_the_whole_concatenation() {
    let quad = |word: u32| match a32::decode(word) {
        Ok(Instruction::VuzpA1Q { Qd, .. }) => Qd,
        other => panic!("{word:08x} decoded to {other:?}"),
    };
    // vuzp.8 q0, q0 and vuzp.8 q8, q0, which differ only in the D bit.
    assert_eq!(quad(0xf3b20140), QReg::new(0));
    assert_eq!(quad(0xf3f20140), QReg::new(8));
}

/// The bundle writes `<align>`'s values, 64, 128 or 256, as a list rather than
/// a table. The value meaning none is written is the one the prose states
/// around the list.
#[test]
fn an_alignment_comes_from_the_list_of_its_values() {
    let alignment = |word: u32| match a32::decode(word) {
        Ok(Instruction::Vld1MA4Nowb { align, .. }) => align,
        other => panic!("{word:08x} decoded to {other:?}"),
    };
    // vld1.8 {d0,d1,d2,d3}, [r0] and the same with :64, per llvm-mc.
    assert_eq!(alignment(0xf420020f), None);
    assert_eq!(alignment(0xf420021f), Some(64));
}

/// Every alignment the field spells, over the encoding that permits all four.
#[test]
fn every_alignment_a_field_spells_is_read() {
    let alignment = |word: u32| match a32::decode(word) {
        Ok(Instruction::Vld4MA1Nowb { align, .. }) => align,
        other => panic!("{word:08x} decoded to {other:?}"),
    };
    // vld4.8 {d0,d1,d2,d3}, [r0], then :64, :128 and :256, per llvm-mc.
    assert_eq!(alignment(0xf420000f), None);
    assert_eq!(alignment(0xf420001f), Some(64));
    assert_eq!(alignment(0xf420002f), Some(128));
    assert_eq!(alignment(0xf420003f), Some(256));
}

/// An operand whose prose says only "an immediate value. See Modified immediate
/// constants" is the value the decode computes. That is `A32ExpandImm` of the
/// field, in the 32 bits the decode holds it rather than the 12 the field has.
#[test]
fn a_modified_immediate_is_what_the_decode_expands_it_to() {
    let expanded = |word: u32| match a32::decode(word) {
        Ok(Instruction::AdcIA1 { const_, .. }) => const_,
        other => panic!("{word:08x} decoded to {other:?}"),
    };
    // adc r0, r1, #0xff, #0xff000000 and #1020, per llvm-mc. The rotation in
    // the top four bits of imm12 is what makes one field spell all three.
    assert_eq!(expanded(0xe2a100ff), 0xff);
    assert_eq!(expanded(0xe2a104ff), 0xff00_0000);
    assert_eq!(expanded(0xe2a10fff), 1020);
}

/// An operand another operand's value decides is read arm by arm. `<align>` is
/// a slice of index_align whose width and meaning both depend on `<size>`, and
/// the deciding value is matched as the field spells it rather than as the
/// assembler writes it.
#[test]
fn an_alignment_another_operand_decides_is_read_per_size() {
    // The data size picks the encoding as well as the alignment, so each is its
    // own variant.
    let aligned = |word: u32| match a32::decode(word) {
        Ok(Instruction::Vst21A1Nowb { size, align, .. })
        | Ok(Instruction::Vst21A2Nowb { size, align, .. })
        | Ok(Instruction::Vst21A3Nowb { size, align, .. }) => (size, align),
        other => panic!("{word:08x} decoded to {other:?}"),
    };
    // vst2.8 {d0[0], d1[0]}, [r0] and [r0:16], then .16 with :32 and .32 with
    // :64, per llvm-mc. One index_align bit carries the alignment at size 8 and
    // 16, and two carry it at 32.
    assert_eq!(aligned(0xf480010f), (8, None));
    assert_eq!(aligned(0xf480011f), (8, Some(16)));
    assert_eq!(aligned(0xf480051f), (16, Some(32)));
    assert_eq!(aligned(0xf480091f), (32, Some(64)));
}

/// The shift amount is the decode's `shift_n`, which reads the shift type
/// beside the field.
#[test]
fn a_shift_amount_is_decoded_with_the_shift_type() {
    let shifted = |word: u32| match a32::decode(word) {
        Ok(Instruction::AdcRA1 { shift, amount, .. }) => (shift, amount),
        other => panic!("{word:08x} decoded to {other:?}"),
    };
    // adc r0, r1, r2, lsl #3, then lsr #32, asr #32 and lsr #1, per llvm-mc.
    // The two #32 forms hold a zero in imm5, which only the shift type tells
    // apart from no shift at all.
    assert_eq!(shifted(0xe0a10182), (Shift_Lsl_Lsr_Asr_Ror::Lsl, 3));
    assert_eq!(shifted(0xe0a10022), (Shift_Lsl_Lsr_Asr_Ror::Lsr, 32));
    assert_eq!(shifted(0xe0a10042), (Shift_Lsl_Lsr_Asr_Ror::Asr, 32));
    assert_eq!(shifted(0xe0a100a2), (Shift_Lsl_Lsr_Asr_Ror::Lsr, 1));
}

/// The `!` beside a register list is the writeback bit the prose names.
#[test]
fn writeback_is_the_bit_the_prose_names() {
    let written_back = |word: u32| match t32::decode(word, ItState::Outside) {
        Ok(Instruction::LdmT2 { wback, .. }) => wback,
        other => panic!("{word:08x} decoded to {other:?}"),
    };
    // ldm.w r0, {r1, r3} and ldm.w r0!, {r1, r3}, per llvm-mc. The ! sets W.
    assert!(!written_back(0xe890000a));
    assert!(written_back(0xe8b0000a));
}

#[test]
fn the_view_gives_the_operands_in_order() {
    use exarmo_aarch32::operands::{Operand, Reg};

    // adc r0, r1, #0xff. The condition, which the template writes inside the
    // mnemonic, is the view's first operand.
    let Ok(instruction) = a32::decode(0xe2a100ff) else {
        panic!("adc should decode");
    };
    let operands = instruction.operands();
    assert_eq!(operands.len(), 4);
    assert!(matches!(operands[0], Operand::Cond(Cond::Al)));
    assert!(matches!(
        operands[1],
        Operand::Reg(r) if r.reg == Reg::Core(GpReg::new(0))
    ));
    assert!(matches!(
        operands[2],
        Operand::Reg(r) if r.reg == Reg::Core(GpReg::new(1))
    ));
    assert!(matches!(operands[3], Operand::Imm { value: 0xff, .. }));
}

/// A modified immediate is what the instruction expands the field to, which the
/// Execute pseudocode says and the decode does not.
#[test]
fn a_modified_immediate_is_expanded_where_only_execute_says_so() {
    let written = |word: u32| {
        let mut text = String::new();
        a32::decode(word)
            .expect("it should decode")
            .at(0)
            .write_tokens(&mut text)
            .expect("it should be written");
        text
    };
    // tst r10, #0x4000002d and eor r12, r12, #0xf4000, per llvm-mc. TST's decode
    // is `let imm : bits(12) = imm12`, so the twelve bits alone would be wrong.
    assert_eq!(written(0x531ae1b5), "tstpl\tr10, #0x4000002d");
    assert_eq!(written(0xd22cc93d), "eorle\tr12, r12, #0xf4000");
}

/// A negative offset is written as one signed number.
#[test]
fn a_memory_operand_is_written_with_its_offset_and_writeback() {
    let written = |word: u32| {
        let mut text = String::new();
        a32::decode(word)
            .expect("it should decode")
            .at(0)
            .write_tokens(&mut text)
            .expect("it should be written");
        text
    };
    // ldr r0, [r1, #4], then [r1, #-4] and [r1, #4]!, per llvm-mc.
    assert_eq!(written(0xe5910004), "ldr\tr0, [r1, #0x4]");
    assert_eq!(written(0xe5110004), "ldr\tr0, [r1, #-0x4]");
    assert_eq!(written(0xe5b10004), "ldr\tr0, [r1, #0x4]!");
}

#[test]
fn an_optional_shift_is_written_only_when_there_is_one() {
    let written = |word: u32| {
        let mut text = String::new();
        a32::decode(word)
            .expect("it should decode")
            .at(0)
            .write_tokens(&mut text)
            .expect("it should be written");
        text
    };
    // add r2, r1, r3 and adc r0, r1, r2, lsl #3, per llvm-mc.
    assert_eq!(written(0xe0812003), "add\tr2, r1, r3");
    assert_eq!(written(0xe0a10182), "adc\tr0, r1, r2, lsl #0x3");
}

/// A wide ADD with a 12-bit immediate is written ADDW, as an assembler takes
/// it for every value, and a 16-bit MOV with an immediate MOVW.
#[test]
fn the_wide_immediate_forms_are_written_with_their_w() {
    assert_eq!(
        t32::decode(0xf2000000, ItState::Outside)
            .unwrap()
            .mnemonic()
            .name(),
        "addw"
    );
    assert_eq!(
        t32::decode(0xf2400000, ItState::Outside)
            .unwrap()
            .mnemonic()
            .name(),
        "movw"
    );
    assert_eq!(a32::decode(0xe3000000).unwrap().mnemonic().name(), "movw");
}

/// An alias is a form of the encoding it spells, written where the bundle
/// prefers it, and named after the alias encoding.
#[test]
fn an_alias_is_written_where_the_architecture_prefers_it() {
    // MOV (register) with a shift is written as the shift. e1a01102 is
    // `lsl r1, r2, #2`, e1b01102 `lsls`, and without a shift `mov r0, r0`.
    let lsl = a32::decode(0xe1a01102).unwrap();
    assert_eq!(lsl.encoding().name(), "LslMovRA1");
    assert_eq!(lsl.at(0).to_string(), "lsl\tr1, r2, #0x2");
    assert_eq!(a32::decode(0xe1b01102).unwrap().mnemonic().name(), "lsls");
    assert_eq!(a32::decode(0xe1a00000).unwrap().mnemonic().name(), "mov");
    // The rotate through the carry, which is the alias of a MOV with a
    // rotate by zero.
    assert_eq!(
        a32::decode(0xe1a01062).unwrap().at(0).to_string(),
        "rrx\tr1, r2"
    );
    // VAND with an immediate is never preferred over VBIC, the bundle says.
    assert_eq!(a32::decode(0xf2800130).unwrap().mnemonic().name(), "vbic");
}

/// A label is written as the address it names. That is the offset from the
/// PC as the instruction reads it, 8 bytes ahead in A32 and 4 in T32, aligned
/// down to a word first for a literal load or ADR.
#[test]
fn a_label_is_the_address_it_names() {
    use exarmo_aarch32::{Operand, PcRead};
    // bl +0 at 0x1000 branches to 0x1008, and b.eq the same.
    assert_eq!(
        a32::decode(0xeb000000).unwrap().at(0x1000).to_string(),
        "bl\t0x1008"
    );
    assert_eq!(
        a32::decode(0x0a000000).unwrap().at(0x1000).to_string(),
        "beq\t0x1008"
    );
    // ldr r0, [pc, #4] loads from 0x1000 + 8 + 4, and the SUB form of ADR
    // subtracts.
    assert_eq!(
        a32::decode(0xe59f0004).unwrap().at(0x1000).to_string(),
        "ldr\tr0, 0x100c"
    );
    assert_eq!(
        a32::decode(0xe24f0004).unwrap().at(0x1000).to_string(),
        "adr\tr0, 0x1004"
    );
    // In T32 the PC is 4 ahead, and a halfword-aligned instruction's literal
    // load aligns it down first. From 0x1002 that is 0x1004, plus 4.
    let b = t32::decode(0xe000_0000, ItState::Outside).unwrap();
    assert_eq!(b.at(0x1000).to_string(), "b\t0x1004");
    let ldr = t32::decode(0x4801_0000, ItState::Outside).unwrap();
    assert_eq!(ldr.at(0x1002).to_string(), "ldr\tr0, 0x1008");
    // The label follows the condition and the register in the view.
    assert_eq!(
        ldr.operands()[2],
        Operand::Label {
            offset: 4,
            pc: PcRead { ahead: 4, align: 4 }
        }
    );
    assert_eq!(PcRead { ahead: 4, align: 4 }.at(0x1002), 0x1004);
}

/// A register list is one operand with its braces, and what is in it comes
/// from the decode as the explanation says: a mask, a run from a first
/// register, or the shapes the rows spell, keyed by a field, an encoding or a
/// quantity the rows place.
#[test]
fn a_register_list_holds_what_the_explanation_says_is_in_it() {
    use exarmo_aarch32::{Lane, ListFile, Operand, RegList};
    let list = |inst: &Instruction| {
        inst.operands()
            .iter()
            .find_map(|op| match op {
                Operand::List(list) => Some(*list),
                _ => None,
            })
            .expect("a list in the view")
    };
    // push is stmdb sp!, and its list is the mask the decode binds.
    let push = a32::decode(0xe92d4010).unwrap();
    assert_eq!(push.at(0).to_string(), "push\t{r4, lr}");
    assert_eq!(
        list(&push),
        RegList {
            mask: 0x4010,
            file: ListFile::Core,
            lane: Lane::Whole
        }
    );
    // The 16-bit encoding's list is eight bits wide.
    let ldm = t32::decode(0xc803_0000, ItState::Outside).unwrap();
    assert_eq!(list(&ldm).mask, 0b11);
    // A single register in braces, from the field naming it.
    let pop = a32::decode(0xe49d0004).unwrap();
    assert_eq!(pop.at(0).to_string(), "pop\t{r0}");
    assert_eq!(list(&pop).mask, 1);
    // Consecutive from a first register and counted by a field, as in
    // vldmia r0, {s0, s1, s2}.
    let vldm = a32::decode(0xec900a03).unwrap();
    assert_eq!(list(&vldm).file, ListFile::Single);
    assert_eq!(list(&vldm).mask, 0b111);
    // Shapes keyed by a field. VLD4's itype says double spacing, and VTBL's
    // len says how many.
    let vld4 = a32::decode(0xf46b718f).unwrap();
    assert_eq!(
        vld4.at(0).to_string(),
        "vld4.32\t{d23, d25, d27, d29}, [r11]"
    );
    assert_eq!(list(&vld4).mask, 1 << 23 | 1 << 25 | 1 << 27 | 1 << 29);
    let vtbl = a32::decode(0xf3b9ea26).unwrap();
    assert_eq!(vtbl.at(0).to_string(), "vtbl.8\td14, {d9, d10, d11}, d22");
    assert_eq!(list(&vtbl).file, ListFile::Double);
    // A quantity the rows place by the size. VLD2 to all lanes is double
    // spaced at a size of 16 by the T bit, with every lane written.
    let vld2 = a32::decode(0xf4a2cd6f).unwrap();
    assert_eq!(vld2.at(0).to_string(), "vld2.16\t{d12[], d14[]}, [r2]");
    assert_eq!(list(&vld2).lane, Lane::All);
    // The lane index the rows place by the size, index_align<3:1> at 8.
    let vld1 = a32::decode(0xf4e780cf).unwrap();
    assert_eq!(vld1.at(0).to_string(), "vld1.8\t{d24[6]}, [r7]");
    assert_eq!(list(&vld1).lane, Lane::Index(6));
    // Four from d30 runs two past d31.
    assert_eq!(
        RegList::spaced(ListFile::Double, 30, 4, 1, Lane::Whole),
        None
    );
}

/// A register with a lane in one operand, `<Dm[x]>`, is the register and the
/// lane the decode binds. Only the decode states the 16-bit split of the four
/// bits, since the prose's `encodedin` gives only the 32-bit one.
#[test]
fn a_scalar_is_the_register_and_lane_the_decode_binds() {
    use exarmo_aarch32::{Operand, Reg, RegOperand, Scalar};
    let scalar = |inst: &Instruction| {
        inst.operands()
            .iter()
            .find_map(|op| match op {
                Operand::Reg(RegOperand {
                    reg: Reg::Double(reg),
                    index: Some(index),
                    ..
                }) => Some((*reg, *index)),
                _ => None,
            })
            .expect("a scalar in the view")
    };
    // vmla.i16 d0, d15, d0[3]. At 16 bits the register is Vm<2:0> and the
    // lane M:Vm<3>, so Vm = 1000 with M set is d0[3], not d8[1].
    let vmla = a32::decode(0xf29f0068).unwrap();
    assert_eq!(vmla.at(0).to_string(), "vmla.i16\td0, d15, d0[3]");
    assert_eq!(scalar(&vmla), (DReg::new(0), 3));
    match vmla {
        Instruction::VmlaSA1D { Dm_x, .. } => assert_eq!(
            Dm_x,
            Scalar {
                reg: DReg::new(0),
                lane: 3
            }
        ),
        other => panic!("{other:?}"),
    }
    // At 32 bits the register is all of Vm and the lane is M alone.
    let vmul = a32::decode(0xf2982864).unwrap();
    assert_eq!(vmul.at(0).to_string(), "vmul.i16\td2, d8, d4[2]");
    // VDUP's lane is in imm4 above the size's low set bit, and VMOV's in
    // opc1:opc2, as in vdup.8 d0, d22[7], vmov.8 d0[1], r0 and
    // vmov.u16 r0, d0[2].
    assert_eq!(
        scalar(&a32::decode(0xf3bf0c26).unwrap()),
        (DReg::new(22), 7)
    );
    assert_eq!(scalar(&a32::decode(0xee400b30).unwrap()), (DReg::new(0), 1));
    assert_eq!(scalar(&a32::decode(0xeeb00b30).unwrap()), (DReg::new(0), 2));
}

/// A table's reserved rows name nothing. A row of two spellings is written
/// by the second. A row holding only some of the bits names its symbol at
/// each value of the rest. An operand written as letters, one per bit, is
/// the table those letters make.
#[test]
fn a_table_names_what_it_names_and_the_field_holds_the_rest() {
    let text = |word: u32| a32::decode(word).unwrap().at(0).to_string();
    // MRS's special register is `CPSR|APSR`, written as the second.
    assert_eq!(text(0xe10f8000), "mrs\tr8, apsr");
    // VMRS's rows reserve values as UNPREDICTABLE by patterns.
    assert_eq!(text(0xeef65a10), "vmrs\tr5, mvfr1");
    // VCVT's data types are rows holding only some of the bits.
    assert_eq!(text(0xf3bbb625), "vcvt.f32.s32\td11, d21");
    // MSR's special register is letters by mask bit behind CPSR or SPSR by R,
    // with the APSR forms the prose says are the same as some of them.
    assert_eq!(text(0xe121f000), "msr\tcpsr_c, r0");
    assert_eq!(text(0xe128f000), "msr\tapsr_nzcvq, r0");
    assert_eq!(text(0xe16ff000), "msr\tspsr_fsxc, r0");
    // CPS's interrupt flags are the letters of its fields.
    assert_eq!(text(0xf10c53ce), "cpsid\taif");
    assert_eq!(
        t32::decode(0xb671_0000, ItState::Outside)
            .unwrap()
            .at(0)
            .to_string(),
        "cpsid\tf"
    );
    // A coprocessor register's number is written behind the `c` its prose
    // numbers it with.
    assert_eq!(text(0xec45be40), "mcrr\tp14, #0x4, r11, r5, c0");
    // A size the decode computes is written in the mnemonic, and a Q register
    // whose encodedin carries the Q bit is the decode's own number.
    assert_eq!(text(0x5eea7b90), "vduppl.8\tq13, r7");
    // An Execute naming both expansions is read by the instruction set.
    assert_eq!(
        t32::decode(0xf07f70aa, ItState::Outside)
            .unwrap()
            .at(0)
            .to_string(),
        "mvns\tr0, #0x1540000"
    );
}

/// A register the decode names where its fields are not its name, a
/// writeback mark written by the register it belongs to, a register a
/// template names outright, and a label that wraps at 32 bits.
#[test]
fn a_register_is_the_decodes_and_its_mark_is_its_own() {
    let a32 = |word: u32| a32::decode(word).unwrap().at(0).to_string();
    let t32 = |word: u32| {
        t32::decode(word, ItState::Outside)
            .unwrap()
            .at(0)
            .to_string()
    };
    // LDRD's <Rt2> is the decode's t+1, VMOV's <Sm1> the register after <Sm>,
    // and T32's high-register CMP holds <Rn> in N:Rn.
    assert_eq!(a32(0xe14b25f0), "strd\tr2, r3, [r11, #-0x50]");
    assert_eq!(a32(0xec57ea12), "vmov\tlr, r7, s4, s5");
    assert_eq!(t32(0x4500_0000), "cmp\tr0, r0");
    assert_eq!(t32(0x4468_0000), "add\tr0, sp, r0");
    // The mark comes from the W field, from the list for the 16-bit LDM,
    // outright for STM T1, and on a register the template names, SRS's SP.
    assert_eq!(a32(0xe8b00010), "ldm\tr0!, {r4}");
    assert_eq!(a32(0xe8900010), "ldm\tr0, {r4}");
    assert_eq!(t32(0xc810_0000), "ldm\tr0!, {r4}");
    assert_eq!(t32(0xc010_0000), "stm\tr0!, {r4}");
    assert_eq!(a32(0xf96d0500), "srsdb\tsp!, #0x0");
    // A shift amount of zero leaves its block out, and an optional number
    // with no default is written.
    assert_eq!(a32(0x06bad017), "ssateq\tsp, #0x1b, r7");
    assert_eq!(a32(0xee12efff), "mrc\tp15, #0x0, lr, c2, c15, #0x7");
    // VEXT's offset is the field, in bytes, since no other operand shares
    // it. VSHL's and VQSHRN's are the decode's, since <size> shares imm6.
    assert_eq!(a32(0xf2b547a1), "vext.8\td4, d21, d17, #0x7");
    assert_eq!(a32(0xf29ce51b), "vshl.i16\td14, d11, #0xc");
    assert_eq!(a32(0xf3ffa916), "vqshrn.u64\td26, q3, #0x1");
    // A branch back past the start of memory wraps to 32 bits.
    assert_eq!(a32(0xeafffffd), "b\t0xfffffffc");
}

/// A constant of the type an instruction names is the element of the value
/// the decode expands to, at the width the type gives, and a floating-point
/// one is the number. A width the prose gives as a sum is solved for.
#[test]
fn a_constant_is_the_element_its_type_gives() {
    use exarmo_aarch32::Operand;
    let text = |word: u32| a32::decode(word).unwrap().at(0).to_string();
    // The type in the mnemonic, at 16, 8 and 64 bits of the replicated value.
    assert_eq!(text(0xf2827836), "vmvn.i16\td7, #0x26");
    assert_eq!(text(0xf3c21e17), "vmov.i8\td17, #0xa7");
    assert_eq!(text(0xf2c2ae30), "vmov.i64\td26, #0xff0000000000");
    assert_eq!(text(0xf2c2c352), "vorr.i32\tq14, #0x2200");
    // A floating-point type in <dt> gives the number, in the view too.
    assert_eq!(text(0xf285cf5a), "vmov.f32\tq6, #0.40625");
    // The pattern the encoding holds comes through beside the value, so a
    // consumer building a constant of that width reads it rather than
    // encoding the value back.
    assert_eq!(
        a32::decode(0xf285cf5a).unwrap().operands()[2],
        Operand::FpImm {
            value: 0.40625,
            width: 32,
            bits: 0x3ed00000,
        }
    );
    // VFPExpandImm's constant at each precision.
    assert_eq!(text(0xeeb00a08), "vmov.f32\ts0, #3.0");
    assert_eq!(text(0xeef00b08), "vmov.f64\td16, #3.0");
    // BFI's width is msb - lsb + 1, and UBFX's the field plus one.
    assert_eq!(text(0xe7cb0290), "bfi\tr0, r0, #0x5, #0x7");
    assert_eq!(text(0xe7e74c57), "ubfx\tr4, r7, #0x18, #0x8");
    // Fraction bits are 16 minus the field, through the decode.
    assert_eq!(text(0xeefa0ac6), "vcvt.f32.s32\ts1, s1, #0x14");
}

/// A shift by a register is the modifier's register, an option is written
/// in the braces its prose encloses it in, a post-indexed offset is read
/// behind its sign, and an offset the prose fixes at zero is left out.
#[test]
fn a_shift_by_a_register_and_the_offsets_after_the_brackets() {
    use exarmo_aarch32::{GpReg, Modifier, ModifierKind, Operand, Reg, RegOperand};
    let text = |word: u32| a32::decode(word).unwrap().at(0).to_string();
    assert_eq!(text(0xe0a01352), "adc\tr1, r0, r2, asr r3");
    // The shift is a slot of its own in the template, `<Rm>, <shift> <Rs>`,
    // and the view folds it onto the register it applies to.
    let adc = a32::decode(0xe0a01352).unwrap();
    assert_eq!(
        adc.operands()[3],
        Operand::Reg(RegOperand {
            reg: Reg::Core(GpReg::new(2)),
            index: None,
            writeback: false,
            modifier: Some(Modifier {
                kind: ModifierKind::Asr,
                amount: None,
                by: Some(GpReg::new(3)),
            }),
        })
    );
    assert_eq!(text(0xec975ecc), "ldc\tp14, c5, [r7], {204}");
    assert_eq!(text(0xe4110004), "ldr\tr0, [r1], #-0x4");
    assert_eq!(text(0xe6910002), "ldr\tr0, [r1], r2");
    assert_eq!(text(0xe1910f9f), "ldrex\tr0, [r1]");
    assert_eq!(
        t32::decode(0xf360_0107, ItState::Outside)
            .unwrap()
            .at(0)
            .to_string(),
        "bfi\tr1, r0, #0x0, #0x8"
    );
}

/// A table the bundle states as sentences is read as one, a value fixed for
/// an encoding is written as its name, and IT's letters come from the mask.
#[test]
fn a_table_stated_in_sentences_is_a_table() {
    let a32 = |word: u32| a32::decode(word).unwrap().at(0).to_string();
    let t32 = |word: u32| {
        t32::decode(word, ItState::Outside)
            .unwrap()
            .at(0)
            .to_string()
    };
    // LDM's addressing mode, "Encoded as P = 1, U = 1", and IA left out as
    // the default.
    assert_eq!(a32(0xe9d00002), "ldmib\tr0, {r1}^");
    assert_eq!(a32(0xe8d00002), "ldm\tr0, {r1}^");
    // ISB's one name, and the bits where it names none.
    assert_eq!(a32(0xf57ff06f), "isb\tsy");
    assert_eq!(a32(0xf57ff061), "isb\t#0x1");
    // VMOV's type from fields taken in parts, "U = 1, opc1<1> = 0, opc2<0> = 1".
    assert_eq!(a32(0xeeb00b30), "vmov.u16\tr0, d0[2]");
    // VSHLL's size from the column for its encoding, its type from the
    // sentence for T1/A1 or fixed to I, and its shift from the decode where
    // the prose names a different slice per size.
    assert_eq!(a32(0xf29b8a1f), "vshll.s16\tq4, d15, #0xb");
    assert_eq!(a32(0xf3b64308), "vshll.i16\tq2, d8, #0x10");
    // VQSHRUN's type "is S when U is 1".
    assert_eq!(a32(0xf3cd5810), "vqshrun.s16\td21, q0, #0x3");
    // Each IT letter is the mask bit against the condition's low bit, and
    // the letters stop where the mask says.
    assert_eq!(t32(0xbf08_0000), "it\teq");
    assert_eq!(t32(0xbf09_0000), "itett\teq");
    assert_eq!(t32(0xbf1a_0000), "itte\tne");
    // A rotation left out is no modifier at all.
    assert_eq!(a32(0xe6af0072), "sxtb\tr0, r2");
    assert_eq!(a32(0xe6af0472), "sxtb\tr0, r2, ror #0x8");
    // A zero subtracted is written with its sign, the U bit clear.
    assert_eq!(a32(0xe4001000), "str\tr1, [r0], #-0x0");
    assert_eq!(a32(0xe4801000), "str\tr1, [r0], #0x0");
    // VQSHLU's type is S, as its decode says and its table does not.
    assert_eq!(a32(0xf3f18698), "vqshlu.s64\td24, d8, #0x31");
    assert_eq!(t32(0xfff4_3617), "vqshlu.s32\td19, d7, #0x14");
    // The by-scalar register at 16 bits is Vm<2:0>, whichever Vm<3> holds.
    assert_eq!(a32(0xf2d2194a), "vmul.f16\td17, d2, d2[1]");
    // A base the template names outright.
    assert_eq!(t32(0x9801_0000), "ldr\tr0, [sp, #0x4]");
}
