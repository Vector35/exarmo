//! How long the C entry points take, per instruction, over the corpus.
//!
//! Ignored because it exists to print. Run it in release with `--nocapture`,
//! and `PERF_PASSES` for how many times to go over the corpus:
//!
//! ```text
//! PERF_PASSES=10 cargo test --release -p exarmo-aarch64-capi --test perf -- --ignored --nocapture
//! ```
//!
//! It prints four figures, in nanoseconds per corpus encoding, each through
//! the C entry points as a caller would use them: decoding into the caller's
//! storage, and decoding followed by the operands, by the text, or by the
//! tokens and their text.

use std::mem::MaybeUninit;
use std::time::Instant;

use exarmo_aarch64_capi::model::COperand;
use exarmo_aarch64_capi::*;

/// The first word of each corpus line that holds one.
fn encodings() -> Vec<u32> {
    let text = exarmo_testing::read(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../aarch64/tests/cases"
    )));
    exarmo_testing::encodings(&text, exarmo_testing::word_of).collect()
}

/// Time the four figures over `words`, decoding each through `decode`.
///
/// The decode is a closure to keep this the same shape as the AArch32 test,
/// which has two entry points.
fn measure(words: &[u32], passes: usize, decode: impl Fn(u32, *mut CInstruction) -> Status) {
    let count = (words.len() * passes) as f64;
    let address = 0x8000000000000004u64;

    let mut sink = 0usize;
    let mut inst = MaybeUninit::<CInstruction>::uninit();

    let t0 = Instant::now();
    for _ in 0..passes {
        for &w in words {
            sink += (decode(w, inst.as_mut_ptr()) == Status::Ok) as usize;
        }
    }
    let t1 = Instant::now();

    let mut operands = vec![COperand::from(exarmo_aarch64::Operand::Other); 8];
    for _ in 0..passes {
        for &w in words {
            if decode(w, inst.as_mut_ptr()) == Status::Ok {
                sink += unsafe {
                    exarmo_aarch64_instruction_operands(
                        inst.as_ptr(),
                        operands.as_mut_ptr(),
                        operands.len(),
                    )
                };
            }
        }
    }
    let t2 = Instant::now();

    let mut text = [0u8; 128];
    for _ in 0..passes {
        for &w in words {
            if decode(w, inst.as_mut_ptr()) == Status::Ok {
                sink += unsafe {
                    exarmo_aarch64_instruction_text(
                        inst.as_ptr(),
                        address,
                        text.as_mut_ptr(),
                        text.len(),
                    )
                };
            }
        }
    }
    let t3 = Instant::now();

    let mut tokens = [Token {
        kind: 0,
        operand: Token::NO_OPERAND,
        offset: 0,
        length: 0,
        value: 0,
    }; 32];
    for _ in 0..passes {
        for &w in words {
            if decode(w, inst.as_mut_ptr()) == Status::Ok {
                let size = unsafe {
                    exarmo_aarch64_instruction_tokens(
                        inst.as_ptr(),
                        address,
                        text.as_mut_ptr(),
                        text.len(),
                        tokens.as_mut_ptr(),
                        tokens.len(),
                    )
                };
                sink += size.tokens;
            }
        }
    }
    let t4 = Instant::now();
    std::hint::black_box(sink);

    let per = |a: Instant, b: Instant| (b - a).as_nanos() as f64 / count;
    eprintln!(
        "{} encodings x {passes} passes: decode {:.1} ns, decode + operands {:.1} ns, decode + text {:.1} ns, decode + tokens {:.1} ns",
        words.len(),
        per(t0, t1),
        per(t1, t2),
        per(t2, t3),
        per(t3, t4)
    );
}

#[test]
#[ignore = "prints how long the C entry points take"]
fn throughput() {
    let passes = exarmo_testing::perf::passes();
    measure(&encodings(), passes, |w, out| unsafe {
        exarmo_aarch64_decode(w, out)
    });
}
