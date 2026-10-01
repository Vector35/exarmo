//! `tests/c/smoke.c` is compiled against `include/` and linked with the
//! static library Cargo built for this test run, and must run clean.

#[test]
fn c_can_use_the_library() {
    exarmo_testing::c_smoke(env!("CARGO_MANIFEST_DIR"), "exarmo-aarch64-capi");
}
