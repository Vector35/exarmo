# Baselining and seeding against llvm-mc

The tools are generic over the instruction set and read no absolute path.
`LLVM_MC` says where llvm-mc is, if it is not where Homebrew puts it.

`baseline.py` sweeps the encoding space of each instruction set, runs each word
through the release `exarmo decode <set> -`, and compares that text with what
llvm-mc writes for the same bytes. It reports how many agree and shows the ones
that do not.

    cargo build --release -p exarmo-cli
    python3 tools/baseline.py            # both sets
    python3 tools/baseline.py a64 50000  # one set, more words

`BASELINE_EXAMPLES` says how many differences to print; the default is ten.

`corpus.py` adds to the blocks of `aarch32/tests/cases/a32/` and `cases/t32/`
what llvm-mc writes for the words `tests/coverage.rs`'s sweep reached, and for
the words in Binary Ninja's own armv7 and thumb2 tests, each to the block of the
form the release `exarmo` decodes it to. Its docstring says what of llvm-mc's
text it rewrites and why. What it adds is a starting point to curate, not an
authority: llvm-mc is more permissive than the current architecture in places.

    COVERAGE_PER=8 cargo test --release -p exarmo-aarch32 --test coverage -- \
        --ignored --nocapture sweep > sweep.txt
    cargo build --release -p exarmo-cli
    BINJA_ARMV7=<path> python3 tools/corpus.py sweep.txt

`BINJA_ARMV7` is a checkout of Binary Ninja's armv7 plugin, for words a sweep
does not reach. It is not required; without it the seeding is the sweep's own
words.

`fill.py` adds a case to a corpus for every field value `tests/coverage.rs`
finds no case holding, with what the disassembler writes for it.
`coverage.rs` fails while any is missing, and lists them. `--aarch32` fills
the A32 and T32 corpora rather than AArch64's.

    cargo test --release -p exarmo-aarch64 --test coverage -- \
        --ignored --nocapture field_values > gaps.txt
    python3 tools/fill.py gaps.txt
    cargo test --release -p exarmo-aarch32 --test coverage -- \
        --ignored --nocapture field_values > gaps.txt
    python3 tools/fill.py --aarch32 gaps.txt

`llvm.py` asks llvm-mc about one word at a time. It has to: llvm-mc reads its
input as a stream of bytes and resets at each line, so a line of four bytes may
come back as two sixteen bit instructions, and nothing in the output says which
line it came from. Asking about words in bulk and matching the answers by
position silently pairs the wrong ones.

None of these is a test. They read the XML's output through llvm-mc, which is a
second opinion rather than an authority: it is more permissive than the current
architecture in places, and it makes different choices about how to write a
number and how to spell a condition.

## What the differences mean

Not every difference is a defect of ours.

`cs` and `cc` against llvm-mc's `hs` and `lo` is deliberate. The spelling
follows the architecture.

A number written in hexadecimal against llvm-mc's decimal is this
disassembler's own policy, which `baseline.py` folds out by comparing a number
by its value.

On A64, llvm-mc writes a `#` before a branch target, spaces inside a
register list, `x29` and `x30` where the assembly here writes `fp` and
`lr`, and a `// =value` comment after a shifted immediate. None of those
is a defect.

`#819200` against llvm-mc's `#200, #20` is the modified immediate written as
what it means rather than as the two fields holding it.
