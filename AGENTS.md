# AGENTS.md

Guidance for AI coding agents working on the *restrace* crate. It focuses on
extending the generic system call parser, but also covers the layout,
commands and conventions of the project.

## Project overview

*restrace* runs a command under [strace](https://strace.io) and parses the
emitted system calls into Rust values. Every line of `strace` output becomes
an `Event` (a pid, a timestamp and an `Action`); an `Action` is either a
`Generic` system call or the `Exit` of a traced process.

## Repository layout

- `src/lib.rs`: the crate root and the public API docs.
- `src/action.rs`: the `Action` enum and the `FromStr` dispatcher that turns
  a trace line into a `Generic` call or an `Exit`.
- `src/event.rs`: the `Event` type (pid + timestamp + `Action`).
- `src/exit.rs`: the `Exit` record.
- `src/reconstruct.rs`: joins split `<unfinished ...>` /
  `<... resumed>` lines.
- `src/stdprocess.rs`, `src/tokio.rs`: spawn the command under `strace` and
  yield `Event`s (blocking iterator and async stream).
- `src/syscalls/`: shared argument and return types, plus parsing helpers.
  The modules are private; the types are re-exported from
  `src/syscalls/mod.rs`, so the directory layout is not part of the public API.
- `tests/functional.rs`: tests that run real commands under `strace` and
  parse the result. They skip themselves when `strace` is unavailable.
- `strace/`: a reference clone of the strace sources, used only to check how
  `strace` formats a call. Never edit it. The authoritative file for the
  read/write family is `strace/src/io.c`.

## Build, test and lint

Run these before committing:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features
cargo test --all-features
omnilint repository
```

[omnilint](https://github.com/lpenz/omnilint) is the repository linter that
CI runs; `omnilint repository` runs all of its checks locally using the
`omnilint.toml` configuration. Install it from the repository above if the
command is not available.

`cargo test` also runs `tests/functional.rs`; the tests that need to trace
forks skip themselves when the environment does not allow `ptrace`. The
crate sets `#![deny(missing_docs)]` and
`#![deny(rustdoc::broken_intra_doc_links)]`, so every public item needs a
doc comment and every intra-doc link must resolve.

## Extending the parser

Every system call parses into `syscalls::Generic`, which keeps its arguments
as `Arg` values and its result as a `Ret`. Adding support for an unfamiliar
call usually means teaching `Arg` or `Ret` to parse a new shape, not adding a
per-syscall type. Keep the work in one commit.

### 1. Check how `strace` prints the call

Read the decoder for the call in the reference `strace/` tree (for the
read/write family, `strace/src/io.c`). Note how many arguments it prints,
their order and types, which argument the return value is derived from, and
whether the buffer is decoded on entry or on exit (this changes what a
short or failed call looks like). The `Display` and `FromStr`
implementations must round-trip exactly the text `strace` produces.

### 2. Extend the shared representation

Most calls already parse as `Arg::Unsigned`, `Arg::Hex`, `Arg::Str`,
`Arg::Array` or `Arg::Raw`, and as a `Ret` value. When a call still fails to
parse, adjust `src/syscalls/generic.rs` (and its unit tests) so that the new
argument or result shape is handled. Keep `Display` and `FromStr` symmetric,
and fall back to `Arg::Raw` or `Ret`'s plain value for text that needs no
decoding.

### 3. Add a shared type module when needed

If the new shape deserves its own type, add `src/syscalls/<name>.rs` with the
two-line copyright header that every file has:

```rust
// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.
```

The module contains a doc comment, a public type with documented fields, a
`Display` impl, a `FromStr` impl, and unit tests. Register `mod <name>;` and
`pub use <name>::<Name>;` in `src/syscalls/mod.rs`, keeping both lists
alphabetically sorted.

### 4. Reuse the existing shared types

Prefer the shared types over ad-hoc parsing:

- `Bytes` (`bytes.rs`): a byte string decoded from a buffer. `Arg::Str`
  carries it, so every argument that `strace` prints double-quoted keeps its
  exact bytes, including NUL and non-UTF-8 ones.
- `Errno` (`errno.rs`): an errno name or number.
- Helpers in `parse.rs`: `split_args_nested` (ignores commas inside quoted
  strings and bracketed groups), `strip_description` (removes the
  parenthesized errno text) and `split_annotation_verbatim` (splits a `<...>`
  annotation from a result).

`strace` truncates decoded strings longer than its string limit (default
32 bytes) and appends `...` after the closing quote; `Bytes` keeps that
marker in its `truncated` field and prints it back, so the call round-trips
even when the trace was truncated.

### 5. Add a CHANGELOG entry

Under `## [Unreleased]` in `CHANGELOG.md`, add a bullet in the section that
fits the change, for example:

```markdown
- Parse the `...` that `strace` appends when it truncates a decoded string,
  keeping the marker in `syscalls::Bytes` so the call round-trips.
```

### 6. Add a functional test

In `tests/functional.rs`, add a test that runs a real command through
`trace` or `trace_shell` and asserts that the new argument or result is
parsed. Guard it with `if !strace_available() { return; }` (and, when it
uses `python3`, `if !python3_available() { return; }`) and pick a command
that reliably produces the call.

### 7. Verify and commit

Run `cargo fmt`, `cargo clippy --all-targets --all-features`,
`cargo test --all-features` and `omnilint repository`, then review
`git diff` before committing.

## Commit conventions

History uses short, bodyless subject lines in the conventional-commit style,
for example `feat: add a write syscall record` or
`fix: recognize the full strace errno table`. Match that style: a `feat:`,
`fix:` or similar prefix, an imperative summary, and no body unless a
non-obvious detail needs explaining. The CI runs a commit-check action, so
keep the format consistent.

## Keeping this file up to date

This `AGENTS.md` is a living document. Update it whenever the workflow it
describes changes, for example when a new shared type or helper is added,
when the lint or test commands change, or when a step in the "Extending the
parser" process turns out to be incomplete or wrong. Keep it short, concrete
and aligned with the actual code.
