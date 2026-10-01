//! The ACLE intrinsics an instruction realises.
//!
//! ARM's C Language Extensions name a C intrinsic for most Advanced SIMD
//! instructions, `vmla_lane_s16` for `MLA Vd.4H,Vn.4H,Vm.H[lane]`, and say
//! which register each argument reaches and where the result comes from.
//! [`Instruction::intrinsics`](crate::Instruction::intrinsics) gives the
//! intrinsics a decoded instruction is, with each argument bound to the
//! operand of [`Instruction::operands`](crate::Instruction::operands) that
//! holds it, so a decompiler can show the call a compiler would have
//! written. An instruction that is the same for signed and unsigned
//! elements, `ADD Vd.8B` for `vadd_s8` and `vadd_u8`, lists both, and a
//! consumer chooses.
//!
//! The table is generated from ACLE's intrinsic database, whose commit
//! [`PROVENANCE`](crate::PROVENANCE) records.

/// Every intrinsic the table names, once each, in name order. An index into
/// it is an intrinsic's identity, which [`Intrinsic::id`] holds, so a
/// consumer that keeps intrinsics by number, as Binary Ninja's plugin API
/// does, can use it directly. Because the order is by name, an index holds
/// only for the ACLE commit [`PROVENANCE`](crate::PROVENANCE) records.
pub static DEFS: &[IntrinsicDef] = crate::generated::intrinsics::INTRINSIC_DEFS.as_slice();

/// An intrinsic as ACLE declares it, apart from any instruction. Its C
/// signature is `result`, the name and `parameters`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntrinsicDef {
    /// The name, as in `vmla_lane_s16`. Unique, and what ACLE keys on.
    pub name: &'static str,
    /// The result's type.
    pub result: IntrinsicType,
    /// The parameters' types, in order.
    pub parameters: &'static [IntrinsicType],
}

/// What each [`IntrinsicType`] is, in the enum's order, so `TYPES[t as usize]`
/// describes `t`. [`IntrinsicType::def`] reads it.
pub static TYPES: &[IntrinsicTypeDef] = crate::generated::intrinsics::INTRINSIC_TYPES.as_slice();

pub use crate::generated::intrinsics::IntrinsicType;

/// What family of value a type holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TypeKind {
    /// Nothing, as a store returns.
    Void = 0,
    /// A signed integer, as in `int8x16_t` or `int64_t`.
    Int = 1,
    /// An unsigned integer, as in `uint8x16_t`.
    Uint = 2,
    /// An IEEE float, as in `float32x4_t`.
    Float = 3,
    /// A polynomial, as in `poly8x16_t`.
    Poly = 4,
    /// A 16-bit brain float, as in `bfloat16x8_t`.
    BFloat = 5,
    /// An 8-bit float, as in `mfloat8x16_t`.
    MFloat = 6,
    /// A value the intrinsic takes at compile time, `const int`, such as a
    /// lane index, whose bound the instruction's field sets.
    ConstInt = 7,
    /// The floating-point mode register's value, `fpm_t`.
    Fpm = 8,
}

/// A C type ACLE writes an intrinsic in, read apart so a consumer need not
/// parse the name. `int8x16_t` is a signed 8-bit element in 16 lanes, and
/// `float32x2x4_t` is four vectors of two 32-bit floats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntrinsicTypeDef {
    /// The name as ACLE writes it, as in `int8x16_t` or `int8_t const *`.
    pub name: &'static str,
    /// What family of value it holds.
    pub kind: TypeKind,
    /// How wide one element is, in bits, or zero for `void`.
    pub element_bits: u16,
    /// How many elements, 1 for a scalar.
    pub lanes: u8,
    /// How many such vectors. That is 2, 3 or 4 for a structure such as
    /// `int8x8x2_t`, which a multi-register load writes, and 1 otherwise.
    pub vectors: u8,
    /// Whether the argument is a pointer to that, as a load's `ptr` is.
    pub pointer: bool,
    /// Whether that pointer is to a constant, which a load's is and a
    /// store's is not.
    pub readonly: bool,
}

impl IntrinsicTypeDef {
    /// How wide the whole value is, in bits. An `int8x16_t` is 128, and a
    /// pointer's width is the pointee's.
    #[must_use]
    pub const fn bits(&self) -> u32 {
        self.element_bits as u32 * self.lanes as u32 * self.vectors as u32
    }
}

impl IntrinsicType {
    /// The type's name, kind and shape.
    #[must_use]
    pub fn def(self) -> &'static IntrinsicTypeDef {
        &TYPES[self as usize]
    }

    /// The name as ACLE writes it, as in `int8x16_t`.
    #[must_use]
    pub fn name(self) -> &'static str {
        self.def().name
    }
}

/// An intrinsic, and how a decoded instruction supplies it.
///
/// The intrinsic's name and types are in [`DEFS`]`[id as usize]`, once for
/// the whole architecture, and [`Self::def`] looks them up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Intrinsic {
    /// Which intrinsic, as an index into [`DEFS`].
    pub id: u32,
    /// Where the instruction holds each argument, in the order
    /// [`IntrinsicDef::parameters`] declares them.
    pub arguments: &'static [Source],
    /// Where the result comes from.
    pub result: Output,
}

impl Intrinsic {
    /// The intrinsic's name and types.
    #[must_use]
    pub fn def(&self) -> &'static IntrinsicDef {
        &DEFS[self.id as usize]
    }

    /// The name, as in `vmla_lane_s16`.
    #[must_use]
    pub fn name(&self) -> &'static str {
        self.def().name
    }
}

/// Where an instruction holds an argument, as an index into
/// [`Instruction::operands`](crate::Instruction::operands).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Source {
    /// The operand itself: a register, a register list for a structure of
    /// vectors, or the memory operand for a pointer.
    Operand(u8),
    /// The element index written on a register operand, `lane` in
    /// `Vm.H[lane]`.
    Index(u8),
    /// An immediate operand, and how far the argument was shifted to
    /// become it. `EXT`'s `#(n<<1)` holds `n` shifted by one.
    Immediate {
        /// The operand.
        operand: u8,
        /// The shift the instruction's immediate applied to the argument.
        shift: u8,
    },
    /// The floating-point mode register, FPMR, which the FP8 intrinsics
    /// take as their `fpm` argument and the instruction reads.
    Fpmr,
    /// The instruction does not carry the argument, whose only value the
    /// intrinsic fixes, as a lane index that can only be 0.
    Unused,
}

/// Where an intrinsic's result comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Output {
    /// None, as for a store.
    None,
    /// An operand: a register, or a register list for a structure of
    /// vectors.
    Operand(u8),
    /// One element of a structure of vectors, where the instruction writes
    /// that element alone, as `ZIP1` writes `vzip_s8`'s `result.val[0]`.
    Element {
        /// The operand.
        operand: u8,
        /// Which element.
        element: u8,
    },
}
