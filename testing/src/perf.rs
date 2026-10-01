//! How long decoding and formatting take, per instruction, over a corpus.
//!
//! It takes three figures, each in nanoseconds per corpus word. The first is
//! the decoder alone. The second adds the token stream into a sink that
//! keeps nothing, which is the formatter's own work. The third adds the text
//! into a buffer instead, which also counts building it.

use std::fmt;
use std::time::Instant;

use exarmo_core::tokens::{OperandIndex, TokenKind, TokenSink};
use exarmo_core::{DecodeError, Decoded};

/// How many times to go over the corpus, as `PERF_PASSES` says, ten by
/// default.
pub fn passes() -> usize {
    std::env::var("PERF_PASSES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10)
}

/// A sink that counts its tokens and keeps nothing.
struct Count(usize);

impl TokenSink for Count {
    fn text(&mut self, _kind: TokenKind, _operand: OperandIndex, text: &str) -> fmt::Result {
        self.0 += text.len();
        Ok(())
    }

    fn value(
        &mut self,
        _kind: TokenKind,
        _operand: OperandIndex,
        _value: &dyn fmt::Display,
    ) -> fmt::Result {
        self.0 += 1;
        Ok(())
    }
}

/// Time the three figures over `words`, each read at `address`, and print
/// them under `label`.
pub fn measure<I: Decoded>(
    label: &str,
    words: &[u32],
    passes: usize,
    address: u64,
    decode: impl Fn(u32) -> Result<I, DecodeError>,
) {
    let count = (words.len() * passes) as f64;
    let mut sink = 0usize;
    let t0 = Instant::now();
    for _ in 0..passes {
        for &w in words {
            sink += std::hint::black_box(decode(w)).is_ok() as usize;
        }
    }
    let t1 = Instant::now();
    let mut counting = Count(0);
    for _ in 0..passes {
        for &w in words {
            if let Ok(inst) = decode(w) {
                inst.write_tokens_at(address, &mut counting).unwrap();
            }
        }
    }
    let t2 = Instant::now();
    let mut buf = String::new();
    for _ in 0..passes {
        for &w in words {
            buf.clear();
            if let Ok(inst) = decode(w) {
                inst.write_tokens_at(address, &mut buf).unwrap();
                sink += buf.len();
            }
        }
    }
    let t3 = Instant::now();
    std::hint::black_box((sink, counting.0));

    let decode_ns = (t1 - t0).as_nanos() as f64 / count;
    let tokens_ns = (t2 - t1).as_nanos() as f64 / count;
    let text_ns = (t3 - t2).as_nanos() as f64 / count;
    eprintln!(
        "{label}: {} encodings x {passes} passes: decode {decode_ns:.1} ns, \
         decode + tokens {tokens_ns:.1} ns, decode + text {text_ns:.1} ns",
        words.len()
    );
}
