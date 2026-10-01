//! Writing an instruction from its operands view.
//!
//! The generated formatter is one function per shape of template, each a
//! sequence of calls on a [`Writer`]. The calls write the mnemonic, the text
//! between operands, and each part of an operand as the assembly writes it,
//! such as a register, the arrangement on it, an immediate in hex or decimal,
//! or a branch target. Each optional block sits in an `if` on the tests here.
//! Every encoding names the function for its shape, so the formatter is a
//! table and a few hundred functions rather than one per encoding.

use core::fmt;

use crate::operands::{Mem, Modifier, Offset, Operand, Reg, ZaArray, ZaSlice};
use crate::tokens::TokenKind;

use crate::{Arrangement, ModifierKind};
/// How a number is written. Re-exported because the generated renderings
/// name it by this path.
pub use exarmo_core::write::Form;

exarmo_core::rendering_writer!(
    /// Where a rendering writes, the sink with the instruction's name and
    /// address. Its methods are out of line, so that a rendering is a call
    /// per piece and no more.
    Writer,
    64
);

fn modifier_of(operand: &Operand) -> Option<Modifier> {
    match operand {
        Operand::Reg(r) => r.modifier,
        Operand::Imm { modifier, .. } => *modifier,
        Operand::Modifier(m) => Some(*m),
        Operand::Mem(m) => offset_modifier(&m.offset),
        _ => None,
    }
}

fn offset_modifier(offset: &Offset) -> Option<Modifier> {
    match offset {
        Offset::Reg { modifier, .. } | Offset::Vector { modifier, .. } => *modifier,
        _ => None,
    }
}

fn offset_reg(offset: &Offset) -> Option<Reg> {
    match offset {
        Offset::Reg { reg, .. } | Offset::Vector { reg, .. } => Some(*reg),
        _ => None,
    }
}

fn offset_imm(offset: &Offset) -> Option<i64> {
    match offset {
        Offset::Imm { value, .. } => Some(*value),
        _ => None,
    }
}

fn mem_of(operand: &Operand) -> Option<&Mem> {
    match operand {
        Operand::Mem(m) => Some(m),
        _ => None,
    }
}

fn slice_of(operand: &Operand) -> Option<&ZaSlice> {
    match operand {
        Operand::ZaSlice(s) => Some(s),
        _ => None,
    }
}

/// An immediate's value, for a test.
pub fn imm(operand: &Operand) -> Option<i64> {
    match operand {
        Operand::Imm { value, .. } => Some(*value),
        _ => None,
    }
}

/// A symbol's bits, for a test.
pub fn bits(operand: &Operand) -> Option<u16> {
    match operand {
        Operand::Symbol(s) => Some(s.bits),
        Operand::SysOp(op) => Some(op.field()),
        _ => None,
    }
}

/// The name a symbol or a system operation writes, for a test.
pub fn written(operand: &Operand) -> Option<&'static str> {
    match operand {
        Operand::Symbol(s) => Some(s.name),
        Operand::SysOp(op) => Some(op.def().name),
        _ => None,
    }
}

/// A register's number, for a test.
pub fn num(operand: &Operand) -> Option<u8> {
    match operand {
        Operand::Reg(r) => Some(r.reg.num()),
        _ => None,
    }
}

/// The kind of the modifier on an operand, for a test.
pub fn kind(operand: &Operand) -> Option<ModifierKind> {
    modifier_of(operand).map(|m| m.kind)
}

/// The amount of the modifier on an operand, for a test.
pub fn amount(operand: &Operand) -> Option<u32> {
    modifier_of(operand)?.amount
}

/// A memory operand's immediate offset, for a test.
pub fn mem_offset(operand: &Operand) -> Option<i64> {
    offset_imm(&mem_of(operand)?.offset)
}

/// The kind of the modifier on a memory operand's register offset, for a
/// test.
pub fn mem_kind(operand: &Operand) -> Option<ModifierKind> {
    offset_modifier(&mem_of(operand)?.offset).map(|m| m.kind)
}

/// The amount of the modifier on a memory operand's register offset, for
/// a test.
pub fn mem_amount(operand: &Operand) -> Option<u32> {
    offset_modifier(&mem_of(operand)?.offset)?.amount
}

/// The number of a memory operand's register offset, for a test.
pub fn mem_reg(operand: &Operand) -> Option<u8> {
    offset_reg(&mem_of(operand)?.offset).map(Reg::num)
}

/// A slice's offset, or the first of its range, for a test.
pub fn za_offset(operand: &Operand) -> Option<u32> {
    slice_of(operand).map(|s| s.offset)
}

/// The element index on a register or a list, for a test.
pub fn index(operand: &Operand) -> Option<u32> {
    match operand {
        Operand::Reg(r) => r.index,
        Operand::List(l) => l.index,
        _ => None,
    }
}

/// The writer derefs to the shared one, so a rendering writes text, a
/// separator or a bracket through it directly and only what reads an operand
/// is here.
impl<'a> Writer<'a> {
    fn some_arrangement(&mut self, arrangement: Option<Arrangement>) -> fmt::Result {
        self.out
            .valued(TokenKind::Symbol, &arrangement.ok_or(fmt::Error)?)
    }

    fn some_modifier(&mut self, modifier: Option<Modifier>) -> fmt::Result {
        self.out
            .token(TokenKind::Symbol, modifier.ok_or(fmt::Error)?.kind.name())
    }

    fn some_amount(&mut self, modifier: Option<Modifier>, form: Form) -> fmt::Result {
        let amount = modifier.and_then(|m| m.amount).ok_or(fmt::Error)?;
        self.number(i64::from(amount), form)
    }

    /// A register operand.
    #[inline(never)]
    pub fn reg(&mut self, operand: &Operand) -> fmt::Result {
        match operand {
            Operand::Reg(r) => self.some_register(Some(r.reg)),
            Operand::SysReg(reg) => self.out.valued(TokenKind::Register, reg),
            // ZERO's mask is written as the tiles it names, in one token, and
            // a mask naming none writes none.
            Operand::TileMask(mask) if mask.0 == 0 => Ok(()),
            Operand::TileMask(mask) => self.out.valued(TokenKind::Register, mask),
            _ => Err(fmt::Error),
        }
    }

    /// A word the template writes whole for an operand the view holds,
    /// spelt as the template spells it and of the operand's kind.
    #[inline(never)]
    pub fn word(&mut self, operand: &Operand, text: &str) -> fmt::Result {
        let kind = match operand {
            Operand::Reg(_) | Operand::SysReg(_) | Operand::TileMask(_) => TokenKind::Register,
            Operand::Symbol(_) | Operand::SysOp(_) | Operand::Cond(_) => TokenKind::Symbol,
            _ => TokenKind::Text,
        };
        self.out.token(kind, text)
    }

    /// An immediate operand, written as the form says.
    #[inline(never)]
    pub fn imm(&mut self, operand: &Operand, form: Form) -> fmt::Result {
        self.some_number(imm(operand), form)
    }

    /// A floating-point immediate operand.
    #[inline(never)]
    pub fn float(&mut self, operand: &Operand) -> fmt::Result {
        match operand {
            Operand::FpImm { value, .. } => self.out.float(value),
            _ => Err(fmt::Error),
        }
    }

    /// A branch target or a literal's address, the PC the instruction reads
    /// plus the offset.
    #[inline(never)]
    pub fn label(&mut self, operand: &Operand) -> fmt::Result {
        match operand {
            Operand::Label { offset, pc } => self.out.label(*offset, *pc),
            _ => Err(fmt::Error),
        }
    }

    fn named(&mut self, operand: &Operand, unnamed: Form, kind: TokenKind) -> fmt::Result {
        match operand {
            Operand::Symbol(s) if s.name.is_empty() => self.number(i64::from(s.bits), unnamed),
            Operand::Symbol(s) => self.out.token(kind, s.name),
            Operand::SysOp(op) => self.out.token(kind, op.def().name),
            Operand::Cond(cond) => self.out.valued(kind, cond),
            // Written as its immediate where the table names no value.
            Operand::Imm { value, .. } => self.number(*value, unnamed),
            _ => Err(fmt::Error),
        }
    }

    /// A name from a table, or a condition. A value the table does not
    /// name is written as the form says.
    #[inline(never)]
    pub fn symbol(&mut self, operand: &Operand, unnamed: Form) -> fmt::Result {
        self.named(operand, unnamed, TokenKind::Symbol)
    }

    /// A name written as part of the mnemonic, such as the `eq` of `b.eq`.
    #[inline(never)]
    pub fn mnemonic_symbol(&mut self, operand: &Operand, unnamed: Form) -> fmt::Result {
        match operand {
            // A number written onto the mnemonic is part of the mnemonic
            // rather than an operand after it, and carries its value.
            Operand::Imm { value, .. } => self.out.valued(TokenKind::Mnemonic, value),
            _ => self.named(operand, unnamed, TokenKind::Mnemonic),
        }
    }

    /// The arrangement written on a register, a list or a slice.
    #[inline(never)]
    pub fn arrangement(&mut self, operand: &Operand) -> fmt::Result {
        let arrangement = match operand {
            Operand::Reg(r) => r.arrangement,
            Operand::List(l) => l.arrangement,
            Operand::ZaSlice(s) => s.arrangement,
            _ => None,
        };
        self.some_arrangement(arrangement)
    }

    /// The element index on a register or a list.
    #[inline(never)]
    pub fn index(&mut self, operand: &Operand, form: Form) -> fmt::Result {
        self.some_number(index(operand).map(i64::from), form)
    }

    /// The register an element index counts from.
    #[inline(never)]
    pub fn index_reg(&mut self, operand: &Operand) -> fmt::Result {
        match operand {
            Operand::Reg(r) => self.some_register(r.index_reg),
            _ => Err(fmt::Error),
        }
    }

    /// The predication qualifier on a register.
    #[inline(never)]
    pub fn predication(&mut self, operand: &Operand) -> fmt::Result {
        match operand {
            Operand::Reg(r) => self
                .out
                .valued(TokenKind::Symbol, &r.predication.ok_or(fmt::Error)?),
            _ => Err(fmt::Error),
        }
    }

    /// The kind of the modifier written after an operand.
    #[inline(never)]
    pub fn modifier(&mut self, operand: &Operand) -> fmt::Result {
        self.some_modifier(modifier_of(operand))
    }

    /// The amount of the modifier written after an operand.
    #[inline(never)]
    pub fn amount(&mut self, operand: &Operand, form: Form) -> fmt::Result {
        self.some_amount(modifier_of(operand), form)
    }

    /// The modifier written after an operand where one operand of the
    /// template names it whole, its kind and then its amount, as in
    /// `lsl #0xc`.
    #[inline(never)]
    pub fn whole_modifier(&mut self, operand: &Operand, form: Form) -> fmt::Result {
        let modifier = modifier_of(operand);
        self.some_modifier(modifier)?;
        self.out.text(" ")?;
        self.some_amount(modifier, form)
    }

    /// A memory operand's base register.
    #[inline(never)]
    pub fn mem_base(&mut self, operand: &Operand) -> fmt::Result {
        self.some_register(mem_of(operand).map(|m| m.base))
    }

    /// The arrangement written on a vector base.
    #[inline(never)]
    pub fn mem_base_arrangement(&mut self, operand: &Operand) -> fmt::Result {
        self.some_arrangement(mem_of(operand).and_then(|m| m.base_arrangement))
    }

    /// A memory operand's immediate offset.
    #[inline(never)]
    pub fn mem_offset(&mut self, operand: &Operand, form: Form) -> fmt::Result {
        self.some_number(mem_offset(operand), form)
    }

    /// A memory operand's register offset.
    #[inline(never)]
    pub fn mem_offset_reg(&mut self, operand: &Operand) -> fmt::Result {
        self.some_register(mem_of(operand).and_then(|m| offset_reg(&m.offset)))
    }

    /// The arrangement written on a vector offset.
    #[inline(never)]
    pub fn mem_offset_arrangement(&mut self, operand: &Operand) -> fmt::Result {
        let arrangement = match mem_of(operand).map(|m| m.offset) {
            Some(Offset::Vector { arrangement, .. }) => arrangement,
            _ => None,
        };
        self.some_arrangement(arrangement)
    }

    /// The kind of the modifier on a register offset.
    #[inline(never)]
    pub fn mem_offset_modifier(&mut self, operand: &Operand) -> fmt::Result {
        self.some_modifier(mem_of(operand).and_then(|m| offset_modifier(&m.offset)))
    }

    /// The amount of the modifier on a register offset.
    #[inline(never)]
    pub fn mem_offset_amount(&mut self, operand: &Operand, form: Form) -> fmt::Result {
        self.some_amount(
            mem_of(operand).and_then(|m| offset_modifier(&m.offset)),
            form,
        )
    }

    /// The `n`th register of a list.
    #[inline(never)]
    pub fn list_reg(&mut self, operand: &Operand, n: usize) -> fmt::Result {
        match operand {
            Operand::List(list) => self.some_register(list.regs().get(n).copied()),
            _ => Err(fmt::Error),
        }
    }

    /// The tile a slice is taken from.
    #[inline(never)]
    pub fn za_tile(&mut self, operand: &Operand) -> fmt::Result {
        match slice_of(operand).map(|s| s.array) {
            Some(ZaArray::Tile(tile)) => self.out.valued(TokenKind::Register, &tile),
            Some(ZaArray::Zt0) => self.out.token(TokenKind::Register, "zt0"),
            _ => Err(fmt::Error),
        }
    }

    /// The direction of a tile slice.
    #[inline(never)]
    pub fn za_direction(&mut self, operand: &Operand) -> fmt::Result {
        let direction = slice_of(operand).and_then(|s| s.direction);
        self.out
            .valued(TokenKind::Symbol, &direction.ok_or(fmt::Error)?)
    }

    /// The register a slice index counts from.
    #[inline(never)]
    pub fn za_index_reg(&mut self, operand: &Operand) -> fmt::Result {
        self.some_register(slice_of(operand).and_then(|s| s.index))
    }

    /// A slice's offset, or the first of its range.
    #[inline(never)]
    pub fn za_offset(&mut self, operand: &Operand, form: Form) -> fmt::Result {
        self.some_number(slice_of(operand).map(|s| i64::from(s.offset)), form)
    }

    /// The last offset of a slice's range.
    #[inline(never)]
    pub fn za_last(&mut self, operand: &Operand, form: Form) -> fmt::Result {
        self.some_number(slice_of(operand).and_then(|s| s.last).map(i64::from), form)
    }
}
