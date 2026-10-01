//! The system operations and PSTATE fields the architecture names.
//!
//! [`SysReg`](crate::SysReg) is the registers MRS and MSR name. The rest of
//! the system instruction space is named the same way and reached through
//! other instructions. AT, DC, IC and TLBI name an operation, and MSR names a
//! PSTATE field, each by a word rather than by an encoding. A consumer
//! numbering the whole space in one place needs those under their encodings
//! too.
//!
//! What a decode reports for one of these is the operand's own field, which
//! is a different concatenation for each instruction. TLBI's holds `CRn`
//! where AT's, DC's and IC's do not, and for most PSTATE fields it holds
//! MSR's immediate, so `msr daifset, #3` and `msr daifset, #5` report
//! different bits for the same field. The operand's bits are therefore no
//! identity, and this table is where the encoding behind a name is read.

use crate::Mnemonic;
use crate::generated::sysops::ENTRIES;

/// One system operation or PSTATE field, under the encoding that reaches it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SysOp {
    /// The `op0:op1:CRn:CRm:op2` encoding, packed as
    /// [`SysRegDef::encoding`](crate::SysRegDef::encoding) is, so that the
    /// whole system instruction space is numbered one way.
    ///
    /// `op0` is 0 or 1 here and 2 or 3 for a register, so no operation and
    /// no register come to one number: `amair_el1` is 0xC518 where PLBI's
    /// `vmalle1is` is 0x4518.
    ///
    /// It says where in that space an operation sits, not which row this
    /// is. TLBI and TLBIP reach the same point by the same five fields under
    /// the same name, and are told apart only by the instruction carrying
    /// them, so 120 of these encodings name two rows. A consumer keying on
    /// one row keys on [`SysOp::instruction`] and [`SysOp::name`], which
    /// `tests/sysops.rs` holds unique.
    pub encoding: u16,
    /// Which mechanism names it, 1 for an operation SYS reaches and 0 for a
    /// PSTATE field MSR writes.
    pub op0: u8,
    /// The rest of the encoding, each field as the instruction holds it.
    pub op1: u8,
    /// See [`SysOp::op1`].
    pub crn: u8,
    /// See [`SysOp::op1`].
    pub crm: u8,
    /// See [`SysOp::op1`].
    pub op2: u8,
    /// Which bits of `crm` name it, the rest being what the instruction
    /// writes as its immediate.
    ///
    /// Every bit for an operation the architecture documents on a page of
    /// its own, and none for most PSTATE fields, whose `CRm` is the whole
    /// of MSR's immediate. SVCRSM and its neighbours are named by three of
    /// the four bits and take the fourth as the immediate.
    pub crm_names: u8,
    /// The instruction that names it, such as `at`, `dc`, `ic`, `tlbi` or
    /// `msr`.
    pub instruction: Mnemonic,
    /// The name the assembly writes, in lower case.
    pub name: &'static str,
    /// Whether the operation takes a general-purpose register.
    pub reg_use: RegisterUse,
    /// Whether it reads the register or writes it. Every row of one
    /// instruction that takes a register says the same, so a consumer
    /// declaring one signature per instruction reads it off any of them.
    pub reg_access: RegisterAccess,
    /// How many bits of the register move. That is 64, or 128 for TLBIP's
    /// pair, of which the first register holds bits 0 to 63 and the second
    /// bits 64 to 127, and 0 where the operation takes no register. The
    /// same on every row of one instruction that takes a register.
    pub reg_bits: u8,
}

/// Whether a system operation takes a general-purpose register, as the
/// SysReg page documenting it lays that register out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RegisterUse {
    /// It takes none, as IC IALLUIS does. Rt is 0b11111 and the assembly
    /// writes no register.
    None = 0,
    /// The register carries something only where a feature is implemented,
    /// as for TLBI VMALLE1IS on FEAT_TLBID. The assembly writes it where it
    /// is not XZR.
    Optional = 1,
    /// It takes one, which the assembly always writes, as DC ZVA, GICR CDIA
    /// and TLBIP VAE1IS do.
    Required = 2,
}

/// Whether a system operation reads its register or writes it, as the
/// pseudocode on its page does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RegisterAccess {
    /// TLBI VAE1IS reads the address it invalidates.
    Read = 0,
    /// GICR CDIA writes what it acknowledges.
    Write = 1,
}

/// A system operation or PSTATE field an operand names, as the row of the
/// table it landed on and the bits the instruction's own field holds.
///
/// The row is the identity, carrying the name, the instruction naming it
/// and the five encoding fields. The field is a different concatenation for
/// each instruction, and for most PSTATE fields it holds the instruction's
/// immediate as well, so `msr daifset, #3` and `msr daifset, #5` land on one
/// row with different field bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SysOpRef {
    row: u16,
    field: u16,
}

impl SysOpRef {
    /// The row at `row` of [`SysOp::all`], named by a field holding `field`.
    ///
    /// The generator reads `row` out of the table it wrote, so the row
    /// always names something.
    pub(crate) const fn at(row: u16, field: u16) -> Self {
        assert!((row as usize) < SysOp::COUNT);
        SysOpRef { row, field }
    }

    /// The row's index into [`SysOp::all`], and into
    /// `exarmo_aarch64_sysops` in the C face.
    pub const fn index(self) -> u16 {
        self.row
    }

    /// The row itself, with the operation's name, the instruction naming it,
    /// and the five encoding fields.
    pub const fn def(self) -> SysOp {
        SysOp::at(self.row as usize)
    }

    /// The bits the instruction's own operand field holds. See [`SysOpRef`]
    /// for why these are not the identity.
    pub const fn field(self) -> u16 {
        self.field
    }
}

impl SysOp {
    /// How many the architecture names.
    pub const COUNT: usize = ENTRIES.len();

    /// Every one the architecture names, in encoding order and then by
    /// name.
    pub fn all() -> impl Iterator<Item = SysOp> {
        ENTRIES.iter().copied()
    }

    /// The one at `index` of [`SysOp::all`], which must be below
    /// [`SysOp::COUNT`].
    ///
    /// A `const fn` so that a C face can lay the whole table out as a
    /// constant.
    pub const fn at(index: usize) -> SysOp {
        ENTRIES[index]
    }
}
