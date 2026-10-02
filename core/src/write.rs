//! Writing an instruction out as tokens.
//!
//! Each instruction set names its own registers and symbols and reads them
//! out of its operands. This writes everything else the same way for both,
//! including the mnemonic and the tab after it, the `, ` between operands,
//! the brackets, and the form of a number. A runtime wraps [`Writer`] in a
//! writer of its own, declared with `rendering_writer!`.

use core::fmt;
use core::fmt::Write as _;

use crate::address::PcRead;
use crate::tokens::{Hex, OperandIndex, TokenKind, TokenSink};

/// How a number is written. For a symbol, how a value its table does not name
/// is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    /// `#0x10`, an immediate.
    Hex,
    /// `#-0x10`, an immediate with its sign.
    Signed,
    /// `3`, a bare integer.
    Decimal,
}

/// A number written in hexadecimal with its sign before the `0x`.
///
/// `negative` carries the sign apart from the value because a subtracted zero
/// is written `-0x0`. That is a distinct word, with the U bit clear, which
/// `#0` would not assemble to.
pub struct SignedHex {
    /// The value, signed.
    pub value: i64,
    /// Whether the assembly writes it subtracted.
    pub negative: bool,
}

impl fmt::Display for SignedHex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.negative || self.value < 0 {
            true => write!(f, "-0x{:x}", self.value.unsigned_abs()),
            false => write!(f, "0x{:x}", self.value),
        }
    }
}

/// The floating-point value of the low `width` bits of `bits`, for a width of
/// 16, 32 or 64.
///
/// Rust has no stable half-precision float, so a 16-bit constant is decoded
/// as IEEE 754 binary16 by hand.
pub fn float_from_bits(bits: u64, width: u32) -> f64 {
    match width {
        16 => {
            let half = bits as u16;
            let sign = if half >> 15 == 1 { -1.0 } else { 1.0 };
            let exponent = (half >> 10) & 0x1f;
            let fraction = f64::from(half & 0x3ff);
            match exponent {
                0 => sign * fraction * power_of_two(-24),
                0x1f if fraction == 0.0 => sign * f64::INFINITY,
                0x1f => f64::NAN,
                _ => sign * (1.0 + fraction / 1024.0) * power_of_two(i32::from(exponent) - 15),
            }
        }
        32 => f64::from(f32::from_bits(bits as u32)),
        _ => f64::from_bits(bits),
    }
}

/// `2f64.powi(exponent)` without `std`. `exponent` must be in `-1022..=1023`.
const fn power_of_two(exponent: i32) -> f64 {
    debug_assert!(-1022 <= exponent && exponent <= 1023);
    f64::from_bits(((exponent + 1023) as u64) << 52)
}

/// A floating-point number, written so that a whole one keeps its point.
pub struct Float<T>(pub T);

impl<T: fmt::Display> fmt::Display for Float<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = DetectDecimalPoint { f, found: false };
        write!(out, "{}", self.0)?;
        match out.found {
            true => Ok(()),
            false => f.write_str(".0"),
        }
    }
}

/// Passes text through, noting whether it held a point or an exponent.
struct DetectDecimalPoint<'a, 'b> {
    f: &'a mut fmt::Formatter<'b>,
    found: bool,
}

impl fmt::Write for DetectDecimalPoint<'_, '_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.found |= s.contains(['.', 'e', 'E']);
        self.f.write_str(s)
    }
}

/// Writes an instruction's tokens to a sink.
///
/// Its methods are kept out of line so that each piece of a rendering is one
/// call. There are a few thousand renderings, and inlining the writing into
/// each would cost code size without gaining speed.
pub struct Writer<'a> {
    sink: &'a mut dyn TokenSink,
    name: &'static str,
    address: u64,
    /// How wide an address is in this instruction set, so a target running
    /// off either end wraps as the architecture's PC does.
    bits: u32,
    /// The operand the tokens being written are part of, which a rendering
    /// sets as it reaches each one.
    operand: OperandIndex,
}

impl<'a> Writer<'a> {
    /// A writer for an instruction with the mnemonic, at the address, in an
    /// instruction set whose addresses are `bits` wide.
    pub fn new(sink: &'a mut dyn TokenSink, name: &'static str, address: u64, bits: u32) -> Self {
        Writer {
            sink,
            name,
            address,
            bits,
            operand: None,
        }
    }

    /// Which operand the tokens after this are part of, or none for the
    /// text between operands. A [`Writer::separator`] clears it.
    #[inline]
    pub fn operand(&mut self, operand: OperandIndex) {
        self.operand = operand;
    }

    /// The instruction's mnemonic.
    #[inline(never)]
    pub fn name(&mut self) -> fmt::Result {
        self.sink.text(TokenKind::Mnemonic, self.operand, self.name)
    }

    /// A piece of a mnemonic written literally: the `b.` of `b.eq`.
    #[inline(never)]
    pub fn mnemonic(&mut self, text: &str) -> fmt::Result {
        self.sink.text(TokenKind::Mnemonic, self.operand, text)
    }

    /// Plain text: the tab after the mnemonic, a `.`, a `!`.
    #[inline(never)]
    pub fn text(&mut self, text: &str) -> fmt::Result {
        self.sink.text(TokenKind::Text, self.operand, text)
    }

    /// The `, ` between operands, which belongs to no operand.
    #[inline(never)]
    pub fn separator(&mut self) -> fmt::Result {
        self.operand = None;
        self.sink.text(TokenKind::Separator, None, ", ")
    }

    /// The `, ` between the parts of one operand: the members of a
    /// register list, the base and the offset of a memory operand, a
    /// register and the shift written on it. It is text of the operand it is
    /// written inside, so that a consumer counting operands can count
    /// separators.
    #[inline(never)]
    pub fn inner_separator(&mut self) -> fmt::Result {
        self.sink.text(TokenKind::Text, self.operand, ", ")
    }

    /// A bracket or brace.
    #[inline(never)]
    pub fn bracket(&mut self, text: &str) -> fmt::Result {
        self.sink.text(TokenKind::Bracket, self.operand, text)
    }

    /// A number in the given form. An immediate's `#` is a text token of its
    /// own.
    #[inline(never)]
    pub fn number(&mut self, value: i64, form: Form) -> fmt::Result {
        match form {
            Form::Hex => {
                self.sink.text(TokenKind::Text, self.operand, "#")?;
                self.sink.value(
                    TokenKind::Immediate(value),
                    self.operand,
                    &Hex(value as u64),
                )
            }
            Form::Signed => self.signed(value, false),
            Form::Decimal => {
                self.sink
                    .value(TokenKind::Integer(value as u64), self.operand, &value)
            }
        }
    }

    /// A floating-point number, after its `#`.
    #[inline(never)]
    pub fn float(&mut self, value: impl fmt::Display) -> fmt::Result {
        self.sink.text(TokenKind::Text, self.operand, "#")?;
        self.sink
            .value(TokenKind::Float, self.operand, &Float(value))
    }

    /// A register, however the instruction set writes one.
    #[inline(never)]
    pub fn register(&mut self, reg: &dyn fmt::Display) -> fmt::Result {
        self.sink.value(TokenKind::Register, self.operand, reg)
    }

    /// A register the view may not hold. A missing one is a formatting error
    /// rather than half a line of text.
    #[inline(never)]
    pub fn some_register<R: fmt::Display>(&mut self, reg: Option<R>) -> fmt::Result {
        self.register(&reg.ok_or(fmt::Error)?)
    }

    /// A number a reading may not have found, as [`Writer::some_register`]
    /// is for a register.
    #[inline(never)]
    pub fn some_number(&mut self, value: Option<i64>, form: Form) -> fmt::Result {
        self.number(value.ok_or(fmt::Error)?, form)
    }

    /// A name from a value table: a shift, a condition, a data type.
    #[inline(never)]
    pub fn symbol(&mut self, name: &dyn fmt::Display) -> fmt::Result {
        self.sink.value(TokenKind::Symbol, self.operand, name)
    }

    /// A token of a kind the instruction set chooses, for a register file or a
    /// symbol the other methods do not cover.
    #[inline(never)]
    pub fn token(&mut self, kind: TokenKind, text: &str) -> fmt::Result {
        self.sink.text(kind, self.operand, text)
    }

    /// A token whose text is a value's own rendering, of a kind the instruction
    /// set names itself.
    #[inline(never)]
    pub fn valued(&mut self, kind: TokenKind, value: &dyn fmt::Display) -> fmt::Result {
        self.sink.value(kind, self.operand, value)
    }

    /// A number after its `#`, written as [`SignedHex`].
    #[inline(never)]
    pub fn signed(&mut self, value: i64, negative: bool) -> fmt::Result {
        self.sink.text(TokenKind::Text, self.operand, "#")?;
        self.sink.value(
            TokenKind::Immediate(value),
            self.operand,
            &SignedHex { value, negative },
        )
    }

    /// An address, written in hex and carrying the address it names.
    #[inline(never)]
    pub fn address(&mut self, target: u64) -> fmt::Result {
        self.sink
            .value(TokenKind::Address(target), self.operand, &Hex(target))
    }

    /// A branch target or a literal's address, written as the PC the
    /// instruction reads plus `offset`, wrapped to the address width.
    #[inline(never)]
    pub fn label(&mut self, offset: i64, pc: PcRead) -> fmt::Result {
        self.address(pc.target(self.address, offset, self.bits))
    }
}

/// Declares a runtime's rendering writer. It wraps the shared [`Writer`] with
/// the instruction set's address width fixed and derefs to it, and the
/// runtime adds the methods that read its own operands.
///
/// ```
/// # mod runtime {
/// exarmo_core::rendering_writer!(
///     /// Where a rendering of this instruction set writes.
///     Writer, 32
/// );
/// # }
/// ```
#[macro_export]
macro_rules! rendering_writer {
    ($(#[$doc:meta])* $name:ident, $bits:literal) => {
        $(#[$doc])*
        pub struct $name<'a> {
            out: $crate::write::Writer<'a>,
        }

        impl<'a> ::core::ops::Deref for $name<'a> {
            type Target = $crate::write::Writer<'a>;
            fn deref(&self) -> &Self::Target {
                &self.out
            }
        }

        impl<'a> ::core::ops::DerefMut for $name<'a> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.out
            }
        }

        impl<'a> $name<'a> {
            /// A writer for an instruction with the mnemonic, at the address.
            pub fn new(
                sink: &'a mut dyn $crate::tokens::TokenSink,
                name: &'static str,
                address: u64,
            ) -> Self {
                $name {
                    out: $crate::write::Writer::new(sink, name, address, $bits),
                }
            }
        }
    };
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::{Float, Form, Writer, float_from_bits};
    use crate::address::PcRead;
    use crate::tokens::{Token, TokenKind};
    use alloc::string::{String, ToString};
    use alloc::vec;
    use alloc::vec::Vec;
    use core::fmt;

    /// What a writer wrote, as its tokens and as the text they come to.
    fn written(
        at: u64,
        bits: u32,
        write: impl FnOnce(&mut Writer) -> fmt::Result,
    ) -> (Vec<Token>, String) {
        let mut sink: Vec<Token> = Vec::new();
        let mut writer = Writer::new(&mut sink, "b", at, bits);
        write(&mut writer).expect("a Vec sink cannot fail");
        let text = sink.iter().map(|t| t.text.as_str()).collect();
        (sink, text)
    }

    #[test]
    fn a_decimal_is_written_bare() {
        let (tokens, text) = written(0, 64, |w| w.number(3, Form::Decimal));
        assert_eq!(text, "3");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Integer(3));
    }

    #[test]
    fn a_subtracted_zero_keeps_its_sign() {
        assert_eq!(written(0, 64, |w| w.signed(0, true)).1, "#-0x0");
        assert_eq!(written(0, 64, |w| w.signed(0, false)).1, "#0x0");
        assert_eq!(written(0, 64, |w| w.signed(16, true)).1, "#-0x10");
    }

    #[test]
    fn a_signed_form_writes_the_value_it_has() {
        assert_eq!(written(0, 64, |w| w.number(-16, Form::Signed)).1, "#-0x10");
    }

    #[test]
    fn a_target_wraps_within_the_instruction_set_s_reach() {
        let pc = PcRead { ahead: 0, align: 1 };
        assert_eq!(
            written(0xffff_fffc, 32, |w| w.label(0x10, pc)).1,
            "0xc",
            "a 32-bit target past the end wraps to the bottom"
        );
        assert_eq!(written(0x10, 32, |w| w.label(-0x20, pc)).1, "0xfffffff0");
    }

    #[test]
    fn each_piece_carries_its_kind() {
        let (tokens, text) = written(0, 64, |w| {
            w.name()?;
            w.text("\t")?;
            w.register(&"x0")?;
            w.separator()?;
            w.operand(Some(1));
            w.bracket("[")?;
            w.register(&"x1")?;
            w.inner_separator()?;
            w.symbol(&"lsl")?;
            w.bracket("]")
        });
        assert_eq!(text, "b\tx0, [x1, lsl]");
        let kinds: Vec<(TokenKind, Option<u8>)> =
            tokens.iter().map(|t| (t.kind, t.operand)).collect();
        assert_eq!(
            kinds,
            vec![
                (TokenKind::Mnemonic, None),
                (TokenKind::Text, None),
                (TokenKind::Register, None),
                (TokenKind::Separator, None),
                (TokenKind::Bracket, Some(1)),
                (TokenKind::Register, Some(1)),
                // Within an operand, a comma is its text
                (TokenKind::Text, Some(1)),
                (TokenKind::Symbol, Some(1)),
                (TokenKind::Bracket, Some(1)),
            ]
        );
    }

    #[test]
    fn a_whole_number_keeps_its_point() {
        assert_eq!(Float(2.0).to_string(), "2.0");
        assert_eq!(Float(-0.125).to_string(), "-0.125");
    }

    /// `f64` writes no exponent, so 1e100 is 101 digits, longer than any
    /// immediate the architecture encodes.
    #[test]
    fn a_long_number_is_written_whole() {
        let written = Float(1e100).to_string();
        assert_eq!(written.len(), 103);
        assert!(written.starts_with('1') && written.ends_with("0.0"));
    }

    /// The smallest denormal at each width.
    #[test]
    fn denormal() {
        assert_eq!(float_from_bits(0x0001, 16), 2f64.powi(-24));
        assert_eq!(float_from_bits(0x0000_0001, 32), 2f64.powi(-149));
        // 2^-1074 is itself subnormal, so it is named by its bits rather
        // than computed, which would round to zero.
        assert_eq!(
            float_from_bits(0x0000_0000_0000_0001, 64),
            f64::from_bits(1)
        );
    }

    #[test]
    fn infinity() {
        assert_eq!(float_from_bits(0x7c00, 16), f64::INFINITY);
        assert_eq!(float_from_bits(0xfc00, 16), f64::NEG_INFINITY);
        assert_eq!(float_from_bits(0x7f80_0000, 32), f64::INFINITY);
        assert_eq!(float_from_bits(0x7ff0_0000_0000_0000, 64), f64::INFINITY);
    }

    #[test]
    fn nan() {
        assert!(float_from_bits(0x7e00, 16).is_nan());
        assert!(float_from_bits(0x7fc0_0000, 32).is_nan());
        assert!(float_from_bits(0x7ff8_0000_0000_0000, 64).is_nan());
    }
}
