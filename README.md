# restrace

[![CI](https://github.com/lpenz/restrace/actions/workflows/ci.yml/badge.svg)](https://github.com/lpenz/restrace/actions/workflows/ci.yml)
[![coveralls](https://coveralls.io/repos/github/lpenz/restrace/badge.svg?branch=main)](https://coveralls.io/github/lpenz/restrace?branch=main)
[![dependency status](https://deps.rs/repo/github/lpenz/restrace/status.svg)](https://deps.rs/repo/github/lpenz/restrace)
[![crates.io](https://img.shields.io/crates/v/restrace)](https://crates.io/crates/restrace)

*restrace* runs a command under [strace](https://strace.io) and parses
the emitted system calls into Rust values.

It is easier to explain the crate in terms of the types it provides:

- [`Event`]: a single, complete line of `strace` output, carrying the
  pid, the timestamp and an [`Action`].
- [`Action`]: what the line reports: a system call record or the [`Exit`]
  that terminated the process.
- [`syscalls`]: typed records for the supported system calls, plus a
  [`syscalls::Generic`] fallback for calls without a dedicated type.
- [`stdprocess`]: run a [`std::process::Command`] under `strace` and get a
  blocking iterator of [`Event`]s with [`stdprocess::stream`].
- [`tokio`]: the same for a `tokio::process::Command`, as an async stream
  of [`Event`]s; only compiled with the `tokio` feature, which is
  enabled by default.

`strace` must be installed in the environment where the commands run; this
crate only spawns it and parses its output.

## Parsing trace lines

An [`Event`] parses the `strace` output produced with
`--absolute-timestamps=format:unix,precision:us`, `--follow-forks` and a
trace redirected with `-o`, which makes `strace` prefix every line with the
pid of the process that made the call:

```rust
use restrace::Event;

let line: Event = "251751 1700000000.000000 execve(\"/bin/echo\", \
                  [\"echo\"], []) = 0"
    .parse()
    .unwrap();
assert_eq!(line.pid, 251751);
```

Every supported call parses into its own type inside [`syscalls`]; calls
without a dedicated type fall back to [`syscalls::Generic`], which keeps
the raw arguments and return value. The `+++ exited with ... +++` and
`+++ killed by ... +++` lines that report the end of a traced process
parse into [`Exit`], carried as [`Action::Exit`].

## Reconstructing split calls

When more than one process or thread is traced at a time, a call that is
still running when another traced process emits output is printed in two
lines: the beginning ends with `<unfinished ...>` and the end begins with
`<... name resumed>`. [`stdprocess::stream`] and [`tokio::stream`] join
the two lines of every pid into the single [`Event`] the call would
otherwise produce, keeping the pid and the timestamp of the unfinished
line.

Reconstructed events keep the timestamp of the unfinished line, so they
are not necessarily ordered by time: a call that started early can be
emitted after one that completed in between. A resumed call whose merged
text is not decodable, such as one that resumes with the indeterminate
result `= ?`, is dropped, as it would be if `strace` had printed it on one
line.

## Running a command under `strace`

[`stdprocess::stream`] wraps a [`std::process::Command`] under `strace` and
returns a blocking [`stdprocess::EventIter`] of parsed lines. The trace is
read from a dedicated pipe, so the standard streams of the traced process
are left untouched:

```rust
use std::process::Command;

let mut cmd = Command::new("ls");
cmd.arg("-l");
let mut lines = restrace::stdprocess::stream(cmd).unwrap();
for line in lines.by_ref() {
    println!("{}", line.expect("read trace"));
}
lines.wait().unwrap();
```

## Tokio

With the `tokio` feature (enabled by default), [`tokio::stream`] does the
same for a `tokio::process::Command`, returning a [`tokio::EventStream`] of
[`Event`]s:

```rust
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use tokio::process::Command;
    use tokio_stream::StreamExt;

    let mut cmd = Command::new("ls");
    cmd.arg("-l");
    let mut lines = restrace::tokio::stream(cmd)?;
    while let Some(line) = lines.next().await {
        println!("{}", line?);
    }
    lines.wait().await?;
    Ok(())
}
```

[`Event`]: https://docs.rs/restrace/latest/restrace/struct.Event.html
[`Action`]: https://docs.rs/restrace/latest/restrace/enum.Action.html
[`Action::Exit`]: https://docs.rs/restrace/latest/restrace/enum.Action.html#variant.Exit
[`syscalls`]: https://docs.rs/restrace/latest/restrace/syscalls/index.html
[`Exit`]: https://docs.rs/restrace/latest/restrace/enum.Exit.html
[`syscalls::Generic`]: https://docs.rs/restrace/latest/restrace/syscalls/struct.Generic.html
[`stdprocess`]: https://docs.rs/restrace/latest/restrace/stdprocess/index.html
[`stdprocess::stream`]: https://docs.rs/restrace/latest/restrace/stdprocess/fn.stream.html
[`stdprocess::EventIter`]: https://docs.rs/restrace/latest/restrace/stdprocess/struct.EventIter.html
[`std::process::Command`]: https://doc.rust-lang.org/std/process/struct.Command.html
[`tokio`]: https://docs.rs/restrace/latest/restrace/tokio/index.html
[`tokio::stream`]: https://docs.rs/restrace/latest/restrace/tokio/fn.stream.html
[`tokio::EventStream`]: https://docs.rs/restrace/latest/restrace/tokio/struct.EventStream.html
