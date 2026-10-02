//! Whether a condition code can fail.
//!
//! An instruction written with a condition that cannot fail reads no flags
//! and always takes its branch, so this is what tells a conditional encoding
//! from a conditional instruction. Both instruction sets number the
//! conditions the same way.

/// Whether a condition holds whatever the flags are.
///
/// ARM's `ConditionHolds` reads `cond[3:1] == '111'` as TRUE and then
/// exempts `'1111'` from the inversion that `cond[0]` otherwise applies. So
/// AL at 14 and A64's NV at 15 both hold always, and `b.nv` branches every
/// time rather than never.
pub const fn always_holds(bits: u8) -> bool {
    bits >> 1 == 0b111
}

#[cfg(test)]
mod tests {
    use super::always_holds;
    use alloc::vec;
    use alloc::vec::Vec;

    #[test]
    fn the_last_two_conditions_hold_always() {
        let always: Vec<u8> = (0..16).filter(|bits| always_holds(*bits)).collect();
        assert_eq!(always, vec![14, 15]);
    }
}
