"""Compare what we write for an instruction with what llvm-mc writes.

A second opinion, not a test. llvm-mc is more permissive than the XML in
places, and the two make different choices about how to spell a number and a
branch target. Only the encodings the formatter renders are compared, and the
rest are counted.

    cargo build --release -p exarmo-cli
    python3 tools/baseline.py            # both, 20,000 words of each kind
    python3 tools/baseline.py a64 50000  # one set, more words

BASELINE_EXAMPLES says how many differences to print, ten by default.
LLVM_MC says where llvm-mc is, if it is not where Homebrew puts it.
"""
import os, pathlib, subprocess, sys, random, collections, re
sys.path.insert(0, str(pathlib.Path(__file__).parent))
from llvm import many

ROOT = pathlib.Path(__file__).resolve().parent.parent
A32_FEATURES = "+bf16,+i8mm,+dotprod,+fp16fml,+fullfp16,+crypto,+ras,+sb,+crc"

def binary():
    """The exarmo command, built in release."""
    path = ROOT / "target" / "release" / "exarmo"
    if not path.exists():
        sys.exit(f"{path} is not built: cargo build --release -p exarmo-cli")
    return str(path)

def ours(iset, words):
    run = subprocess.run([binary(), "decode", iset, "-"],
                         input="\n".join(f"{w:08x}" for w in words),
                         capture_output=True, text=True)
    return [l.rstrip("\n") for l in run.stdout.splitlines()]

def tidy(text, reach=0xffffffff):
    """One spelling of an instruction, so a difference is a real one.

    We write a number in hexadecimal and llvm-mc in decimal, which is a choice
    each makes rather than a disagreement, so a number is compared by its value.
    Nothing here writes the `.w` qualifier llvm-mc puts on a T32
    instruction, since the encoding's length says what it would have.

    `reach` is how wide an address is in the instruction set, since a
    negative offset and the address it reaches agree only modulo that.
    """
    # We mark a word that is CONSTRAINED UNPREDICTABLE, which llvm-mc does not.
    text = text.removesuffix(" (unpredictable)")
    text = " ".join(text.replace("\t", " ").split()).lower()
    text = re.sub(r"^(\S+?)\.[wn](?=\s|$)", r"\1", text)
    def number(m):
        raw = m.group(0)
        sign = -1 if raw.startswith("-") else 1
        digits = raw.lstrip("-")
        value = int(digits, 16) if digits.startswith("0x") else int(digits, 10)
        # A branch target we write as an address and llvm-mc as an offset, both
        # from zero, so the two agree once the sign is folded in.
        return str(sign * value & reach)
    return re.sub(r"-?(?:0x[0-9a-f]+|\d+)", number, text)

def sweep(name, iset, words, *, triple, wide, thumb, features, reach=0xffffffff):
    us = ours(iset, words)
    them = many(words, triple=triple, wide=wide, thumb=thumb, features=features)
    agree = differ = 0
    unrendered = undecoded = theirs_only = 0
    examples = []
    for w, o, t in zip(words, us, them):
        # The command writes every reason a word is no instruction after
        # this, so an UNPREDICTABLE word or a reserved hint is not compared.
        if o.startswith("error: "):
            undecoded += 1
            continue
        if o == "-":
            unrendered += 1
            continue
        if t is None:
            theirs_only += 1
            continue
        if tidy(o, reach) == tidy(t, reach):
            agree += 1
        else:
            differ += 1
            if len(examples) < int(os.environ.get("BASELINE_EXAMPLES", "10")):
                examples.append((w, tidy(o, reach), tidy(t, reach)))
    compared = agree + differ
    print(f"{name}: {len(words)} words, {compared} compared, {agree} agree "
          f"({100*agree//max(compared,1)}%), {differ} differ")
    print(f"   {unrendered} we decode but do not write, {undecoded} we do not decode, "
          f"{theirs_only} llvm-mc does not read")
    for w, o, t in examples:
        width = 4 if not wide else 8
        print(f"  {w >> (16 if not wide else 0):0{width}x}  ours: {o}")
        print(f"  {'':{width}}  llvm: {t}")
    print()

def aarch64(n):
    sweep("A64", "a64", [random.getrandbits(32) for _ in range(n)],
          triple="aarch64", wide=True, thumb=False, features="+all",
          reach=0xffffffffffffffff)

def aarch32(n):
    sweep("A32", "a32", [random.getrandbits(32) for _ in range(n)],
          triple="armv8.6a", wide=True, thumb=False, features=A32_FEATURES)
    # A halfword sits in the high half, which is how the decoder takes it.
    sweep("T32 halfwords", "t32", [random.getrandbits(16) << 16 for _ in range(n)],
          triple="thumbv8.6a", wide=False, thumb=True, features=A32_FEATURES)
    # A wide word's first halfword is 111 and then anything but 00.
    def wide_word():
        w = random.getrandbits(32) | 0xe0000000
        return w if (w >> 27) & 3 else w | 0x08000000
    sweep("T32 wide", "t32", [wide_word() for _ in range(n)],
          triple="thumbv8.6a", wide=True, thumb=True, features=A32_FEATURES)

SETS = {"a64": aarch64, "a32": aarch32, "aarch64": aarch64, "aarch32": aarch32}

if __name__ == "__main__":
    args = sys.argv[1:]
    which = [SETS[args.pop(0).lower()]] if args and not args[0].isdigit() else [aarch64, aarch32]
    n = int(args[0]) if args else 20000
    for sweep_set in which:
        random.seed(20260918)
        sweep_set(n)
