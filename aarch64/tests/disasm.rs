//! Every case of the corpus renders as it expects, or decodes to the outcome
//! it names, and a failure lists each that does not, with what was expected
//! beside what came out.

use std::panic;

mod corpus;

#[test]
fn every_case_is_what_the_corpus_expects() {
    panic::set_hook(Box::new(|_| {}));
    exarmo_testing::check(corpus::Corpus, env!("CARGO_MANIFEST_DIR"));
}
