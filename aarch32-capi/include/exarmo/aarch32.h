/* The C face of the exarmo-aarch32 disassembler.
 *
 * Decode an instruction once, with exarmo_aarch32_decode_a32_word for an A32 word or
 * exarmo_aarch32_decode_t32_word for a T32 one, then ask the decoded instruction what
 * it is (its encoding and mnemonic), how long it is, what it does to the
 * condition flags, what its operands are, and how it is written. Nothing
 * here allocates. The caller owns every buffer, and a function that writes
 * into one returns how much it needed, so a buffer too small is detected
 * rather than overrun.
 *
 * Every function is safe to call from any thread, and none can fail
 * except as its documentation says. An internal failure is reported, never
 * propagated, and never leaves the process.
 *
 * exarmo_aarch32_encoding, exarmo_aarch32_mnemonic and the kinds of shift are
 * generated from the architecture's XML into aarch32_generated.h.
 */
#ifndef EXARMO_AARCH32_H
#define EXARMO_AARCH32_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "aarch32_generated.h"

#ifdef __cplusplus
extern "C" {
#endif

#if defined(__GNUC__) || defined(__clang__)
#define EXARMO_AARCH32_DEPRECATED(message) __attribute__((deprecated(message)))
#elif defined(_MSC_VER)
#define EXARMO_AARCH32_DEPRECATED(message) __declspec(deprecated(message))
#else
#define EXARMO_AARCH32_DEPRECATED(message)
#endif

/* ------------------------------------------------------------------------
 * Decoding
 * ---------------------------------------------------------------------- */

/* A decoded instruction of either instruction set. Opaque, so read it
 * through the functions below. It has no address. The functions that need
 * one take it as a parameter. */
typedef struct exarmo_aarch32_instruction {
    uint64_t opaque[5];
} exarmo_aarch32_instruction;

/* How many bytes of storage a decode writes, as the library was built. A
 * caller linked against a library from another release compares this with
 * sizeof(exarmo_aarch32_instruction) to check its storage still holds an
 * instruction. */
size_t exarmo_aarch32_instruction_size(void);

typedef enum exarmo_aarch32_status {
    /* The instruction decoded. */
    EXARMO_AARCH32_STATUS_OK = 0,
    /* The encoding is allocated to no instruction. */
    EXARMO_AARCH32_STATUS_UNALLOCATED = 1,
    /* The instruction's decode says it is UNDEFINED. */
    EXARMO_AARCH32_STATUS_UNDEFINED = 2,
    /* The instruction's decode says it executes as a NOP. No encoding in
     * this release's bundles reaches this. */
    EXARMO_AARCH32_STATUS_NOP = 3,
    /* The library failed internally. Nothing was written. */
    EXARMO_AARCH32_STATUS_FAILED = 4,
    /* The encoding index marks the encoding UNPREDICTABLE. */
    EXARMO_AARCH32_STATUS_UNPREDICTABLE = 5,
    /* The encoding is a hint the architecture reserves, which behaves as a
     * NOP and may be allocated by a later release. */
    EXARMO_AARCH32_STATUS_RESERVED_HINT = 6,
    /* The bytes ended before the instruction did. Only
     * exarmo_aarch32_decode_a32_bytes and exarmo_aarch32_decode_t32_bytes
     * return this. */
    EXARMO_AARCH32_STATUS_TRUNCATED = 7,
} exarmo_aarch32_status;

/* Decode the 32-bit A32 instruction word `bits` into `out`. `out` is
 * written only when the status is EXARMO_AARCH32_STATUS_OK. */
exarmo_aarch32_status exarmo_aarch32_decode_a32_word(uint32_t bits, exarmo_aarch32_instruction *out);

/* The former name of exarmo_aarch32_decode_a32_word. */
EXARMO_AARCH32_DEPRECATED("renamed to exarmo_aarch32_decode_a32_word, or use exarmo_aarch32_decode_a32_bytes")
exarmo_aarch32_status exarmo_aarch32_decode_a32(uint32_t bits, exarmo_aarch32_instruction *out);

/* Decode the A32 instruction at the start of the `len` bytes at `bytes` into
 * `out`. Returns EXARMO_AARCH32_STATUS_TRUNCATED if `len` is less than four,
 * and EXARMO_AARCH32_STATUS_FAILED if `bytes` is null and `len` is not zero,
 * or if `out` is null. `out` is written only when the status is
 * EXARMO_AARCH32_STATUS_OK. */
exarmo_aarch32_status exarmo_aarch32_decode_a32_bytes(const uint8_t *bytes, size_t len,
                                                      exarmo_aarch32_instruction *out);

/* Whether a T32 instruction is inside an IT block. A 16-bit T32 encoding
 * has no condition field. The condition it runs under comes from a
 * preceding IT instruction, so a decode is told it. */
typedef enum exarmo_aarch32_it_kind {
    /* The caller has not tracked the blocks. Read as being outside one,
     * which is what Binary Ninja's own ARMv7 plugin does with its
     * IFTHEN_UNKNOWN. */
    EXARMO_AARCH32_IT_UNKNOWN = 0,
    /* Outside any block, so the instruction is unconditional. */
    EXARMO_AARCH32_IT_OUTSIDE = 1,
    /* Inside a block, under `cond`, with `mask` of the block left. */
    EXARMO_AARCH32_IT_INSIDE = 2,
} exarmo_aarch32_it_kind;

typedef struct exarmo_aarch32_it_state {
    uint8_t kind; /* exarmo_aarch32_it_kind */
    /* Inside a block, the condition this instruction runs under, an
     * exarmo_aarch32_cond. */
    uint8_t cond;
    /* Inside a block, what is left of it, as the low four bits of the
     * architecture's ITSTATE. The lowest set bit marks the block's end, so
     * 0x8 is the last instruction. Each bit above it says whether the next
     * instruction's condition is this one (the bit equals the low bit of
     * `cond`) or its inverse. A caller that knows only whether the
     * instruction is the last passes 0x8 for the last and 0x4 otherwise. */
    uint8_t mask;
} exarmo_aarch32_it_state;

/* Decode the T32 instruction `bits` into `out`, under the IT state `state`.
 * `bits` holds the first halfword in its high half and the second, for a
 * 32-bit instruction, in its low half. A 16-bit instruction's low half is
 * ignored. `out` is written only when the status is EXARMO_AARCH32_STATUS_OK. */
exarmo_aarch32_status exarmo_aarch32_decode_t32_word(uint32_t bits, exarmo_aarch32_it_state state,
                                                     exarmo_aarch32_instruction *out);

/* The former name of exarmo_aarch32_decode_t32_word. */
EXARMO_AARCH32_DEPRECATED("renamed to exarmo_aarch32_decode_t32_word, or use exarmo_aarch32_decode_t32_bytes")
exarmo_aarch32_status exarmo_aarch32_decode_t32(uint32_t bits, exarmo_aarch32_it_state state,
                                                exarmo_aarch32_instruction *out);

/* Decode the T32 instruction at the start of the `len` bytes at `bytes`,
 * under the IT state `state`, into `out`. Reads either two or four bytes,
 * depending on the size encoded in the first 16 bits of the instruction, and
 * returns EXARMO_AARCH32_STATUS_TRUNCATED if the bytes end partway through
 * it. After a successful decode, exarmo_aarch32_instruction_length gives the
 * instruction's size, and after a failed one, exarmo_aarch32_t32_length
 * does. Returns EXARMO_AARCH32_STATUS_FAILED if `bytes` is null and `len` is
 * not zero, or if `out` is null. `out` is written only when the status is
 * EXARMO_AARCH32_STATUS_OK. */
exarmo_aarch32_status exarmo_aarch32_decode_t32_bytes(const uint8_t *bytes, size_t len, exarmo_aarch32_it_state state,
                                                      exarmo_aarch32_instruction *out);

/* How many bytes the T32 instruction beginning with the halfword `hw1`
 * takes, 2 or 4. The first halfword alone decides it, so a caller holding
 * only two bytes asks this whether it may decode. */
uint8_t exarmo_aarch32_t32_length(uint16_t hw1);

/* The IT state of the instruction that follows `inst` in straight-line
 * execution, given that `inst` was decoded under `state`. An IT instruction
 * begins its block. Inside a block the state advances as the architecture's
 * ITAdvance has it, and the block ends after its last instruction. A branch
 * out of a block is the caller's to track. An unknown state stays unknown
 * until an IT instruction settles it. */
exarmo_aarch32_it_state exarmo_aarch32_it_state_after(exarmo_aarch32_it_state state, const exarmo_aarch32_instruction *inst);

/* How many bytes of the instruction stream the instruction takes. 4 for an
 * A32 instruction, 2 or 4 for a T32 one, and 0 for a null pointer. The text
 * never writes an assembler's `.n` or `.w`, so this is the only place the
 * width shows. */
uint8_t exarmo_aarch32_instruction_length(const exarmo_aarch32_instruction *inst);

/* Whether the word `inst` was decoded from is CONSTRAINED UNPREDICTABLE: it
 * reached a test its decode pseudocode calls UNPREDICTABLE, or a bit its
 * diagram writes (0) or (1) holds the other value. The architecture then
 * permits one of a documented set of behaviours, and the instruction is still
 * written as the word says. False for a null pointer. */
bool exarmo_aarch32_instruction_unpredictable(const exarmo_aarch32_instruction *inst);

/* ------------------------------------------------------------------------
 * Identity
 * ---------------------------------------------------------------------- */

/* A string that is not NUL-terminated, `length` bytes at `data`. */
typedef struct exarmo_aarch32_str {
    const char *data;
    size_t length;
} exarmo_aarch32_str;

/* Which encoding the instruction is, or EXARMO_AARCH32_ENCODING_COUNT for a
 * null pointer. */
exarmo_aarch32_encoding exarmo_aarch32_instruction_encoding(const exarmo_aarch32_instruction *inst);

/* The mnemonic the instruction is written with, or
 * EXARMO_AARCH32_MNEMONIC_COUNT for a null pointer. */
exarmo_aarch32_mnemonic exarmo_aarch32_instruction_mnemonic(const exarmo_aarch32_instruction *inst);

/* The mnemonic an encoding is written with, or EXARMO_AARCH32_MNEMONIC_COUNT
 * for a value that is no encoding. */
exarmo_aarch32_mnemonic exarmo_aarch32_encoding_mnemonic(exarmo_aarch32_encoding encoding);

/* An encoding's name, as aarch32_generated.h spells it after
 * EXARMO_AARCH32_ENC_. Empty for a value that is no encoding. */
exarmo_aarch32_str exarmo_aarch32_encoding_name(exarmo_aarch32_encoding encoding);

/* How many bytes an encoding is, 2 or 4, or 0 for a value that is no
 * encoding. */
uint8_t exarmo_aarch32_encoding_length(exarmo_aarch32_encoding encoding);

/* A mnemonic as the assembly writes it, in lower case. Empty for a value
 * that is no mnemonic. */
exarmo_aarch32_str exarmo_aarch32_mnemonic_name(exarmo_aarch32_mnemonic mnemonic);

/* ------------------------------------------------------------------------
 * Condition flags
 * ---------------------------------------------------------------------- */

/* The condition flags, one bit each, as they sit in NZCV. */
enum {
    EXARMO_AARCH32_FLAG_N = 1 << 3,
    EXARMO_AARCH32_FLAG_Z = 1 << 2,
    EXARMO_AARCH32_FLAG_C = 1 << 1,
    EXARMO_AARCH32_FLAG_V = 1 << 0,
    EXARMO_AARCH32_FLAG_NZCV = 0xF,
};

/* What an instruction does with the condition flags, as its pseudocode
 * says. */
typedef struct exarmo_aarch32_flag_effect {
    /* The flags it writes, as EXARMO_AARCH32_FLAG_ bits. */
    uint8_t writes;
    /* The flags it reads. All four through a condition other than AL, or
     * the carry alone. */
    uint8_t reads;
    /* Whether the flags written come from comparing floating-point values,
     * which sets them differently from an integer comparison. */
    bool float_compare;
} exarmo_aarch32_flag_effect;

exarmo_aarch32_flag_effect exarmo_aarch32_instruction_flags(const exarmo_aarch32_instruction *inst);

/* ------------------------------------------------------------------------
 * Flow of control
 * ---------------------------------------------------------------------- */

/* What kind of branch an instruction takes, as its Execute pseudocode
 * says. This is the `BranchType` it carries, or the exception it raises.
 * Numbered as the AArch64 library numbers them. */
typedef enum exarmo_aarch32_branch_kind {
    EXARMO_AARCH32_BRANCH_NONE = 0,
    /* To an address it names, `BranchType_DIR`. */
    EXARMO_AARCH32_BRANCH_DIRECT = 1,
    /* To an address it computes, `BranchType_INDIR`. Writing the PC as a
     * register is one too, as in `ldr pc, [r1]` or `pop {r4, pc}`. */
    EXARMO_AARCH32_BRANCH_INDIRECT = 2,
    /* A call to an address it names, `BranchType_DIRCALL`. */
    EXARMO_AARCH32_BRANCH_DIRECT_CALL = 3,
    /* A call to an address it computes, `BranchType_INDCALL`. */
    EXARMO_AARCH32_BRANCH_INDIRECT_CALL = 4,
    /* A return from a call, `BranchType_RET`. */
    EXARMO_AARCH32_BRANCH_RETURN = 5,
    /* A return from an exception, restoring the state the exception saved.
     * ERET and RFE return so, as does writing the PC with the flags set, as
     * in `movs pc, lr`. */
    EXARMO_AARCH32_BRANCH_EXCEPTION_RETURN = 6,
    /* An exception whose handler returns to the instruction after it, as
     * SVC, HVC and SMC take. */
    EXARMO_AARCH32_BRANCH_SYSTEM_CALL = 7,
    /* An exception, as BKPT and UDF raise. */
    EXARMO_AARCH32_BRANCH_EXCEPTION = 8,
    /* A halt into debug state, as HLT does. */
    EXARMO_AARCH32_BRANCH_HALT = 9,
} exarmo_aarch32_branch_kind;

/* What an instruction does to the flow of control, as it reads at an
 * address.
 *
 * An instruction that writes a register branches only when that register is
 * the PC, though the encoding is the same either way, so `ldr r0, [r1]` does
 * not branch and `ldr pc, [r1]` does. When the
 * branch is not taken, execution continues at the address plus the
 * instruction's length. */
typedef struct exarmo_aarch32_branch {
    uint8_t kind; /* exarmo_aarch32_branch_kind */
    /* Whether the branch is taken only where a test succeeds, so that the
     * instruction after it runs where the test fails. The test is a
     * condition other than AL, a T32 instruction's condition inside an IT
     * block, or CBZ's test of a register. */
    bool conditional;
    /* Whether the instruction names the address it branches to, which a
     * branch through a register does not. */
    bool has_target;
    /* The address it branches to where `has_target`, and 0 otherwise. */
    uint64_t target;
} exarmo_aarch32_branch;

/* What the instruction does to the flow of control, as it would at
 * `address`. EXARMO_AARCH32_BRANCH_NONE for a null pointer. */
exarmo_aarch32_branch exarmo_aarch32_instruction_branch(const exarmo_aarch32_instruction *inst, uint64_t address);

/* ------------------------------------------------------------------------
 * Text
 * ---------------------------------------------------------------------- */

/* What a token is. The kinds are numbered as the AArch64 library numbers
 * them, so a consumer of both switches on one set. */
typedef enum exarmo_aarch32_token_kind {
    /* The mnemonic, or part of it, as in `add`, or `add` and then `eq`. */
    EXARMO_AARCH32_TOKEN_MNEMONIC = 0,
    /* Text that is none of the kinds below. The tab after the mnemonic, a
     * `.` before a data type, a `!` for writeback, and a `, ` within one
     * operand, as in `[r1, #0x8]` or `{r0, r1}`. */
    EXARMO_AARCH32_TOKEN_TEXT = 1,
    /* The `, ` between two operands, and only there, so a consumer can
     * number operands by counting these. Its operand is always
     * EXARMO_AARCH32_TOKEN_NO_OPERAND. */
    EXARMO_AARCH32_TOKEN_SEPARATOR = 2,
    /* A `[`, `]`, `{` or `}`. */
    EXARMO_AARCH32_TOKEN_BRACKET = 3,
    /* A register, as in `r0`, `sp` or `d1`. */
    EXARMO_AARCH32_TOKEN_REGISTER = 4,
    /* A name from a value table, such as a shift, a data type, a barrier
     * option or a special register. */
    EXARMO_AARCH32_TOKEN_SYMBOL = 5,
    /* An immediate, as in `0x8` or `-0x10`. `value` holds it as an
     * int64_t. The `#` before it is an EXARMO_AARCH32_TOKEN_TEXT token of
     * its own. */
    EXARMO_AARCH32_TOKEN_IMMEDIATE = 6,
    /* A number written bare, an element index or a coprocessor number.
     * `value` holds it. */
    EXARMO_AARCH32_TOKEN_INTEGER = 7,
    /* A branch target or a literal's address. `value` holds the address. */
    EXARMO_AARCH32_TOKEN_ADDRESS = 8,
    /* A floating-point immediate, as in `1.0`, after its `#`. */
    EXARMO_AARCH32_TOKEN_FLOAT = 9,
} exarmo_aarch32_token_kind;

/* What `operand` holds for a token that is part of no operand. Every
 * instruction has far fewer operands than this, so no index collides. */
#define EXARMO_AARCH32_TOKEN_NO_OPERAND 0xFF

/* One token of an instruction's text, a span of the text buffer the tokens
 * were written with. */
typedef struct exarmo_aarch32_token {
    exarmo_aarch32_token_kind kind;
    /* Where the token's text starts in the buffer, and how long it is. */
    uint32_t offset;
    uint32_t length;
    /* Which operand of exarmo_aarch32_instruction_operands the token is part
     * of, or EXARMO_AARCH32_TOKEN_NO_OPERAND for the mnemonic, the tab after
     * it and the `, ` between operands.
     *
     * This is what tells one `[` from another. A memory operand's brackets
     * are its own, while the brackets of a scalar's index belong to the
     * register they are written after. A register list written in braces is
     * one operand however many registers it names, its own separators
     * included.
     *
     * The condition and the width are written onto the mnemonic, and are
     * operands of the view all the same, so those tokens name one too. A
     * condition that is AL is written as nothing at all, so an
     * unconditional instruction writes no token for it. */
    uint8_t operand;
    /* The number an immediate, integer or address token writes, an
     * immediate as an int64_t and the rest as written. Zero for other
     * kinds. */
    uint64_t value;
} exarmo_aarch32_token;

/* Write the instruction as text, as it would be at `address`, into `text`,
 * NUL-terminated when `capacity` allows. Returns the length of the whole
 * text without the NUL, which is more than `capacity - 1` when the text was
 * cut short, or zero when the library failed. */
size_t exarmo_aarch32_instruction_text(const exarmo_aarch32_instruction *inst, uint64_t address, char *text,
                                size_t capacity);

/* How much exarmo_aarch32_instruction_tokens wrote, or would have. */
typedef struct exarmo_aarch32_text_size {
    /* How many tokens the instruction has. */
    size_t tokens;
    /* The length of the whole text without the NUL. */
    size_t length;
} exarmo_aarch32_text_size;

/* Write the instruction as tokens, the text into `text`, NUL-terminated
 * when `text_capacity` allows, and each token into `tokens`, up to
 * `token_capacity` of them.
 *
 * A caller that reserves EXARMO_AARCH32_MAX_TOKENS tokens and
 * EXARMO_AARCH32_MAX_TEXT + 1 bytes of text never has to ask for more,
 * whatever this release decodes. The result says how many tokens and how
 * much text the instruction has. Either is beyond its capacity when that
 * buffer was too small, and both are zero when the library failed. A token
 * cut short by the text buffer keeps its full length. */
exarmo_aarch32_text_size exarmo_aarch32_instruction_tokens(const exarmo_aarch32_instruction *inst, uint64_t address,
                                             char *text, size_t text_capacity,
                                             exarmo_aarch32_token *tokens, size_t token_capacity);

/* ------------------------------------------------------------------------
 * Operands
 * ---------------------------------------------------------------------- */

/* Which register file a register is in. */
typedef enum exarmo_aarch32_reg_class {
    /* A core register, r0 to r12, then sp, lr and pc as 13, 14 and 15. */
    EXARMO_AARCH32_REG_CORE = 0,
    /* SIMD and floating-point registers by width, s0 to s31, d0 to d31 and
     * q0 to q15. The three are views of one bank. s0 and s1 are the halves
     * of d0, and d0 and d1 the halves of q0. */
    EXARMO_AARCH32_REG_S = 1,
    EXARMO_AARCH32_REG_D = 2,
    EXARMO_AARCH32_REG_Q = 3,
} exarmo_aarch32_reg_class;

typedef struct exarmo_aarch32_reg {
    uint8_t class_; /* exarmo_aarch32_reg_class */
    uint8_t num;
} exarmo_aarch32_reg;

/* A value from a value table, such as a data type, a barrier option, an
 * endianness or a special register, as its bits and its name. */
typedef struct exarmo_aarch32_symbol {
    /* The bits the encoding holds, or zero for a name the template writes
     * itself. */
    uint16_t bits;
    /* The name the assembly writes, in lower case. Empty for a value the
     * table does not name. */
    exarmo_aarch32_str name;
} exarmo_aarch32_symbol;

/* A shift applied to a register or an immediate. */
typedef struct exarmo_aarch32_modifier {
    bool present;
    uint8_t kind; /* exarmo_aarch32_modifier_kind */
    /* How far, in bits. -1 where no amount is written, as for rrx, which
     * rotates through the carry by one, and for a shift by a register. */
    int32_t amount;
    /* The register holding how far, as in `lsl r3`, where `has_by`. */
    bool has_by;
    exarmo_aarch32_reg by;
} exarmo_aarch32_modifier;

/* A register operand, with what the assembly writes on it. */
typedef struct exarmo_aarch32_reg_operand {
    exarmo_aarch32_reg reg;
    /* An element index, as in `d0[1]`, or -1 where none is written. */
    int32_t index;
    /* A shift written after it, as in `r1, lsl #2`. */
    exarmo_aarch32_modifier modifier;
    /* Whether the register is written back, as in `r0!`. A load or store
     * of several registers writes back the base it steps. */
    bool writeback;
} exarmo_aarch32_reg_operand;

/* What a memory operand adds to its base. */
typedef enum exarmo_aarch32_offset_kind {
    EXARMO_AARCH32_OFFSET_NONE = 0,
    /* An immediate, in bytes, which may be negative. */
    EXARMO_AARCH32_OFFSET_IMM = 1,
    /* A register, shifted as the modifier says, added or subtracted. */
    EXARMO_AARCH32_OFFSET_REG = 2,
} exarmo_aarch32_offset_kind;

typedef struct exarmo_aarch32_offset {
    exarmo_aarch32_offset_kind kind;
    /* For EXARMO_AARCH32_OFFSET_IMM, the value, signed. */
    int64_t imm;
    /* For EXARMO_AARCH32_OFFSET_REG, the register, and how it is shifted. */
    exarmo_aarch32_reg reg;
    /* Whether the assembly writes the offset subtracted. For an immediate
     * this is its sign, except that a zero may be subtracted too. `[r0],
     * #-0` is a word of its own, with the U bit clear. */
    bool subtract;
    exarmo_aarch32_modifier modifier;
} exarmo_aarch32_offset;

/* Whether a memory operand's base is written back. */
typedef enum exarmo_aarch32_writeback {
    EXARMO_AARCH32_WRITEBACK_NONE = 0,
    /* Before the access, with the offset applied, as in `[r0, #8]!`. */
    EXARMO_AARCH32_WRITEBACK_PRE = 1,
    /* After the access, by the offset, as in `[r0], #8`. */
    EXARMO_AARCH32_WRITEBACK_POST = 2,
} exarmo_aarch32_writeback;

/* A memory operand, giving an address and whether the base is updated. */
typedef struct exarmo_aarch32_mem {
    exarmo_aarch32_reg base;
    exarmo_aarch32_offset offset;
    exarmo_aarch32_writeback writeback;
    /* The alignment written after the base, in bits, as in `[r0:64]`, or 0
     * where none is. */
    uint32_t align;
} exarmo_aarch32_mem;

/* Which register file a list holds. */
typedef enum exarmo_aarch32_list_file {
    /* Core registers, as LDM and PUSH write. */
    EXARMO_AARCH32_LIST_CORE = 0,
    /* Single-precision registers, as VLDM writes. */
    EXARMO_AARCH32_LIST_S = 1,
    /* Double-precision registers, as VLD1 writes. */
    EXARMO_AARCH32_LIST_D = 2,
} exarmo_aarch32_list_file;

/* What a list writes on each of its registers. */
typedef enum exarmo_aarch32_lane {
    /* The whole register, as in `{d0, d1}`. */
    EXARMO_AARCH32_LANE_WHOLE = 0,
    /* Every element, as a load to all lanes writes, `{d0[], d1[]}`. */
    EXARMO_AARCH32_LANE_ALL = 1,
    /* One element, which `index` names, as in `{d0[1], d1[1]}`. */
    EXARMO_AARCH32_LANE_INDEX = 2,
} exarmo_aarch32_lane;

/* A list of registers written in braces, as in `{r4, r5, lr}` or
 * `{d0-d3}`. */
typedef struct exarmo_aarch32_reg_list {
    /* Bit n set for register n of the file. */
    uint32_t mask;
    uint8_t file; /* exarmo_aarch32_list_file */
    uint8_t lane; /* exarmo_aarch32_lane */
    /* The element, for EXARMO_AARCH32_LANE_INDEX. */
    uint32_t index;
} exarmo_aarch32_reg_list;

/* A branch target or a literal's address, as an offset from the PC the
 * instruction reads. That PC is ahead of the instruction by 8 bytes in A32
 * and 4 in T32, and a literal load aligns it down to a word first. The
 * address named is ((address + pc_ahead) & ~(pc_align - 1)) + offset,
 * within 32 bits. */
typedef struct exarmo_aarch32_label {
    int64_t offset;
    uint8_t pc_ahead;
    uint32_t pc_align;
} exarmo_aarch32_label;

/* A floating-point immediate, the value and the pattern the encoding
 * holds where it holds one.
 *
 * A VMOV immediate is the pattern VFPExpandImm gives at the instruction's
 * width, so a consumer building a constant of that width reads the bits
 * rather than encoding the value back, which C cannot do for half
 * precision. Where `width` is zero there is no such pattern. VCMP against
 * zero writes its value in the template and has no field at all. */
typedef struct exarmo_aarch32_fp_imm {
    double value;
    /* The pattern the encoding holds, where `width` says there is one. */
    uint64_t bits;
    /* How wide that pattern is, 16, 32 or 64, or zero for none. */
    uint8_t width;
} exarmo_aarch32_fp_imm;

/* What an operand is, and which member of the union holds it. */
typedef enum exarmo_aarch32_operand_kind {
    /* A register, with what is written on it, in `reg`. */
    EXARMO_AARCH32_OPERAND_REG = 0,
    /* An immediate, with any shift written after it, in `imm`. */
    EXARMO_AARCH32_OPERAND_IMM = 1,
    EXARMO_AARCH32_OPERAND_FP_IMM = 2,
    /* A branch target or a literal's address, in `label`. */
    EXARMO_AARCH32_OPERAND_LABEL = 3,
    EXARMO_AARCH32_OPERAND_MEM = 4,
    EXARMO_AARCH32_OPERAND_LIST = 5,
    /* The condition the instruction is written with, in `cond`. AL is held
     * too, so the view has the same shape whether or not a condition is
     * written. */
    EXARMO_AARCH32_OPERAND_COND = 6,
    /* A value from a value table, in `symbol`. */
    EXARMO_AARCH32_OPERAND_SYMBOL = 7,
    /* A shift written as an operand of its own, applying to the operand
     * before it, in `modifier`. */
    EXARMO_AARCH32_OPERAND_MODIFIER = 8,
    /* An operand the library does not describe. */
    EXARMO_AARCH32_OPERAND_OTHER = 9,
} exarmo_aarch32_operand_kind;

/* One operand, as the assembly writes it. */
typedef struct exarmo_aarch32_operand {
    exarmo_aarch32_operand_kind kind;
    union {
        exarmo_aarch32_reg_operand reg;
        struct {
            int64_t value;
            exarmo_aarch32_modifier modifier;
        } imm;
        exarmo_aarch32_fp_imm fp_imm;
        exarmo_aarch32_label label;
        exarmo_aarch32_mem mem;
        exarmo_aarch32_reg_list list;
        /* An exarmo_aarch32_cond, which numbers each condition by the bits
         * a `cond` field holds for it. */
        uint8_t cond;
        exarmo_aarch32_symbol symbol;
        exarmo_aarch32_modifier modifier;
    };
} exarmo_aarch32_operand;

/* Write the instruction's operands, in the order the assembly writes them,
 * into `out`, up to `capacity` of them. Returns how many operands the
 * instruction has, which is never more than EXARMO_AARCH32_MAX_OPERANDS. */
size_t exarmo_aarch32_instruction_operands(const exarmo_aarch32_instruction *inst, exarmo_aarch32_operand *out,
                                    size_t capacity);

/* ------------------------------------------------------------------------
 * System registers
 * ---------------------------------------------------------------------- */

/* A system register an MRC, MCR, MRRC or MCRR accesses, by its key in the
 * table of the SysReg index that names it. The text is written as the
 * architecture writes it, `mrc p15, #0x0, r0, c1, c0, #0x0`, and the name is
 * for a caller to annotate it with. */
typedef struct exarmo_aarch32_sysreg {
    /* Which table names it, an exarmo_aarch32_sysreg_space. */
    uint8_t space;
    /* Whether the instruction writes it. */
    bool write;
    /* The instruction's fields run together as the table orders them.
     * coproc:opc1:CRn:CRm:opc2 for MRC and MCR, and coproc:opc1:CRm for
     * MRRC and MCRR, where coproc is 14 or 15. SCTLR is 0x3C080. */
    uint32_t encoding;
} exarmo_aarch32_sysreg;

/* Write the system register the instruction accesses into `out` and return
 * true. Returns false for an instruction that reaches none, leaving `out`
 * untouched. */
bool exarmo_aarch32_instruction_sysreg(const exarmo_aarch32_instruction *inst, exarmo_aarch32_sysreg *out);

/* The name the architecture gives the register with this key, accessed in
 * this direction, or empty where it gives none. Two registers can share a
 * key where one is read-only and the other write-only, so the direction is
 * part of the lookup. */
exarmo_aarch32_str exarmo_aarch32_sysreg_name(uint8_t space, uint32_t encoding, bool write);

/* A system register the architecture names. */
typedef struct exarmo_aarch32_sysreg_def {
    /* Which table names it, an exarmo_aarch32_sysreg_space. */
    uint8_t space;
    /* Whether it can be read under this name, and whether written. */
    bool readable;
    bool writable;
    /* The key, as exarmo_aarch32_sysreg carries it. */
    uint32_t encoding;
    /* The name, in lower case. */
    exarmo_aarch32_str name;
} exarmo_aarch32_sysreg_def;

/* Every system register the index names for these instructions, a space at
 * a time and in key order within one. It is a constant of the library, so
 * read it in place. Its names stay valid as long as the library is
 * loaded. */
extern const exarmo_aarch32_sysreg_def exarmo_aarch32_sysregs[EXARMO_AARCH32_SYSREG_COUNT];

#ifdef __cplusplus
}
#endif

#endif
