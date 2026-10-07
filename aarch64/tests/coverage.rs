//! Which encodings, and which of their field values, the corpus never
//! reaches, and where to find them.
//!
//! One test holds every case to a form of the block it stands in, and every
//! field value a block ought to hold to some case, as an instruction or as a
//! word expecting what it is instead, `UNDEFINED` say. `exarmo_testing::blocks`
//! says which values those are. Another holds every form of an encoding
//! written in more than one to a case a step from a case of another form,
//! where the choice between them changes, as `exarmo_testing::boundaries`
//! defines a step. The rest are ignored because they exist to print. One
//! names every `Encoding` no corpus line decodes to. One names the field
//! values no case holds, each with a word holding it and its text, for
//! `tools/fill.py` to add. One sweeps the 32-bit space for words that reach
//! the unreached encodings and prints them as corpus lines. Run them by name
//! with `--nocapture`, in release, as the sweep decodes millions of words:
//!
//! ```text
//! cargo test --release --test coverage -- --ignored --nocapture unreached
//! cargo test --release --test coverage -- --ignored --nocapture field_values > gaps.txt
//! COVERAGE_STRIDE=997 COVERAGE_PER=1 \
//!     cargo test --release --test coverage -- --ignored --nocapture sweep
//! ```
//!
//! The sweep's stride is any odd number, and a prime spreads the visits over
//! every field. `COVERAGE_PER` is how many encodings to keep per variant.

use std::collections::{BTreeMap, BTreeSet};
use std::panic;

mod corpus;

use exarmo_aarch64::{Encoding, decode_word};

/// The encoding a word decodes to, if it decodes at all, without letting a
/// panic in the decoder end the sweep.
fn encoding_of(bits: u32) -> Option<Encoding> {
    panic::catch_unwind(|| decode_word(bits).ok().map(|inst| inst.encoding())).ok()?
}

/// Every encoding no line of the corpus reaches.
fn unreached() -> BTreeSet<Encoding> {
    let text = corpus::text();
    let reached: BTreeSet<Encoding> = corpus::encodings(&text).filter_map(encoding_of).collect();
    (0..Encoding::COUNT)
        .filter_map(Encoding::from_index)
        .filter(|encoding| !reached.contains(encoding))
        .collect()
}

/// The corpus's blocks, as the generator heads them.
fn blocks() -> Vec<exarmo_testing::blocks::Block> {
    exarmo_testing::blocks::blocks(&corpus::text(), exarmo_testing::word_of)
}

/// What a word decodes to, as a case can say it, which is the encoding or
/// that it is no instruction.
fn decoded(bits: u32) -> exarmo_testing::blocks::Decoded {
    use exarmo_testing::blocks::Decoded;
    match (encoding_of(bits), corpus::run(bits)) {
        (Some(encoding), _) => Decoded::As(encoding.name().to_string()),
        (None, corpus::Outcome::Undecodable(_)) => Decoded::Fails,
        _ => Decoded::Neither,
    }
}

#[test]
fn every_case_is_of_its_block_and_every_field_value_is_reached() {
    panic::set_hook(Box::new(|_| {}));
    let blocks = blocks();
    assert!(
        blocks.len() > 4000,
        "the corpus has {} blocks",
        blocks.len()
    );
    let coverage = exarmo_testing::blocks::coverage(&blocks, |bits, _| decoded(bits));
    let misfiled: Vec<String> = coverage
        .misfiled
        .iter()
        .map(|m| {
            format!(
                "{:08X} under {} decodes to {}",
                m.word,
                m.encoding,
                match &m.decodes_to {
                    exarmo_testing::blocks::Decoded::As(encoding) => encoding.as_str(),
                    _ => "nothing",
                }
            )
        })
        .collect();
    assert!(misfiled.is_empty(), "{}", misfiled.join("\n"));
    let gaps: Vec<String> = coverage
        .gaps
        .iter()
        .map(|g| format!("{:08X} {} {}={}", g.word, g.encoding, g.field, g.value))
        .collect();
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
    let boundaries =
        exarmo_testing::boundaries::boundaries(&blocks(), |bits, _| decoded(bits), false);
    let gaps: Vec<String> = boundaries
        .gaps
        .iter()
        .map(|g| format!("{:08X} {} {}", g.word, g.encoding, g.value))
        .collect();
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
    let blocks = blocks();
    let mut coverage = exarmo_testing::blocks::coverage(&blocks, |bits, _| decoded(bits));
    // And each form beside another, searched for where no case leads to one
    let boundaries = exarmo_testing::boundaries::boundaries(&blocks, |bits, _| decoded(bits), true);
    coverage.gaps.extend(boundaries.gaps);
    for (encoding, form) in &boundaries.unreached {
        eprintln!("{encoding}: no word of {form} found beside a word of another form");
    }
    // A line per gap, tab-separated: the word, its block, the value it
    // holds, what a case of it expects at the harness address, and whether
    // that is an instruction. `tools/fill.py` reads them.
    for gap in &coverage.gaps {
        println!(
            "{:08X}\t{}\t{}={}\t{}\t{}",
            gap.word,
            gap.encoding,
            gap.field,
            gap.value,
            corpus::run(gap.word).text().replace('\t', " "),
            if gap.no_instruction {
                "none"
            } else {
                "instruction"
            }
        );
    }
    for encoding in &coverage.unreached {
        eprintln!("{encoding}: no case, and no word of its diagram found that decodes or fails");
    }
    let blocks: BTreeSet<&str> = coverage.gaps.iter().map(|g| g.encoding.as_str()).collect();
    let words: BTreeSet<u32> = coverage.gaps.iter().map(|g| g.word).collect();
    eprintln!(
        "{} field values unreached, in {} blocks, which {} words would reach; \
         {} blocks with no word found",
        coverage.gaps.len(),
        blocks.len(),
        words.len(),
        coverage.unreached.len()
    );
}

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
#[ignore = "sweeps the encoding space for the encodings the corpus never reaches"]
fn sweep() {
    panic::set_hook(Box::new(|_| {}));
    let stride = setting("COVERAGE_STRIDE", 997) as u32;
    let per = setting("COVERAGE_PER", 1) as usize;
    let wanted = unreached();

    let mut found: BTreeMap<Encoding, Vec<u32>> = BTreeMap::new();
    let mut bits: u64 = 0;
    while bits <= u32::MAX as u64 {
        let word = bits as u32;
        bits += stride as u64;
        let Some(encoding) = encoding_of(word) else {
            continue;
        };
        if !wanted.contains(&encoding) {
            continue;
        }
        let seen = found.entry(encoding).or_default();
        if seen.len() < per {
            seen.push(word);
        }
    }

    // Each as a corpus line, the word and how it reads at the harness address.
    let mut count = 0;
    for (encoding, words) in &found {
        for word in words {
            match corpus::run(*word) {
                corpus::Outcome::Rendered { text, .. } => {
                    println!("{word:08X} {text}");
                    count += 1;
                }
                outcome => eprintln!("{word:08X} ({}) {}", encoding.name(), outcome.text()),
            }
        }
    }
    eprintln!(
        "{} of {} unreached encodings found, {count} corpus lines",
        found.len(),
        wanted.len()
    );
}
