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
