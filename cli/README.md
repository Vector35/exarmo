# exarmo-cli

The `exarmo` command, a disassembler for AArch64, A32 and T32. It is built on
[`exarmo-aarch64`](https://crates.io/crates/exarmo-aarch64) and
[`exarmo-aarch32`](https://crates.io/crates/exarmo-aarch32).

```sh
cargo install exarmo-cli

exarmo decode a64 d503201f      # nop
exarmo decode a32 e0812003      # add r2, r1, r3
exarmo decode t32 bf000000      # nop
```

Each instruction is given in hexadecimal as its 32-bit word. A T32
instruction's first halfword goes in the high half, so a 16-bit instruction is
still written as eight digits, `bf000000`, with its low half ignored. An
argument of `-` reads one per line from standard input.

`--byteswap` reads the digits as the bytes sit in memory, as a hex dump shows
them. `--debug` prints the instruction's encoding and fields beside its text.
`--intrinsics` prints the first ACLE intrinsic an AArch64 instruction
implements. A word that is CONSTRAINED UNPREDICTABLE has ` (unpredictable)`
after its text. A word that decodes to no instruction prints why in place of
its text, such as `error: Unallocated`.

The `aarch64` and `aarch32` features choose which instruction sets are built.
Both are on by default.
