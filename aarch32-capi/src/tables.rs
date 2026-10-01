//! The tables the architecture fixes, laid out for C to read directly.
//!
//! The header states how long each one is. An entry's name points into the
//! library and lives as long as it does.

use crate::model::SysRegDef;

/// Every system register the index names for MRC, MCR, MRRC and MCRR, a
/// space at a time and in key order within one. `exarmo_aarch32_sysregs`
/// in C.
#[unsafe(export_name = "exarmo_aarch32_sysregs")]
pub static SYSREGS: [SysRegDef; exarmo_aarch32::SysRegDef::COUNT] = {
    let mut out = [SysRegDef::at(0); exarmo_aarch32::SysRegDef::COUNT];
    let mut index = 1;
    while index < exarmo_aarch32::SysRegDef::COUNT {
        out[index] = SysRegDef::at(index);
        index += 1;
    }
    out
};
