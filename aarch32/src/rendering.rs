//! Writing an instruction from its operands view.
//!
//! The generated formatter is one function per shape of template, and every
//! encoding names the function for its shape. Each function is a sequence of
//! calls on a [`Writer`], for the mnemonic, the text between operands, and
//! each part of an operand as the assembly writes it.
//!
//! Tokens are put out by [`exarmo_core::write`], which AArch64 shares. This
//! module reads AArch32's operands for the parts to write.

use core::fmt;

use crate::operands::{Lane, Mem, Modifier, Offset, Operand, Reg};
use exarmo_core::tokens::TokenKind;
pub use exarmo_core::write::Form;

/// An immediate's value, for a test in a rendering.
pub fn imm(operand: &Operand) -> Option<i64> {
    match operand {
        Operand::Imm { value, .. } => Some(*value),
        _ => None,
    }
}

/// A shift's amount, for a test in a rendering.
pub fn amount(operand: &Operand) -> Option<u32> {
    modifier_of(operand)?.amount
}

/// The modifier written on an operand, or the one it is.
fn modifier_of(operand: &Operand) -> Option<Modifier> {
    match operand {
        Operand::Modifier(m) => Some(*m),
        Operand::Reg(r) => r.modifier,
        Operand::Imm { modifier, .. } => *modifier,
        Operand::Mem(m) => match m.offset {
            Offset::Reg { modifier, .. } => modifier,
            _ => None,
        },
        _ => None,
    }
}

/// The immediate a memory operand adds to its base, for a test in a
/// rendering against what the assembly leaves off.
pub fn mem_offset(operand: &Operand) -> Option<i64> {
    match operand {
        Operand::Mem(Mem {
            offset: Offset::Imm { value, .. },
            ..
        }) => Some(*value),
        _ => None,
    }
}

/// Whether a memory operand subtracts its immediate, for a test in a
/// rendering: a block writing the sign is written for a zero subtracted,
/// which the assembly leaving the block off would read as added.
pub fn mem_subtracted(operand: &Operand) -> bool {
    matches!(
        operand,
        Operand::Mem(Mem {
            offset: Offset::Imm { subtract: true, .. },
            ..
        })
    )
}

/// Whether a memory operand's base carries an alignment, for a test in a
/// rendering.
pub fn mem_align_written(operand: &Operand) -> bool {
    matches!(operand, Operand::Mem(Mem { align: Some(_), .. }))
}

/// Whether a memory operand's offset register is shifted at all, for a test
/// in a rendering. A shift by zero is not written.
pub fn mem_shift_written(operand: &Operand) -> bool {
    match operand {
        Operand::Mem(Mem {
            offset: Offset::Reg { modifier, .. },
            ..
        }) => modifier.is_some_and(|m| m.amount != Some(0)),
        _ => false,
    }
}

/// Whether an operand written onto the mnemonic writes anything, for a test
/// in a rendering around a block written only sometimes. A name a table
/// gives, a number and a condition do.
pub fn mnemonic_written(operand: &Operand) -> bool {
    match operand {
        Operand::Symbol(s) => !s.name.is_empty(),
        Operand::Imm { .. } | Operand::Cond(_) => true,
        _ => false,
    }
}

/// Whether an operand written onto the mnemonic is a name other than
/// `default`, the one the assembly leaves out, as LDM leaves out `ia`. For a
/// test in a rendering.
pub fn mnemonic_other_than(operand: &Operand, default: &str) -> bool {
    match operand {
        Operand::Symbol(s) => !s.name.is_empty() && s.name != default,
        Operand::Cond(cond) => cond.name() != default,
        _ => mnemonic_written(operand),
    }
}

/// Whether a register operand is written back, `r0!`, for a test in a
/// rendering.
pub fn reg_writeback(operand: &Operand) -> bool {
    matches!(operand, Operand::Reg(r) if r.writeback)
}

exarmo_core::rendering_writer!(
    /// The sink a rendering writes to, with the instruction's name and
    /// address. It derefs to the shared writer, so a rendering writes text,
    /// separators and brackets through it directly.
    Writer,
    32
);

impl<'a> Writer<'a> {
    /// The condition written onto the mnemonic, the `eq` of `addeq`.
    ///
    /// The generator decides which condition the assembly leaves off, as it
    /// does for every name the prose gives a default for, so this writes
    /// whatever it is given.
    #[inline(never)]
    pub fn condition(&mut self, operand: &Operand) -> fmt::Result {
        match operand {
            Operand::Cond(cond) => self.out.mnemonic(cond.name()),
            _ => Err(fmt::Error),
        }
    }

    /// A name from a value table, or a number one spells, written onto the
    /// mnemonic, as the `i32` of `vadd.i32` or the `32` of `vld1.32`. Nothing
    /// for a value its table does not name.
    #[inline(never)]
    pub fn mnemonic_symbol(&mut self, operand: &Operand, unnamed: Form) -> fmt::Result {
        match operand {
            // A number written onto the mnemonic is part of the mnemonic
            // rather than an operand after it, and carries its value.
            Operand::Imm { value, .. } => self.out.valued(TokenKind::Mnemonic, value),
            _ => self.named(operand, unnamed, TokenKind::Mnemonic),
        }
    }

    /// A name from a value table, or a condition, as a token of this kind. A
    /// value its table does not name is written as its bits.
    fn named(&mut self, operand: &Operand, unnamed: Form, kind: TokenKind) -> fmt::Result {
        match operand {
            Operand::Symbol(s) if s.name.is_empty() => self.out.number(i64::from(s.bits), unnamed),
            Operand::Symbol(s) => self.out.token(kind, s.name),
            Operand::Cond(cond) => self.out.valued(kind, cond),
            _ => Err(fmt::Error),
        }
    }

    /// A register operand.
    #[inline(never)]
    pub fn reg(&mut self, operand: &Operand) -> fmt::Result {
        match operand {
            Operand::Reg(r) => self.out.register(&r.reg),
            // A register the assembly writes by a name from a table, as MRC
            // writes `apsr_nzcv` for 1111. It stays a register token so a
            // consumer colouring registers colours it too.
            Operand::Symbol(s) => self.out.valued(TokenKind::Register, &s.name),
            _ => Err(fmt::Error),
        }
    }

    /// A word the template writes whole for an operand the view holds,
    /// spelt as the template spells it and of the operand's kind.
    #[inline(never)]
    pub fn word(&mut self, operand: &Operand, text: &str) -> fmt::Result {
        let kind = match operand {
            Operand::Reg(_) => TokenKind::Register,
            Operand::Symbol(_) | Operand::Cond(_) => TokenKind::Symbol,
            _ => TokenKind::Text,
        };
        self.out.token(kind, text)
    }

    /// An immediate operand, written as the form says.
    #[inline(never)]
    pub fn imm(&mut self, operand: &Operand, form: Form) -> fmt::Result {
        // A constant of a floating-point type is written as the number.
        if let Operand::FpImm { value, .. } = operand {
            return self.out.float(value);
        }
        self.out.number(imm(operand).ok_or(fmt::Error)?, form)
    }

    /// A floating-point immediate operand.
    #[inline(never)]
    pub fn float(&mut self, operand: &Operand) -> fmt::Result {
        match operand {
            Operand::FpImm { value, .. } => self.out.float(value),
            _ => Err(fmt::Error),
        }
    }

    /// A label, the PC the instruction reads plus the offset.
    #[inline(never)]
    pub fn label(&mut self, operand: &Operand) -> fmt::Result {
        match operand {
            Operand::Label { offset, pc } => self.out.label(*offset, *pc),
            _ => Err(fmt::Error),
        }
    }

    /// A name from a value table, or the bits where the table names nothing.
    #[inline(never)]
    pub fn symbol(&mut self, operand: &Operand, unnamed: Form) -> fmt::Result {
        self.named(operand, unnamed, TokenKind::Symbol)
    }

    /// An element index written on a register, the `1` of `d0[1]`.
    #[inline(never)]
    pub fn index(&mut self, operand: &Operand, form: Form) -> fmt::Result {
        let index = match operand {
            Operand::Reg(r) => r.index,
            _ => None,
        };
        self.some_number(index.map(i64::from), form)
    }

    /// A modifier's kind, as `lsl` or `rrx`. [`Writer::amount`] writes its
    /// amount.
    #[inline(never)]
    pub fn modifier(&mut self, operand: &Operand) -> fmt::Result {
        let modifier = modifier_of(operand).ok_or(fmt::Error)?;
        self.out.symbol(&modifier.kind)
    }

    /// How far a modifier shifts, where the template writes an amount.
    #[inline(never)]
    pub fn amount(&mut self, operand: &Operand, form: Form) -> fmt::Result {
        let amount = modifier_of(operand).and_then(|m| m.amount);
        self.some_number(amount.map(i64::from), form)
    }

    /// The register a modifier shifts by, `lsl <Rs>`.
    #[inline(never)]
    pub fn amount_reg(&mut self, operand: &Operand) -> fmt::Result {
        let by = modifier_of(operand).and_then(|m| m.by);
        self.some_register(by.map(Reg::Core))
    }

    /// The base register of a memory operand.
    #[inline(never)]
    pub fn mem_base(&mut self, operand: &Operand) -> fmt::Result {
        match operand {
            Operand::Mem(Mem { base, .. }) => self.some_register(Some(*base)),
            _ => Err(fmt::Error),
        }
    }

    /// The immediate a memory operand adds to its base.
    #[inline(never)]
    pub fn mem_offset(&mut self, operand: &Operand, form: Form) -> fmt::Result {
        match operand {
            // A subtracted zero encodes apart from an added one and is written
            // `#-0`, so the sign is passed apart from the value.
            Operand::Mem(Mem {
                offset: Offset::Imm { value, subtract },
                ..
            }) if form == Form::Signed => self.out.signed(*value, *subtract),
            Operand::Mem(Mem {
                offset: Offset::Imm { value, .. },
                ..
            }) => self.some_number(Some(*value), form),
            _ => Err(fmt::Error),
        }
    }

    /// The register a memory operand adds to its base.
    #[inline(never)]
    pub fn mem_offset_reg(&mut self, operand: &Operand) -> fmt::Result {
        match operand {
            Operand::Mem(Mem {
                offset: Offset::Reg { reg, .. },
                ..
            }) => self.some_register(Some(*reg)),
            _ => Err(fmt::Error),
        }
    }

    /// The `-` before a memory operand's offset, where it is subtracted.
    #[inline(never)]
    pub fn mem_sign(&mut self, operand: &Operand) -> fmt::Result {
        let negative = match operand {
            Operand::Mem(Mem {
                offset: Offset::Reg { subtract, .. },
                ..
            }) => *subtract,
            Operand::Mem(Mem {
                offset: Offset::Imm { subtract, .. },
                ..
            }) => *subtract,
            _ => return Err(fmt::Error),
        };
        match negative {
            true => self.out.text("-"),
            false => Ok(()),
        }
    }

    /// The alignment written on a memory operand's base.
    #[inline(never)]
    pub fn mem_align(&mut self, operand: &Operand) -> fmt::Result {
        match operand {
            Operand::Mem(Mem {
                align: Some(align), ..
            }) => self.some_number(Some(i64::from(*align)), Form::Decimal),
            _ => Err(fmt::Error),
        }
    }

    /// A register list, written in its braces as the registers it names,
    /// each with what the list writes on it.
    #[inline(never)]
    pub fn list(&mut self, operand: &Operand) -> fmt::Result {
        let Operand::List(list) = operand else {
            return Err(fmt::Error);
        };
        self.out.bracket("{")?;
        for (written, reg) in list.regs().enumerate() {
            if written > 0 {
                // Written inside the list's braces, so it is part of the
                // list rather than between it and the next operand.
                self.out.inner_separator()?;
            }
            self.out.register(&reg)?;
            match list.lane {
                Lane::Whole => {}
                Lane::All => {
                    self.out.bracket("[")?;
                    self.out.bracket("]")?;
                }
                Lane::Index(index) => {
                    self.out.bracket("[")?;
                    self.out.number(i64::from(index), Form::Decimal)?;
                    self.out.bracket("]")?;
                }
            }
        }
        self.out.bracket("}")
    }
}
