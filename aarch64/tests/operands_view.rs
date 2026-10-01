//! The operands view agrees with the text over the whole corpus. Every
//! register the formatter writes is a register the view holds, every
//! immediate a value it holds, every name from a table a name it holds, and
//! every memory operand written in brackets is one Mem. The test also
//! reports how much of the corpus the view describes.

use std::collections::{BTreeMap, HashSet};

mod corpus;

use exarmo_aarch64::{Arrangement, Modifier, Offset, Operand, Token, TokenKind, ZaArray, decode};

fn modifier_name(m: Option<Modifier>, out: &mut HashSet<String>) {
    if let Some(m) = m {
        out.insert(m.kind.name().to_string());
    }
}

fn arrangement_name(a: Option<Arrangement>, out: &mut HashSet<String>) {
    if let Some(a) = a {
        out.insert(a.to_string());
    }
}

/// Every name a view holds: a symbol, a condition, a shift or extend, an
/// arrangement, a predication qualifier, a slice direction.
fn names(operands: &[Operand]) -> HashSet<String> {
    let mut out = HashSet::new();
    for operand in operands {
        match operand {
            Operand::Symbol(s) => {
                out.insert(s.name.to_string());
            }
            Operand::SysOp(op) => {
                out.insert(op.def().name.to_string());
            }
            Operand::Cond(c) => {
                out.insert(c.name().to_string());
            }
            Operand::Modifier(m) => {
                out.insert(m.kind.name().to_string());
            }
            Operand::Reg(r) => {
                modifier_name(r.modifier, &mut out);
                arrangement_name(r.arrangement, &mut out);
                if let Some(p) = r.predication {
                    out.insert(p.to_string());
                }
            }
            Operand::Imm { modifier, .. } => modifier_name(*modifier, &mut out),
            Operand::List(list) => arrangement_name(list.arrangement, &mut out),
            Operand::ZaSlice(slice) => {
                arrangement_name(slice.arrangement, &mut out);
                if let Some(d) = slice.direction {
                    out.insert(d.to_string());
                }
            }
            Operand::Mem(m) => {
                arrangement_name(m.base_arrangement, &mut out);
                for o in [m.offset] {
                    match o {
                        Offset::Reg { modifier, .. } => modifier_name(modifier, &mut out),
                        Offset::Vector {
                            modifier,
                            arrangement,
                            ..
                        } => {
                            modifier_name(modifier, &mut out);
                            arrangement_name(arrangement, &mut out);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// The register an offset adds, if any.
fn offset_reg(o: &Offset, out: &mut HashSet<String>) {
    if let Offset::Reg { reg, .. } | Offset::Vector { reg, .. } = o {
        out.insert(reg.to_string());
    }
}

/// Every register a view names, as written.
fn registers(operands: &[Operand]) -> HashSet<String> {
    let mut out = HashSet::new();
    for operand in operands {
        match operand {
            Operand::Reg(r) => {
                out.insert(r.reg.to_string());
                if let Some(index_reg) = r.index_reg {
                    out.insert(index_reg.to_string());
                }
            }
            Operand::ZaSlice(slice) => {
                if let Some(index) = slice.index {
                    out.insert(index.to_string());
                }
                match slice.array {
                    ZaArray::Tile(tile) => {
                        out.insert(tile.to_string());
                    }
                    ZaArray::Zt0 => {
                        out.insert("zt0".to_string());
                    }
                    ZaArray::Za => {}
                }
            }
            Operand::Mem(m) => {
                out.insert(m.base.to_string());
                offset_reg(&m.offset, &mut out);
            }
            Operand::List(list) => {
                for r in list.regs() {
                    out.insert(r.to_string());
                }
            }
            Operand::SysReg(sysreg) => {
                out.insert(sysreg.to_string());
            }
            // ZERO's mask is written as the tiles it names, in one token.
            Operand::TileMask(mask) => {
                out.insert(mask.to_string());
            }
            _ => {}
        }
    }
    out
}

/// The numbers an offset holds, its immediate or its shift amount.
fn offset_ints(o: &Offset, out: &mut HashSet<i64>) {
    match o {
        Offset::Imm { value, .. } => {
            out.insert(*value);
        }
        Offset::Reg { modifier, .. } | Offset::Vector { modifier, .. } => {
            if let Some(a) = modifier.and_then(|m| m.amount) {
                out.insert(i64::from(a));
            }
        }
        Offset::None => {}
    }
}

/// Every integer a view holds: immediates, offsets, amounts, indexes.
fn integers(operands: &[Operand]) -> HashSet<i64> {
    let mut out = HashSet::new();
    for operand in operands {
        match operand {
            Operand::Imm { value, modifier } => {
                out.insert(*value);
                if let Some(a) = modifier.and_then(|m| m.amount) {
                    out.insert(i64::from(a));
                }
            }
            Operand::Reg(r) => {
                if let Some(i) = r.index {
                    out.insert(i64::from(i));
                }
                if let Some(a) = r.modifier.and_then(|m| m.amount) {
                    out.insert(i64::from(a));
                }
            }
            Operand::List(list) => {
                if let Some(i) = list.index {
                    out.insert(i64::from(i));
                }
            }
            Operand::ZaSlice(slice) => {
                out.insert(i64::from(slice.offset));
                if let Some(last) = slice.last {
                    out.insert(i64::from(last));
                }
            }
            Operand::Mem(m) => {
                offset_ints(&m.offset, &mut out);
            }
            Operand::Modifier(m) => {
                if let Some(a) = m.amount {
                    out.insert(i64::from(a));
                }
            }
            // A value its table does not name is written as its bits.
            Operand::Symbol(s) if s.name.is_empty() => {
                out.insert(i64::from(s.bits));
            }
            _ => {}
        }
    }
    out
}

/// The value an immediate or integer token writes, where it is a number.
fn number(text: &str) -> Option<i64> {
    let text = text.strip_prefix('#').unwrap_or(text);
    let (negative, text) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    // A 64-bit bitmask can fill the top bit, and is compared as the same
    // bits in an i64.
    let value = match text.strip_prefix("0x") {
        Some(hex) => u64::from_str_radix(hex, 16).ok()? as i64,
        None => text.parse::<i64>().ok()?,
    };
    Some(if negative { -value } else { value })
}

#[test]
fn view_agrees_with_text() {
    let text = corpus::text();
    let mut checked = 0usize;
    let mut operands_seen = 0usize;
    let mut others: BTreeMap<String, usize> = BTreeMap::new();
    let mut failures: Vec<String> = Vec::new();
    for bits in corpus::encodings(&text) {
        let Ok(inst) = decode(bits) else { continue };
        let at = inst.at(corpus::INSTR_ADDRESS);
        let mut tokens: Vec<Token> = Vec::new();
        at.write_tokens(&mut tokens).unwrap();
        let view = inst.operands();
        checked += 1;
        operands_seen += view.len();
        let name: String = format!("{inst:?}")
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        if view.iter().any(|o| matches!(o, Operand::Other)) {
            *others.entry(name.clone()).or_default() += 1;
            continue;
        }
        let regs = registers(&view);
        let ints = integers(&view);
        let symbols = names(&view);
        for token in &tokens {
            match token.kind {
                TokenKind::Register if !regs.contains(&token.text) => {
                    failures.push(format!(
                        "{bits:08X} {at}: register {} not in view {view:?}",
                        token.text
                    ));
                }
                TokenKind::Symbol if !symbols.contains(&token.text) => {
                    failures.push(format!(
                        "{bits:08X} {at}: name {} not in view {view:?}",
                        token.text
                    ));
                }
                TokenKind::Immediate(value) => {
                    // The token's value must be both the text's and the view's.
                    if number(&token.text) != Some(value) || !ints.contains(&value) {
                        failures.push(format!(
                            "{bits:08X} {at}: immediate {} ({value}) not in view {view:?}",
                            token.text
                        ));
                    }
                }
                TokenKind::Integer(value) => {
                    let value = value as i64;
                    if number(&token.text) != Some(value) || !ints.contains(&value) {
                        failures.push(format!(
                            "{bits:08X} {at}: integer {} ({value}) not in view {view:?}",
                            token.text
                        ));
                    }
                }
                _ => {}
            }
        }
    }
    let undescribed: usize = others.values().sum();
    println!(
        "{checked} encodings, {operands_seen} operands; {undescribed} encodings across {} variants have an operand the view does not describe",
        others.len()
    );
    for (name, n) in others.iter().take(40) {
        println!("  {n:5}  {name}");
    }
    for failure in failures.iter().take(40) {
        println!("  {failure}");
    }
    assert!(
        failures.is_empty(),
        "{} disagreements between the view and the text",
        failures.len()
    );
}

/// A shift amount the architecture fixes is reported as that amount, not as
/// the bit that says whether it is written.
///
/// LDRB's `<amount>` is "the index shift amount, it must be #0, encoded in
/// \"S\" as 0 if omitted, or as 1 if present". The text writes `lsl #0`, and
/// a view reading the raw bit would answer `Some(1)`, a one-bit shift the
/// architecture does not have. `view_agrees_with_text` cannot see it, since
/// the formatter writes the amount as literal text, not as a number token.
#[test]
fn a_fixed_shift_amount_is_the_amount_and_not_the_bit() {
    // ldrb w4, [x24, lr, lsl #0x0] has S set, so the shift is written.
    let written = decode(0x387E_7B04).expect("LDRB (register), shifted");
    // ldrb w27, [x15, x16] has S clear, so it is not.
    let omitted = decode(0x386A_6A20).expect("LDRB (register), unshifted");

    for (bits, instruction, expected) in [
        (0x387E_7B04u32, written, Some(0u32)),
        (0x386A_6A20, omitted, None),
    ] {
        let operands = instruction.operands();
        let Some(Operand::Mem(mem)) = operands.iter().find(|o| matches!(o, Operand::Mem(_))) else {
            panic!("{bits:08X}: no memory operand in {operands:?}");
        };
        let Offset::Reg { modifier, .. } = mem.offset else {
            panic!("{bits:08X}: offset is not a register: {:?}", mem.offset);
        };
        assert_eq!(
            modifier.expect("the offset is scaled").amount,
            expected,
            "{bits:08X}: {}",
            instruction.at(0)
        );
    }
}

/// Every token says which operand it is part of, and says it consistently:
/// an index the instruction has, a run that does not come back once it has
/// been left, and nothing at all for the mnemonic, the gap after it and the
/// separators between operands. A separator is only ever between two
/// operands.
///
/// The brackets are the reason. A memory operand's are its own and a lane
/// index's belong to the register they are written after, and both are
/// written `[`, so a consumer reading only where they sit would have to
/// guess.
#[test]
fn every_token_says_which_operand_it_is_part_of() {
    let text = corpus::text();
    let mut failures: Vec<String> = Vec::new();
    let mut marked = 0usize;
    let mut unmarked: HashSet<String> = HashSet::new();
    for bits in corpus::encodings(&text) {
        let Ok(inst) = decode(bits) else { continue };
        let at = inst.at(corpus::INSTR_ADDRESS);
        let mut tokens: Vec<Token> = Vec::new();
        at.write_tokens(&mut tokens).unwrap();
        let view = inst.operands();
        let mut seen: Vec<u8> = Vec::new();
        for token in &tokens {
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
                    "{bits:08X} {at}: {:?} names operand {operand} of {}",
                    token.text,
                    view.len()
                ));
                continue;
            }
            match seen.last() {
                Some(last) if *last == operand => {}
                _ if seen.contains(&operand) => failures.push(format!(
                    "{bits:08X} {at}: operand {operand} is written in two places"
                )),
                _ => seen.push(operand),
            }
        }
        // A separator is between two operands and a `, ` within one is its
        // text, so a consumer counting separators counts operands.
        for (index, token) in tokens.iter().enumerate() {
            if token.kind != TokenKind::Separator {
                continue;
            }
            let before = tokens[..index].iter().rev().find_map(|t| t.operand);
            let after = tokens[index + 1..].iter().find_map(|t| t.operand);
            if token.operand.is_some() || before.is_none() || after.is_none() || before == after {
                failures.push(format!(
                    "{bits:08X} {at}: the separator at token {index} is not between two operands"
                ));
            }
        }
        // The assembly writes the operands in the order the view holds
        // them, so the marks run up from the first.
        let in_order: Vec<u8> = (0..seen.len() as u8).collect();
        if seen != in_order {
            failures.push(format!("{bits:08X} {at}: operands written as {seen:?}"));
        }
    }
    assert!(marked > 0, "the corpus writes operands");
    // Only the mnemonic, the gap after it and the `, ` between operands are
    // part of none.
    let mut unmarked: Vec<String> = unmarked.into_iter().collect();
    unmarked.sort();
    assert_eq!(
        unmarked,
        vec![
            "Mnemonic".to_string(),
            "Separator".to_string(),
            "Text \"\\t\"".to_string(),
        ]
    );
    assert!(
        failures.is_empty(),
        "{} of them:\n{}",
        failures.len(),
        failures
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// A memory operand's brackets are its own, and the brackets of a lane
/// index or an SME slice belong to the operand they are written on.
#[test]
fn a_memory_operands_brackets_are_its_own() {
    // ldr x0, [x1, #8], whose brackets are the memory operand's.
    let ldr = decode(0xF9400420).unwrap();
    let mut tokens: Vec<Token> = Vec::new();
    ldr.at(0).write_tokens(&mut tokens).unwrap();
    let brackets: Vec<Option<u8>> = tokens
        .iter()
        .filter(|t| t.kind == TokenKind::Bracket)
        .map(|t| t.operand)
        .collect();
    assert_eq!(brackets, vec![Some(1), Some(1)]);
    assert!(matches!(ldr.operands()[1], Operand::Mem(_)));

    // mov v0.s[1], w0, whose brackets are the register's lane index. It has
    // no memory operand at all.
    let mov = decode(0x4E0C1C00).unwrap();
    let mut tokens: Vec<Token> = Vec::new();
    mov.at(0).write_tokens(&mut tokens).unwrap();
    let brackets: Vec<Option<u8>> = tokens
        .iter()
        .filter(|t| t.kind == TokenKind::Bracket)
        .map(|t| t.operand)
        .collect();
    assert_eq!(brackets, vec![Some(0), Some(0)]);
    assert!(matches!(mov.operands()[0], Operand::Reg(_)));

    // st1b {za0v.b[w12, 11]}, p0, [x3, x4]. An SME slice written in braces
    // is one operand, its own inner brackets included, and the memory
    // operand after it is another.
    let st1b = decode(0xE024806B).unwrap();
    let mut tokens: Vec<Token> = Vec::new();
    st1b.at(0).write_tokens(&mut tokens).unwrap();
    let brackets: Vec<Option<u8>> = tokens
        .iter()
        .filter(|t| t.kind == TokenKind::Bracket)
        .map(|t| t.operand)
        .collect();
    assert_eq!(
        brackets,
        vec![Some(0), Some(0), Some(0), Some(0), Some(2), Some(2)]
    );
    assert!(matches!(st1b.operands()[0], Operand::ZaSlice(_)));
    assert!(matches!(st1b.operands()[2], Operand::Mem(_)));
}

/// A list written in braces is one operand, however many registers it
/// holds, so the braces and every member of it name the same one.
#[test]
fn a_register_list_is_one_operand() {
    // ext z24.b, {z30.b, z31.b}, #0xb5
    let ext = decode(0x057617D8).unwrap();
    let mut tokens: Vec<Token> = Vec::new();
    ext.at(0).write_tokens(&mut tokens).unwrap();
    let written: Vec<(Option<u8>, &str)> = tokens
        .iter()
        .map(|t| (t.operand, t.text.as_str()))
        .collect();
    assert_eq!(
        written,
        vec![
            (None, "ext"),
            (None, "\t"),
            (Some(0), "z24"),
            (Some(0), ".b"),
            (None, ", "),
            (Some(1), "{"),
            (Some(1), "z30"),
            (Some(1), ".b"),
            (Some(1), ", "),
            (Some(1), "z31"),
            (Some(1), ".b"),
            (Some(1), "}"),
            (None, ", "),
            (Some(2), "#"),
            (Some(2), "0xb5"),
        ]
    );
    assert!(matches!(ext.operands()[1], Operand::List(_)));
}

/// Which kinds of token an operand of each kind may be written with, and
/// the kinds of which it must be written with one: its own value, which a
/// consumer reads off the token rather than the text. `#`, brackets and the
/// punctuation within an operand are text anyone may write, but a label is
/// written as its address and nothing else.
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
            &["Text", "Immediate", "Integer", "Symbol"],
            &["Immediate", "Integer"],
        ),
        Operand::FpImm { .. } => (&["Text", "Float"], &["Float"]),
        Operand::Label { .. } => (&["Address"], &["Address"]),
        Operand::Mem(_) => (
            &["Bracket", "Register", "Text", "Immediate", "Symbol"],
            &["Register"],
        ),
        Operand::List(_) => (
            &["Bracket", "Register", "Text", "Symbol", "Integer"],
            &["Register"],
        ),
        Operand::SysReg(_) => (&["Register"], &["Register"]),
        Operand::SysOp(_) => (&["Symbol", "Text"], &["Symbol"]),
        Operand::Cond(_) => (&["Symbol", "Mnemonic"], &["Symbol", "Mnemonic"]),
        // A value its table does not name is written as a number
        Operand::Symbol(_) => (
            &["Symbol", "Mnemonic", "Immediate", "Text"],
            &["Symbol", "Mnemonic", "Immediate"],
        ),
        Operand::ZaSlice(_) => (
            &["Bracket", "Register", "Text", "Symbol", "Integer"],
            &["Register", "Symbol", "Text"],
        ),
        // ZERO with no tile named writes only its braces
        Operand::TileMask(mask) if mask.0 == 0 => (&["Bracket"], &["Bracket"]),
        Operand::TileMask(_) => (&["Bracket", "Register"], &["Register"]),
        Operand::Modifier(_) => (&["Symbol", "Text", "Immediate"], &["Symbol"]),
        Operand::Other => return None,
    };
    Some(kinds)
}

/// Over the whole corpus, every token is of a kind its operand holds, and
/// every operand written is written with a token carrying its value. An
/// optional operand at its default is in the view and not in the text, so
/// an operand no token names is not a failure.
#[test]
fn every_token_is_of_a_kind_its_operand_holds() {
    let mut failures = Vec::new();
    let mut checked = 0;
    for (case, inst) in corpus::instructions() {
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
                    corpus::named(&case, &inst)
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
