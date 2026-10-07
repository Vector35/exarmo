//! The flag effect an instruction reports, against what the architecture
//! says each of these does, and over both corpora against its encoding, its
//! condition and its flag-setting sibling.

use std::collections::{HashMap, HashSet};

mod corpus;

use corpus::Set;
use exarmo_aarch32::{Cond, Encoding, FlagEffect, Flags, ItState, Mnemonic, Operand, a32, t32};

fn a32_effect(bits: u32) -> FlagEffect {
    a32::decode_word(bits).unwrap().flags()
}

#[test]
fn integer_arithmetic() {
    assert_eq!(a32_effect(0xe2800001), FlagEffect::NONE, "add r0, r0, #1");
    let adds = a32_effect(0xe2900001);
    assert_eq!(
        (adds.writes, adds.reads, adds.float_compare),
        (Flags::NZCV, Flags::NONE, false),
        "adds r0, r0, #1"
    );
    assert_eq!(a32_effect(0xe3500001).writes, Flags::NZCV, "cmp r0, #1");
    let adc = a32_effect(0xe2a00001);
    assert_eq!(
        (adc.writes, adc.reads),
        (Flags::NONE, Flags::C),
        "adc reads the carry"
    );
    // A shifted register may be rotated through the carry, which the Execute
    // reads whatever the shift turns out to be.
    assert_eq!(a32_effect(0xe0800001).reads, Flags::C, "add r0, r0, r1");
}

#[test]
fn a_condition_reads_every_flag_unless_it_is_always() {
    assert_eq!(a32_effect(0x0a000000).reads, Flags::NZCV, "beq");
    assert_eq!(a32_effect(0xea000000).reads, Flags::NONE, "b");
    assert_eq!(a32_effect(0x02800001).reads, Flags::NZCV, "addeq");
    assert_eq!(a32_effect(0xe2800001).reads, Flags::NONE, "add");
    let adceq = a32_effect(0x02a00001);
    assert_eq!(
        adceq.reads,
        Flags::NZCV,
        "adceq reads the carry and the rest"
    );
}

/// The 16-bit encoding is a form of its own inside and outside a block, and
/// the effect follows the form.
#[test]
fn a_halfword_sets_the_flags_only_outside_an_it_block() {
    let adds = t32::decode_word(0x1840_0000, ItState::Outside)
        .unwrap()
        .flags();
    assert_eq!(adds.writes, Flags::NZCV, "adds r0, r0, r1");
    let add = t32::decode_word(
        0x1840_0000,
        ItState::Inside {
            cond: Cond::Eq,
            mask: 0b1000,
        },
    )
    .unwrap()
    .flags();
    assert_eq!(add.writes, Flags::NONE, "addeq r0, r0, r1");
}

/// VCMP writes `FPSCR.<N,Z,C,V>`, which a later VMRS moves into PSTATE. The
/// effect answers for PSTATE, so VCMP reports writing no flag and
/// `float_compare` is false. A64's FCMP writes PSTATE directly and reports
/// NZCV with `float_compare` true. The condition VCMP is written under is
/// read from PSTATE as usual.
#[test]
fn a_floating_point_compare_writes_no_flag_this_reports() {
    let vcmp = a32_effect(0x9eb4da6a);
    assert_eq!(vcmp.writes, Flags::NONE, "vcmpls.f32 s26, s21");
    assert!(!vcmp.float_compare, "the flags it writes are the FPSCR's");
    assert_eq!(vcmp.reads, Flags::NZCV, "ls reads every flag");
}

/// A wide T32 encoding carries the S bit itself, so it sets the flags
/// whether or not it is inside an IT block, unlike its 16-bit sibling.
#[test]
fn a_wide_encoding_sets_the_flags_from_its_own_bit() {
    let outside = t32::decode_word(0xf11d6707, ItState::Outside)
        .unwrap()
        .flags();
    assert_eq!(outside.writes, Flags::NZCV, "adds.w r7, sp, #...");
    let inside = t32::decode_word(
        0xf11d6707,
        ItState::Inside {
            cond: Cond::Eq,
            mask: 0b1000,
        },
    )
    .unwrap()
    .flags();
    assert_eq!(
        inside.writes,
        Flags::NZCV,
        "the S bit is the encoding's, not the block's"
    );
    assert!(!outside.float_compare, "an integer add is no FP compare");
}

/// The condition an instruction is written under, where it holds one. IT
/// holds the condition of the block it begins, which the instructions in
/// the block are under and it is not.
fn condition(inst: &exarmo_aarch32::Instruction) -> Option<Cond> {
    if inst.it_state_set().is_some() {
        return None;
    }
    inst.operands().iter().find_map(|operand| match operand {
        Operand::Cond(cond) => Some(*cond),
        _ => None,
    })
}

/// Over both corpora, what an instruction does with the flags is its
/// encoding's, whatever its fields hold, except that the condition it is
/// written under reads all four flags unless it always holds. What an
/// instruction reads besides, as ADC reads the carry, is hidden by a
/// condition that reads everything, so the encoding's reads are compared
/// only where the condition always holds. A comparison of floating-point
/// values writes all four.
#[test]
fn every_instruction_touches_the_flags_its_encoding_does() {
    let mut failures = Vec::new();
    let mut checked = 0;
    // What each encoding was first seen writing, and reading under a
    // condition that always holds, and where
    let mut writes: HashMap<Encoding, (String, Flags)> = HashMap::new();
    let mut reads: HashMap<Encoding, (String, Flags)> = HashMap::new();
    for (set, case, inst) in corpus::instructions() {
        let effect = inst.flags();
        checked += 1;
        let named = corpus::named(set, &case, &inst);
        let mut fail = |problem: String| failures.push(format!("{named}: {problem}"));
        if !Flags::NZCV.contains(effect.writes) || !Flags::NZCV.contains(effect.reads) {
            fail(format!("{effect:?} names a flag there is not"));
        }
        if effect.float_compare && effect.writes != Flags::NZCV {
            fail(format!(
                "compares floating-point values and writes {:?}",
                effect.writes
            ));
        }
        let always = match condition(&inst) {
            Some(cond) => {
                let can_fail = Flags::for_condition(cond as u8);
                if !effect.reads.contains(can_fail) {
                    fail(format!(
                        "is written under {cond} and reads {:?}",
                        effect.reads
                    ));
                }
                can_fail.is_empty()
            }
            None => true,
        };
        let mut agree =
            |seen: &mut HashMap<Encoding, (String, Flags)>, what: &str, flags| match seen
                .get(&inst.encoding())
            {
                Some((there, first)) if *first != flags => {
                    fail(format!("{what} {flags:?} where {there} {what} {first:?}"))
                }
                Some(_) => {}
                None => {
                    seen.insert(inst.encoding(), (named.clone(), flags));
                }
            };
        agree(&mut writes, "writes", effect.writes);
        if always {
            agree(&mut reads, "reads", effect.reads);
        }
    }
    exarmo_testing::report(
        "an instruction touches the flags its encoding does",
        checked,
        &failures,
    );
}

/// The bits a word of the set spends on the instruction: a 16-bit T32
/// instruction is the high half alone.
fn instruction_bits(set: Set, inst: &exarmo_aarch32::Instruction) -> std::ops::Range<u32> {
    match (set, inst.encoding().length()) {
        (Set::T32, 2) => 16..32,
        _ => 0..32,
    }
}

/// The architecture names an instruction that sets the flags by adding an
/// `s` to the mnemonic of the one that does not, and encodes the two a bit
/// or two apart: ADD and ADDS, MOV and MOVS. Over both corpora, every word
/// within two bits of its flag-setting sibling writes no flag the sibling
/// does not, and the sibling writes one it does not, while reading the
/// same. The sibling is written under the same condition with the same
/// operands, which tells an instruction's own flag-setting form from a
/// neighbour whose name happens to end in `s`.
#[test]
fn a_flag_setting_sibling_sets_flags_the_other_does_not() {
    let setting: HashSet<&str> = (0..Mnemonic::COUNT)
        .filter_map(Mnemonic::from_index)
        .map(|mnemonic| mnemonic.name())
        .collect();
    let mut failures = Vec::new();
    let mut checked = 0;
    for (set, case, inst) in corpus::instructions() {
        let sibling = format!("{}s", inst.mnemonic().name());
        if !setting.contains(sibling.as_str()) {
            continue;
        }
        let effect = inst.flags();
        let written = inst.at(corpus::INSTR_ADDRESS).to_string();
        let operands = written.split_once('\t').map(|(_, operands)| operands);
        let cond = condition(&inst);
        let bits = instruction_bits(set, &inst);
        let flips = bits.clone().map(|bit| 1u32 << bit).chain(
            bits.clone()
                .flat_map(|a| (a + 1..bits.end).map(move |b| (1u32 << a) | (1 << b))),
        );
        for flip in flips {
            let Ok(other) = set.decode_in(case.encoding ^ flip, case.tag.as_ref()) else {
                continue;
            };
            if other.mnemonic().name() != sibling || condition(&other) != cond {
                continue;
            }
            let theirs_written = other.at(corpus::INSTR_ADDRESS).to_string();
            if theirs_written
                .split_once('\t')
                .map(|(_, operands)| operands)
                != operands
            {
                continue;
            }
            checked += 1;
            let theirs = other.flags();
            if !theirs.writes.contains(effect.writes)
                || theirs.writes == effect.writes
                || theirs.reads != effect.reads
            {
                failures.push(format!(
                    "{}: {effect:?}, and {:08X} {} {theirs:?}",
                    corpus::named(set, &case, &inst),
                    case.encoding ^ flip,
                    theirs_written.replace('\t', " ")
                ));
            }
        }
    }
    exarmo_testing::report(
        "a flag-setting sibling sets flags the other does not",
        checked,
        &failures,
    );
}

/// Over the T32 corpus decoded inside an IT block, an instruction under the
/// block's condition says so twice, as the condition it holds and as the
/// flags it reads, and the two agree. One written under AL outside a block
/// is written under the block's condition inside it and reads every flag.
/// One whose own field holds a condition, as B does, holds it still, since
/// the architecture calls such a word in a block unpredictable and writes
/// it as its fields say. One that holds none, as BKPT and HLT run whatever
/// the condition, reads what it reads outside, unless it is a form of its
/// own inside a block, as the 16-bit ADDS is ADD there, and written under
/// the block's condition.
#[test]
fn inside_an_it_block_the_condition_is_the_blocks() {
    let mut failures = Vec::new();
    let mut checked = 0;
    let mut seen = HashSet::new();
    for case in Set::T32.corpus() {
        if !seen.insert(case.encoding) {
            continue;
        }
        let Ok(inst) = t32::decode_word(case.encoding, corpus::INSIDE) else {
            continue;
        };
        checked += 1;
        let reads = inst.flags().reads;
        let named = corpus::named(Set::T32, &case, &inst);
        let Ok(outside) = t32::decode_word(case.encoding, ItState::Outside) else {
            failures.push(format!("{named}: decodes only inside a block"));
            continue;
        };
        let held = condition(&inst);
        let expected = match condition(&outside) {
            Some(Cond::Al) => Some(Cond::Ne),
            None if held == Some(Cond::Ne) => held,
            held => held,
        };
        if held != expected {
            failures.push(format!(
                "{named}: is written under {held:?} inside a block under ne, and {expected:?} was expected"
            ));
        } else if held == Some(Cond::Ne) && reads != Flags::NZCV {
            failures.push(format!(
                "{named}: is written under ne inside a block and reads {reads:?}"
            ));
        } else if held.is_none() && reads != outside.flags().reads {
            failures.push(format!(
                "{named}: holds no condition and reads {reads:?} inside a block, and {:?} outside",
                outside.flags().reads
            ));
        }
    }
    exarmo_testing::report(
        "inside an IT block the condition is the block's",
        checked,
        &failures,
    );
}
