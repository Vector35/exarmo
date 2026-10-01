//! Where an instruction's label points, as the architecture reads the PC.
//!
//! The formatter writes a label as the address it names, a branch reports
//! that address as its target, and each C face hands the PC reading over for
//! a caller to do the same. All three take it from here, so they cannot
//! disagree about where a label goes.

/// How an instruction reads the PC for a label. The architecture reads the PC
/// ahead of the instruction, and a literal load or a page-relative address
/// aligns it down first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PcRead {
    /// How many bytes ahead of the instruction the PC reads: 8 in A32, 4 in
    /// T32, none in A64.
    pub ahead: u8,
    /// What the PC is aligned down to before the offset is added, as a power
    /// of two in bytes: 4 for a literal load, 4096 for ADRP's page, and 1
    /// where the PC is read as it is.
    pub align: u32,
}

impl PcRead {
    /// The PC an instruction at the address reads.
    pub const fn at(self, address: u64) -> u64 {
        let pc = address.wrapping_add(self.ahead as u64);
        pc & !(self.align as u64 - 1)
    }

    /// The address a label at this offset names, for an instruction at
    /// `address` in an instruction set whose addresses are `bits` wide.
    pub const fn target(self, address: u64, offset: i64, bits: u32) -> u64 {
        within(self.at(address).wrapping_add(offset as u64), bits)
    }
}

/// An address held to the instruction set's width, so one running off
/// either end wraps as the architecture's PC does.
pub const fn within(address: u64, bits: u32) -> u64 {
    address & (u64::MAX >> (64 - bits))
}

#[cfg(test)]
mod tests {
    use super::PcRead;

    #[test]
    fn the_pc_is_read_as_the_instruction_reads_it() {
        // A64 reads its own address
        assert_eq!(PcRead { ahead: 0, align: 1 }.at(0x1004), 0x1004);
        // A32 reads eight bytes ahead, T32 four
        assert_eq!(PcRead { ahead: 8, align: 1 }.at(0x1000), 0x1008);
        assert_eq!(PcRead { ahead: 4, align: 1 }.at(0x1002), 0x1006);
        // A literal load aligns down after reading ahead. 0x1002 + 4 is
        // 0x1006, which aligns down to 0x1004
        assert_eq!(PcRead { ahead: 4, align: 4 }.at(0x1002), 0x1004);
        // ADRP names the page the target is in
        assert_eq!(
            PcRead {
                ahead: 0,
                align: 4096
            }
            .at(0x1234),
            0x1000
        );
    }
}
