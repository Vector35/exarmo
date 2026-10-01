//! Two spellings of one instruction, written in the one form they are
//! compared in.
//!
//! Radix is not part of the architecture and neither is the case of a letter,
//! so both are settled here. Every step preserves value, so two spellings
//! that differ after this differ in what they say.
//!
//! A corpus written by another disassembler can spell a thing its own way,
//! which is the `Rewrite` the set hands in. Each one covers a difference
//! between that disassembler and the architecture, so it is worth reaching
//! for only where the corpus cannot be held to the XML instead.

/// What one instruction set does to its text before its numbers are read.
pub type Rewrite = fn(&str) -> String;

/// Disassembly in the form two spellings of it are compared in: lower case,
/// one space between words, the set's own rewriting applied, and every number
/// written by its value.
pub(crate) fn text(text: &str, rewrite: Rewrite) -> String {
    let collapsed = text.to_lowercase();
    let collapsed = collapsed.split_whitespace().collect::<Vec<_>>().join(" ");
    numbers_by_value(&rewrite(&collapsed))
}

/// Every number written by its value, in decimal with its sign, and a
/// floating-point constant as the number it is.
///
/// Sign is part of the architecture, so `#0xfe` and `#-2` stay apart. A
/// disassembler that writes one where the other is meant is wrong, so an
/// unsigned value is never read as a negative one by guessing how wide the
/// field was.
pub fn numbers_by_value(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '#' {
            result.push(c);
            continue;
        }
        // A floating-point constant is compared by its value, since llvm-mc
        // writes `#3.000000e+00` where this writes `#3.0`.
        let ahead: String = chars
            .clone()
            .take_while(|c| !matches!(c, ' ' | ',' | ']'))
            .collect();
        if ahead.contains(['.', 'e'])
            && let Ok(value) = ahead.parse::<f64>()
        {
            for _ in 0..ahead.chars().count() {
                chars.next();
            }
            result.push_str(&format!("#{value}"));
            continue;
        }
        let mut imm = String::from("#");
        if chars.peek() == Some(&'-') {
            imm.push(chars.next().unwrap());
        }
        if chars.peek() == Some(&'0') {
            imm.push(chars.next().unwrap());
            if chars.peek() == Some(&'x') {
                imm.push(chars.next().unwrap());
            }
        }
        while chars.peek().is_some_and(char::is_ascii_hexdigit) {
            imm.push(chars.next().unwrap());
        }
        result.push_str(&immediate(&imm));
    }
    result
}

/// One immediate by its value.
fn immediate(imm: &str) -> String {
    let (negative, digits) = match imm.strip_prefix("#-") {
        Some(rest) => (true, rest),
        None => (false, &imm[1..]),
    };
    let value = match digits.strip_prefix("0x") {
        Some(hex) => u64::from_str_radix(hex, 16).ok(),
        None => digits.parse::<u64>().ok(),
    };
    match value {
        Some(value) if negative => format!("#-{value}"),
        Some(value) => format!("#{value}"),
        None => imm.to_string(),
    }
}
