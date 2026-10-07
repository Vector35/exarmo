//! `SysReg::all` lists every system register the index names for MRC, MCR,
//! MRRC and MCRR, and an instruction accessing one names it.

use exarmo_aarch32::{ItState, SysReg, SysRegSpace, a32, t32};

fn register(bits: u32) -> Option<SysReg> {
    a32::decode_word(bits).unwrap().system_register()
}

/// An MRC names the register it reads, and an MCR the one it writes, while
/// the text stays as the architecture writes it.
#[test]
fn a_coprocessor_access_names_its_register() {
    let mrc = a32::decode_word(0xEE110F10).unwrap();
    assert_eq!(
        mrc.at(0).to_string(),
        "mrc\tp15, #0x0, r0, c1, c0, #0x0",
        "the text names no register"
    );
    let sctlr = mrc.system_register().unwrap();
    assert_eq!(sctlr.space(), SysRegSpace::MrcMcr);
    assert_eq!(sctlr.name(), Some("sctlr"));
    assert!(!sctlr.is_write());

    let mcr = register(0xEE010F10).unwrap();
    assert!(mcr.is_write());
    assert_eq!(mcr.name(), Some("sctlr"));

    // T32 writes the same instruction in the same fields
    let t32 = t32::decode_word(0xEE110F10, ItState::Outside)
        .unwrap()
        .system_register()
        .unwrap();
    assert_eq!(t32.name(), Some("sctlr"));
}

/// MRRC and MCRR name a 64-bit register by a key of their own.
#[test]
fn a_64_bit_access_names_its_register() {
    let ttbr0 = register(0xEC510F02).unwrap();
    assert_eq!(ttbr0.space(), SysRegSpace::MrrcMcrr);
    assert_eq!(ttbr0.name(), Some("ttbr0"));
}

#[test]
fn anything_else_names_none() {
    assert_eq!(register(0xE2800001), None, "add r0, r0, #1");
}

#[test]
fn every_register_looks_itself_up() {
    let all: Vec<_> = SysReg::all().collect();
    assert!(all.len() > 500, "{} registers", all.len());
    for def in &all {
        for write in [false, true] {
            let named = match write {
                false => def.readable,
                true => def.writable,
            };
            if named {
                let reg = SysReg::new(def.space, def.encoding, write);
                assert_eq!(reg.name(), Some(def.name), "{def:?}");
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    for def in &all {
        assert!(
            seen.insert((def.space, def.encoding, def.name)),
            "{def:?} twice"
        );
    }
}
