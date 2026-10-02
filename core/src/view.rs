//! The operands of an instruction, as many as it has and no more.
//!
//! An instruction's operands are a short list, and a rendering reads them at
//! positions it knows at compile time. So they are held in an array as wide as
//! the widest instruction, with the length beside it, rather than allocated.
//! Each instruction set defines the operand type itself.

use core::fmt;

/// Whether and how a memory operand writes its base back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Writeback {
    /// The base is unchanged: `[r0, #4]`.
    None,
    /// The offset is added before the access and kept: `[r0, #4]!`.
    Pre,
    /// The access is at the base and the offset added after, which the
    /// assembly writes after the bracket: `[r0], #4`.
    Post,
}

/// A value from a value table, such as a shift, an extend, a barrier option
/// or a data type, as its bits and its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Symbol {
    /// The bits the encoding holds.
    pub bits: u16,
    /// The name the assembly writes, in lower case. It is empty for a value
    /// the table does not name.
    pub name: &'static str,
}

impl Symbol {
    /// A symbol the template writes itself, with no bits behind it.
    pub const fn named(name: &'static str) -> Self {
        Symbol { bits: 0, name }
    }
}

/// A number's value, whatever integer type a field holds it in.
///
/// An instruction holds a field in the narrowest type that fits it, while the
/// view writes every number as one signed type, so a projection asks for the
/// value rather than naming the type it is widening from.
pub trait IntValue {
    /// The value, as i64.
    fn value(self) -> i64;
}

macro_rules! int_value {
    ($($t:ty),*) => { $(impl IntValue for $t { #[inline] fn value(self) -> i64 { self as i64 } })* };
}
int_value!(u8, u16, u32, u64, i8, i16, i32, i64, bool);

/// A short list of operands, held inline.
///
/// `N` is the most any instruction of the set has, so a rendering can index the
/// whole array at a position the compiler checks.
#[derive(Clone, Copy, PartialEq)]
pub struct Operands<T, const N: usize> {
    items: [T; N],
    len: u8,
}

impl<T: Copy + Default, const N: usize> Default for Operands<T, N> {
    fn default() -> Self {
        Operands {
            items: [T::default(); N],
            len: 0,
        }
    }
}

impl<T: Copy + Default, const N: usize> Operands<T, N> {
    /// No operands.
    pub fn empty() -> Self {
        Self::default()
    }

    /// The operands as a slice.
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        &self.items[..self.len as usize]
    }

    /// The whole array, defaulted past the length. A rendering indexes it at
    /// constant positions, which are bounds checked at compile time.
    #[inline]
    pub fn as_array(&self) -> &[T; N] {
        &self.items
    }

    /// How many operands there are.
    #[inline]
    pub fn len(&self) -> usize {
        self.len as usize
    }

    /// Whether there are none.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Holds the operands of an array of at most `N`.
    #[inline]
    pub fn of<const M: usize>(operands: [T; M]) -> Self {
        const {
            assert!(M <= N, "more operands than any instruction has");
        }
        let mut out = Self::default();
        out.items[..M].copy_from_slice(&operands);
        out.len = M as u8;
        out
    }
}

impl<T: Copy + Default, const N: usize> core::ops::Deref for Operands<T, N> {
    type Target = [T];
    #[inline]
    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<'a, T: Copy + Default, const N: usize> IntoIterator for &'a Operands<T, N> {
    type Item = &'a T;
    type IntoIter = core::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

/// Printed as the operands there are, without the padding past the length.
impl<T: Copy + Default + fmt::Debug, const N: usize> fmt::Debug for Operands<T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_slice().fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::Operands;
    use alloc::format;

    #[test]
    fn it_holds_as_many_as_it_was_given() {
        let none: Operands<u8, 4> = Operands::empty();
        assert!(none.is_empty());
        assert_eq!(none.len(), 0);
        assert_eq!(none.as_slice(), &[] as &[u8]);

        let two: Operands<u8, 4> = Operands::of([7, 9]);
        assert_eq!(two.len(), 2);
        assert_eq!(two.as_slice(), &[7, 9]);
        assert_eq!(two.as_array(), &[7, 9, 0, 0]);
        assert_eq!(format!("{two:?}"), "[7, 9]");
    }

    #[test]
    fn it_holds_a_full_array() {
        let full: Operands<u8, 3> = Operands::of([1, 2, 3]);
        assert_eq!(full.len(), 3);
        assert_eq!(full.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn it_reads_as_a_slice() {
        let three: Operands<u8, 5> = Operands::of([4, 5, 6]);
        assert_eq!(three.first(), Some(&4));
        assert_eq!(three.iter().copied().sum::<u8>(), 15);
        assert_eq!((&three).into_iter().count(), 3);
    }
}
