//! The result of a decode in either runtime.

use core::fmt;

use crate::branch::Branch;
use crate::flags::FlagEffect;
use crate::tokens::TokenSink;

/// A decoded instruction, whichever instruction set read it.
///
/// This lets a consumer of both instruction sets be written once, without
/// naming either runtime's `Instruction`. It holds only what both instruction
/// sets share. Register files, arrangements and data types belong to each
/// runtime's own types.
///
/// [`decoded!`](crate::decoded) implements it for both runtimes.
pub trait Decoded: fmt::Debug {
    /// Write the instruction as tokens, as it reads at this address.
    fn write_tokens_at(&self, address: u64, sink: &mut dyn TokenSink) -> fmt::Result;

    /// The instruction's length in bytes. This is four in A64 and A32, and
    /// two or four in T32.
    fn length(&self) -> u8;

    /// What the instruction does with the condition flags, as its Execute
    /// pseudocode says.
    fn flags(&self) -> FlagEffect;

    /// What the instruction does to the flow of control, with any label
    /// resolved against this address.
    fn branch_at(&self, address: u64) -> Branch;

    /// The encoding's name as the encoding index writes it, such as
    /// `AddAddsubImm` or `ADC_r_A1`.
    fn encoding_name(&self) -> &'static str;

    /// Whether the word is CONSTRAINED UNPREDICTABLE, and still decodes as
    /// the instruction.
    fn unpredictable(&self) -> bool;

    /// The instruction's text at this address, followed by
    /// [`UNPREDICTABLE_MARK`] where its word is CONSTRAINED UNPREDICTABLE.
    fn marked_text_at(&self, address: u64) -> Result<String, fmt::Error> {
        let mut text = String::new();
        self.write_tokens_at(address, &mut text)?;
        if self.unpredictable() {
            text.push_str(UNPREDICTABLE_MARK);
        }
        Ok(text)
    }
}

/// What follows an instruction's text where its word is CONSTRAINED
/// UNPREDICTABLE, as the command writes it and the corpus expects it.
pub const UNPREDICTABLE_MARK: &str = " (unpredictable)";

/// Implements [`Decoded`] for a runtime's generated `Instruction`.
#[macro_export]
macro_rules! decoded {
    ($instruction:ty) => {
        impl $crate::decode::Decoded for $instruction {
            fn write_tokens_at(
                &self,
                address: u64,
                sink: &mut dyn $crate::tokens::TokenSink,
            ) -> ::core::fmt::Result {
                self.at(address).write_tokens(sink)
            }

            fn length(&self) -> u8 {
                self.encoding().length()
            }

            fn flags(&self) -> $crate::flags::FlagEffect {
                <$instruction>::flags(self)
            }

            fn branch_at(&self, address: u64) -> $crate::branch::Branch {
                self.at(address).branch()
            }

            fn encoding_name(&self) -> &'static str {
                self.encoding().name()
            }

            fn unpredictable(&self) -> bool {
                <$instruction>::unpredictable(self)
            }
        }
    };
}

/// Why a word decoded to no instruction.
///
/// Only the AArch32 encoding index marks rows UNPREDICTABLE or as reserved
/// hints. The A64 index does not, so such words decode as `Unallocated`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DecodeError {
    /// The architecture defines the encoding as UNDEFINED, which its own decode
    /// pseudocode says by reaching `Undefined()`.
    UNDEF,
    /// The architecture defines the encoding as a NOP, which its own decode
    /// pseudocode says by reaching `ExecuteAsNOP()`.
    NOP,
    /// The encoding is allocated to no instruction, either because no row of
    /// the index claims it or because a field holds a value its value table
    /// reserves.
    Unallocated,
    /// The index marks the encoding UNPREDICTABLE.
    Unpredictable,
    /// The index marks the encoding a reserved hint, which behaves as a NOP
    /// and may be allocated later.
    ReservedHint,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UNDEF => f.write_str("UNDEFINED"),
            Self::NOP => f.write_str("NOP"),
            Self::Unallocated => f.write_str("UNALLOCATED"),
            Self::Unpredictable => f.write_str("UNPREDICTABLE"),
            Self::ReservedHint => f.write_str("reserved hint"),
        }
    }
}
