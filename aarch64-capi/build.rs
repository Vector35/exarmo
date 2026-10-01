// Tells a dependent's build script where the C headers are, as
// DEP_EXARMO_AARCH64_INCLUDE, so C and C++ code built against this crate uses the
// headers of the library it links.
fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let include = std::path::Path::new(&manifest_dir).join("include");
    println!("cargo::metadata=include={}", include.display());
    println!("cargo::rerun-if-changed=build.rs");
}
