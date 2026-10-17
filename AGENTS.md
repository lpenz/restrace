# AGENTS.md

Guidance for AI coding agents working on the *restrace* crate. It focuses on
the most common change, adding a new typed system call record, but also
covers the layout, commands and conventions of the project.

## Project overview

*restrace* runs a command under [strace](https://strace.io) and parses the
emitted system calls into Rust values. Every line of `strace` output becomes
an `Event` (a pid, a timestamp and an `Action`); an `Action` is either a
typed system call record, the `Exit` of a traced process, or the `Generic`
fallback for calls without a dedicated type.

## Repository layout

- `src/lib.rs`: the crate root and the public API docs.
- `src/action.rs`: the `Action` enum and the `FromStr` dispatcher that turns
  a trace line into a record. New syscalls are wired in here.
- `src/event.rs`: the `Event` type (pid + timestamp + `Action`).
- `src/exit.rs`: the `Exit` record.
- `src/reconstruct.rs`: joins split `<unfinished ...>` /
  `<... resumed>` lines.
- `src/stdprocess.rs`, `src/tokio.rs`: spawn the command under `strace` and
  yield `Event`s (blocking iterator and async stream).
- `src/syscalls/`: one module per supported system call, plus shared
  argument and return types. The type modules are private; the types are
  re-exported from `src/syscalls/mod.rs`, so the directory layout is not
  part of the public API.
- `tests/functional.rs`: tests that run real commands under `strace` and
  parse the result. They skip themselves when `strace` is unavailable.
- `strace/`: a reference clone of the strace sources, used only to check how
  `strace` formats a call. Never edit it. The authoritative file for the
  read/write family is `strace/src/io.c`.

## Build, test and lint

Run these before committing:

```sh
cargo fmt
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

## Adding a new syscall record

The steps below mirror the existing `read(2)` and `write(2)` records; use
`src/syscalls/write.rs` as a template. Keep the work in one commit.

### 1. Check how `strace` prints the call

Read the decoder for the call in the reference `strace/` tree (for the
read/write family, `strace/src/io.c`). Note how many arguments it prints,
their order and types, which argument the return value is derived from, and
whether the buffer is decoded on entry or on exit (this changes what a
short or failed call looks like). The `Display` and `FromStr`
implementations must round-trip exactly the text `strace` produces.

### 2. Create the record module

Add `src/syscalls/<name>.rs`, starting with the two-line copyright header
that every file has:

```rust
// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.
```

The module contains a doc comment that links the man page, a public struct
with one documented field per argument and the return value, a
`from_raw` constructor for building the record from kernel-style values, a
`Display` impl, a `FromStr` impl, and unit tests. For example, `write`:

```rust
pub struct Write {
    pub fd: i32,
    pub buf: Buffer<Bytes>,
    pub count: u64,
    pub result: LenResult,
}
```

`Display` prints the arguments in the same order and syntax as `strace`,
followed by ` = ` and the result. `FromStr` splits the line at the last
`) =`, strips the `name(` prefix, splits the argument list with the right
helper, parses each part and returns `ParseError` on any mismatch.

### 3. Register the module

In `src/syscalls/mod.rs`, add `mod <name>;` and `pub use <name>::<Name>;`,
keeping both lists alphabetically sorted.

### 4. Add the `Action` variant

In `src/action.rs`:

- add the type to the `use crate::syscalls::{...}` import list, in
  alphabetical order;
- add a `/// A \`name(2)\` call.` variant to the `Action` enum, before
  `Generic`;
- add `Self::<Name>(syscall) => syscall.fmt(f),` to the `Display` impl;
- add the `if let Ok(syscall) = <Name>::from_str(s) { ... }` branch to the
  `FromStr` impl, before the `Generic::from_str(s)` fallback and keeping the
  existing alphabetical order;
- extend the tests in the same file: add an `is_<name>` closure next to the
  others in `parse_variants`, add one `("<line>", is_<name>)` case there,
  and add the same line to the `round_trip` list.

The dispatch is a try-each-parser chain, so a record that fails to parse
falls through to later parsers and finally to `Generic`.

### 5. Reuse the existing argument and return types

Prefer the shared types over ad-hoc parsing, and link them in the docs the
same way the existing records do:

- `Buffer<T>` (`buffer.rs`): an argument that `strace` decodes or shows as a
  raw address; pair it with `Bytes` (`bytes.rs`) for byte strings, `Quoted`
  (`quoted.rs`) for quoted paths, or `IovecList` (`iovec.rs`) for iovec
  arrays.
- `LenResult` (`lenresult.rs`): a byte-count-or-errno return value.
- `OkResult` (`result.rs`): a generic success-or-errno return value.
- `FdResult` (`fd.rs`), `Errno` (`errno.rs`), `Mode`/`FileMode`/`AccessMode`,
  `Device`, `DirFd` and the various flag types.
- Helpers in `parse.rs`: `split_args` and `split_args_nested` (ignores
  commas inside brackets and braces), `unquote`, `strip_description`
  (removes the parenthesized errno text) and `strip_comment`.

Note that `strace` truncates decoded strings longer than its string limit
(default 32 bytes) and appends `...`; this crate does not parse that marker
yet, so keep functional-test buffers short.

### 6. Update the `LenResult` doc list

If the record uses `LenResult`, add its link to the list in
`src/syscalls/lenresult.rs` so the documented set of calls stays complete.

### 7. Add a CHANGELOG entry

Under `## [Unreleased]` and `### Added` in `CHANGELOG.md`, add a bullet in
the same form as the others, for example:

```markdown
- A typed `write(2)` record, carried as the `Action::Write` variant, with the
  `Bytes` buffer-value type.
```

### 8. Add a functional test

In `tests/functional.rs`, add a `parses_<name>` test that runs a real
command through `trace_shell` and asserts that the record is parsed. Model
it on `parses_read` and `parses_write`. Guard it with
`if !strace_available() { return; }` and pick a command that reliably makes
the call.

### 9. Verify and commit

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
when the lint or test commands change, or when a step in the "Adding a new
syscall record" process turns out to be incomplete or wrong. Keep it short,
concrete and aligned with the actual code.
