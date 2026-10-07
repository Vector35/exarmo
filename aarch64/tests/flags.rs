//! The flag effect an instruction reports, against what the architecture
//! says each of these does, and over the whole corpus against its encoding,
//! its condition and its flag-setting sibling.

use std::collections::HashMap;

mod corpus;

use exarmo_aarch64::{Encoding, FlagEffect, Flags, Mnemonic, Operand, decode_word};

fn effect(bits: u32) -> FlagEffect {
    decode_word(bits).unwrap().flags()
}

#[test]
fn integer_arithmetic() {
    assert_eq!(effect(0x8B020020), FlagEffect::NONE, "add");
    let adds = effect(0xAB020020);
    assert_eq!(
        (adds.writes, adds.reads, adds.float_compare),
        (Flags::NZCV, Flags::NONE, false),
        "adds"
    );
    assert_eq!(
        effect(0xEB02001F).writes,
        Flags::NZCV,
        "cmp, an alias of subs"
    );
    let adc = effect(0x9A020020);
    assert_eq!(
        (adc.writes, adc.reads),
        (Flags::NONE, Flags::C),
        "adc reads the carry"
    );
}

#[test]
fn single_flags_and_float() {
    let cfinv = effect(0xD500401F);
    assert_eq!((cfinv.writes, cfinv.reads), (Flags::C, Flags::C), "cfinv");
    let setf8 = effect(0x3A0009AD);
    assert!(
        setf8.writes.contains(Flags::N)
            && setf8.writes.contains(Flags::Z)
            && setf8.writes.contains(Flags::V),
        "setf8 {setf8:?}"
    );
    assert!(
        !setf8.writes.contains(Flags::C),
        "setf8 leaves the carry {setf8:?}"
    );
    // Its Execute says so in a comment, `//PSTATE.C unchanged;`, and leaving
    // the carry alone is not reading it. A reading that cannot tell a comment
    // from a statement says it does.
    assert_eq!(setf8.reads, Flags::NONE, "setf8 reads nothing {setf8:?}");
    let setf16 = effect(0x3A0049AD);
    assert_eq!(
        (setf16.writes, setf16.reads),
        (setf8.writes, Flags::NONE),
        "setf16 does as setf8 does {setf16:?}"
    );
    let fcmp = effect(0x1E212000);
    assert!(
        fcmp.float_compare && fcmp.writes == Flags::NZCV,
        "fcmp {fcmp:?}"
    );
}

#[test]
fn conditions_read() {
    assert_eq!(effect(0x54000040).reads, Flags::NZCV, "b.eq");
    let csel = effect(0x9A820020);
    assert_eq!(
        (csel.reads, csel.writes),
        (Flags::NZCV, Flags::NONE),
        "csel"
    );
}

#[test]
fn sve_setflags_is_a_constant_per_instruction() {
    assert_eq!(effect(0x25434440).writes, Flags::NZCV, "ands p0.b");
    assert_eq!(effect(0x25034440).writes, Flags::NONE, "and p0.b");
}

/// Over the whole corpus, what an instruction does with the flags is its
/// encoding's, whatever its fields hold, except that the condition it is
/// written under reads all four flags unless it always holds. A comparison
/// of floating-point values writes all four.
#[test]
fn every_instruction_touches_the_flags_its_encoding_does() {
    let mut failures = Vec::new();
    let mut checked = 0;
    // The effect each encoding was first seen with, and where, for an
    // encoding whose effect does not depend on a condition
    let mut first: HashMap<Encoding, (String, FlagEffect)> = HashMap::new();
    for (case, inst) in corpus::instructions() {
        let effect = inst.flags();
        checked += 1;
        let named = corpus::named(&case, &inst);
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
        let cond = inst.operands().iter().find_map(|operand| match operand {
            Operand::Cond(cond) => Some(*cond),
            _ => None,
        });
        match cond {
            Some(cond) => {
                let reads = Flags::for_condition(cond.bits() as u8);
                if effect.reads != reads {
                    fail(format!(
                        "is written under {cond} and reads {:?}",
                        effect.reads
                    ));
                }
            }
            None => match first.get(&inst.encoding()) {
                Some((there, seen)) if *seen != effect => {
                    fail(format!("{effect:?} where {there} is {seen:?}"))
                }
                Some(_) => {}
                None => {
                    first.insert(inst.encoding(), (named.clone(), effect));
                }
            },
        }
    }
    exarmo_testing::report(
        "an instruction touches the flags its encoding does",
        checked,
        &failures,
    );
}

/// The architecture names an instruction that sets the flags by adding an
/// `s` to the mnemonic of the one that does not, and encodes the two a bit
/// or two apart: ADD and ADDS, SVE's AND and ANDS on predicates. Over the
/// corpus, every word within two bits of its flag-setting sibling writes no
/// flag the sibling does not, and the sibling writes one it does not, while
/// reading the same.
///
/// The sibling writes the same operands. An `s` means other things too, as
/// FCVTNS is FCVTN's signed neighbour two bits away, and operands written
/// alike are what tell an instruction's own flag-setting form from those.
#[test]
fn a_flag_setting_sibling_sets_flags_the_other_does_not() {
    let setting: std::collections::HashSet<&str> = (0..Mnemonic::COUNT)
        .filter_map(Mnemonic::from_index)
        .map(|mnemonic| mnemonic.name())
        .collect();
    let mut failures = Vec::new();
    let mut checked = 0;
    for (case, inst) in corpus::instructions() {
        let sibling = format!("{}s", inst.mnemonic().name());
        if !setting.contains(sibling.as_str()) {
            continue;
        }
        let effect = inst.flags();
        let written = inst.at(corpus::INSTR_ADDRESS).to_string();
        let operands = written.split_once('\t').map(|(_, operands)| operands);
        let flips = (0..32)
            .map(|bit| 1u32 << bit)
            .chain((0..32).flat_map(|a| (a + 1..32).map(move |b| (1u32 << a) | (1 << b))));
        for flip in flips {
            let Ok(other) = decode_word(case.encoding ^ flip) else {
                continue;
            };
            let theirs_written = other.at(corpus::INSTR_ADDRESS).to_string();
            if other.mnemonic().name() != sibling
                || theirs_written
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
                    corpus::named(&case, &inst),
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
