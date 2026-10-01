# exarmo-testing

The harness the AArch64 and AArch32 corpus tests are written on. It reads a
corpus of `<hex> <expected disassembly>` lines, decodes each word without
letting a panic out, and reports every case that fails. Each instruction set
implements `Corpus` for what differs between them. It is a dev-dependency
only.
