//! An instruction rendered as tokens.
//!
//! The generated formatter writes every instruction through a [`TokenSink`],
//! each token with its kind and the operand it belongs to. `Display` is the
//! sink that drops both and writes the text. A consumer that wants the
//! structure, such as a disassembler view that colours registers and links
//! addresses, supplies another. A number's token carries its value, as an
//! address token carries its address, so a consumer need not parse the text.

use core::fmt;

#[cfg(feature = "alloc")]
use alloc::string::{String, ToString};
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

/// What a token is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TokenKind {
    /// The mnemonic, or part of it. `b.eq` is written as `b.` and then `eq`,
    /// both of this kind.
    Mnemonic,
    /// Text that is none of the kinds below: the tab after the mnemonic,
    /// a `.` before an arrangement, a `!` for writeback, the `#` before an
    /// immediate, the `-` before a register offset the assembly subtracts,
    /// and the `, ` within an operand, as in `[x1, #0x8]` or
    /// `x2, lsl #0x3`. A subtracted immediate carries its sign in its value
    /// instead.
    Text,
    /// The `, ` between two operands, and only there, so that a consumer
    /// counting operands counts these. It belongs to no operand.
    Separator,
    /// A `[`, `]`, `{` or `}`.
    Bracket,
    /// A register, with any arrangement written on it: `x0`, `v1.4s`.
    Register,
    /// A name from a value table: a shift, an extend, a condition, a
    /// barrier option, a prefetch operation.
    Symbol,
    /// An immediate, with its value: `0x8`, `-0x10`. The `#` before it is
    /// a [`TokenKind::Text`] token of its own.
    Immediate(i64),
    /// A number written bare, with its value: an element index, a lane.
    Integer(u64),
    /// A floating-point immediate: `1.0`, after its `#`.
    Float,
    /// A branch target, with the address it names.
    Address(u64),
}

/// Which operand of `Instruction::operands` a token is part of.
///
/// The mnemonic, the tab after it and the `, ` between operands are part of
/// none, and every other token names the operand it is written on. A
/// memory operand's brackets belong to it, while the brackets of a lane
/// index belong to the register they follow.
///
/// The index is below that instruction set's `MAX_OPERANDS`.
pub type OperandIndex = Option<u8>;

/// Where the formatter writes its tokens.
pub trait TokenSink {
    /// Write a token whose text is fixed.
    fn text(&mut self, kind: TokenKind, operand: OperandIndex, text: &str) -> fmt::Result;
    /// Write a token whose text is a value's rendering.
    fn value(
        &mut self,
        kind: TokenKind,
        operand: OperandIndex,
        value: &dyn fmt::Display,
    ) -> fmt::Result;
}

/// Plain text, dropping the kinds.
impl TokenSink for fmt::Formatter<'_> {
    #[inline]
    fn text(&mut self, _kind: TokenKind, _operand: OperandIndex, text: &str) -> fmt::Result {
        self.write_str(text)
    }

    #[inline]
    fn value(
        &mut self,
        _kind: TokenKind,
        _operand: OperandIndex,
        value: &dyn fmt::Display,
    ) -> fmt::Result {
        value.fmt(self)
    }
}

/// A token as the `Vec<Token>` sink records it.
#[cfg(feature = "alloc")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    /// The token's kind.
    pub kind: TokenKind,
    /// Which operand it is part of, where it is part of one.
    pub operand: OperandIndex,
    /// The text as the formatter wrote it.
    pub text: String,
}

/// Records every token in order.
#[cfg(feature = "alloc")]
impl TokenSink for Vec<Token> {
    fn text(&mut self, kind: TokenKind, operand: OperandIndex, text: &str) -> fmt::Result {
        self.push(Token {
            kind,
            operand,
            text: text.to_string(),
        });
        Ok(())
    }

    fn value(
        &mut self,
        kind: TokenKind,
        operand: OperandIndex,
        value: &dyn fmt::Display,
    ) -> fmt::Result {
        self.push(Token {
            kind,
            operand,
            text: value.to_string(),
        });
        Ok(())
    }
}

/// Appends the text, dropping the kinds.
#[cfg(feature = "alloc")]
impl TokenSink for String {
    fn text(&mut self, _kind: TokenKind, _operand: OperandIndex, text: &str) -> fmt::Result {
        self.push_str(text);
        Ok(())
    }

    fn value(
        &mut self,
        _kind: TokenKind,
        _operand: OperandIndex,
        value: &dyn fmt::Display,
    ) -> fmt::Result {
        use fmt::Write;
        write!(self, "{value}")
    }
}

/// A number written in hex with its `0x`.
pub struct Hex<T>(pub T);

impl<T: fmt::LowerHex> fmt::Display for Hex<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:x}", self.0)
    }
}
