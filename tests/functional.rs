// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! Functional tests: run real commands under `strace` and parse the resulting
//! output with [`restrace::line::Line`].

use restrace::line::Line;
use restrace::stdprocess::under_strace;
use restrace::syscalls::Syscall;
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

/// Runs `cmd` under `strace` and returns the parsed [`Line`]s of its traces.
///
/// Lines that do not form a complete system call (unfinished/resumed calls,
/// exit messages, ...) are skipped, matching [`restrace::tokio::LineStream`].
fn trace(cmd: Command) -> Vec<Line> {
    let out = under_strace(cmd).output().expect("run strace");
    String::from_utf8_lossy(&out.stderr)
        .lines()
        .filter_map(|line| line.parse::<Line>().ok())
        .collect()
}

#[test]
fn traces_a_single_process() {
    if !strace_available() {
        return;
    }
    let mut cmd = Command::new("echo");
    cmd.arg("hello");
    let lines = trace(cmd);

    assert!(!lines.is_empty(), "expected some parsed syscalls");
    assert!(
        lines
            .iter()
            .any(|line| matches!(line.syscall, Syscall::Openat(_))),
        "expected a dedicated syscall type to be parsed: {lines:?}"
    );
    // A single, non-forking process is printed by strace without a `[pid ...]`
    // prefix, so every parsed line has no pid.
    assert!(
        lines.iter().all(|line| line.pid.is_none()),
        "expected no pid prefixes for a single process: {lines:?}"
    );
}

#[test]
fn traces_forks() {
    if !strace_available() || !strace_can_trace_forks() {
        return;
    }
    let mut cmd = Command::new("sh");
    cmd.args(["-c", "sleep 0.02 & wait"]);
    let lines = trace(cmd);

    assert!(!lines.is_empty(), "expected some parsed syscalls");
    // The top-level process is printed without a pid, the forked children with
    // one.
    assert!(
        lines.iter().any(|line| line.pid.is_none()),
        "expected at least one line without a pid"
    );
    assert!(
        lines.iter().any(|line| line.pid.is_some_and(|pid| pid > 0)),
        "expected at least one line with a pid"
    );
}

#[test]
fn parsed_lines_round_trip() {
    if !strace_available() {
        return;
    }
    let mut cmd = Command::new("echo");
    cmd.arg("hello");
    let lines = trace(cmd);

    assert!(!lines.is_empty(), "expected some parsed syscalls");
    for line in lines {
        let reparsed: Line = line.to_string().parse().expect("reparse displayed line");
        assert_eq!(line, reparsed);
    }
}
