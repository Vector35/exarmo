//! Each type of `exarmo_aarch32::operands` laid out as
//! `include/exarmo/aarch32.h` declares it, and the conversion from the Rust
//! one. The header is the authority on layout, and every struct here follows
//! its field order.

use exarmo_aarch32::{Lane, ListFile, Operand, Reg};
use exarmo_core::capi::Str;
pub use exarmo_core::capi::{CFpImm, Label, Symbol, Writeback};

/// Which register file a register is in, `exarmo_aarch32_reg_class`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegClass {
    /// One of `r0` to `r12`, `sp`, `lr`, `pc`, by number.
    Core = 0,
    /// A 32-bit SIMD and floating-point register, `s0` to `s31`.
    S = 1,
    /// A 64-bit one, `d0` to `d31`.
    D = 2,
    /// A 128-bit one, `q0` to `q15`.
    Q = 3,
}

/// A register, `exarmo_aarch32_reg`.
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
            Reg::Core(r) => (RegClass::Core, r.num()),
            Reg::Single(r) => (RegClass::S, r.num()),
            Reg::Double(r) => (RegClass::D, r.num()),
            Reg::Quad(r) => (RegClass::Q, r.num()),
        };
        CReg {
            class: class as u8,
            num,
        }
    }
}

/// A shift, `exarmo_aarch32_modifier`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Modifier {
    /// Whether one is written.
    pub present: bool,
    /// What is applied, a `ModifierKind` by number.
    pub kind: u8,
    /// How far, in bits, or -1 where no amount is written.
    pub amount: i32,
    /// Whether `by` is written.
    pub has_by: bool,
    /// The register holding how far, for a shift by a register.
    pub by: CReg,
}

impl Modifier {
    const NONE: Modifier = Modifier {
        present: false,
        kind: 0,
        amount: -1,
        has_by: false,
        by: CReg::NONE,
    };
}

impl From<Option<exarmo_aarch32::Modifier>> for Modifier {
    fn from(modifier: Option<exarmo_aarch32::Modifier>) -> Self {
        match modifier {
            Some(m) => Modifier {
                present: true,
                kind: m.kind as u8,
                amount: m.amount.map_or(-1, |a| a as i32),
                has_by: m.by.is_some(),
                by: m.by.map_or(CReg::NONE, |r| Reg::Core(r).into()),
            },
            None => Modifier::NONE,
        }
    }
}

/// A register operand, `exarmo_aarch32_reg_operand`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RegOperand {
    /// The register.
    pub reg: CReg,
    /// An element index, or -1 where none is written.
    pub index: i32,
    /// A shift written after it.
    pub modifier: Modifier,
    /// Whether the register is written back, `r0!`.
    pub writeback: bool,
}

impl From<exarmo_aarch32::RegOperand> for RegOperand {
    fn from(op: exarmo_aarch32::RegOperand) -> Self {
        RegOperand {
            reg: op.reg.into(),
            index: op.index.map_or(-1, |i| i as i32),
            modifier: op.modifier.into(),
            writeback: op.writeback,
        }
    }
}

/// What a memory operand adds to its base, `exarmo_aarch32_offset_kind`.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OffsetKind {
    /// Nothing.
    None = 0,
    /// An immediate.
    Imm = 1,
    /// A register.
    Reg = 2,
}

/// What a memory operand adds to its base, `exarmo_aarch32_offset`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Offset {
    /// Which of the fields below apply.
    pub kind: OffsetKind,
    /// An immediate's value, in bytes, which may be negative.
    pub imm: i64,
    /// A register offset's register.
    pub reg: CReg,
    /// Whether the assembly writes the offset subtracted. An immediate
    /// below zero always is, and a zero one may be, as in `[r0], #-0`.
    pub subtract: bool,
    /// How a register offset is shifted.
    pub modifier: Modifier,
}

impl Offset {
    const NONE: Offset = Offset {
        kind: OffsetKind::None,
        imm: 0,
        reg: CReg::NONE,
        subtract: false,
        modifier: Modifier::NONE,
    };
}

impl From<exarmo_aarch32::Offset> for Offset {
    fn from(offset: exarmo_aarch32::Offset) -> Self {
        match offset {
            exarmo_aarch32::Offset::None => Offset::NONE,
            exarmo_aarch32::Offset::Imm { value, subtract } => Offset {
                kind: OffsetKind::Imm,
                imm: value,
                subtract,
                ..Offset::NONE
            },
            exarmo_aarch32::Offset::Reg {
                reg,
                subtract,
                modifier,
            } => Offset {
                kind: OffsetKind::Reg,
                reg: reg.into(),
                subtract,
                modifier: modifier.into(),
                ..Offset::NONE
            },
        }
    }
}

/// A memory operand, `exarmo_aarch32_mem`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Mem {
    /// The register the address starts from.
    pub base: CReg,
    /// What is added to the base.
    pub offset: Offset,
    /// Whether the base is written back.
    pub writeback: Writeback,
    /// The alignment written after the base, in bits, or 0 where none is.
    pub align: u32,
}

impl From<exarmo_aarch32::Mem> for Mem {
    fn from(mem: exarmo_aarch32::Mem) -> Self {
        Mem {
            base: mem.base.into(),
            offset: mem.offset.into(),
            writeback: mem.writeback.into(),
            align: mem.align.unwrap_or(0),
        }
    }
}

/// Which register file a list holds, `exarmo_aarch32_list_file`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CListFile {
    /// Core registers.
    Core = 0,
    /// Single-precision registers.
    S = 1,
    /// Double-precision registers.
    D = 2,
}

/// What a list writes on each of its registers, `exarmo_aarch32_lane`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CLane {
    /// The whole register.
    Whole = 0,
    /// Every element, `d0[]`.
    All = 1,
    /// One element, `d0[1]`, which `index` names.
    Index = 2,
}

/// A register list, `exarmo_aarch32_reg_list`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RegList {
    /// Which registers, bit n for register n of the file.
    pub mask: u32,
    /// Which file, a `CListFile`.
    pub file: u8,
    /// What is written on each register, a `CLane`.
    pub lane: u8,
    /// The element, for `CLane::Index`.
    pub index: u32,
}

impl From<exarmo_aarch32::RegList> for RegList {
    fn from(list: exarmo_aarch32::RegList) -> Self {
        let (lane, index) = match list.lane {
            Lane::Whole => (CLane::Whole, 0),
            Lane::All => (CLane::All, 0),
            Lane::Index(index) => (CLane::Index, index),
        };
        RegList {
            mask: list.mask,
            file: match list.file {
                ListFile::Core => CListFile::Core,
                ListFile::Single => CListFile::S,
                ListFile::Double => CListFile::D,
            } as u8,
            lane: lane as u8,
            index,
        }
    }
}

/// What an operand is, `exarmo_aarch32_operand_kind`.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperandKind {
    /// A register.
    Reg = 0,
    /// An immediate.
    Imm = 1,
    /// A floating-point immediate.
    FpImm = 2,
    /// A branch target or a literal's address.
    Label = 3,
    /// A memory operand.
    Mem = 4,
    /// A register list.
    List = 5,
    /// A condition.
    Cond = 6,
    /// A value from a value table.
    Symbol = 7,
    /// A shift written as an operand of its own.
    Modifier = 8,
    /// An operand the library does not describe.
    Other = 9,
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

/// The value of an operand, the union in `exarmo_aarch32_operand`.
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
    /// `OperandKind::Cond`, an `exarmo_aarch32_cond`.
    pub cond: u8,
    /// `OperandKind::Symbol`.
    pub symbol: Symbol,
    /// `OperandKind::Modifier`.
    pub modifier: Modifier,
}

/// One operand, `exarmo_aarch32_operand`.
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
            Operand::Cond(c) => (OperandKind::Cond, OperandValue { cond: c as u8 }),
            Operand::Symbol(s) => (OperandKind::Symbol, OperandValue { symbol: s.into() }),
            Operand::Modifier(m) => (
                OperandKind::Modifier,
                OperandValue {
                    modifier: Some(m).into(),
                },
            ),
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

/// A system register an instruction accesses, `exarmo_aarch32_sysreg`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SysReg {
    /// Which table of the index names it, an `exarmo_aarch32_sysreg_space`.
    pub space: u8,
    /// Whether the instruction writes it.
    pub write: bool,
    /// The key, the instruction's fields run together as the index orders
    /// them.
    pub encoding: u32,
}

impl From<exarmo_aarch32::SysReg> for SysReg {
    fn from(reg: exarmo_aarch32::SysReg) -> Self {
        SysReg {
            space: reg.space() as u8,
            write: reg.is_write(),
            encoding: reg.encoding(),
        }
    }
}

/// A system register the architecture names, `exarmo_aarch32_sysreg_def`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SysRegDef {
    /// Which table of the index names it, an `exarmo_aarch32_sysreg_space`.
    pub space: u8,
    /// Whether it can be read under this name.
    pub readable: bool,
    /// Whether it can be written under this name.
    pub writable: bool,
    /// The key, as `SysReg` carries it.
    pub encoding: u32,
    /// The name, in lower case.
    pub name: Str,
}

impl SysRegDef {
    /// The register at `index` of the architecture's table, read at compile
    /// time so the whole table is a constant.
    pub const fn at(index: usize) -> SysRegDef {
        let def = exarmo_aarch32::SysRegDef::at(index);
        SysRegDef {
            space: def.space as u8,
            readable: def.readable,
            writable: def.writable,
            encoding: def.encoding,
            name: Str::of(def.name),
        }
    }
}
