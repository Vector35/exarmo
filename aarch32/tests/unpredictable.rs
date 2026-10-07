//! Whether a word is CONSTRAINED UNPREDICTABLE: it reached a test its decode
//! pseudocode calls UNPREDICTABLE, or it breaks a bit its diagram writes
//! `(0)` or `(1)`.

mod corpus;

use corpus::Set;
use exarmo_aarch32::{ItState, t32};

/// SETEND is 16 bits, so the halfword after it is not its own, and however
/// that is set it breaks none of SETEND's should-be bits. A corpus word of a
/// 16-bit encoding holds nothing there.
#[test]
fn the_halfword_after_a_16_bit_instruction_breaks_nothing() {
    assert!(
        !t32::decode_word(0xb658_ffff, ItState::Outside)
            .unwrap()
            .unpredictable()
    );
}

/// Every case breaking a bit its block's header writes `(0)` or `(1)` is
/// CONSTRAINED UNPREDICTABLE, which holds the test the generator writes into
/// its decode to the header's own reading of the diagram.
#[test]
fn every_case_breaking_a_bit_is_unpredictable() {
    let (mut checked, mut failures) = (0, Vec::new());
    for set in [Set::A32, Set::T32] {
        for block in exarmo_testing::blocks::blocks(&set.text(), exarmo_testing::t32_word_of) {
            for held in block.words.iter().filter(|held| !held.fails) {
                let Ok(inst) = set.decode_in(held.word, held.tag.as_ref()) else {
                    continue;
                };
                checked += 1;
                failures.extend(exarmo_testing::invariant::unpredictable_where_broken(
                    &block, held.word, &inst,
                ));
            }
        }
    }
    exarmo_testing::report(
        exarmo_testing::invariant::UNPREDICTABLE_WHERE_BROKEN,
        checked,
        &failures,
    );
}
