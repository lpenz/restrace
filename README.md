# restrace

[![CI](https://github.com/lpenz/restrace/actions/workflows/ci.yml/badge.svg)](https://github.com/lpenz/restrace/actions/workflows/ci.yml)
[![coveralls](https://coveralls.io/repos/github/lpenz/restrace/badge.svg?branch=main)](https://coveralls.io/github/lpenz/restrace?branch=main)
[![dependency status](https://deps.rs/repo/github/lpenz/restrace/status.svg)](https://deps.rs/repo/github/lpenz/restrace)
[![crates.io](https://img.shields.io/crates/v/restrace)](https://crates.io/crates/restrace)

*restrace* runs a command under [strace](https://strace.io) and parses
the emitted system calls into Rust values.

It is easier to explain the crate in terms of the types it provides:

- [`line::Line`]: a single, complete line of `strace` output, carrying the
  pid, the timestamp and a [`syscalls::Syscall`].
- [`syscalls`]: typed records for the supported system calls behind the
  [`syscalls::Syscall`] enum, plus a [`syscalls::Generic`] fallback.
- [`stdprocess`]: run a [`std::process::Command`] under `strace` and get a
  blocking iterator of [`line::Line`]s with [`stdprocess::stream`].
- [`tokio`]: the same for a `tokio::process::Command`, as an async stream
  of [`line::Line`]s; only compiled with the `tokio` feature, which is
  enabled by default.

`strace` must be installed in the environment where the commands run; this
crate only spawns it and parses its output.

## Parsing trace lines

A [`line::Line`] parses the `strace` output produced with
`--absolute-timestamps=format:unix,precision:us`, `--follow-forks` and a
trace redirected with `-o`, which makes `strace` prefix every line with the
pid of the process that made the call:

```rust
use restrace::line::Line;

let line: Line = "251751 1700000000.000000 execve(\"/bin/echo\", \
                  [\"echo\"], []) = 0"
    .parse()
    .unwrap();
assert_eq!(line.pid, 251751);
```

Every supported call parses into its own type inside [`syscalls`]; calls
without a dedicated type fall back to [`syscalls::Generic`], which keeps
the raw arguments and return value.

## Running a command under `strace`

[`stdprocess::stream`] wraps a [`std::process::Command`] under `strace` and
returns a blocking [`stdprocess::LineIter`] of parsed lines. The trace is
read from a dedicated pipe, so the standard streams of the traced process
are left untouched:

```rust
use std::process::Command;

let mut cmd = Command::new("ls");
cmd.arg("-l");
let mut lines = restrace::stdprocess::stream(cmd).unwrap();
for line in lines.by_ref() {
    println!("{line}");
}
lines.wait().unwrap();
```

## Tokio

With the `tokio` feature (enabled by default), [`tokio::stream`] does the
same for a `tokio::process::Command`, returning a [`tokio::LineStream`] of
[`line::Line`]s:

```rust
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use tokio::process::Command;
    use tokio_stream::StreamExt;

    let mut cmd = Command::new("ls");
    cmd.arg("-l");
    let mut lines = restrace::tokio::stream(cmd)?;
    while let Some(line) = lines.next().await {
        println!("{line}");
    }
    lines.wait().await?;
    Ok(())
}
```

[`line::Line`]: https://docs.rs/restrace/latest/restrace/line/struct.Line.html
[`syscalls`]: https://docs.rs/restrace/latest/restrace/syscalls/index.html
[`syscalls::Syscall`]: https://docs.rs/restrace/latest/restrace/syscalls/syscall/enum.Syscall.html
[`syscalls::Generic`]: https://docs.rs/restrace/latest/restrace/syscalls/generic/struct.Generic.html
[`stdprocess`]: https://docs.rs/restrace/latest/restrace/stdprocess/index.html
[`stdprocess::stream`]: https://docs.rs/restrace/latest/restrace/stdprocess/fn.stream.html
[`stdprocess::LineIter`]: https://docs.rs/restrace/latest/restrace/stdprocess/struct.LineIter.html
[`std::process::Command`]: https://doc.rust-lang.org/std/process/struct.Command.html
[`tokio`]: https://docs.rs/restrace/latest/restrace/tokio/index.html
[`tokio::stream`]: https://docs.rs/restrace/latest/restrace/tokio/fn.stream.html
[`tokio::LineStream`]: https://docs.rs/restrace/latest/restrace/tokio/struct.LineStream.html
