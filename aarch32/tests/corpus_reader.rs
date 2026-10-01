//! AArch32's own corpus rules. exarmo-testing tests the harness's.

mod corpus;

use corpus::{Set, parse};
use exarmo_aarch32::{Cond, ItState};

/// Each tag is one state, so a line decodes the same wherever it stands.
/// Not the last is a mask with a slot left after this one, and the last is
/// the mask ITSTATE holds for a block's final instruction.
#[test]
fn a_t32_tag_is_one_it_state() {
    let text = "1c00 adds r0, r0, #0\n1c00 [eq] addeq r0, r0, #0\n\
                1c00 [ne last] addne r0, r0, #0\ne7fe [eq] UNPREDICTABLE\n\
                1c00 [al] add r0, r0, #0\n";
    let states: Vec<ItState> = parse(Set::T32, text)
        .unwrap()
        .iter()
        .map(|case| corpus::state(case.tag.as_ref()))
        .collect();
    assert_eq!(
        states,
        [
            ItState::Outside,
            ItState::Inside {
                cond: Cond::Eq,
                mask: 0b0100
            },
            ItState::Inside {
                cond: Cond::Ne,
                mask: 0b1000
            },
            ItState::Inside {
                cond: Cond::Eq,
                mask: 0b0100
            },
            ItState::Inside {
                cond: Cond::Al,
                mask: 0b0100
            },
        ]
    );
    assert!(!states[1].last_in_block() && states[2].last_in_block());
}

/// `1111` is no condition an IT block can be under, and llvm-mc's `hs` is
/// not how `Cond` spells CS. An A32 word is never in an IT block.
#[test]
fn a_tag_names_a_condition_of_a_t32_case() {
    assert!(parse(Set::T32, "1c00 [nv] add r0, r0, #0\n").is_err());
    assert!(parse(Set::T32, "1c00 [hs] addhs r0, r0, #0\n").is_err());
    assert!(parse(Set::T32, "1c00 [cs] addcs r0, r0, #0\n").is_ok());
    assert!(parse(Set::A32, "e0810002 [eq] add r0, r1, r2\n").is_err());
    assert!(parse(Set::A32, "e0810002 add r0, r1, r2\n").is_ok());
}
