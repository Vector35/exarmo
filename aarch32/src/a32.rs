//! The A32 instruction set.

use exarmo_core::bytes::read_u32;

use crate::{DecodeError, Instruction};

pub use crate::generated::a32::decode_word;

/// Decode the 32-bit A32 instruction word `bits`.
#[deprecated(
    since = "0.4.0",
    note = "renamed to `decode_word`. Use `decode_bytes` to decode from a byte slice"
)]
#[inline]
pub fn decode(bits: u32) -> Result<Instruction, DecodeError> {
    decode_word(bits)
}

/// Decode the little-endian A32 instruction in the first four bytes of
/// `bytes`.
///
/// Returns [`DecodeError::Truncated`] if `bytes` holds fewer than four bytes.
///
/// ```
/// use exarmo_aarch32::{DecodeError, a32};
///
/// // bx lr
/// let bx = a32::decode_bytes(&[0x1e, 0xff, 0x2f, 0xe1]).unwrap();
/// assert_eq!(bx.at(0).to_string(), "bx\tlr");
///
/// assert_eq!(a32::decode_bytes(&[0x1e, 0xff]), Err(DecodeError::Truncated));
/// ```
pub fn decode_bytes(bytes: &[u8]) -> Result<Instruction, DecodeError> {
    read_u32(bytes).and_then(decode_word)
}

/// Decode each instruction of `bytes`, the first of which is at `address`.
///
/// Each item is an instruction's address and the result of decoding it. A
/// word that is not an instruction does not end the walk. If the buffer ends
/// partway through a word, the last item is [`DecodeError::Truncated`].
///
/// ```
/// use exarmo_aarch32::a32;
///
/// // push {r4, lr}
/// // bx lr
/// let code = [0x10, 0x40, 0x2d, 0xe9, 0x1e, 0xff, 0x2f, 0xe1];
/// let text: Vec<_> = a32::disassemble(&code, 0x1000)
///     .map(|(address, instruction)| instruction.unwrap().at(address).to_string())
///     .collect();
/// assert_eq!(text, ["push\t{r4, lr}", "bx\tlr"]);
/// ```
pub fn disassemble(
    bytes: &[u8],
    address: u64,
) -> impl Iterator<Item = (u64, Result<Instruction, DecodeError>)> + '_ {
    bytes
        .chunks(4)
        .zip((address..).step_by(4))
        .map(|(word, address)| (address, read_u32(word).and_then(decode_word)))
}
