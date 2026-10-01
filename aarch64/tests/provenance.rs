//! The generated code says what it was generated from.

use exarmo_aarch64::PROVENANCE;

#[test]
fn the_inputs_are_named() {
    assert!(
        PROVENANCE.isa.starts_with("ISA_A64_xml_A_profile"),
        "{}",
        PROVENANCE.isa
    );
    assert!(
        PROVENANCE.sysreg.starts_with("SysReg_xml_A_profile"),
        "{}",
        PROVENANCE.sysreg
    );
    // The two bundles are one release, FAT or not.
    let release = |name: &str| name.rsplit('_').next().unwrap_or("").to_string();
    assert_eq!(release(PROVENANCE.isa), release(PROVENANCE.sysreg));
    assert_eq!(
        PROVENANCE.acle_commit.len(),
        40,
        "{}",
        PROVENANCE.acle_commit
    );
    assert!(!exarmo_aarch64::generated::intrinsics::INTRINSICS.is_empty());
}
