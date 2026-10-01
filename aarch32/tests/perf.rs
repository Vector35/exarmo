//! How long decoding and formatting take, per instruction, over the corpus.
//!
//! Ignored because it exists to print. Run it in release with `--nocapture`,
//! and `PERF_PASSES` for how many times to go over the corpus:
//!
//! ```text
//! PERF_PASSES=10 cargo test --release --test perf -- --ignored --nocapture
//! ```

mod corpus;

use corpus::Set;

#[test]
#[ignore = "prints how long decoding and formatting take"]
fn throughput() {
    let passes = exarmo_testing::perf::passes();
    for set in [Set::A32, Set::T32] {
        let text = set.text();
        let words: Vec<u32> = corpus::encodings(&text).collect();
        exarmo_testing::perf::measure(
            &format!("{set:?}"),
            &words,
            passes,
            corpus::INSTR_ADDRESS,
            |w| set.decode(w),
        );
    }
}
