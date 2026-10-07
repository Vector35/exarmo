//! What an instruction reports doing to the flow of control, against what
//! the architecture says each of these does, and over the whole corpus
//! against what its own operands and tokens say.

mod corpus;

use exarmo_aarch64::{Branch, BranchKind, Flags, Operand, Token, decode_word};

/// The branch a word takes, read as though the instruction sat at 0x1000.
fn branch(bits: u32) -> Branch {
    decode_word(bits).unwrap().at(0x1000).branch()
}

fn kind(bits: u32) -> BranchKind {
    branch(bits).kind
}

#[test]
fn a_direct_branch_names_its_target() {
    let b = branch(0x14000002);
    assert_eq!(b.kind, BranchKind::Direct);
    assert_eq!(b.target, Some(0x1008), "b .+8");
    assert!(!b.conditional);

    let bl = branch(0x94000002);
    assert_eq!(bl.kind, BranchKind::DirectCall);
    assert_eq!(bl.target, Some(0x1008), "bl .+8");
}

/// A64 reads the PC as the instruction's own address.
#[test]
fn a_branch_backwards_counts_back_from_the_instruction() {
    assert_eq!(branch(0x17FFFFFF).target, Some(0xffc), "b .-4");
}

/// CBZ, CBNZ, TBZ and TBNZ test something other than the flags and are
/// conditional too.
#[test]
fn a_conditional_branch_says_it_may_not_be_taken() {
    for (bits, written) in [
        (0x54000040, "b.eq"),
        (0xB4000040, "cbz"),
        (0xB5000040, "cbnz"),
        (0xB6000040, "tbz"),
        (0xB7000040, "tbnz"),
    ] {
        let branch = branch(bits);
        assert_eq!(branch.kind, BranchKind::Direct, "{written}");
        assert!(branch.conditional, "{written} may not be taken");
        assert_eq!(branch.target, Some(0x1008), "{written} names its target");
    }
}

/// NV holds whatever the flags are, as AL does, because the architecture
/// exempts it from the inversion.
#[test]
fn a_condition_that_always_holds_is_no_condition() {
    assert!(branch(0x54000040).conditional, "b.eq");
    assert!(!branch(0x5400004E).conditional, "b.al");
    assert!(!branch(0x5400004F).conditional, "b.nv branches every time");
}

#[test]
fn a_branch_through_a_register_names_no_target() {
    for (bits, expected, written) in [
        (0xD61F0000, BranchKind::Indirect, "br x0"),
        (0xD63F0000, BranchKind::IndirectCall, "blr x0"),
        (0xD65F03C0, BranchKind::Return, "ret"),
        (0xD71F0801, BranchKind::Indirect, "braa x0, x1"),
        (0xD73F0801, BranchKind::IndirectCall, "blraa x0, x1"),
        (0xD65F0BFF, BranchKind::Return, "retaa"),
    ] {
        let branch = branch(bits);
        assert_eq!(branch.kind, expected, "{written}");
        assert_eq!(branch.target, None, "{written} names no target");
    }
}

/// RETAASPPC writes a label, which the guarded control stack checks against
/// rather than branches to, so it is not the target.
#[test]
fn a_return_that_writes_a_label_does_not_branch_to_it() {
    let retaasppc = branch(0x5500003F);
    assert_eq!(retaasppc.kind, BranchKind::Return);
    assert_eq!(retaasppc.target, None);
}

#[test]
fn an_exception_is_told_from_a_branch() {
    assert_eq!(kind(0xD4000001), BranchKind::SystemCall, "svc");
    assert_eq!(kind(0xD4000002), BranchKind::SystemCall, "hvc");
    assert_eq!(kind(0xD4000003), BranchKind::SystemCall, "smc");
    assert_eq!(kind(0xD4E00000), BranchKind::SystemCall, "tenter #0");
    assert_eq!(kind(0xD6FF03E0), BranchKind::ExceptionReturn, "texit");
    assert_eq!(kind(0xD69F03E0), BranchKind::ExceptionReturn, "eret");
    assert_eq!(kind(0xD6BF03E0), BranchKind::ExceptionReturn, "drps");
    assert_eq!(kind(0xD4200000), BranchKind::Exception, "brk");
    assert_eq!(kind(0xD4400000), BranchKind::Halt, "hlt");
}

/// UDF's Execute says nothing at all, and its decode says the encoding is
/// permanently undefined, which is the exception it raises.
#[test]
fn a_permanently_undefined_encoding_raises_an_exception() {
    assert_eq!(kind(0x00000000), BranchKind::Exception, "udf #0");
}

/// ADR and a literal load write a label that names an address to read, not
/// one to run.
#[test]
fn naming_an_address_is_not_branching_to_it() {
    for (bits, written) in [
        (0x10000040, "adr x0, .+8"),
        (0x90000000, "adrp x0, .+0"),
        (0x58000040, "ldr x0, .+8"),
    ] {
        let branch = branch(bits);
        assert_eq!(branch.kind, BranchKind::None, "{written}");
        assert_eq!(branch.target, None, "{written} names no branch target");
    }
}

#[test]
fn an_instruction_that_does_not_branch_says_so() {
    let add = branch(0x91000400);
    assert_eq!(add.kind, BranchKind::None, "add x0, x0, #1");
    assert!(!add.conditional);
    assert_eq!(add.target, None);
    assert_eq!(kind(0xD503201F), BranchKind::None, "nop");
    assert_eq!(kind(0xF9400000), BranchKind::None, "ldr x0, [x0]");
}

/// Over the whole corpus, a branch that names its target names the address
/// its label does, in the view and in the token stream alike, and one that
/// names none has no target. A consumer laying out blocks follows the
/// target, and a lifter reading the label or the token must arrive at the
/// same place.
#[test]
fn every_branch_goes_where_its_label_says() {
    let mut failures = Vec::new();
    let mut checked = 0;
    for (case, inst) in corpus::instructions() {
        let at = inst.at(corpus::INSTR_ADDRESS);
        let branch = at.branch();
        let labels: Vec<(usize, u64)> = inst
            .operands()
            .iter()
            .enumerate()
            .filter_map(|(index, operand)| match operand {
                Operand::Label { offset, pc } => {
                    Some((index, pc.target(corpus::INSTR_ADDRESS, *offset, 64)))
                }
                _ => None,
            })
            .collect();
        let mut tokens: Vec<Token> = Vec::new();
        at.write_tokens(&mut tokens).unwrap();
        checked += 1;
        for problem in
            exarmo_testing::invariant::branch_goes_where_labelled(branch, &labels, &tokens)
        {
            failures.push(format!("{}: {problem}", corpus::named(&case, &inst)));
        }
    }
    exarmo_testing::report("a branch goes where its label says", checked, &failures);
}

/// Over the whole corpus, a branch is conditional where a test it makes can
/// fail, and only then: one written under a condition is conditional unless
/// the condition always holds, one that reads the flags is taken under
/// them, and an instruction that takes no branch is taken under nothing.
#[test]
fn every_branch_is_conditional_where_its_test_can_fail() {
    let mut failures = Vec::new();
    let mut checked = 0;
    for (case, inst) in corpus::instructions() {
        let branch = inst.at(corpus::INSTR_ADDRESS).branch();
        checked += 1;
        let mut fail =
            |problem: String| failures.push(format!("{}: {problem}", corpus::named(&case, &inst)));
        if branch.conditional && !branch.kind.sets_the_pc() {
            fail(format!("{:?} is conditional", branch.kind));
        }
        if branch.kind == BranchKind::None {
            continue;
        }
        let reads = inst.flags().reads;
        if reads != Flags::NONE && !branch.conditional {
            fail(format!("reads {reads:?} and branches every time"));
        }
        for operand in inst.operands().iter() {
            if let Operand::Cond(cond) = operand {
                let can_fail = !exarmo_core::condition::always_holds(cond.bits() as u8);
                if can_fail != branch.conditional {
                    fail(format!(
                        "is written under {cond} and conditional is {}",
                        branch.conditional
                    ));
                }
            }
        }
    }
    exarmo_testing::report(
        "a branch is conditional where its test can fail",
        checked,
        &failures,
    );
}
