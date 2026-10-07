//! The C entry points against the Rust library they wrap, over every case of
//! the AArch64 corpus: what a C caller reads of an instruction is what a
//! Rust one does, field by field, and reading it allocates nothing.
//!
//! Binary Ninja reads the library through these entry points alone, so a
//! value lost or bent on the way through C is one it never sees. What is
//! compared here is read back from the C layout independently of the
//! conversions that wrote it: a register by the name its class and number
//! spell, a token's kind by the header's numbering, its text by its span.

use std::collections::HashSet;
use std::mem::MaybeUninit;

use exarmo_aarch64::{Instruction, Operand};
use exarmo_aarch64_capi::model::{
    Arrangement, CIntrinsic, COperand, CReg, Modifier, OffsetKind, OperandKind, RegClass, ZaArray,
};
use exarmo_aarch64_capi::*;

/// The address the corpus is written at, so a label names one target.
const ADDRESS: u64 = 0x8000000000000004;

/// Room for more intrinsics than any instruction realises, so a count
/// beyond it is a failure rather than a cut.
const INTRINSICS: usize = 32;

#[global_allocator]
static ALLOCATOR: exarmo_testing::capi::Counting = exarmo_testing::capi::Counting;

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
    flags: FlagEffect,
    branch: Branch,
    operands: [COperand; Instruction::MAX_OPERANDS],
    operand_count: usize,
    text: [u8; Instruction::MAX_TEXT + 1],
    text_length: usize,
    size: TextSize,
    tokens: [Token; Instruction::MAX_TOKENS],
    intrinsics: [CIntrinsic; INTRINSICS],
    intrinsic_count: usize,
}

/// Read a word through every entry point, as a caller holding the header's
/// buffers would.
fn read(word: u32) -> Read {
    let mut read = Read {
        status: Status::Failed,
        encoding: 0,
        mnemonic: 0,
        length: 0,
        unpredictable: false,
        flags: FlagEffect {
            writes: 0,
            reads: 0,
            float_compare: false,
        },
        branch: Branch::NONE,
        operands: [COperand::from(Operand::Other); Instruction::MAX_OPERANDS],
        operand_count: 0,
        text: [0; Instruction::MAX_TEXT + 1],
        text_length: 0,
        size: TextSize::NONE,
        tokens: [BLANK_TOKEN; Instruction::MAX_TOKENS],
        intrinsics: [CIntrinsic::default(); INTRINSICS],
        intrinsic_count: 0,
    };
    let mut storage = MaybeUninit::<CInstruction>::uninit();
    read.status = unsafe { exarmo_aarch64_decode_word(word, storage.as_mut_ptr()) };
    if read.status != Status::Ok {
        return read;
    }
    let inst = storage.as_ptr();
    unsafe {
        read.encoding = exarmo_aarch64_instruction_encoding(inst);
        read.mnemonic = exarmo_aarch64_instruction_mnemonic(inst);
        read.length = exarmo_aarch64_instruction_length(inst);
        read.unpredictable = exarmo_aarch64_instruction_unpredictable(inst);
        read.flags = exarmo_aarch64_instruction_flags(inst);
        read.branch = exarmo_aarch64_instruction_branch(inst, ADDRESS);
        read.operand_count = exarmo_aarch64_instruction_operands(
            inst,
            read.operands.as_mut_ptr(),
            read.operands.len(),
        );
        read.text_length = exarmo_aarch64_instruction_text(inst, ADDRESS, std::ptr::null_mut(), 0);
        read.size = exarmo_aarch64_instruction_tokens(
            inst,
            ADDRESS,
            read.text.as_mut_ptr(),
            read.text.len(),
            read.tokens.as_mut_ptr(),
            read.tokens.len(),
        );
        read.intrinsic_count = exarmo_aarch64_instruction_intrinsics(
            inst,
            read.intrinsics.as_mut_ptr(),
            read.intrinsics.len(),
        );
    }
    read
}

/// The corpus's words, each once, with the first line naming it.
fn cases() -> Vec<exarmo_testing::Case> {
    #[derive(Clone, Copy)]
    struct Corpus;
    impl exarmo_testing::Corpus for Corpus {
        fn path(self) -> &'static str {
            "../aarch64/tests/cases"
        }
        fn render(self, word: u32) -> exarmo_testing::Outcome {
            exarmo_testing::Outcome::of(exarmo_aarch64::decode_word(word), ADDRESS)
        }
    }
    let mut seen = HashSet::new();
    exarmo_testing::Corpus::cases(Corpus, env!("CARGO_MANIFEST_DIR"))
        .into_iter()
        .filter(|case| seen.insert(case.encoding))
        .collect()
}

/// The name a C register spells, from its class and number alone, as the
/// header describes each class.
fn name(reg: CReg) -> String {
    let n = reg.num;
    let classes = [
        RegClass::W,
        RegClass::WSp,
        RegClass::X,
        RegClass::XSp,
        RegClass::Gp,
        RegClass::B,
        RegClass::H,
        RegClass::S,
        RegClass::D,
        RegClass::Q,
        RegClass::V,
        RegClass::Z,
        RegClass::P,
        RegClass::Pn,
        RegClass::ZaTile,
        RegClass::Zt0,
    ];
    let Some(class) = classes.into_iter().find(|c| *c as u8 == reg.class) else {
        return format!("<class {}>", reg.class);
    };
    let x = |sp: &str| match n {
        29 => "fp".to_string(),
        30 => "lr".to_string(),
        31 => sp.to_string(),
        n => format!("x{n}"),
    };
    match class {
        RegClass::W if n == 31 => "wzr".to_string(),
        RegClass::WSp if n == 31 => "wsp".to_string(),
        RegClass::W | RegClass::WSp => format!("w{n}"),
        RegClass::X => x("xzr"),
        RegClass::XSp => x("sp"),
        RegClass::Gp => format!("r{n}"),
        RegClass::B => format!("b{n}"),
        RegClass::H => format!("h{n}"),
        RegClass::S => format!("s{n}"),
        RegClass::D => format!("d{n}"),
        RegClass::Q => format!("q{n}"),
        RegClass::V => format!("v{n}"),
        RegClass::Z => format!("z{n}"),
        RegClass::P => format!("p{n}"),
        RegClass::Pn => format!("pn{n}"),
        RegClass::ZaTile => format!("za{n}"),
        RegClass::Zt0 => "zt0".to_string(),
    }
}

/// An arrangement as the assembly writes it, from the C one's width and
/// lanes, and empty for none.
fn arrangement(a: Arrangement) -> String {
    let element = match a.element {
        0 => return String::new(),
        8 => "b",
        16 => "h",
        32 => "s",
        64 => "d",
        128 => "q",
        bits => return format!("<{bits} bits>"),
    };
    match a.lanes {
        0 => element.to_string(),
        lanes => format!("{lanes}{element}"),
    }
}

fn rust_arrangement(a: Option<exarmo_aarch64::Arrangement>) -> String {
    a.map_or_else(String::new, |a| a.to_string())
}

/// A C modifier as the kind, by number, and amount it says.
fn modifier(m: Modifier) -> Option<(u8, Option<u32>)> {
    m.present.then(|| (m.kind, u32::try_from(m.amount).ok()))
}

fn rust_modifier(m: Option<exarmo_aarch64::Modifier>) -> Option<(u8, Option<u32>)> {
    m.map(|m| (m.kind as u8, m.amount))
}

/// A number the C face writes as -1 for none.
fn optional(n: i32) -> Option<u32> {
    u32::try_from(n).ok()
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
    match r {
        Operand::Reg(r) => {
            same("the kind", kind(c.kind), kind(OperandKind::Reg));
            if c.kind != OperandKind::Reg {
                return out;
            }
            let c = of!(reg);
            same("the register", name(c.reg), r.reg.to_string());
            same(
                "the arrangement",
                arrangement(c.arrangement),
                rust_arrangement(r.arrangement),
            );
            same(
                "the index",
                format!("{:?}", optional(c.index)),
                format!("{:?}", r.index),
            );
            same(
                "the index register",
                format!("{:?}", c.has_index_reg.then(|| name(c.index_reg))),
                format!("{:?}", r.index_reg.map(|r| r.to_string())),
            );
            same(
                "the predication",
                format!(
                    "{:?}",
                    (c.predication != 0).then_some(c.predication as char)
                ),
                format!("{:?}", r.predication),
            );
            same(
                "the modifier",
                format!("{:?}", modifier(c.modifier)),
                format!("{:?}", rust_modifier(r.modifier)),
            );
        }
        Operand::Imm { value, modifier: m } => {
            same("the kind", kind(c.kind), kind(OperandKind::Imm));
            if c.kind != OperandKind::Imm {
                return out;
            }
            let c = of!(imm);
            same("the value", c.value.to_string(), value.to_string());
            same(
                "the modifier",
                format!("{:?}", modifier(c.modifier)),
                format!("{:?}", rust_modifier(*m)),
            );
        }
        Operand::FpImm { value, width, bits } => {
            same("the kind", kind(c.kind), kind(OperandKind::FpImm));
            if c.kind != OperandKind::FpImm {
                return out;
            }
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
            same("the kind", kind(c.kind), kind(OperandKind::Label));
            if c.kind != OperandKind::Label {
                return out;
            }
            let c = of!(label);
            same(
                "the label",
                format!("{} {} {}", c.offset, c.pc_ahead, c.pc_align),
                format!("{offset} {} {}", pc.ahead, pc.align),
            );
        }
        Operand::Mem(m) => {
            same("the kind", kind(c.kind), kind(OperandKind::Mem));
            if c.kind != OperandKind::Mem {
                return out;
            }
            let c = of!(mem);
            same("the base", name(c.base), m.base.to_string());
            same(
                "the base's arrangement",
                arrangement(c.base_arrangement),
                rust_arrangement(m.base_arrangement),
            );
            let o = c.offset;
            let offset = match o.kind {
                OffsetKind::None => "none".to_string(),
                OffsetKind::Imm => format!("{} mul_vl {}", o.imm, o.mul_vl),
                OffsetKind::Reg => format!("{} {:?}", name(o.reg), modifier(o.modifier)),
                OffsetKind::Vector => format!(
                    "{}.{} {:?}",
                    name(o.reg),
                    arrangement(o.arrangement),
                    modifier(o.modifier)
                ),
            };
            let rust = match m.offset {
                exarmo_aarch64::Offset::None => "none".to_string(),
                exarmo_aarch64::Offset::Imm { value, mul_vl } => {
                    format!("{value} mul_vl {mul_vl}")
                }
                exarmo_aarch64::Offset::Reg { reg, modifier } => {
                    format!("{reg} {:?}", rust_modifier(modifier))
                }
                exarmo_aarch64::Offset::Vector {
                    reg,
                    arrangement,
                    modifier,
                } => format!(
                    "{reg}.{} {:?}",
                    rust_arrangement(arrangement),
                    rust_modifier(modifier)
                ),
            };
            same("the offset", offset, rust);
            same(
                "the writeback",
                format!("{:?}", c.writeback),
                format!("{:?}", m.writeback),
            );
        }
        Operand::List(l) => {
            same("the kind", kind(c.kind), kind(OperandKind::List));
            if c.kind != OperandKind::List {
                return out;
            }
            let c = of!(list);
            let regs: Vec<String> = c.regs[..usize::from(c.len).min(4)]
                .iter()
                .map(|r| name(*r))
                .collect();
            same(
                "the registers",
                format!("{regs:?}"),
                format!(
                    "{:?}",
                    l.regs().iter().map(|r| r.to_string()).collect::<Vec<_>>()
                ),
            );
            same(
                "the arrangement",
                arrangement(c.arrangement),
                rust_arrangement(l.arrangement),
            );
            same(
                "the index",
                format!("{:?}", optional(c.index)),
                format!("{:?}", l.index),
            );
        }
        Operand::SysReg(s) => {
            same("the kind", kind(c.kind), kind(OperandKind::SysReg));
            if c.kind != OperandKind::SysReg {
                return out;
            }
            let c = of!(sysreg);
            same(
                "the system register",
                format!(
                    "{:#x} {} {}:{}:{}:{}:{}",
                    c.encoding, c.write, c.op0, c.op1, c.crn, c.crm, c.op2
                ),
                format!(
                    "{:#x} {} {}:{}:{}:{}:{}",
                    s.encoding(),
                    s.is_write(),
                    s.op0(),
                    s.op1(),
                    s.crn(),
                    s.crm(),
                    s.op2()
                ),
            );
        }
        Operand::SysOp(op) => {
            same("the kind", kind(c.kind), kind(OperandKind::SysOp));
            if c.kind != OperandKind::SysOp {
                return out;
            }
            let c = of!(sysop);
            let row = exarmo_aarch64_capi::tables::SYSOPS
                .get(usize::from(c.index))
                .map(|def| unsafe {
                    std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                        def.name.data,
                        def.name.length,
                    ))
                });
            same(
                "the operation",
                format!("{:?} {}", row, c.field),
                format!("{:?} {}", Some(op.def().name), op.field()),
            );
        }
        Operand::Cond(cond) => {
            same("the kind", kind(c.kind), kind(OperandKind::Cond));
            if c.kind != OperandKind::Cond {
                return out;
            }
            same(
                "the condition",
                of!(cond).to_string(),
                (*cond as u8).to_string(),
            );
        }
        Operand::Symbol(s) => {
            same("the kind", kind(c.kind), kind(OperandKind::Symbol));
            if c.kind != OperandKind::Symbol {
                return out;
            }
            let c = of!(symbol);
            let written = unsafe {
                std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                    c.name.data,
                    c.name.length,
                ))
            };
            same(
                "the symbol",
                format!("{} {written}", c.bits),
                format!("{} {}", s.bits, s.name),
            );
        }
        Operand::ZaSlice(s) => {
            same("the kind", kind(c.kind), kind(OperandKind::ZaSlice));
            if c.kind != OperandKind::ZaSlice {
                return out;
            }
            let c = of!(za_slice);
            let array = match c.array {
                ZaArray::Array => "za".to_string(),
                ZaArray::Zt0 => "zt0".to_string(),
                ZaArray::Tile => format!("za{}", c.tile),
            };
            let rust_array = match s.array {
                exarmo_aarch64::ZaArray::Za => "za".to_string(),
                exarmo_aarch64::ZaArray::Zt0 => "zt0".to_string(),
                exarmo_aarch64::ZaArray::Tile(t) => t.to_string(),
            };
            same("the array", array, rust_array);
            same(
                "the slice",
                format!(
                    "{:?} {} {:?} {} {:?} {} {}",
                    (c.direction != 0).then_some(c.direction as char),
                    arrangement(c.arrangement),
                    c.has_index.then(|| name(c.index)),
                    c.offset,
                    optional(c.last),
                    c.mul_vl,
                    c.vector_group
                ),
                format!(
                    "{:?} {} {:?} {} {:?} {} {}",
                    s.direction,
                    rust_arrangement(s.arrangement),
                    s.index.map(|r| r.to_string()),
                    s.offset,
                    s.last,
                    s.mul_vl,
                    s.vector_group.unwrap_or(0)
                ),
            );
        }
        Operand::TileMask(m) => {
            same("the kind", kind(c.kind), kind(OperandKind::TileMask));
            if c.kind != OperandKind::TileMask {
                return out;
            }
            same("the mask", of!(tile_mask).to_string(), m.0.to_string());
        }
        Operand::Modifier(m) => {
            same("the kind", kind(c.kind), kind(OperandKind::Modifier));
            if c.kind != OperandKind::Modifier {
                return out;
            }
            same(
                "the modifier",
                format!("{:?}", modifier(of!(modifier))),
                format!("{:?}", rust_modifier(Some(*m))),
            );
        }
        Operand::Other => same("the kind", kind(c.kind), kind(OperandKind::Other)),
    }
    out
}

/// A token kind's number, by the name the hand-written header gives it.
fn token_kinds() -> Vec<(String, u32)> {
    let header = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/include/exarmo/aarch64.h"
    ))
    .expect("include/exarmo/aarch64.h");
    exarmo_testing::capi::token_kinds(&header, "EXARMO_AARCH64_TOKEN_")
}

/// Every case of the corpus reads the same through C as through Rust:
/// whether it decodes and why not, what it is, what it does with the flags
/// and to the flow of control, its operands field by field, its text and
/// every token of it, and the intrinsics it realises.
#[test]
fn the_c_api_agrees_with_rust_over_the_corpus() {
    let kinds = token_kinds();
    assert_eq!(kinds.len(), 10, "the header numbers every kind of token");
    let mut failures = Vec::new();
    let mut checked = 0;
    for case in cases() {
        checked += 1;
        let c = read(case.encoding);
        let rust = exarmo_aarch64::decode_word(case.encoding);
        let place = format!("{}  {}", case.place, case.hex);
        let inst = match rust {
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
        if (c.encoding, c.mnemonic, c.length)
            != (
                inst.encoding() as u32,
                inst.mnemonic() as u32,
                inst.encoding().length(),
            )
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
        for difference in
            exarmo_testing::capi::effect_differences(c.flags, inst.flags(), c.branch, at.branch())
        {
            fail(difference);
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

        let mut tokens: Vec<exarmo_aarch64::Token> = Vec::new();
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

        let intrinsics = inst.intrinsics();
        if c.intrinsic_count != intrinsics.len() {
            fail(format!(
                "{} intrinsics in C and {} in Rust",
                c.intrinsic_count,
                intrinsics.len()
            ));
        }
        for (c, r) in c.intrinsics.iter().zip(intrinsics.iter()) {
            let arguments: Vec<String> = c.arguments
                [..usize::from(c.argument_count).min(c.arguments.len())]
                .iter()
                .map(|a| format!("{:?} {} {}", a.kind, a.operand, a.shift))
                .collect();
            let rust: Vec<String> = r
                .arguments
                .iter()
                .map(|a| {
                    let a = model::IntrinsicSource::from(*a);
                    format!("{:?} {} {}", a.kind, a.operand, a.shift)
                })
                .collect();
            let result = model::IntrinsicOutput::from(r.result);
            if c.id != r.id
                || arguments != rust
                || (c.result.kind, c.result.operand, c.result.element)
                    != (result.kind, result.operand, result.element)
            {
                fail(format!(
                    "intrinsic {} is {arguments:?} {:?} in C and {rust:?} {result:?} in Rust",
                    r.id, c.result
                ));
            }
        }
    }
    exarmo_testing::report("the C API agrees with Rust", checked, &failures);
}

/// Reading every case of the corpus through C, with the buffers the header
/// sizes, asks nothing of the allocator, and so raises no panic either.
#[test]
fn the_c_api_allocates_nothing_over_the_corpus() {
    let cases = cases();
    // Counted from the very first call: a decode keeps no state, so nothing
    // is made on first use that a later call is spared
    let mut failures = Vec::new();
    for case in &cases {
        let (allocations, read) = exarmo_testing::capi::allocations(|| read(case.encoding));
        if allocations > 0 {
            failures.push(format!(
                "{}  {}: {allocations} allocations reading it, {:?}",
                case.place, case.hex, read.status
            ));
        }
    }
    exarmo_testing::report("the C API allocates nothing", cases.len(), &failures);
}
