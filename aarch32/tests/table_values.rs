//! A value from a table is a symbol when the table names it and a number
//! when it does not, in the view and in the tokens alike.

use exarmo_aarch32::{Operand, Token, TokenKind, a32};

fn tokens(bits: u32) -> Vec<(TokenKind, String)> {
    let mut sink: Vec<Token> = Vec::new();
    a32::decode(bits)
        .unwrap()
        .at(0)
        .write_tokens(&mut sink)
        .unwrap();
    sink.into_iter().map(|t| (t.kind, t.text)).collect()
}

/// `option` is four bits and the table names eleven of the sixteen, so both
/// answers come out of the same table on the same instruction.
#[test]
fn a_barrier_option_is_a_name_where_the_table_has_one() {
    let inst = a32::decode(0xf57ff041).unwrap();
    assert_eq!(inst.at(0).to_string(), "dsb\toshld");
    assert!(matches!(inst.operands()[0], Operand::Symbol(s) if s.name == "oshld" && s.bits == 1));
    assert_eq!(
        tokens(0xf57ff041)[2],
        (TokenKind::Symbol, "oshld".to_string())
    );

    // The table names nothing at 0, so the number is what is written
    let inst = a32::decode(0xf57ff050).unwrap();
    assert_eq!(inst.at(0).to_string(), "dmb\t#0x0");
    assert!(matches!(inst.operands()[0], Operand::Symbol(s) if s.name.is_empty()));
    let got = tokens(0xf57ff050);
    assert_eq!(got[2], (TokenKind::Text, "#".to_string()));
    assert_eq!(got[3], (TokenKind::Immediate(0), "0x0".to_string()));
}

#[test]
fn an_element_index_is_an_integer() {
    let got = tokens(0xf3bf0c26);
    let text: String = got.iter().map(|(_, t)| t.as_str()).collect();
    assert_eq!(text, "vdup.8\td0, d22[7]");
    assert!(got.contains(&(TokenKind::Integer(7), "7".to_string())));
    assert!(!got.iter().any(|(k, t)| *k == TokenKind::Symbol && t == "7"));
}

#[test]
fn a_data_type_is_written_onto_the_mnemonic() {
    let got = tokens(0xf3bc8c0d);
    let text: String = got.iter().map(|(_, t)| t.as_str()).collect();
    assert_eq!(text, "vdup.32\td8, d13[1]");
    assert_eq!(got[0].0, TokenKind::Mnemonic);
    assert!(got.iter().any(|(_, t)| t == "32"));
}
