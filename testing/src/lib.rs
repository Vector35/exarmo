//! The harness both instruction sets' tests are written on.
//!
//! A corpus is the same shape on either side. It holds one `<hex> <expected
//! disassembly>` per line, with blank lines and comments between blocks.
//! Where the word is no instruction, the expectation names what it is as
//! `DecodeError` writes it, such as `UNDEFINED`, `UNALLOCATED`,
//! `UNPREDICTABLE` or `reserved hint`. Reading it, running a word through a
//! decoder without letting a panic out, and judging what came back are
//! shared here.
//!
//! What differs is named by [`Corpus`], which each side implements: where its
//! corpus files are, how a line's hex becomes a word, how a word is decoded
//! and rendered, and the rewriting its text needs before two spellings of one
//! instruction can be compared.
//!
//! A dev-dependency only. Nothing of the harness belongs in a library.

pub mod blocks;
pub mod boundaries;
pub mod bounds;
pub mod capi;
pub mod corpus;
pub mod invariant;
pub mod normalize;
pub mod perf;

pub use capi::c_smoke;
pub use corpus::{
    Case, Corpus, ItTag, Outcome, Verdict, catch, check, encodings, expected_outcome, parse, read,
    t32_word_of, word_of,
};
pub use invariant::report;
pub use normalize::{Rewrite, numbers_by_value};
