//! Which ACLE intrinsics each corpus instruction carries, as a snapshot of
//! one line per encoding that has any, with its text and the intrinsic
//! names, then how many encodings and intrinsics that is. A change in the
//! matcher, the table or the generated lookup shows here as a diff to
//! review.
//!
//! Run with: cargo test --test intrinsics_snapshot
//! Review changes with: cargo insta review

use std::collections::BTreeSet;

mod corpus;

use exarmo_aarch64::decode_word;

#[test]
fn intrinsics_snapshot() {
    let text = corpus::text();
    let mut lines = Vec::new();
    let mut with = 0usize;
    let mut names = BTreeSet::new();
    for bits in corpus::encodings(&text) {
        let Ok(inst) = decode_word(bits) else {
            continue;
        };
        let intrinsics = inst.intrinsics();
        if intrinsics.is_empty() {
            continue;
        }
        with += 1;
        let listed: Vec<&str> = intrinsics.iter().map(|i| i.name()).collect();
        names.extend(listed.iter().copied());
        lines.push(format!(
            "{bits:08X} {}: {}\n",
            inst.at(corpus::INSTR_ADDRESS)
                .to_string()
                .replace('\t', " "),
            listed.join(" ")
        ));
    }
    // In the words' order, so that moving a case between files of the
    // corpus changes nothing here
    lines.sort();
    let mut output = lines.concat();
    output.push_str(&format!(
        "\n{with} encodings carry an intrinsic; {} distinct intrinsics\n",
        names.len()
    ));
    insta::assert_snapshot!(output);
}
