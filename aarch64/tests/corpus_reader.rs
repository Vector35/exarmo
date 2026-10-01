//! AArch64's own corpus rules. exarmo-testing tests the harness's.

mod corpus;

use corpus::normalize;

/// A condition and a prefetch hint are compared as the corpus writes them,
/// which is what the XML's value tables name.
#[test]
fn nothing_is_rewritten() {
    assert_eq!(normalize("b.cc 0x100"), "b.cc 0x100");
    assert_eq!(normalize("b.lo 0x100"), "b.lo 0x100");
    assert_eq!(normalize("prfm pldl1strm, [x0]"), "prfm pldl1strm, [x0]");
}
