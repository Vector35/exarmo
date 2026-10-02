//! An instruction rendered as tokens.
//!
//! The token stream itself is [`exarmo_core::tokens`], shared with AArch64.

pub use exarmo_core::decode::Decoded;
pub use exarmo_core::tokens::{Hex, TokenKind, TokenSink};

#[cfg(feature = "alloc")]
pub use exarmo_core::tokens::Token;

exarmo_core::decoded!(crate::Instruction);

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;
    use crate::a32;
    use alloc::string::{String, ToString};
    use alloc::vec::Vec;

    fn tokens(bits: u32, address: u64) -> Vec<(TokenKind, String)> {
        let mut sink: Vec<Token> = Vec::new();
        a32::decode(bits)
            .unwrap()
            .at(address)
            .write_tokens(&mut sink)
            .unwrap();
        sink.into_iter().map(|t| (t.kind, t.text)).collect()
    }

    #[test]
    fn a_condition_rides_on_the_mnemonic() {
        use TokenKind::*;
        // addeq r8, lr, sp, rrx
        let got = tokens(0x008e806d, 0);
        assert_eq!(got[0], (Mnemonic, "add".to_string()));
        assert_eq!(got[1], (Mnemonic, "eq".to_string()));
        assert_eq!(got[2], (Text, "\t".to_string()));
        assert_eq!(got[3], (Register, "r8".to_string()));
        assert_eq!(got.last(), Some(&(Symbol, "rrx".to_string())));
    }

    #[test]
    fn rrx_is_the_shift_on_its_register() {
        use crate::ModifierKind;
        use crate::operands::{Operand, Reg};
        // addeq r8, lr, sp, rrx
        let inst = a32::decode(0x008e806d).unwrap();
        let ops = inst.operands();
        assert_eq!(ops.len(), 4);
        match ops[3] {
            Operand::Reg(r) => {
                assert_eq!(r.reg, Reg::Core(crate::GpReg::new(13)));
                let modifier = r.modifier.expect("sp carries its shift");
                assert_eq!(modifier.kind, ModifierKind::Rrx);
                assert_eq!(modifier.amount, None);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_shift_is_a_symbol_and_its_amount_a_number() {
        use TokenKind::*;
        // addeq r8, lr, sp, ror #0x1e
        let got = tokens(0x008e8f6d, 0);
        let at = got.iter().position(|(k, _)| *k == Symbol).unwrap();
        assert_eq!(got[at], (Symbol, "ror".to_string()));
        assert_eq!(got[at + 1], (Text, " ".to_string()));
        assert_eq!(got[at + 2], (Text, "#".to_string()));
        assert_eq!(got[at + 3], (Immediate(30), "0x1e".to_string()));
    }

    #[test]
    fn a_load_is_its_parts() {
        use TokenKind::*;
        // ldr r6, [r3], #-0xda5
        let got = tokens(0xe4136da5, 0);
        let text: String = got.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(text, "ldr\tr6, [r3], #-0xda5");
        assert_eq!(got[0], (Mnemonic, "ldr".to_string()));
        assert_eq!(got[4], (Bracket, "[".to_string()));
        assert_eq!(got[5], (Register, "r3".to_string()));
        assert_eq!(got[6], (Bracket, "]".to_string()));
    }

    #[test]
    fn a_negative_immediate_keeps_its_sign_with_the_number() {
        use TokenKind::*;
        let got = tokens(0xe4136da5, 0);
        let hash = got
            .iter()
            .position(|(k, t)| *k == Text && t == "#")
            .unwrap();
        assert_eq!(got[hash + 1], (Immediate(-0xda5), "-0xda5".to_string()));
    }

    #[test]
    fn a_register_list_is_bracketed() {
        use TokenKind::*;
        // ldm r2, {r1, r3, r4, r5, r6, sp}
        let got = tokens(0xe892207a, 0);
        let open = got.iter().position(|(k, _)| *k == Bracket).unwrap();
        assert_eq!(got[open], (Bracket, "{".to_string()));
        assert_eq!(got.last().unwrap(), &(Bracket, "}".to_string()));
        assert!(got.iter().any(|(k, t)| *k == Register && t == "sp"));
    }

    #[test]
    fn a_data_type_rides_on_the_mnemonic() {
        use TokenKind::*;
        // vld1.32 {d18[0]}, [r10]
        let got = tokens(0xf4ea280f, 0);
        let text: String = got.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(text, "vld1.32\t{d18[0]}, [r10]");
        assert_eq!(got[0], (Mnemonic, "vld1".to_string()));
        assert!(got.iter().any(|(k, t)| *k == Register && t == "d18"));
    }
}
