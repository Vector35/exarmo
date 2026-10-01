# exarmo-core

Runtime support shared by the
[`exarmo-aarch64`](https://crates.io/crates/exarmo-aarch64) and
[`exarmo-aarch32`](https://crates.io/crates/exarmo-aarch32) disassemblers and
their C APIs. It holds the token stream an instruction's text is written as, the
storage behind the operand view, the condition flag and control flow models, and
the error a decode returns.

Use `exarmo-aarch64` or `exarmo-aarch32`, which re-export what a consumer
needs from here. A consumer handling both instruction sets can write against
the `Decoded` trait both implement.
