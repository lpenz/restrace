// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! Functional tests: run real commands under `strace` and parse the resulting
//! trace with [`restrace::Event`].
//!
//! Traces are read through [`restrace::tokio::stream`], the same path used by
//! the `restrace run` command: `strace --follow-forks -a 0 -o /dev/fd/3` writes
//! every line with a bare pid prefix, which [`restrace::Event`] parses
//! into a mandatory pid.

use ::tokio::process::Command as TokioCommand;
use ::tokio_stream::StreamExt;
use restrace::Event;
use restrace::syscalls::{Errno, OkResult};
use std::process::Command;

/// Returns whether `strace` is available in this environment.
fn strace_available() -> bool {
    Command::new("strace")
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// Returns whether `python3` is available in this environment.
fn python3_available() -> bool {
    Command::new("python3")
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// Returns whether `strace` can trace forks in this environment.
fn strace_can_trace_forks() -> bool {
    let out = Command::new("strace")
        .args([
            "--absolute-timestamps=format:unix,precision:us",
            "--follow-forks",
            "sh",
            "-c",
            "sleep 0.02 & wait",
        ])
        .output();
    match out {
        Ok(out) => String::from_utf8_lossy(&out.stderr).contains("[pid "),
        Err(_) => false,
    }
}

/// Runs `cmd` under `strace` and returns the parsed [`Event`]s of its trace.
///
/// The trace is read through [`restrace::tokio::stream`], so every yielded
/// line carries a pid.
async fn trace(cmd: Command) -> Vec<Event> {
    let mut lines = restrace::tokio::stream(TokioCommand::from(cmd)).expect("spawn strace");
    let mut collected = Vec::new();
    while let Some(line) = lines.next().await {
        collected.push(line);
    }
    lines.wait().await.expect("wait for strace");
    collected
}

/// Like [`trace`], but runs a shell command line.
async fn trace_shell(script: &str) -> Vec<Event> {
    let mut cmd = Command::new("sh");
    cmd.args(["-c", script]);
    trace(cmd).await
}

/// Like [`trace`], but inserts `extra` `strace` arguments between the base
/// flags and the traced program.
async fn trace_with<I, S>(cmd: Command, extra: I) -> Vec<Event>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let mut lines =
        restrace::tokio::stream_with(TokioCommand::from(cmd), extra).expect("spawn strace");
    let mut collected = Vec::new();
    while let Some(line) = lines.next().await {
        collected.push(line);
    }
    lines.wait().await.expect("wait for strace");
    collected
}

#[::tokio::test]
async fn traces_a_single_process() {
    if !strace_available() {
        return;
    }
    let mut cmd = Command::new("echo");
    cmd.arg("hello");
    let lines = trace(cmd).await;

    assert!(!lines.is_empty(), "expected some parsed syscalls");
    assert!(
        lines
            .iter()
            .any(|line| matches!(line.action, restrace::Action::Openat(_))),
        "expected a dedicated syscall type to be parsed: {lines:?}"
    );
    // strace reports the end of the process as an exit event.
    assert!(
        lines
            .iter()
            .any(|line| matches!(line.action, restrace::Action::Exit(_))),
        "expected an exit event: {lines:?}"
    );
    // `--follow-forks` with the trace redirected by `-o` makes strace prefix
    // every line with a pid, so every parsed line carries one.
    assert!(
        lines.iter().all(|line| line.pid > 0),
        "expected a pid on every line: {lines:?}"
    );
}

/// Runs a shell command line under `strace` and checks that the new
/// file-operation syscalls parse from a real trace.
///
/// `strace` is started with `-a 0`, so a short call like `close(2)` has no
/// column padding before its `=`, and it appends a parenthesized description
/// to a failing `errno`. The dynamic loader's `access(2)` check on
/// `/etc/ld.so.preload` covers the short call with an `ENOENT` description,
/// and `ln`/`rm` cover the link/symlink/unlink types.
#[::tokio::test]
async fn parses_described_and_short_syscalls() {
    if !strace_available() {
        return;
    }
    let lines = trace_shell(
        "d=$(mktemp -d); touch $d/a; ln $d/a $d/b; ln -s $d/a $d/c; \
         rm $d/b $d/c $d/a; rmdir $d; test -r /nonexistent; rm $d/missing 2>/dev/null",
    )
    .await;

    // The loader probes /etc/ld.so.preload with access(2); the call fails with
    // ENOENT, so the line carries a description that must be stripped.
    let access = lines
        .iter()
        .find_map(|line| match &line.action {
            restrace::Action::Access(access) => Some(access),
            _ => None,
        })
        .expect("expected the loader's access(2) check");
    assert_eq!(access.result, OkResult::Errno(Errno::ENOENT));

    // `close(2)` is a short call; it must parse as a typed Close and not fall
    // back to Generic.
    assert!(
        lines
            .iter()
            .any(|line| matches!(line.action, restrace::Action::Close(_))),
        "expected a parsed close(2): {lines:?}"
    );

    // The file operations must parse as their dedicated types.
    let has_link = lines.iter().any(|line| {
        matches!(
            line.action,
            restrace::Action::Link(_) | restrace::Action::Linkat(_)
        )
    });
    let has_symlink = lines.iter().any(|line| {
        matches!(
            line.action,
            restrace::Action::Symlink(_) | restrace::Action::Symlinkat(_)
        )
    });
    let has_unlink = lines.iter().any(|line| {
        matches!(
            line.action,
            restrace::Action::Unlink(_) | restrace::Action::Unlinkat(_)
        )
    });
    assert!(has_link, "expected a link/linkat call: {lines:?}");
    assert!(has_symlink, "expected a symlink/symlinkat call: {lines:?}");
    assert!(has_unlink, "expected an unlink/unlinkat call: {lines:?}");

    // The failing `rm` reports an errno description on unlink; it must parse as
    // the typed call with the errno stripped, not fall back to Generic.
    let failed_unlink = lines.iter().any(|line| match &line.action {
        restrace::Action::Unlink(unlink) => !unlink.result.is_ok(),
        restrace::Action::Unlinkat(unlinkat) => !unlinkat.result.is_ok(),
        _ => false,
    });
    assert!(
        failed_unlink,
        "expected a failing unlink/unlinkat with a stripped errno: {lines:?}"
    );
}

/// Runs a shell command line whose redirections make the shell `dup2(2)` the
/// standard descriptors, and checks that the calls parse as `Action::Dup2`.
#[::tokio::test]
async fn parses_dup2() {
    if !strace_available() {
        return;
    }
    let lines = trace_shell("exec 3>&1; echo x >&3; exec 3>&-").await;

    assert!(
        lines
            .iter()
            .any(|line| matches!(line.action, restrace::Action::Dup2(_))),
        "expected a parsed dup2(2): {lines:?}"
    );
}

/// Runs a shell command line whose pipeline makes `cat(1)` read the
/// producer's output through a pipe, and checks that the call parses as
/// `Action::Read` with the bytes decoded from its buffer.
#[::tokio::test]
async fn parses_read() {
    if !strace_available() {
        return;
    }
    let lines = trace_shell("printf 'hi\\n' | cat").await;

    let buf = lines
        .iter()
        .filter_map(|line| match &line.action {
            restrace::Action::Read(read) => read.buf.decoded(),
            _ => None,
        })
        .find(|buf| buf.0 == b"hi\n")
        .expect("expected a parsed read(2) with the pipe bytes");
    assert_eq!(buf.0, b"hi\n");
}

/// Runs a `/bin/pwd`, which resolves the current working directory with
/// `getcwd(2)`, and checks that the call parses as `Action::Getcwd` with the
/// path decoded from its buffer.
#[::tokio::test]
async fn parses_getcwd() {
    if !strace_available() {
        return;
    }
    let lines = trace_shell("/bin/pwd").await;

    let buf = lines
        .iter()
        .filter_map(|line| match &line.action {
            restrace::Action::Getcwd(getcwd) => getcwd.buf.decoded(),
            _ => None,
        })
        .find(|buf| !buf.0.to_bytes().is_empty())
        .expect("expected a parsed getcwd(2) with the current directory");
    assert!(buf.0.to_bytes().starts_with(b"/"));
}

/// Runs a shell command line whose builtin `printf` writes to its standard
/// output, and checks that the call parses as `Action::Write` with the bytes
/// decoded from its buffer.
#[::tokio::test]
async fn parses_write() {
    if !strace_available() {
        return;
    }
    let lines = trace_shell("printf 'hi\\n'").await;

    let buf = lines
        .iter()
        .filter_map(|line| match &line.action {
            restrace::Action::Write(write) => write.buf.decoded(),
            _ => None,
        })
        .find(|buf| buf.0 == b"hi\n")
        .expect("expected a parsed write(2) with the buffer bytes");
    assert_eq!(buf.0, b"hi\n");
}

/// Runs a Python one-liner whose `os.fsync` call makes the traced process
/// issue a `fsync(2)`, and checks that the call parses as `Action::Fsync`.
///
/// No common shell command issues `fsync(2)` directly, so this test needs
/// `python3` on top of `strace`; it skips itself when either is missing.
#[::tokio::test]
async fn parses_fsync() {
    if !strace_available() || !python3_available() {
        return;
    }
    let path = std::env::temp_dir().join(format!("restrace-fsync-{}.txt", std::process::id()));
    let script = format!(
        "import os; fd = os.open({path:?}, os.O_CREAT | os.O_WRONLY, 0o644); \
         os.write(fd, b'hi\\n'); os.fsync(fd); os.close(fd); os.unlink({path:?})",
        path = path.display().to_string()
    );
    let mut cmd = Command::new("python3");
    cmd.args(["-c", &script]);
    let lines = trace(cmd).await;

    let fsync = lines
        .iter()
        .find_map(|line| match &line.action {
            restrace::Action::Fsync(fsync) => Some(fsync),
            _ => None,
        })
        .expect("expected a parsed fsync(2)");
    assert!(fsync.result.is_ok(), "expected a successful fsync(2)");
}

/// Runs a Python one-liner whose `os.fdatasync` call makes the traced process
/// issue a `fdatasync(2)`, and checks that the call parses as
/// `Action::Fdatasync`.
///
/// No common shell command issues `fdatasync(2)` directly, so this test needs
/// `python3` on top of `strace`; it skips itself when either is missing.
#[::tokio::test]
async fn parses_fdatasync() {
    if !strace_available() || !python3_available() {
        return;
    }
    let path = std::env::temp_dir().join(format!("restrace-fdatasync-{}.txt", std::process::id()));
    let script = format!(
        "import os; fd = os.open({path:?}, os.O_CREAT | os.O_WRONLY, 0o644); \
         os.write(fd, b'hi\\n'); os.fdatasync(fd); os.close(fd); os.unlink({path:?})",
        path = path.display().to_string()
    );
    let mut cmd = Command::new("python3");
    cmd.args(["-c", &script]);
    let lines = trace(cmd).await;

    let fdatasync = lines
        .iter()
        .find_map(|line| match &line.action {
            restrace::Action::Fdatasync(fdatasync) => Some(fdatasync),
            _ => None,
        })
        .expect("expected a parsed fdatasync(2)");
    assert!(
        fdatasync.result.is_ok(),
        "expected a successful fdatasync(2)"
    );
}

/// Runs a shell command line whose `ldd(1)` writes its report with
/// `writev(2)`, and checks that the call parses as `Action::Writev` with the
/// bytes decoded from one of its buffers.
#[::tokio::test]
async fn parses_writev() {
    if !strace_available() {
        return;
    }
    let lines = trace_shell("ldd /bin/ls").await;

    let decoded = lines
        .iter()
        .filter_map(|line| match &line.action {
            restrace::Action::Writev(writev) => writev.iov.decoded(),
            _ => None,
        })
        .find(|list| {
            list.as_slice().iter().any(|iovec| {
                iovec
                    .base
                    .decoded()
                    .is_some_and(|bytes| !bytes.0.is_empty())
            })
        })
        .expect("expected a parsed writev(2) with a decoded buffer");
    assert!(!decoded.is_empty());
}

/// Runs a Python one-liner whose `os.copy_file_range` call makes the traced
/// process issue a `copy_file_range(2)`, and checks that the call parses as
/// `Action::CopyFileRange`.
///
/// No common shell command issues `copy_file_range(2)` directly, so this test
/// needs `python3` on top of `strace`; it skips itself when either is missing.
#[::tokio::test]
async fn parses_copy_file_range() {
    if !strace_available() || !python3_available() {
        return;
    }
    let dir = std::env::temp_dir();
    let src = dir.join(format!("restrace-cfr-src-{}", std::process::id()));
    let dst = dir.join(format!("restrace-cfr-dst-{}", std::process::id()));
    let script = format!(
        "import os; open({src:?}, 'wb').write(b'hello'); \
         fd_in = os.open({src:?}, os.O_RDONLY); fd_out = os.open({dst:?}, os.O_CREAT | os.O_WRONLY, 0o644); \
         os.copy_file_range(fd_in, fd_out, 5, 0, 0); os.close(fd_in); os.close(fd_out); \
         os.unlink({src:?}); os.unlink({dst:?})",
        src = src.display().to_string(),
        dst = dst.display().to_string()
    );
    let mut cmd = Command::new("python3");
    cmd.args(["-c", &script]);
    let lines = trace(cmd).await;

    assert!(
        lines
            .iter()
            .any(|line| matches!(line.action, restrace::Action::CopyFileRange(_))),
        "expected a parsed copy_file_range(2): {lines:?}"
    );
}

/// Runs a Python one-liner whose `os.pwritev` call makes the traced process
/// issue a `pwritev2(2)` — the syscall glibc implements `os.pwritev` with —
/// and checks that the call parses as `Action::Pwritev2` with the bytes
/// decoded from one of its buffers.
///
/// No common shell command issues the `pwritev` family directly, so this
/// test needs `python3` on top of `strace`; it skips itself when either is
/// missing.
#[::tokio::test]
async fn parses_pwritev2() {
    if !strace_available() || !python3_available() {
        return;
    }
    let path = std::env::temp_dir().join(format!("restrace-pwritev2-{}.out", std::process::id()));
    let script = format!(
        "import os; fd = os.open({path:?}, os.O_CREAT | os.O_WRONLY, 0o644); \
         os.pwritev(fd, [b'hi\\n'], 0); os.close(fd); os.unlink({path:?})",
        path = path.display().to_string()
    );
    let mut cmd = Command::new("python3");
    cmd.args(["-c", &script]);
    let lines = trace(cmd).await;

    let decoded = lines
        .iter()
        .filter_map(|line| match &line.action {
            restrace::Action::Pwritev2(pwritev2) => pwritev2.iov.decoded(),
            _ => None,
        })
        .find(|list| {
            list.as_slice()
                .iter()
                .any(|iovec| iovec.base.decoded().is_some_and(|bytes| bytes.0 == b"hi\n"))
        })
        .expect("expected a parsed pwritev2(2) with a decoded buffer");
    assert!(!decoded.is_empty());
}

/// Runs a shell command line whose command substitution makes the shell create
/// a `pipe2(2)`, and checks that the call parses as `Action::Pipe2`.
#[::tokio::test]
async fn parses_pipe2() {
    if !strace_available() {
        return;
    }
    let lines = trace_shell("x=$(echo hi); echo $x").await;

    assert!(
        lines
            .iter()
            .any(|line| matches!(line.action, restrace::Action::Pipe2(_))),
        "expected a parsed pipe2(2): {lines:?}"
    );
}

/// Runs a shell command line whose dynamic loader maps the executable and its
/// shared libraries with `mmap(2)`, and checks that the call parses as
/// `Action::Mmap`.
#[::tokio::test]
async fn parses_mmap() {
    if !strace_available() {
        return;
    }
    let lines = trace_shell("true").await;

    assert!(
        lines
            .iter()
            .any(|line| matches!(line.action, restrace::Action::Mmap(_))),
        "expected a parsed mmap(2): {lines:?}"
    );
}

/// Runs a Python script that moves bytes from a file to a pipe with
/// `splice(2)`, and checks that the call parses as `Action::Splice`.
///
/// No common shell command issues `splice` directly, so this test needs
/// `python3` on top of `strace`; it skips itself when either is missing.
#[::tokio::test]
async fn parses_splice() {
    if !strace_available() || !python3_available() {
        return;
    }
    let script = "import os; fd = os.open('/etc/passwd', os.O_RDONLY); \
                  r, w = os.pipe(); os.splice(fd, w, 5); \
                  os.close(fd); os.close(r); os.close(w)";
    let mut cmd = Command::new("python3");
    cmd.args(["-c", script]);
    let lines = trace(cmd).await;

    let splice = lines
        .iter()
        .find_map(|line| match &line.action {
            restrace::Action::Splice(splice) => Some(splice),
            _ => None,
        })
        .expect("expected a parsed splice(2)");
    assert_eq!(splice.len, 5);
    assert!(splice.off_in.decoded().is_none(), "expected a NULL offset");
    assert_eq!(splice.result.byte_count(), Some(5));
}

/// Runs a Python one-liner whose `os.sendfile` call moves bytes from a file
/// to its standard output, and checks that the call parses as
/// `Action::Sendfile` with the offset it started from and the value the call
/// moved it to.
///
/// No common shell command issues `sendfile` directly, so this test needs
/// `python3` on top of `strace`; it skips itself when either is missing.
#[::tokio::test]
async fn parses_sendfile() {
    if !strace_available() || !python3_available() {
        return;
    }
    let script = "import os; fd = os.open('/etc/passwd', os.O_RDONLY); \
                  os.sendfile(1, fd, 0, 5); os.close(fd)";
    let mut cmd = Command::new("python3");
    cmd.args(["-c", script]);
    let lines = trace(cmd).await;

    let sendfile = lines
        .iter()
        .find_map(|line| match &line.action {
            restrace::Action::Sendfile(sendfile) => Some(sendfile),
            _ => None,
        })
        .expect("expected a parsed sendfile(2)");
    assert_eq!(sendfile.count, 5);
    assert_eq!(sendfile.result.byte_count(), Some(5));
    let offset = sendfile
        .offset
        .decoded()
        .expect("expected a decoded offset");
    assert_eq!(offset.value, 0);
    assert_eq!(offset.moved_to, Some(5));
}

/// Runs a `stat(1)` call, which glibc implements with `statx(2)` on modern
/// systems, and checks that the call parses as `Action::Statx` with the
/// buffer decoded.
///
/// No common shell builtin issues `statx` directly, so this test needs a
/// `stat` binary on top of `strace`; it skips itself when either is missing.
#[::tokio::test]
async fn parses_statx() {
    if !strace_available() {
        return;
    }
    let lines = trace_with(
        {
            let mut cmd = Command::new("stat");
            cmd.arg("/etc/hostname");
            cmd
        },
        ["-e", "trace=statx"],
    )
    .await;

    let statx = lines
        .iter()
        .find_map(|line| match &line.action {
            restrace::Action::Statx(statx) => Some(statx),
            _ => None,
        })
        .expect("expected a parsed statx(2)");
    let buf = statx
        .buf
        .decoded()
        .expect("expected a decoded statx buffer");
    assert!(buf.blksize > 0, "expected a decoded block size");
    assert!(buf.size.is_some(), "expected a decoded size");
}

#[::tokio::test]
async fn exits_with_code() {
    if !strace_available() {
        return;
    }
    let mut cmd = Command::new("sh");
    cmd.args(["-c", "exit 42"]);
    let lines = trace(cmd).await;

    let exit = lines
        .iter()
        .find_map(|line| match &line.action {
            restrace::Action::Exit(exit) => Some(exit),
            _ => None,
        })
        .expect("expected an exit event");
    assert_eq!(exit.code(), Some(42));
    assert_eq!(exit.signal(), None);
    assert!(!exit.is_success());
}

#[::tokio::test]
async fn killed_by_signal() {
    if !strace_available() {
        return;
    }
    let mut cmd = Command::new("sh");
    cmd.args(["-c", "kill -KILL $$"]);
    let lines = trace(cmd).await;

    let exit = lines
        .iter()
        .find_map(|line| match &line.action {
            restrace::Action::Exit(exit) => Some(exit),
            _ => None,
        })
        .expect("expected an exit event");
    assert_eq!(exit.code(), None);
    assert_eq!(exit.signal(), Some("SIGKILL"));
    assert!(!exit.is_success());
}

#[::tokio::test]
async fn traces_forks() {
    if !strace_available() || !strace_can_trace_forks() {
        return;
    }
    let mut cmd = Command::new("sh");
    cmd.args(["-c", "sleep 0.02 & wait"]);
    let lines = trace(cmd).await;

    assert!(!lines.is_empty(), "expected some parsed syscalls");
    // The top-level process and the forked children all carry a pid, and the
    // `--follow-forks` tracing of the child shows up as additional pids.
    assert!(
        lines.iter().all(|line| line.pid > 0),
        "expected a pid on every line: {lines:?}"
    );
    let pids: std::collections::BTreeSet<u32> = lines.iter().map(|line| line.pid).collect();
    assert!(
        pids.len() > 1,
        "expected the forked children to show up with distinct pids"
    );
}

#[::tokio::test]
async fn parsed_lines_round_trip() {
    if !strace_available() {
        return;
    }
    let mut cmd = Command::new("echo");
    cmd.arg("hello");
    let lines = trace(cmd).await;

    assert!(!lines.is_empty(), "expected some parsed syscalls");
    for line in lines {
        let reparsed: Event = line.to_string().parse().expect("reparse displayed line");
        assert_eq!(line, reparsed);
    }
}

/// The command used by the reconstruction tests: several processes doing file
/// calls at the same time make `strace` split many of them into an
/// `<unfinished ...>` line and a `<... name resumed>` line.
const SPLIT_CALLS_CMD: &[&str] = &[
    "sh",
    "-c",
    "for i in 1 2 3 4 5 6 7 8; do ls /usr/bin >/dev/null & done; wait",
];

#[::tokio::test]
async fn stream_reconstructs_split_calls() {
    if !strace_available() || !strace_can_trace_forks() {
        return;
    }
    let mut cmd = Command::new("sh");
    cmd.args(["-c", SPLIT_CALLS_CMD[2]]);
    let events = trace(cmd).await;

    assert!(!events.is_empty(), "expected some parsed syscalls");
    assert!(
        events.iter().all(|event| event.pid > 0),
        "expected a pid on every event: {events:?}"
    );
    // A stream that only accepted complete, single-line calls would parse
    // strictly fewer events than the ones `strace` splits and reconstructs.
    // The reference is a separate run of the same command, so the comparison
    // is only safe because the command produces far more split calls than
    // complete ones; the same-input check is the `reconstructs_real_split_calls`
    // unit test in `src/reconstruct.rs`.
    let text = raw_trace(SPLIT_CALLS_CMD);
    let single_line = text
        .lines()
        .filter(|line| line.parse::<Event>().is_ok())
        .count();
    assert!(single_line > 0, "expected some complete syscalls");
    assert!(
        events.len() > single_line,
        "expected the stream to reconstruct the split calls: \
         {} events vs {single_line} single-line calls",
        events.len()
    );
}

/// Runs `cmd` under `strace`, capturing the raw trace to a temporary file.
fn raw_trace(cmd: &[&str]) -> String {
    // Tests run in parallel threads of the same process, so the path must be
    // unique per call, not just per process.
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("restrace-trace-{}-{n}.log", std::process::id()));
    let status = Command::new("strace")
        .args([
            "--absolute-timestamps=format:unix,precision:us",
            "--follow-forks",
            "-o",
        ])
        .arg(&path)
        .args(cmd)
        .status()
        .expect("run strace");
    assert!(status.success(), "strace failed");
    let text = std::fs::read_to_string(&path).expect("read the raw trace");
    std::fs::remove_file(&path).expect("remove the temporary trace");
    text
}

/// Checks that `strace -y` annotations are parsed when present. A `cat(1)`
/// reading from `/dev/null` produces `openat` with the resolved path on the
/// result and `read` with the resolved path on the descriptor, which the
/// parser must round-trip.
#[::tokio::test]
async fn parses_y_annotated_fd() {
    if !strace_available() {
        return;
    }
    let mut cmd = Command::new("cat");
    cmd.arg("/dev/null");
    let lines = trace_with(cmd, ["-e", "trace=openat,read"]).await;

    let has_annotated_openat = lines.iter().any(|line| {
        if let restrace::Action::Openat(openat) = &line.action {
            if let restrace::syscalls::FdResult::Fd(fd) = &openat.result {
                fd.path()
                    .and_then(|p| p.to_str().ok())
                    .is_some_and(|p| p.starts_with("/dev/null"))
            } else {
                false
            }
        } else {
            false
        }
    });
    let has_annotated_read = lines.iter().any(|line| {
        if let restrace::Action::Read(read) = &line.action {
            read.fd
                .path()
                .and_then(|p| p.to_str().ok())
                .is_some_and(|p| p.starts_with("/dev/null"))
        } else {
            false
        }
    });
    assert!(
        has_annotated_openat,
        "expected annotated openat result: {lines:?}"
    );
    assert!(has_annotated_read, "expected annotated read fd: {lines:?}");
}
