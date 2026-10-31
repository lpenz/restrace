// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! restrace's debug binary: run a command under `strace` and emit the parsed
//! system calls to stderr, or to the file given by `--output`.

mod cli {
    //! restrace's CLI using [`clap`].
    //!
    //! [`clap`]: https://docs.rs/clap/latest/clap/

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
}

use ::tokio::io::{AsyncWrite, AsyncWriteExt, BufWriter};
use ::tokio_stream::StreamExt;
use clap::Parser;
use cli::{Cli, Command};
use color_eyre::Result;
use color_eyre::eyre::{WrapErr, eyre};

#[::tokio::main(crate = "::tokio", flavor = "current_thread")]
async fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::ACTIVE)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let Command::Run(args) = Cli::parse().command;
    let (program, program_args) = args
        .command
        .split_first()
        .ok_or_else(|| eyre!("no command was given"))?;
    let mut cmd = ::tokio::process::Command::new(program);
    cmd.args(program_args);
    let mut lines = restrace::tokio::stream(cmd).wrap_err("running the command under strace")?;

    let mut output: Box<dyn AsyncWrite + Unpin> = match &args.output {
        Some(path) => {
            let file = ::tokio::fs::File::create(path)
                .await
                .wrap_err_with(|| format!("creating output file {}", path.display()))?;
            Box::new(BufWriter::new(file))
        }
        None => Box::new(::tokio::io::stderr()),
    };

    while let Some(line) = lines.next().await {
        let line = line.wrap_err("reading the trace")?;
        output.write_all(line.to_string().as_bytes()).await?;
        output.write_all(b"\n").await?;
    }
    output.flush().await?;

    lines.wait().await.wrap_err("waiting for strace")?;
    Ok(())
}
