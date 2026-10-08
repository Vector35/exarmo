//! An instruction's operands, seen without matching on its variant.
//!
//! `Instruction` types each operand for what it is, one variant per encoding. A
//! consumer that cannot match on every variant reads [`Instruction::operands`],
//! the operands in the order the assembly writes them, each an [`Operand`],
//! with the structure the template implied made explicit. A memory operand is
//! one value, its base, offset and writeback together. A register carries what
//! is written on it. A register list is one operand.
//!
//! The shape follows the AArch64 crate's, held in
//! [`exarmo_core::view::Operands`], over AArch32's register files, shifts and
//! data types.

use crate::generated::enums::Cond;
use crate::generated::instruction::Instruction;
use crate::{DReg, GpReg, QReg, SReg};

exarmo_core::register_enum!(
    /// A register of any of AArch32's files, with which file it is.
    Reg {
        /// One of `r0` to `r12`, `sp`, `lr`, `pc`.
        Core(GpReg),
        /// A 32-bit SIMD and floating-point register, `s0` to `s31`.
        Single(SReg),
        /// A 64-bit one, `d0` to `d31`.
        Double(DReg),
        /// A 128-bit one, `q0` to `q15`.
        Quad(QReg),
    }
);

/// A shift applied to a register or an immediate, and how far.
///
/// `rrx` is the one shift written with no amount, being a rotate through the
/// carry by exactly one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Modifier {
    /// What is applied: `lsl`, `asr`, `rrx`.
    pub kind: crate::generated::modifier::ModifierKind,
    /// How far, where an amount is written.
    pub amount: Option<u32>,
    /// The register holding how far, where the shift is by a register:
    /// `lsl r3`.
    pub by: Option<crate::GpReg>,
}

/// A register operand, with what the assembly writes on it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegOperand {
    /// The register.
    pub reg: Reg,
    /// An element index: `d0[1]`.
    pub index: Option<u32>,
    /// A shift written after it: `r1, lsl #2`.
    pub modifier: Option<Modifier>,
    /// Whether the register is written back, `r0!`, as a load or store of
    /// several registers writes the base it steps.
    pub writeback: bool,
}

impl RegOperand {
    /// A bare register.
    pub const fn plain(reg: Reg) -> Self {
        RegOperand {
            writeback: false,
            reg,
            index: None,
            modifier: None,
        }
    }
}

/// What a memory operand adds to its base.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Offset {
    /// Nothing: `[r0]`.
    None,
    /// An immediate number of bytes, which may run backwards.
    Imm {
        /// The value, signed.
        value: i64,
        /// Whether the assembly writes it subtracted, `#-4`. A value below
        /// zero always is, and zero may be, since `[r0], #-0` is a word of
        /// its own with the U bit clear.
        subtract: bool,
    },
    /// A register, shifted as the modifier says, added or subtracted.
    Reg {
        /// The register.
        reg: Reg,
        /// Whether the assembly writes it subtracted, `[r0, -r1]`.
        subtract: bool,
        /// How it is shifted.
        modifier: Option<Modifier>,
    },
}

/// A memory operand, what it reads from and what it does to the base.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mem {
    /// The base register.
    pub base: Reg,
    /// What is added to it.
    pub offset: Offset,
    /// Whether the base is written back.
    pub writeback: Writeback,
    /// The alignment written after the base, in bits: `[r0:64]`.
    pub align: Option<u32>,
}

/// A list of registers, written in braces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RegList {
    /// Which registers, as a bitmask over the file's numbering.
    pub mask: u32,
    /// Which file they are of.
    pub file: ListFile,
    /// What is written on each register.
    pub lane: Lane,
}

impl RegList {
    /// A count of registers from a first, a stride apart: `{d0, d2, d4}` is
    /// three from d0 by two. None where one runs past the end of the file,
    /// since there is no such register.
    pub fn spaced(file: ListFile, first: u8, count: u8, stride: u8, lane: Lane) -> Option<Self> {
        let mut mask = 0u32;
        for k in 0..u32::from(count) {
            let number = u32::from(first) + k * u32::from(stride);
            if number >= file.registers() {
                return None;
            }
            mask |= 1 << number;
        }
        Some(RegList { mask, file, lane })
    }

    /// The registers in the list, lowest first.
    ///
    /// ```
    /// use exarmo_aarch32::{GpReg, Operand, Reg, a32};
    ///
    /// // push {r4, lr}
    /// let push = a32::decode_word(0xe92d4010).unwrap();
    /// let Operand::List(list) = push.operands()[1] else {
    ///     unreachable!()
    /// };
    /// assert_eq!(
    ///     list.regs().collect::<Vec<_>>(),
    ///     [Reg::Core(GpReg::new(4)), Reg::Core(GpReg::new(14))]
    /// );
    /// ```
    pub fn regs(self) -> impl Iterator<Item = Reg> {
        (0..self.file.registers() as u8)
            .filter(move |&number| self.mask & (1 << number) != 0)
            .map(move |number| self.file.reg(number))
    }
}

/// What a list writes on each of its registers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lane {
    /// The whole register: `{d0, d1}`.
    Whole,
    /// Every element, as a load to all lanes writes: `{d0[], d1[]}`.
    All,
    /// One element: `{d0[1], d1[1]}`.
    Index(u32),
}

/// Which register file a list holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ListFile {
    /// Core registers, as LDM and PUSH write.
    Core,
    /// Single-precision, as VLDM writes.
    Single,
    /// Double-precision, as VLD1 writes.
    Double,
}

impl ListFile {
    const fn reg(self, number: u8) -> Reg {
        match self {
            ListFile::Core => Reg::Core(GpReg::new(number)),
            ListFile::Single => Reg::Single(SReg::new(number)),
            ListFile::Double => Reg::Double(DReg::new(number)),
        }
    }

    /// How many registers the file holds.
    pub const fn registers(self) -> u32 {
        match self {
            ListFile::Core => 16,
            ListFile::Single | ListFile::Double => 32,
        }
    }
}

pub use exarmo_core::address::PcRead;
pub use exarmo_core::view::{IntValue, Symbol, Writeback};
pub use exarmo_core::write::float_from_bits;

/// One operand, whatever the instruction holds it as.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Operand {
    /// A register, with what is written on it.
    Reg(RegOperand),
    /// An immediate, with any shift written after it.
    Imm {
        /// The value.
        value: i64,
        /// How it is shifted.
        modifier: Option<Modifier>,
    },
    /// A floating-point immediate.
    FpImm {
        /// The value.
        value: f64,
        /// The width in bits of the pattern the encoding holds, 16, 32 or
        /// 64. Zero where it holds none, as for VCMP against zero, whose
        /// value the template writes.
        width: u8,
        /// The pattern at that width, where `width` says there is one. It
        /// spares a consumer encoding the value back, which Rust cannot do
        /// for half precision.
        bits: u64,
    },
    /// A branch target or a literal's address, as the offset from the PC the
    /// instruction reads.
    Label {
        /// The offset, which may run backwards.
        offset: i64,
        /// The PC the offset is from.
        pc: PcRead,
    },
    /// A memory operand.
    Mem(Mem),
    /// A register list.
    List(RegList),
    /// The condition the instruction is written with.
    Cond(Cond),
    /// A value from a value table.
    Symbol(Symbol),
    /// A shift written as an operand of its own.
    Modifier(Modifier),
    /// An operand this view does not yet describe.
    #[default]
    Other,
}

/// The operands of one instruction.
pub type Operands = exarmo_core::view::Operands<Operand, { Instruction::MAX_OPERANDS }>;
