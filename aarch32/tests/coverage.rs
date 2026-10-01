//! Which encodings, and which of their field values, the corpus never
//! reaches, and where to find them.
//!
//! One test holds every case to a form of the block it stands in, and every
//! field value a block ought to hold to some case, as an instruction or as a
//! word expecting what it is instead, `UNDEFINED` say. `exarmo_testing::blocks`
//! says which values those are. Another holds every form of an encoding
//! written in more than one to a case a step from a case of another form,
//! where the choice between them changes, as `exarmo_testing::boundaries`
//! defines a step. A T32 case is decoded in the IT state its tag names, and
//! a T32 block is held to cases inside a block as far as its header says its
//! forms read the state, a form chosen on whether the word is in a block
//! being held to a case of the same word on the other side of that. The
//! rest are ignored because they exist to print. One names the field values
//! no case holds, each with a word holding it, the tag it is decoded under
//! and its text, for `tools/fill.py --aarch32` to add. One names
//! every encoding no corpus line decodes to. One decodes random words of each
//! instruction set and writes, for every encoding it reaches, up to
//! `COVERAGE_PER` of the words that reached it, as `<set> <hex> <encoding>`,
//! which `tools/corpus.py` reads and asks llvm-mc about. Run them by name
//! with `--nocapture`, in release, as the sweep decodes millions of words:
//!
//! ```text
//! cargo test --release --test coverage -- --ignored --nocapture field_values > gaps.txt
//! cargo test --release --test coverage -- --ignored --nocapture unreached
//! COVERAGE_PER=8 cargo test --release --test coverage -- --ignored --nocapture sweep
//! ```
//!
//! A T32 halfword is written as four hex digits and a wide T32 word as eight,
//! the first halfword above the second, which is how the decoder takes it.

use std::collections::{BTreeMap, BTreeSet};
use std::panic;

mod corpus;

use corpus::{ItTag, Set};
use exarmo_aarch32::Encoding;

fn setting(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .map(|value| {
            value
                .parse()
                .unwrap_or_else(|_| panic!("{name}={value} is not a number"))
        })
        .unwrap_or(default)
}

/// The words of one instruction set, as a stream the sweep draws from.
struct Words {
    state: u64,
    set: Set,
}

impl Words {
    /// One word, uniformly random over the set's own space. For T32 that is a
    /// wide word, whose first halfword the hierarchy sends to the 32-bit
    /// tree.
    fn next(&mut self) -> u32 {
        // xorshift64*, which is plenty for spreading visits over the fields.
        self.state ^= self.state >> 12;
        self.state ^= self.state << 25;
        self.state ^= self.state >> 27;
        let word = (self.state.wrapping_mul(0x2545F4914F6CDD1D) >> 32) as u32;
        match self.set {
            Set::A32 => word,
            // 111 in bits 31:29 and anything but 00 in 28:27.
            Set::T32 => {
                let op1 = (word >> 27) & 0b11;
                let op1 = if op1 == 0 { 0b01 } else { op1 };
                (word & 0x07ff_ffff) | 0xe000_0000 | (op1 << 27)
            }
        }
    }
}

/// The encoding a word decodes to in the state a tag names, if it decodes
/// at all, without letting a panic in the decoder end the sweep.
fn encoding_of(set: Set, bits: u32, tag: Option<&ItTag>) -> Option<Encoding> {
    panic::catch_unwind(|| set.decode_in(bits, tag).ok().map(|inst| inst.encoding())).ok()?
}

/// One set's corpus's blocks, as the generator heads them.
fn blocks(set: Set) -> Vec<exarmo_testing::blocks::Block> {
    exarmo_testing::blocks::blocks(&set.text(), exarmo_testing::t32_word_of)
}

/// What a word of a set decodes to in the state a tag names, as a case can
/// say it: the encoding, or that it is no instruction.
fn decoded(set: Set, bits: u32, tag: Option<&ItTag>) -> exarmo_testing::blocks::Decoded {
    use exarmo_testing::blocks::Decoded;
    match (encoding_of(set, bits, tag), corpus::run(set, bits, tag)) {
        (Some(encoding), _) => Decoded::As(encoding.name().to_string()),
        (None, corpus::Outcome::Undecodable(_)) => Decoded::Fails,
        _ => Decoded::Neither,
    }
}

/// A word as a case of its set writes it: a T32 halfword as its four
/// digits, which the high half of the word holds.
fn hex(set: Set, word: u32) -> String {
    match set == Set::T32 && exarmo_aarch32::t32::length((word >> 16) as u16) == 2 {
        true => format!("{:04X}", word >> 16),
        false => format!("{word:08X}"),
    }
}

/// A word and the tag it is decoded under, as a case writes them before its
/// text.
fn placed(set: Set, word: u32, tag: Option<&ItTag>) -> String {
    match tag {
        Some(tag) => format!("{} {tag}", hex(set, word)),
        None => hex(set, word),
    }
}

#[test]
fn every_case_is_of_its_block_and_every_field_value_is_reached() {
    panic::set_hook(Box::new(|_| {}));
    let mut misfiled: Vec<String> = Vec::new();
    let mut gaps: Vec<String> = Vec::new();
    for set in [Set::A32, Set::T32] {
        let blocks = blocks(set);
        assert!(
            blocks.len() > 1000,
            "the {} corpus has {} blocks",
            set.name(),
            blocks.len()
        );
        let coverage =
            exarmo_testing::blocks::coverage(&blocks, |bits, tag| decoded(set, bits, tag));
        misfiled.extend(coverage.misfiled.iter().map(|m| {
            format!(
                "{} {} under {} decodes to {}",
                set.name(),
                placed(set, m.word, m.tag.as_ref()),
                m.encoding,
                match &m.decodes_to {
                    exarmo_testing::blocks::Decoded::As(encoding) => encoding.as_str(),
                    _ => "nothing",
                }
            )
        }));
        gaps.extend(coverage.gaps.iter().map(|g| {
            format!(
                "{} {} {} {}={}",
                set.name(),
                placed(set, g.word, g.tag.as_ref()),
                g.encoding,
                g.field,
                g.value
            )
        }));
    }
    // The hook was silenced for the decoder's panics, and these are the
    // report
    drop(panic::take_hook());
    assert!(misfiled.is_empty(), "{}", misfiled.join("\n"));
    assert!(
        gaps.is_empty(),
        "{} field values no case holds, or blocks with no case; \
         `field_values` and tools/fill.py fill them:\n{}",
        gaps.len(),
        gaps.join("\n")
    );
}

#[test]
fn every_form_has_a_case_beside_a_case_of_another_form() {
    panic::set_hook(Box::new(|_| {}));
    let mut gaps: Vec<String> = Vec::new();
    for set in [Set::A32, Set::T32] {
        let boundaries = exarmo_testing::boundaries::boundaries(
            &blocks(set),
            |bits, tag| decoded(set, bits, tag),
            false,
        );
        gaps.extend(boundaries.gaps.iter().map(|g| {
            format!(
                "{} {} {} {}",
                set.name(),
                placed(set, g.word, g.tag.as_ref()),
                g.encoding,
                g.value
            )
        }));
    }
    drop(panic::take_hook());
    assert!(
        gaps.is_empty(),
        "{} forms no two cases hold on either side of the choice; \
         `field_values` and tools/fill.py fill them:\n{}",
        gaps.len(),
        gaps.join("\n")
    );
}

#[test]
#[ignore = "prints the field values the corpus never reaches"]
fn field_values() {
    panic::set_hook(Box::new(|_| {}));
    for set in [Set::A32, Set::T32] {
        let blocks = blocks(set);
        let mut coverage =
            exarmo_testing::blocks::coverage(&blocks, |bits, tag| decoded(set, bits, tag));
        // And each form beside another, searched for where no case leads to one
        let boundaries = exarmo_testing::boundaries::boundaries(
            &blocks,
            |bits, tag| decoded(set, bits, tag),
            true,
        );
        coverage.gaps.extend(boundaries.gaps);
        for (encoding, form) in &boundaries.unreached {
            eprintln!("{encoding}: no word of {form} found beside a word of another form");
        }
        // A line per gap, tab-separated: the word as a case writes it, its
        // block, the value it holds, what a case of it expects at the
        // harness address, whether that is an instruction, and the tag the
        // case carries, empty outside an IT block. `tools/fill.py` reads them
        for gap in &coverage.gaps {
            let tag = gap.tag.as_ref();
            println!(
                "{}\t{}\t{}={}\t{}\t{}\t{}",
                hex(set, gap.word),
                gap.encoding,
                gap.field,
                gap.value,
                corpus::run(set, gap.word, tag).text().replace('\t', " "),
                if gap.no_instruction {
                    "none"
                } else {
                    "instruction"
                },
                tag.map_or_else(String::new, ItTag::to_string)
            );
        }
        for encoding in &coverage.unreached {
            eprintln!(
                "{encoding}: no case, and no word of its diagram found that decodes or fails"
            );
        }
        let blocks: BTreeSet<&str> = coverage.gaps.iter().map(|g| g.encoding.as_str()).collect();
        let words: BTreeSet<(u32, Option<&ItTag>)> = coverage
            .gaps
            .iter()
            .map(|g| (g.word, g.tag.as_ref()))
            .collect();
        eprintln!(
            "{}: {} field values unreached, in {} blocks, which {} words would reach; \
             {} blocks with no word found",
            set.name(),
            coverage.gaps.len(),
            blocks.len(),
            words.len(),
            coverage.unreached.len()
        );
    }
}

/// Every encoding no line of either corpus reaches.
fn unreached() -> BTreeSet<Encoding> {
    let mut reached = BTreeSet::new();
    for set in [Set::A32, Set::T32] {
        for case in set.corpus() {
            reached.extend(encoding_of(set, case.encoding, case.tag.as_ref()));
        }
    }
    (0..Encoding::COUNT)
        .filter_map(Encoding::from_index)
        .filter(|encoding| !reached.contains(encoding))
        .collect()
}

#[test]
#[ignore = "prints the encodings the corpus never reaches"]
fn unreached_encodings() {
    panic::set_hook(Box::new(|_| {}));
    let missing = unreached();
    for encoding in &missing {
        println!("{}", encoding.name());
    }
    eprintln!(
        "{} of {} encodings unreached by the corpus",
        missing.len(),
        Encoding::COUNT
    );
}

#[test]
#[ignore = "sweeps the encoding space and prints the words reaching each encoding"]
fn sweep() {
    panic::set_hook(Box::new(|_| {}));
    let per = setting("COVERAGE_PER", 8) as usize;
    let count = setting("COVERAGE_WORDS", 8_000_000);

    let mut found: BTreeMap<(Set, Encoding), Vec<u32>> = BTreeMap::new();
    let mut keep = |set: Set, word: u32| {
        if let Some(encoding) = encoding_of(set, word, None) {
            let seen = found.entry((set, encoding)).or_default();
            if seen.len() < per && !seen.contains(&word) {
                seen.push(word);
            }
        }
    };

    // Every halfword, since there are only 65,536 of them. One that the
    // hierarchy sends to the 32-bit tree decodes with a second halfword of
    // zero, and is dropped below rather than kept as a wide word.
    for halfword in 0..=u16::MAX {
        keep(Set::T32, u32::from(halfword) << 16);
    }
    for set in [Set::A32, Set::T32] {
        let mut words = Words {
            state: 0x9E37_79B9_7F4A_7C15,
            set,
        };
        for _ in 0..count {
            keep(set, words.next());
        }
    }

    for ((set, encoding), words) in &found {
        for word in words {
            let narrow = *set == Set::T32 && encoding.length() == 2;
            let wide_from_halfword = *set == Set::T32 && !narrow && word & 0xffff == 0;
            if wide_from_halfword {
                continue;
            }
            let set = set.name();
            match narrow {
                true => println!("{set} {:04x} {encoding:?}", word >> 16),
                false => println!("{set} {word:08x} {encoding:?}"),
            }
        }
    }
    eprintln!("{} encodings reached", found.len());
}
