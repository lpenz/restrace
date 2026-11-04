# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Configurable `strace` options via `StraceOptions` and the new
  `under_strace_with_options` and `stream_with_options` entry points in
  `stdprocess`, with matching helpers in the `tokio` integration.
- `stdprocess::stream_with` and, under the `tokio` feature, `tokio::stream_with`
  and `tokio::under_strace_with`, to run a command under `strace` with extra
  `strace` arguments while still reading and parsing the trace from a dedicated
  pipe.
- A new `Fd` argument type that carries the path `strace` resolves for a file
  descriptor when run with `-y`, such as the `3</tmp/file>` of
  `close(3</tmp/file>)`. The `close(2)` and `fchdir(2)` descriptor arguments
  and the `Fd` of `FdResult` now use it, and `DirFd` gained the same optional
  path so the `*at` records round-trip `AT_FDCWD</cwd>` and `4</dir>`.
- `Reconstructor` is now public, so a trace captured with `strace -o FILE` can
  be parsed line by line with the same split-call reconstruction the streaming
  APIs use.

### Changed

- Removed the typed system call records; every system call now parses into
  `syscalls::Generic`, with `Action` carrying only `Generic` and `Exit`.
- `strace` now always runs with `-y`, so file descriptors and the current
  working directory are annotated with their resolved paths.
- `EventIter` and `EventStream` now yield `io::Result<Event>`, so a trace read
  error is reported instead of ending the stream as if it were EOF.
- `syscalls::Ret` gained an `Annotated` variant for return values that carry
  `-y` associated information, and is no longer `Copy`.

### Fixed

- Run `strace` with `-v`, which stops it from abbreviating structures and
  arrays. Calls that carry a decoded buffer, such as `stat(2)` and
  `getdents(2)`, are now complete instead of ending in `...`.
- Run `strace` with `-s 1048576`, which raises the default 32-byte limit on
  decoded strings. Buffers longer than that, such as `readlink(2)` results,
  are now complete instead of ending in `...`.
- Run `strace` with `-a 0`, which stops it from aligning the `=` of a return
  value to a column. Short calls such as `close(2)`, `access(2)`, `unlink(2)`,
  `link(2)` and `symlink(2)` were previously dropped when their result was
  padded away from the closing parenthesis.
- Parse a failing call whose errno carries the parenthesized description
  `strace` appends, such as `= -1 ENOENT (No such file or directory)`.
- Split call arguments on commas only at the top level of `[...]`, `{...}` and
  `(...)` groups, so a generic call that carries a structure or a nested call,
  such as `stat(2)`'s buffer or a `makedev(0, 0x1e)`, parses.
- Ignore the `/* ... */` comment `strace` appends to a flag value with no
  known name, such as `0x2000 /* AT_??? */`, and print the same comment for
  unnamed bits so the value round-trips.
- Recognize the full set of `errno` names that `strace` prints, instead of a
  small subset. Failure results using an unlisted name, such as `EFAULT` or
  `EXDEV`, previously failed to parse and caused the whole call to be dropped.
- Parse the octal and control-character escapes that `strace` emits for
  non-printable bytes in quoted strings, such as `\33` for the escape
  character. Calls carrying such a string, for example `execve(2)` with an
  environment variable that holds terminal escapes, previously failed to parse
  and caused the whole call to be dropped.
- Parse a generic call whose return value `strace` annotated with `-y`, such as
  `fcntl(0</dev/null>, F_DUPFD, 10) = 10</dev/null>`. The generic result could
  only hold a plain value or an errno, so these lines failed to parse and were
  dropped from the trace.

## [0.2.0] - 2026-10-13

### Added

- Parse the `+++ exited with N +++` and `+++ killed by SIGNAME
  [(core dumped)] +++` lines that report the end of a traced process into a
  new `Exit` type, carried as `Action::Exit`.
- Add `Exit::code`, `Exit::signal` and `Exit::is_success` to read a process's
  termination without matching on the variant.
- Join calls that `strace` splits into an `<unfinished ...>` line and a
  `<... name resumed>` line into the single `Event` the call would otherwise
  produce.

### Changed

- Rename the `syscalls::Syscall` enum to `Action` and the `Event.syscall`
  field to `Event.action`, now that a line can describe either a system call
  or a process exit.

## [0.1.0] - 2026-10-11

### Added

- Initial release.
- Parse `strace` output produced with `--follow-forks` and
  `--absolute-timestamps=format:unix,precision:us` into `Event`s, each carrying
  a pid, a timestamp and a `Syscall`.
- Typed records for the supported system calls: `open`, `openat`, `creat`,
  `close`, `chdir`, `fchdir`, `mkdir`, `mkdirat`, `rmdir`, `rename`,
  `renameat`, `renameat2`, `fork`, `vfork`, `clone`, `clone3`, `execve` and
  `execveat`, plus a `Generic` fallback for calls without a dedicated type.
- `stdprocess::stream` and, under the `tokio` feature, `tokio::stream` to run a
  command under `strace` and iterate over the parsed `Event`s.
- The `restrace run` debug binary to emit the parsed calls of a command.

[Unreleased]: https://github.com/lpenz/restrace/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/lpenz/restrace/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/lpenz/restrace/releases/tag/v0.1.0
