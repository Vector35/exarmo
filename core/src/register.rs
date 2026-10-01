//! Macros that declare register files.

/// Declares a one-byte register newtype holding a number below the count.
/// Given a prefix it also implements `Display`, writing the prefix and the
/// number, or the name listed for that number.
///
/// ```
/// exarmo_core::register!(
///     /// One of the sixteen general-purpose registers.
///     Reg, 16, "r", 13 => "sp", 14 => "lr", 15 => "pc"
/// );
/// assert_eq!(Reg::new(3).to_string(), "r3");
/// assert_eq!(Reg::new(15).to_string(), "pc");
/// ```
#[macro_export]
macro_rules! register {
    ($(#[$doc:meta])* $name:ident, $count:literal) => {
        $(#[$doc])*
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(u8);

        impl $name {
            /// The register a number names.
            #[inline]
            pub const fn new(num: u8) -> Self {
                debug_assert!(num < $count);
                Self(num)
            }

            /// The register a number names, or nothing where the file has no
            /// register of that number.
            #[inline]
            pub const fn try_new(num: u8) -> Option<Self> {
                match num < $count {
                    true => Some(Self(num)),
                    false => None,
                }
            }

            /// The register number.
            #[inline]
            pub const fn num(self) -> u8 {
                self.0
            }
        }

        impl From<u8> for $name {
            #[inline]
            fn from(num: u8) -> Self {
                Self::new(num)
            }
        }
    };
    ($(#[$doc:meta])* $name:ident, $count:literal, $prefix:literal $(, $num:literal => $written:literal)*) => {
        $crate::register!($(#[$doc])* $name, $count);

        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                match self.0 {
                    $($num => f.write_str($written),)*
                    n => write!(f, concat!($prefix, "{}"), n),
                }
            }
        }
    };
}

/// Declares an enum over several register files, each variant holding a
/// register of one file.
///
/// `num` and `Display` delegate to the register held. A variant in the
/// `named` block holds no register and is written as the text it is given.
///
/// ```
/// exarmo_core::register!(
///     /// A 32-bit general-purpose register.
///     WReg, 32, "w"
/// );
/// exarmo_core::register!(
///     /// A 64-bit general-purpose register.
///     XReg, 32, "x"
/// );
/// exarmo_core::register_enum!(
///     /// A general-purpose register of either width.
///     Gp {
///         /// 32-bit.
///         W(WReg),
///         /// 64-bit.
///         X(XReg),
///     }
/// );
/// assert_eq!(Gp::X(XReg::new(3)).num(), 3);
/// assert_eq!(Gp::W(WReg::new(3)).to_string(), "w3");
/// ```
#[macro_export]
macro_rules! register_enum {
    (
        $(#[$doc:meta])* $name:ident {
            $($(#[$vdoc:meta])* $variant:ident($file:ty),)*
        }
        $(named {
            $($(#[$ndoc:meta])* $named:ident => $written:literal,)*
        })?
    ) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum $name {
            $($(#[$vdoc])* $variant($file),)*
            $($($(#[$ndoc])* $named,)*)?
        }

        impl $name {
            /// The register's number in its file, and zero for one the enum
            /// names outright.
            #[inline]
            pub const fn num(self) -> u8 {
                match self {
                    $(Self::$variant(r) => r.num(),)*
                    $($(Self::$named => 0,)*)?
                }
            }
        }

        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                match self {
                    $(Self::$variant(r) => r.fmt(f),)*
                    $($(Self::$named => f.write_str($written),)*)?
                }
            }
        }
    };
}
