// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! Wrapping of [`tokio::process::Command`] values under `strace`.
//!
//! This module is only compiled when the `tokio` feature is enabled.

use ::tokio::process::Command;

/// Turns `cmd` into a command that runs the same program with the same
/// arguments, but under `strace`.
///
/// The returned command executes `strace --absolute-timestamps=format:unix,precision:us
/// --follow-forks <program> <args...>`, so that the traced calls can be read
/// with [`crate::line::Line`].  The working directory and environment of
/// `cmd` are copied over to the wrapped command.
#[must_use]
pub fn under_strace(cmd: Command) -> Command {
    crate::stdprocess::under_strace(cmd.into_std()).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn program_and_args() {
        let mut cmd = Command::new("echo");
        cmd.arg("hello");
        let wrapped = under_strace(cmd).into_std();
        assert_eq!(wrapped.get_program(), OsStr::new("strace"));
        assert_eq!(
            wrapped.get_args().collect::<Vec<_>>(),
            vec![
                OsStr::new("--absolute-timestamps=format:unix,precision:us"),
                OsStr::new("--follow-forks"),
                OsStr::new("echo"),
                OsStr::new("hello"),
            ]
        );
    }

    #[test]
    fn current_dir_and_env_copied() {
        let mut cmd = Command::new("pwd");
        cmd.current_dir("/tmp");
        cmd.env("FOO", "bar");
        let wrapped = under_strace(cmd).into_std();
        assert_eq!(
            wrapped.get_current_dir(),
            Some(std::path::Path::new("/tmp"))
        );
        let envs = wrapped.get_envs().collect::<Vec<_>>();
        assert_eq!(envs, vec![(OsStr::new("FOO"), Some(OsStr::new("bar")))]);
    }
}
