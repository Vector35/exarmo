//! What an instruction does with the condition flags.
//!
//! Both instruction sets keep N, Z, C and V in PSTATE and write them from the
//! same Execute pseudocode, so one type serves both runtimes.

/// A set of the condition flags N, Z, C and V.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Flags(pub u8);

impl Flags {
    /// No flags.
    pub const NONE: Flags = Flags(0);
    /// The negative flag.
    pub const N: Flags = Flags(0b1000);
    /// The zero flag.
    pub const Z: Flags = Flags(0b0100);
    /// The carry flag.
    pub const C: Flags = Flags(0b0010);
    /// The overflow flag.
    pub const V: Flags = Flags(0b0001);
    /// All four.
    pub const NZCV: Flags = Flags(0b1111);

    /// Whether every flag in `other` is in this set.
    pub const fn contains(self, other: Flags) -> bool {
        self.0 & other.0 == other.0
    }

    /// Whether the set is empty.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The flags read to test the condition in a `cond` field. A condition
    /// that always holds reads none.
    pub const fn for_condition(bits: u8) -> Flags {
        match crate::condition::always_holds(bits) {
            true => Flags::NONE,
            false => Flags::NZCV,
        }
    }
}

impl core::ops::BitOr for Flags {
    type Output = Flags;
    fn bitor(self, other: Flags) -> Flags {
        Flags(self.0 | other.0)
    }
}

/// What an instruction does with the condition flags, read from its
/// Execute pseudocode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FlagEffect {
    /// The flags the instruction writes.
    pub writes: Flags,
    /// The flags the instruction reads. This is all four for a condition, or
    /// the carry alone.
    pub reads: Flags,
    /// Whether the flags written come from comparing floating-point values,
    /// which sets them differently from an integer comparison.
    pub float_compare: bool,
}

impl FlagEffect {
    /// Touches no flag.
    pub const NONE: FlagEffect = FlagEffect {
        writes: Flags::NONE,
        reads: Flags::NONE,
        float_compare: false,
    };
}
