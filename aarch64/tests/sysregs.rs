//! The system registers the architecture names can be listed, and the list
//! agrees with the lookup.
//!
//! An encoding is the whole `op0:op1:CRn:CRm:op2`, `op0` as the two bits it
//! is. Every register MRS and MSR reach has `op0` 2 or 3, so the top bit is
//! always set, and the space below it is where `SysOp` names the operations
//! and PSTATE fields.

use exarmo_aarch64::{SysReg, SysRegDef};

#[test]
fn the_list_is_long_sorted_and_named() {
    let all: Vec<SysRegDef> = SysReg::all().collect();
    assert!(all.len() > 1000, "{}", all.len());
    assert!(all.windows(2).all(|p| p[0].encoding <= p[1].encoding));
    assert!(
        all.iter()
            .all(|r| !r.name.is_empty() && (r.readable || r.writable))
    );
    let nzcv = all.iter().find(|r| r.name == "nzcv").unwrap();
    assert_eq!(nzcv.encoding, 0xda10);
    assert!(nzcv.readable && nzcv.writable);
}

#[test]
fn every_listed_register_looks_itself_up() {
    for def in SysReg::all() {
        if def.readable {
            assert_eq!(SysReg::read(def.encoding).name(), Some(def.name));
        }
        if def.writable {
            assert_eq!(SysReg::write(def.encoding).name(), Some(def.name));
        }
    }
}

#[test]
fn a_family_is_listed_member_by_member() {
    let members: Vec<&str> = SysReg::all()
        .filter(|r| r.name.starts_with("dbgbvr") && r.name.ends_with("_el1"))
        .map(|r| r.name)
        .collect();
    assert_eq!(members.len(), 16);
    assert!(members.contains(&"dbgbvr15_el1"));
}

#[test]
fn a_shared_encoding_is_listed_once_per_name() {
    let shared: Vec<SysRegDef> = SysReg::all().filter(|r| r.encoding == 0x9828).collect();
    let names: Vec<(&str, bool, bool)> = shared
        .iter()
        .map(|r| (r.name, r.readable, r.writable))
        .collect();
    assert_eq!(
        names,
        [("dbgdtrrx_el0", true, false), ("dbgdtrtx_el0", false, true)]
    );
}
