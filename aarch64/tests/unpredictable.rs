//! Whether a word is CONSTRAINED UNPREDICTABLE: it reached a test its decode
//! pseudocode calls UNPREDICTABLE, or it breaks a bit its diagram writes
//! `(0)` or `(1)`.

mod corpus;

use exarmo_aarch64::decode_word;

/// Every case breaking a bit its block's header writes `(0)` or `(1)` is
/// CONSTRAINED UNPREDICTABLE, which holds the test the generator writes into
/// its decode to the header's own reading of the diagram.
#[test]
fn every_case_breaking_a_bit_is_unpredictable() {
    let (mut checked, mut failures) = (0, Vec::new());
    for block in exarmo_testing::blocks::blocks(&corpus::text(), exarmo_testing::word_of) {
        for held in block.words.iter().filter(|held| !held.fails) {
            let Ok(inst) = decode_word(held.word) else {
                continue;
            };
            checked += 1;
            failures.extend(exarmo_testing::invariant::unpredictable_where_broken(
                &block, held.word, &inst,
            ));
        }
    }
    exarmo_testing::report(
        exarmo_testing::invariant::UNPREDICTABLE_WHERE_BROKEN,
        checked,
        &failures,
    );
}
