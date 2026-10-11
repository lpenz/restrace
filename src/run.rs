// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The `run` subcommand: run a command under `strace` and emit the parsed
//! [`Line`](crate::line::Line)s.
//!
//! It uses the tokio integration in [`crate::tokio`] to stream and parse the
//! trace of the wrapped command, printing each [`Line`](crate::line::Line) to
//! stderr or to the file given by `--output`.

use crate::cli::RunArgs;
use ::tokio::io::{AsyncWrite, AsyncWriteExt, BufWriter};
use ::tokio_stream::StreamExt;
use color_eyre::Result;
use color_eyre::eyre::{WrapErr, eyre};

/// Runs the command described by `args` under `strace`, emitting its parsed
/// [`Line`](crate::line::Line)s to stderr, or to the file given by `--output`.
///
/// # Errors
///
/// Returns an error if no command is given, if the output file cannot be
/// created, or if `strace` cannot be run.
pub async fn run(args: RunArgs) -> Result<()> {
    let (program, program_args) = args
        .command
        .split_first()
        .ok_or_else(|| eyre!("no command was given"))?;
    let mut cmd = ::tokio::process::Command::new(program);
    cmd.args(program_args);
    let mut lines = crate::tokio::stream(cmd).wrap_err("running the command under strace")?;

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
        output.write_all(line.to_string().as_bytes()).await?;
        output.write_all(b"\n").await?;
    }
    output.flush().await?;

    lines.wait().await.wrap_err("waiting for strace")?;
    Ok(())
}
