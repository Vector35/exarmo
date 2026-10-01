//! Where the AArch32 corpora are, how a word is run, and how their text is
//! written for comparison.
//!
//! `aarch32/tests/cases/a32/` and `cases/t32/` hold a file per instruction
//! file of the bundle, each with a block per encoding of that set, headed as
//! the generator writes it, and one `<hex> <expected disassembly>` per line
//! beneath. `unallocated.txt` holds by hand the words no encoding is. A T32
//! halfword is four hex digits and a wide T32 word eight, the first
//! halfword above the second. An expectation naming an outcome, `UNDEFINED`
//! or `UNALLOCATED` as `DecodeError` writes it, says the word is not an
//! instruction and must decode to that outcome.
//!
//! A T32 case is decoded outside any IT block unless a tag between its hex
//! and its text says otherwise: `1c00 [eq] addeq r0, r0, #0` is inside a
//! block under EQ with more of it to come, and `[eq last]` the block's last
//! instruction. The condition is spelt as `Cond` writes it. AL is allowed,
//! since the architecture allows IT AL where every instruction of the
//! block is under AL, and `1111` is not, since IT is UNPREDICTABLE with it.
//! An A32 case takes no tag.
//!
//! Reading, running and judging are `exarmo_testing`'s. This module supplies
//! the two sets, the address a rendering is taken at, and how a condition is
//! spelt.

// Each test takes in the whole module and uses the part it needs.
#![allow(dead_code)]

use exarmo_aarch32::{DecodeError, ItState, a32, t32};
pub use exarmo_testing::{Case, ItTag, Outcome, Verdict};

/// The address every harness disassembles at, so a PC-relative operand in an
/// expectation names one target.
pub const INSTR_ADDRESS: u64 = 0x10000;

/// Which instruction set a corpus is of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Set {
    A32,
    T32,
}

impl Set {
    /// The set's name as the CLI and `tools/corpus.py` write it.
    pub fn name(self) -> &'static str {
        match self {
            Set::A32 => "a32",
            Set::T32 => "t32",
        }
    }

    /// The corpus's text.
    pub fn text(self) -> String {
        exarmo_testing::Corpus::text(self, env!("CARGO_MANIFEST_DIR"))
    }

    /// The corpus, read from this crate's tests directory.
    pub fn corpus(self) -> Vec<Case> {
        exarmo_testing::Corpus::cases(self, env!("CARGO_MANIFEST_DIR"))
    }

    /// Decode a word of the set, outside any IT block.
    pub fn decode(self, bits: u32) -> Result<exarmo_aarch32::Instruction, DecodeError> {
        self.decode_in(bits, None)
    }

    /// Decode a word of the set in the IT state a tag names, outside any
    /// block where there is none.
    pub fn decode_in(
        self,
        bits: u32,
        tag: Option<&ItTag>,
    ) -> Result<exarmo_aarch32::Instruction, DecodeError> {
        match self {
            Set::A32 => a32::decode(bits),
            Set::T32 => t32::decode(bits, state(tag)),
        }
    }

    /// Decode a case of the set, in the state its tag names.
    pub fn decode_case(self, case: &Case) -> Result<exarmo_aarch32::Instruction, DecodeError> {
        self.decode_in(case.encoding, case.tag.as_ref())
    }
}

/// The condition a tag names, as `Cond` spells it.
pub fn cond(name: &str) -> Option<exarmo_aarch32::Cond> {
    (0..16)
        .filter_map(exarmo_aarch32::Cond::from_bits)
        .find(|cond| cond.name() == name)
}

/// The IT state a tag names, outside any block where there is none.
pub fn state(tag: Option<&ItTag>) -> ItState {
    match tag {
        Some(tag) => ItState::Inside {
            cond: cond(&tag.cond).expect("a tag the reader took names a condition"),
            mask: tag.mask(),
        },
        None => ItState::Outside,
    }
}

impl exarmo_testing::Corpus for Set {
    fn path(self) -> &'static str {
        match self {
            Set::A32 => "tests/cases/a32",
            Set::T32 => "tests/cases/t32",
        }
    }

    /// A halfword sits in the high half of the word, which is how the
    /// decoder takes it.
    fn word_of(self, hex: &str) -> Option<u32> {
        exarmo_testing::t32_word_of(hex)
    }

    fn render(self, word: u32) -> Outcome {
        Outcome::of(self.decode(word), INSTR_ADDRESS)
    }

    fn render_in(self, word: u32, tag: &ItTag) -> Outcome {
        Outcome::of(self.decode_in(word, Some(tag)), INSTR_ADDRESS)
    }

    /// Only a T32 decode is told the IT state.
    fn condition(self, name: &str) -> Option<u8> {
        match self {
            Set::A32 => None,
            Set::T32 => cond(name).map(|cond| cond.bits() as u8),
        }
    }

    fn rewrite(self) -> exarmo_testing::Rewrite {
        rewrite
    }
}

/// Read a corpus of a set, one case per line. A T32 halfword reads the same
/// way in either.
pub fn parse(set: Set, text: &str) -> Result<Vec<Case>, String> {
    exarmo_testing::Corpus::parse(set, text)
}

/// The words a corpus names.
pub fn encodings(text: &str) -> impl Iterator<Item = u32> + '_ {
    exarmo_testing::encodings(text, |hex| exarmo_testing::Corpus::word_of(Set::T32, hex))
}

/// Decode and render one word in the state a tag names, catching any panic
/// the decoder raises.
pub fn run(set: Set, word: u32, tag: Option<&ItTag>) -> Outcome {
    exarmo_testing::Corpus::run_in(set, word, tag)
}

/// Whether a rendering is what the corpus expects.
pub fn judge(case: &Case, outcome: &Outcome) -> Verdict {
    exarmo_testing::Corpus::judge(Set::A32, case, outcome)
}

/// Disassembly in the form two spellings of it are compared in.
pub fn normalize(text: &str) -> String {
    exarmo_testing::Corpus::normalize(Set::A32, text)
}

/// The block an instruction is decoded inside for the checks that ask what
/// an IT block does: the last of one under NE, where every instruction a
/// block may hold is allowed, a branch included.
pub const INSIDE: ItState = ItState::Inside {
    cond: exarmo_aarch32::Cond::Ne,
    mask: 0b1000,
};

/// Every word of both corpora that decodes in the state its case names,
/// once each for its set and state, with the first case naming it, which is
/// where a failure points.
pub fn instructions() -> Vec<(Set, Case, exarmo_aarch32::Instruction)> {
    let mut seen = std::collections::HashSet::new();
    [Set::A32, Set::T32]
        .into_iter()
        .flat_map(|set| set.corpus().into_iter().map(move |case| (set, case)))
        .filter(|(set, case)| seen.insert((*set, case.encoding, case.tag.clone())))
        .filter_map(|(set, case)| set.decode_case(&case).ok().map(|inst| (set, case, inst)))
        .collect()
}

/// A case as a failure names it: where it stands, its word, its tag and its
/// text. The two sets name their files alike, so the set is part of the
/// place.
pub fn named(set: Set, case: &Case, inst: &exarmo_aarch32::Instruction) -> String {
    let text = inst.at(INSTR_ADDRESS).to_string().replace('\t', " ");
    let tag = case
        .tag
        .as_ref()
        .map_or_else(String::new, |tag| format!(" {tag}"));
    format!("{}/{}  {}{tag}  {text}", set.name(), case.place, case.hex)
}

/// llvm-mc writes `hs` and `lo` where the architecture's table says `cs` and
/// `cc`. The condition is written onto the mnemonic, as in `bhs`, `addhs.w`
/// and `vaddhs.f32`, and only the stem before any `.` carries it.
fn rewrite(text: &str) -> String {
    let (mnemonic, rest) = match text.find(' ') {
        Some(at) => (&text[..at], &text[at..]),
        None => (text, ""),
    };
    let (stem, suffix) = match mnemonic.find('.') {
        Some(at) => (&mnemonic[..at], &mnemonic[at..]),
        None => (mnemonic, ""),
    };
    let stem = match stem.strip_suffix("hs") {
        Some(head) => format!("{head}cs"),
        None => match stem.strip_suffix("lo") {
            Some(head) => format!("{head}cc"),
            None => stem.to_string(),
        },
    };
    format!("{stem}{suffix}{rest}")
}
