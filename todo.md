# busyna and restrace work plan

This file is a work plan. It expands the request recorded in `TODO` into
concrete, independently implementable tasks, and it records the findings the
tasks were derived from.

Feed this file back to the coding agent to execute the tasks in order. Each
task states the problem, the evidence, the design decision, the files to
touch, and the acceptance criteria. Follow `AGENTS.md` for the project
conventions: the two-line copyright header, the `Display`/`FromStr`
round-trip, the `Action` wiring, the `CHANGELOG.md` entry, and the functional
test.

## Status legend

- [ ] not started
- [x] done

## Summary of findings

Measurements taken on this machine with `strace` 7.2, on a scratch directory
with a two-line shell script.

- [x] `restrace` already covers most of what busyna needs: trace ingestion,
  the process tree, command identity, file paths with `-y` annotations,
  read/write classification, file metadata for fingerprints, and exit codes.
- [x] The gaps are in four places: strace flag configuration, the `...`
  string-truncation marker, missing typed records on the write side, and
  the per-line dispatch cost.
- [x] The busyna-specific logic (file descriptor table, working directory
  chain, path resolution, read/write classification) belongs in busyna, not
  in `restrace`.

### F1 — the strace flags are hardcoded

`STRACE_FLAGS` in `src/stdprocess.rs` is a private constant, and
`under_strace_with`/`stream_with` can only *append* arguments. There is no
way to replace them.

Consequences:

- `-v` cannot be turned off, so every `execve` prints the whole
  environment. A measured `execve` line carried about 2 KB of `envp`,
  which is pure overhead for busyna, which needs `argv` only.
- A smaller `-s` cannot be used: `-s` also truncates `argv` and `envp`.
  With `-s 0` strace renders `execve("/usr/bin/cat", [""..., ""...], ...)`.
- Appending `-s N` *does* override the earlier `-s 1048576`, because strace
  keeps the last `-s`, but it breaks the parsing of any argument longer
  than `N` bytes, which is fatal for `argv`.

### F2 — the `...` truncation marker is not parsed

strace prints `"abc"...` when a decoded string is longer than the `-s`
limit. The `Bytes`, `Quoted` and string-array parsers require a closing
quote, so such a line fails to parse in full and degrades to
`Action::Generic`. For an `execve` that means losing the command identity.

This is the direct reason why F1 cannot be worked around by shrinking `-s`.

### F3 — the write side is under-covered

The following calls have no typed record and fall back to `Action::Generic`:
`truncate`, `pwrite64`, `chmod`, `fchmod`, `fchmodat`, `chown`, `fchown`,
`fchownat`, `lchown`, `mknod`, `mknodat`, `setxattr`, `fsetxattr`,
`lsetxattr`, `removexattr`, `fremovexattr`, `lremovexattr`, `fallocate`,
`tee`, `vmsplice`, `utimes` and `futimesat`.

busyna can match on the generic name, but each call then needs ad-hoc
argument extraction.

On a measured 138060-line trace, 34024 of the 112045 parsed lines (30%)
fell back to `Generic`. The bulk of them are process and memory calls that
busyna does not care about (`mprotect`, `rt_sigprocmask`, `munmap`,
`arch_prctl`, `set_robust_list`, `set_tid_address`, `rseq`, `prlimit64`,
`fadvise64`, `fcntl`, `wait4`), so the fallback rate is not a problem by
itself; the missing write-side records are.

### F4 — the per-line dispatch cost

`Action::from_str` tries every typed parser in sequence, about 70 of them,
for each line of the trace. The measured rate is 198000 lines per second:
138060 lines, 32 MB, 0.70 s, single threaded. That is fine for a
script-sized trace and about 20 s for a 1 GB build trace.

## Part 1 — restrace tasks

Do these first: they are small, they are independently testable, and they
make busyna cheaper and less brittle.

### [ ] R1 — configurable strace flags

Problem: F1.

Design: add a `StraceOptions` value that describes the flags, with a
builder, and use it in the new `under_strace_with_options` and
`stream_with_options` entry points. The existing `under_strace`,
`under_strace_with`, `stream` and `stream_with` keep their current
behaviour, so the change is additive and backwards compatible.

The options must cover at least:

- `verbose`: whether `-v` is passed. Off by default only for the new
  entry points; the old ones keep `-v`.
- `strings`: the `-s` limit.
- `trace_filter`: the `-e trace=` selection.
- `follow_forks`: the `--follow-forks` flag.
- `timestamps`: the `--absolute-timestamps` format.
- `decode_fds`: the `-y` flag.

Files to touch:

- `src/stdprocess.rs`: the new options type, the new entry points, and
  the removal of the hardcoded constant from the code path that builds the
  command.
- `src/tokio.rs`: the matching `tokio::under_strace_with_options` and
  `tokio::stream_with_options`.
- `src/lib.rs`: re-export the new public items.
- `CHANGELOG.md`: an entry under `## [Unreleased]` and `### Added`.

Acceptance criteria:

- `under_strace_with_options(cmd, opts)` produces the expected argument
  vector, and the default options reproduce the current `STRACE_FLAGS`
  exactly.
- The existing unit tests in `src/stdprocess.rs` keep passing unchanged.
- A new unit test asserts the argument vector for a non-default options
  value.

### [ ] R2 — parse the `...` truncation marker

Problem: F2.

Design: teach the string parsers to accept a trailing `...` after the
closing quote, and record that the value was truncated.

- Add a helper in `src/syscalls/parse.rs` that strips a trailing `...` and
  reports whether it was present.
- Add a `truncated` flag to the `Bytes` and `Quoted` types, or a
  `Bytes`/`Quoted` variant that carries it, and make `Display` write the
  `...` back so the round-trip is exact.
- Apply the same rule to the string-array parser used by `execve` and
  `execveat`, so an `argv` entry that was truncated does not break the
  whole record.

This changes the public shape of `Bytes` and `Quoted`, so bump the minor
version of the crate and note the change in `CHANGELOG.md`.

Files to touch:

- `src/syscalls/parse.rs`
- `src/syscalls/bytes.rs`
- `src/syscalls/quoted.rs`
- `src/syscalls/execve.rs` and `src/syscalls/execveat.rs`, for the
  argument vector.
- `src/syscalls/read.rs`, `src/syscalls/write.rs` and the other
  buffer-carrying records, if the buffer type changes shape.
- `CHANGELOG.md`

Acceptance criteria:

- `read(3</tmp/x>, "abc"..., 3) = 3` parses into a `Read` record, not into
  a `Generic` one, and the `Display` output is byte-identical to the
  input.
- `execve("/bin/sh", ["sh"..., "x"], []) = 0` parses into an `Execve`
  record.
- Unit tests cover the truncated and the untruncated forms of every
  changed type.

### [ ] R3 — typed records for the missing write-side calls

Problem: F3.

Do these calls, in this order:

1. `truncate` and `pwrite64`, the two that appear most often.
2. `chmod`, `fchmod` and `fchmodat`, with a `ChmodMode` argument type.
3. `chown`, `fchown`, `fchownat` and `lchown`.
4. `mknod` and `mknodat`, with the device and mode argument types.
5. `setxattr`, `fsetxattr`, `lsetxattr` and the three `removexattr`
   variants.
6. `fallocate`, `tee` and `vmsplice`, if the effort is warranted.

Each record follows the `src/syscalls/write.rs` template exactly: the
module doc comment linking the man page, the public struct with one
documented field per argument, `from_raw`, `Display`, `FromStr`, and unit
tests. Register each one in `src/syscalls/mod.rs`, keeping both lists
alphabetically sorted, and wire it into `src/action.rs`: the import, the
enum variant before `Generic`, the `Display` arm, the `FromStr` branch
before the `Generic` fallback, and the two test lists.

Files to touch, per call:

- `src/syscalls/<name>.rs`, new
- `src/syscalls/mod.rs`
- `src/action.rs`
- `src/syscalls/lenresult.rs`, if the record uses `LenResult`, to keep
  the documented list of calls complete
- `tests/functional.rs`, a `parses_<name>` test
- `CHANGELOG.md`, one bullet per call

Acceptance criteria:

- Every new call round-trips through `Display` and `FromStr`.
- The `parse_variants` and `round_trip` test lists in `src/action.rs`
  cover every new variant.
- `cargo test --all-features` passes, and the functional tests skip
  themselves when `strace` is unavailable.

### [ ] R4 — dispatch `Action::from_str` on the syscall name

Problem: F4.

Design: split the call name off the line first, up to the first `(`, and
dispatch on the name with a single `match`, instead of trying every typed
parser in turn. Keep the generic fallback as the last arm of the match.

Files to touch:

- `src/action.rs`: the `FromStr` implementation.
- `src/syscalls/parse.rs`, if a helper for splitting the name off a call
  is added there.

Acceptance criteria:

- The set of parsed actions is unchanged: the existing tests in
  `src/action.rs` pass unchanged, and a test asserts that an unknown name
  still yields `Action::Generic`.
- The measured rate rises to at least 1000000 lines per second on the
  138060-line trace described in F4.

### [ ] R5 — restrace housekeeping

- [ ] Update `AGENTS.md` if the new option type or the new shared types
  change the workflow it describes.
- [ ] Update the `README.md` if the public API grew.
- [ ] Run `cargo fmt`, `cargo clippy --all-targets --all-features`,
  `cargo test --all-features` and `omnilint repository`.

## Part 2 — the busyna design

busyna is a separate tool. It has two modes.

- In discovery mode it runs a shell script, records the files each command
  reads and writes, and writes that to a manifest.
- In replay mode it runs the same commands, but only when a file the
  command read has changed; otherwise it skips the command.

### Terms

- **script**: the shell script being recorded.
- **command**: one external program invoked by the script, a direct child
  of the shell, including its whole process subtree.
- **input**: a file whose content or metadata the command consumed.
- **output**: a file the command created or modified.
- **fingerprint**: the value used to detect that a file changed.

### B1 — the skeleton and the command line

- [ ] Create the busyna crate, with the same layout conventions as
  `restrace`.
- [ ] Implement the command line: `busyna discover <script>`,
  `busyna replay <script>` and `busyna status`, with `--manifest`,
  `--dry-run`, `--force` and `--why` options.
- [ ] Use `restrace::stdprocess::stream_with` to run the script under
  `strace`.

### B2 — from trace lines to commands

- [ ] Collect the `restrace::Event` values and sort them by timestamp.
  The `Reconstructor` yields resumed calls late, so the raw order is not
  causal.
- [ ] Keep, per process id, the current command instance, the working
  directory, the file descriptor table and the parent process id.
- [ ] On a successful `execve` or `execveat`, start a new command
  instance for that process, and record the program path, the argument
  vector, the working directory and a snapshot of the descriptor table.
- [ ] On a failed `execve`, record the attempt without starting an
  instance.
- [ ] On `fork`, `vfork`, `clone` or `clone3`, record the parent to child
  edge and let the child inherit the current instance until it execs.
- [ ] On an `Exit`, close the instance and record the exit code.
- [ ] Define a command as a process whose parent is the script's shell;
  the deeper descendants belong to that command.

### B3 — paths, descriptors and classification

- [ ] Resolve the working directory from the `AT_FDCWD</abs/cwd>`
  annotation of the `*at` calls, plus `chdir`, `fchdir` and `getcwd`.
- [ ] Resolve the absolute path of an opened file from the `-y` annotation
  of the returned descriptor, and fall back to joining the directory
  descriptor path with the pathname when the open failed.
- [ ] Maintain the descriptor table from `open`, `openat`, `openat2`,
  `creat`, `dup`, `dup2`, `dup3`, `close` and `pipe2`, so that the
  redirections of each command are known at its `execve`.
- [ ] Classify the calls into content reads, metadata reads, directory
  reads, content writes, metadata writes and namespace writes, following
  the table in the design discussion, and using `Action::Generic` for the
  calls that `restrace` does not type yet.
- [ ] Filter the pseudo-paths that `-y` produces: `pipe:[N]`,
  `socket:[N]`, `anon_inode:[N]` and the deleted marker.

### B4 — fingerprints and the manifest

- [ ] Compute a fingerprint per input: `(size, mtime_ns)` for a content
  read, `(exists, size, mtime_ns)` for a metadata read, `(mtime_ns, hash
  of the entry names)` for a directory, and the target string for a
  symlink. Add a `--hash` option that uses a content hash instead of the
  modification time.
- [ ] Write the manifest as JSON: the script, the working directory, the
  creation time, and one entry per command with the program path, the
  argument vector, the working directory, the exit code, the
  skippability, the descriptor snapshot, the inputs and the outputs.
- [ ] Mark the inputs under the system prefixes as system inputs, and
  exclude them from the decision unless `--track-system` is given.

### B5 — the replay path

- [ ] Replay re-runs the same script, so the redirections, the
  pipelines, the `&&` chains and the loops are preserved for free.
- [ ] Generate a directory of shims, one per recorded program, and
  prepend it to `PATH` for the replayed run.
- [ ] The shim finds its manifest entry by program name and argument
  vector, disambiguating repeated invocations with a per-key counter kept
  in a lock-protected state file.
- [ ] The shim compares the live fingerprint of every recorded input with
  the baseline. If they all match, the shim writes nothing and exits with
  the recorded exit code. Otherwise it executes the recorded program and
  updates the baselines with the values it observed.
- [ ] Keep the manifest as the state: the baselines are updated after
  each real execution, so the next run skips what is now up to date.

### B6 — the safety rules

A command must never be skipped when:

- its standard output is a pipe or a socket, because the output is
  consumed by another command or by a command substitution;
- its standard input is a pipe or a socket, because the input is data
  rather than a file;
- it opened a socket or connected, because it has untracked inputs;
- one of its input paths could not be resolved;
- it was invoked by absolute path, because the `PATH` shim cannot
  intercept it;
- it is a shell builtin, because it never execs.

- [ ] Implement the rules, record the reason in the manifest, and show it
  in `busyna status --why`.

### B7 — the busyna tests

- [ ] A unit test for the command tree builder, over a small recorded
  trace.
- [ ] A unit test for the path resolution and the classification tables.
- [ ] A functional test that discovers a script, changes one of its
  inputs, replays it, and checks that only the affected command ran.
- [ ] A functional test that replays twice in a row and checks that the
  second run skips everything.

## Part 3 — the open decisions

These need an answer from a human before the corresponding task is
implemented.

- [ ] Shell builtins: always re-run them, as recommended, or use a
  `PS4` timestamp and `set -x` to segment them.
- [ ] Commands invoked by absolute path: always re-run them, as
  recommended, or add a `--rewrite` mode that emits a patched copy of the
  script.
- [ ] Whether R1, R2 and R3 are implemented in `restrace` first, as
  recommended, or worked around inside busyna.

## Part 4 — the verification checklist

Run all of these before committing any of the work.

- [ ] `cargo fmt`
- [ ] `cargo clippy --all-targets --all-features`
- [ ] `cargo test --all-features`
- [ ] `omnilint repository`
- [ ] `git diff` reviewed before each commit, with the conventional
  commit subject style used by the history.
