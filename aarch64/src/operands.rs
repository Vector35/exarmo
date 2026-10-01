//! An instruction's operands, seen without matching on its variant.
//!
//! `Instruction` types each operand for what it is, one variant per
//! encoding. A consumer that cannot match on thousands of variants, such as a
//! lifter behind a C boundary, asks for [`Instruction::operands`] instead.
//! That gives the operands in the order the assembly writes them, each an
//! [`Operand`], with the structure the template implied made explicit. A
//! memory operand is one value, its base, offset and writeback together. A
//! register carries the arrangement, index and modifier written on it, and a
//! register list is one operand.

use crate::generated::enums::Cond;
use crate::generated::{Instruction, ModifierKind};
use crate::{
    Arrangement, BReg, DReg, DynRegSp, DynRegZr, DynScalarSimd, GpReg, HReg, PNReg, PReg, QReg,
    SReg, SysOpRef, SysReg, VReg, WRegSp, WRegZr, XRegSp, XRegZr, ZATile, ZReg, ZaTileMask,
};

exarmo_core::register_enum!(
    /// A register of any file, with what it is.
    Reg {
        /// A 32-bit general-purpose register, 31 the zero register.
        W(WRegZr),
        /// A 32-bit general-purpose register, 31 the stack pointer.
        WSp(WRegSp),
        /// A 64-bit general-purpose register, 31 the zero register.
        X(XRegZr),
        /// A 64-bit general-purpose register, 31 the stack pointer.
        XSp(XRegSp),
        /// A general-purpose register whose width the instruction did not fix.
        Gp(GpReg),
        /// W or X, chosen as the instruction was decoded.
        WX(DynRegZr),
        /// W or X with 31 the stack pointer, chosen as the instruction was decoded.
        WXSp(DynRegSp),
        /// An 8-bit scalar SIMD register.
        B(BReg),
        /// A 16-bit scalar SIMD register.
        H(HReg),
        /// A 32-bit scalar SIMD register.
        S(SReg),
        /// A 64-bit scalar SIMD register.
        D(DReg),
        /// A 128-bit scalar SIMD register.
        Q(QReg),
        /// A scalar SIMD register whose width the instruction chose.
        Scalar(DynScalarSimd),
        /// A vector register.
        V(VReg),
        /// An SVE vector register.
        Z(ZReg),
        /// An SVE predicate register.
        P(PReg),
        /// An SVE predicate-as-counter register.
        PN(PNReg),
        /// An SME ZA tile.
        ZA(ZATile),
    }
    named {
        /// The SME lookup table register, ZT0.
        ZT0 => "zt0",
    }
);

pub use exarmo_core::view::Symbol;

/// A shift, extend or multiplier applied to a register or an immediate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Modifier {
    /// What is applied, such as `lsl`, `uxtw` or `msl`.
    pub kind: ModifierKind,
    /// How far, in bits, where an amount is written.
    pub amount: Option<u32>,
}

/// A register operand, with what the assembly writes on it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegOperand {
    /// The register.
    pub reg: Reg,
    /// The arrangement, as in `.4s` or `.d`.
    pub arrangement: Option<Arrangement>,
    /// An element index, as in `v0.s[2]`.
    pub index: Option<u32>,
    /// A register the element index counts from, as in PSEL's
    /// `p0.b[w12, #3]`.
    pub index_reg: Option<Reg>,
    /// A predication qualifier, `/z` or `/m`.
    pub predication: Option<char>,
    /// A shift or extend written after it, as in `x1, lsl #2`.
    pub modifier: Option<Modifier>,
}

impl RegOperand {
    /// A bare register.
    pub const fn plain(reg: Reg) -> Self {
        RegOperand {
            reg,
            arrangement: None,
            index: None,
            index_reg: None,
            predication: None,
            modifier: None,
        }
    }
}

/// What a memory operand adds to its base.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Offset {
    /// None.
    None,
    /// An immediate, in bytes, or in vector lengths with `mul vl`.
    Imm {
        /// The value.
        value: i64,
        /// Whether the value is in vector lengths.
        mul_vl: bool,
    },
    /// A general-purpose register, shifted or extended as the modifier says.
    Reg {
        /// The register.
        reg: Reg,
        /// How it is shifted or extended.
        modifier: Option<Modifier>,
    },
    /// A vector of offsets, one per lane.
    Vector {
        /// The register.
        reg: Reg,
        /// The arrangement written on it.
        arrangement: Option<Arrangement>,
        /// How it is shifted or extended.
        modifier: Option<Modifier>,
    },
}

/// A memory operand, giving an address and whether the base is updated.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mem {
    /// The register the address starts from.
    pub base: Reg,
    /// The arrangement of a vector base, as in `[z0.d]`.
    pub base_arrangement: Option<Arrangement>,
    /// What is added to the base, which for a post-indexed operand is what
    /// the base is advanced by after the access.
    pub offset: Offset,
    /// Whether the base is written back.
    pub writeback: Writeback,
}

/// A list of registers, as in `{ v0.16b, v1.16b }` or `{ z0.b - z3.b }`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegList {
    /// The registers, in order, of which only the first `len` are the list's.
    pub regs: [Reg; 4],
    /// How many registers the list has.
    pub len: u8,
    /// The arrangement written on it.
    pub arrangement: Option<Arrangement>,
    /// An element index on the whole list, as in `{ v0.s, v1.s }[2]`.
    pub index: Option<u32>,
}

impl RegList {
    /// The registers in the list.
    pub fn regs(&self) -> &[Reg] {
        &self.regs[..self.len as usize]
    }
}

/// The SME storage a slice is taken from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ZaArray {
    /// The ZA array as a whole, `ZA[w12, 3]`, or by element width, `ZA.S[...]`.
    Za,
    /// The lookup table register, `ZT0[3]`.
    Zt0,
    /// One tile, `ZA0H.B[w12, 3]`.
    Tile(ZATile),
}

/// A slice of SME storage, rows or columns of a tile or vectors of the
/// array. It is selected by an index register plus an offset or a range of
/// offsets, and for the array optionally as a vector group.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZaSlice {
    /// What the slice is taken from.
    pub array: ZaArray,
    /// `h` for horizontal, `v` for vertical, where a tile says.
    pub direction: Option<char>,
    /// The element width written on it.
    pub arrangement: Option<Arrangement>,
    /// The register the slice index counts from, or none for ZT0.
    pub index: Option<Reg>,
    /// The offset added to the index, or the first of a range.
    pub offset: u32,
    /// The last offset of a range, as in `ZA.S[w12, 0:3]`.
    pub last: Option<u32>,
    /// Whether the offset is in vector lengths.
    pub mul_vl: bool,
    /// The vector group, 2 or 4, where written.
    pub vector_group: Option<u8>,
}

/// The operands of one instruction, in the order the assembly writes them.
pub type Operands = exarmo_core::view::Operands<Operand, { Instruction::MAX_OPERANDS }>;

/// One operand, as the assembly writes it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Operand {
    /// A register, with what is written on it.
    Reg(RegOperand),
    /// An immediate, with any shift written after it, as in `#1, lsl #12`.
    Imm {
        /// The value.
        value: i64,
        /// How it is shifted or extended.
        modifier: Option<Modifier>,
    },
    /// A floating-point immediate.
    FpImm {
        /// The value.
        value: f64,
        /// How wide the pattern the encoding holds is, in bits, which is 16,
        /// 32 or 64. Zero where it holds none, as for SVE's FCPY, whose element
        /// width is the arrangement's rather than the constant's, and for a
        /// table of reals, which names a value.
        width: u8,
        /// The pattern at that width, where `width` says there is one. A
        /// consumer building a constant of that width reads it rather than
        /// encoding the value back, which C cannot do for half precision.
        bits: u64,
    },
    /// A branch target or a page's address, as the offset from the PC the
    /// instruction reads.
    Label {
        /// The offset, which may be negative.
        offset: i64,
        /// The PC the offset is from.
        pc: PcRead,
    },
    /// A memory operand.
    Mem(Mem),
    /// A register list.
    List(RegList),
    /// A system register.
    SysReg(SysReg),
    /// A system operation or PSTATE field, as the row of the table naming
    /// it. What the operand's own field holds is a different concatenation
    /// for each instruction, so the row is the identity rather than the
    /// bits.
    SysOp(SysOpRef),
    /// A condition.
    Cond(Cond),
    /// A value from a value table.
    Symbol(Symbol),
    /// A slice of SME storage.
    ZaSlice(ZaSlice),
    /// The set of ZA tiles ZERO clears.
    TileMask(ZaTileMask),
    /// A shift or multiplier written as an operand of its own, applying to
    /// the instruction rather than to the operand before it, as CNTB's
    /// `mul #14` does.
    Modifier(Modifier),
    /// An operand this view does not describe, such as a shape a later
    /// release adds.
    #[default]
    Other,
}

pub use exarmo_core::address::PcRead;
pub use exarmo_core::view::{IntValue, Writeback};
pub use exarmo_core::write::float_from_bits;
