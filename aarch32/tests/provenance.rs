//! The generated code says what it was generated from.

use exarmo_aarch32::PROVENANCE;

#[test]
fn the_inputs_are_named() {
    assert!(
        PROVENANCE.isa.starts_with("ISA_AArch32_xml_A_profile"),
        "{}",
        PROVENANCE.isa
    );
    assert!(
        PROVENANCE.sysreg.starts_with("SysReg_xml_A_profile"),
        "{}",
        PROVENANCE.sysreg
    );
    // The two bundles are of one release. The ISA bundle is the plain
    // A-profile one, since Arm publishes no FAT release of AArch32, so only
    // the release is compared.
    fn release(name: &str) -> &str {
        name.split_once('-').map_or("", |(_, rest)| rest)
    }
    assert_eq!(release(PROVENANCE.isa), release(PROVENANCE.sysreg));
}
