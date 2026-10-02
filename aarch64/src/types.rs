//! Register, arrangement and system register types.
//!
//! A general-purpose register's type carries its width and whether register
//! 31 is the zero register or the stack pointer, so its value is a
//! transparent `u8` holding only the number.

use core::fmt;

use exarmo_core::{register, register_enum};

register!(
    /// A 32-bit W register where register 31 is WZR, the zero register.
    WRegZr, 32, "w", 31 => "wzr"
);

register!(
    /// A 32-bit W register where register 31 is WSP, the stack pointer.
    WRegSp, 32, "w", 31 => "wsp"
);

register!(
    /// A 64-bit X register where register 31 is XZR, the zero register.
    XRegZr, 32, "x", 29 => "fp", 30 => "lr", 31 => "xzr"
);

register!(
    /// A 64-bit X register where register 31 is SP, the stack pointer.
    XRegSp, 32, "x", 29 => "fp", 30 => "lr", 31 => "sp"
);

register!(
    /// A general-purpose register whose width the instruction does not fix.
    GpReg, 32, "r"
);

register_enum!(
    /// A general-purpose register whose width another field chooses, such as
    /// TBZ's `b5`, with register 31 the zero register.
    DynRegZr {
        /// 32-bit W register (reg 31 = WZR).
        W(WRegZr),
        /// 64-bit X register (reg 31 = XZR).
        X(XRegZr),
    }
);

impl DynRegZr {
    /// Whether this is an X register.
    #[inline]
    pub const fn is_64(self) -> bool {
        matches!(self, Self::X(_))
    }

    /// Whether this is WZR or XZR.
    #[inline]
    pub const fn is_zr(self) -> bool {
        match self {
            Self::W(r) => r.is_zr(),
            Self::X(r) => r.is_zr(),
        }
    }
}

impl From<WRegZr> for DynRegZr {
    #[inline]
    fn from(r: WRegZr) -> Self {
        Self::W(r)
    }
}

impl From<XRegZr> for DynRegZr {
    #[inline]
    fn from(r: XRegZr) -> Self {
        Self::X(r)
    }
}

/// How many lanes of what width a vector operand holds.
///
/// The architecture writes this as an *arrangement specifier*, where `4S` is
/// four 32-bit lanes and `16B` sixteen 8-bit lanes. Where the assembly names a
/// single element rather than a vector, as `<V>` does in
/// `DUP <Vd>.<T>, <Vn>.<Ts>[<index>]`, it writes the width letter alone, with
/// no lane count.
///
/// Some 1,300 operand value tables in the ISA are written over a closed set of
/// fifteen of these symbols, so the symbol says what it means without a table
/// per encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Arrangement {
    lanes: Option<u8>,
    element: ElementWidth,
}

/// The width of one lane of a vector, as the architecture letters it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ElementWidth {
    /// Byte, 8 bits.
    B,
    /// Halfword, 16 bits.
    H,
    /// Single word, 32 bits.
    S,
    /// Doubleword, 64 bits.
    D,
    /// Quadword, 128 bits.
    Q,
}

impl ElementWidth {
    /// The width in bits.
    #[inline]
    pub const fn bits(self) -> u16 {
        match self {
            Self::B => 8,
            Self::H => 16,
            Self::S => 32,
            Self::D => 64,
            Self::Q => 128,
        }
    }
}

impl fmt::Display for ElementWidth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::B => "b",
            Self::H => "h",
            Self::S => "s",
            Self::D => "d",
            Self::Q => "q",
        })
    }
}

impl Arrangement {
    /// A vector of `lanes` lanes of `element`.
    #[inline]
    pub const fn vector(lanes: u8, element: ElementWidth) -> Self {
        Self {
            lanes: Some(lanes),
            element,
        }
    }

    /// One element of `element`, where the assembly names no lane count.
    #[inline]
    pub const fn element(element: ElementWidth) -> Self {
        Self {
            lanes: None,
            element,
        }
    }

    /// How many lanes, where the assembly says.
    #[inline]
    pub const fn lanes(self) -> Option<u8> {
        self.lanes
    }

    /// The width of one lane.
    #[inline]
    pub const fn element_width(self) -> ElementWidth {
        self.element
    }

    /// The width of the whole operand in bits, where the lane count is written.
    #[inline]
    pub const fn bits(self) -> Option<u16> {
        match self.lanes {
            Some(lanes) => Some(lanes as u16 * self.element.bits()),
            None => None,
        }
    }
}

impl fmt::Display for Arrangement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.lanes {
            Some(lanes) => write!(f, "{lanes}{}", self.element),
            None => write!(f, "{}", self.element),
        }
    }
}

#[cfg(test)]
mod arrangement_tests {
    use super::*;
    use alloc::string::ToString;

    /// The fifteen symbols the ISA's arrangement tables are written in.
    #[test]
    fn renders_the_architecture_vocabulary() {
        let cases = [
            (Arrangement::element(ElementWidth::B), "b", None),
            (Arrangement::element(ElementWidth::H), "h", None),
            (Arrangement::element(ElementWidth::S), "s", None),
            (Arrangement::element(ElementWidth::D), "d", None),
            (Arrangement::element(ElementWidth::Q), "q", None),
            (Arrangement::vector(8, ElementWidth::B), "8b", Some(64)),
            (Arrangement::vector(16, ElementWidth::B), "16b", Some(128)),
            (Arrangement::vector(2, ElementWidth::H), "2h", Some(32)),
            (Arrangement::vector(4, ElementWidth::H), "4h", Some(64)),
            (Arrangement::vector(8, ElementWidth::H), "8h", Some(128)),
            (Arrangement::vector(2, ElementWidth::S), "2s", Some(64)),
            (Arrangement::vector(4, ElementWidth::S), "4s", Some(128)),
            (Arrangement::vector(1, ElementWidth::D), "1d", Some(64)),
            (Arrangement::vector(2, ElementWidth::D), "2d", Some(128)),
            (Arrangement::vector(1, ElementWidth::Q), "1q", Some(128)),
        ];
        for (arrangement, written, bits) in cases {
            assert_eq!(arrangement.to_string(), written);
            assert_eq!(arrangement.bits(), bits, "{written}");
        }
    }
}

/// The set of ZA tiles an instruction names, one per bit.
///
/// ZERO writes its operand as `{ {<mask>} }`, "the optional list of up to eight
/// 64-bit element tile names separated by commas, encoded in the imm8 field",
/// and the decode fixes the element size at 64, so bit 0 is `za0.d`.
///
/// A wider tile is the same bits under one name. `za<n>.s` is the pair n and
/// n+4, `za<n>.h` is n, n+2, n+4 and n+6, and `za` is all eight. The
/// architecture asks for the shortest list that names the mask, so the widest
/// names are taken first. An all-ones mask is `za`, and the four tiles 0, 2, 4
/// and 6 are `za0.h` rather than four separate names. An empty mask names
/// nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ZaTileMask(pub u8);

impl ZaTileMask {
    /// Every tile name with the bits it covers, widest first so that taking
    /// each one that fits leaves the shortest list.
    fn candidates() -> impl Iterator<Item = (ZaTileName, u8)> {
        let whole = core::iter::once((ZaTileName { n: 0, size: None }, 0xff));
        let tiles = |size, count, covered: u8| {
            (0..count).map(move |n| {
                (
                    ZaTileName {
                        n,
                        size: Some(size),
                    },
                    covered << n,
                )
            })
        };
        whole
            .chain(tiles('h', 2, 0x55))
            .chain(tiles('s', 4, 0x11))
            .chain(tiles('d', 8, 0x01))
    }

    /// The shortest list of tile names covering the mask.
    ///
    /// Written without allocating, since the C face renders this into a
    /// caller's buffer and promises to allocate nothing.
    pub fn tiles(self) -> impl Iterator<Item = ZaTileName> {
        let mut left = self.0;
        Self::candidates()
            .filter(move |&(_, covered)| {
                let fits = left & covered == covered;
                if fits {
                    left &= !covered;
                }
                fits
            })
            .map(|(name, _)| name)
    }
}

/// One tile's name as ZERO writes it: `za`, or `za<n>.<size>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ZaTileName {
    n: u8,
    size: Option<char>,
}

impl fmt::Display for ZaTileName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.size {
            Some(size) => write!(f, "za{}.{size}", self.n),
            None => f.write_str("za"),
        }
    }
}

impl fmt::Display for ZaTileMask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (written, tile) in self.tiles().enumerate() {
            if written > 0 {
                f.write_str(", ")?;
            }
            tile.fmt(f)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod za_tile_mask_tests {
    use super::ZaTileMask;
    use alloc::string::ToString;

    /// The cases the architecture gives for ZERO's preferred disassembly.
    #[test]
    fn shortest_list_of_tile_names() {
        assert_eq!(ZaTileMask(0b0000_0000).to_string(), "");
        assert_eq!(ZaTileMask(0b1111_1111).to_string(), "za");
        // ZA0.D, ZA1.D, ZA4.D and ZA5.D are ZA0.S and ZA1.S.
        assert_eq!(ZaTileMask(0b0011_0011).to_string(), "za0.s, za1.s");
        // ZA0.D, ZA2.D, ZA4.D and ZA6.D are ZA0.H.
        assert_eq!(ZaTileMask(0b0101_0101).to_string(), "za0.h");
        assert_eq!(ZaTileMask(0b0000_0010).to_string(), "za1.d");
        assert_eq!(ZaTileMask(0b0000_1001).to_string(), "za0.d, za3.d");
    }
}

register_enum!(
    /// A general-purpose register whose width another field chooses, with
    /// register 31 the stack pointer. The assembly writes it `<R><n|SP>`, as
    /// SVE's DUP and CPY do.
    DynRegSp {
        /// 32-bit W register (reg 31 = WSP).
        W(WRegSp),
        /// 64-bit X register (reg 31 = SP).
        X(XRegSp),
    }
);

impl DynRegSp {
    /// Whether this is an X register.
    #[inline]
    pub const fn is_64(self) -> bool {
        matches!(self, Self::X(_))
    }

    /// Whether this is WSP or SP.
    #[inline]
    pub const fn is_sp(self) -> bool {
        match self {
            Self::W(r) => r.is_sp(),
            Self::X(r) => r.is_sp(),
        }
    }
}

impl From<WRegSp> for DynRegSp {
    #[inline]
    fn from(r: WRegSp) -> Self {
        Self::W(r)
    }
}

impl From<XRegSp> for DynRegSp {
    #[inline]
    fn from(r: XRegSp) -> Self {
        Self::X(r)
    }
}

impl WRegZr {
    /// Whether this is the zero register, WZR.
    #[inline]
    pub const fn is_zr(self) -> bool {
        self.0 == 31
    }
}

impl WRegSp {
    /// Whether this is the stack pointer, WSP.
    #[inline]
    pub const fn is_sp(self) -> bool {
        self.0 == 31
    }
}

impl XRegZr {
    /// Whether this is the zero register, XZR.
    #[inline]
    pub const fn is_zr(self) -> bool {
        self.0 == 31
    }
}

impl XRegSp {
    /// Whether this is the stack pointer, SP.
    #[inline]
    pub const fn is_sp(self) -> bool {
        self.0 == 31
    }
}

impl GpReg {
    /// Whether this is register 31, ZR when used as a source.
    #[inline]
    pub const fn is_zr(self) -> bool {
        self.0 == 31
    }
}

impl From<GpReg> for WRegZr {
    #[inline]
    fn from(gp: GpReg) -> Self {
        Self::new(gp.num())
    }
}

impl From<GpReg> for WRegSp {
    #[inline]
    fn from(gp: GpReg) -> Self {
        Self::new(gp.num())
    }
}

impl From<GpReg> for XRegZr {
    #[inline]
    fn from(gp: GpReg) -> Self {
        Self::new(gp.num())
    }
}

impl From<GpReg> for XRegSp {
    #[inline]
    fn from(gp: GpReg) -> Self {
        Self::new(gp.num())
    }
}

register!(
    /// 8-bit SIMD/FP scalar register (b0-b31).
    BReg, 32, "b"
);

register!(
    /// 16-bit SIMD/FP scalar register (h0-h31).
    HReg, 32, "h"
);

register!(
    /// 32-bit SIMD/FP scalar register (s0-s31).
    SReg, 32, "s"
);

register!(
    /// 64-bit SIMD/FP scalar register (d0-d31).
    DReg, 32, "d"
);

register!(
    /// 128-bit SIMD/FP vector register (q0-q31).
    QReg, 32, "q"
);

register_enum!(
    /// A scalar SIMD and floating-point register whose width a size field
    /// chooses, as a template's `<V><d>` writes it.
    DynScalarSimd {
        /// 8-bit scalar (b0-b31)
        B(BReg),
        /// 16-bit scalar (h0-h31)
        H(HReg),
        /// 32-bit scalar (s0-s31)
        S(SReg),
        /// 64-bit scalar (d0-d31)
        D(DReg),
        /// 128-bit scalar (q0-q31)
        Q(QReg),
    }
);

impl DynScalarSimd {
    /// Create from register number and size in bits.
    #[inline]
    pub fn from_bits(num: u8, size_bits: u8) -> Self {
        match size_bits {
            8 => Self::B(BReg::new(num)),
            16 => Self::H(HReg::new(num)),
            32 => Self::S(SReg::new(num)),
            64 => Self::D(DReg::new(num)),
            128 => Self::Q(QReg::new(num)),
            _ => panic!("invalid scalar SIMD size: {}", size_bits),
        }
    }

    /// Get the element size in bits.
    #[inline]
    pub const fn size_bits(self) -> u8 {
        match self {
            Self::B(_) => 8,
            Self::H(_) => 16,
            Self::S(_) => 32,
            Self::D(_) => 64,
            Self::Q(_) => 128,
        }
    }
}

register!(
    /// 128-bit SIMD vector register (v0-v31).
    VReg, 32, "v"
);

register!(
    /// SVE scalable vector register (z0-z31).
    ///
    /// Its width is implementation defined, 128 to 2048 bits in 128-bit
    /// steps.
    ZReg, 32, "z"
);

register!(
    /// SVE predicate register (p0-p15).
    PReg, 16, "p"
);

register!(
    /// SVE predicate-as-counter register (pn0-pn15).
    ///
    /// The same registers as P0 to P15, written `pn` where an instruction
    /// reads one as a counter.
    PNReg, 16, "pn"
);

register!(
    /// SME ZA tile register (za0-za15).
    ///
    /// How many tiles there are depends on the element size:
    /// - Byte (B): ZA0 only (1 tile, 0 bits to encode)
    /// - Halfword (H): ZA0-ZA1 (2 tiles, 1 bit)
    /// - Word (S): ZA0-ZA3 (4 tiles, 2 bits)
    /// - Doubleword (D): ZA0-ZA7 (8 tiles, 3 bits)
    /// - Quadword (Q): ZA0-ZA15 (16 tiles, 4 bits)
    ZATile, 16, "za"
);

/// A system register operand of MRS/MSR, holding its `op0:op1:CRn:CRm:op2`
/// encoding and the direction of the access.
///
/// Displays the architectural name where the encoding has one, and the generic
/// `s<op0>_<op1>_c<Cn>_c<Cm>_<op2>` form where it does not.
///
/// The direction is part of naming the register. Two registers can share an
/// encoding when one is read-only and the other write-only, as DBGDTRRX_EL0
/// and DBGDTRTX_EL0 do. A register accessed in a direction it does not
/// support has no name, only the generic form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SysReg {
    encoding: u16,
    write: bool,
}

impl SysReg {
    /// Wrap an `op0:op1:CRn:CRm:op2` encoding read by MRS.
    pub const fn read(encoding: u16) -> Self {
        Self {
            encoding,
            write: false,
        }
    }

    /// Wrap an `op0:op1:CRn:CRm:op2` encoding written by MSR.
    pub const fn write(encoding: u16) -> Self {
        Self {
            encoding,
            write: true,
        }
    }

    /// The `op0:op1:CRn:CRm:op2` encoding, `op0` as the two bits it is.
    ///
    /// The whole system instruction space is numbered this way, so a
    /// register and a [`SysOp`](crate::SysOp), whose `op0` is 0 or 1,
    /// cannot come to the same number.
    pub const fn encoding(self) -> u16 {
        self.encoding
    }

    /// Whether this access writes the register.
    pub const fn is_write(self) -> bool {
        self.write
    }

    /// The `op0` field, always 2 or 3, since MRS and MSR encode only its low
    /// bit.
    pub const fn op0(self) -> u8 {
        ((self.encoding >> 14) & 0x3) as u8
    }

    /// The `op1` field.
    pub const fn op1(self) -> u8 {
        ((self.encoding >> 11) & 0x7) as u8
    }

    /// The `CRn` field.
    pub const fn crn(self) -> u8 {
        ((self.encoding >> 7) & 0xf) as u8
    }

    /// The `CRm` field.
    pub const fn crm(self) -> u8 {
        ((self.encoding >> 3) & 0xf) as u8
    }

    /// The `op2` field.
    pub const fn op2(self) -> u8 {
        (self.encoding & 0x7) as u8
    }

    /// The architectural name of this register, if it has one.
    pub fn name(self) -> Option<&'static str> {
        crate::generated::mrs_msr_name(self.encoding, self.write)
    }

    /// Every system register the architecture names, in encoding order. A
    /// register with the same name in both directions appears once, and one
    /// with a different name in each direction, such as DBGDTRRX_EL0 and
    /// DBGDTRTX_EL0, once per name.
    pub fn all() -> impl Iterator<Item = SysRegDef> {
        (0..SysRegDef::COUNT).map(SysRegDef::at)
    }
}

/// A system register the architecture names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SysRegDef {
    /// The `op0:op1:CRn:CRm:op2` encoding, as [`SysReg::read`] and
    /// [`SysReg::write`] take it, `op0` as the two bits it is.
    pub encoding: u16,
    /// Whether MRS can read it under this name.
    pub readable: bool,
    /// Whether MSR can write it under this name.
    pub writable: bool,
    /// The name, in lower case.
    pub name: &'static str,
}

impl SysRegDef {
    /// How many registers the architecture names.
    pub const COUNT: usize = crate::generated::sysreg_names::COUNT;

    /// The register at `index` of [`SysReg::all`], which must be below
    /// [`SysRegDef::COUNT`].
    ///
    /// A `const fn` so that a C face can lay the whole table out as a
    /// constant.
    pub const fn at(index: usize) -> SysRegDef {
        use crate::generated::sysreg_names;
        let (encoding, access, name) = sysreg_names::entry(index);
        SysRegDef {
            encoding,
            readable: access & sysreg_names::READ != 0,
            writable: access & sysreg_names::WRITE != 0,
            name,
        }
    }
}

impl fmt::Display for SysReg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => f.write_str(name),
            None => write!(
                f,
                "s{}_{}_c{}_c{}_{}",
                self.op0(),
                self.op1(),
                self.crn(),
                self.crm(),
                self.op2()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_zr_sp() {
        assert!(!WRegZr::new(0).is_zr());
        assert!(WRegZr::new(31).is_zr());
        assert!(!WRegSp::new(0).is_sp());
        assert!(WRegSp::new(31).is_sp());
        assert!(!XRegZr::new(0).is_zr());
        assert!(XRegZr::new(31).is_zr());
        assert!(!XRegSp::new(0).is_sp());
        assert!(XRegSp::new(31).is_sp());
    }

    #[test]
    fn test_size_of_types() {
        assert_eq!(core::mem::size_of::<WRegZr>(), 1);
        assert_eq!(core::mem::size_of::<WRegSp>(), 1);
        assert_eq!(core::mem::size_of::<XRegZr>(), 1);
        assert_eq!(core::mem::size_of::<XRegSp>(), 1);
    }
}
