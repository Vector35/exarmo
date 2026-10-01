//! What an instruction reports doing to the flow of control, against what
//! the architecture says each of these does, and over both corpora against
//! what its own operands and tokens say.

mod corpus;

use corpus::Set;
use exarmo_aarch32::{Branch, BranchKind, Flags, ItState, Operand, Token, a32, t32};

/// The branch an A32 word takes, read as though it sat at 0x1000.
fn a32(bits: u32) -> Branch {
    a32::decode(bits).unwrap().at(0x1000).branch()
}

/// The branch a T32 instruction takes outside any IT block, its first
/// halfword in the high half of `bits`.
fn t32(bits: u32) -> Branch {
    t32::decode(bits, ItState::Outside)
        .unwrap()
        .at(0x1000)
        .branch()
}

/// A32 reads the PC eight bytes ahead of the instruction.
#[test]
fn a_direct_branch_names_its_target() {
    let b = a32(0xEA000000);
    assert_eq!(b.kind, BranchKind::Direct, "b");
    assert_eq!(b.target, Some(0x1008));
    assert!(!b.conditional, "an AL branch is taken every time");

    let bl = a32(0xEB000000);
    assert_eq!(bl.kind, BranchKind::DirectCall, "bl");
    assert_eq!(bl.target, Some(0x1008));

    let beq = a32(0x0A000000);
    assert_eq!(beq.kind, BranchKind::Direct, "beq");
    assert!(beq.conditional);
}

#[test]
fn a_branch_through_a_register_names_no_target() {
    for (bits, expected, written) in [
        (0xE12FFF1E, BranchKind::Indirect, "bx lr"),
        (0xE12FFF33, BranchKind::IndirectCall, "blx r3"),
    ] {
        let branch = a32(bits);
        assert_eq!(branch.kind, expected, "{written}");
        assert_eq!(branch.target, None, "{written} names no target");
    }
}

/// Each pair shares an encoding. Only writing the PC makes it a branch.
#[test]
fn writing_the_pc_is_a_branch() {
    for (bits, expected, written) in [
        (0xE591F000, BranchKind::Indirect, "ldr pc, [r1]"),
        (0xE5910000, BranchKind::None, "ldr r0, [r1]"),
        (0xE8BD8010, BranchKind::Indirect, "pop {r4, pc}"),
        (0xE8BD0030, BranchKind::None, "pop {r4, r5}"),
        // LDR's alias, a list of one read back from the register LDR loads.
        (0xE49DF004, BranchKind::Indirect, "pop {pc}"),
        (0xE49D0004, BranchKind::None, "pop {r0}"),
        (0xE1A0F00E, BranchKind::Indirect, "mov pc, lr"),
        (0xE1A00001, BranchKind::None, "mov r0, r1"),
        (0xE08FF000, BranchKind::Indirect, "add pc, pc, r0"),
    ] {
        assert_eq!(a32(bits).kind, expected, "{written}");
    }
}

/// Setting the flags while writing the PC restores them from the SPSR, which
/// is a return from an exception.
#[test]
fn setting_the_flags_into_the_pc_returns_from_an_exception() {
    assert_eq!(
        a32(0xE1B0F00E).kind,
        BranchKind::ExceptionReturn,
        "movs pc, lr"
    );
}

#[test]
fn an_exception_is_the_call_that_takes_it() {
    assert_eq!(a32(0xEF000000).kind, BranchKind::SystemCall, "svc #0");
    assert_eq!(a32(0xE1200070).kind, BranchKind::Exception, "bkpt");
    assert_eq!(a32(0xE7F000F0).kind, BranchKind::Exception, "udf");
}

/// CBZ branches on a register rather than on the flags, which makes it
/// conditional without a condition.
#[test]
fn t32_reads_the_same_way() {
    let cbz = t32(0xB1000000);
    assert_eq!(cbz.kind, BranchKind::Direct, "cbz");
    assert!(cbz.conditional);
    assert_eq!(cbz.target, Some(0x1004), "T32 reads the PC four ahead");

    assert_eq!(t32(0xBD000000).kind, BranchKind::Indirect, "pop {{pc}}");
    assert_eq!(t32(0xBC010000).kind, BranchKind::None, "pop {{r0}}");
    assert_eq!(
        t32(0xF85DFB04).kind,
        BranchKind::Indirect,
        "LDR's pop {{pc}}"
    );
    assert_eq!(t32(0x47700000).kind, BranchKind::Indirect, "bx lr");
    assert_eq!(t32(0xF000B800).kind, BranchKind::Direct, "b.w");
}

/// A32's BLX (immediate) has its cond field fixed at 1111, so it holds no
/// condition.
#[test]
fn an_unconditional_encoding_branches_every_time() {
    let blx = a32(0xFA000000);
    assert_eq!(blx.kind, BranchKind::DirectCall, "blx label");
    assert!(!blx.conditional);
}

/// Over both corpora, a branch that names its target names the address its
/// label does, in the view and in the token stream alike, and one that names
/// none has no target. A32 reads the PC eight bytes ahead and T32 four, and
/// a literal load aligns it down, which the label carries.
#[test]
fn every_branch_goes_where_its_label_says() {
    let mut failures = Vec::new();
    let mut checked = 0;
    for (set, case, inst) in corpus::instructions() {
        let at = inst.at(corpus::INSTR_ADDRESS);
        let labels: Vec<(usize, u64)> = inst
            .operands()
            .iter()
            .enumerate()
            .filter_map(|(index, operand)| match operand {
                Operand::Label { offset, pc } => {
                    Some((index, pc.target(corpus::INSTR_ADDRESS, *offset, 32)))
                }
                _ => None,
            })
            .collect();
        let mut tokens: Vec<Token> = Vec::new();
        at.write_tokens(&mut tokens).unwrap();
        checked += 1;
        for problem in
            exarmo_testing::invariant::branch_goes_where_labelled(at.branch(), &labels, &tokens)
        {
            failures.push(format!("{}: {problem}", corpus::named(set, &case, &inst)));
        }
    }
    exarmo_testing::report("a branch goes where its label says", checked, &failures);
}

/// Where a branch is conditional other than its condition says: an
/// instruction that takes no branch is taken under nothing, one written
/// under a condition is conditional exactly where the condition can fail,
/// and one that reads every flag is taken under them.
///
/// Almost every A32 instruction is written under a condition, and an SVC
/// under one raises its exception only where it holds, so an
/// exception is as conditional as a branch. A branch without a condition
/// may still test something, as CBZ tests a register.
fn conditional_other_than_its_condition(inst: &exarmo_aarch32::Instruction) -> Vec<String> {
    let branch = inst.at(corpus::INSTR_ADDRESS).branch();
    let mut out = Vec::new();
    if branch.kind == BranchKind::None {
        if branch.conditional {
            out.push("takes no branch and is conditional".to_string());
        }
        return out;
    }
    for operand in inst.operands().iter() {
        if let Operand::Cond(cond) = operand {
            let can_fail = !exarmo_core::condition::always_holds(*cond as u8);
            if can_fail != branch.conditional {
                out.push(format!(
                    "is written under {cond} and conditional is {}",
                    branch.conditional
                ));
            }
        }
    }
    if inst.flags().reads == Flags::NZCV && !branch.conditional {
        out.push("reads every flag and branches every time".to_string());
    }
    out
}

/// Over both corpora, a branch is conditional where a test it makes can
/// fail, and only then.
#[test]
fn every_branch_is_conditional_where_its_test_can_fail() {
    let mut failures = Vec::new();
    let mut checked = 0;
    for (set, case, inst) in corpus::instructions() {
        checked += 1;
        for problem in conditional_other_than_its_condition(&inst) {
            failures.push(format!("{}: {problem}", corpus::named(set, &case, &inst)));
        }
    }
    exarmo_testing::report(
        "a branch is conditional where its test can fail",
        checked,
        &failures,
    );
}

/// Over the T32 corpus decoded inside an IT block, an instruction that
/// branches or raises an exception is conditional exactly where it reads
/// the flags the block's condition tests or makes a test of its own, as CBZ
/// tests a register outside a block too, and one that takes no branch is
/// taken under nothing. The flags are read from where the Execute reads
/// them and the branch from the tests it branches under, so the two agree
/// only where both read the block's condition.
///
/// Not every instruction in a block is under its condition. BKPT and HLT
/// are unconditional even there, and an HVC the architecture calls
/// unpredictable in a block is written as its Execute has it, unconditional
/// too.
#[test]
fn inside_an_it_block_a_branch_is_conditional_where_it_reads_the_flags() {
    let mut failures = Vec::new();
    let mut checked = 0;
    let mut seen = std::collections::HashSet::new();
    for case in Set::T32.corpus() {
        if !seen.insert(case.encoding) {
            continue;
        }
        let Ok(inst) = t32::decode(case.encoding, corpus::INSIDE) else {
            continue;
        };
        checked += 1;
        let branch = inst.at(corpus::INSTR_ADDRESS).branch();
        let reads = inst.flags().reads;
        let outside = t32(case.encoding).conditional;
        let expected = branch.kind != BranchKind::None && (reads == Flags::NZCV || outside);
        if branch.conditional != expected {
            failures.push(format!(
                "{}: {:?} reads {reads:?} and is conditional {} inside a block",
                corpus::named(Set::T32, &case, &inst),
                branch.kind,
                branch.conditional
            ));
        }
    }
    exarmo_testing::report(
        "inside an IT block a branch is conditional where it reads the flags",
        checked,
        &failures,
    );
}
