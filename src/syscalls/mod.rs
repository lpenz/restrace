// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! System call definitions: arguments and return values.
//!
//! The concrete types are re-exported here; the modules that hold them are
//! private, so the layout of this directory is not part of the public API.
//!
//! The [`Action`](crate::Action) enum that unifies these records with the
//! process [`Exit`](crate::Exit) lives at the crate root.

mod bytes;
mod errno;
mod generic;
mod parse;

pub use bytes::Bytes;
pub use errno::Errno;
pub use generic::{Arg, Generic, Ret};
pub use parse::ParseError;
