//! A value from a table is a symbol when the table names it and a number
//! when it does not, in the view and in the tokens alike.

use exarmo_aarch64::{Operand, Token, TokenKind, decode};

fn tokens(bits: u32) -> Vec<(TokenKind, String)> {
    let mut sink: Vec<Token> = Vec::new();
    decode(bits).unwrap().at(0).write_tokens(&mut sink).unwrap();
    sink.into_iter().map(|t| (t.kind, t.text)).collect()
}

#[test]
fn a_barrier_option_is_a_name_where_the_table_has_one() {
    // DSB ISH
    let inst = decode(0xD5033B9F).unwrap();
    assert_eq!(inst.at(0).to_string(), "dsb\tish");
    assert!(matches!(inst.operands()[0], Operand::Symbol(s) if s.name == "ish" && s.bits == 11));
    assert_eq!(
        tokens(0xD5033B9F)[2],
        (TokenKind::Symbol, "ish".to_string())
    );

    // DSB #8. The table names nothing there, so the alternation's immediate
    // branch is what is written, and what the view holds
    let inst = decode(0xD503389F).unwrap();
    assert_eq!(inst.at(0).to_string(), "dsb\t#0x8");
    assert!(matches!(inst.operands()[0], Operand::Imm { value: 8, .. }));
    assert_eq!(tokens(0xD503389F)[2], (TokenKind::Text, "#".to_string()));
    assert_eq!(
        tokens(0xD503389F)[3],
        (TokenKind::Immediate(8), "0x8".to_string())
    );
}

#[test]
fn an_element_index_from_a_table_is_an_integer() {
    // DUP v0.4s, v1.s[1]
    let got = tokens(0x4E0C0420);
    let text: String = got.iter().map(|(_, t)| t.as_str()).collect();
    assert_eq!(text, "dup\tv0.4s, v1.s[1]");
    assert!(got.contains(&(TokenKind::Integer(1), "1".to_string())));
    assert!(!got.iter().any(|(k, t)| *k == TokenKind::Symbol && t == "1"));
}

/// Every width the decode expands a constant to is a pattern of that width,
/// half precision included, which C cannot encode back. SVE's FDUP writes its
/// constant at the arrangement's element width, and FCMP against zero has no
/// field at all, so neither carries one.
#[test]
fn a_float_carries_the_pattern_the_encoding_holds() {
    let float = |bits: u32| match decode(bits).unwrap().operands()[1] {
        Operand::FpImm { value, width, bits } => (value, width, bits),
        other => panic!("{other:?} is not a floating-point immediate"),
    };

    // fmov h0, #1.0, whose sixteen bits 0x3c00 are IEEE binary16 for one
    assert_eq!(float(0x1EEE1000), (1.0, 16, 0x3c00));
    // fmov h0, #-2.0
    assert_eq!(float(0x1EF01000).0, -2.0);
    assert_eq!(float(0x1EF01000).1, 16);
    assert_eq!(
        f64::from(half_from_bits(float(0x1EF01000).2 as u16)),
        -2.0,
        "the pattern reads back as the value"
    );

    // fmov s0, #1.0 and fmov d0, #1.0, IEEE binary32 and binary64 for one
    assert_eq!(float(0x1E2E1000), (1.0, 32, 0x3f80_0000));
    assert_eq!(float(0x1E6E1000), (1.0, 64, 0x3ff0_0000_0000_0000));
    // fmov z0.h, #1.0, FDUP's alias, whose element width is the arrangement's
    assert_eq!(float(0x2579CE00), (1.0, 0, 0));
    // fcmp h0, #0.0, whose template writes the value, so there is no field
    assert_eq!(float(0x1EE02008), (0.0, 0, 0));
}

/// IEEE binary16 read back as a float, to check the pattern against the
/// value the view gives.
fn half_from_bits(bits: u16) -> f32 {
    let sign = if bits >> 15 == 1 { -1.0 } else { 1.0 };
    let exponent = i32::from((bits >> 10) & 0x1f);
    let fraction = f32::from(bits & 0x3ff);
    match exponent {
        0 => sign * fraction * 2f32.powi(-24),
        _ => sign * (1.0 + fraction / 1024.0) * 2f32.powi(exponent - 15),
    }
}

/// CSET and its four neighbours read the condition field inverted, through
/// a table of their own that names fourteen of its sixteen values, leaving
/// out AL and NV since the alias is not written for them. The field a plainly
/// read condition holds and the field one of these holds spell opposite
/// conditions, so the view carries what the instruction means rather than
/// the bits.
#[test]
fn a_condition_is_the_one_the_instruction_is_written_under() {
    use exarmo_aarch64::Cond;
    // The condition field sits where each encoding puts it, low for a
    // conditional branch and at bit 12 for the conditional selects.
    for (word, at, cond, lsb, inverted) in [
        // Read plainly, so the field holds what is written
        (0x7A400862u32, 3, Cond::Eq, 12, false),
        (0x54000041u32, 0, Cond::Ne, 0, false),
        // Read inverted, so the field holds the opposite condition's bits
        (0x1A9F07E8u32, 1, Cond::Ne, 12, true),
        (0x1A8A354Eu32, 2, Cond::Cs, 12, true),
        (0x5A9F03E8u32, 1, Cond::Ne, 12, true),
        (0xDA8A1544u32, 2, Cond::Eq, 12, true),
    ] {
        let inst = decode(word).unwrap();
        let ops = inst.operands();
        assert_eq!(ops[at], Operand::Cond(cond), "{word:08X}");
        assert!(
            inst.at(0).to_string().contains(cond.name()),
            "{word:08X} writes {}",
            cond.name()
        );
        let held = u16::try_from((word >> lsb) & 0xF).unwrap();
        assert_eq!(held != cond.bits(), inverted, "{word:08X} holds {held}");
    }
}
