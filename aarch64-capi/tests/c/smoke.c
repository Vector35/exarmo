/* The library as C sees it, through the header and the static library
 * alone. Exits non-zero, naming the check, at the first thing wrong.
 *
 * Nothing here allocates. Every table the library names is a constant it
 * holds, and everything written out goes into storage declared here. */
#include <stdio.h>
#include <string.h>

#include <exarmo/aarch64.h>

#define CHECK(cond)                                                    \
    do {                                                               \
        if (!(cond)) {                                                 \
            fprintf(stderr, "smoke.c:%d: %s\n", __LINE__, #cond);      \
            return 1;                                                  \
        }                                                              \
    } while (0)

int main(void) {
    /* The header's storage size and the library's are two literals that
     * nothing else holds together. Checked first, since everything below is
     * written through it. */
    CHECK(exarmo_aarch64_instruction_size() == sizeof(exarmo_aarch64_instruction));

    exarmo_aarch64_instruction inst;
    CHECK(exarmo_aarch64_decode_word(0x00010000, &inst) == EXARMO_AARCH64_STATUS_UNALLOCATED);

    /* ldr x0, [x1, #8] */
    CHECK(exarmo_aarch64_decode_word(0xF9400420, &inst) == EXARMO_AARCH64_STATUS_OK);
    CHECK(exarmo_aarch64_instruction_encoding(&inst) == EXARMO_AARCH64_ENC_Ldr64LdstPos);
    CHECK(exarmo_aarch64_instruction_mnemonic(&inst) == EXARMO_AARCH64_LDR);
    exarmo_aarch64_str name = exarmo_aarch64_mnemonic_name(EXARMO_AARCH64_LDR);
    CHECK(name.length == 3 && memcmp(name.data, "ldr", 3) == 0);
    /* An encoding names itself and its mnemonic without an instruction */
    exarmo_aarch64_str encoding = exarmo_aarch64_encoding_name(EXARMO_AARCH64_ENC_Ldr64LdstPos);
    CHECK(encoding.length == 12 &&
          memcmp(encoding.data, "Ldr64LdstPos", 12) == 0);
    CHECK(exarmo_aarch64_encoding_mnemonic(EXARMO_AARCH64_ENC_Ldr64LdstPos) == EXARMO_AARCH64_LDR);
    CHECK(exarmo_aarch64_encoding_length(EXARMO_AARCH64_ENC_Ldr64LdstPos) == 4);
    CHECK(exarmo_aarch64_instruction_length(&inst) == 4);

    /* The deprecated name still links and decodes the same instruction */
    exarmo_aarch64_instruction old;
    CHECK(exarmo_aarch64_decode(0xF9400420, &old) == EXARMO_AARCH64_STATUS_OK);
    CHECK(exarmo_aarch64_instruction_encoding(&old) == EXARMO_AARCH64_ENC_Ldr64LdstPos);

    /* The same ldr as bytes, then cut short */
    const uint8_t ldr[] = {0x20, 0x04, 0x40, 0xf9};
    CHECK(exarmo_aarch64_decode_bytes(ldr, sizeof ldr, &inst) == EXARMO_AARCH64_STATUS_OK);
    CHECK(exarmo_aarch64_instruction_encoding(&inst) == EXARMO_AARCH64_ENC_Ldr64LdstPos);
    CHECK(exarmo_aarch64_decode_bytes(ldr, 3, &inst) == EXARMO_AARCH64_STATUS_TRUNCATED);
    CHECK(!exarmo_aarch64_instruction_unpredictable(&inst));

    /* Sized as a caller should, so whatever this release decodes fits. */
    char text[EXARMO_AARCH64_MAX_TEXT + 1];
    size_t length = exarmo_aarch64_instruction_text(&inst, 0x1000, text, sizeof text);
    CHECK(strcmp(text, "ldr\tx0, [x1, #0x8]") == 0);
    CHECK(length == strlen(text));

    exarmo_aarch64_token tokens[EXARMO_AARCH64_MAX_TOKENS];
    exarmo_aarch64_text_size size =
        exarmo_aarch64_instruction_tokens(&inst, 0x1000, text, sizeof text, tokens, EXARMO_AARCH64_MAX_TOKENS);
    CHECK(size.tokens == 10);
    CHECK(tokens[0].kind == EXARMO_AARCH64_TOKEN_MNEMONIC);
    CHECK(tokens[2].kind == EXARMO_AARCH64_TOKEN_REGISTER);
    CHECK(strncmp(text + tokens[2].offset, "x0", tokens[2].length) == 0);
    /* the immediate is its number, after a # of its own */
    CHECK(tokens[7].kind == EXARMO_AARCH64_TOKEN_TEXT && text[tokens[7].offset] == '#');
    CHECK(tokens[8].kind == EXARMO_AARCH64_TOKEN_IMMEDIATE && tokens[8].value == 8);
    CHECK(strncmp(text + tokens[8].offset, "0x8", tokens[8].length) == 0);

    exarmo_aarch64_operand ops[EXARMO_AARCH64_MAX_OPERANDS];
    size_t count = exarmo_aarch64_instruction_operands(&inst, ops, EXARMO_AARCH64_MAX_OPERANDS);
    CHECK(count == 2);
    CHECK(ops[0].kind == EXARMO_AARCH64_OPERAND_REG);
    CHECK(ops[0].reg.reg.class_ == EXARMO_AARCH64_REG_X && ops[0].reg.reg.num == 0);
    CHECK(ops[1].kind == EXARMO_AARCH64_OPERAND_MEM);
    CHECK(ops[1].mem.base.class_ == EXARMO_AARCH64_REG_X_SP && ops[1].mem.base.num == 1);
    CHECK(ops[1].mem.offset.kind == EXARMO_AARCH64_OFFSET_IMM && ops[1].mem.offset.imm == 8);
    CHECK(ops[1].mem.writeback == EXARMO_AARCH64_WRITEBACK_NONE);

    /* add x0, x1, x2, lsl #3, whose shift is a kind to switch on */
    CHECK(exarmo_aarch64_decode_word(0x8B020C20, &inst) == EXARMO_AARCH64_STATUS_OK);
    count = exarmo_aarch64_instruction_operands(&inst, ops, EXARMO_AARCH64_MAX_OPERANDS);
    CHECK(count == 3 && ops[2].reg.modifier.present);
    CHECK(ops[2].reg.modifier.kind == EXARMO_AARCH64_MOD_LSL && ops[2].reg.modifier.amount == 3);

    /* adds x0, x1, x2 writes all four flags */
    CHECK(exarmo_aarch64_decode_word(0xAB020020, &inst) == EXARMO_AARCH64_STATUS_OK);
    exarmo_aarch64_flag_effect flags = exarmo_aarch64_instruction_flags(&inst);
    CHECK(flags.writes == EXARMO_AARCH64_FLAG_NZCV && flags.reads == 0);

    /* each token says which operand it is part of, so a memory operand's
     * brackets are told from a lane index's */
    CHECK(exarmo_aarch64_decode_word(0xF9400420, &inst) == EXARMO_AARCH64_STATUS_OK); /* ldr x0, [x1, #8] */
    size = exarmo_aarch64_instruction_tokens(&inst, 0, text, sizeof text, tokens, EXARMO_AARCH64_MAX_TOKENS);
    CHECK(size.tokens == 10);
    CHECK(tokens[0].operand == EXARMO_AARCH64_TOKEN_NO_OPERAND); /* the mnemonic */
    CHECK(tokens[2].operand == 0);                               /* x0 */
    CHECK(tokens[3].operand == EXARMO_AARCH64_TOKEN_NO_OPERAND); /* the `, ` */
    CHECK(tokens[4].operand == 1 && text[tokens[4].offset] == '[');
    /* the `, ` within the memory operand is its text */
    CHECK(tokens[6].kind == EXARMO_AARCH64_TOKEN_TEXT && tokens[6].operand == 1);
    CHECK(tokens[9].operand == 1 && text[tokens[9].offset] == ']');
    count = exarmo_aarch64_instruction_operands(&inst, ops, EXARMO_AARCH64_MAX_OPERANDS);
    CHECK(count == 2 && ops[1].kind == EXARMO_AARCH64_OPERAND_MEM);

    /* The system operations and PSTATE fields, with the five fields apart.
     * The table is a constant of the library, walked in place, and the
     * header says how long it is. */
    bool found_at = false, found_daifset = false, found_cdia = false, found_vae1is = false;
    for (size_t i = 0; i < EXARMO_AARCH64_SYSOP_COUNT; i++)
    {
        const exarmo_aarch64_sysop_def *sysop = &exarmo_aarch64_sysops[i];
        if (sysop->instruction == EXARMO_AARCH64_AT && sysop->name.length == 5
            && memcmp(sysop->name.data, "s1e1r", 5) == 0)
        {
            found_at = true;
            CHECK(sysop->op0 == 1 && sysop->op1 == 0 && sysop->crn == 7);
            CHECK(sysop->crm == 8 && sysop->op2 == 0 && sysop->crm_names == 0xF);
        }
        if (sysop->instruction == EXARMO_AARCH64_MSR && sysop->name.length == 7
            && memcmp(sysop->name.data, "daifset", 7) == 0)
        {
            found_daifset = true;
            /* CRm is the whole of MSR's immediate, so none of it names the field */
            CHECK(sysop->crm_names == 0);
            CHECK(sysop->reg_use == EXARMO_AARCH64_SYSOP_REG_NONE && sysop->reg_bits == 0);
        }
        if (sysop->instruction == EXARMO_AARCH64_GICR && sysop->name.length == 4
            && memcmp(sysop->name.data, "cdia", 4) == 0)
        {
            found_cdia = true;
            /* its result is written to the register */
            CHECK(sysop->reg_use == EXARMO_AARCH64_SYSOP_REG_REQUIRED);
            CHECK(sysop->reg_access == EXARMO_AARCH64_SYSOP_REG_WRITE && sysop->reg_bits == 64);
        }
        if (sysop->instruction == EXARMO_AARCH64_TLBIP && sysop->name.length == 6
            && memcmp(sysop->name.data, "vae1is", 6) == 0)
        {
            found_vae1is = true;
            /* the pair is read as one 128-bit value */
            CHECK(sysop->reg_access == EXARMO_AARCH64_SYSOP_REG_READ && sysop->reg_bits == 128);
        }
    }
    CHECK(found_at && found_daifset && found_cdia && found_vae1is);


    /* the branch an instruction takes, and where it goes */
    CHECK(exarmo_aarch64_decode_word(0x94000002, &inst) == EXARMO_AARCH64_STATUS_OK);
    exarmo_aarch64_branch branch = exarmo_aarch64_instruction_branch(&inst, 0x1000);
    CHECK(branch.kind == EXARMO_AARCH64_BRANCH_DIRECT_CALL && !branch.conditional);
    CHECK(branch.has_target && branch.target == 0x1008); /* bl .+8 */
    CHECK(exarmo_aarch64_decode_word(0xD65F03C0, &inst) == EXARMO_AARCH64_STATUS_OK);
    branch = exarmo_aarch64_instruction_branch(&inst, 0x1000);
    CHECK(branch.kind == EXARMO_AARCH64_BRANCH_RETURN && !branch.has_target);

    /* b.eq at 0x1000 names its target */
    CHECK(exarmo_aarch64_decode_word(0x54000020, &inst) == EXARMO_AARCH64_STATUS_OK);
    size = exarmo_aarch64_instruction_tokens(&inst, 0x1000, text, sizeof text, tokens, EXARMO_AARCH64_MAX_TOKENS);
    CHECK(tokens[size.tokens - 1].kind == EXARMO_AARCH64_TOKEN_ADDRESS);
    CHECK(tokens[size.tokens - 1].value == 0x1004);
    count = exarmo_aarch64_instruction_operands(&inst, ops, EXARMO_AARCH64_MAX_OPERANDS);
    CHECK(count == 2 && ops[0].kind == EXARMO_AARCH64_OPERAND_COND);
    CHECK(ops[1].kind == EXARMO_AARCH64_OPERAND_LABEL && ops[1].label.offset == 4);
    CHECK(ops[1].label.pc_ahead == 0 && ops[1].label.pc_align == 1);
    branch = exarmo_aarch64_instruction_branch(&inst, 0x1000);
    CHECK(branch.kind == EXARMO_AARCH64_BRANCH_DIRECT && branch.conditional);
    CHECK(branch.target == 0x1004);

    /* adrp at 0x1000 names the page its target is in */
    CHECK(exarmo_aarch64_decode_word(0xb0000000, &inst) == EXARMO_AARCH64_STATUS_OK);
    count = exarmo_aarch64_instruction_operands(&inst, ops, EXARMO_AARCH64_MAX_OPERANDS);
    CHECK(count == 2 && ops[1].kind == EXARMO_AARCH64_OPERAND_LABEL);
    CHECK(ops[1].label.pc_align == 4096);

    /* A condition operand carries the condition the instruction is written
     * under. cset reads the condition field inverted, so `cset w8, ne` holds
     * a zero there where a plainly read field's zero spells EQ. */
    CHECK(exarmo_aarch64_decode_word(0x1A9F07E8, &inst) == EXARMO_AARCH64_STATUS_OK);
    count = exarmo_aarch64_instruction_operands(&inst, ops, EXARMO_AARCH64_MAX_OPERANDS);
    CHECK(count == 2 && ops[1].kind == EXARMO_AARCH64_OPERAND_COND);
    CHECK(ops[1].cond == EXARMO_AARCH64_COND_NE);
    /* ccmp w3, #0, #2, eq reads it plainly */
    CHECK(exarmo_aarch64_decode_word(0x7A400862, &inst) == EXARMO_AARCH64_STATUS_OK);
    count = exarmo_aarch64_instruction_operands(&inst, ops, EXARMO_AARCH64_MAX_OPERANDS);
    CHECK(count == 4 && ops[3].kind == EXARMO_AARCH64_OPERAND_COND);
    CHECK(ops[3].cond == EXARMO_AARCH64_COND_EQ);

    /* An operand naming a system operation says which row of the table it
     * is, so nothing here looks a name up. `at s1e1wp, x10` */
    CHECK(exarmo_aarch64_decode_word(0xD508792A, &inst) == EXARMO_AARCH64_STATUS_OK);
    count = exarmo_aarch64_instruction_operands(&inst, ops, EXARMO_AARCH64_MAX_OPERANDS);
    CHECK(count == 2 && ops[0].kind == EXARMO_AARCH64_OPERAND_SYSOP);
    CHECK(ops[0].sysop.index < EXARMO_AARCH64_SYSOP_COUNT);
    {
        const exarmo_aarch64_sysop_def *named = &exarmo_aarch64_sysops[ops[0].sysop.index];
        CHECK(named->instruction == EXARMO_AARCH64_AT);
        CHECK(named->name.length == 6 && memcmp(named->name.data, "s1e1wp", 6) == 0);
        CHECK(named->op0 == 1 && named->op1 == 0 && named->crn == 7);
        CHECK(named->crm == 9 && named->op2 == 1);
    }

    /* The registers are a constant table too, in encoding order */
    CHECK(exarmo_aarch64_sysregs[0].name.length > 0);
    for (size_t i = 1; i < EXARMO_AARCH64_SYSREG_COUNT; i++)
        CHECK(exarmo_aarch64_sysregs[i - 1].encoding <= exarmo_aarch64_sysregs[i].encoding);

    char sysreg[32];
    /* NZCV is op0 3, op1 3, CRn 4, CRm 2, op2 0, packed with op0 whole */
    CHECK(exarmo_aarch64_sysreg_name(0xDA10, false, sysreg, sizeof sysreg) == 4);
    CHECK(strcmp(sysreg, "nzcv") == 0);

    /* add v0.16b, v1.16b, v0.16b is vaddq_s8 first, then vaddq_u8. The bytes
     * are the same signed or unsigned, and the signed spelling comes first. */
    CHECK(exarmo_aarch64_decode_word(0x4E208420, &inst) == EXARMO_AARCH64_STATUS_OK);
    exarmo_aarch64_intrinsic intrinsics[8];
    count = exarmo_aarch64_instruction_intrinsics(&inst, intrinsics, 8);
    CHECK(count == 2);
    exarmo_aarch64_intrinsic_def def;
    CHECK(exarmo_aarch64_intrinsic_at(intrinsics[0].id, &def));
    CHECK(def.name.length == 8 && memcmp(def.name.data, "vaddq_s8", 8) == 0);
    CHECK(def.parameter_count == 2 && intrinsics[0].argument_count == 2);
    /* a <- operands[1], b <- operands[2], result -> operands[0] */
    CHECK(intrinsics[0].arguments[0].kind == EXARMO_AARCH64_INTRINSIC_SOURCE_OPERAND);
    CHECK(intrinsics[0].arguments[0].operand == 1);
    CHECK(intrinsics[0].arguments[1].operand == 2);
    CHECK(intrinsics[0].result.kind == EXARMO_AARCH64_INTRINSIC_OUTPUT_OPERAND);
    CHECK(intrinsics[0].result.operand == 0);
    CHECK(exarmo_aarch64_intrinsic_at(intrinsics[1].id, &def));
    CHECK(memcmp(def.name.data, "vaddq_u8", 8) == 0);

    /* bfdot v1.2s, v2.4h, v16.2h[0] takes its lane from an operand's index */
    CHECK(exarmo_aarch64_decode_word(0x0F50F041, &inst) == EXARMO_AARCH64_STATUS_OK);
    count = exarmo_aarch64_instruction_intrinsics(&inst, intrinsics, 8);
    CHECK(count >= 1 && intrinsics[0].argument_count == 4);
    CHECK(intrinsics[0].arguments[3].kind == EXARMO_AARCH64_INTRINSIC_SOURCE_INDEX);
    CHECK(intrinsics[0].arguments[3].operand == 2);
    CHECK(exarmo_aarch64_intrinsic_at(intrinsics[0].id, &def));
    CHECK(def.parameter_count == 4);
    /* the lane is a compile-time constant, and `b` the whole of v16, an
     * 8-lane bfloat vector, since the index reaches every lane of it */
    CHECK(def.parameters[3] == EXARMO_AARCH64_INTRINSIC_TYPE_CONST_INT);
    exarmo_aarch64_intrinsic_type_def tdef;
    CHECK(exarmo_aarch64_intrinsic_type_at(def.parameters[2], &tdef));
    CHECK(tdef.kind == EXARMO_AARCH64_INTRINSIC_KIND_BRAIN_FLOAT);
    CHECK(tdef.element_bits == 16 && tdef.lanes == 8 && !tdef.pointer);

    /* An instruction ACLE does not name has none, and a bad id is refused */
    CHECK(exarmo_aarch64_decode_word(0xAB020020, &inst) == EXARMO_AARCH64_STATUS_OK);
    CHECK(exarmo_aarch64_instruction_intrinsics(&inst, intrinsics, 8) == 0);
    CHECK(!exarmo_aarch64_intrinsic_at(EXARMO_AARCH64_INTRINSIC_COUNT, &def));
    CHECK(!exarmo_aarch64_intrinsic_type_at(EXARMO_AARCH64_INTRINSIC_TYPE_COUNT, &tdef));
    CHECK(exarmo_aarch64_intrinsic_at(EXARMO_AARCH64_INTRINSIC_COUNT - 1, &def));
    return 0;
}
