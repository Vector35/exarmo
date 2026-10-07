//! Reading instructions from byte slices.
//!
//! AArch64 and AArch32 instructions are always little-endian, whatever the
//! endianness of data. A T32 instruction is one or two little-endian 16-bit
//! values, with the first at the lower address.

use crate::DecodeError;

/// The 32-bit value the first four bytes hold.
pub fn read_u32(bytes: &[u8]) -> Result<u32, DecodeError> {
    match bytes.first_chunk::<4>() {
        Some(word) => Ok(u32::from_le_bytes(*word)),
        None => Err(DecodeError::Truncated),
    }
}

/// The 16-bit value the first two bytes hold.
pub fn read_u16(bytes: &[u8]) -> Result<u16, DecodeError> {
    match bytes.first_chunk::<2>() {
        Some(unit) => Ok(u16::from_le_bytes(*unit)),
        None => Err(DecodeError::Truncated),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_word_is_read_little_endian() {
        assert_eq!(read_u32(&[0x1f, 0x20, 0x03, 0xd5, 0xff]), Ok(0xd503201f));
        assert_eq!(read_u16(&[0x70, 0x47]), Ok(0x4770));
    }

    #[test]
    fn too_few_bytes_are_truncated() {
        assert_eq!(read_u32(&[0x1f, 0x20, 0x03]), Err(DecodeError::Truncated));
        assert_eq!(read_u16(&[0x70]), Err(DecodeError::Truncated));
        assert_eq!(read_u16(&[]), Err(DecodeError::Truncated));
    }
}
