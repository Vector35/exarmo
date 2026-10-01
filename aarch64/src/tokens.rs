//! An instruction rendered as tokens.
//!
//! The token stream itself is [`exarmo_core::tokens`], shared with AArch32.

pub use exarmo_core::decode::Decoded;
pub use exarmo_core::tokens::{Hex, Token, TokenKind, TokenSink};

exarmo_core::decoded!(crate::Instruction);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode;

    fn tokens(bits: u32, address: u64) -> Vec<(TokenKind, String)> {
        let mut sink: Vec<Token> = Vec::new();
        decode(bits)
            .unwrap()
            .at(address)
            .write_tokens(&mut sink)
            .unwrap();
        sink.into_iter().map(|t| (t.kind, t.text)).collect()
    }

    /// A shift the template writes out whole, a fixed pre-index and a zero
    /// compared against are values as much as what a field holds.
    #[test]
    fn what_the_template_fixes_is_valued() {
        use TokenKind::*;
        // ldrh w0, [x1, x2, lsl #0x1]
        let got = tokens(0x78627820, 0);
        let at = got.iter().position(|(k, _)| *k == Symbol).unwrap();
        assert_eq!(got[at], (Symbol, "lsl".to_string()));
        assert_eq!(got[at + 2], (Text, "#".to_string()));
        assert_eq!(got[at + 3], (Immediate(1), "0x1".to_string()));
        // stilp w4, w0, [x0, #-0x8]!
        let got = tokens(0x99000804, 0);
        assert!(got.contains(&(Immediate(-8), "-0x8".to_string())));
        // cmeq v0.4s, v0.4s, #0x0
        let got = tokens(0x4EA09800, 0);
        assert_eq!(got.last(), Some(&(Immediate(0), "0x0".to_string())));
    }

    #[test]
    fn a_load_is_its_parts() {
        use TokenKind::*;
        let expected = [
            (Mnemonic, "ldr"),
            (Text, "\t"),
            (Register, "x0"),
            (Separator, ", "),
            (Bracket, "["),
            (Register, "x1"),
            // Within the memory operand, so its text
            (Text, ", "),
            (Text, "#"),
            (Immediate(8), "0x8"),
            (Bracket, "]"),
        ];
        assert_eq!(
            tokens(0xF9400420, 0),
            expected.map(|(k, t)| (k, t.to_string()))
        );
    }

    #[test]
    fn a_negative_immediate_keeps_its_sign_with_the_number() {
        use TokenKind::*;
        // LDUR x0, [x1, #-16]
        let got = tokens(0xF85F0020, 0);
        assert_eq!(got[7], (Text, "#".to_string()));
        assert_eq!(got[8], (Immediate(-16), "-0x10".to_string()));
    }

    /// The first token is the mnemonic the instruction names, and what the
    /// template writes onto it follows as its own token.
    #[test]
    fn a_branch_names_its_target() {
        use TokenKind::*;
        // B.EQ #4 at 0x1000
        let got = tokens(0x54000020, 0x1000);
        assert_eq!(got[0], (Mnemonic, "b".to_string()));
        assert_eq!(got[1], (Mnemonic, ".".to_string()));
        assert_eq!(got[2], (Mnemonic, "eq".to_string()));
        assert_eq!(got[4], (Address(0x1004), "0x1004".to_string()));
    }

    #[test]
    fn a_register_list_is_bracketed() {
        use TokenKind::*;
        // LD1 { v0.16b }, [x1]
        let got = tokens(0x4C407020, 0);
        assert_eq!(got[2], (Bracket, "{".to_string()));
        assert!(got.iter().any(|(k, t)| *k == Register && t == "v0"));
        assert_eq!(got.last().unwrap(), &(Bracket, "]".to_string()));
    }
}
