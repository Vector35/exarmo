# exarmo

Disassemblers for ARM's AArch64 and AArch32 instruction sets, generated
from ARM's machine-readable specification. `exarmo-aarch64` decodes
AArch64, and `exarmo-aarch32` decodes both A32 and T32.

The decoders, formatters and operand tables are generated from ARM's
[ISA and system register XML](https://developer.arm.com/Architectures/A-Profile%20Architecture#Downloads).
The AArch64 intrinsic mappings come from the Advanced SIMD table in
[ACLE](https://github.com/ARM-software/acle).

The generated code is checked in, but the generator that writes it is not
public. If exarmo disassembles an instruction incorrectly, file an issue with
its encoding and the disassembly you expected.

## Crates

The crates a consumer uses:

| crate | what it is |
| --- | --- |
| `exarmo-aarch64` | the AArch64 runtime library, which decodes a word, renders it and reads its operands |
| `exarmo-aarch32` | the AArch32 runtime library, which decodes an A32 or T32 word, renders it and reads its operands |
| `exarmo-cli` | the `exarmo` command, which decodes instructions in either instruction set |
| `exarmo-aarch64-capi` | the C interface to `exarmo-aarch64` |
| `exarmo-aarch32-capi` | the C interface to `exarmo-aarch32` |

The crates that support them:

| crate | what it is |
| --- | --- |
| `exarmo-core` | code shared by the two runtime libraries and their C interfaces |
| `exarmo-testing` | the harness both sides' corpus tests are written on |

## Building

The generated decoder is checked in, so building needs only a Rust toolchain.

```sh
cargo build --release
cargo test
```

`cargo test -p exarmo-aarch64-capi` and `cargo test -p exarmo-aarch32-capi`
also need a C compiler on the path, for the smoke test that compiles against
the header.

## The command

`exarmo` decodes a word in either instruction set.

```sh
cargo run -p exarmo-cli -- decode a64 d503201f      # nop
cargo run -p exarmo-cli -- decode a32 e0812003      # add r2, r1, r3
cargo run -p exarmo-cli -- decode t32 bf000000      # nop
```

A hex argument of `-` reads a word per line from the input instead, which is
how a sweep of the encoding space asks about thousands at once, and `--debug`
prints the instruction's fields beside its text. On AArch64, `--intrinsics`
prints the first ACLE intrinsic the instruction realises.

`--byteswap` reads the digits as the bytes in the order they sit in memory,
which is how a byte dump writes them, so a word can be pasted from a
disassembler that shows them that way without swapping it by hand. T32 is
swapped halfword by halfword, since that is how the architecture orders hw1 and
hw2.

```sh
cargo run -p exarmo-cli -- decode a64 --byteswap f1c7e5f2
# movk x17, #0x2e3f, lsl #0x30
```

Each decoder is a feature, both on by default, so a build that wants one
instruction set need not compile the other's generated tree.

```sh
cargo build -p exarmo-cli --no-default-features --features aarch64
```

## Using it from C

Each of `exarmo-aarch64-capi` and `exarmo-aarch32-capi` builds a `staticlib`
and an `rlib`, with the same shape of interface. The examples below
are the AArch64 one; read `exarmo-aarch32` for `exarmo-aarch64` throughout for
the other.

The C interface is two headers that install as a pair.
[`include/exarmo/aarch64.h`](aarch64-capi/include/exarmo/aarch64.h) is
written by hand and is the authority on layout.
[`include/exarmo/aarch64_generated.h`](aarch64-capi/include/exarmo/aarch64_generated.h)
sits beside it and is written by the generator.

Linking the static library needs no extra flags on macOS, since everything it
uses is in libSystem. Elsewhere, ask the toolchain what it wants.

```sh
cargo rustc --release -p exarmo-aarch64-capi --crate-type staticlib -- \
    --print native-static-libs
```

Every entry point catches a panic and reports it as a failure rather than
unwinding into C. That needs panics to unwind, which is Cargo's default, so a
build with `panic = "abort"` kills the host process instead.

The AArch32 interface differs in how an instruction is decoded. It has
`exarmo_aarch32_decode_a32_word` and `exarmo_aarch32_decode_a32_bytes` for A32,
and `exarmo_aarch32_decode_t32_word` and `exarmo_aarch32_decode_t32_bytes` for
T32. A T32 decode takes the IT state as an argument, and
`exarmo_aarch32_it_state_after` gives the state for the next instruction. A T32
decode from bytes reads either two or four bytes, depending on the size encoded
in the first 16 bits of the instruction.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
