//! Where the AArch64 corpus is, how a word is run, and how its text is
//! written for comparison.
//!
//! `aarch64/tests/cases/` holds a file per instruction file of the bundle,
//! each a block per encoding headed by its diagram and its forms, which the
//! generator writes, and beneath them one `<hex> <expected disassembly>` per
//! line. An expectation naming an outcome, `UNDEFINED` or `UNALLOCATED` as
//! `DecodeError` writes it, says the word is no instruction, for that reason.
//! `unallocated.txt` holds the words that are no instruction of any encoding.
//!
//! Reading, running and judging are `exarmo_testing`'s. This module supplies
//! the corpus path and the address a rendering is taken at.

// Each test takes in the whole module and uses the part it needs.
#![allow(dead_code)]

use exarmo_aarch64::decode;
pub use exarmo_testing::{Case, Outcome, Verdict};

/// The address every harness disassembles at, so a PC-relative operand in an
/// expectation names one target.
pub const INSTR_ADDRESS: u64 = 0x8000000000000004;

/// The one corpus this crate has.
#[derive(Debug, Clone, Copy)]
pub struct Corpus;

impl exarmo_testing::Corpus for Corpus {
    fn path(self) -> &'static str {
        "tests/cases"
    }

    fn render(self, word: u32) -> Outcome {
        Outcome::of(decode(word), INSTR_ADDRESS)
    }
}

/// The corpus, read from this crate's tests directory.
pub fn cases() -> Vec<Case> {
    exarmo_testing::Corpus::cases(Corpus, env!("CARGO_MANIFEST_DIR"))
}

/// The corpus's text, every file of it.
pub fn text() -> String {
    exarmo_testing::Corpus::text(Corpus, env!("CARGO_MANIFEST_DIR"))
}

/// Read a corpus, one case per line.
pub fn parse(text: &str) -> Result<Vec<Case>, String> {
    exarmo_testing::parse(text, |hex| exarmo_testing::Corpus::word_of(Corpus, hex))
}

/// The words a corpus names.
pub fn encodings(text: &str) -> impl Iterator<Item = u32> + '_ {
    exarmo_testing::encodings(text, |hex| exarmo_testing::Corpus::word_of(Corpus, hex))
}

/// Decode and render one word, catching any panic the decoder raises.
pub fn run(word: u32) -> Outcome {
    exarmo_testing::Corpus::run(Corpus, word)
}

/// Whether a rendering is what the corpus expects.
pub fn judge(case: &Case, outcome: &Outcome) -> Verdict {
    exarmo_testing::Corpus::judge(Corpus, case, outcome)
}

/// Disassembly in the form two spellings of it are compared in.
pub fn normalize(text: &str) -> String {
    exarmo_testing::Corpus::normalize(Corpus, text)
}

/// Every word of the corpus that decodes, once each, with the first case
/// naming it, which is where a failure points.
pub fn instructions() -> Vec<(Case, exarmo_aarch64::Instruction)> {
    let mut seen = std::collections::HashSet::new();
    cases()
        .into_iter()
        .filter(|case| seen.insert(case.encoding))
        .filter_map(|case| decode(case.encoding).ok().map(|inst| (case, inst)))
        .collect()
}

/// A case as a failure names it: where it stands, its word and its text.
pub fn named(case: &Case, inst: &exarmo_aarch64::Instruction) -> String {
    let text = inst.at(INSTR_ADDRESS).to_string().replace('\t', " ");
    format!("{}  {}  {text}", case.place, case.hex)
}
