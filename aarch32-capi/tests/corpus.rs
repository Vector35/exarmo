//! The C entry points against the Rust library they wrap, over every case of
//! both AArch32 corpora: what a C caller reads of an instruction is what a
//! Rust one does, field by field, and reading it allocates nothing.
//!
//! A32 and T32 are two decode entry points, and a T32 decode is told the IT
//! state, so every T32 case with no tag is read both outside a block and
//! inside one, which is where the state a caller passes changes what comes
//! back, and a tagged case in the state its tag names. What
//! is compared is read back from the C layout independently of the
//! conversions that wrote it: a register by the name its class and number
//! spell, a token's kind by the header's numbering, its text by its span.

use std::collections::HashSet;
use std::mem::MaybeUninit;

use exarmo_aarch32::{Cond, Instruction, ItState, Lane, ListFile, Operand};
use exarmo_aarch32_capi::model::{
    CLane, CListFile, COperand, CReg, Modifier, OffsetKind, OperandKind, RegClass, SysReg,
    Writeback,
};
use exarmo_aarch32_capi::*;

#[global_allocator]
static ALLOCATOR: exarmo_testing::capi::Counting = exarmo_testing::capi::Counting;

/// The address the corpus is written at, so a label names one target.
const ADDRESS: u64 = 0x10000;

/// The block a T32 case is read inside as well as outside: the last of one
/// under NE, where a branch is allowed.
const INSIDE: ItState = ItState::Inside {
    cond: Cond::Ne,
    mask: 0b1000,
};

/// Which entry point a word is decoded through, and for T32 the state it is
/// told.
#[derive(Clone, Copy, Debug)]
enum Entry {
    A32,
    T32(ItState),
}

impl Entry {
    fn rust(self, word: u32) -> Result<Instruction, exarmo_aarch32::DecodeError> {
        match self {
            Entry::A32 => exarmo_aarch32::a32::decode_word(word),
            Entry::T32(state) => exarmo_aarch32::t32::decode_word(word, state),
        }
    }

    /// The state as C holds it, written out field by field as the header
    /// lays it out rather than through the conversion.
    fn c_state(state: ItState) -> CItState {
        match state {
            ItState::Unknown => CItState {
                kind: 0,
                cond: 0,
                mask: 0,
            },
            ItState::Outside => CItState {
                kind: 1,
                cond: 0,
                mask: 0,
            },
            ItState::Inside { cond, mask } => CItState {
                kind: 2,
                cond: cond.bits() as u8,
                mask,
            },
        }
    }
}

/// A token before anything is written into it.
const BLANK_TOKEN: Token = Token {
    kind: 0,
    operand: Token::NO_OPERAND,
    offset: 0,
    length: 0,
    value: 0,
};

/// Everything a C caller reads of one word, into buffers of the sizes the
/// header states, with nothing on the heap.
struct Read {
    status: Status,
    encoding: u32,
    mnemonic: u32,
    length: u8,
    unpredictable: bool,
    t32_length: u8,
    flags: FlagEffect,
    branch: Branch,
    sysreg: Option<SysReg>,
    after: CItState,
    operands: [COperand; Instruction::MAX_OPERANDS],
    operand_count: usize,
    text: [u8; Instruction::MAX_TEXT + 1],
    text_length: usize,
    size: TextSize,
    tokens: [Token; Instruction::MAX_TOKENS],
}

/// Read a word through every entry point, as a caller holding the header's
/// buffers would.
fn read(entry: Entry, word: u32) -> Read {
    let mut read = Read {
        status: Status::Failed,
        encoding: 0,
        mnemonic: 0,
        length: 0,
        unpredictable: false,
        t32_length: 0,
        flags: FlagEffect {
            writes: 0,
            reads: 0,
            float_compare: false,
        },
        branch: Branch::NONE,
        sysreg: None,
        after: CItState {
            kind: 0xff,
            cond: 0,
            mask: 0,
        },
        operands: [COperand::from(Operand::Other); Instruction::MAX_OPERANDS],
        operand_count: 0,
        text: [0; Instruction::MAX_TEXT + 1],
        text_length: 0,
        size: TextSize::NONE,
        tokens: [BLANK_TOKEN; Instruction::MAX_TOKENS],
    };
    let mut storage = MaybeUninit::<CInstruction>::uninit();
    let state = match entry {
        Entry::A32 => {
            read.status = unsafe { exarmo_aarch32_decode_a32_word(word, storage.as_mut_ptr()) };
            Entry::c_state(ItState::Outside)
        }
        Entry::T32(state) => {
            let state = Entry::c_state(state);
            read.status =
                unsafe { exarmo_aarch32_decode_t32_word(word, state, storage.as_mut_ptr()) };
            read.t32_length = exarmo_aarch32_t32_length((word >> 16) as u16);
            state
        }
    };
    if read.status != Status::Ok {
        return read;
    }
    let inst = storage.as_ptr();
    unsafe {
        read.encoding = exarmo_aarch32_instruction_encoding(inst);
        read.mnemonic = exarmo_aarch32_instruction_mnemonic(inst);
        read.length = exarmo_aarch32_instruction_length(inst);
        read.unpredictable = exarmo_aarch32_instruction_unpredictable(inst);
        read.flags = exarmo_aarch32_instruction_flags(inst);
        read.branch = exarmo_aarch32_instruction_branch(inst, ADDRESS);
        let mut sysreg = MaybeUninit::<SysReg>::uninit();
        if exarmo_aarch32_instruction_sysreg(inst, sysreg.as_mut_ptr()) {
            read.sysreg = Some(sysreg.assume_init());
        }
        read.after = exarmo_aarch32_it_state_after(state, inst);
        read.operand_count = exarmo_aarch32_instruction_operands(
            inst,
            read.operands.as_mut_ptr(),
            read.operands.len(),
        );
        read.text_length = exarmo_aarch32_instruction_text(inst, ADDRESS, std::ptr::null_mut(), 0);
        read.size = exarmo_aarch32_instruction_tokens(
            inst,
            ADDRESS,
            read.text.as_mut_ptr(),
            read.text.len(),
            read.tokens.as_mut_ptr(),
            read.tokens.len(),
        );
    }
    read
}

/// The condition a case's tag names, as `Cond` spells it.
fn cond(name: &str) -> Option<Cond> {
    (0..16)
        .filter_map(Cond::from_bits)
        .find(|cond| cond.name() == name)
}

/// The words of both corpora, each once for its set and tag, with the first
/// line naming it and the entry points it is read through.
fn cases() -> Vec<(&'static str, Vec<Entry>, exarmo_testing::Case)> {
    #[derive(Clone, Copy)]
    struct Corpus(&'static str, bool);
    impl exarmo_testing::Corpus for Corpus {
        fn path(self) -> &'static str {
            self.0
        }
        fn word_of(self, hex: &str) -> Option<u32> {
            exarmo_testing::t32_word_of(hex)
        }
        fn render(self, _word: u32) -> exarmo_testing::Outcome {
            unreachable!("the cases are read, not judged")
        }
        fn condition(self, name: &str) -> Option<u8> {
            self.1.then(|| cond(name).map(|c| c.bits() as u8)).flatten()
        }
    }
    let mut out = Vec::new();
    for (set, corpus) in [
        ("a32", Corpus("../aarch32/tests/cases/a32", false)),
        ("t32", Corpus("../aarch32/tests/cases/t32", true)),
    ] {
        let mut seen = HashSet::new();
        for case in exarmo_testing::Corpus::cases(corpus, env!("CARGO_MANIFEST_DIR")) {
            if !seen.insert((case.encoding, case.tag.clone())) {
                continue;
            }
            let entries = match (set, &case.tag) {
                ("a32", _) => vec![Entry::A32],
                (_, None) => vec![Entry::T32(ItState::Outside), Entry::T32(INSIDE)],
                (_, Some(tag)) => vec![Entry::T32(ItState::Inside {
                    cond: cond(&tag.cond).expect("the reader took the tag"),
                    mask: tag.mask(),
                })],
            };
            out.push((set, entries, case));
        }
    }
    out
}

/// The name a C register spells, from its class and number alone, as the
/// header describes each class.
fn name(reg: CReg) -> String {
    let n = reg.num;
    let classes = [RegClass::Core, RegClass::S, RegClass::D, RegClass::Q];
    let Some(class) = classes.into_iter().find(|c| *c as u8 == reg.class) else {
        return format!("<class {}>", reg.class);
    };
    match class {
        RegClass::Core => match n {
            13 => "sp".to_string(),
            14 => "lr".to_string(),
            15 => "pc".to_string(),
            n => format!("r{n}"),
        },
        RegClass::S => format!("s{n}"),
        RegClass::D => format!("d{n}"),
        RegClass::Q => format!("q{n}"),
    }
}

/// A C modifier as the kind, by number, amount and register it says.
fn modifier(m: Modifier) -> Option<(u8, Option<u32>, Option<String>)> {
    m.present.then(|| {
        (
            m.kind,
            u32::try_from(m.amount).ok(),
            m.has_by.then(|| name(m.by)),
        )
    })
}

fn rust_modifier(m: Option<exarmo_aarch32::Modifier>) -> Option<(u8, Option<u32>, Option<String>)> {
    m.map(|m| (m.kind as u8, m.amount, m.by.map(|r| r.to_string())))
}

/// A number the C face writes as -1 for none.
fn optional(n: i32) -> Option<u32> {
    u32::try_from(n).ok()
}

/// The text a C string spans.
fn text_of(s: exarmo_core::capi::Str) -> String {
    let bytes = unsafe { std::slice::from_raw_parts(s.data, s.length) };
    String::from_utf8_lossy(bytes).into_owned()
}

/// Where a C operand says something other than the Rust one.
fn differences(c: &COperand, r: &Operand) -> Vec<String> {
    let mut out = Vec::new();
    let mut same = |what: &str, c: String, r: String| {
        if c != r {
            out.push(format!("{what} is {c} in C and {r} in Rust"));
        }
    };
    let kind = |k: OperandKind| format!("{k:?}");
    macro_rules! of {
        ($member:ident) => {
            unsafe { c.value.$member }
        };
    }
    let expected = match r {
        Operand::Reg(_) => OperandKind::Reg,
        Operand::Imm { .. } => OperandKind::Imm,
        Operand::FpImm { .. } => OperandKind::FpImm,
        Operand::Label { .. } => OperandKind::Label,
        Operand::Mem(_) => OperandKind::Mem,
        Operand::List(_) => OperandKind::List,
        Operand::Cond(_) => OperandKind::Cond,
        Operand::Symbol(_) => OperandKind::Symbol,
        Operand::Modifier(_) => OperandKind::Modifier,
        Operand::Other => OperandKind::Other,
    };
    same("the kind", kind(c.kind), kind(expected));
    if c.kind != expected {
        return out;
    }
    match r {
        Operand::Reg(r) => {
            let c = of!(reg);
            same("the register", name(c.reg), r.reg.to_string());
            same(
                "the index",
                format!("{:?}", optional(c.index)),
                format!("{:?}", r.index),
            );
            same(
                "the modifier",
                format!("{:?}", modifier(c.modifier)),
                format!("{:?}", rust_modifier(r.modifier)),
            );
            same(
                "the writeback",
                c.writeback.to_string(),
                r.writeback.to_string(),
            );
        }
        Operand::Imm { value, modifier: m } => {
            let c = of!(imm);
            same("the value", c.value.to_string(), value.to_string());
            same(
                "the modifier",
                format!("{:?}", modifier(c.modifier)),
                format!("{:?}", rust_modifier(*m)),
            );
        }
        Operand::FpImm { value, width, bits } => {
            let c = of!(fp_imm);
            same(
                "the value",
                format!("{:#x}", c.value.to_bits()),
                format!("{:#x}", value.to_bits()),
            );
            same("the width", c.width.to_string(), width.to_string());
            same(
                "the pattern",
                format!("{:#x}", c.bits),
                format!("{bits:#x}"),
            );
        }
        Operand::Label { offset, pc } => {
            let c = of!(label);
            same(
                "the label",
                format!("{} {} {}", c.offset, c.pc_ahead, c.pc_align),
                format!("{offset} {} {}", pc.ahead, pc.align),
            );
        }
        Operand::Mem(m) => {
            let c = of!(mem);
            same("the base", name(c.base), m.base.to_string());
            let o = c.offset;
            let offset = match o.kind {
                OffsetKind::None => "none".to_string(),
                OffsetKind::Imm => format!("{} subtract {}", o.imm, o.subtract),
                OffsetKind::Reg => format!(
                    "{} subtract {} {:?}",
                    name(o.reg),
                    o.subtract,
                    modifier(o.modifier)
                ),
            };
            let rust = match m.offset {
                exarmo_aarch32::Offset::None => "none".to_string(),
                exarmo_aarch32::Offset::Imm { value, subtract } => {
                    format!("{value} subtract {subtract}")
                }
                exarmo_aarch32::Offset::Reg {
                    reg,
                    subtract,
                    modifier,
                } => format!("{reg} subtract {subtract} {:?}", rust_modifier(modifier)),
            };
            same("the offset", offset, rust);
            let writeback = match c.writeback {
                Writeback::None => "None",
                Writeback::Pre => "Pre",
                Writeback::Post => "Post",
            };
            same(
                "the writeback",
                writeback.to_string(),
                format!("{:?}", m.writeback),
            );
            same(
                "the alignment",
                format!("{:?}", (c.align != 0).then_some(c.align)),
                format!("{:?}", m.align),
            );
        }
        Operand::List(l) => {
            let c = of!(list);
            let file = [CListFile::Core, CListFile::S, CListFile::D]
                .into_iter()
                .find(|f| *f as u8 == c.file);
            let lane = [CLane::Whole, CLane::All, CLane::Index]
                .into_iter()
                .find(|f| *f as u8 == c.lane);
            same(
                "the list",
                format!(
                    "{:#x} {file:?} {:?}",
                    c.mask,
                    match lane {
                        Some(CLane::Index) => format!("Index({})", c.index),
                        other => format!("{other:?}"),
                    }
                ),
                format!(
                    "{:#x} {:?} {:?}",
                    l.mask,
                    Some(match l.file {
                        ListFile::Core => CListFile::Core,
                        ListFile::Single => CListFile::S,
                        ListFile::Double => CListFile::D,
                    }),
                    match l.lane {
                        Lane::Whole => "Some(Whole)".to_string(),
                        Lane::All => "Some(All)".to_string(),
                        Lane::Index(index) => format!("Index({index})"),
                    }
                ),
            );
        }
        Operand::Cond(cond) => {
            same(
                "the condition",
                of!(cond).to_string(),
                cond.bits().to_string(),
            );
        }
        Operand::Symbol(s) => {
            let c = of!(symbol);
            same(
                "the symbol",
                format!("{} {}", c.bits, text_of(c.name)),
                format!("{} {}", s.bits, s.name),
            );
        }
        Operand::Modifier(m) => {
            same(
                "the modifier",
                format!("{:?}", modifier(of!(modifier))),
                format!("{:?}", rust_modifier(Some(*m))),
            );
        }
        Operand::Other => {}
    }
    out
}

/// A token kind's number, by the name the hand-written header gives it.
fn token_kinds() -> Vec<(String, u32)> {
    let header = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/include/exarmo/aarch32.h"
    ))
    .expect("include/exarmo/aarch32.h");
    exarmo_testing::capi::token_kinds(&header, "EXARMO_AARCH32_TOKEN_")
}

/// Every case of both corpora reads the same through C as through Rust,
/// through either decode entry point and under either IT state: whether it
/// decodes and why not, what it is and how long, what it does with the
/// flags and to the flow of control, the system register it reaches, the
/// IT state after it, its operands field by field, and its text and every
/// token of it.
#[test]
fn the_c_api_agrees_with_rust_over_the_corpus() {
    let kinds = token_kinds();
    assert_eq!(kinds.len(), 10, "the header numbers every kind of token");
    let mut failures = Vec::new();
    let mut checked = 0;
    for (set, entries, case) in cases() {
        for entry in entries {
            checked += 1;
            let c = read(entry, case.encoding);
            let place = format!("{set}/{}  {}  {entry:?}", case.place, case.hex);
            let inst = match entry.rust(case.encoding) {
                Ok(inst) => inst,
                Err(error) => {
                    if c.status != Status::from(error) {
                        failures.push(format!("{place}: {:?} in C and {error} in Rust", c.status));
                    }
                    continue;
                }
            };
            let at = inst.at(ADDRESS);
            let written = at.to_string();
            let mut fail = |problem: String| {
                failures.push(format!(
                    "{place}  {}: {problem}",
                    written.replace('\t', " ")
                ))
            };
            if c.status != Status::Ok {
                fail(format!("{:?} in C", c.status));
                continue;
            }
            let length = inst.encoding().length();
            if (c.encoding, c.mnemonic, c.length)
                != (inst.encoding() as u32, inst.mnemonic() as u32, length)
            {
                fail(format!(
                    "encoding {} mnemonic {} length {} in C",
                    c.encoding, c.mnemonic, c.length
                ));
            }
            if c.unpredictable != inst.unpredictable() {
                fail(format!(
                    "unpredictable {} in C and {} in Rust",
                    c.unpredictable,
                    inst.unpredictable()
                ));
            }
            if let Entry::T32(state) = entry {
                if c.t32_length != length {
                    fail(format!(
                        "the first halfword says {} bytes in C",
                        c.t32_length
                    ));
                }
                let after = Entry::c_state(state.after(&inst));
                if c.after != after {
                    fail(format!(
                        "the state after is {:?} in C and {after:?} in Rust",
                        c.after
                    ));
                }
            }
            for difference in exarmo_testing::capi::effect_differences(
                c.flags,
                inst.flags(),
                c.branch,
                at.branch(),
            ) {
                fail(difference);
            }
            let sysreg = inst
                .system_register()
                .map(|r| (r.space() as u8, r.is_write(), r.encoding()));
            if c.sysreg.map(|r| (r.space, r.write, r.encoding)) != sysreg {
                fail(format!(
                    "the system register is {:?} in C and {sysreg:?} in Rust",
                    c.sysreg
                ));
            }

            let operands = inst.operands();
            if c.operand_count != operands.len() {
                fail(format!(
                    "{} operands in C and {} in Rust",
                    c.operand_count,
                    operands.len()
                ));
            }
            for (index, (c, r)) in c.operands.iter().zip(operands.iter()).enumerate() {
                for difference in differences(c, r) {
                    fail(format!("operand {index}: {difference}"));
                }
            }

            let mut tokens: Vec<exarmo_aarch32::Token> = Vec::new();
            at.write_tokens(&mut tokens).unwrap();
            for difference in exarmo_testing::capi::text_differences(
                &kinds,
                &written,
                &tokens,
                &c.text,
                c.size,
                c.text_length,
                &c.tokens,
            ) {
                fail(difference);
            }
        }
    }
    exarmo_testing::report("the C API agrees with Rust", checked, &failures);
}

/// Reading every case of both corpora through C, with the buffers the
/// header sizes, asks nothing of the allocator, and so raises no panic
/// either. It is counted from the very first call, since a decode keeps no
/// state that a first call would make and a later one be spared.
#[test]
fn the_c_api_allocates_nothing_over_the_corpus() {
    let cases = cases();
    let mut failures = Vec::new();
    let mut checked = 0;
    for (set, entries, case) in &cases {
        for entry in entries {
            checked += 1;
            let (allocations, read) =
                exarmo_testing::capi::allocations(|| read(*entry, case.encoding));
            if allocations > 0 {
                failures.push(format!(
                    "{set}/{}  {}  {entry:?}: {allocations} allocations reading it, {:?}",
                    case.place, case.hex, read.status
                ));
            }
        }
    }
    exarmo_testing::report("the C API allocates nothing", checked, &failures);
}
