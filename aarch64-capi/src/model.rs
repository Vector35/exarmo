//! Each type of `exarmo_aarch64::operands` laid out as
//! `include/exarmo/aarch64.h` declares it, and the conversion from the Rust
//! one. The header is the authority on layout, and every struct here follows
//! its field order.

use exarmo_aarch64::{Operand, Reg};
use exarmo_core::capi::Str;
pub use exarmo_core::capi::{CFpImm, Label, Symbol, Writeback};

/// Which register file a register is in, `exarmo_aarch64_reg_class`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegClass {
    /// A 32-bit general-purpose register, 31 the zero register.
    W = 0,
    /// A 32-bit general-purpose register, 31 the stack pointer.
    WSp = 1,
    /// A 64-bit general-purpose register, 31 the zero register.
    X = 2,
    /// A 64-bit general-purpose register, 31 the stack pointer.
    XSp = 3,
    /// A general-purpose register whose width the instruction did not fix.
    Gp = 4,
    /// An 8-bit scalar SIMD register.
    B = 5,
    /// A 16-bit scalar SIMD register.
    H = 6,
    /// A 32-bit scalar SIMD register.
    S = 7,
    /// A 64-bit scalar SIMD register.
    D = 8,
    /// A 128-bit scalar SIMD register.
    Q = 9,
    /// A vector register.
    V = 10,
    /// An SVE vector register.
    Z = 11,
    /// An SVE predicate register.
    P = 12,
    /// An SVE predicate-as-counter register.
    Pn = 13,
    /// An SME ZA tile.
    ZaTile = 14,
    /// The SME lookup table register.
    Zt0 = 15,
}

/// A register, `exarmo_aarch64_reg`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CReg {
    /// The file, a `RegClass`.
    pub class: u8,
    /// The number.
    pub num: u8,
}

impl CReg {
    const NONE: CReg = CReg { class: 0, num: 0 };
}

impl From<Reg> for CReg {
    fn from(reg: Reg) -> Self {
        let (class, num) = match reg {
            Reg::W(r) => (RegClass::W, r.num()),
            Reg::WSp(r) => (RegClass::WSp, r.num()),
            Reg::X(r) => (RegClass::X, r.num()),
            Reg::XSp(r) => (RegClass::XSp, r.num()),
            Reg::Gp(r) => (RegClass::Gp, r.num()),
            Reg::WX(r) => (if r.is_64() { RegClass::X } else { RegClass::W }, r.num()),
            Reg::WXSp(r) => (
                if r.is_64() {
                    RegClass::XSp
                } else {
                    RegClass::WSp
                },
                r.num(),
            ),
            Reg::B(r) => (RegClass::B, r.num()),
            Reg::H(r) => (RegClass::H, r.num()),
            Reg::S(r) => (RegClass::S, r.num()),
            Reg::D(r) => (RegClass::D, r.num()),
            Reg::Q(r) => (RegClass::Q, r.num()),
            Reg::Scalar(r) => (
                match r.size_bits() {
                    8 => RegClass::B,
                    16 => RegClass::H,
                    32 => RegClass::S,
                    64 => RegClass::D,
                    _ => RegClass::Q,
                },
                r.num(),
            ),
            Reg::V(r) => (RegClass::V, r.num()),
            Reg::Z(r) => (RegClass::Z, r.num()),
            Reg::P(r) => (RegClass::P, r.num()),
            Reg::PN(r) => (RegClass::Pn, r.num()),
            Reg::ZA(r) => (RegClass::ZaTile, r.num()),
            Reg::ZT0 => (RegClass::Zt0, 0),
        };
        CReg {
            class: class as u8,
            num,
        }
    }
}

/// An arrangement, `exarmo_aarch64_arrangement`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arrangement {
    /// The width of one lane in bits, or zero.
    pub element: u8,
    /// How many lanes, or zero.
    pub lanes: u8,
}

impl Arrangement {
    const NONE: Arrangement = Arrangement {
        element: 0,
        lanes: 0,
    };
}

impl From<Option<exarmo_aarch64::Arrangement>> for Arrangement {
    fn from(arrangement: Option<exarmo_aarch64::Arrangement>) -> Self {
        match arrangement {
            Some(a) => Arrangement {
                element: a.element_width().bits() as u8,
                lanes: a.lanes().unwrap_or(0),
            },
            None => Arrangement::NONE,
        }
    }
}

/// A system operation or PSTATE field an operand names,
/// `exarmo_aarch64_sysop`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CSysOp {
    /// Where the row sits in `exarmo_aarch64_sysops`, which carries the
    /// name, the instruction naming it and the five encoding fields.
    pub index: u16,
    /// The bits the instruction's own operand field holds, which is not an
    /// identity. See `exarmo_aarch64_sysops`.
    pub field: u16,
}

impl From<exarmo_aarch64::SysOpRef> for CSysOp {
    fn from(op: exarmo_aarch64::SysOpRef) -> Self {
        CSysOp {
            index: op.index(),
            field: op.field(),
        }
    }
}

/// A shift, extend or multiplier, `exarmo_aarch64_modifier`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Modifier {
    /// Whether one is written.
    pub present: bool,
    /// What is applied, a `ModifierKind` by number.
    pub kind: u8,
    /// How far, in bits, or -1 where no amount is written.
    pub amount: i32,
}

impl Modifier {
    const NONE: Modifier = Modifier {
        present: false,
        kind: 0,
        amount: -1,
    };
}

impl From<Option<exarmo_aarch64::Modifier>> for Modifier {
    fn from(modifier: Option<exarmo_aarch64::Modifier>) -> Self {
        match modifier {
            Some(m) => Modifier {
                present: true,
                kind: m.kind as u8,
                amount: m.amount.map_or(-1, |a| a as i32),
            },
            None => Modifier::NONE,
        }
    }
}

/// A register operand, `exarmo_aarch64_reg_operand`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RegOperand {
    /// The register.
    pub reg: CReg,
    /// The arrangement written on it.
    pub arrangement: Arrangement,
    /// An element index, or -1 where none is written.
    pub index: i32,
    /// Whether `index_reg` is written.
    pub has_index_reg: bool,
    /// A register the element index counts from.
    pub index_reg: CReg,
    /// A predication qualifier, `z` or `m`, or 0 where none is written.
    pub predication: u8,
    /// A shift or extend written after it.
    pub modifier: Modifier,
}

impl From<exarmo_aarch64::RegOperand> for RegOperand {
    fn from(op: exarmo_aarch64::RegOperand) -> Self {
        RegOperand {
            reg: op.reg.into(),
            arrangement: op.arrangement.into(),
            index: op.index.map_or(-1, |i| i as i32),
            has_index_reg: op.index_reg.is_some(),
            index_reg: op.index_reg.map_or(CReg::NONE, CReg::from),
            predication: op.predication.map_or(0, |c| c as u8),
            modifier: op.modifier.into(),
        }
    }
}

/// What a memory operand adds to its base, `exarmo_aarch64_offset_kind`.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OffsetKind {
    /// Nothing.
    None = 0,
    /// An immediate.
    Imm = 1,
    /// A register.
    Reg = 2,
    /// A vector of offsets.
    Vector = 3,
}

/// What a memory operand adds to its base, `exarmo_aarch64_offset`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Offset {
    /// Which of the fields below apply.
    pub kind: OffsetKind,
    /// An immediate's value.
    pub imm: i64,
    /// Whether the immediate is in vector lengths.
    pub mul_vl: bool,
    /// A register offset's register.
    pub reg: CReg,
    /// A vector offset's arrangement.
    pub arrangement: Arrangement,
    /// How a register offset is shifted or extended.
    pub modifier: Modifier,
}

impl Offset {
    const NONE: Offset = Offset {
        kind: OffsetKind::None,
        imm: 0,
        mul_vl: false,
        reg: CReg::NONE,
        arrangement: Arrangement::NONE,
        modifier: Modifier::NONE,
    };
}

impl From<exarmo_aarch64::Offset> for Offset {
    fn from(offset: exarmo_aarch64::Offset) -> Self {
        match offset {
            exarmo_aarch64::Offset::None => Offset::NONE,
            exarmo_aarch64::Offset::Imm { value, mul_vl } => Offset {
                kind: OffsetKind::Imm,
                imm: value,
                mul_vl,
                ..Offset::NONE
            },
            exarmo_aarch64::Offset::Reg { reg, modifier } => Offset {
                kind: OffsetKind::Reg,
                reg: reg.into(),
                modifier: modifier.into(),
                ..Offset::NONE
            },
            exarmo_aarch64::Offset::Vector {
                reg,
                arrangement,
                modifier,
            } => Offset {
                kind: OffsetKind::Vector,
                reg: reg.into(),
                arrangement: arrangement.into(),
                modifier: modifier.into(),
                ..Offset::NONE
            },
        }
    }
}

/// A memory operand, `exarmo_aarch64_mem`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Mem {
    /// The register the address starts from.
    pub base: CReg,
    /// The arrangement of a vector base.
    pub base_arrangement: Arrangement,
    /// What is added to the base, which for a post-indexed operand is what
    /// the base is advanced by after the access.
    pub offset: Offset,
    /// Whether the base is written back.
    pub writeback: Writeback,
}

impl From<exarmo_aarch64::Mem> for Mem {
    fn from(mem: exarmo_aarch64::Mem) -> Self {
        Mem {
            base: mem.base.into(),
            base_arrangement: mem.base_arrangement.into(),
            offset: mem.offset.into(),
            writeback: mem.writeback.into(),
        }
    }
}

/// A register list, `exarmo_aarch64_reg_list`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RegList {
    /// The registers, of which only the first `len` are the list's.
    pub regs: [CReg; 4],
    /// How many registers the list has.
    pub len: u8,
    /// The arrangement written on it.
    pub arrangement: Arrangement,
    /// An element index on the whole list, or -1 where none is written.
    pub index: i32,
}

impl From<exarmo_aarch64::RegList> for RegList {
    fn from(list: exarmo_aarch64::RegList) -> Self {
        RegList {
            regs: list.regs.map(CReg::from),
            len: list.len,
            arrangement: list.arrangement.into(),
            index: list.index.map_or(-1, |i| i as i32),
        }
    }
}

/// The SME storage a slice is taken from, `exarmo_aarch64_za_array`.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZaArray {
    /// The ZA array as a whole.
    Array = 0,
    /// The lookup table register.
    Zt0 = 1,
    /// One tile.
    Tile = 2,
}

/// A slice of SME storage, `exarmo_aarch64_za_slice`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ZaSlice {
    /// What the slice is taken from.
    pub array: ZaArray,
    /// The tile number, for a tile.
    pub tile: u8,
    /// `h` or `v` where a tile says, or 0.
    pub direction: u8,
    /// The element width written on it.
    pub arrangement: Arrangement,
    /// Whether `index` is written.
    pub has_index: bool,
    /// The register the slice index counts from.
    pub index: CReg,
    /// The offset added to the index, or the first of a range.
    pub offset: u32,
    /// The last offset of a range, or -1 where none.
    pub last: i32,
    /// Whether the offset is in vector lengths.
    pub mul_vl: bool,
    /// The vector group where written, or 0.
    pub vector_group: u8,
}

impl From<exarmo_aarch64::ZaSlice> for ZaSlice {
    fn from(slice: exarmo_aarch64::ZaSlice) -> Self {
        let (array, tile) = match slice.array {
            exarmo_aarch64::ZaArray::Za => (ZaArray::Array, 0),
            exarmo_aarch64::ZaArray::Zt0 => (ZaArray::Zt0, 0),
            exarmo_aarch64::ZaArray::Tile(t) => (ZaArray::Tile, t.num()),
        };
        ZaSlice {
            array,
            tile,
            direction: slice.direction.map_or(0, |c| c as u8),
            arrangement: slice.arrangement.into(),
            has_index: slice.index.is_some(),
            index: slice.index.map_or(CReg::NONE, CReg::from),
            offset: slice.offset,
            last: slice.last.map_or(-1, |l| l as i32),
            mul_vl: slice.mul_vl,
            vector_group: slice.vector_group.unwrap_or(0),
        }
    }
}

/// A system register, `exarmo_aarch64_sysreg`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SysReg {
    /// The `op0:op1:CRn:CRm:op2` encoding, `op0` as the two bits it is.
    pub encoding: u16,
    /// Whether the instruction writes it.
    pub write: bool,
    /// The fields of the encoding.
    pub op0: u8,
    /// See `op0`.
    pub op1: u8,
    /// See `op0`.
    pub crn: u8,
    /// See `op0`.
    pub crm: u8,
    /// See `op0`.
    pub op2: u8,
}

impl From<exarmo_aarch64::SysReg> for SysReg {
    fn from(reg: exarmo_aarch64::SysReg) -> Self {
        SysReg {
            encoding: reg.encoding(),
            write: reg.is_write(),
            op0: reg.op0(),
            op1: reg.op1(),
            crn: reg.crn(),
            crm: reg.crm(),
            op2: reg.op2(),
        }
    }
}

/// A system register the architecture names, `exarmo_aarch64_sysreg_def`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SysRegDef {
    /// The `op0:op1:CRn:CRm:op2` encoding, `op0` as the two bits it is.
    pub encoding: u16,
    /// Whether MRS can read it under this name.
    pub readable: bool,
    /// Whether MSR can write it under this name.
    pub writable: bool,
    /// The name, in lower case.
    pub name: Str,
}

impl SysRegDef {
    /// The register at `index` of the architecture's table, read at compile
    /// time so the whole table is a constant.
    pub const fn at(index: usize) -> SysRegDef {
        let def = exarmo_aarch64::SysRegDef::at(index);
        SysRegDef {
            encoding: def.encoding,
            readable: def.readable,
            writable: def.writable,
            name: Str::of(def.name),
        }
    }
}

/// A system operation or PSTATE field as C holds it,
/// `exarmo_aarch64_sysop_def`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SysOpDef {
    /// The `op0:op1:CRn:CRm:op2` encoding, packed as `SysRegDef` packs one.
    pub encoding: u16,
    /// Which instruction names it, an `exarmo_aarch64_mnemonic`.
    pub instruction: u32,
    /// Which mechanism names it, 1 for an operation SYS reaches and 0 for a
    /// PSTATE field MSR writes.
    pub op0: u8,
    /// The rest of the encoding, each field as the instruction holds it.
    pub op1: u8,
    /// See `op1`.
    pub crn: u8,
    /// See `op1`.
    pub crm: u8,
    /// See `op1`.
    pub op2: u8,
    /// Which bits of `crm` name it, the rest being the instruction's
    /// immediate.
    pub crm_names: u8,
    /// Whether it takes a general-purpose register, an
    /// `exarmo_aarch64_sysop_reg_use`.
    pub reg_use: u8,
    /// Whether it reads that register or writes it, an
    /// `exarmo_aarch64_sysop_reg_access`.
    pub reg_access: u8,
    /// How many bits of the register move, which is 64, 128 for a pair, or 0.
    pub reg_bits: u8,
    /// The name, in lower case.
    pub name: Str,
}

impl SysOpDef {
    /// The operation at `index` of the architecture's table, read at compile
    /// time so the whole table is a constant.
    pub const fn at(index: usize) -> SysOpDef {
        let op = exarmo_aarch64::SysOp::at(index);
        SysOpDef {
            encoding: op.encoding,
            instruction: op.instruction as u32,
            op0: op.op0,
            op1: op.op1,
            crn: op.crn,
            crm: op.crm,
            op2: op.op2,
            crm_names: op.crm_names,
            reg_use: op.reg_use as u8,
            reg_access: op.reg_access as u8,
            reg_bits: op.reg_bits,
            name: Str::of(op.name),
        }
    }
}

/// What an operand is, `exarmo_aarch64_operand_kind`.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperandKind {
    /// A register.
    Reg = 0,
    /// An immediate.
    Imm = 1,
    /// A floating-point immediate.
    FpImm = 2,
    /// A branch target.
    Label = 3,
    /// A memory operand.
    Mem = 4,
    /// A register list.
    List = 5,
    /// A system register.
    SysReg = 6,
    /// A system operation or PSTATE field.
    SysOp = 7,
    /// A condition.
    Cond = 8,
    /// A value from a value table.
    Symbol = 9,
    /// A slice of SME storage.
    ZaSlice = 10,
    /// The set of ZA tiles ZERO clears.
    TileMask = 11,
    /// A shift or multiplier written as an operand of its own.
    Modifier = 12,
    /// An operand the library does not describe.
    Other = 13,
}

/// An immediate with any shift written after it.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Imm {
    /// The value.
    pub value: i64,
    /// How it is shifted.
    pub modifier: Modifier,
}

/// The value of an operand, the union in `exarmo_aarch64_operand`.
#[repr(C)]
#[derive(Clone, Copy)]
pub union OperandValue {
    /// `OperandKind::Reg`.
    pub reg: RegOperand,
    /// `OperandKind::Imm`.
    pub imm: Imm,
    /// `OperandKind::FpImm`.
    pub fp_imm: CFpImm,
    /// `OperandKind::Label`.
    pub label: Label,
    /// `OperandKind::Mem`.
    pub mem: Mem,
    /// `OperandKind::List`.
    pub list: RegList,
    /// `OperandKind::SysReg`.
    pub sysreg: SysReg,
    /// `OperandKind::SysOp`.
    pub sysop: CSysOp,
    /// `OperandKind::Cond`, an `exarmo_aarch64_cond`.
    pub cond: u8,
    /// `OperandKind::Symbol`.
    pub symbol: Symbol,
    /// `OperandKind::ZaSlice`.
    pub za_slice: ZaSlice,
    /// `OperandKind::TileMask`.
    pub tile_mask: u8,
    /// `OperandKind::Modifier`.
    pub modifier: Modifier,
}

/// One operand, `exarmo_aarch64_operand`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct COperand {
    /// Which member of `value` applies.
    pub kind: OperandKind,
    /// The value.
    pub value: OperandValue,
}

impl From<Operand> for COperand {
    fn from(op: Operand) -> Self {
        let (kind, value) = match op {
            Operand::Reg(r) => (OperandKind::Reg, OperandValue { reg: r.into() }),
            Operand::Imm { value, modifier } => (
                OperandKind::Imm,
                OperandValue {
                    imm: Imm {
                        value,
                        modifier: modifier.into(),
                    },
                },
            ),
            Operand::FpImm { value, width, bits } => (
                OperandKind::FpImm,
                OperandValue {
                    fp_imm: CFpImm { value, width, bits },
                },
            ),
            Operand::Label { offset, pc } => (
                OperandKind::Label,
                OperandValue {
                    label: Label::of(offset, pc),
                },
            ),
            Operand::Mem(m) => (OperandKind::Mem, OperandValue { mem: m.into() }),
            Operand::List(l) => (OperandKind::List, OperandValue { list: l.into() }),
            Operand::SysReg(r) => (OperandKind::SysReg, OperandValue { sysreg: r.into() }),
            Operand::SysOp(op) => (OperandKind::SysOp, OperandValue { sysop: op.into() }),
            Operand::Cond(c) => (OperandKind::Cond, OperandValue { cond: c as u8 }),
            Operand::Symbol(s) => (OperandKind::Symbol, OperandValue { symbol: s.into() }),
            Operand::ZaSlice(s) => (OperandKind::ZaSlice, OperandValue { za_slice: s.into() }),
            Operand::TileMask(m) => (OperandKind::TileMask, OperandValue { tile_mask: m.0 }),
            Operand::Modifier(m) => (
                OperandKind::Modifier,
                OperandValue {
                    modifier: Some(m).into(),
                },
            ),
            // Nothing to hold, so fill the narrowest member the two faces
            // share. A caller reading it is reading what the library has said
            // it does not describe.
            Operand::Other => (
                OperandKind::Other,
                OperandValue {
                    fp_imm: CFpImm {
                        value: 0.0,
                        width: 0,
                        bits: 0,
                    },
                },
            ),
        };
        COperand { kind, value }
    }
}

/// Where an instruction holds an intrinsic's argument,
/// `exarmo_aarch64_intrinsic_source_kind`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntrinsicSourceKind {
    /// The operand itself.
    Operand = 0,
    /// The element index written on the operand.
    Index = 1,
    /// An immediate operand, shifted right by `shift` to recover the value.
    Immediate = 2,
    /// The FPMR register, which the instruction reads.
    Fpmr = 3,
    /// The instruction does not carry it.
    Unused = 4,
}

/// Where an instruction holds an argument, `exarmo_aarch64_intrinsic_source`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct IntrinsicSource {
    /// Which of the ways below.
    pub kind: IntrinsicSourceKind,
    /// The operand, for `Operand`, `Index` and `Immediate`.
    pub operand: u8,
    /// How far the immediate was shifted, for `Immediate`.
    pub shift: u8,
}

impl From<exarmo_aarch64::intrinsics::Source> for IntrinsicSource {
    fn from(source: exarmo_aarch64::intrinsics::Source) -> Self {
        use exarmo_aarch64::intrinsics::Source;
        let (kind, operand, shift) = match source {
            Source::Operand(operand) => (IntrinsicSourceKind::Operand, operand, 0),
            Source::Index(operand) => (IntrinsicSourceKind::Index, operand, 0),
            Source::Immediate { operand, shift } => {
                (IntrinsicSourceKind::Immediate, operand, shift)
            }
            Source::Fpmr => (IntrinsicSourceKind::Fpmr, 0, 0),
            Source::Unused => (IntrinsicSourceKind::Unused, 0, 0),
        };
        IntrinsicSource {
            kind,
            operand,
            shift,
        }
    }
}

/// Where an intrinsic's result comes from, `exarmo_aarch64_intrinsic_output_kind`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntrinsicOutputKind {
    /// None, as for a store.
    None = 0,
    /// An operand.
    Operand = 1,
    /// One element of a structure of vectors.
    Element = 2,
}

/// Where the result comes from, `exarmo_aarch64_intrinsic_output`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct IntrinsicOutput {
    /// Which of the ways below.
    pub kind: IntrinsicOutputKind,
    /// The operand, for `Operand` and `Element`.
    pub operand: u8,
    /// Which element, for `Element`.
    pub element: u8,
}

impl From<exarmo_aarch64::intrinsics::Output> for IntrinsicOutput {
    fn from(output: exarmo_aarch64::intrinsics::Output) -> Self {
        use exarmo_aarch64::intrinsics::Output;
        let (kind, operand, element) = match output {
            Output::None => (IntrinsicOutputKind::None, 0, 0),
            Output::Operand(operand) => (IntrinsicOutputKind::Operand, operand, 0),
            Output::Element { operand, element } => {
                (IntrinsicOutputKind::Element, operand, element)
            }
        };
        IntrinsicOutput {
            kind,
            operand,
            element,
        }
    }
}

/// The most arguments any intrinsic takes, as the generated header says.
pub const MAX_INTRINSIC_ARGUMENTS: usize = 5;

/// One intrinsic a decoded instruction realises, `exarmo_aarch64_intrinsic`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CIntrinsic {
    /// Which intrinsic, an index `exarmo_aarch64_intrinsic_at` reads, below
    /// `EXARMO_AARCH64_INTRINSIC_COUNT`.
    pub id: u32,
    /// How many of `arguments` are filled.
    pub argument_count: u8,
    /// Where the instruction holds each argument, in the signature's order.
    pub arguments: [IntrinsicSource; MAX_INTRINSIC_ARGUMENTS],
    /// Where the result comes from.
    pub result: IntrinsicOutput,
}

impl Default for CIntrinsic {
    fn default() -> Self {
        CIntrinsic {
            id: 0,
            argument_count: 0,
            arguments: [IntrinsicSource {
                kind: IntrinsicSourceKind::Unused,
                operand: 0,
                shift: 0,
            }; MAX_INTRINSIC_ARGUMENTS],
            result: IntrinsicOutput {
                kind: IntrinsicOutputKind::None,
                operand: 0,
                element: 0,
            },
        }
    }
}

impl From<&exarmo_aarch64::intrinsics::Intrinsic> for CIntrinsic {
    fn from(intrinsic: &exarmo_aarch64::intrinsics::Intrinsic) -> Self {
        let mut out = CIntrinsic {
            id: intrinsic.id,
            argument_count: intrinsic.arguments.len() as u8,
            result: intrinsic.result.into(),
            ..CIntrinsic::default()
        };
        for (slot, argument) in out.arguments.iter_mut().zip(intrinsic.arguments) {
            *slot = (*argument).into();
        }
        out
    }
}

/// A C type an intrinsic is written in, `exarmo_aarch64_intrinsic_type`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CIntrinsicTypeDef {
    /// The name as ACLE writes it, as in `int8x16_t`.
    pub name: Str,
    /// What family of value it holds, `exarmo_aarch64_intrinsic_type_kind`.
    pub kind: u8,
    /// How many elements, 1 for a scalar.
    pub lanes: u8,
    /// How many such vectors, which is 2, 3 or 4 for a structure and 1
    /// otherwise.
    pub vectors: u8,
    /// Whether the argument is a pointer to that.
    pub pointer: bool,
    /// Whether that pointer is to a constant, as a load's is and a store's
    /// is not.
    pub readonly: bool,
    /// How wide one element is, in bits, or zero for `void`.
    pub element_bits: u16,
}

impl From<&exarmo_aarch64::intrinsics::IntrinsicTypeDef> for CIntrinsicTypeDef {
    fn from(ty: &exarmo_aarch64::intrinsics::IntrinsicTypeDef) -> Self {
        CIntrinsicTypeDef {
            name: Str::of(ty.name),
            kind: ty.kind as u8,
            lanes: ty.lanes,
            vectors: ty.vectors,
            pointer: ty.pointer,
            readonly: ty.readonly,
            element_bits: ty.element_bits,
        }
    }
}

/// An intrinsic as ACLE declares it, `exarmo_aarch64_intrinsic_def`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CIntrinsicDef {
    /// The name, as in `vmla_lane_s16`.
    pub name: Str,
    /// The result's type, an `exarmo_aarch64_intrinsic_type`.
    pub result: u32,
    /// How many of `parameters` are filled.
    pub parameter_count: u8,
    /// Each parameter's type, in order, as `exarmo_aarch64_intrinsic_type`.
    pub parameters: [u32; MAX_INTRINSIC_ARGUMENTS],
}

impl From<&exarmo_aarch64::intrinsics::IntrinsicDef> for CIntrinsicDef {
    fn from(def: &exarmo_aarch64::intrinsics::IntrinsicDef) -> Self {
        let mut out = CIntrinsicDef {
            name: Str::of(def.name),
            result: def.result as u32,
            parameter_count: def.parameters.len() as u8,
            parameters: [0; MAX_INTRINSIC_ARGUMENTS],
        };
        for (slot, parameter) in out.parameters.iter_mut().zip(def.parameters) {
            *slot = *parameter as u32;
        }
        out
    }
}
