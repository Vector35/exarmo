//! Compiling and running a C API's smoke test.
//!
//! Each C face has a `tests/c/smoke.c` that must compile against the
//! hand-written header and link against the static library, with no
//! warnings.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::path::PathBuf;
use std::process::Command;

use exarmo_core::branch::Branch;
use exarmo_core::capi::{self, TextSize};
use exarmo_core::flags::FlagEffect;
use exarmo_core::tokens::{Token, TokenKind};

/// Compile `tests/c/smoke.c` of the crate against its `include/` and the
/// static library Cargo built for this test run, and run it.
///
/// `manifest` is the crate's own `CARGO_MANIFEST_DIR`, and `crate_name` its
/// name as Cargo knows it. The library's file name and the compiled
/// program's follow from it. Panics with what the compiler or the program
/// said, so a failure reads as the C toolchain wrote it.
///
/// Needs `cc` on the path.
pub fn c_smoke(manifest: &str, crate_name: &str) {
    let manifest = PathBuf::from(manifest);
    // The test binary is in target/<profile>/deps, and the library one up.
    let profile_dir = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    // A test run builds the crate only as an rlib. The static library must
    // be asked for, in this run's profile.
    let mut build = Command::new(env!("CARGO"));
    build
        .args(["build", "-p", crate_name, "--lib", "--target-dir"])
        .arg(profile_dir.parent().unwrap());
    if !cfg!(debug_assertions) {
        build.arg("--release");
    }
    let built = build.output().expect("cargo");
    assert!(
        built.status.success(),
        "building the static library:\n{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let library = profile_dir.join(format!("lib{}.a", crate_name.replace('-', "_")));
    assert!(library.exists(), "{} was not built", library.display());
    let exe = profile_dir.join(format!("{crate_name}-smoke"));
    let compile = Command::new("cc")
        .arg("-std=c11")
        .arg("-Wall")
        .arg("-Wextra")
        .arg("-Werror")
        .arg("-I")
        .arg(manifest.join("include"))
        .arg(manifest.join("tests/c/smoke.c"))
        .arg(&library)
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("a C compiler");
    assert!(
        compile.status.success(),
        "compiling smoke.c:\n{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let run = Command::new(&exe).output().unwrap();
    assert!(
        run.status.success(),
        "running smoke:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
}

/// Every number a generated header states, in the order it states them: each
/// member of an enum, and each define that is a number.
///
/// A count among them fixes how long an array a caller walks is, so a test
/// holds every one to the library rather than to what it expects.
pub fn header_numbers(header: &str) -> Vec<(String, usize)> {
    // A member written without a value is one more than the member before
    // it, as C numbers it.
    let mut next = None;
    let mut numbers = Vec::new();
    for line in header.lines() {
        let line = line.trim().trim_end_matches(',');
        if line.starts_with("typedef enum ") && line.ends_with('{') {
            next = Some(0);
            continue;
        }
        if line.starts_with('}') {
            next = None;
            continue;
        }
        // The provenance defines are strings, not numbers.
        let numbered = match line.strip_prefix("#define ") {
            Some(define) => define.split_once(' '),
            None => line.split_once(" = "),
        };
        let (name, value) = match numbered {
            Some((name, value)) => match value.trim().parse() {
                Ok(value) => (name, value),
                // A member written as an expression, `1 << 3`, is not read,
                // and so neither is any member after it written without a
                // value.
                Err(_) => {
                    if !line.starts_with('#') {
                        next = None;
                    }
                    continue;
                }
            },
            None => match next {
                Some(value) if line.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') => {
                    if line.is_empty() {
                        continue;
                    }
                    (line, value)
                }
                // A comment is no member. Any other line of an enum that
                // is not a bare name, a member with a comment after it say,
                // ends the numbering as an expression does, since the
                // members after it cannot be counted from a line not read.
                Some(_) => {
                    if !(line.starts_with("/*") && line.ends_with("*/")) {
                        next = None;
                    }
                    continue;
                }
                None => continue,
            },
        };
        if next.is_some() && !line.starts_with('#') {
            next = Some(value + 1);
        }
        numbers.push((name.to_string(), value));
    }
    numbers
}

/// The system allocator, counting what the thread asks of it while
/// [`allocations`] runs. A C face's corpus test installs it as the test's
/// global allocator, since a library cannot. A panic's payload is allocated
/// too, so an entry point that panics and reports a failure instead is
/// counted as well.
pub struct Counting;

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.with(Cell::get) {
            ALLOCATIONS.with(|n| n.set(n.get() + 1));
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if COUNTING.with(Cell::get) {
            ALLOCATIONS.with(|n| n.set(n.get() + 1));
        }
        unsafe { System.realloc(ptr, layout, size) }
    }
}

/// Run `f`, and say how many times it asked [`Counting`] for memory, with
/// what it returned.
pub fn allocations<T>(f: impl FnOnce() -> T) -> (usize, T) {
    ALLOCATIONS.with(|n| n.set(0));
    COUNTING.with(|c| c.set(true));
    let value = f();
    COUNTING.with(|c| c.set(false));
    (ALLOCATIONS.with(Cell::get), value)
}

/// A token kind's number, by the name the hand-written header gives it
/// after `prefix`, such as `EXARMO_AARCH64_TOKEN_`.
pub fn token_kinds(header: &str, prefix: &str) -> Vec<(String, u32)> {
    header_numbers(header)
        .into_iter()
        .filter_map(|(name, value)| {
            let kind = name.strip_prefix(prefix)?;
            (kind != "NO_OPERAND").then(|| (kind.to_string(), value as u32))
        })
        .collect()
}

/// Where the text and tokens a C caller read disagree with the Rust ones.
/// Each C token is read back by its span of the text and its kind by the
/// header's numbering, `kinds`, rather than by the conversion that wrote
/// it. `text` is the buffer the tokens were written into, `size` what the
/// tokens call returned and `text_length` what the text call alone did.
pub fn text_differences(
    kinds: &[(String, u32)],
    written: &str,
    rust: &[Token],
    text: &[u8],
    size: TextSize,
    text_length: usize,
    tokens: &[capi::Token],
) -> Vec<String> {
    let number = |kind: TokenKind| {
        let named = crate::invariant::kind_name(kind).to_uppercase();
        kinds.iter().find(|(k, _)| *k == named).map(|(_, n)| *n)
    };
    if (size.length, size.tokens, text_length) != (written.len(), rust.len(), written.len()) {
        return vec![format!(
            "the text is {} long in {} tokens in C, and {text_length} as the text alone",
            size.length, size.tokens
        )];
    }
    if &text[..written.len()] != written.as_bytes() || text[written.len()] != 0 {
        return vec![format!(
            "the text is {:?} in C",
            String::from_utf8_lossy(&text[..written.len()])
        )];
    }
    let mut out = Vec::new();
    let mut end = 0;
    for (index, (c, r)) in tokens.iter().zip(rust).enumerate() {
        let span = &written.as_bytes()[(c.offset as usize).min(written.len())
            ..((c.offset + c.length) as usize).min(written.len())];
        let value = match r.kind {
            TokenKind::Immediate(value) => value as u64,
            TokenKind::Integer(value) | TokenKind::Address(value) => value,
            _ => 0,
        };
        let operand = r.operand.unwrap_or(capi::Token::NO_OPERAND);
        if Some(c.kind) != number(r.kind)
            || c.offset as usize != end
            || span != r.text.as_bytes()
            || c.operand != operand
            || c.value != value
        {
            out.push(format!(
                "token {index} is kind {} operand {} value {:#x} at {}..{} in C, and {:?} operand {operand} {:?} at {end} in Rust",
                c.kind,
                c.operand,
                c.value,
                c.offset,
                c.offset + c.length,
                r.kind,
                r.text
            ));
        }
        end += r.text.len();
    }
    out
}

/// Where what C reads of the flags and the flow of control disagrees with
/// Rust, compared field by field.
pub fn effect_differences(
    c_flags: capi::FlagEffect,
    flags: FlagEffect,
    c_branch: capi::Branch,
    branch: Branch,
) -> Vec<String> {
    let mut out = Vec::new();
    if (c_flags.writes, c_flags.reads, c_flags.float_compare)
        != (flags.writes.0, flags.reads.0, flags.float_compare)
    {
        out.push(format!(
            "the flags are {c_flags:?} in C and {flags:?} in Rust"
        ));
    }
    if (
        c_branch.kind,
        c_branch.conditional,
        c_branch.has_target.then_some(c_branch.target),
    ) != (branch.kind as u8, branch.conditional, branch.target)
        || (!c_branch.has_target && c_branch.target != 0)
    {
        out.push(format!(
            "the branch is {c_branch:?} in C and {branch:?} in Rust"
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_enum_numbers_its_members_as_c_does() {
        let header = "typedef enum e {\n    E_A,\n    /* `b` */\n    E_B = 5,\n\n    E_C,\n    E_D = 1 << 3,\n    E_E,\n} e;\n#define E_COUNT 3\n";
        let numbers = header_numbers(header);
        assert_eq!(
            numbers,
            [("E_A", 0), ("E_B", 5), ("E_C", 6), ("E_COUNT", 3)]
                .map(|(name, value)| (name.to_string(), value))
        );
    }

    #[test]
    fn a_member_that_is_not_a_bare_name_ends_the_numbering() {
        let header = "typedef enum e {\n    E_A,\n    E_B, /* note */\n    E_C,\n} e;\n";
        let numbers = header_numbers(header);
        assert_eq!(numbers, [("E_A".to_string(), 0)]);
    }
}
