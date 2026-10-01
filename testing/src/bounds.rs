//! How many tokens and characters instructions write, against the bounds
//! each runtime states.
//!
//! `Instruction::MAX_TOKENS` and `MAX_TEXT` are what each C header states,
//! so that a caller reserves a buffer once and never has to ask for more.

use exarmo_core::tokens::Token;
use exarmo_core::{DecodeError, Decoded};

/// The widest a set of words is written, and the words that reach it.
#[derive(Debug, Clone, Copy, Default)]
pub struct Widest {
    /// The most tokens any word writes, and the first word writing that many.
    pub tokens: (usize, u32),
    /// The most characters, and the first word writing that many.
    pub chars: (usize, u32),
    /// The longest single token, and the first word writing it.
    pub token: (usize, u32),
}

/// What the words write at `address`, skipping those that do not decode or
/// are not written.
pub fn widest<I: Decoded>(
    words: impl IntoIterator<Item = u32>,
    address: u64,
    decode: impl Fn(u32) -> Result<I, DecodeError>,
) -> Widest {
    let mut widest = Widest::default();
    for word in words {
        let Ok(inst) = decode(word) else { continue };
        let mut written: Vec<Token> = Vec::new();
        if inst.write_tokens_at(address, &mut written).is_err() {
            continue;
        }
        let chars: usize = written.iter().map(|token| token.text.len()).sum();
        let token = written
            .iter()
            .map(|token| token.text.len())
            .max()
            .unwrap_or(0);
        for (most, now) in [
            (&mut widest.tokens, written.len()),
            (&mut widest.chars, chars),
            (&mut widest.token, token),
        ] {
            if now > most.0 {
                *most = (now, word);
            }
        }
    }
    widest
}

/// The stride a sweep steps through a word space by, from `BOUNDS_STRIDE`.
/// The default is a prime, which reaches every encoding without walking
/// every word.
pub fn stride() -> u64 {
    std::env::var("BOUNDS_STRIDE")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(997)
}

/// The words from `first` to `last`, `stride` apart.
pub fn stepped(first: u32, last: u32, stride: u64) -> impl Iterator<Item = u32> {
    (u64::from(first)..=u64::from(last))
        .step_by(stride as usize)
        .map(|word| word as u32)
}
