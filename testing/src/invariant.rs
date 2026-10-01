//! Checks held over a whole corpus, and how their failures are reported.
//!
//! An invariant is checked case by case, and a failing one lists every case
//! that broke it rather than stopping at the first, so a run says
//! everything there is to act on at once. What each check reads is the
//! token stream both runtimes write, which is why the checks that read
//! only tokens are shared here.

use exarmo_core::branch::Branch;
use exarmo_core::tokens::{Token, TokenKind};

/// Fail listing every case that broke an invariant, each on its own line.
pub fn report(invariant: &str, checked: usize, failures: &[String]) {
    assert!(checked > 0, "the corpus holds no case {invariant}");
    println!("{checked} cases checked that {invariant}");
    assert!(
        failures.is_empty(),
        "{} of {checked} cases break the invariant that {invariant}\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// What [`unpredictable_where_broken`] checks, as [`report`] names it.
pub const UNPREDICTABLE_WHERE_BROKEN: &str = "each case breaking a bit its block's header writes \
     (0) or (1) is CONSTRAINED UNPREDICTABLE";

/// Where a case of `block` breaks a bit the header writes `(0)` or `(1)`
/// and is not CONSTRAINED UNPREDICTABLE.
pub fn unpredictable_where_broken(
    block: &crate::blocks::Block,
    word: u32,
    instruction: &impl exarmo_core::Decoded,
) -> Option<String> {
    let (mask, value) = block.should_be();
    (word & mask != value && !instruction.unpredictable())
        .then(|| format!("{} {word:08X} is not unpredictable", block.encoding))
}

/// Where a branch goes somewhere other than its label says, given the
/// labels the instruction holds, each as the operand it is and the address
/// it names, and the tokens it writes. A branch that names its target has
/// one label, naming that target, and writes it. One that names none has no
/// target. Every address written is a label's, at the address it names,
/// and every label is written, whether it is a target or not.
pub fn branch_goes_where_labelled(
    branch: Branch,
    labels: &[(usize, u64)],
    tokens: &[Token],
) -> Vec<String> {
    let addresses: Vec<(Option<u8>, u64)> = tokens
        .iter()
        .filter_map(|token| match token.kind {
            TokenKind::Address(address) => Some((token.operand, address)),
            _ => None,
        })
        .collect();
    let mut failures = Vec::new();
    match (branch.kind.names_target(), branch.target) {
        (true, Some(target)) => {
            if labels.len() != 1 {
                failures.push(format!(
                    "{:?} names {target:#x} with {} labels",
                    branch.kind,
                    labels.len()
                ));
            } else if labels[0].1 != target {
                failures.push(format!(
                    "{:?} names {target:#x} and its label {:#x}",
                    branch.kind, labels[0].1
                ));
            }
            if !addresses.iter().any(|(_, address)| *address == target) {
                failures.push(format!(
                    "the target {target:#x} is written as none of {addresses:x?}"
                ));
            }
        }
        (true, None) => failures.push(format!("{:?} names no target", branch.kind)),
        (false, Some(target)) => failures.push(format!("{:?} names {target:#x}", branch.kind)),
        (false, None) => {}
    }
    for (operand, address) in &addresses {
        if !labels
            .iter()
            .any(|(index, named)| Some(*index as u8) == *operand && named == address)
        {
            failures.push(format!(
                "the address {address:#x} of operand {operand:?} is no label's among {labels:x?}"
            ));
        }
    }
    for (index, named) in labels {
        if !addresses.contains(&(Some(*index as u8), *named)) {
            failures.push(format!(
                "the label {named:#x} of operand {index} is not written"
            ));
        }
    }
    failures
}

/// A token kind's name, without the value it carries.
pub fn kind_name(kind: TokenKind) -> String {
    let kind = format!("{kind:?}");
    kind.split('(').next().unwrap_or_default().to_string()
}

/// Where the tokens written for one operand are of a kind it cannot hold,
/// or where none of them carries its value. `may` names the kinds it may be
/// written with and `must` those one of which carries its value. An
/// operand no token names is at its default and not written, which is no
/// failure.
pub fn kinds_written(tokens: &[Token], index: usize, may: &[&str], must: &[&str]) -> Vec<String> {
    let kinds: Vec<(String, &Token)> = tokens
        .iter()
        .filter(|token| token.operand == Some(index as u8))
        .map(|token| (kind_name(token.kind), token))
        .collect();
    if kinds.is_empty() {
        return Vec::new();
    }
    let mut failures = Vec::new();
    let wrong: Vec<String> = kinds
        .iter()
        .filter(|(kind, _)| !may.contains(&kind.as_str()) && !must.contains(&kind.as_str()))
        .map(|(_, token)| format!("{:?} {:?}", token.kind, token.text))
        .collect();
    if !wrong.is_empty() {
        failures.push(format!("is written with {}", wrong.join(", ")));
    }
    if !kinds.iter().any(|(kind, _)| must.contains(&kind.as_str())) {
        failures.push(format!("is written with no {}", must.join(" or ")));
    }
    failures
}
