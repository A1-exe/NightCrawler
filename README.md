# NightCrawler
A Rust-based RISC-V emulator for snapshot fuzzing linux ELF binaries.
Heavily derived from the work of Brandon Falk (gamozolabs).

## Future
The plan is to:
- Extend this userland emulator to that of a full-system emulator.
- Introduce a symbolic analysis backend for concolic (Concrete + Symbolic) fuzzing.
- Implement more advanced taint tracking.
- Eventually support multiple architectures.

## Current
This project is an educational project, primarily centered around learning:
- More about implementing userland and system emulators.
- How to implement coverage and feedback from scratch.
- The process for implementing an fuzzer, especially one that is emulator-based fuzzer.
- How to write and maintain more code in Rust.