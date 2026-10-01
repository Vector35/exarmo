//! The tables the architecture fixes, laid out for C to read directly.
//!
//! These are constants, so a caller reads them in place without sizing a
//! buffer, copying or freeing anything. The header states how long each one
//! is.
//!
//! An entry's name points into the library and lives as long as it does.

use crate::model::{SysOpDef, SysRegDef};

/// `exarmo_aarch64_sysregs`, every system register the architecture names,
/// in encoding order.
///
/// A register with one name in both directions appears once. One with a
/// different name in each, as DBGDTRRX_EL0 and DBGDTRTX_EL0, appears once
/// per name.
#[unsafe(export_name = "exarmo_aarch64_sysregs")]
pub static SYSREGS: [SysRegDef; exarmo_aarch64::SysRegDef::COUNT] = {
    let mut out = [SysRegDef::at(0); exarmo_aarch64::SysRegDef::COUNT];
    let mut index = 1;
    while index < exarmo_aarch64::SysRegDef::COUNT {
        out[index] = SysRegDef::at(index);
        index += 1;
    }
    out
};

/// `exarmo_aarch64_sysops`, every system operation and PSTATE field the
/// architecture names, in encoding order and then by name.
#[unsafe(export_name = "exarmo_aarch64_sysops")]
pub static SYSOPS: [SysOpDef; exarmo_aarch64::SysOp::COUNT] = {
    let mut out = [SysOpDef::at(0); exarmo_aarch64::SysOp::COUNT];
    let mut index = 1;
    while index < exarmo_aarch64::SysOp::COUNT {
        out[index] = SysOpDef::at(index);
        index += 1;
    }
    out
};
