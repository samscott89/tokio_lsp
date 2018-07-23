//! Basic support for async LSP clients using Tokio.
//!
//! Possible requests/notifications are type-checked (via the
//! `languageserver_types` crate).
//!
//! Also provides a basic codec for encoding/decoding the request format
//! which just handles the additional header string and uses the 
//! tokio_jsonrpc codec for the body.

extern crate bytes;
extern crate futures;
extern crate languageserver_types as ls_types;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate tokio;
extern crate tokio_io;
extern crate tokio_core;
extern crate tokio_jsonrpc as jsonrpc;


pub mod client;
mod codec;
mod lsp;
// pub mod sync;

pub use client::Client;
pub use codec::LspCodec;
pub use lsp::LspClient;

use std::io::{Error as IoError, ErrorKind};

pub(crate) fn custom_err(msg: &str) -> IoError {
    IoError::new(ErrorKind::Other, msg)
}

