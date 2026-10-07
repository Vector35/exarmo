//! The buffers an instruction's text needs, against what it writes.
//!
//! `Instruction::MAX_TOKENS` and `MAX_TEXT` are what the C header states, so
//! that a caller reserves a buffer once and never has to ask for more. They
//! are worked out at generation over every encoding's template, each
//! operand counted at the widest it is written, so they are an upper bound
//! rather than a measurement. This holds the runtime to them.

mod corpus;

use exarmo_aarch64::{Instruction, SysReg, decode_word};
use exarmo_testing::bounds::{stepped, stride, widest};

/// The most tokens and the most characters the corpus writes.
fn measured() -> (usize, usize) {
    let text = corpus::text();
    let widest = widest(corpus::encodings(&text), corpus::INSTR_ADDRESS, decode_word);
    (widest.tokens.0, widest.chars.0)
}

#[test]
fn the_corpus_fits_the_buffers_the_bound_asks_for() {
    let (tokens, chars) = measured();
    assert!(tokens > 0, "the corpus writes instructions");
    assert!(
        tokens <= Instruction::MAX_TOKENS,
        "the corpus writes {tokens} tokens and the bound is {}",
        Instruction::MAX_TOKENS
    );
    assert!(
        chars <= Instruction::MAX_TEXT,
        "the corpus writes {chars} characters and the bound is {}",
        Instruction::MAX_TEXT
    );
}

/// The bound is held within a few times what the corpus reaches, so that a
/// reading which makes it wildly loose is noticed. A buffer nobody can
/// overrun is no use if it is a kilobyte.
#[test]
fn the_bound_is_not_wildly_loose() {
    let (tokens, chars) = measured();
    assert!(
        Instruction::MAX_TOKENS <= tokens * 3,
        "{} tokens bounds a corpus that writes {tokens}",
        Instruction::MAX_TOKENS
    );
    assert!(
        Instruction::MAX_TEXT <= chars * 6,
        "{} characters bounds a corpus that writes {chars}",
        Instruction::MAX_TEXT
    );
}

/// The widest register, symbol and operand written whole that the generator
/// was told of, checked against what the runtime writes.
///
/// The generator states the widths itself because it cannot see a
/// `Display`. A later release that names something
/// longer fails here rather than handing a caller a buffer that does not
/// hold.
#[test]
fn nothing_is_written_wider_than_the_generator_was_told() {
    // A system register's name can be an operand written whole.
    let name = SysReg::all()
        .max_by_key(|def| def.name.len())
        .expect("the architecture names system registers");
    assert!(
        name.name.len() <= 32,
        "{} is {} characters, and an operand written whole is bounded at 32",
        name.name,
        name.name.len()
    );

    // ZERO's mask names the tiles it clears, which is the widest operand
    // written whole in the architecture.
    let widest = widest((0u32..256).map(|mask| 0xC008_0000 | mask), 0, decode_word)
        .token
        .0;
    assert!(
        widest <= 32,
        "ZERO writes a mask of {widest} characters, and an operand written whole is bounded at 32"
    );
}

/// The bounds over the encoding space, which reaches encodings and field
/// values the corpus does not. Ignored, being a sweep. Run it in release
/// with
///
/// ```text
/// BOUNDS_STRIDE=1 cargo test --release -p exarmo-aarch64 --test bounds \
///     -- --ignored --nocapture sweep
/// ```
///
/// A stride of one walks all four billion words, which takes a few minutes.
/// The default steps by a prime, which reaches every encoding without
/// walking every word.
#[test]
#[ignore]
fn sweep() {
    let found = widest(stepped(0, u32::MAX, stride()), 0, decode_word);
    let (tokens, widest_tokens) = found.tokens;
    let (chars, widest_chars) = found.chars;
    println!(
        "most tokens {tokens} of {} at {widest_tokens:08X}: {}",
        Instruction::MAX_TOKENS,
        decode_word(widest_tokens).unwrap().at(0)
    );
    println!(
        "most characters {chars} of {} at {widest_chars:08X}: {}",
        Instruction::MAX_TEXT,
        decode_word(widest_chars).unwrap().at(0)
    );
    assert!(tokens <= Instruction::MAX_TOKENS);
    assert!(chars <= Instruction::MAX_TEXT);
}
