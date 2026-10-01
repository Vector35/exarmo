# exarmo-aarch32

An AArch32 disassembler, generated from Arm's machine-readable AArch32 ISA
specification. It decodes every instruction the specification describes,
in both the A32 and T32 instruction sets.

This version is generated from Arm's 2026-09 release of the specification.

```rust
use exarmo_aarch32::{ItState, a32, t32};

let add = a32::decode(0xe0812003).unwrap();
assert_eq!(add.at(0x1000).to_string(), "add\tr2, r1, r3");

// A T32 word holds its first halfword in the high half.
let nop = t32::decode(0xbf000000, ItState::Outside).unwrap();
assert_eq!(nop.at(0x1000).to_string(), "nop");
```

Both instruction sets decode to one `Instruction`. A T32 decode takes the IT
state, since an instruction inside an IT block takes its condition from the
block. `ItState::after` gives the state for the instruction that follows.

An instruction holds no address. `at` gives it one, for the text and for
anything PC-relative. An instruction also gives:

- its encoding and mnemonic, as fieldless enums to switch on
- its operands, as a positional view a consumer can read without matching
  on every encoding
- its text as a token stream, each token with its kind and the operand it
  belongs to, for syntax highlighting
- the condition flags it reads and writes, and what it does to the flow of
  control
- whether the word is CONSTRAINED UNPREDICTABLE

[`exarmo-aarch32-capi`](https://crates.io/crates/exarmo-aarch32-capi) provides
a C API around this crate.
[`exarmo-cli`](https://crates.io/crates/exarmo-cli) provides a command-line
disassembler built on this crate and
[`exarmo-aarch64`](https://crates.io/crates/exarmo-aarch64).
