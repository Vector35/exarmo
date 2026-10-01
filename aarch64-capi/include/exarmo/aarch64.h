/* The C face of the exarmo-aarch64 disassembler.
 *
 * Decode an instruction once with exarmo_aarch64_decode, then ask the decoded
 * instruction what it is (its encoding and mnemonic), what it does to the
 * condition flags, what its operands are, and how it is written. Nothing
 * here allocates. The caller owns every buffer, and a function that writes
 * into one says how much it needed, so a buffer too small is detected and
 * resized rather than overrun.
 *
 * Every function is safe to call from any thread, and none can fail
 * except as its documentation says. An internal failure is reported, never
 * propagated, and never leaves the process.
 *
 * The identity of each instruction, exarmo_aarch64_encoding and
 * exarmo_aarch64_mnemonic, and the kinds of modifier are generated from the
 * architecture's XML into aarch64_generated.h.
 */
#ifndef EXARMO_AARCH64_H
#define EXARMO_AARCH64_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "aarch64_generated.h"

#ifdef __cplusplus
extern "C" {
#endif

/* ------------------------------------------------------------------------
 * Decoding
 * ---------------------------------------------------------------------- */

/* A decoded instruction, opaque, read through the functions below. It has
 * no address. The functions that need one take it as an argument. */
typedef struct exarmo_aarch64_instruction {
    uint64_t opaque[3];
} exarmo_aarch64_instruction;

/* How many bytes of storage a decode writes, as the library was built. A
 * caller linked against a library from another release compares it with
 * sizeof(exarmo_aarch64_instruction) to tell that its storage still holds
 * an instruction. */
size_t exarmo_aarch64_instruction_size(void);

typedef enum exarmo_aarch64_status {
    /* The instruction decoded. */
    EXARMO_AARCH64_STATUS_OK = 0,
    /* The encoding is allocated to no instruction. */
    EXARMO_AARCH64_STATUS_UNALLOCATED = 1,
    /* The instruction's decode says it is UNDEFINED. */
    EXARMO_AARCH64_STATUS_UNDEFINED = 2,
    /* The instruction's decode says it executes as a NOP. No encoding of
     * the bundles read so far reaches this. */
    EXARMO_AARCH64_STATUS_NOP = 3,
    /* The library failed internally. Nothing was written. */
    EXARMO_AARCH64_STATUS_FAILED = 4,
    /* The encoding index marks the encoding UNPREDICTABLE. Only the AArch32
     * index marks a row so, and an AArch64 decode never returns this. */
    EXARMO_AARCH64_STATUS_UNPREDICTABLE = 5,
    /* The encoding is a hint the architecture reserves, which behaves as a
     * NOP and may be allocated by a later release. Only the AArch32 index
     * marks a row so, and an AArch64 decode never returns this. */
    EXARMO_AARCH64_STATUS_RESERVED_HINT = 6,
} exarmo_aarch64_status;

/* Decode the 32-bit instruction word `bits` into `out`. `out` is written
 * only when the status is EXARMO_AARCH64_STATUS_OK. */
exarmo_aarch64_status exarmo_aarch64_decode(uint32_t bits, exarmo_aarch64_instruction *out);

/* ------------------------------------------------------------------------
 * Identity
 * ---------------------------------------------------------------------- */

/* A string that is not NUL-terminated, `length` bytes at `data`. */
typedef struct exarmo_aarch64_str {
    const char *data;
    size_t length;
} exarmo_aarch64_str;

/* Which encoding the instruction is, or EXARMO_AARCH64_ENCODING_COUNT for a
 * null pointer. */
exarmo_aarch64_encoding exarmo_aarch64_instruction_encoding(const exarmo_aarch64_instruction *inst);

/* The mnemonic the instruction is written with, or
 * EXARMO_AARCH64_MNEMONIC_COUNT for a null pointer. */
exarmo_aarch64_mnemonic exarmo_aarch64_instruction_mnemonic(const exarmo_aarch64_instruction *inst);

/* How many bytes of the instruction stream the instruction is, or 0 for a
 * null pointer. Every A64 instruction is four. The AArch32 face asks the
 * same question, where a T32 instruction is two or four. */
uint8_t exarmo_aarch64_instruction_length(const exarmo_aarch64_instruction *inst);

/* Whether the word `inst` was decoded from is CONSTRAINED UNPREDICTABLE: it
 * reached a test its decode pseudocode calls UNPREDICTABLE, or a bit its
 * diagram writes (0) or (1) holds the other value. The architecture then
 * permits one of a documented set of behaviours, and the instruction is still
 * written as the word says. False for a null pointer. */
bool exarmo_aarch64_instruction_unpredictable(const exarmo_aarch64_instruction *inst);

/* The mnemonic an encoding is written with, or EXARMO_AARCH64_MNEMONIC_COUNT
 * for a value that is no encoding. */
exarmo_aarch64_mnemonic exarmo_aarch64_encoding_mnemonic(exarmo_aarch64_encoding encoding);

/* An encoding's name, as aarch64_generated.h spells it after
 * EXARMO_AARCH64_ENC_, or empty for a value that is no encoding. */
exarmo_aarch64_str exarmo_aarch64_encoding_name(exarmo_aarch64_encoding encoding);

/* How many bytes an encoding is, always 4, or 0 for a value that is no
 * encoding. */
uint8_t exarmo_aarch64_encoding_length(exarmo_aarch64_encoding encoding);

/* A mnemonic as the assembly writes it, in lower case, or empty for a value
 * that is no mnemonic. */
exarmo_aarch64_str exarmo_aarch64_mnemonic_name(exarmo_aarch64_mnemonic mnemonic);

/* ------------------------------------------------------------------------
 * Condition flags
 * ---------------------------------------------------------------------- */

/* The condition flags, one bit each, as they sit in NZCV. */
enum {
    EXARMO_AARCH64_FLAG_N = 1 << 3,
    EXARMO_AARCH64_FLAG_Z = 1 << 2,
    EXARMO_AARCH64_FLAG_C = 1 << 1,
    EXARMO_AARCH64_FLAG_V = 1 << 0,
    EXARMO_AARCH64_FLAG_NZCV = 0xF,
};

/* What an instruction does with the condition flags, as its pseudocode
 * says. */
typedef struct exarmo_aarch64_flag_effect {
    /* The flags it writes, a set of EXARMO_AARCH64_FLAG_ bits. */
    uint8_t writes;
    /* The flags it reads, all four through a condition, or the carry alone. */
    uint8_t reads;
    /* Whether the flags written come from comparing floating-point values,
     * which sets them differently from an integer comparison. */
    bool float_compare;
} exarmo_aarch64_flag_effect;

exarmo_aarch64_flag_effect exarmo_aarch64_instruction_flags(const exarmo_aarch64_instruction *inst);

/* ------------------------------------------------------------------------
 * Control flow
 * ---------------------------------------------------------------------- */

/* What kind of branch an instruction takes, from the `BranchType` its
 * Execute pseudocode carries or the exception it raises. */
typedef enum exarmo_aarch64_branch_kind {
    /* It does not branch. */
    EXARMO_AARCH64_BRANCH_NONE = 0,
    /* It branches to an address it names, `BranchType_DIR`. */
    EXARMO_AARCH64_BRANCH_DIRECT = 1,
    /* It branches to an address it computes, `BranchType_INDIR`. */
    EXARMO_AARCH64_BRANCH_INDIRECT = 2,
    /* It calls an address it names, leaving a return address behind,
     * `BranchType_DIRCALL`. */
    EXARMO_AARCH64_BRANCH_DIRECT_CALL = 3,
    /* It calls an address it computes, `BranchType_INDCALL`. */
    EXARMO_AARCH64_BRANCH_INDIRECT_CALL = 4,
    /* It returns from a call, `BranchType_RET`. */
    EXARMO_AARCH64_BRANCH_RETURN = 5,
    /* It returns from an exception, restoring the state the exception
     * saved, as ERET does, and DRPS in debug state. */
    EXARMO_AARCH64_BRANCH_EXCEPTION_RETURN = 6,
    /* It takes an exception whose handler returns to the instruction after
     * it, as SVC, HVC, SMC and TENTER do. */
    EXARMO_AARCH64_BRANCH_SYSTEM_CALL = 7,
    /* It raises an exception, as BRK and an encoding the architecture
     * permanently leaves undefined do. */
    EXARMO_AARCH64_BRANCH_EXCEPTION = 8,
    /* It halts the processor into debug state, as HLT does. */
    EXARMO_AARCH64_BRANCH_HALT = 9,
} exarmo_aarch64_branch_kind;

/* What an instruction does to the flow of control, as it reads at an
 * address.
 *
 * Where execution goes when the branch is not taken is the caller's to say.
 * The instruction after a conditional branch is at the address plus its
 * length, and where an exception handler returns to is the handler's
 * business. The last three kinds raise an exception, and the architecture
 * does not say where each resumes. */
typedef struct exarmo_aarch64_branch {
    /* What kind of branch it takes, an exarmo_aarch64_branch_kind. */
    uint8_t kind;
    /* Whether the branch is taken only where a test succeeds, so that the
     * instruction after it runs where the test fails.
     *
     * This is the instruction's conditionality, not its encoding's. A
     * `b.al` is written in the conditional encoding and branches every
     * time, so it is not conditional here. Nor is `b.nv`, which the
     * architecture makes always hold, as AL does. */
    bool conditional;
    /* Whether the instruction names the address it branches to, which a
     * branch through a register does not. */
    bool has_target;
    /* The address it branches to where `has_target`, and 0 otherwise. */
    uint64_t target;
} exarmo_aarch64_branch;

/* What the instruction does to the flow of control, as it would at
 * `address`. The kind is EXARMO_AARCH64_BRANCH_NONE for a null pointer. */
exarmo_aarch64_branch exarmo_aarch64_instruction_branch(const exarmo_aarch64_instruction *inst, uint64_t address);

/* ------------------------------------------------------------------------
 * Text
 * ---------------------------------------------------------------------- */

/* What a token is. */
typedef enum exarmo_aarch64_token_kind {
    /* The mnemonic, or part of it, as `ldr`, or `b.` and then `eq`. */
    EXARMO_AARCH64_TOKEN_MNEMONIC = 0,
    /* Text that is none of the kinds below: the tab after the mnemonic, a
     * `.` before an arrangement, a `!` for writeback, and a `, ` within one
     * operand, as in `[x1, #0x8]` or `x2, lsl #0x3`. */
    EXARMO_AARCH64_TOKEN_TEXT = 1,
    /* The `, ` between two operands, and only there, so a consumer
     * numbering operands by counting these counts right. Always
     * EXARMO_AARCH64_TOKEN_NO_OPERAND. */
    EXARMO_AARCH64_TOKEN_SEPARATOR = 2,
    /* A `[`, `]`, `{` or `}`. */
    EXARMO_AARCH64_TOKEN_BRACKET = 3,
    /* A register, with any arrangement written on it: `x0`, `v1.4s`. */
    EXARMO_AARCH64_TOKEN_REGISTER = 4,
    /* A name from a value table: a shift, an extend, a condition, a barrier
     * option, a prefetch operation. */
    EXARMO_AARCH64_TOKEN_SYMBOL = 5,
    /* An immediate, such as `0x8` or `-0x10`, which `value` holds as an
     * int64_t. The `#` before it is an EXARMO_AARCH64_TOKEN_TEXT token of
     * its own. */
    EXARMO_AARCH64_TOKEN_IMMEDIATE = 6,
    /* A number written bare, an element index or a lane, which `value`
     * holds. */
    EXARMO_AARCH64_TOKEN_INTEGER = 7,
    /* A branch target or a literal's address, which `value` holds. */
    EXARMO_AARCH64_TOKEN_ADDRESS = 8,
    /* A floating-point immediate, such as `1.0`, after its `#`. */
    EXARMO_AARCH64_TOKEN_FLOAT = 9,
} exarmo_aarch64_token_kind;

/* What `operand` holds for a token that is part of no operand. Every
 * instruction has far fewer operands than this, so no index collides. */
#define EXARMO_AARCH64_TOKEN_NO_OPERAND 0xFF

/* One token of an instruction's text, a span of the text buffer the tokens
 * were written with. */
typedef struct exarmo_aarch64_token {
    exarmo_aarch64_token_kind kind;
    /* Where the token's text starts in the buffer, and how long it is. */
    uint32_t offset;
    uint32_t length;
    /* Which operand of exarmo_aarch64_instruction_operands the token is part
     * of, or EXARMO_AARCH64_TOKEN_NO_OPERAND for the mnemonic, the tab after
     * it and the `, ` between operands.
     *
     * This is what tells one `[` from another. A memory operand's brackets
     * are its own, where the brackets of a lane index or an SME slice
     * belong to the operand they are written on, and a register list
     * written in braces is one operand however many registers it names. */
    uint8_t operand;
    /* The number an immediate, integer or address token writes, an
     * immediate as an int64_t and the rest as written. Zero for other
     * kinds. */
    uint64_t value;
} exarmo_aarch64_token;

/* Write the instruction as text, as it would be at `address`, into `text`,
 * NUL-terminated when `capacity` allows. Returns the length of the whole
 * text without the NUL, which is more than `capacity - 1` when the text was
 * cut short, or zero when the library failed. */
size_t exarmo_aarch64_instruction_text(const exarmo_aarch64_instruction *inst, uint64_t address, char *text,
                                size_t capacity);

/* How much exarmo_aarch64_instruction_tokens wrote, or would have. */
typedef struct exarmo_aarch64_text_size {
    /* How many tokens the instruction has. */
    size_t tokens;
    /* The length of the whole text without the NUL. */
    size_t length;
} exarmo_aarch64_text_size;

/* Write the instruction as tokens. The text goes into `text`,
 * NUL-terminated when `text_capacity` allows, and each token into `tokens`,
 * up to `token_capacity` of them.
 *
 * A caller that reserves EXARMO_AARCH64_MAX_TOKENS tokens and
 * EXARMO_AARCH64_MAX_TEXT + 1 bytes of text never has to ask for more,
 * whatever this release decodes. The result says how many tokens and how much
 * text the instruction has. Either is beyond its capacity when that buffer
 * was too small, and both are zero when the library failed. A token cut
 * short by the text buffer keeps its full length. */
exarmo_aarch64_text_size exarmo_aarch64_instruction_tokens(const exarmo_aarch64_instruction *inst, uint64_t address,
                                             char *text, size_t text_capacity,
                                             exarmo_aarch64_token *tokens, size_t token_capacity);

/* ------------------------------------------------------------------------
 * Operands
 * ---------------------------------------------------------------------- */

/* Which register file a register is in, and for a general-purpose register
 * its width and what number 31 means. */
typedef enum exarmo_aarch64_reg_class {
    /* A 32-bit general-purpose register, 31 the zero register. */
    EXARMO_AARCH64_REG_W = 0,
    /* A 32-bit general-purpose register, 31 the stack pointer. */
    EXARMO_AARCH64_REG_W_SP = 1,
    /* A 64-bit general-purpose register, 31 the zero register. */
    EXARMO_AARCH64_REG_X = 2,
    /* A 64-bit general-purpose register, 31 the stack pointer. */
    EXARMO_AARCH64_REG_X_SP = 3,
    /* A general-purpose register whose width the instruction did not fix. */
    EXARMO_AARCH64_REG_GP = 4,
    /* Scalar SIMD and floating-point registers, by width. */
    EXARMO_AARCH64_REG_B = 5,
    EXARMO_AARCH64_REG_H = 6,
    EXARMO_AARCH64_REG_S = 7,
    EXARMO_AARCH64_REG_D = 8,
    EXARMO_AARCH64_REG_Q = 9,
    /* A vector register. */
    EXARMO_AARCH64_REG_V = 10,
    /* An SVE vector register. */
    EXARMO_AARCH64_REG_Z = 11,
    /* An SVE predicate register. */
    EXARMO_AARCH64_REG_P = 12,
    /* An SVE predicate-as-counter register. */
    EXARMO_AARCH64_REG_PN = 13,
    /* An SME ZA tile, za0 to za15. */
    EXARMO_AARCH64_REG_ZA_TILE = 14,
    /* The SME lookup table register, zt0. */
    EXARMO_AARCH64_REG_ZT0 = 15,
} exarmo_aarch64_reg_class;

typedef struct exarmo_aarch64_reg {
    uint8_t class_; /* exarmo_aarch64_reg_class */
    uint8_t num;
} exarmo_aarch64_reg;

/* The width of one lane of a vector. */
typedef enum exarmo_aarch64_element_width {
    /* No arrangement is written. */
    EXARMO_AARCH64_ELEMENT_NONE = 0,
    EXARMO_AARCH64_ELEMENT_B = 8,
    EXARMO_AARCH64_ELEMENT_H = 16,
    EXARMO_AARCH64_ELEMENT_S = 32,
    EXARMO_AARCH64_ELEMENT_D = 64,
    EXARMO_AARCH64_ELEMENT_Q = 128,
} exarmo_aarch64_element_width;

/* An arrangement written on a register. `.4s` is four 32-bit lanes, and
 * `.d` is 64-bit elements with no lane count written. */
typedef struct exarmo_aarch64_arrangement {
    /* The width of one lane in bits, or EXARMO_AARCH64_ELEMENT_NONE. */
    uint8_t element;
    /* How many lanes, or zero where none is written. */
    uint8_t lanes;
} exarmo_aarch64_arrangement;

/* A value from a value table, such as a shift, an extend, a barrier option
 * or a prefetch operation, as its bits and its name. A condition is an
 * operand of its own, EXARMO_AARCH64_OPERAND_COND. */
typedef struct exarmo_aarch64_symbol {
    /* The bits the encoding holds. */
    uint16_t bits;
    /* The name the assembly writes, in lower case, or empty for a value the
     * table does not name. */
    exarmo_aarch64_str name;
} exarmo_aarch64_symbol;

/* A system operation or PSTATE field an operand names.
 *
 * `index` is the row of exarmo_aarch64_sysops, which is the identity and
 * carries the name, the instruction naming it, and the five encoding
 * fields. `field` is what the instruction's own operand field holds. It is
 * a different concatenation for each instruction, and for most PSTATE
 * fields it carries the instruction's immediate as well, so it names
 * nothing on its own. */
typedef struct exarmo_aarch64_sysop {
    uint16_t index;
    uint16_t field;
} exarmo_aarch64_sysop;

/* A shift, extend or multiplier applied to a register or an immediate. */
typedef struct exarmo_aarch64_modifier {
    /* Whether one is written. */
    bool present;
    /* What is applied, an exarmo_aarch64_modifier_kind. */
    uint8_t kind;
    /* How far, in bits, or -1 where no amount is written. */
    int32_t amount;
} exarmo_aarch64_modifier;

/* A register operand, with what the assembly writes on it. */
typedef struct exarmo_aarch64_reg_operand {
    exarmo_aarch64_reg reg;
    /* The arrangement, such as `.4s` or `.d`. */
    exarmo_aarch64_arrangement arrangement;
    /* An element index, `v0.s[2]`, or -1 where none is written. */
    int32_t index;
    /* A register the element index counts from, `p0.b[w12, #3]`, where
     * `has_index_reg`. */
    bool has_index_reg;
    exarmo_aarch64_reg index_reg;
    /* A predication qualifier, 'z' or 'm', or 0 where none is written. */
    char predication;
    /* A shift or extend written after it, as in `x1, lsl #2`. */
    exarmo_aarch64_modifier modifier;
} exarmo_aarch64_reg_operand;

/* What a memory operand adds to its base. */
typedef enum exarmo_aarch64_offset_kind {
    EXARMO_AARCH64_OFFSET_NONE = 0,
    /* An immediate, in bytes, or in vector lengths with `mul vl`. */
    EXARMO_AARCH64_OFFSET_IMM = 1,
    /* A general-purpose register, shifted or extended as the modifier says. */
    EXARMO_AARCH64_OFFSET_REG = 2,
    /* A vector of offsets, one per lane. */
    EXARMO_AARCH64_OFFSET_VECTOR = 3,
} exarmo_aarch64_offset_kind;

typedef struct exarmo_aarch64_offset {
    exarmo_aarch64_offset_kind kind;
    /* For EXARMO_AARCH64_OFFSET_IMM, the value and whether it is in vector
     * lengths. */
    int64_t imm;
    bool mul_vl;
    /* For EXARMO_AARCH64_OFFSET_REG and EXARMO_AARCH64_OFFSET_VECTOR, the
     * register, its arrangement for a vector, and how it is shifted or
     * extended. */
    exarmo_aarch64_reg reg;
    exarmo_aarch64_arrangement arrangement;
    exarmo_aarch64_modifier modifier;
} exarmo_aarch64_offset;

/* Whether a memory operand's base is written back. */
typedef enum exarmo_aarch64_writeback {
    EXARMO_AARCH64_WRITEBACK_NONE = 0,
    /* Before the access, with the offset applied, as in `[x0, #8]!`. */
    EXARMO_AARCH64_WRITEBACK_PRE = 1,
    /* After the access, by `offset`, which the assembly writes after the
     * bracket, as in `[x0], #8`. */
    EXARMO_AARCH64_WRITEBACK_POST = 2,
} exarmo_aarch64_writeback;

/* A memory operand, an address and whether the base is updated. */
typedef struct exarmo_aarch64_mem {
    /* The register the address starts from. */
    exarmo_aarch64_reg base;
    /* The arrangement of a vector base, as in `[z0.d]`. */
    exarmo_aarch64_arrangement base_arrangement;
    /* What is added to the base, which for EXARMO_AARCH64_WRITEBACK_POST is what
     * the base is advanced by after the access. */
    exarmo_aarch64_offset offset;
    exarmo_aarch64_writeback writeback;
} exarmo_aarch64_mem;

/* A list of registers, such as `{ v0.16b, v1.16b }` or `{ z0.b - z3.b }`. */
typedef struct exarmo_aarch64_reg_list {
    /* The registers, in order. Only the first `len` are the list's. */
    exarmo_aarch64_reg regs[4];
    uint8_t len;
    /* The arrangement written on it. */
    exarmo_aarch64_arrangement arrangement;
    /* An element index on the whole list, `{ v0.s, v1.s }[2]`, or -1 where
     * none is written. */
    int32_t index;
} exarmo_aarch64_reg_list;

/* The SME storage a slice is taken from. */
typedef enum exarmo_aarch64_za_array {
    /* The ZA array as a whole, `za[w12, 3]`, or by element width. */
    EXARMO_AARCH64_ZA_ARRAY = 0,
    /* The lookup table register, `zt0[3]`. */
    EXARMO_AARCH64_ZA_ZT0 = 1,
    /* One tile, `za0h.b[w12, 3]`, which `tile` names. */
    EXARMO_AARCH64_ZA_TILE = 2,
} exarmo_aarch64_za_array;

/* A slice of SME storage. It is rows or columns of a tile, or vectors of
 * the array, selected by an index register plus an offset or a range of
 * offsets, and for the array optionally as a vector group. */
typedef struct exarmo_aarch64_za_slice {
    exarmo_aarch64_za_array array;
    /* For EXARMO_AARCH64_ZA_TILE, the tile number. */
    uint8_t tile;
    /* 'h' for horizontal or 'v' for vertical where a tile says, or 0. */
    char direction;
    /* The element width written on it. */
    exarmo_aarch64_arrangement arrangement;
    /* The register the slice index counts from, where `has_index`. ZT0 has
     * none. */
    bool has_index;
    exarmo_aarch64_reg index;
    /* The offset added to the index, or the first of a range. */
    uint32_t offset;
    /* The last offset of a range, `za.s[w12, 0:3]`, or -1 where none. */
    int32_t last;
    /* Whether the offset is in vector lengths. */
    bool mul_vl;
    /* The vector group, 2 or 4, where written, or 0. */
    uint8_t vector_group;
} exarmo_aarch64_za_slice;

/* A system register, as MRS and MSR name it. */
typedef struct exarmo_aarch64_sysreg {
    /* The op0:op1:CRn:CRm:op2 encoding, `op0` as the two bits it is, so
     * that a register and a system operation, whose op0 is 0 or 1, cannot
     * come to one number. Every register MRS and MSR reach has op0 2 or 3,
     * so the top bit is always set. */
    uint16_t encoding;
    /* Whether the instruction writes it. */
    bool write;
    uint8_t op0, op1, crn, crm, op2;
} exarmo_aarch64_sysreg;

/* A branch target or a page's address, as an offset from the PC the
 * instruction reads. A64 reads the PC as the instruction's own address, so
 * pc_ahead is 0. ADRP names the page the target is in, so pc_align is 4096
 * there and 1 everywhere else. The address named is
 * ((address + pc_ahead) & ~(pc_align - 1)) + offset, held to 64 bits, so a
 * target running off either end wraps as the architecture's PC does.
 *
 * Where the label is a branch target, exarmo_aarch64_instruction_branch has
 * already worked this out and hands over the address. This is for the
 * labels that are not branch targets: ADR, ADRP and a literal load. */
typedef struct exarmo_aarch64_label {
    int64_t offset;
    uint8_t pc_ahead;
    uint32_t pc_align;
} exarmo_aarch64_label;

/* A floating-point immediate, the value and the pattern the encoding holds
 * where it holds one.
 *
 * An immediate the decode expands to a width of its own is that pattern, so
 * a consumer building a constant of that width reads the bits rather than
 * encoding the value back, which C cannot do for half precision. Where
 * `width` is zero there is no such pattern: SVE's FCPY writes its constant
 * at the arrangement's element width, a table of reals names a value, and
 * FCMP against zero has no field at all. */
typedef struct exarmo_aarch64_fp_imm {
    double value;
    /* The pattern the encoding holds, where `width` says there is one. */
    uint64_t bits;
    /* How wide that pattern is, 16, 32 or 64, or zero for none. */
    uint8_t width;
} exarmo_aarch64_fp_imm;

/* What an operand is. */
typedef enum exarmo_aarch64_operand_kind {
    /* A register, with what is written on it, in `reg`. */
    EXARMO_AARCH64_OPERAND_REG = 0,
    /* An immediate, with any shift written after it, in `imm`. */
    EXARMO_AARCH64_OPERAND_IMM = 1,
    /* A floating-point immediate, in `fp_imm`. */
    EXARMO_AARCH64_OPERAND_FP_IMM = 2,
    /* A branch target or a page's address, as the offset from the PC the
     * instruction reads, in `label`. */
    EXARMO_AARCH64_OPERAND_LABEL = 3,
    /* A memory operand, in `mem`. */
    EXARMO_AARCH64_OPERAND_MEM = 4,
    /* A register list, in `list`. */
    EXARMO_AARCH64_OPERAND_LIST = 5,
    /* A system register, in `sysreg`. */
    EXARMO_AARCH64_OPERAND_SYSREG = 6,
    /* A system operation or PSTATE field, as the row of
     * exarmo_aarch64_sysops naming it, in `sysop`. */
    EXARMO_AARCH64_OPERAND_SYSOP = 7,
    /* The condition the instruction is written under, after any inversion
     * its form applies, in `cond`. */
    EXARMO_AARCH64_OPERAND_COND = 8,
    /* A value from a value table, in `symbol`. */
    EXARMO_AARCH64_OPERAND_SYMBOL = 9,
    /* A slice of SME storage, in `za_slice`. */
    EXARMO_AARCH64_OPERAND_ZA_SLICE = 10,
    /* The set of ZA tiles ZERO clears, bit n for za<n>.d, in `tile_mask`. */
    EXARMO_AARCH64_OPERAND_TILE_MASK = 11,
    /* A shift or multiplier written as an operand of its own, applying to
     * the instruction rather than to the operand before it, in `modifier`. */
    EXARMO_AARCH64_OPERAND_MODIFIER = 12,
    /* An operand the library does not describe. */
    EXARMO_AARCH64_OPERAND_OTHER = 13,
} exarmo_aarch64_operand_kind;

/* One operand, as the assembly writes it. */
typedef struct exarmo_aarch64_operand {
    exarmo_aarch64_operand_kind kind;
    union {
        exarmo_aarch64_reg_operand reg;
        struct {
            int64_t value;
            exarmo_aarch64_modifier modifier;
        } imm;
        exarmo_aarch64_fp_imm fp_imm;
        exarmo_aarch64_label label;
        exarmo_aarch64_mem mem;
        exarmo_aarch64_reg_list list;
        exarmo_aarch64_sysreg sysreg;
        exarmo_aarch64_sysop sysop;
        /* An exarmo_aarch64_cond. CSET and its neighbours read the condition
         * field inverted, and this is the condition they are written under,
         * not the bits the field holds. `cset w8, ne` is COND_NE, where the
         * field holds what a plainly read one would spell EQ. */
        uint8_t cond;
        exarmo_aarch64_symbol symbol;
        exarmo_aarch64_za_slice za_slice;
        uint8_t tile_mask;
        exarmo_aarch64_modifier modifier;
    };
} exarmo_aarch64_operand;

/* Write the instruction's operands, in the order the assembly writes them,
 * into `out`, up to `capacity` of them. Returns how many operands the
 * instruction has, which is never more than EXARMO_AARCH64_MAX_OPERANDS. */
size_t exarmo_aarch64_instruction_operands(const exarmo_aarch64_instruction *inst, exarmo_aarch64_operand *out,
                                    size_t capacity);

/* The ACLE intrinsics an instruction realises.
 *
 * ARM's C Language Extensions name a C intrinsic for most Advanced SIMD
 * instructions and say which register each argument reaches. An instruction
 * says which intrinsics it is and where each argument sits among its
 * operands. The table below says what each intrinsic is, apart from any
 * instruction, so a lifter can name and type a call it has decided to make.
 *
 * An intrinsic is an `id`, an index below EXARMO_AARCH64_INTRINSIC_COUNT that is
 * the same in every build of a given table. */

/* Where an instruction holds an argument. */
typedef enum exarmo_aarch64_intrinsic_source_kind {
    /* The operand itself: a register, a register list, or a memory operand
     * for a pointer. */
    EXARMO_AARCH64_INTRINSIC_SOURCE_OPERAND = 0,
    /* The element index written on the operand, `lane` in `Vm.H[lane]`. */
    EXARMO_AARCH64_INTRINSIC_SOURCE_INDEX = 1,
    /* An immediate operand. The argument was shifted left by `shift` to
     * become it, so shift right to recover it. */
    EXARMO_AARCH64_INTRINSIC_SOURCE_IMMEDIATE = 2,
    /* The FPMR register, which the instruction reads. */
    EXARMO_AARCH64_INTRINSIC_SOURCE_FPMR = 3,
    /* Nowhere. The intrinsic fixes its only value, as for a lane index that
     * can only be 0, so the instruction does not carry it. */
    EXARMO_AARCH64_INTRINSIC_SOURCE_UNUSED = 4,
} exarmo_aarch64_intrinsic_source_kind;

/* Where an instruction holds one argument. */
typedef struct exarmo_aarch64_intrinsic_source {
    uint8_t kind; /* exarmo_aarch64_intrinsic_source_kind */
    /* Which operand of exarmo_aarch64_instruction_operands, for OPERAND, INDEX
     * and IMMEDIATE. */
    uint8_t operand;
    /* How far the immediate was shifted, for IMMEDIATE. */
    uint8_t shift;
} exarmo_aarch64_intrinsic_source;

/* Where an intrinsic's result comes from. */
typedef enum exarmo_aarch64_intrinsic_output_kind {
    /* Nowhere, as for a store. */
    EXARMO_AARCH64_INTRINSIC_OUTPUT_NONE = 0,
    /* An operand. */
    EXARMO_AARCH64_INTRINSIC_OUTPUT_OPERAND = 1,
    /* One element of a structure of vectors, where the instruction writes
     * that element alone, as ZIP1 does for vzip_s8's result.val[0]. */
    EXARMO_AARCH64_INTRINSIC_OUTPUT_ELEMENT = 2,
} exarmo_aarch64_intrinsic_output_kind;

/* Where the result comes from. */
typedef struct exarmo_aarch64_intrinsic_output {
    uint8_t kind; /* exarmo_aarch64_intrinsic_output_kind */
    /* Which operand, for OPERAND and ELEMENT. */
    uint8_t operand;
    /* Which element, for ELEMENT. */
    uint8_t element;
} exarmo_aarch64_intrinsic_output;

/* One intrinsic a decoded instruction realises. */
typedef struct exarmo_aarch64_intrinsic {
    /* Which intrinsic, an index below EXARMO_AARCH64_INTRINSIC_COUNT. */
    uint32_t id;
    /* How many of `arguments` are filled. */
    uint8_t argument_count;
    /* Where the instruction holds each argument, in the signature's
     * order. */
    exarmo_aarch64_intrinsic_source arguments[EXARMO_AARCH64_MAX_INTRINSIC_ARGUMENTS];
    /* Where the result comes from. */
    exarmo_aarch64_intrinsic_output result;
} exarmo_aarch64_intrinsic;

/* Write the intrinsics the instruction realises into `out`, up to
 * `capacity` of them. Returns how many it has, or zero for an instruction
 * the table does not name.
 *
 * The instruction cannot always tell them apart, since signed and unsigned
 * bytes are the same bytes. Where several are listed they are ordered by
 * what a reader would write first:
 *
 * 1. Types as wide as the registers the instruction reads: `vqtbl1_u8`
 *    before `vtbl1_u8`, whose table is half the register.
 * 2. One kind of element in every register: `vbslq_u8` before `vbslq_s8`,
 *    whose mask is unsigned.
 * 3. Signed integers, then unsigned, floating point, polynomial, 16-bit
 *    brain float and 8-bit float.
 * 4. Elements as wide as the arrangement's: `vandq_s8` before `vandq_s16`.
 *
 * `out[0]` is the one to show, and the rest are there for a caller that
 * would rather choose.
 *
 * The list is every intrinsic the instruction's own operands allow, so a
 * list of one is the only one it can be rather than a choice made for the
 * caller.
 *
 * Each one carries its own arguments, and they differ. CMGT is vcgt with
 * its operands one way round and vclt with them the other, and ADDP is
 * vpadd, which takes two vectors, and vaddv, which takes one. A caller that
 * shows an intrinsic other than `out[0]` reads that one's `arguments`. */
size_t exarmo_aarch64_instruction_intrinsics(const exarmo_aarch64_instruction *inst, exarmo_aarch64_intrinsic *out,
                                      size_t capacity);

/* What family of value a C type holds. */
typedef enum exarmo_aarch64_intrinsic_type_kind {
    /* Nothing, the result of a store. */
    EXARMO_AARCH64_INTRINSIC_KIND_VOID = 0,
    /* A signed integer, such as `int8x16_t` or `int64_t`. */
    EXARMO_AARCH64_INTRINSIC_KIND_INT = 1,
    /* An unsigned integer, such as `uint8x16_t`. */
    EXARMO_AARCH64_INTRINSIC_KIND_UINT = 2,
    /* An IEEE float, such as `float32x4_t`. */
    EXARMO_AARCH64_INTRINSIC_KIND_FLOAT = 3,
    /* A polynomial, such as `poly8x16_t`. */
    EXARMO_AARCH64_INTRINSIC_KIND_POLY = 4,
    /* A 16-bit brain float, such as `bfloat16x8_t`. */
    EXARMO_AARCH64_INTRINSIC_KIND_BRAIN_FLOAT = 5,
    /* An 8-bit float, such as `mfloat8x16_t`. */
    EXARMO_AARCH64_INTRINSIC_KIND_MICRO_FLOAT = 6,
    /* A value the intrinsic takes at compile time, `const int`, such as a
     * lane index, whose bound the instruction's field sets. */
    EXARMO_AARCH64_INTRINSIC_KIND_CONST_INT = 7,
    /* The floating-point mode register's value, `fpm_t`. */
    EXARMO_AARCH64_INTRINSIC_KIND_FPM = 8,
} exarmo_aarch64_intrinsic_type_kind;

/* What one exarmo_aarch64_intrinsic_type is, read apart so that a consumer
 * building its own type for each need not parse the name. An `int8x16_t`
 * is a signed 8-bit element with 16 lanes, and a `float32x2x4_t` is four
 * vectors of two, the structure a multi-register load writes. */
typedef struct exarmo_aarch64_intrinsic_type_def {
    /* The name as ACLE writes it, such as `int8x16_t`. */
    exarmo_aarch64_str name;
    uint8_t kind; /* exarmo_aarch64_intrinsic_type_kind */
    /* How many elements, 1 for a scalar. */
    uint8_t lanes;
    /* How many such vectors, 1, or 2 to 4 for a structure. */
    uint8_t vectors;
    /* Whether the argument is a pointer to that. */
    bool pointer;
    /* Whether that pointer is to a constant, as a load's is and a store's
     * is not. */
    bool readonly;
    /* How wide one element is, in bits, or zero for `void`. The whole value
     * is element_bits * lanes * vectors wide. */
    uint16_t element_bits;
} exarmo_aarch64_intrinsic_type_def;

/* Write what the type `ty` is into `out`, and return true. Returns false,
 * leaving `out` untouched, if `ty` is not one of them. Every one is below
 * EXARMO_AARCH64_INTRINSIC_TYPE_COUNT. The name points into the library and
 * lives as long as it does. */
bool exarmo_aarch64_intrinsic_type_at(exarmo_aarch64_intrinsic_type ty, exarmo_aarch64_intrinsic_type_def *out);

/* An intrinsic as ACLE declares it, apart from any instruction.
 *
 * ACLE's parameter names, `a`, `b`, `c` and the like, are not kept. What a
 * parameter means is in its type and in where the instruction holds it. */
typedef struct exarmo_aarch64_intrinsic_def {
    /* The name, such as `vmla_lane_s16`. */
    exarmo_aarch64_str name;
    /* The result's type, EXARMO_AARCH64_INTRINSIC_TYPE_VOID for a store. */
    exarmo_aarch64_intrinsic_type result;
    /* How many of `parameters` are filled. */
    uint8_t parameter_count;
    /* Each parameter's type, in order. */
    exarmo_aarch64_intrinsic_type parameters[EXARMO_AARCH64_MAX_INTRINSIC_ARGUMENTS];
} exarmo_aarch64_intrinsic_def;

/* Write what `id` names into `out`, and return true. Returns false,
 * leaving `out` untouched, for an `id` the table does not name. Every `id`
 * is below EXARMO_AARCH64_INTRINSIC_COUNT. Every string points into the
 * library and lives as long as it does. */
bool exarmo_aarch64_intrinsic_at(uint32_t id, exarmo_aarch64_intrinsic_def *out);

/* A system register the architecture names. */
typedef struct exarmo_aarch64_sysreg_def {
    /* The encoding, as exarmo_aarch64_sysreg carries it. */
    uint16_t encoding;
    /* Whether MRS can read it under this name, and whether MSR can write
     * it. */
    bool readable;
    bool writable;
    /* The name, in lower case. */
    exarmo_aarch64_str name;
} exarmo_aarch64_sysreg_def;

/* Every system register the architecture names, in encoding order. A
 * register with the same name in both directions appears once, and one with
 * a different name in each direction once per name.
 *
 * The table is a constant of the library. Read it where it sits, and read
 * its names for as long as the library is loaded. */
extern const exarmo_aarch64_sysreg_def exarmo_aarch64_sysregs[EXARMO_AARCH64_SYSREG_COUNT];

/* Whether a system operation takes a general-purpose register, as the
 * SysReg page documenting it lays that register out. */
typedef enum exarmo_aarch64_sysop_reg_use {
    /* It takes none, as IC IALLUIS does. Rt is 0b11111 and the assembly
     * writes no register. */
    EXARMO_AARCH64_SYSOP_REG_NONE = 0,
    /* The register carries something only where a feature is implemented,
     * as for TLBI VMALLE1IS on FEAT_TLBID. The assembly writes it where it
     * is not XZR. */
    EXARMO_AARCH64_SYSOP_REG_OPTIONAL = 1,
    /* It takes one, which the assembly always writes, as DC ZVA, GICR CDIA
     * and TLBIP VAE1IS do. */
    EXARMO_AARCH64_SYSOP_REG_REQUIRED = 2,
} exarmo_aarch64_sysop_reg_use;

/* Whether a system operation reads its register or writes it, as the
 * pseudocode on its page does. */
typedef enum exarmo_aarch64_sysop_reg_access {
    /* TLBI VAE1IS reads the address it invalidates. */
    EXARMO_AARCH64_SYSOP_REG_READ = 0,
    /* GICR CDIA writes what it acknowledges. */
    EXARMO_AARCH64_SYSOP_REG_WRITE = 1,
} exarmo_aarch64_sysop_reg_access;

/* A system operation or PSTATE field the architecture names.
 *
 * exarmo_aarch64_sysregs gives the registers MRS and MSR name. The rest of
 * the system instruction space is named the same way and reached through
 * other instructions. AT, DC, IC and TLBI name an operation, and MSR names
 * a PSTATE field, each by a word rather than by an encoding. A consumer
 * numbering the whole space in one place needs those under their encodings
 * too.
 *
 * What a decode reports for one of these is the operand's own field, which
 * is a different concatenation for each instruction. TLBI's holds CRn where
 * AT's, DC's and IC's do not, and for most PSTATE fields it holds MSR's
 * immediate, so `msr daifset, #3` and `msr daifset, #5` report different
 * bits for the same field. The operand's bits are therefore no identity,
 * and this table is where the encoding behind a name is read. */
typedef struct exarmo_aarch64_sysop_def {
    /* The op0:op1:CRn:CRm:op2 encoding, packed as
     * exarmo_aarch64_sysreg_def's is. op0 is 0 or 1 here and 2 or 3 for a
     * register, so no operation and no register come to one number:
     * amair_el1 is 0xC518 where PLBI's vmalle1is is 0x4518.
     *
     * It says where in that space an operation sits, not which row this is.
     * TLBI and TLBIP reach the same point by the same five fields under the
     * same name, and are told apart by the instruction carrying them, so 120
     * of these encodings name two rows. Key on `instruction` and `name` to
     * name one row. */
    uint16_t encoding;
    /* Which instruction names it, such as AT, DC, IC, TLBI or MSR. */
    exarmo_aarch64_mnemonic instruction;
    /* Which mechanism names it, 1 for an operation SYS reaches and 0 for a
     * PSTATE field MSR writes. */
    uint8_t op0;
    /* The rest of the encoding, each field as the instruction holds it. */
    uint8_t op1, crn, crm, op2;
    /* Which bits of `crm` name it, the rest being what the instruction
     * writes as its immediate.
     *
     * Every bit for an operation the architecture documents on a page of
     * its own, and none for most PSTATE fields, whose CRm is the whole of
     * MSR's immediate. SVCRSM and its neighbours are named by three of the
     * four bits and take the fourth as the immediate. */
    uint8_t crm_names;
    /* Whether the operation takes a general-purpose register, an
     * exarmo_aarch64_sysop_reg_use. A PSTATE field takes none. */
    uint8_t reg_use;
    /* Whether it reads that register or writes it, an
     * exarmo_aarch64_sysop_reg_access. READ where it takes none. */
    uint8_t reg_access;
    /* How many bits of the register move. That is 64, or 128 for TLBIP's
     * pair, of which the first register holds bits 0 to 63 and the second
     * bits 64 to 127, and 0 where it takes none.
     *
     * Every row of one instruction that takes a register has the same
     * reg_access and reg_bits, which generation checks. So a consumer
     * declaring one signature per instruction reads them off any row whose
     * reg_use is not NONE. */
    uint8_t reg_bits;
    /* The name the assembly writes, in lower case. */
    exarmo_aarch64_str name;
} exarmo_aarch64_sysop_def;

/* Every system operation and PSTATE field, in encoding order and then by
 * name. A constant of the library, as exarmo_aarch64_sysregs is. */
extern const exarmo_aarch64_sysop_def exarmo_aarch64_sysops[EXARMO_AARCH64_SYSOP_COUNT];

/* Write a system register's name into `name`, NUL-terminated when
 * `capacity` allows. It is the architectural name where the encoding has
 * one for that direction of access, else the `s<op0>_<op1>_c<n>_c<m>_<op2>`
 * form. Returns the length of the whole name without the NUL.
 *
 * `encoding` is what exarmo_aarch64_sysreg carries, op0 as the two bits it
 * is, so NZCV is 0xDA10. */
size_t exarmo_aarch64_sysreg_name(uint16_t encoding, bool write, char *name, size_t capacity);

#ifdef __cplusplus
}
#endif

#endif
