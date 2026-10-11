// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

// restrace's CLI using [`clap`]
//
// This is not a module-level doc because we `include!` it in build.rs.
//
// [`clap`]: https://docs.rs/clap/latest/clap/

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about,
    long_about = "restrace runs a command under strace and emits the parsed system calls.\n\nThe trace is printed to stderr, or written to a file with --output."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

/// The subcommand to run.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Run a command under strace and emit the parsed system calls
    Run(RunArgs),
}

/// Arguments of the `run` subcommand.
#[derive(Args, Debug)]
pub struct RunArgs {
    /// Write the trace to this file instead of stderr
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,

    /// The command to run, followed by its arguments
    #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
    pub command: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_subcommand() {
        assert!(Cli::try_parse_from(["restrace"]).is_err());
    }

    #[test]
    fn run_requires_command() {
        assert!(Cli::try_parse_from(["restrace", "run"]).is_err());
    }

    #[test]
    fn run_command() {
        let cli = Cli::try_parse_from(["restrace", "run", "echo", "hello"]).unwrap();
        match cli.command {
            Command::Run(args) => {
                assert_eq!(args.command, vec!["echo", "hello"]);
                assert!(args.output.is_none());
            }
        }
    }

    #[test]
    fn run_output() {
        let cli =
            Cli::try_parse_from(["restrace", "run", "--output", "trace.log", "echo"]).unwrap();
        match cli.command {
            Command::Run(args) => {
                assert_eq!(args.output, Some(PathBuf::from("trace.log")));
                assert_eq!(args.command, vec!["echo"]);
            }
        }
    }

    #[test]
    fn run_hyphen_args() {
        let cli = Cli::try_parse_from(["restrace", "run", "ls", "-l"]).unwrap();
        match cli.command {
            Command::Run(args) => assert_eq!(args.command, vec!["ls", "-l"]),
        }
    }
}
