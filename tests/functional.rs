// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! Functional tests: run real commands under `strace` and parse the resulting
//! trace with [`restrace::line::Line`].
//!
//! Traces are read through [`restrace::tokio::stream`], the same path used by
//! the `restrace run` command: `strace --follow-forks -o /dev/fd/3` writes
//! every line with a bare pid prefix, which [`restrace::line::Line`] parses
//! into a mandatory pid.

use ::tokio::process::Command as TokioCommand;
use ::tokio_stream::StreamExt;
use restrace::line::Line;
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

/// Runs `cmd` under `strace` and returns the parsed [`Line`]s of its trace.
///
/// The trace is read through [`restrace::tokio::stream`], so every yielded
/// line carries a pid.
async fn trace(cmd: Command) -> Vec<Line> {
    let mut lines = restrace::tokio::stream(TokioCommand::from(cmd)).expect("spawn strace");
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
            .any(|line| matches!(line.syscall, restrace::syscalls::Syscall::Openat(_))),
        "expected a dedicated syscall type to be parsed: {lines:?}"
    );
    // `--follow-forks` with the trace redirected by `-o` makes strace prefix
    // every line with a pid, so every parsed line carries one.
    assert!(
        lines.iter().all(|line| line.pid > 0),
        "expected a pid on every line: {lines:?}"
    );
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
        let reparsed: Line = line.to_string().parse().expect("reparse displayed line");
        assert_eq!(line, reparsed);
    }
}
