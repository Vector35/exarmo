"""Fill a corpus's gaps with what the disassembler writes for them.

Reads the lines a `tests/coverage.rs`'s `field_values` prints, one per field
value a block's cases never hold, as
`<hex>\t<encoding>\t<field>=<value>\t<text>\t<instruction or none>` with
what a case expects at the harness address, and adds each word to its block as
a case. An AArch32 line has a sixth column, the IT state tag a T32 case is
decoded under, `[eq]` or `[eq last]`, or nothing outside a block, and the
case is written with the tag between its hex and its text. A word the disassembler decodes but does not write is printed rather
than added. So is each word added as no instruction, such as `UNDEFINED`,
since a decode that refuses what it should take is what those cases are there
to show.

    cargo test --release -p exarmo-aarch64 --test coverage -- \\
        --ignored --nocapture field_values > gaps.txt
    python3 tools/fill.py gaps.txt

    cargo test --release -p exarmo-aarch32 --test coverage -- \\
        --ignored --nocapture field_values > gaps.txt
    python3 tools/fill.py --aarch32 gaps.txt

`--aarch32` fills the A32 and T32 corpora, whose blocks the encodings' names
tell apart, and whose lines write a T32 halfword as four digits.

The cases pin what the disassembler, which is read from the XML, writes for
every value the corpus ought to reach, so that a change to any of them shows
in review. Nothing is written unless every block the gaps name is found.
"""
import collections
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
# Each corpus's directories, a block's name being unique across them
CORPORA = {
    "aarch64": [ROOT / "aarch64" / "tests" / "cases"],
    "aarch32": [ROOT / "aarch32" / "tests" / "cases" / s for s in ("a32", "t32")],
}
# A block's header as the generator writes it, the encoding and its diagram
HEADER = re.compile(r"^// ([A-Za-z0-9_]+) [01x()|=A-Za-z0-9_]+$")
# A form's line of a header, which names its variant
FORM = re.compile(r"^// ([A-Z][A-Za-z0-9]*): ")


def gaps(path):
    """Each word the gaps name, with its encoding and text, in their order.

    A word is taken once for each block naming it: the A32 and T32 gaps may
    name the same digits for two words of different sets.
    """
    words = collections.OrderedDict()
    for line in pathlib.Path(path).read_text().splitlines():
        parts = line.split("\t")
        if len(parts) in (5, 6):
            hex_, encoding, _, text, kind = parts[:5]
            tag = parts[5] if len(parts) == 6 else ""
            head = f"{hex_} {tag}" if tag else hex_
            words.setdefault((head, encoding), (text, kind))
    return words


def headers(dirs):
    """The file each encoding's block stands in, by the encoding's name."""
    where = {}
    for d in dirs:
        for path in sorted(d.glob("*.txt")):
            for line in path.read_text().splitlines():
                header = HEADER.match(line)
                if header:
                    where.setdefault(header.group(1), path)
    return where


def forms(d):
    """The encoding each form's variant is written over, by the variant."""
    over = {}
    for path in sorted(d.glob("*.txt")):
        encoding = None
        for line in path.read_text().splitlines():
            header = HEADER.match(line)
            if header:
                encoding = header.group(1)
                continue
            form = FORM.match(line)
            if form and encoding:
                over.setdefault(form.group(1), encoding)
    return over


def insert(path, cases):
    """The file with each block's new cases after its last line."""
    out = []
    current = None

    def close():
        if current in cases:
            # Before the blank lines that end the block
            at = len(out)
            while at and not out[at - 1].strip():
                at -= 1
            out[at:at] = cases.pop(current)

    for line in path.read_text().split("\n"):
        header = HEADER.match(line)
        if header:
            close()
            current = header.group(1)
        out.append(line)
    close()
    assert not cases, f"{path.name}: no block for {sorted(cases)}"
    return "\n".join(out)


def add(where, cases):
    """Write each block's cases into its file, and say how many files."""
    by_file = collections.defaultdict(dict)
    for encoding, lines in cases.items():
        by_file[where[encoding]][encoding] = lines
    texts = {path: insert(path, dict(blocks)) for path, blocks in by_file.items()}
    for path, text in texts.items():
        path.write_text(text)
    return len(texts)


if __name__ == "__main__":
    args = sys.argv[1:]
    corpus = "aarch64"
    if args[:1] == ["--aarch32"]:
        corpus, args = "aarch32", args[1:]
    if len(args) != 1:
        sys.exit(__doc__)
    words = gaps(args[0])
    where = headers(CORPORA[corpus])
    missing = sorted({e for _, e in words if e not in where})
    if missing:
        sys.exit(f"no block in the corpus for {', '.join(missing)}; nothing written")

    added = collections.defaultdict(list)
    unwritten, failing = [], []
    for (hex_, encoding), (text, kind) in words.items():
        # The harness names why it has no text for a word, in angle brackets
        if text.startswith("<"):
            unwritten.append(f"{hex_}\t{encoding}\t{text}")
            continue
        if kind == "none":
            failing.append(f"{hex_}\t{encoding}\t{text}")
        added[encoding].append(f"{hex_} {text}")

    files = add(where, added)
    for line in unwritten:
        print(f"not written\t{line}")
    for line in failing:
        print(f"added as no instruction\t{line}")
    count = sum(len(c) for c in added.values())
    print(f"{len(words)} words: {count} added to {files} files, "
          f"{len(failing)} of them as no instruction, "
          f"{len(unwritten)} not written by the disassembler", file=sys.stderr)
