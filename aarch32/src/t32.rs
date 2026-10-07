//! The T32 instruction set.

use exarmo_core::bytes::read_u16;

use crate::{DecodeError, Instruction, ItState};

pub use crate::generated::t32::{decode_word, length};

/// Decode the T32 instruction `bits`, its first 16 bits in the high half.
#[deprecated(
    since = "0.4.0",
    note = "renamed to `decode_word`. Use `decode_bytes` to decode from a byte slice"
)]
#[inline]
pub fn decode(bits: u32, state: ItState) -> Result<Instruction, DecodeError> {
    decode_word(bits, state)
}

/// Decode the T32 instruction at the start of `bytes`, under the IT state
/// `state`.
///
/// Reads either two or four bytes, depending on the size encoded in the first
/// 16 bits of the instruction. Returns [`DecodeError::Truncated`] if the bytes
/// end partway through the instruction.
///
/// ```
/// use exarmo_aarch32::{DecodeError, ItState, t32};
///
/// // bx lr
/// let bx = t32::decode_bytes(&[0x70, 0x47], ItState::Outside).unwrap();
/// assert_eq!(bx.at(0).to_string(), "bx\tlr");
///
/// // The first 16 bits of bl, which is 32 bits long
/// let bl = t32::decode_bytes(&[0x00, 0xf0], ItState::Outside);
/// assert_eq!(bl, Err(DecodeError::Truncated));
/// ```
pub fn decode_bytes(bytes: &[u8], state: ItState) -> Result<Instruction, DecodeError> {
    let first = read_u16(bytes)?;
    let mut word = u32::from(first) << 16;
    if length(first) == 4 {
        word |= u32::from(read_u16(&bytes[2..])?);
    }
    decode_word(word, state)
}

/// Decode each instruction of `bytes`, the first of which is at `address`
/// and in the IT state `state`.
///
/// Each item is an instruction's address and the result of decoding it. The
/// IT state advances from one instruction to the next using
/// [`ItState::after`], which assumes no branch intervenes. A word that is not
/// an instruction does not end the walk, and still uses up its place in an IT
/// block. If the buffer ends partway through an instruction, the last item is
/// [`DecodeError::Truncated`].
///
/// ```
/// use exarmo_aarch32::{ItState, t32};
///
/// // ite eq
/// // moveq r0, #1
/// // movne r0, #0
/// // bx lr
/// let code = [0x0c, 0xbf, 0x01, 0x20, 0x00, 0x20, 0x70, 0x47];
/// let text: Vec<_> = t32::disassemble(&code, 0x1000, ItState::Outside)
///     .map(|(address, instruction)| instruction.unwrap().at(address).to_string())
///     .collect();
/// assert_eq!(text, ["ite\teq", "moveq\tr0, #0x1", "movne\tr0, #0x0", "bx\tlr"]);
/// ```
pub fn disassemble(
    bytes: &[u8],
    address: u64,
    state: ItState,
) -> impl Iterator<Item = (u64, Result<Instruction, DecodeError>)> + '_ {
    let mut rest = bytes;
    let mut address = address;
    let mut state = state;
    core::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let result = decode_bytes(rest, state);
        let item_address = address;
        state = match &result {
            Ok(instruction) => state.after(instruction),
            Err(_) => state.advanced(),
        };
        let step = match read_u16(rest) {
            Ok(first) => usize::from(length(first)).min(rest.len()),
            Err(_) => rest.len(),
        };
        rest = &rest[step..];
        address = address.wrapping_add(step as u64);
        Some((item_address, result))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::{String, ToString};
    use alloc::vec::Vec;

    fn listing(code: &[u8]) -> Vec<(u64, Result<String, DecodeError>)> {
        disassemble(code, 0x1000, ItState::Outside)
            .map(|(address, result)| (address, result.map(|i| i.at(address).to_string())))
            .collect()
    }

    /// `itt eq` covers the next two instructions. The first is 0xb620, which
    /// no instruction is, and the second must still be the block's last.
    #[test]
    fn a_word_that_is_no_instruction_keeps_its_place_in_a_block() {
        let code = [0x04, 0xbf, 0x20, 0xb6, 0x01, 0x20, 0x01, 0x20];
        assert_eq!(
            listing(&code),
            [
                (0x1000, Ok("itt\teq".to_string())),
                (0x1002, Err(DecodeError::Unallocated)),
                (0x1004, Ok("moveq\tr0, #0x1".to_string())),
                (0x1006, Ok("movs\tr0, #0x1".to_string())),
            ]
        );
    }

    /// bl is 32 bits long, but only its first 16 bits follow `bx lr`.
    #[test]
    fn an_instruction_the_bytes_cut_short_is_truncated() {
        let code = [0x70, 0x47, 0x00, 0xf0];
        assert_eq!(
            listing(&code),
            [
                (0x1000, Ok("bx\tlr".to_string())),
                (0x1002, Err(DecodeError::Truncated)),
            ]
        );
    }

    #[test]
    fn a_lone_byte_is_truncated() {
        assert_eq!(listing(&[0x70]), [(0x1000, Err(DecodeError::Truncated))]);
    }

    /// bl is 32 bits, so the instruction after it is four bytes on.
    #[test]
    fn a_32_bit_instruction_steps_four_bytes() {
        let code = [0x00, 0xf0, 0x00, 0xf8, 0x70, 0x47];
        let addresses: Vec<_> = listing(&code).into_iter().map(|(a, _)| a).collect();
        assert_eq!(addresses, [0x1000, 0x1004]);
    }
}
