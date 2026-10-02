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

## Crate features

The crate is `#![no_std]`. Its default `std` feature can be turned off for use
in an environment without the standard library. That removes `capi`, the
support the C APIs are built on.

The `alloc` feature, which `std` enables, can be turned off as well for use in
an environment without an allocator. That removes `Token` and the `TokenSink`
impls for `Vec<Token>` and `String`.
