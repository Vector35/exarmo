//! The system operations and PSTATE fields, against what the architecture
//! encodes each of these as.
//!
//! Every row is read from the bundle, so this checks that the reading lands
//! where the encoding does. A word that names an operation is decoded, and
//! the fields it holds are the ones the table gives.

use std::collections::HashSet;

mod corpus;

use exarmo_aarch64::{Mnemonic, Operand, SysOp, decode};

/// The table's entry for a name written by an instruction.
fn entry(instruction: Mnemonic, name: &str) -> SysOp {
    SysOp::all()
        .find(|op| op.instruction == instruction && op.name == name)
        .unwrap_or_else(|| panic!("{instruction:?} names {name}"))
}

/// The five fields of a system instruction, as the word holds them.
fn fields(bits: u32) -> [u8; 5] {
    let at = |hi: u32, width: u32| ((bits >> hi) & ((1 << width) - 1)) as u8;
    [at(19, 2), at(16, 3), at(12, 4), at(8, 4), at(5, 3)]
}

/// An operation's row is the encoding the word it is written in holds. The
/// four instructions hold their operand's bits differently, which is the
/// whole reason the table exists.
#[test]
fn an_operation_is_the_encoding_its_word_holds() {
    for (bits, instruction, name) in [
        (0xD508780Cu32, Mnemonic::At, "s1e1r"),
        (0xD508831Fu32, Mnemonic::Tlbi, "vmalle1is"),
        (0xD50B7420u32, Mnemonic::Dc, "zva"),
        (0xD50B7520u32, Mnemonic::Ic, "ivau"),
    ] {
        let op = entry(instruction, name);
        assert_eq!(
            [op.op0, op.op1, op.crn, op.crm, op.op2],
            fields(bits),
            "{instruction:?} {name}"
        );
        // The page fixes every bit of CRm, so all of them name it.
        assert_eq!(op.crm_names, 0xF, "{instruction:?} {name}");
        // The decoded instruction's first operand names the row.
        let inst = decode(bits).unwrap();
        assert_eq!(inst.mnemonic(), instruction);
        let Operand::SysOp(named) = inst.operands()[0] else {
            panic!("{instruction:?} {name} names a row of the table");
        };
        assert_eq!(named.def(), op, "{instruction:?} {name}");
    }
}

/// The row is written into the decode at generation. This sweeps the whole
/// corpus rather than chosen words, since a row written wrong would be wrong
/// for one instruction and right for its neighbours.
#[test]
fn an_operand_names_the_row_its_word_encodes() {
    let text = corpus::text();
    let mut seen = std::collections::HashSet::new();
    for line in text.lines() {
        let Some((hex, _)) = line.split_once(' ') else {
            continue;
        };
        let Ok(word) = u32::from_str_radix(hex, 16) else {
            continue;
        };
        let Ok(inst) = decode(word) else { continue };
        for operand in inst.operands().iter() {
            let Operand::SysOp(named) = operand else {
                continue;
            };
            let def = named.def();
            assert_eq!(def.instruction, inst.mnemonic(), "{hex}");
            // An operation is named by every bit the page fixes. A PSTATE
            // field takes the rest of CRm as the instruction's immediate,
            // so only the bits the table names are compared.
            let [op0, op1, crn, crm, op2] = fields(word);
            assert_eq!(
                [def.op0, def.op1, def.crn, def.op2],
                [op0, op1, crn, op2],
                "{hex}"
            );
            assert_eq!(def.crm & def.crm_names, crm & def.crm_names, "{hex}");
            seen.insert(named.index());
        }
    }
    assert!(
        seen.len() > 200,
        "the corpus reaches {} of the {} rows",
        seen.len(),
        SysOp::COUNT
    );
}

/// A PSTATE field is named by op1 and op2, and MSR writes its immediate in
/// CRm, so the same field is reached by sixteen words that differ there.
#[test]
fn a_pstate_field_takes_its_immediate_in_crm() {
    let daifset = entry(Mnemonic::Msr, "daifset");
    assert_eq!([daifset.op0, daifset.op1, daifset.op2], [0, 3, 6]);
    assert_eq!(daifset.crn, 4, "MSR fixes CRn in its diagram");
    assert_eq!(daifset.crm_names, 0, "CRm is the whole of the immediate");

    // Two words that write the same field with different immediates land on
    // the one row, carrying the field bits that differ between them. A
    // consumer given the bits alone would read these as two different
    // fields.
    let mut rows = Vec::new();
    for (bits, imm) in [(0xD50343DFu32, 3), (0xD50345DFu32, 5)] {
        let inst = decode(bits).unwrap();
        let operands = inst.operands();
        let Operand::SysOp(op) = operands[0] else {
            panic!("a PSTATE field names a row of the table");
        };
        assert_eq!(op.def().name, "daifset");
        assert_eq!(op.def().instruction, Mnemonic::Msr);
        assert_eq!(
            operands[1],
            Operand::Imm {
                value: imm,
                modifier: None
            }
        );
        // The word holds the immediate where the table says CRm is.
        assert_eq!(fields(bits)[3], imm as u8);
        rows.push((op.index(), op.field()));
    }
    assert_eq!(rows[0].0, rows[1].0, "one field, so one row");
    assert_ne!(rows[0].1, rows[1].1, "different immediates, different bits");
}

/// SVCRSM and its neighbours are named by three of CRm's four bits and take
/// the fourth as the immediate, which is why the table says which bits name
/// an operation rather than whether any do.
#[test]
fn a_field_named_by_part_of_crm_says_which_part() {
    for name in ["svcrsm", "svcrza", "svcrsmza"] {
        let op = entry(Mnemonic::Msr, name);
        assert_eq!(op.crm_names, 0xE, "{name}");
        assert_eq!(
            op.crm & op.crm_names,
            op.crm,
            "{name} fixes only those bits"
        );
    }
    // ALLINT is named by the same three, and PM likewise.
    assert_eq!(entry(Mnemonic::Msr, "allint").crm_names, 0xE);
}

/// The packed encoding is the five fields in order, at the widths the
/// architecture gives them, which is how `SysRegDef` packs one too.
#[test]
fn the_packed_encoding_is_the_fields_in_order() {
    for op in SysOp::all() {
        let packed = u16::from(op.op0) << 14
            | u16::from(op.op1) << 11
            | u16::from(op.crn) << 7
            | u16::from(op.crm) << 3
            | u16::from(op.op2);
        assert_eq!(op.encoding, packed, "{:?} {}", op.instruction, op.name);
    }
}

#[test]
fn every_row_is_named_once_by_one_mechanism() {
    let mut seen = std::collections::HashSet::new();
    for op in SysOp::all() {
        assert!(
            op.op0 == 0 || op.op0 == 1,
            "{:?} {} is op0 {}",
            op.instruction,
            op.name,
            op.op0
        );
        assert!(
            seen.insert((op.instruction, op.name)),
            "{:?} names {} twice",
            op.instruction,
            op.name
        );
    }
    assert_eq!(seen.len(), SysOp::COUNT);
    const { assert!(SysOp::COUNT > 400, "the architecture names hundreds") };
}

/// Both tables pack `op0:op1:CRn:CRm:op2` the same way, `op0` as the two
/// bits it is, so the whole system instruction space is numbered once. An
/// operation is `op0` 0 or 1 and a register 2 or 3, which keeps them apart.
/// PLBI's `vmalle1is` and `amair_el1` share every other field, so a packing
/// that dropped `op0`'s high bit would give both 0x4518.
#[test]
fn an_operation_and_a_register_never_share_an_encoding() {
    use exarmo_aarch64::SysReg;

    let registers: std::collections::HashMap<u16, &str> =
        SysReg::all().map(|def| (def.encoding, def.name)).collect();
    let clashes: Vec<String> = SysOp::all()
        .filter_map(|op| {
            registers.get(&op.encoding).map(|name| {
                format!(
                    "0x{:04X} is {:?} {} and the register {name}",
                    op.encoding, op.instruction, op.name
                )
            })
        })
        .collect();
    assert!(clashes.is_empty(), "{clashes:#?}");

    // The pair that differ only in op0's high bit
    let plbi = SysOp::all()
        .find(|op| op.instruction == Mnemonic::Plbi && op.name == "vmalle1is")
        .expect("PLBI names it");
    let amair = SysReg::all()
        .find(|def| def.name == "amair_el1")
        .expect("the architecture names it");
    assert_eq!((plbi.encoding, amair.encoding), (0x4518, 0xC518));
    assert_eq!((plbi.op0, SysReg::read(amair.encoding).op0()), (1, 3));

    // Every operation is below the registers, which all have op0's high bit
    assert!(SysOp::all().all(|op| op.encoding & 0x8000 == 0));
    assert!(SysReg::all().all(|def| def.encoding & 0x8000 != 0));
}

/// An encoding says where in the space an operation sits, not which row it
/// is.
///
/// TLBI and TLBIP reach the same point by the same five fields, under the
/// same name. The 128-bit form is a different instruction, not a different
/// place. So a consumer looking an operation up by its fields alone cannot
/// tell which of the two it holds, and one keying a map on the encoding
/// silently keeps one row of each pair.
#[test]
fn two_operations_can_reach_one_point_in_the_space() {
    let mut by_fields: std::collections::HashMap<(u8, u8, u8, u8, u8), Vec<SysOp>> =
        std::collections::HashMap::new();
    for op in SysOp::all() {
        by_fields
            .entry((op.op0, op.op1, op.crn, op.crm, op.op2))
            .or_default()
            .push(op);
    }
    let shared: Vec<&Vec<SysOp>> = by_fields.values().filter(|ops| ops.len() > 1).collect();

    // Every one of them is a TLBI and its 128-bit TLBIP, named alike, and
    // they carry the one encoding between them.
    for ops in &shared {
        assert_eq!(ops.len(), 2, "more than two rows at one point");
        let mut instructions = [ops[0].instruction, ops[1].instruction];
        instructions.sort_by_key(|m| m.name());
        assert_eq!(
            instructions,
            [Mnemonic::Tlbi, Mnemonic::Tlbip],
            "{} and {} share a point",
            ops[0].name,
            ops[1].name
        );
        assert_eq!(ops[0].name, ops[1].name);
        assert_eq!(ops[0].encoding, ops[1].encoding);
    }
    assert_eq!(shared.len(), 120);

    // Nothing else in the space is shared, so the instruction and the name
    // together are the identity, which the listing test holds unique.
    let tlbip = SysOp::all().filter(|op| op.instruction == Mnemonic::Tlbip);
    assert_eq!(tlbip.count(), 120);
}

/// Each alias's preference condition asks ARM's `SysOp` pseudocode what the
/// four fields name, so an operation that function leaves unclassified is
/// written as a bare `SYS` however the rest of the bundle spells it. The
/// bundle's `SysOp` omits operations its value tables carry, which the
/// generator supplies.
///
/// A release that adds an operation to a value table and to the SysReg pages
/// but not to `SysOp` fails here, rather than quietly disassembling as
/// `sys`.
#[test]
fn every_operation_sys_reaches_is_written_as_its_alias() {
    // GICR's are read through SYSL, so a SYS word does not name them. The
    // rest of the space is written by SYS.
    const READ_THROUGH_SYSL: Mnemonic = Mnemonic::Gicr;

    let mut bare = Vec::new();
    let mut reached = 0;
    for op in SysOp::all().filter(|op| op.op0 == 1) {
        if op.instruction == READ_THROUGH_SYSL {
            continue;
        }
        let word = 0xD508_0000
            | (u32::from(op.op1) << 16)
            | (u32::from(op.crn) << 12)
            | (u32::from(op.crm) << 8)
            | (u32::from(op.op2) << 5)
            | 31;
        // TLBIP shares its encodings with TLBI and is written by SYSP, so
        // what the word names is asked rather than which row it came from.
        match decode(word) {
            Ok(inst) if inst.mnemonic() != Mnemonic::Sys => reached += 1,
            _ => bare.push(format!("{:?} {}", op.instruction, op.name)),
        }
    }
    assert!(
        bare.is_empty(),
        "SysOp leaves these unclassified, so they are written as a bare sys: {}",
        bare.join(", ")
    );
    assert!(reached > 300, "only {reached} operations were reached");
}

/// A consumer lifting these finds the operation by the kind of the operand,
/// so an instruction whose operation arrives as anything else falls out of
/// that path however plainly the disassembly reads. CFP, DVP, COSP and CPP
/// need care. Their template writes `RCTX` as a literal where the other
/// families write a placeholder with a value table behind it, and without
/// the row nothing but the mnemonic tells CFP RCTX from CPP RCTX.
#[test]
fn every_instruction_naming_an_operation_carries_the_row() {
    let naming: HashSet<Mnemonic> = SysOp::all()
        .filter(|op| op.op0 == 1)
        .map(|op| op.instruction)
        .collect();

    let text = corpus::text();
    let mut carried: HashSet<Mnemonic> = HashSet::new();
    for line in text.lines() {
        let Some((hex, _)) = line.split_once(' ') else {
            continue;
        };
        let Ok(word) = u32::from_str_radix(hex, 16) else {
            continue;
        };
        let Ok(inst) = decode(word) else { continue };
        if !naming.contains(&inst.mnemonic()) {
            continue;
        }
        let named = inst.operands().iter().any(|operand| {
            matches!(operand, Operand::SysOp(op) if op.def().instruction == inst.mnemonic())
        });
        assert!(named, "{hex} {} names no row", inst.at(0));
        carried.insert(inst.mnemonic());
    }

    // And the corpus reaches all of them, so the assertion above is asked of
    // every family rather than of whichever the corpus happens to hold.
    let missing: Vec<String> = naming
        .difference(&carried)
        .map(|m| format!("{m:?}"))
        .collect();
    assert!(
        missing.is_empty(),
        "the corpus reaches no word of: {}",
        missing.join(", ")
    );
}

/// Whether an operation takes a register is what its page lays out, and
/// which way and how wide it moves is what its pseudocode does with it.
#[test]
fn an_operation_says_what_it_does_with_its_register() {
    use exarmo_aarch64::{RegisterAccess, RegisterUse};
    for (instruction, name, reg_use, reg_access, reg_bits) in [
        // Reads nothing, and Rt has to be 0b11111
        (
            Mnemonic::Ic,
            "ialluis",
            RegisterUse::None,
            RegisterAccess::Read,
            0,
        ),
        // The register carries something only on FEAT_TLBID
        (
            Mnemonic::Tlbi,
            "vmalle1is",
            RegisterUse::Optional,
            RegisterAccess::Read,
            64,
        ),
        (
            Mnemonic::Dc,
            "zva",
            RegisterUse::Required,
            RegisterAccess::Read,
            64,
        ),
        // Its result is written to the register, as SYSL's are
        (
            Mnemonic::Gicr,
            "cdia",
            RegisterUse::Required,
            RegisterAccess::Write,
            64,
        ),
        // Reads the pair as one value
        (
            Mnemonic::Tlbip,
            "vae1is",
            RegisterUse::Required,
            RegisterAccess::Read,
            128,
        ),
        // A PSTATE field takes its immediate and no register
        (
            Mnemonic::Msr,
            "daifset",
            RegisterUse::None,
            RegisterAccess::Read,
            0,
        ),
    ] {
        let op = entry(instruction, name);
        assert_eq!(
            (op.reg_use, op.reg_access, op.reg_bits),
            (reg_use, reg_access, reg_bits),
            "{instruction:?} {name}"
        );
    }
}

/// Every operation of one instruction that takes a register moves it the
/// same way and at the same width, which is what lets a consumer declare
/// one signature per instruction.
#[test]
fn an_instruction_moves_its_register_one_way() {
    use exarmo_aarch64::RegisterUse;
    let mut seen = std::collections::HashMap::new();
    for op in SysOp::all().filter(|op| op.reg_use != RegisterUse::None) {
        let first = *seen
            .entry(op.instruction)
            .or_insert((op.reg_access, op.reg_bits));
        assert_eq!(
            first,
            (op.reg_access, op.reg_bits),
            "{:?} {}",
            op.instruction,
            op.name
        );
        assert!(op.reg_bits == 64 || op.reg_bits == 128, "{}", op.name);
    }
    assert!(seen.len() > 1);
}
