//! Decoding instructions from byte slices.

use exarmo_core::bytes::read_u32;

use crate::{DecodeError, Instruction, decode_word};

/// Decode the little-endian instruction in the first four bytes of `bytes`.
///
/// Returns [`DecodeError::Truncated`] if `bytes` holds fewer than four bytes.
///
/// ```
/// use exarmo_aarch64::{DecodeError, decode_bytes};
///
/// // nop
/// let nop = decode_bytes(&[0x1f, 0x20, 0x03, 0xd5]).unwrap();
/// assert_eq!(nop.at(0).to_string(), "nop");
///
/// assert_eq!(decode_bytes(&[0x1f, 0x20]), Err(DecodeError::Truncated));
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
/// use exarmo_aarch64::{DecodeError, disassemble};
///
/// // str x0, [sp, #-16]!
/// // ldr x0, [sp], #16
/// // and two bytes more
/// let code = [0xe0, 0x0f, 0x1f, 0xf8, 0xe0, 0x07, 0x41, 0xf8, 0x1f, 0x20];
/// let mut listing = disassemble(&code, 0x1000);
///
/// let (address, str) = listing.next().unwrap();
/// assert_eq!(address, 0x1000);
/// assert_eq!(str.unwrap().at(address).to_string(), "str\tx0, [sp, #-0x10]!");
///
/// let (address, ldr) = listing.next().unwrap();
/// assert_eq!(address, 0x1004);
/// assert_eq!(ldr.unwrap().at(address).to_string(), "ldr\tx0, [sp], #0x10");
///
/// assert_eq!(listing.next(), Some((0x1008, Err(DecodeError::Truncated))));
/// assert_eq!(listing.next(), None);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Encoding;
    use alloc::vec::Vec;

    /// No row of the exception generation class allocates 0xd478da60, and
    /// the nop after it is still decoded.
    #[test]
    fn a_word_that_is_no_instruction_does_not_end_the_walk() {
        let code = [0x60, 0xda, 0x78, 0xd4, 0x1f, 0x20, 0x03, 0xd5];
        let walked: Vec<_> = disassemble(&code, 0x1000)
            .map(|(address, result)| (address, result.map(|i| i.encoding())))
            .collect();
        assert_eq!(
            walked,
            [
                (0x1000, Err(DecodeError::Unallocated)),
                (0x1004, Ok(Encoding::NopHiHints)),
            ]
        );
    }
}
