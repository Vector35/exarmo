//! The intrinsics table agrees with the instructions. Over the corpus,
//! every binding points at an operand of the right kind, and a few
//! instructions of each shape carry the intrinsics they should, bound as
//! they should be. Which intrinsics every corpus encoding carries is the
//! snapshot in `intrinsics_snapshot.rs`.

use std::collections::BTreeSet;

mod corpus;

use exarmo_aarch64::{Instruction, Operand, Output, Source, TypeKind, decode_word};

#[test]
fn every_binding_names_an_operand_of_its_kind() {
    let text = corpus::text();
    let mut with = 0usize;
    let mut names = BTreeSet::new();
    let mut failures = Vec::new();
    for bits in corpus::encodings(&text) {
        let Ok(inst) = decode_word(bits) else {
            continue;
        };
        let intrinsics = inst.intrinsics();
        if intrinsics.is_empty() {
            continue;
        }
        with += 1;
        let operands = inst.operands();
        let mut check = |what: String, ok: bool| {
            if !ok {
                failures.push(format!("{bits:08X} {}: {what}", inst.at(0)));
            }
        };
        for intrinsic in intrinsics {
            names.insert(intrinsic.name());
            check(
                format!(
                    "{} has no arguments in a signature with some",
                    intrinsic.name()
                ),
                !intrinsic.arguments.is_empty() || intrinsic.def().parameters.is_empty(),
            );
            for argument in intrinsic.arguments {
                let at = |i: u8| operands.get(i as usize);
                let ok = match *argument {
                    Source::Operand(i) => matches!(
                        at(i),
                        Some(Operand::Reg(_) | Operand::List(_) | Operand::Mem(_))
                    ),
                    Source::Index(i) => {
                        matches!(at(i), Some(Operand::Reg(r)) if r.index.is_some())
                            || matches!(at(i), Some(Operand::List(l)) if l.index.is_some())
                    }
                    Source::Immediate { operand, .. } => {
                        matches!(at(operand), Some(Operand::Imm { .. }))
                    }
                    Source::Fpmr | Source::Unused => true,
                };
                check(
                    format!(
                        "{} argument at {argument:?} is not that kind of operand",
                        intrinsic.name()
                    ),
                    ok,
                );
            }
            let ok = match intrinsic.result {
                Output::None => intrinsic.def().result.def().kind == TypeKind::Void,
                Output::Operand(i) | Output::Element { operand: i, .. } => {
                    matches!(at(i, &operands), Some(Operand::Reg(_) | Operand::List(_)))
                }
            };
            check(
                format!(
                    "{} result {:?} is not a register",
                    intrinsic.name(),
                    intrinsic.result
                ),
                ok,
            );
        }
    }
    println!(
        "{with} corpus encodings have an intrinsic; {} distinct intrinsics",
        names.len()
    );
    for failure in failures.iter().take(30) {
        println!("  {failure}");
    }
    assert!(
        failures.is_empty(),
        "{} bindings do not fit",
        failures.len()
    );
}

fn at(i: u8, operands: &[Operand]) -> Option<&Operand> {
    operands.get(i as usize)
}

#[test]
fn a_lane_multiply_binds_its_lane() {
    // MLA v0.4h, v1.4h, v2.h[3]
    let inst = decode_word(0x2F720020).unwrap();
    assert_eq!(inst.at(0).to_string(), "mla\tv0.4h, v1.4h, v2.h[3]");
    let names: Vec<&str> = inst.intrinsics().iter().map(|i| i.name()).collect();
    assert!(names.contains(&"vmla_lane_s16"), "{names:?}");
    let vmla = inst
        .intrinsics()
        .iter()
        .find(|i| i.name() == "vmla_lane_s16")
        .unwrap();
    let sources: Vec<Source> = vmla.arguments.to_vec();
    assert_eq!(
        sources,
        [
            Source::Operand(0),
            Source::Operand(1),
            Source::Operand(2),
            Source::Index(2)
        ]
    );
    assert_eq!(vmla.result, Output::Operand(0));

    // add x0, x1, x2, which the table does not name, has none.
    assert!(decode_word(0x8B020020).unwrap().intrinsics().is_empty());
}

#[test]
fn the_table_carries_the_special_sources() {
    let all = &exarmo_aarch64::generated::intrinsics::INTRINSICS;
    let fp8 = all
        .iter()
        .find(|i| i.name().ends_with("_fpm"))
        .expect("an FP8 intrinsic");
    assert_eq!(*fp8.arguments.last().unwrap(), Source::Fpmr);
    let zip = all
        .iter()
        .filter(|i| i.name() == "vzip_s8")
        .map(|i| i.result)
        .collect::<BTreeSet<_>>();
    assert!(zip.contains(&Output::Element {
        operand: 0,
        element: 0
    }));
    assert!(zip.contains(&Output::Element {
        operand: 0,
        element: 1
    }));
    let _: &[Instruction] = &[];
}

/// Every binding names an intrinsic the table declares and agrees with it,
/// so a consumer that keeps intrinsics by number can trust the id.
#[test]
fn every_binding_matches_its_definition() {
    use exarmo_aarch64::intrinsics::DEFS;
    let mut seen = vec![false; DEFS.len()];
    for (index, def) in DEFS.iter().enumerate() {
        assert!(!def.name.is_empty(), "def {index} has no name");
        assert!(
            (def.result.def().kind == TypeKind::Void) == def.result.name().starts_with("void"),
            "{}'s result type disagrees with its kind",
            def.name
        );
        if index > 0 {
            assert!(
                DEFS[index - 1].name < def.name,
                "{} is out of name order",
                def.name
            );
        }
    }
    for intrinsic in exarmo_aarch64::generated::intrinsics::INTRINSICS.iter() {
        let def = DEFS.get(intrinsic.id as usize).unwrap_or_else(|| {
            panic!(
                "{} has id {} past the table",
                intrinsic.name(),
                intrinsic.id
            )
        });
        assert_eq!(
            def.name,
            intrinsic.name(),
            "id {} names two things",
            intrinsic.id
        );
        assert_eq!(
            def.parameters.len(),
            intrinsic.arguments.len(),
            "{} binds a different number of arguments than it declares",
            def.name
        );
        seen[intrinsic.id as usize] = true;
    }
    let unbound = seen.iter().filter(|s| !**s).count();
    assert_eq!(
        unbound, 0,
        "{unbound} intrinsics are declared but never bound"
    );
}

/// An intrinsic whose argument the instruction scales is listed only where
/// the instruction holds a value that argument can reach.
///
/// `vext_s16` is written `0 <= n <= 3` over `#(n<<1)`, so it realises
/// `ext ..., #0`, `#2`, `#4` and `#6` and no odd immediate. Read as the
/// span 0 to 6 it would be claimed for `#5` too, from which a consumer
/// reconstructs n=2, and `vext_s16(a, b, 2)` assembles to `#4`.
#[test]
fn a_scaled_argument_does_not_reach_between_its_steps() {
    let names = |bits: u32| -> BTreeSet<&'static str> {
        decode_word(bits)
            .expect("EXT")
            .intrinsics()
            .iter()
            .map(|i| i.name())
            .collect()
    };
    // ext v0.8b, v1.8b, v2.8b, #<imm>
    let ext = |imm: u32| 0x2E00_0000 | (2 << 16) | (imm << 11) | (1 << 5);

    for (imm, stepped, byte_wise) in [(4u32, true, true), (5, false, true)] {
        let names = names(ext(imm));
        assert_eq!(
            names.contains("vext_s16"),
            stepped,
            "#{imm} lists {names:?}"
        );
        // The unscaled sibling reaches every immediate, stepped or not.
        assert_eq!(
            names.contains("vext_s8"),
            byte_wise,
            "#{imm} lists {names:?}"
        );
    }
}

/// A row that writes its one allowed value as `lane==0` constrains the
/// instruction as one written `0 <= lane <= 0` does.
///
/// `vset_lane_s64` takes an `int64x1_t`, whose only lane is 0, and the table
/// says so as `lane==0`. Read only in the `0 <= lane <= 0` spelling, the row
/// constrains nothing and the intrinsic is offered for `mov v0.d[1], x1`.
#[test]
fn a_single_allowed_value_constrains_however_it_is_written() {
    let names = |bits: u32| -> BTreeSet<&'static str> {
        decode_word(bits)
            .expect("MOV (element, from general)")
            .intrinsics()
            .iter()
            .map(|i| i.name())
            .collect()
    };
    // mov v0.d[1], x1. Lane 1 exists only on the q-form's int64x2_t.
    let one = names(0x4E18_1C20);
    assert!(
        !one.contains("vset_lane_s64"),
        "lane 1 lists {one:?}, but int64x1_t has only lane 0"
    );
    // The q-form, whose lane 1 does exist, is still listed, so the check
    // above is not vacuous.
    assert!(
        one.contains("vsetq_lane_s64"),
        "lane 1 lists {one:?}, which should still hold the q-form"
    );

    // Lane 0, where both apply, keeps them both.
    let zero = names(0x4E08_1C20);
    assert!(
        zero.contains("vset_lane_s64") && zero.contains("vsetq_lane_s64"),
        "lane 0 lists {zero:?}"
    );
}

/// The vfmlal and vfmlsl families bind each multiplicand to the register
/// the instruction reads it from.
///
/// ACLE's table maps every argument of these to `Vd`, where the instruction
/// writes `Vd`, `Vn` and `Vm`, so read as written it binds the accumulator
/// where the first multiplicand belongs. `acle::MAPPING_ERRATA` corrects it,
/// and this holds the correction. A binding that names a vector operand fits
/// whichever one it names, so nothing else here would notice.
#[test]
fn a_widening_multiply_binds_each_multiplicand_to_its_own_register() {
    // fmlal v0.2s, v1.2h, v2.2h
    let inst = decode_word(0x0E22EC20).unwrap();
    assert_eq!(inst.at(0).to_string(), "fmlal\tv0.2s, v1.2h, v2.2h");
    let low = inst
        .intrinsics()
        .iter()
        .find(|i| i.name() == "vfmlal_low_f16")
        .expect("the table names it");
    assert_eq!(
        low.arguments.to_vec(),
        [Source::Operand(0), Source::Operand(1), Source::Operand(2)],
        "r is the accumulator, a the first multiplicand, b the second"
    );
    assert_eq!(low.result, Output::Operand(0));

    // fmlal2 v12.2s, v26.2h, v15.h[2]. The by-element form binds its lane
    // from the operand it multiplies by, not from the accumulator.
    let lane = decode_word(0x2FAF834C).unwrap();
    assert_eq!(lane.at(0).to_string(), "fmlal2\tv12.2s, v26.2h, v15.h[2]");
    let high = lane
        .intrinsics()
        .iter()
        .find(|i| i.name() == "vfmlal_lane_high_f16")
        .expect("the table names it");
    assert_eq!(
        high.arguments.to_vec(),
        [
            Source::Operand(0),
            Source::Operand(1),
            Source::Operand(2),
            Source::Index(2)
        ]
    );

    // Each mnemonic's 64-bit form binds three distinct operands, which the
    // table as written does not.
    let mut checked = 0;
    for intrinsic in [
        decode_word(0x0E22EC20).unwrap(), // fmlal  v0.2s, v1.2h, v2.2h
        decode_word(0x0EA2EC20).unwrap(), // fmlsl  v0.2s, v1.2h, v2.2h
        decode_word(0x2E22CC20).unwrap(), // fmlal2 v0.2s, v1.2h, v2.2h
        decode_word(0x2EA2CC20).unwrap(), // fmlsl2 v0.2s, v1.2h, v2.2h
    ]
    .iter()
    .flat_map(|inst| inst.intrinsics().to_vec())
    {
        let operands: BTreeSet<u8> = intrinsic
            .arguments
            .iter()
            .filter_map(|source| match source {
                Source::Operand(n) => Some(*n),
                _ => None,
            })
            .collect();
        assert_eq!(operands.len(), 3, "{} binds {operands:?}", intrinsic.name());
        checked += 1;
    }
    assert!(checked >= 4, "the family is named for each type");
}

/// BSL, BIT and BIF are one select, with the mask and the two values in
/// different registers. vbsl is `(a AND b) OR (NOT a AND c)`.
#[test]
fn a_bitwise_select_binds_its_mask_wherever_the_instruction_reads_it() {
    // v0, v1 and v2 in each, so every argument lands in a distinct register
    for (bits, text, arguments) in [
        (0x6E621C20, "bsl\tv0.16b, v1.16b, v2.16b", [0, 1, 2]),
        (0x6EA21C20, "bit\tv0.16b, v1.16b, v2.16b", [2, 1, 0]),
        (0x6EE21C20, "bif\tv0.16b, v1.16b, v2.16b", [2, 0, 1]),
    ] {
        let inst = decode_word(bits).unwrap();
        assert_eq!(inst.at(0).to_string(), text);
        let intrinsics = inst.intrinsics();
        assert!(!intrinsics.is_empty(), "{text}");
        for intrinsic in intrinsics.iter() {
            assert_eq!(
                intrinsic.arguments.to_vec(),
                arguments.map(Source::Operand),
                "{text}: {}",
                intrinsic.name()
            );
            assert_eq!(intrinsic.result, Output::Operand(0), "{text}");
        }
    }
}

/// A row that maps one argument to two registers says the instruction reads
/// one vector twice, so it is not the instruction whose two sources differ.
///
/// `vaddv_s32` is written `a -> Vn.2S; a -> Vm.2S` over
/// `ADDP Vd.2S,Vn.2S,Vm.2S`. It reduces one vector by pairing it with
/// itself. Listed for an ADDP that pairs two different vectors it is the
/// first intrinsic offered, so a consumer taking that one reads the
/// instruction as adding a register to itself that it does not.
#[test]
fn a_reduction_is_only_the_instruction_that_reads_one_vector() {
    let names = |bits: u32| -> Vec<&'static str> {
        decode_word(bits)
            .expect("ADDP")
            .intrinsics()
            .iter()
            .map(|i| i.name())
            .collect()
    };

    // addp v16.2s, v25.2s, v0.2s reads two vectors, so is the pairwise add
    // alone.
    assert_eq!(names(0x0EA0BF30), ["vpadd_s32", "vpadd_u32"]);
    // addp v16.2s, v25.2s, v25.2s reads one vector twice, so is the
    // reduction too.
    assert_eq!(
        names(0x0EB9BF30),
        ["vpadd_s32", "vpadd_u32", "vaddv_s32", "vaddv_u32"]
    );

    // The same for the others the table writes that way.
    for (differing, same, reduction) in [
        (0x0EB3A5E8u32, 0x0EAFA5EFu32, "vmaxv_s32"), // smaxp v8.2s, ...
        (0x2EB3A5E8, 0x2EAFA5EF, "vmaxv_u32"),       // umaxp
        (0x0EB3ADE8, 0x0EAFADEF, "vminv_s32"),       // sminp
        (0x2EB3ADE8, 0x2EAFADEF, "vminv_u32"),       // uminp
    ] {
        assert!(
            !names(differing).contains(&reduction),
            "{reduction} is offered for {differing:08X}, whose sources differ"
        );
        assert!(
            names(same).contains(&reduction),
            "{reduction} is not offered for {same:08X}, which reads one vector"
        );
    }
}

/// A comparison of `a` and `b` is one instruction with the sources the other
/// way round from its converse, so CMGE is vcge with `a` in `Vn` and vcle
/// with `a` in `Vm`. The table writes some rows in the converse's order,
/// and `ORDER_ERRATA` corrects them. Each instruction carries both, and
/// takes `a` from the operand that side of the comparison is.
#[test]
fn a_comparison_binds_each_side_to_the_operand_on_that_side() {
    // Words the table writes the wrong way round, at each mnemonic and at
    // both precisions it gets wrong
    for (bits, text, greater, less) in [
        (
            0x4EEC3F22,
            "cmge\tv2.2d, v25.2d, v12.2d",
            "vcgeq_s64",
            "vcleq_s64",
        ),
        (
            0x6EB23DC5,
            "cmhs\tv5.4s, v14.4s, v18.4s",
            "vcgeq_u32",
            "vcleq_u32",
        ),
        (
            0x2E3EE693,
            "fcmge\tv19.2s, v20.2s, v30.2s",
            "vcge_f32",
            "vcle_f32",
        ),
        (
            0x2E422420,
            "fcmge\tv0.4h, v1.4h, v2.4h",
            "vcge_f16",
            "vcle_f16",
        ),
        (
            0x6EE2EC20,
            "facgt\tv0.2d, v1.2d, v2.2d",
            "vcagtq_f64",
            "vcaltq_f64",
        ),
        (0x7E422420, "fcmge\th0, h1, h2", "vcgeh_f16", "vcleh_f16"),
        // The scalar rows are written right, and stay so
        (0x5EE23C20, "cmge\td0, d1, d2", "vcge_s64", "vcle_s64"),
    ] {
        let inst = decode_word(bits).unwrap();
        assert_eq!(inst.at(0).to_string(), text);
        for (name, arguments) in [(greater, [1, 2]), (less, [2, 1])] {
            let intrinsic = inst
                .intrinsics()
                .iter()
                .find(|i| i.name() == name)
                .unwrap_or_else(|| panic!("{text} does not carry {name}"));
            assert_eq!(
                intrinsic.arguments.to_vec(),
                arguments.map(Source::Operand),
                "{text}: {name}"
            );
        }
    }
}

/// The table writes vqdmulhs_lane_s32's lane as `Vm.H[lane]`, which is
/// the 16-bit form's, so the 32-bit one carried nothing.
#[test]
fn a_scalar_multiply_by_element_binds_its_lane_at_its_own_width() {
    // The highest lane, which only the 128-bit vector reaches
    let inst = decode_word(0x5FA1C97D).unwrap();
    assert_eq!(inst.at(0).to_string(), "sqdmulh\ts29, s11, v1.s[3]");
    let names: Vec<&str> = inst.intrinsics().iter().map(|i| i.name()).collect();
    assert_eq!(names, ["vqdmulhs_laneq_s32"]);
    assert_eq!(
        inst.intrinsics()[0].arguments.to_vec(),
        [Source::Operand(1), Source::Operand(2), Source::Index(2)]
    );

    // Lane 0 is in the 64-bit vector too
    let inst = decode_word(0x5F81C17D).unwrap();
    assert_eq!(inst.at(0).to_string(), "sqdmulh\ts29, s11, v1.s[0]");
    let names: BTreeSet<&str> = inst.intrinsics().iter().map(|i| i.name()).collect();
    assert_eq!(
        names,
        BTreeSet::from(["vqdmulhs_lane_s32", "vqdmulhs_laneq_s32"])
    );
}
