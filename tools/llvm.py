"""Ask llvm-mc what a word disassembles to, one word at a time.

llvm-mc reads its input as a stream of bytes and resets at each line, so a line
of four bytes may come back as two sixteen bit instructions. Nothing in the
output says which line it came from, so a word is asked about on its own.

llvm-mc tracks IT blocks, so a T32 word is asked about inside one by putting
an IT instruction before it and reading the line after the IT's. `it_before`
writes the IT that puts the word where a corpus tag says, under its
condition, as the last of the block or with one more instruction to come.
"""
import os
import shutil
import subprocess
from concurrent.futures import ThreadPoolExecutor

# Homebrew keeps llvm off the path, so the usual place is tried before
# whatever a PATH lookup finds. LLVM_MC overrides both.
def _llvm_mc():
    named = os.environ.get("LLVM_MC")
    if named:
        return named
    for path in ("/opt/homebrew/opt/llvm/bin/llvm-mc", "/usr/local/opt/llvm/bin/llvm-mc"):
        if os.path.exists(path):
            return path
    return shutil.which("llvm-mc") or "llvm-mc"

LLVM = _llvm_mc()

def bytes_of(word, wide, thumb):
    if thumb:
        hw1, hw2 = (word >> 16) & 0xffff, word & 0xffff
        pieces = [hw1 & 0xff, hw1 >> 8] + ([hw2 & 0xff, hw2 >> 8] if wide else [])
    else:
        pieces = [(word >> (8 * i)) & 0xff for i in range(4)]
    return ",".join(f"0x{b:02x}" for b in pieces)

# The conditions as a corpus tag spells them, in the order of their bits.
CONDITIONS = ["eq", "ne", "cs", "cc", "mi", "pl", "vs", "vc",
              "hi", "ls", "ge", "lt", "gt", "le", "al"]

def it_before(cond, last):
    """The IT halfword that puts the next instruction under `cond`, as the
    last of its block or the first of two, both under the condition."""
    first = CONDITIONS.index(cond)
    mask = 0b1000 if last else ((first & 1) << 3) | 0b0100
    return 0xbf00 | first << 4 | mask

def one(word, triple="armv8a", wide=True, thumb=False, features=None, it=None):
    args = [LLVM, "-disassemble", f"-triple={triple}"]
    if features:
        args.append(f"-mattr={features}")
    given = bytes_of(word, wide, thumb)
    if it is not None:
        given = f"0x{it & 0xff:02x},0x{it >> 8:02x}," + given
    run = subprocess.run(args, input=given, capture_output=True, text=True)
    # A word llvm-mc cannot read is skipped a byte at a time, and what is
    # left may read as an instruction of its own. A wide T32 word whose first
    # byte is dropped leaves three bytes, two of which may be a halfword. The
    # warning is the only sign, so it says the word was not read.
    if "invalid instruction encoding" in run.stderr:
        return None
    lines = [' '.join(l.split()) for l in run.stdout.splitlines()
             if l.strip() and not l.strip().startswith(('.', '#'))]
    # Anything but one line, after the IT's, means the bytes were not read as
    # one instruction.
    if it is not None:
        return lines[1] if len(lines) == 2 and lines[0].startswith("it") else None
    return lines[0] if len(lines) == 1 else None

def many(words, triple="armv8a", wide=True, thumb=False, workers=16, features=None,
         widths=None, its=None):
    """Each word's instruction. `widths` says per word whether it is wide,
    where the words are not all one width, and `wide` says it otherwise.
    `its` gives per word the IT halfword to put before it, or None."""
    if widths is None:
        widths = [wide] * len(words)
    if its is None:
        its = [None] * len(words)
    with ThreadPoolExecutor(max_workers=workers) as pool:
        return list(pool.map(lambda w: one(w[0], triple, w[1], thumb, features, w[2]),
                             zip(words, widths, its)))
