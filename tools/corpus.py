"""Seed the corpus with what llvm-mc writes for the words a sweep reached.

Reads the lines `tests/coverage.rs`'s sweep prints, `<set> <hex> <encoding>`,
adds the words in Binary Ninja's armv7 and thumb2 test suites, asks llvm-mc
what each word is, and adds each as a case, `<hex> <text>`, to the block of
`tests/cases/a32/` or `cases/t32/` whose forms the word decodes to. The blocks'
headers are the generator's, so the release `exarmo` says which form a word
is, and a word that decodes to none is left out.

    COVERAGE_PER=8 cargo test --release -p exarmo-aarch32 --test coverage -- \\
        --ignored --nocapture sweep > sweep.txt
    cargo build --release -p exarmo-cli
    python3 tools/corpus.py sweep.txt

llvm-mc's text is kept as it wrote it, lower-cased, apart from what this
disassembler will never write the same way:

- `.w` and `.n` are dropped. Nothing in an instruction determines them.
- A branch target is the address it names at the harness address, which is
  what this disassembler writes, rather than the offset llvm-mc writes.
- A literal load's `[pc, #imm]` is the address it names, since the
  architecture's own syntax for it is `<label>`.
- A modified immediate llvm-mc writes as its two fields, `#228, #10`, is
  written as the value those expand to, which is what an assembler takes.

`cs` and `cc` against `hs` and `lo`, and hexadecimal against decimal, are left
to the reader to fold, as the AArch64 corpus leaves them.
"""
import collections
import os
import pathlib
import re
import subprocess
import sys

sys.path.insert(0, str(pathlib.Path(__file__).parent))
from llvm import many

from fill import CORPORA, add, forms, headers

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parent
# Binary Ninja's own armv7 tests, for words a sweep does not reach. Say where
# the checkout is with BINJA_ARMV7. Nothing is read from it if it is unset.
BINJA = pathlib.Path(os.environ.get("BINJA_ARMV7", ""))
# The address every harness disassembles at, so a label names one target.
ADDRESS = 0x10000
FEATURES = "+bf16,+i8mm,+dotprod,+fp16fml,+fullfp16,+crypto,+ras,+sb,+crc"
TRIPLE = {"a32": "armv8.6a", "t32": "thumbv8.6a"}


# What `exarmo` writes after the text of a word that is CONSTRAINED
# UNPREDICTABLE, as the corpus expects it.
UNPREDICTABLE = " (unpredictable)"


def variants(which, hexes):
    """The variant the release `exarmo` decodes each word to, or None, and
    whether the word is CONSTRAINED UNPREDICTABLE."""
    exarmo = ROOT / "target" / "release" / "exarmo"
    if not exarmo.exists():
        sys.exit(f"{exarmo} is not built: cargo build --release -p exarmo-cli")
    # The command takes a halfword in the high half of the word, as the
    # decoder does
    words = [h if len(h) == 8 else h + "0000" for h in hexes]
    run = subprocess.run([str(exarmo), "decode", which, "--debug", "-"],
                         input="\n".join(words) + "\n", capture_output=True, text=True)
    out = []
    for line in run.stdout.splitlines():
        if line.startswith("error:"):
            out.append((None, False))
        else:
            text, debug = line.rsplit("\t", 1)
            out.append((debug.split(" ")[0], text.endswith(UNPREDICTABLE)))
    assert len(out) == len(hexes), "exarmo wrote a line per word"
    return out


def binja_words():
    """The words Binary Ninja's own tests carry, by instruction set.

    Empty where the checkout is not named, leaving only the sweep's own
    words to seed from.
    """
    out = {"a32": [], "t32": []}
    # An unset path is the working directory, which is no checkout
    if not os.environ.get("BINJA_ARMV7") or not BINJA.is_dir():
        print("BINJA_ARMV7 is not set, so no words are read from it", file=sys.stderr)
        return out
    for which, path in (("a32", "armv7_disasm/test.py"), ("t32", "thumb2_disasm/test.py")):
        for m in re.finditer(r"\(b'((?:\\x[0-9a-f]{2})+)'", (BINJA / path).read_text()):
            raw = bytes(int(h, 16) for h in re.findall(r"\\x([0-9a-f]{2})", m.group(1)))
            if which == "a32":
                out[which].append(int.from_bytes(raw, "little"))
            elif len(raw) == 2:
                out[which].append(int.from_bytes(raw, "little"))
            else:
                hw1, hw2 = int.from_bytes(raw[:2], "little"), int.from_bytes(raw[2:], "little")
                out[which].append(hw1 << 16 | hw2)
    return out


MODIFIED = re.compile(r"^(and|eor|sub|rsb|add|adc|sbc|rsc|tst|teq|cmp|cmn|orr|mov|bic|mvn|msr)s?"
                      r"(eq|ne|hs|lo|mi|pl|vs|vc|hi|ls|ge|lt|gt|le)?$")
BRANCH = re.compile(r"^(b|bl|blx|cbz|cbnz)(eq|ne|hs|lo|mi|pl|vs|vc|hi|ls|ge|lt|gt|le)?(\.w)?$")
LITERAL = re.compile(r"^(ldr|ldrb|ldrh|ldrsb|ldrsh|ldrd|vldr|pld|pli|pldw)"
                     r"(eq|ne|hs|lo|mi|pl|vs|vc|hi|ls|ge|lt|gt|le)?(\.w)?$")
# ADR names its label as an offset from the aligned PC, which llvm-mc writes
# as the offset with its sign.
ADR = re.compile(r"^adr(eq|ne|hs|lo|mi|pl|vs|vc|hi|ls|ge|lt|gt|le)?(\.w)?$")


def curate(text, which, narrow):
    """llvm-mc's text as the corpus keeps it."""
    text = " ".join(text.lower().split())
    mnemonic, _, rest = text.partition(" ")
    stem = mnemonic.split(".")[0]
    mnemonic = re.sub(r"\.[wn]$", "", mnemonic)
    # The PC an A32 instruction reads is itself plus 8, a T32 one plus 4.
    pc = ADDRESS + (8 if which == "a32" else 4)
    if BRANCH.match(stem):
        m = re.fullmatch(r"(?:(r\d+|sp|lr|pc), )?#(-?\d+)", rest)
        if m:
            target = (pc + int(m.group(2))) & 0xFFFFFFFF
            head = f"{m.group(1)}, " if m.group(1) else ""
            return f"{mnemonic} {head}0x{target:x}"
    if LITERAL.match(stem):
        m = re.fullmatch(r"(.*)\[pc, #(-?\d+)\]", rest)
        if m:
            offset = int(m.group(2))
            if offset >= 1 << 31:
                offset -= 1 << 32
            target = ((pc & ~3) + offset) & 0xFFFFFFFF
            return f"{mnemonic} {m.group(1)}0x{target:x}"
    if ADR.match(stem):
        m = re.fullmatch(r"(r\d+|sp|lr|pc), #(-?\d+)", rest)
        if m:
            target = ((pc & ~3) + int(m.group(2))) & 0xFFFFFFFF
            return f"{mnemonic} {m.group(1)}, 0x{target:x}"
    if MODIFIED.match(stem):
        m = re.fullmatch(r"(.*)#(\d+), #(\d+)", rest)
        if m:
            imm8, rot = int(m.group(2)), int(m.group(3))
            value = ((imm8 >> rot) | (imm8 << (32 - rot))) & 0xFFFFFFFF if rot else imm8
            return f"{mnemonic} {m.group(1)}#{value}"
        # llvm-mc writes the value as a signed 32-bit number where its top
        # bit is set. The operand is the 32 bits, and is written unsigned.
        m = re.fullmatch(r"(.*)#-(\d+)", rest)
        if m:
            return f"{mnemonic} {m.group(1)}#{(-int(m.group(2))) & 0xFFFFFFFF}"
    return f"{mnemonic} {rest}".strip()


def main(sweep):
    words = {"a32": [], "t32": []}
    # The sweep's lines, and not what cargo test printed around them.
    for line in pathlib.Path(sweep).read_text().splitlines():
        match line.split():
            case [which, hex_, _] if which in words:
                words[which].append(hex_)
            case _:
                pass
    extra = binja_words()
    for which in ("a32", "t32"):
        seen = set(words[which])
        binja = [f"{w:04x}" if w <= 0xFFFF and which == "t32" else f"{w:08x}" for w in extra[which]]
        flat = words[which] + sorted({h for h in binja if h not in seen})
        # A halfword sits in the high half, which is how llvm.py takes it.
        asked = many(
            [int(h, 16) << (16 if len(h) == 4 else 0) for h in flat],
            triple=TRIPLE[which],
            wide=None,
            thumb=which == "t32",
            features=FEATURES,
            widths=[len(h) == 8 for h in flat],
        )
        (directory,) = [d for d in CORPORA["aarch32"] if d.name == which]
        over = forms(directory)
        cases = collections.defaultdict(list)
        kept = unread = undecoded = 0
        for h, text, (variant, unpredictable) in zip(flat, asked, variants(which, flat)):
            if text is None:
                unread += 1
            elif variant not in over:
                undecoded += 1
            else:
                kept += 1
                mark = UNPREDICTABLE if unpredictable else ""
                cases[over[variant]].append(f"{h} {curate(text, which, len(h) == 4)}{mark}")
        files = add(headers([directory]), cases)
        print(f"{which}: {kept} lines added to {files} files, {unread} words llvm-mc did not "
              f"read, {undecoded} decoding to no form of the corpus")


if __name__ == "__main__":
    main(sys.argv[1])
