//! What an instruction does to the flow of control.
//!
//! Both instruction sets express a branch in their pseudocode as a `BranchTo`
//! carrying one of ARM's `BranchType` values, so one type serves both
//! runtimes. The generator reads it from each encoding's Execute section.

use crate::address::PcRead;

/// What kind of branch an instruction takes.
///
/// The discriminants are part of both C headers.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BranchKind {
    /// It does not branch.
    #[default]
    None = 0,
    /// It branches to an address it names (`BranchType_DIR`).
    Direct = 1,
    /// It branches to an address it computes (`BranchType_INDIR`).
    Indirect = 2,
    /// It calls an address it names (`BranchType_DIRCALL`).
    DirectCall = 3,
    /// It calls an address it computes (`BranchType_INDCALL`).
    IndirectCall = 4,
    /// It returns from a call (`BranchType_RET`).
    Return = 5,
    /// It returns from an exception, such as ERET, or DRPS in debug state.
    ExceptionReturn = 6,
    /// It takes an exception whose handler returns to the next instruction,
    /// such as SVC, HVC, SMC and A64's TENTER.
    SystemCall = 7,
    /// It raises an exception, such as BRK or a permanently undefined
    /// encoding.
    Exception = 8,
    /// It halts the processor into debug state (HLT).
    Halt = 9,
}

impl BranchKind {
    /// Whether the instruction names the address it branches to, so that the
    /// label it writes is the target.
    pub const fn names_target(self) -> bool {
        matches!(self, BranchKind::Direct | BranchKind::DirectCall)
    }

    /// Whether the instruction sets the PC itself.
    ///
    /// The kinds that raise an exception do not, because their handler
    /// decides where execution continues.
    pub const fn sets_the_pc(self) -> bool {
        !matches!(
            self,
            BranchKind::None | BranchKind::SystemCall | BranchKind::Exception | BranchKind::Halt
        )
    }
}

/// What an instruction does to the flow of control, apart from any address.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BranchEffect {
    /// What kind of branch it takes.
    pub kind: BranchKind,
    /// Whether it takes the branch only when a test succeeds.
    ///
    /// This describes the instruction, not its encoding. A `b.al` uses the
    /// conditional encoding but always branches, so it is not conditional.
    pub conditional: bool,
}

/// A [`BranchEffect`] resolved at an address.
///
/// The fall-through address of a conditional branch is not included. It is
/// the instruction's address plus its length.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Branch {
    /// What kind of branch it takes.
    pub kind: BranchKind,
    /// Whether the branch is taken only where a test succeeds.
    pub conditional: bool,
    /// The address it branches to, when the instruction names one.
    pub target: Option<u64>,
}

impl BranchEffect {
    /// Does not branch.
    pub const NONE: BranchEffect = BranchEffect {
        kind: BranchKind::None,
        conditional: false,
    };

    /// The branch as it reads at `address`, in an instruction set whose
    /// addresses are `bits` wide.
    ///
    /// `label` is the offset and PC reading of the label the instruction
    /// writes, if any. It is used only when the kind names its target. The
    /// label of RETASPPC is for the guarded control stack and is not a
    /// target.
    pub fn at(self, address: u64, label: Option<(i64, PcRead)>, bits: u32) -> Branch {
        Branch {
            kind: self.kind,
            conditional: self.conditional,
            target: match self.kind.names_target() {
                true => label.map(|(offset, pc)| pc.target(address, offset, bits)),
                false => None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BranchEffect, BranchKind};
    use crate::address::PcRead;

    const A64: PcRead = PcRead { ahead: 0, align: 1 };

    fn effect(kind: BranchKind, conditional: bool) -> BranchEffect {
        BranchEffect { kind, conditional }
    }

    #[test]
    fn a_direct_branch_names_where_it_goes() {
        let forward = effect(BranchKind::Direct, false).at(0x1000, Some((0x20, A64)), 64);
        assert_eq!(forward.target, Some(0x1020));
        let back = effect(BranchKind::Direct, true).at(0x1000, Some((-0x10, A64)), 64);
        assert_eq!(back.target, Some(0xff0));
        assert!(back.conditional);
    }

    /// A label written for another purpose is not a target.
    #[test]
    fn an_indirect_branch_names_no_target() {
        for kind in [
            BranchKind::Indirect,
            BranchKind::IndirectCall,
            BranchKind::Return,
        ] {
            let branch = effect(kind, false).at(0x1000, Some((0x20, A64)), 64);
            assert_eq!(branch.target, None, "{kind:?} names no target");
        }
    }

    /// A32 reads the PC eight bytes ahead, and the target wraps only after
    /// the offset is applied.
    #[test]
    fn a_target_wraps_within_the_instruction_sets_reach() {
        let a32 = PcRead { ahead: 8, align: 1 };
        let branch = effect(BranchKind::Direct, true).at(0xffff_fffc, Some((0x10, a32)), 32);
        assert_eq!(branch.target, Some(0x14));
    }

    #[test]
    fn an_exception_leaves_where_it_goes_on_to_its_handler() {
        for kind in [
            BranchKind::Direct,
            BranchKind::Indirect,
            BranchKind::DirectCall,
            BranchKind::IndirectCall,
            BranchKind::Return,
            BranchKind::ExceptionReturn,
        ] {
            assert!(kind.sets_the_pc(), "{kind:?} sets the PC");
        }
        for kind in [
            BranchKind::None,
            BranchKind::SystemCall,
            BranchKind::Exception,
            BranchKind::Halt,
        ] {
            assert!(!kind.sets_the_pc(), "{kind:?} does not set the PC");
        }
    }
}
