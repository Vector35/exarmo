//! Every case of the A32 and T32 corpora renders as it expects, or decodes to
//! the outcome it names, and a failure lists each that does not, with what
//! was expected beside what came out.

use std::panic;

mod corpus;

use corpus::Set;

#[test]
fn every_a32_case_is_what_the_corpus_expects() {
    panic::set_hook(Box::new(|_| {}));
    exarmo_testing::check(Set::A32, env!("CARGO_MANIFEST_DIR"));
}

#[test]
fn every_t32_case_is_what_the_corpus_expects() {
    panic::set_hook(Box::new(|_| {}));
    exarmo_testing::check(Set::T32, env!("CARGO_MANIFEST_DIR"));
}
