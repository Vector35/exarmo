//! The `exarmo` command, which says what a word decodes to in either
//! instruction set.
//!
//! One command over both runtimes, so that the instruction set is an argument
//! rather than a choice of binary. A hex argument of `-` reads a word per line
//! from the input, printing a line for each, which is how a sweep of the
//! encoding space asks about thousands at once. `--debug` prints the
//! instruction's fields beside its text, for reading what the decode made of
//! them, and `--intrinsics` the first ACLE intrinsic the instruction
//! realises.

use std::fmt;
use std::io::{self, BufRead, Write};
use std::num;

use clap::{Parser, Subcommand, ValueEnum};
#[cfg(feature = "aarch32")]
use exarmo_aarch32::{ItState, a32, t32};
use exarmo_core::decode::{Decoded, UNPREDICTABLE_MARK};

#[cfg(not(any(feature = "aarch64", feature = "aarch32")))]
compile_error!("exarmo-cli needs at least one of the aarch64 and aarch32 features");

/// The command's description, naming the instruction sets its features
/// build in.
const ABOUT: &str = if cfg!(all(feature = "aarch64", feature = "aarch32")) {
    "Disassemble AArch64, A32 and T32 instructions"
} else if cfg!(feature = "aarch64") {
    "Disassemble AArch64 instructions"
} else {
    "Disassemble A32 and T32 instructions"
};

#[derive(Parser)]
#[command(name = "exarmo", about = ABOUT)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Decode one word, or a word per line from the input
    Decode {
        /// Which instruction set the word is of
        set: Set,
        /// The word in hexadecimal, or `-` to read a word per line
        word: String,
        /// Read the digits as the bytes in memory order, as a byte dump shows
        /// them, rather than as the word's own value
        #[arg(long)]
        byteswap: bool,
        /// Print the instruction's fields beside its text
        #[arg(long)]
        debug: bool,
        /// Print the first ACLE intrinsic the instruction realises (a64)
        #[arg(long)]
        intrinsics: bool,
    },
}

/// Which decoder a word goes to.
#[derive(Clone, Copy, ValueEnum)]
enum Set {
    /// AArch64: a 32-bit word
    #[cfg(feature = "aarch64")]
    A64,
    /// A32: a 32-bit word
    #[cfg(feature = "aarch32")]
    A32,
    /// T32: a halfword in the high half of the word, hw1 above hw2
    #[cfg(feature = "aarch32")]
    T32,
}

/// What begins the line for a word that decodes to no instruction.
const ERROR: &str = "error: ";

/// What to print beside an instruction's text.
#[derive(Clone, Copy)]
struct Show {
    debug: bool,
    intrinsics: bool,
}

fn marked_text(instruction: &dyn Decoded) -> Result<String, fmt::Error> {
    let mut text = String::new();
    instruction.write_tokens_at(0, &mut text)?;
    if instruction.unpredictable() {
        text.push_str(UNPREDICTABLE_MARK);
    }
    Ok(text)
}

/// The line printed for one word. For an instruction, it is the text followed
/// by the intrinsic and the fields when they are asked for. Otherwise, it is
/// the reason the word is not an instruction. An instruction with no intrinsic
/// has `-` in that column, so the fields line up on every line.
fn rendered<I: Decoded, E: fmt::Debug>(
    decoded: Result<I, E>,
    show: Show,
    intrinsic: impl FnOnce(&I) -> Option<&'static str>,
) -> String {
    match decoded {
        Ok(instruction) => match marked_text(&instruction) {
            // A rendering fails where the operands view does not hold what it
            // asks of a slot, which a sweep counts and skips rather than
            // comparing a half-written line.
            Err(_) => "-".to_string(),
            Ok(mut text) => {
                if show.intrinsics {
                    let intrinsic = intrinsic(&instruction).unwrap_or("-");
                    text = format!("{text}\t{intrinsic}");
                }
                if show.debug {
                    text = format!("{text}\t{instruction:?}");
                }
                text
            }
        },
        // Every reason a word is no instruction is written the same way, so
        // a reader tells one from a decode by the prefix rather than by a
        // list of the reasons there are.
        Err(why) => format!("{ERROR}{why:?}"),
    }
}

fn decoded(set: Set, bits: u32, show: Show) -> String {
    match set {
        #[cfg(feature = "aarch64")]
        Set::A64 => rendered(exarmo_aarch64::decode_word(bits), show, |instruction| {
            let intrinsics = instruction.intrinsics();
            intrinsics.first().map(|intrinsic| intrinsic.name())
        }),
        #[cfg(feature = "aarch32")]
        Set::A32 => rendered(a32::decode_word(bits), show, |_| None),
        // Outside an IT block, which is what a word on its own is.
        #[cfg(feature = "aarch32")]
        Set::T32 => rendered(t32::decode_word(bits, ItState::Outside), show, |_| None),
    }
}

/// Whether the instruction set has intrinsics to show. ACLE's table is read
/// for AArch64 alone.
fn has_intrinsics(set: Set) -> bool {
    match set {
        #[cfg(feature = "aarch64")]
        Set::A64 => true,
        #[cfg(feature = "aarch32")]
        Set::A32 | Set::T32 => false,
    }
}

/// The word a hex argument names.
///
/// A byte dump writes the bytes in the order they sit in memory, which for a
/// 32-bit instruction is the value's own bytes reversed. T32 is swapped
/// halfword by halfword instead, since the architecture puts hw1 at the lower
/// address and stores each halfword little-endian, and a halfword on its own
/// goes where a caller passing the value would have written it.
fn word(set: Set, hex: &str, byteswap: bool) -> Result<u32, num::ParseIntError> {
    let bits = u32::from_str_radix(hex, 16)?;
    if !byteswap {
        return Ok(bits);
    }
    Ok(match set {
        #[cfg(feature = "aarch64")]
        Set::A64 => bits.swap_bytes(),
        #[cfg(feature = "aarch32")]
        Set::A32 => bits.swap_bytes(),
        #[cfg(feature = "aarch32")]
        Set::T32 if hex.len() <= 4 => u32::from((bits as u16).swap_bytes()) << 16,
        #[cfg(feature = "aarch32")]
        Set::T32 => {
            u32::from(((bits >> 16) as u16).swap_bytes()) << 16
                | u32::from((bits as u16).swap_bytes())
        }
    })
}

/// One word, or a word per line from the input for `-`.
fn decode(set: Set, hex: &str, byteswap: bool, show: Show) {
    if hex == "-" {
        let input = io::stdin().lock();
        let mut out = io::BufWriter::new(io::stdout().lock());
        for line in input.lines() {
            let line = line.expect("the input is readable");
            let hex = line.trim();
            if hex.is_empty() {
                continue;
            }
            match word(set, hex, byteswap) {
                Ok(bits) => writeln!(out, "{}", decoded(set, bits, show)),
                Err(_) => writeln!(out, "not a hexadecimal word"),
            }
            .expect("the output is writable");
        }
        return;
    }
    let bits = word(set, hex, byteswap).unwrap_or_else(|e| {
        eprintln!("not a hexadecimal word: {hex}: {e}");
        std::process::exit(1);
    });
    println!("{}", decoded(set, bits, show));
}

fn main() {
    match Cli::parse().command {
        Command::Decode {
            set,
            word,
            byteswap,
            debug,
            intrinsics,
        } => {
            if intrinsics && !has_intrinsics(set) {
                eprintln!("--intrinsics is for a64 alone");
                std::process::exit(2);
            }
            decode(set, &word, byteswap, Show { debug, intrinsics })
        }
    }
}
