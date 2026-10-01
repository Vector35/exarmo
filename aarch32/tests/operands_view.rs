//! The operands view agrees with the text over the whole corpus. Every
//! register the formatter writes is a register the view holds, every
//! immediate a value it holds, and every name from a table a name it holds.
//! The test also reports how much of the corpus the view describes.

use std::collections::{BTreeMap, HashSet};

mod corpus;

use corpus::Set;
use exarmo_aarch32::{Lane, ListFile, Modifier, Offset, Operand, Reg};
use exarmo_core::tokens::{Token, TokenKind};

fn modifier(
    m: Option<Modifier>,
    names: &mut HashSet<String>,
    registers: &mut HashSet<String>,
    numbers: &mut HashSet<i64>,
) {
    if let Some(m) = m {
        names.insert(m.kind.to_string());
        if let Some(amount) = m.amount {
            numbers.insert(i64::from(amount));
        }
        if let Some(reg) = m.by {
            registers.insert(reg.to_string());
        }
    }
}

/// Every name, register and number a view holds.
fn held(operands: &[Operand]) -> (HashSet<String>, HashSet<String>, HashSet<i64>, bool) {
    let mut names = HashSet::new();
    let mut registers = HashSet::new();
    let mut numbers = HashSet::new();
    let mut label = false;
    fn file(registers: &mut HashSet<String>, reg: &Reg) {
        registers.insert(reg.to_string());
    }
    for operand in operands {
        match operand {
            Operand::Reg(r) => {
                file(&mut registers, &r.reg);
                if let Some(index) = r.index {
                    numbers.insert(i64::from(index));
                }
                modifier(r.modifier, &mut names, &mut registers, &mut numbers);
            }
            Operand::Imm { value, modifier: m } => {
                numbers.insert(*value);
                modifier(*m, &mut names, &mut registers, &mut numbers);
            }
            Operand::FpImm { .. } => {}
            Operand::Label { .. } => label = true,
            Operand::Mem(m) => {
                file(&mut registers, &m.base);
                if let Some(align) = m.align {
                    numbers.insert(i64::from(align));
                }
                match m.offset {
                    Offset::None => {}
                    Offset::Imm { value, .. } => {
                        numbers.insert(value);
                    }
                    Offset::Reg {
                        reg, modifier: m, ..
                    } => {
                        file(&mut registers, &reg);
                        modifier(m, &mut names, &mut registers, &mut numbers);
                    }
                }
            }
            Operand::List(list) => {
                for number in 0..32u8 {
                    if list.mask & (1 << number) != 0 {
                        file(
                            &mut registers,
                            &match list.file {
                                ListFile::Core => Reg::Core(exarmo_aarch32::GpReg::new(number)),
                                ListFile::Single => Reg::Single(exarmo_aarch32::SReg::new(number)),
                                ListFile::Double => Reg::Double(exarmo_aarch32::DReg::new(number)),
                            },
                        );
                    }
                }
                if let Lane::Index(index) = list.lane {
                    numbers.insert(i64::from(index));
                }
            }
            Operand::Cond(cond) => {
                names.insert(cond.to_string());
            }
            Operand::Symbol(s) => {
                // A named register, MRC's `apsr_nzcv`, is written as a
                // register and is one, so its name answers for either token.
                names.insert(s.name.to_string());
                registers.insert(s.name.to_string());
                numbers.insert(i64::from(s.bits));
            }
            Operand::Modifier(m) => modifier(Some(*m), &mut names, &mut registers, &mut numbers),
            Operand::Other => {}
        }
    }
    (names, registers, numbers, label)
}

fn check(set: Set) -> (usize, usize, BTreeMap<String, usize>) {
    let mut rendered = 0;
    let mut with_other = 0;
    let mut disagreements: BTreeMap<String, usize> = BTreeMap::new();
    for case in set.corpus() {
        let Ok(inst) = set.decode_case(&case) else {
            continue;
        };
        let mut sink: Vec<Token> = Vec::new();
        if inst
            .at(corpus::INSTR_ADDRESS)
            .write_tokens(&mut sink)
            .is_err()
        {
            continue;
        }
        rendered += 1;
        let operands = inst.operands();
        if operands.iter().any(|op| matches!(op, Operand::Other)) {
            with_other += 1;
        }
        let (names, registers, numbers, label) = held(&operands);
        for token in &sink {
            let agrees = match token.kind {
                TokenKind::Register => registers.contains(&token.text),
                TokenKind::Symbol => names.contains(&token.text),
                TokenKind::Immediate(value) => numbers.contains(&value),
                TokenKind::Integer(value) => numbers.contains(&(value as i64)),
                TokenKind::Address(_) => label,
                _ => true,
            };
            if !agrees {
                *disagreements
                    .entry(format!("{} {:?} {}", case.hex, token.kind, token.text))
                    .or_default() += 1;
            }
        }
    }
    (rendered, with_other, disagreements)
}

#[test]
fn the_view_holds_what_the_text_says() {
    for set in [Set::A32, Set::T32] {
        let (rendered, with_other, disagreements) = check(set);
        eprintln!(
            "{set:?}: {rendered} rendered, {with_other} hold an operand the view does not describe"
        );
        assert!(
            disagreements.is_empty(),
            "{set:?}: the text says what the view does not hold:\n{}",
            disagreements
                .keys()
                .take(40)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

/// Each token's operand is an index the instruction has, and a run of one
/// operand does not come back once left. The mnemonic, the gap after it and
/// the separators between operands are part of none, and a separator is only
/// ever between two operands.
///
/// The brackets are why this matters. A memory operand's are its own and a
/// scalar's index belongs to the register it is written after, yet both are
/// written `[`, so their position alone does not say which is which.
#[test]
fn every_token_says_which_operand_it_is_part_of() {
    for set in [Set::A32, Set::T32] {
        let mut failures: Vec<String> = Vec::new();
        let mut unmarked: HashSet<String> = HashSet::new();
        let mut marked = 0usize;
        for case in set.corpus() {
            let Ok(inst) = set.decode_case(&case) else {
                continue;
            };
            let mut sink: Vec<Token> = Vec::new();
            if inst
                .at(corpus::INSTR_ADDRESS)
                .write_tokens(&mut sink)
                .is_err()
            {
                continue;
            }
            let view = inst.operands();
            let mut seen: Vec<u8> = Vec::new();
            for token in &sink {
                let Some(operand) = token.operand else {
                    unmarked.insert(match token.kind {
                        TokenKind::Text => format!("Text {:?}", token.text),
                        kind => format!("{kind:?}"),
                    });
                    continue;
                };
                marked += 1;
                if usize::from(operand) >= view.len() {
                    failures.push(format!(
                        "{}: {:?} names operand {operand} of {}",
                        case.hex,
                        token.text,
                        view.len()
                    ));
                    continue;
                }
                match seen.last() {
                    Some(last) if *last == operand => {}
                    _ if seen.contains(&operand) => failures.push(format!(
                        "{}: operand {operand} is written in two places",
                        case.hex
                    )),
                    _ => seen.push(operand),
                }
            }
            // A separator is between two operands and a `, ` within one is its
            // text, so a consumer counting separators counts operands.
            for (at, token) in sink.iter().enumerate() {
                if token.kind != TokenKind::Separator {
                    continue;
                }
                let before = sink[..at].iter().rev().find_map(|t| t.operand);
                let after = sink[at + 1..].iter().find_map(|t| t.operand);
                if token.operand.is_some() || before.is_none() || after.is_none() || before == after
                {
                    failures.push(format!(
                        "{}: the separator at token {at} is not between two operands",
                        case.hex
                    ));
                }
            }
            // The assembly writes the operands in the order the view
            // holds them, so the marks run up. They need not start at the
            // first, because a condition of AL is written as nothing.
            if seen.windows(2).any(|pair| pair[0] >= pair[1]) {
                failures.push(format!("{}: operands written as {seen:?}", case.hex));
            }
        }
        assert!(marked > 0, "{set:?} writes operands");
        assert!(
            failures.is_empty(),
            "{set:?}: {} of them:\n{}",
            failures.len(),
            failures
                .iter()
                .take(20)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
        // Only what is written between operands is part of none. AArch32
        // writes the condition and the width onto the mnemonic, so those
        // are mnemonic tokens rather than text.
        let mut unmarked: Vec<String> = unmarked.into_iter().collect();
        unmarked.sort();
        assert_eq!(
            unmarked,
            vec![
                "Mnemonic".to_string(),
                "Separator".to_string(),
                "Text \"\\t\"".to_string(),
            ],
            "{set:?}"
        );
    }
}

/// Which kinds of token an operand of each kind may be written with, and
/// the kinds of which it must be written with one: its own value, which a
/// consumer reads off the token rather than the text. `#`, brackets and the
/// punctuation within an operand are text anyone may write, but a label is
/// written as its address and nothing else.
///
/// AArch32 writes more onto the mnemonic than a condition. The `.8` of
/// `aesd.8` and the `.f32` of `vadd.f32` are operands of the view too, and
/// what is written onto the mnemonic is a mnemonic token.
fn written_with(operand: &Operand) -> Option<(&'static [&'static str], &'static [&'static str])> {
    let kinds: (&[&str], &[&str]) = match operand {
        Operand::Reg(_) => (
            &[
                "Register",
                "Text",
                "Bracket",
                "Symbol",
                "Integer",
                "Immediate",
            ],
            &["Register"],
        ),
        Operand::Imm { .. } => (
            &["Text", "Immediate", "Integer", "Symbol", "Mnemonic"],
            &["Immediate", "Integer", "Mnemonic"],
        ),
        Operand::FpImm { .. } => (&["Text", "Float"], &["Float"]),
        Operand::Label { .. } => (&["Address"], &["Address"]),
        Operand::Mem(_) => (
            &[
                "Bracket",
                "Register",
                "Text",
                "Immediate",
                "Integer",
                "Symbol",
            ],
            &["Register"],
        ),
        // An empty list, which the architecture calls unpredictable, writes
        // only its braces, and the `^` of a user-mode LDM or STM
        Operand::List(list) if list.mask == 0 => (&["Bracket", "Text"], &["Bracket"]),
        Operand::List(_) => (&["Bracket", "Register", "Text", "Integer"], &["Register"]),
        // Written onto the mnemonic, and not at all for AL
        Operand::Cond(_) => (&["Symbol", "Mnemonic"], &["Symbol", "Mnemonic"]),
        // A value its table does not name is written as a number, and a
        // named register, MRC's `apsr_nzcv`, as a register
        Operand::Symbol(_) => (
            &[
                "Symbol",
                "Mnemonic",
                "Immediate",
                "Integer",
                "Register",
                "Text",
            ],
            &["Symbol", "Mnemonic", "Immediate", "Integer", "Register"],
        ),
        Operand::Modifier(_) => (&["Symbol", "Text", "Immediate", "Register"], &["Symbol"]),
        Operand::Other => return None,
    };
    Some(kinds)
}

/// Over both corpora, every token is of a kind its operand holds, and every
/// operand written is written with a token carrying its value. An optional
/// operand at its default, and a condition of AL, is in the view and not in
/// the text, so an operand no token names is not a failure.
#[test]
fn every_token_is_of_a_kind_its_operand_holds() {
    let mut failures = Vec::new();
    let mut checked = 0;
    for (set, case, inst) in corpus::instructions() {
        let view = inst.operands();
        let mut tokens: Vec<Token> = Vec::new();
        inst.at(corpus::INSTR_ADDRESS)
            .write_tokens(&mut tokens)
            .unwrap();
        checked += 1;
        for (index, operand) in view.iter().enumerate() {
            let Some((may, must)) = written_with(operand) else {
                continue;
            };
            for problem in exarmo_testing::invariant::kinds_written(&tokens, index, may, must) {
                failures.push(format!(
                    "{}: operand {index} {operand:?} {problem}",
                    corpus::named(set, &case, &inst)
                ));
            }
        }
    }
    exarmo_testing::report(
        "every token is of a kind its operand holds",
        checked,
        &failures,
    );
}
