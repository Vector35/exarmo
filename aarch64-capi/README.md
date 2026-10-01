# exarmo-aarch64-capi

A C API for the [`exarmo-aarch64`](https://crates.io/crates/exarmo-aarch64)
disassembler. Its headers are `include/exarmo/aarch64.h` and the generated
`include/exarmo/aarch64_generated.h`.

```c
#include <exarmo/aarch64.h>

exarmo_aarch64_instruction inst;
if (exarmo_aarch64_decode(0xf9400420, &inst) == EXARMO_AARCH64_STATUS_OK) {
    char text[EXARMO_AARCH64_MAX_TEXT + 1];
    exarmo_aarch64_instruction_text(&inst, 0x1000, text, sizeof text);
    /* text is "ldr\tx0, [x1, #0x8]" */
}
```

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
cargo rustc --release -p exarmo-aarch64-capi --crate-type cdylib
```

To link it into a C or C++ project from crates.io, depend on it from a crate
of your own whose `crate-type` is `staticlib` or `cdylib`, and whose `lib.rs`
names it so that it is linked in:

```rust
use exarmo_aarch64_capi as _;
```

Your project then links the library that crate builds.

The crate sets `links = "exarmo_aarch64"`, so your crate's build script is
given the directory holding the headers as `DEP_EXARMO_AARCH64_INCLUDE`.
Compile against the headers from there, which match the library being
linked.
