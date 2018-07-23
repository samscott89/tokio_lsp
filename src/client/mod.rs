//! Provides a generic LSP async client implementation, which implements
//! the `LspClient` trait, and language-specific implementations.
//! (Currently only for RLS).

use futures::{Future, Sink, Stream};
use futures::future;
use ls_types::*;
use ls_types::notification::Notification;
use ls_types::request::Request;
use jsonrpc::{self, server, Endpoint, Message, Parsed};
use jsonrpc::message::Response;
use serde;
use serde_json;
use tokio_core::reactor::Handle;

use std::io::Error as IoError;
use std::str;

use lsp::{InitializeOptions, LspClient};
use super::custom_err;

pub mod rust;

pub use self::rust::RlsClient;

/// A generic async client to a LSP implementation.
pub struct Client {
    pub(crate) inner: Option<jsonrpc::Client>,
}

impl Client {
    /// Create a new `Client` with the given connection and run futures on the
    /// provided handle.
    pub fn new<C>(connection: C, handle: &Handle) -> Self
        where
            C: Stream<Item = Parsed, Error = IoError>,
            C: Sink<SinkItem = Message, SinkError = IoError>,
            C: Send + 'static,
    {
        let (client, _fut) = Endpoint::client_only(connection).start(handle);
        Self {
            inner: Some(client),
        }
    }

    /// Create a new `Client` with a provided handler to handle incoming notifications.
    pub fn with_notification_handler<C, NH>(connection: C, notification_handler: NH, handle: &Handle) -> Self
        where
            C: Stream<Item = Parsed, Error = IoError>,
            C: Sink<SinkItem = Message, SinkError = IoError>,
            C: Send + 'static,
            NH: server::Server + 'static
    {
        let (client, _fut) = Endpoint::new(connection, notification_handler).start(handle);
        Self {
            inner: Some(client),
        }
    }
}


impl Client {
    /// Perfoms the main chunk of making a query from parameters to unwrapping
    /// the reponse
    ///
    /// Use this as a generic way to make `LspClient` calls.
    pub fn call<Req>(&mut self, params: Req::Params) -> Box<Future<Item=Req::Result, Error=IoError>>
        where Req: Request,
              Req::Params: serde::Serialize,
              Req::Result: serde::de::DeserializeOwned + 'static,
    {
        let params = match serde_json::to_value(params) {
            Ok(res) => res,
            Err(_e) => return Box::new(future::err(custom_err("Failed to serialize parameters"))),
        };
        let client = self.inner.take();
        let client = match client {
            None => return Box::new(future::err(custom_err("Tried to make a call on a poisoned client instance"))),
            Some(c) => c,
        };
        // self.inner is a impl Future<Client>
        let (client, fut) = match client.call(
                Req::METHOD.to_string(),
                Some(params),
                None,
        ).wait() {
            Ok(res) => res,
            Err(_e) => return Box::new(future::err(custom_err("Failed to send request"))),
        };
        self.inner = Some(client);
        Box::new(fut.then(|resp| {
            extract_response(resp)
        }))
    }

    /// Perfoms the main chunk of making a notification
    pub fn notify<Not> (&mut self, params: Not::Params)
        where Not: Notification,
              Not::Params: serde::Serialize,
    {
        let params = match serde_json::to_value(params) {
            Ok(res) => res,
            Err(e) => {eprintln!("{}", e); return},
        };
        let client = self.inner.take();
        let client: jsonrpc::Client = match client {
            None => {eprintln!("Missing client"); return},
            Some(c) => c,
        };
        // self.inner is a impl Future<Client>
        self.inner = match client.notify(
                Not::METHOD.to_string(),
                Some(params),
        ).wait() {
            Ok(res) => Some(res),
            Err(e) => {eprintln!("{}", e); return},
        };
    }

}


/// Extract/convert the result and map errors.
fn extract_response<T>(resp: Result<Option<Response>, IoError>) -> Result<T, IoError>
    where for<'de> T: serde::Deserialize<'de>
{
    let resp = resp.map_err(|e| custom_err(&format!("invalid response: {}", e)))?
                    .ok_or(custom_err("expected a response value"))?
                    .result.map_err(|_e| custom_err("invalid encoding"))?;

    serde_json::from_value(resp).map_err(|_e| custom_err("Failed to deserialize"))
}


macro_rules! lscall {
    (@req $fn_name:ident, $name:tt) => {
        fn $fn_name(&mut self, params: <lsp_request!($name) as Request>::Params) -> Box<Future<Item=<lsp_request!($name) as Request>::Result, Error=IoError>> {
            self.call::<lsp_request!($name)>(params)
        }
    };
    (@notify $fn_name:ident, $name:tt) => {
        fn $fn_name(&mut self, params: <lsp_notification!($name) as Notification>::Params) -> Result<(), IoError> {
            self.notify::<lsp_notification!($name)>(params);
            Ok(())
        }
    };
}

impl LspClient for Client {
    fn initialize(&mut self, params: InitializeParams) -> Box<Future<Item=Result<InitializeResult, InitializeError>, Error=IoError>> {
        Box::new(self.call::<InitializeOptions>(params).map(|opt| {
            match opt {
                InitializeOptions::Result(r) => Ok(r),
                InitializeOptions::Error(e) => Err(e),
            }
        }))
    }

    // lscall!(@notify $/cancelRequest, "$/cancelRequest");
    // lscall!(@notify initialized, "initialized");
    lscall!(@notify exit, "exit");
    // lscall!(@notify window/showMessage, "window/showMessage");
    // lscall!(@notify window/logMessage, "window/logMessage");
    // lscall!(@notify telemetry/event, "telemetry/event");
    lscall!(@notify did_open_text_document, "textDocument/didOpen");
    lscall!(@notify did_change_text_document, "textDocument/didChange");
    // lscall!(@notify textDocument/willSave, "textDocument/willSave");
    lscall!(@notify did_save_text_document, "textDocument/didSave");
    lscall!(@notify did_close_text_document, "textDocument/didClose");
    // lscall!(@notify textDocument/publishDiagnostics, "textDocument/publishDiagnostics");
    lscall!(@notify did_change_configuration, "workspace/didChangeConfiguration");
    lscall!(@notify did_change_watched_files, "workspace/didChangeWatchedFiles");

    // lscall!(@req initialize, "initialize");
    lscall!(@req shutdown, "shutdown");
    // lscall!(@req window/showMessageRequest, "window/showMessageRequest");
    // lscall!(@req client/registerCapability, "client/registerCapability");
    // lscall!(@req client/unregisterCapability, "client/unregisterCapability");
    lscall!(@req workspace_symbols, "workspace/symbol");
    // lscall!(@req workspace/executeCommand, "workspace/executeCommand");
    // lscall!(@req textDocument/willSaveWaitUntil, "textDocument/willSaveWaitUntil");
    lscall!(@req completion, "textDocument/completion");
    lscall!(@req resolve_completion_item, "completionItem/resolve");
    lscall!(@req hover, "textDocument/hover");
    lscall!(@req signature_help, "textDocument/signatureHelp");
    lscall!(@req goto_definition, "textDocument/definition");
    lscall!(@req references, "textDocument/references");
    lscall!(@req document_highlight, "textDocument/documentHighlight");
    lscall!(@req document_symbols, "textDocument/documentSymbol");
    lscall!(@req code_action, "textDocument/codeAction");
    lscall!(@req code_lens, "textDocument/codeLens");
    lscall!(@req code_lens_resolve, "codeLens/resolve");
    lscall!(@req document_link, "textDocument/documentLink");
    lscall!(@req document_link_resolve, "documentLink/resolve");
    // lscall!(@req textDocument/applyEdit, "textDocument/applyEdit");
    lscall!(@req range_formatting, "textDocument/rangeFormatting");
    lscall!(@req on_type_formatting, "textDocument/onTypeFormatting");
    lscall!(@req formatting, "textDocument/formatting");
    lscall!(@req rename, "textDocument/rename");
}
