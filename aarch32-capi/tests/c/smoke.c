/* The library as C sees it, through the header and the static library
 * alone. Exits non-zero, naming the check, at the first thing wrong.
 *
 * Nothing here allocates. Every table the library names is a constant it
 * holds, and everything written out goes into storage declared here. */
#include <stdio.h>
#include <string.h>

#include <exarmo/aarch32.h>

#define CHECK(cond)                                                    \
    do {                                                               \
        if (!(cond)) {                                                 \
            fprintf(stderr, "smoke.c:%d: %s\n", __LINE__, #cond);      \
            return 1;                                                  \
        }                                                              \
    } while (0)

int main(void) {
    /* The header's storage size and the library's are two literals that
     * nothing else holds together. Checked first because everything below
     * is written through it. */
    CHECK(exarmo_aarch32_instruction_size() == sizeof(exarmo_aarch32_instruction));

    exarmo_aarch32_instruction inst;
    CHECK(exarmo_aarch32_decode_a32(0xf1200000, &inst) == EXARMO_AARCH32_STATUS_UNALLOCATED);
    CHECK(exarmo_aarch32_decode_a32(0xf1200070, &inst) == EXARMO_AARCH32_STATUS_UNPREDICTABLE);
    CHECK(exarmo_aarch32_decode_a32(0xe320f020, &inst) == EXARMO_AARCH32_STATUS_RESERVED_HINT);

    /* ldr r1, [r0] */
    CHECK(exarmo_aarch32_decode_a32(0xe5901000, &inst) == EXARMO_AARCH32_STATUS_OK);
    CHECK(exarmo_aarch32_instruction_encoding(&inst) == EXARMO_AARCH32_ENC_LdrIA1Off);
    CHECK(exarmo_aarch32_instruction_mnemonic(&inst) == EXARMO_AARCH32_LDR);
    CHECK(exarmo_aarch32_instruction_length(&inst) == 4);
    CHECK(!exarmo_aarch32_instruction_unpredictable(&inst));
    exarmo_aarch32_str name = exarmo_aarch32_mnemonic_name(EXARMO_AARCH32_LDR);
    CHECK(name.length == 3 && memcmp(name.data, "ldr", 3) == 0);
    exarmo_aarch32_str encoding = exarmo_aarch32_encoding_name(EXARMO_AARCH32_ENC_LdrIA1Off);
    CHECK(encoding.length == 9 &&
          memcmp(encoding.data, "LdrIA1Off", 9) == 0);
    CHECK(exarmo_aarch32_encoding_mnemonic(EXARMO_AARCH32_ENC_LdrIA1Off) == EXARMO_AARCH32_LDR);
    CHECK(exarmo_aarch32_encoding_length(EXARMO_AARCH32_ENC_LdrIA1Off) == 4);

    char text[EXARMO_AARCH32_MAX_TEXT + 1];
    size_t length = exarmo_aarch32_instruction_text(&inst, 0x1000, text, sizeof text);
    CHECK(strcmp(text, "ldr\tr1, [r0]") == 0);
    CHECK(length == strlen(text));

    exarmo_aarch32_token tokens[EXARMO_AARCH32_MAX_TOKENS];
    exarmo_aarch32_text_size size =
        exarmo_aarch32_instruction_tokens(&inst, 0x1000, text, sizeof text, tokens, EXARMO_AARCH32_MAX_TOKENS);
    CHECK(size.tokens == 7);
    CHECK(tokens[0].kind == EXARMO_AARCH32_TOKEN_MNEMONIC);
    CHECK(tokens[2].kind == EXARMO_AARCH32_TOKEN_REGISTER);
    CHECK(strncmp(text + tokens[2].offset, "r1", tokens[2].length) == 0);
    CHECK(tokens[4].kind == EXARMO_AARCH32_TOKEN_BRACKET && text[tokens[4].offset] == '[');

    exarmo_aarch32_operand ops[EXARMO_AARCH32_MAX_OPERANDS];
    size_t count = exarmo_aarch32_instruction_operands(&inst, ops, EXARMO_AARCH32_MAX_OPERANDS);
    CHECK(count == 3);
    CHECK(ops[0].kind == EXARMO_AARCH32_OPERAND_COND);
    CHECK(ops[1].kind == EXARMO_AARCH32_OPERAND_REG);
    CHECK(ops[1].reg.reg.class_ == EXARMO_AARCH32_REG_CORE && ops[1].reg.reg.num == 1);
    CHECK(ops[2].kind == EXARMO_AARCH32_OPERAND_MEM);
    CHECK(ops[2].mem.base.class_ == EXARMO_AARCH32_REG_CORE && ops[2].mem.base.num == 0);
    CHECK(ops[2].mem.offset.kind == EXARMO_AARCH32_OFFSET_IMM && ops[2].mem.offset.imm == 0);
    CHECK(!ops[2].mem.offset.subtract);
    CHECK(ops[2].mem.writeback == EXARMO_AARCH32_WRITEBACK_NONE);

    /* add r0, r0, r1, lsl #3. The shift is folded onto the register it
     * applies to. */
    CHECK(exarmo_aarch32_decode_a32(0xe0800181, &inst) == EXARMO_AARCH32_STATUS_OK);
    count = exarmo_aarch32_instruction_operands(&inst, ops, EXARMO_AARCH32_MAX_OPERANDS);
    CHECK(count == 4 && ops[3].kind == EXARMO_AARCH32_OPERAND_REG);
    CHECK(ops[3].reg.modifier.kind == EXARMO_AARCH32_MOD_LSL
          && ops[3].reg.modifier.amount == 3);

    /* adds r0, r0, #1 writes all four flags and reads none */
    CHECK(exarmo_aarch32_decode_a32(0xe2900001, &inst) == EXARMO_AARCH32_STATUS_OK);
    exarmo_aarch32_flag_effect flags = exarmo_aarch32_instruction_flags(&inst);
    CHECK(flags.writes == EXARMO_AARCH32_FLAG_NZCV && flags.reads == 0);

    /* mrc p15, #0, r0, c1, c0, #0 reads SCTLR, which the table lists too */
    CHECK(exarmo_aarch32_decode_a32(0xee110f10, &inst) == EXARMO_AARCH32_STATUS_OK);
    exarmo_aarch32_sysreg reg;
    CHECK(exarmo_aarch32_instruction_sysreg(&inst, &reg) && reg.encoding == 0x3C080);
    exarmo_aarch32_str sysreg = exarmo_aarch32_sysreg_name(reg.space, reg.encoding, reg.write);
    CHECK(sysreg.length == 5 && strncmp(sysreg.data, "sctlr", 5) == 0);
    bool listed = false;
    for (size_t i = 0; i < EXARMO_AARCH32_SYSREG_COUNT; i++) {
        listed |= exarmo_aarch32_sysregs[i].encoding == 0x3C080
                  && exarmo_aarch32_sysregs[i].space == EXARMO_AARCH32_SYSREG_SPACE_MRCMCR;
    }
    CHECK(listed);

    /* bl at 0x1000 names its target, and ldr pc branches where ldr r0 does not */
    CHECK(exarmo_aarch32_decode_a32(0xeb000000, &inst) == EXARMO_AARCH32_STATUS_OK);
    exarmo_aarch32_branch branch = exarmo_aarch32_instruction_branch(&inst, 0x1000);
    CHECK(branch.kind == EXARMO_AARCH32_BRANCH_DIRECT_CALL && !branch.conditional);
    CHECK(branch.has_target && branch.target == 0x1008);
    CHECK(exarmo_aarch32_decode_a32(0xe591f000, &inst) == EXARMO_AARCH32_STATUS_OK);
    branch = exarmo_aarch32_instruction_branch(&inst, 0x1000);
    CHECK(branch.kind == EXARMO_AARCH32_BRANCH_INDIRECT && !branch.has_target);
    CHECK(exarmo_aarch32_decode_a32(0xe5910000, &inst) == EXARMO_AARCH32_STATUS_OK);
    branch = exarmo_aarch32_instruction_branch(&inst, 0x1000);
    CHECK(branch.kind == EXARMO_AARCH32_BRANCH_NONE);

    /* beq at 0x1000 names its target, 8 ahead of the instruction */
    CHECK(exarmo_aarch32_decode_a32(0x0a000000, &inst) == EXARMO_AARCH32_STATUS_OK);
    size = exarmo_aarch32_instruction_tokens(&inst, 0x1000, text, sizeof text, tokens, EXARMO_AARCH32_MAX_TOKENS);
    CHECK(tokens[size.tokens - 1].kind == EXARMO_AARCH32_TOKEN_ADDRESS);
    CHECK(tokens[size.tokens - 1].value == 0x1008);
    count = exarmo_aarch32_instruction_operands(&inst, ops, EXARMO_AARCH32_MAX_OPERANDS);
    CHECK(count == 2 && ops[0].kind == EXARMO_AARCH32_OPERAND_COND);
    CHECK(ops[1].kind == EXARMO_AARCH32_OPERAND_LABEL && ops[1].label.offset == 0);
    CHECK(ops[1].label.pc_ahead == 8);

    /* A T32 halfword says how long its instruction is before it is decoded */
    CHECK(exarmo_aarch32_t32_length(0x4408) == 2);
    CHECK(exarmo_aarch32_t32_length(0xf000) == 4);

    /* adds r0, r0, r1 outside an IT block, addeq r0, r0, r1 inside one */
    exarmo_aarch32_it_state outside = {EXARMO_AARCH32_IT_OUTSIDE, 0, 0};
    CHECK(exarmo_aarch32_decode_t32(0x18400000, outside, &inst) == EXARMO_AARCH32_STATUS_OK);
    CHECK(exarmo_aarch32_instruction_length(&inst) == 2);
    exarmo_aarch32_instruction_text(&inst, 0, text, sizeof text);
    CHECK(strcmp(text, "adds\tr0, r0, r1") == 0);
    exarmo_aarch32_it_state inside = {EXARMO_AARCH32_IT_INSIDE, 0, 0x8};
    CHECK(exarmo_aarch32_decode_t32(0x18400000, inside, &inst) == EXARMO_AARCH32_STATUS_OK);
    exarmo_aarch32_instruction_text(&inst, 0, text, sizeof text);
    CHECK(strcmp(text, "addeq\tr0, r0, r1") == 0);

    /* itett eq begins a block of four, and the state walks through it */
    CHECK(exarmo_aarch32_decode_t32(0xbf090000, outside, &inst) == EXARMO_AARCH32_STATUS_OK);
    exarmo_aarch32_it_state state = exarmo_aarch32_it_state_after(outside, &inst);
    CHECK(state.kind == EXARMO_AARCH32_IT_INSIDE && state.cond == EXARMO_AARCH32_COND_EQ && state.mask == 0x9);
    CHECK(exarmo_aarch32_decode_t32(0x18400000, state, &inst) == EXARMO_AARCH32_STATUS_OK);
    state = exarmo_aarch32_it_state_after(state, &inst);
    CHECK(state.kind == EXARMO_AARCH32_IT_INSIDE && state.cond == EXARMO_AARCH32_COND_NE && state.mask == 0x2);
    state = exarmo_aarch32_it_state_after(state, &inst);
    state = exarmo_aarch32_it_state_after(state, &inst);
    CHECK(state.kind == EXARMO_AARCH32_IT_INSIDE && state.cond == EXARMO_AARCH32_COND_EQ && state.mask == 0x8);
    state = exarmo_aarch32_it_state_after(state, &inst);
    CHECK(state.kind == EXARMO_AARCH32_IT_OUTSIDE);
    return 0;
}
