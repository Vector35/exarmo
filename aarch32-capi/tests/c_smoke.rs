//! Compiles `tests/c/smoke.c` against `include/` and the static library Cargo
//! built for this test run, and runs it.

#[test]
fn c_can_use_the_library() {
    exarmo_testing::c_smoke(env!("CARGO_MANIFEST_DIR"), "exarmo-aarch32-capi");
}
