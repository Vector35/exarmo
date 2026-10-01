# exarmo-aarch32-capi

A C API for the [`exarmo-aarch32`](https://crates.io/crates/exarmo-aarch32)
disassembler, covering A32 and T32. Its headers are `include/exarmo/aarch32.h`
and the generated `include/exarmo/aarch32_generated.h`.

```c
#include <exarmo/aarch32.h>

exarmo_aarch32_instruction inst;
if (exarmo_aarch32_decode_a32(0xe5901000, &inst) == EXARMO_AARCH32_STATUS_OK) {
    char text[EXARMO_AARCH32_MAX_TEXT + 1];
    exarmo_aarch32_instruction_text(&inst, 0x1000, text, sizeof text);
    /* text is "ldr\tr1, [r0]" */
}

/* A T32 word holds its first halfword in the high half. */
exarmo_aarch32_it_state outside = {EXARMO_AARCH32_IT_OUTSIDE, 0, 0};
exarmo_aarch32_decode_t32(0xbf000000, outside, &inst);
```

A32 and T32 each have a decode function. A T32 decode takes the IT state,
which `exarmo_aarch32_it_state_after` advances from one instruction to the
next. `exarmo_aarch32_t32_length` says from the first halfword whether a
T32 instruction is two bytes or four.

An instruction is decoded once into storage the caller owns. Functions then
read its encoding and mnemonic, operands, condition flags, effect on the flow
of control, and text, as plain text or as tokens. Nothing allocates. The
header states the largest text, token count and operand count any
instruction needs, so buffers can be sized once. Every entry point catches a
panic rather than unwinding into C.

## Building against it

The crate builds as a static library and an rlib. A shared library is built
on request:

```sh
cargo rustc --release -p exarmo-aarch32-capi --crate-type cdylib
```

To link it into a C or C++ project from crates.io, depend on it from a crate
of your own whose `crate-type` is `staticlib` or `cdylib`, and whose `lib.rs`
names it so that it is linked in:

```rust
use exarmo_aarch32_capi as _;
```

Your project then links the library that crate builds.

The crate sets `links = "exarmo_aarch32"`, so your crate's build script is
given the directory holding the headers as `DEP_EXARMO_AARCH32_INCLUDE`.
Compile against the headers from there, which match the library being
linked.
