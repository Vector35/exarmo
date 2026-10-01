//! The buffers an instruction's text needs, against what it writes.
//!
//! `Instruction::MAX_TOKENS` and `MAX_TEXT` are what the C header states, so
//! that a caller reserves a buffer once and never has to ask for more. They
//! are worked out at generation over every encoding's template, each
//! operand counted at the widest it is written, so they are an upper bound
//! rather than a measurement. This holds the runtime to them.

mod corpus;

use corpus::Set;
use exarmo_aarch32::{Instruction, SysReg};
use exarmo_testing::bounds::{stepped, stride, widest};

/// Each case of a corpus, as the index of the case, and its decode in the
/// state its tag names, for `widest` to measure.
fn cases(set: Set) -> (Vec<corpus::Case>, Vec<u32>) {
    let cases = set.corpus();
    let indices = (0..cases.len() as u32).collect();
    (cases, indices)
}

/// The most tokens and the most characters a corpus writes.
fn measured(set: Set) -> (usize, usize) {
    let (cases, indices) = cases(set);
    let widest = widest(indices, corpus::INSTR_ADDRESS, |index| {
        set.decode_case(&cases[index as usize])
    });
    (widest.tokens.0, widest.chars.0)
}

#[test]
fn the_corpus_fits_the_buffers_the_bound_asks_for() {
    for set in [Set::A32, Set::T32] {
        let (tokens, chars) = measured(set);
        assert!(tokens > 0, "{set:?} writes instructions");
        assert!(
            tokens <= Instruction::MAX_TOKENS,
            "{set:?} writes {tokens} tokens and the bound is {}",
            Instruction::MAX_TOKENS
        );
        assert!(
            chars <= Instruction::MAX_TEXT,
            "{set:?} writes {chars} characters and the bound is {}",
            Instruction::MAX_TEXT
        );
    }
}

/// The bound is held within a few times what the corpus reaches, so that a
/// reading which makes it wildly loose is noticed.
///
/// AArch32's slack is mostly a register list. VPUSH names up to 32
/// registers and the bound counts them all, where the corpus writes far
/// fewer.
#[test]
fn the_bound_is_not_wildly_loose() {
    let widest = [Set::A32, Set::T32].map(measured);
    let tokens = widest.iter().map(|(tokens, _)| *tokens).max().unwrap();
    let chars = widest.iter().map(|(_, chars)| *chars).max().unwrap();
    assert!(
        Instruction::MAX_TOKENS <= tokens * 4,
        "{} tokens bounds a corpus that writes {tokens}",
        Instruction::MAX_TOKENS
    );
    assert!(
        Instruction::MAX_TEXT <= chars * 6,
        "{} characters bounds a corpus that writes {chars}",
        Instruction::MAX_TEXT
    );
}

/// The generator is told an operand written whole is at most 32 characters.
/// A later release
/// that names something longer fails here rather than handing a caller a
/// buffer that does not hold.
#[test]
fn nothing_is_written_wider_than_the_generator_was_told() {
    // A consumer annotates with a system register's name, so it is held to
    // the same width.
    let name = SysReg::all()
        .max_by_key(|def| def.name.len())
        .expect("the architecture names system registers");
    assert!(
        name.name.len() <= 32,
        "{} is {} characters, and an operand written whole is bounded at 32",
        name.name,
        name.name.len()
    );
    for set in [Set::A32, Set::T32] {
        let (cases, indices) = cases(set);
        let (token, index) = widest(indices, corpus::INSTR_ADDRESS, |index| {
            set.decode_case(&cases[index as usize])
        })
        .token;
        let case = &cases[index as usize];
        assert!(
            token <= 32,
            "{set:?} {} writes a token of {token} characters, and one is bounded at 32",
            case.place
        );
    }
}

/// The bounds over the encoding space, which reaches encodings and field
/// values the corpora do not. It takes every T32 halfword, and A32 and wide
/// T32 words a stride apart. Ignored, being a sweep. Run it in release with
///
/// ```text
/// BOUNDS_STRIDE=1 cargo test --release -p exarmo-aarch32 --test bounds \
///     -- --ignored --nocapture sweep
/// ```
#[test]
#[ignore]
fn sweep() {
    let stride = stride();
    // A wide T32 word's first halfword is 111 and then anything but 00.
    let sweeps: [(Set, Vec<u32>); 3] = [
        (Set::A32, stepped(0, u32::MAX, stride).collect()),
        (
            Set::T32,
            (0..=u16::MAX).map(|h| u32::from(h) << 16).collect(),
        ),
        (Set::T32, stepped(0xE800_0000, u32::MAX, stride).collect()),
    ];
    for (set, words) in sweeps {
        let found = widest(words, 0, |word| set.decode(word));
        let (tokens, widest_tokens) = found.tokens;
        let (chars, widest_chars) = found.chars;
        let (token, widest_token) = found.token;
        println!(
            "{set:?}: most tokens {tokens} of {} at {widest_tokens:08x}, most characters \
             {chars} of {} at {widest_chars:08x}, longest token {token} at {widest_token:08x}",
            Instruction::MAX_TOKENS,
            Instruction::MAX_TEXT
        );
        assert!(tokens <= Instruction::MAX_TOKENS);
        assert!(chars <= Instruction::MAX_TEXT);
        assert!(token <= 32);
    }
}
