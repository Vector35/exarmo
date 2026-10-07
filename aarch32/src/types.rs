//! What an AArch32 instruction holds beyond what the generated code declares:
//! the four register files, one element of a D register, the IT state a
//! T32 decode is told and advances, and the system register a coprocessor
//! instruction reaches.

use crate::{Cond, Instruction, SysRegSpace};
use exarmo_core::register;

register!(
    /// A 32-bit SIMD and floating-point register, `s0` to `s31`.
    ///
    /// The three SIMD files are views of one bank. `s0` and `s1` are the halves
    /// of `d0`, and `d0` and `d1` the halves of `q0`. An instruction's prose
    /// says which one it names, as "the 64-bit name" and so on.
    SReg, 32, "s"
);

register!(
    /// A 64-bit SIMD and floating-point register, `d0` to `d31`.
    DReg, 32, "d"
);

register!(
    /// A 128-bit SIMD and floating-point register, `q0` to `q15`.
    QReg, 16, "q"
);

/// One element of a D register, which the by-scalar instructions write as
/// `d0[1]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Scalar {
    /// The register.
    pub reg: DReg,
    /// Which element of it.
    pub lane: u8,
}

/// Whether a T32 instruction sits inside an IT block, and under what
/// condition.
///
/// A 16-bit T32 encoding has no condition field. The condition it is written
/// with comes from a preceding IT instruction, which ARM's own decode
/// pseudocode reads through `InITBlock()`, so it is something a decode is told
/// rather than something a word says.
///
/// [`ItState::Unknown`] is for a caller that has not tracked the blocks, and
/// reads as being outside one, which is what Binary Ninja's own ARMv7 plugin
/// does with its `IFTHEN_UNKNOWN`. Advancing the state across a block is the
/// caller's, since it depends on control flow a single instruction cannot
/// see. [`ItState::after`] advances it along the straight line, as the
/// architecture's `ITAdvance` does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ItState {
    /// The caller has not tracked the blocks. Read as being outside one.
    #[default]
    Unknown,
    /// Not inside an IT block, so the instruction is unconditional.
    Outside,
    /// Inside an IT block, under this condition, with the rest of the block
    /// ahead.
    Inside {
        /// The condition this instruction runs under.
        cond: Cond,
        /// What is left of the block, as the low four bits of the
        /// architecture's ITSTATE. That is the mask the IT instruction was
        /// written with, shifted up once for each instruction since. Its
        /// lowest set bit marks the block's end, so `0b1000` is the last
        /// instruction. Each bit above that says whether the next
        /// instruction's condition is this one, where the bit equals the
        /// condition's low bit, or its inverse. A caller that knows only
        /// whether the instruction is the last passes `0b1000` for the last
        /// and `0b0100` otherwise.
        mask: u8,
    },
}

impl ItState {
    /// The state ITSTATE's bits hold, with the condition at 7:4 and the mask
    /// at 3:0. A mask of zero is outside any block.
    ///
    /// A condition of `1111` is read as `AL`, as the architecture reads it
    /// where an IT instruction is written with it.
    pub const fn from_bits(itstate: u8) -> Self {
        let mask = itstate & 0xf;
        if mask == 0 {
            return ItState::Outside;
        }
        let cond = match Cond::from_bits(itstate >> 4) {
            Some(cond) => cond,
            None => Cond::Al,
        };
        ItState::Inside { cond, mask }
    }

    /// The state as ITSTATE's bits, zero outside a block.
    pub const fn bits(self) -> u8 {
        match self {
            ItState::Inside { cond, mask } => (cond as u8) << 4 | (mask & 0xf),
            ItState::Unknown | ItState::Outside => 0,
        }
    }

    /// The state the instruction after this one is in, on the straight
    /// line. An IT instruction begins its block, and inside a block the
    /// state advances as the architecture's `ITAdvance` has it. A branch out
    /// of a block is the caller's to see. An unknown state stays unknown
    /// until an IT instruction settles it.
    pub fn after(self, instruction: &Instruction) -> Self {
        match instruction.it_state_set() {
            Some(block) => block,
            None => self.advanced(),
        }
    }

    /// The state after any instruction other than IT, advanced as
    /// `ITAdvance` does. A word that decodes to no instruction still uses up
    /// its place in a block, so `t32::disassemble` advances past it too.
    pub(crate) fn advanced(self) -> Self {
        match self {
            ItState::Inside { .. } => {
                let bits = self.bits();
                if bits & 0b111 == 0 {
                    ItState::Outside
                } else {
                    ItState::from_bits(bits & 0xe0 | (bits << 1) & 0x1f)
                }
            }
            ItState::Unknown | ItState::Outside => self,
        }
    }

    /// The condition an instruction here is written with.
    ///
    /// [`Cond::Al`] outside a block or in an unknown state.
    pub const fn condition(self) -> Cond {
        match self {
            ItState::Inside { cond, .. } => cond,
            ItState::Unknown | ItState::Outside => Cond::Al,
        }
    }

    /// Whether an instruction here is inside an IT block, which is what ARM's
    /// `InITBlock()` asks.
    pub const fn in_block(self) -> bool {
        matches!(self, ItState::Inside { .. })
    }

    /// Whether an instruction here is the last of an IT block, which is what
    /// ARM's `LastInITBlock()` asks.
    pub const fn last_in_block(self) -> bool {
        matches!(self, ItState::Inside { mask: 0b1000, .. })
    }
}

register!(
    /// One of the sixteen general-purpose registers.
    ///
    /// R13, R14 and R15 are written `sp`, `lr` and `pc`, which is how the
    /// architecture reads them and what Binary Ninja's own ARMv7 plugin
    /// prints. The `sb`, `sl`, `fp` and `ip` aliases for R9 to R12 belong to
    /// a calling convention, not to the instruction, and are not written.
    GpReg, 16, "r", 13 => "sp", 14 => "lr", 15 => "pc"
);

/// A system register an MRC, MCR, MRRC or MCRR accesses, by its key in the
/// table of the index naming it, and the direction of the access.
///
/// The direction is part of naming it, as it is for AArch64's registers. Two
/// registers can share a key where one is read-only and the other
/// write-only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SysReg {
    space: SysRegSpace,
    encoding: u32,
    write: bool,
}

impl SysReg {
    /// A register by its space, its key and whether the access writes it.
    pub const fn new(space: SysRegSpace, encoding: u32, write: bool) -> Self {
        Self {
            space,
            encoding,
            write,
        }
    }

    /// Which table of the index the key is in.
    pub const fn space(self) -> SysRegSpace {
        self.space
    }

    /// The key, the fields the instruction writes run together in the order
    /// the index gives them, as `coproc:opc1:CRn:CRm:opc2` for MRC.
    pub const fn encoding(self) -> u32 {
        self.encoding
    }

    /// Whether this access writes the register.
    pub const fn is_write(self) -> bool {
        self.write
    }

    /// The architectural name of this register, if it has one in this
    /// direction.
    pub fn name(self) -> Option<&'static str> {
        self.space.register(self.encoding, self.write)
    }

    /// Every system register the index names for these instructions, a
    /// space at a time and in key order within one.
    pub fn all() -> impl Iterator<Item = SysRegDef> {
        (0..SysRegDef::COUNT).map(SysRegDef::at)
    }
}

/// A system register the architecture names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SysRegDef {
    /// Which table of the index names it.
    pub space: SysRegSpace,
    /// Its key, as [`SysReg::encoding`] gives it.
    pub encoding: u32,
    /// Whether it can be read under this name.
    pub readable: bool,
    /// Whether it can be written under this name.
    pub writable: bool,
    /// The name, in lower case.
    pub name: &'static str,
}

impl SysRegDef {
    /// How many registers the architecture names.
    pub const COUNT: usize = crate::generated::sysreg::COUNT;

    /// The register at `index` of [`SysReg::all`], which must be below
    /// [`SysRegDef::COUNT`]. A `const fn` so that a C face can lay the table
    /// out as a constant.
    pub const fn at(index: usize) -> SysRegDef {
        let (space, encoding, readable, writable, name) = crate::generated::sysreg::entry(index);
        SysRegDef {
            space,
            encoding,
            readable,
            writable,
            name,
        }
    }
}
