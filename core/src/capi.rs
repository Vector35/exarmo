//! What both runtimes' C libraries share: the caller's buffers, the token and
//! string types, and the guard that stops a panic crossing into C. Each
//! library adds its own operand model and entry points.

use core::fmt::{self, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::Decoded;
use crate::tokens::{OperandIndex, TokenKind, TokenSink};

/// The outcome of a decode, `<prefix>_status`.
///
/// One numbering for both faces. An AArch64 decode does not return the last
/// two, since the A64 index marks no row UNPREDICTABLE or a reserved hint.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// The instruction decoded.
    Ok = 0,
    /// The encoding is allocated to no instruction.
    Unallocated = 1,
    /// The instruction's decode says it is UNDEFINED.
    Undefined = 2,
    /// The instruction's decode says it executes as a NOP.
    Nop = 3,
    /// The library failed internally.
    Failed = 4,
    /// The encoding index marks the encoding UNPREDICTABLE.
    Unpredictable = 5,
    /// The encoding is a hint the architecture reserves, which behaves as
    /// a NOP.
    ReservedHint = 6,
    /// The bytes ended before the instruction did.
    Truncated = 7,
}

impl From<crate::DecodeError> for Status {
    fn from(error: crate::DecodeError) -> Self {
        match error {
            crate::DecodeError::Unallocated => Status::Unallocated,
            crate::DecodeError::UNDEF => Status::Undefined,
            crate::DecodeError::NOP => Status::Nop,
            crate::DecodeError::Unpredictable => Status::Unpredictable,
            crate::DecodeError::ReservedHint => Status::ReservedHint,
            crate::DecodeError::Truncated => Status::Truncated,
        }
    }
}

/// A string that is not NUL-terminated.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Str {
    /// The bytes.
    pub data: *const u8,
    /// How many.
    pub length: usize,
}

// A static string's bytes are read-only and never move, so the pointer is
// safe to share between threads.
unsafe impl Send for Str {}
unsafe impl Sync for Str {}

impl Str {
    /// A string that lives as long as the library.
    pub const fn of(s: &'static str) -> Self {
        Str {
            data: s.as_ptr(),
            length: s.len(),
        }
    }

    /// The empty string.
    pub const EMPTY: Str = Str::of("");
}

/// One token of an instruction's text, a span of the text buffer it was
/// written into.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    /// What the token is, a `TokenKind` by number.
    pub kind: u32,
    /// Where its text starts in the buffer.
    pub offset: u32,
    /// How long its text is.
    pub length: u32,
    /// Which operand of the instruction it is part of, or
    /// [`Token::NO_OPERAND`] for the mnemonic, the tab after it and the
    /// `, ` between operands.
    ///
    /// It sits in padding, so it adds nothing to the token's size.
    pub operand: u8,
    /// The number an immediate, integer or address token writes, and zero
    /// for any other token.
    pub value: u64,
}

impl Token {
    /// What `operand` holds for a token that is part of none. Every
    /// instruction has far fewer operands than this, so no index can
    /// collide with it.
    pub const NO_OPERAND: u8 = 0xFF;
}

/// How much text and how many tokens an instruction has.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextSize {
    /// How many tokens.
    pub tokens: usize,
    /// The length of the text without its NUL.
    pub length: usize,
}

impl TextSize {
    /// Nothing written.
    pub const NONE: TextSize = TextSize {
        tokens: 0,
        length: 0,
    };
}

/// What an instruction does with the condition flags, as C holds it.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlagEffect {
    /// The flags it writes, N Z C V from bit 3 down.
    pub writes: u8,
    /// The flags it reads.
    pub reads: u8,
    /// Whether the flags written are a floating-point comparison's.
    pub float_compare: bool,
}

/// What an instruction does to the flow of control, as C holds it.
///
/// `target` is meaningful only where `has_target` is set.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Branch {
    /// The kind of branch, a `crate::branch::BranchKind` by number.
    pub kind: u8,
    /// Whether the branch is taken only where a test succeeds.
    pub conditional: bool,
    /// Whether the instruction names the address it branches to.
    pub has_target: bool,
    /// The address it branches to, where `has_target`.
    pub target: u64,
}

impl Branch {
    /// Takes no branch, for a caller that asked with a null pointer or
    /// whose call failed.
    pub const NONE: Branch = Branch {
        kind: crate::branch::BranchKind::None as u8,
        conditional: false,
        has_target: false,
        target: 0,
    };
}

impl From<crate::branch::Branch> for Branch {
    fn from(branch: crate::branch::Branch) -> Self {
        Branch {
            kind: branch.kind as u8,
            conditional: branch.conditional,
            has_target: branch.target.is_some(),
            target: branch.target.unwrap_or(0),
        }
    }
}

impl From<crate::flags::FlagEffect> for FlagEffect {
    fn from(effect: crate::flags::FlagEffect) -> Self {
        FlagEffect {
            writes: effect.writes.0,
            reads: effect.reads.0,
            float_compare: effect.float_compare,
        }
    }
}

/// A value from a value table, `<prefix>_symbol`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Symbol {
    /// The bits the encoding holds.
    pub bits: u16,
    /// The name the assembly writes, empty where the table has none.
    pub name: Str,
}

impl From<crate::view::Symbol> for Symbol {
    fn from(s: crate::view::Symbol) -> Self {
        Symbol {
            bits: s.bits,
            name: Str::of(s.name),
        }
    }
}

/// A branch target or a literal's address, `<prefix>_label`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Label {
    /// The offset from the PC the instruction reads, which may run
    /// backwards.
    pub offset: i64,
    /// How many bytes ahead of the instruction the PC reads.
    pub pc_ahead: u8,
    /// The alignment in bytes the PC is rounded down to before the offset is
    /// added, 1 where it is not aligned.
    pub pc_align: u32,
}

impl Label {
    /// The label an offset from the PC names, as the instruction reads the PC.
    pub fn of(offset: i64, pc: crate::address::PcRead) -> Self {
        Label {
            offset,
            pc_ahead: pc.ahead,
            pc_align: pc.align,
        }
    }
}

/// A floating-point immediate as C holds it, `<prefix>_fp_imm`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CFpImm {
    /// The value.
    pub value: f64,
    /// The pattern the encoding holds, where `width` says there is one.
    pub bits: u64,
    /// How wide that pattern is, or zero where there is none.
    pub width: u8,
}

/// Whether a memory operand's base is written back, `<prefix>_writeback`.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Writeback {
    /// Not written back.
    None = 0,
    /// Before the access, with the offset applied.
    Pre = 1,
    /// After the access, by the offset.
    Post = 2,
}

impl From<crate::view::Writeback> for Writeback {
    fn from(writeback: crate::view::Writeback) -> Self {
        match writeback {
            crate::view::Writeback::None => Writeback::None,
            crate::view::Writeback::Pre => Writeback::Pre,
            crate::view::Writeback::Post => Writeback::Post,
        }
    }
}

/// Run `f`, and should it panic, report `fallback` instead.
pub fn guarded<T>(fallback: T, f: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(fallback)
}

/// The opaque storage a C face decodes an instruction into,
/// `exarmo_aarch64_instruction` or `exarmo_aarch32_instruction`.
///
/// The header fixes the storage's size, so a caller can hold one without
/// knowing what the runtime's `Instruction` is. The helpers here read and
/// write a pointer to the storage as a pointer to the instruction.
///
/// # Safety
///
/// An implementor must be at least as large and as aligned as
/// `Self::Instruction`, so that a pointer to one is a valid place to write
/// an instruction and read it back. Each crate asserts both at compile time
/// beside its implementation. The instruction is `Copy` so that it has
/// nothing to drop, since a decode overwrites whatever the storage held.
pub unsafe trait Storage {
    /// The runtime's own instruction, which a decode writes into the storage.
    type Instruction: crate::Decoded + Copy;
}

/// The instruction a C pointer holds, or none for a null pointer.
///
/// # Safety
///
/// A non-null `inst` must point to a value that the crate's decode wrote.
pub unsafe fn held<'a, C: Storage>(inst: *const C) -> Option<&'a C::Instruction> {
    if inst.is_null() {
        None
    } else {
        Some(unsafe { &*inst.cast::<C::Instruction>() })
    }
}

/// Ask something of the instruction a C pointer holds. Gives `none` for a
/// null pointer, or where the answer panics.
///
/// # Safety
///
/// As for [`held`].
pub unsafe fn with_held<C: Storage, T>(
    inst: *const C,
    none: T,
    ask: impl FnOnce(&C::Instruction) -> T,
) -> T {
    match unsafe { held(inst) } {
        Some(inst) => guarded(none, || ask(inst)),
        None => none,
    }
}

/// Write items into a caller's buffer of `capacity` at `out`, as many as fit,
/// and return how many there are, so a caller whose buffer was too small
/// learns the size it needed.
///
/// # Safety
///
/// A non-null `out` must point to `capacity` writable items.
pub unsafe fn fill<T>(
    out: *mut T,
    capacity: usize,
    items: impl ExactSizeIterator<Item = T>,
) -> usize {
    let count = items.len();
    for (slot, item) in unsafe { buffer(out, capacity) }.iter_mut().zip(items) {
        *slot = item;
    }
    count
}

/// Write an instruction's text and tokens into the caller's buffers, as it
/// reads at `address`.
///
/// # Safety
///
/// As for [`held`]. A non-null `text` must point to `text_capacity` writable
/// bytes and a non-null `tokens` to `token_capacity` writable tokens.
pub unsafe fn write_tokens<C: Storage>(
    inst: *const C,
    address: u64,
    text: *mut u8,
    text_capacity: usize,
    tokens: *mut Token,
    token_capacity: usize,
) -> TextSize {
    let text = unsafe { buffer(text, text_capacity) };
    let tokens = unsafe { buffer(tokens, token_capacity) };
    let write = |inst: &C::Instruction| {
        let mut spans = Spans::new(text, tokens);
        match inst.write_tokens_at(address, &mut spans) {
            Ok(()) => spans.finish(),
            Err(_) => TextSize::NONE,
        }
    };
    unsafe { with_held(inst, TextSize::NONE, write) }
}

/// Write a decode's outcome into `out`, which is written only on
/// `Status::Ok`.
///
/// A panic in the decode is reported as `Status::Failed` rather than
/// unwinding into C.
///
/// # Safety
///
/// `out` must point to writable storage for a `C`.
pub unsafe fn deliver<C: Storage>(
    decoded: impl FnOnce() -> Result<C::Instruction, crate::DecodeError>,
    out: *mut C,
) -> Status {
    guarded(Status::Failed, || match decoded() {
        Ok(inst) => {
            // SAFETY: the storage has room for the instruction, as
            // `Storage` requires, and the instruction is `Copy`, so nothing
            // the storage held before needs dropping.
            unsafe { out.cast::<C::Instruction>().write(inst) };
            Status::Ok
        }
        Err(error) => Status::from(error),
    })
}

/// A caller's buffer of `capacity` items at `ptr`, empty for a null
/// pointer or no capacity.
///
/// # Safety
///
/// A non-null `ptr` must point to `capacity` writable items.
pub unsafe fn buffer<'a, T>(ptr: *mut T, capacity: usize) -> &'a mut [T] {
    if ptr.is_null() || capacity == 0 {
        &mut []
    } else {
        unsafe { std::slice::from_raw_parts_mut(ptr, capacity) }
    }
}

/// The caller's `len` bytes at `ptr`, or nothing for a null pointer with a
/// length.
///
/// # Safety
///
/// A non-null `ptr` must point to `len` readable bytes.
pub unsafe fn bytes<'a>(ptr: *const u8, len: usize) -> Option<&'a [u8]> {
    match (ptr.is_null(), len) {
        (_, 0) => Some(&[]),
        (true, _) => None,
        (false, _) => Some(unsafe { std::slice::from_raw_parts(ptr, len) }),
    }
}

/// A token sink writing into a caller's text and token buffers. Text beyond
/// the buffer is counted and dropped, so the caller learns the size it
/// needed, and the last byte is kept for the NUL.
pub struct Spans<'a> {
    text: &'a mut [u8],
    length: usize,
    tokens: &'a mut [Token],
    count: usize,
}

impl<'a> Spans<'a> {
    /// A sink writing into the caller's text and token buffers, either of
    /// which may be empty.
    pub fn new(text: &'a mut [u8], tokens: &'a mut [Token]) -> Self {
        Spans {
            text,
            length: 0,
            tokens,
            count: 0,
        }
    }

    fn record(&mut self, kind: TokenKind, operand: OperandIndex, start: usize) {
        if let Some(slot) = self.tokens.get_mut(self.count) {
            let (kind, value) = match kind {
                TokenKind::Mnemonic => (0, 0),
                TokenKind::Text => (1, 0),
                TokenKind::Separator => (2, 0),
                TokenKind::Bracket => (3, 0),
                TokenKind::Register => (4, 0),
                TokenKind::Symbol => (5, 0),
                TokenKind::Immediate(value) => (6, value as u64),
                TokenKind::Integer(value) => (7, value),
                TokenKind::Address(address) => (8, address),
                TokenKind::Float => (9, 0),
            };
            *slot = Token {
                kind,
                operand: operand.unwrap_or(Token::NO_OPERAND),
                offset: start as u32,
                length: (self.length - start) as u32,
                value,
            };
        }
        self.count += 1;
    }

    /// Write the NUL where the text ends, or where the buffer does.
    pub fn finish(self) -> TextSize {
        if let Some(last) = self.text.len().checked_sub(1) {
            self.text[self.length.min(last)] = 0;
        }
        TextSize {
            tokens: self.count,
            length: self.length,
        }
    }
}

impl Write for Spans<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let room = self.text.len().saturating_sub(1);
        if self.length < room {
            let n = s.len().min(room - self.length);
            self.text[self.length..self.length + n].copy_from_slice(&s.as_bytes()[..n]);
        }
        self.length += s.len();
        Ok(())
    }
}

impl TokenSink for Spans<'_> {
    fn text(&mut self, kind: TokenKind, operand: OperandIndex, text: &str) -> fmt::Result {
        let start = self.length;
        self.write_str(text)?;
        self.record(kind, operand, start);
        Ok(())
    }

    fn value(
        &mut self,
        kind: TokenKind,
        operand: OperandIndex,
        value: &dyn fmt::Display,
    ) -> fmt::Result {
        let start = self.length;
        write!(self, "{value}")?;
        self.record(kind, operand, start);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panic_is_reported_not_propagated() {
        assert_eq!(guarded(7, || panic!("inside")), 7);
        assert_eq!(guarded(7, || 1), 1);
    }

    #[test]
    fn a_text_buffer_is_cut_at_its_capacity_and_terminated() {
        let mut text = [0xAAu8; 5];
        let mut spans = Spans::new(&mut text, &mut []);
        spans.text(TokenKind::Mnemonic, None, "ldr").unwrap();
        spans.text(TokenKind::Text, None, "\t").unwrap();
        spans.value(TokenKind::Register, Some(0), &"x0").unwrap();
        let size = spans.finish();
        assert_eq!((size.tokens, size.length), (3, 6));
        assert_eq!(&text, b"ldr\t\0");
    }
}
