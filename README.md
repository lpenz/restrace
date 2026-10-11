# restrace

[![CI](https://github.com/lpenz/restrace/actions/workflows/ci.yml/badge.svg)](https://github.com/lpenz/restrace/actions/workflows/ci.yml)
[![coveralls](https://coveralls.io/repos/github/lpenz/restrace/badge.svg?branch=main)](https://coveralls.io/github/lpenz/restrace?branch=main)
[![dependency status](https://deps.rs/repo/github/lpenz/restrace/status.svg)](https://deps.rs/repo/github/lpenz/restrace)
[![crates.io](https://img.shields.io/crates/v/restrace)](https://crates.io/crates/restrace)

restrace runs a command under [strace](https://strace.io) and emits the
parsed system calls:

```bash
restrace run ls -l
```

The trace is printed to stderr, or written to a file with `--output`:

```bash
restrace run --output trace.log sleep 1
```
