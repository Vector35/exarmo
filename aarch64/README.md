# exarmo-aarch64

An AArch64 disassembler, generated from Arm's machine-readable A64 ISA
specification. It decodes every instruction the specification describes,
including Advanced SIMD, floating-point, SVE, SME and the Future Architecture
Technologies extensions.

This version is generated from Arm's 2026-09 release of the specification.

```sh
cargo add exarmo-aarch64
```

```rust
// nop
let instruction = exarmo_aarch64::decode_bytes(&[0x1f, 0x20, 0x03, 0xd5]).unwrap();
assert_eq!(instruction.at(0x1000).to_string(), "nop");
```

`decode_bytes` decodes an instruction from a byte slice, and `decode_word` from
a 32-bit instruction word. `disassemble` returns an iterator over the
instructions in a byte buffer. Each instruction is an `Instruction`, which holds
no address. `at` gives it one, for the text and for anything PC-relative. An
instruction also gives:

- its encoding and mnemonic, as fieldless enums to switch on
- its operands, as a positional view a consumer can read without matching
  on every encoding
- its text as a token stream, each token with its kind and the operand it
  belongs to, for syntax highlighting
- the condition flags it reads and writes, and what it does to the flow of
  control
- whether the word is CONSTRAINED UNPREDICTABLE
- the ACLE Advanced SIMD intrinsics it implements

[`exarmo-aarch64-capi`](https://crates.io/crates/exarmo-aarch64-capi) provides
a C API around this crate.
[`exarmo-cli`](https://crates.io/crates/exarmo-cli) provides a command-line
disassembler built on this crate and
[`exarmo-aarch32`](https://crates.io/crates/exarmo-aarch32).

## Crate features

The crate is `#![no_std]`. Its default `alloc` feature can be turned off for use
in an environment without an allocator. That removes `Token` and the
`TokenSink` impls for `Vec<Token>` and `String`.
