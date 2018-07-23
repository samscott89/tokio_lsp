//! Traits for the language server protocol clients

#![allow(unused_variables)]

use custom_err;
use futures::{future, Future};
use ls_types::*;
use ls_types::notification::Notification;
use ls_types::request::Request;

use std::io::Error;

macro_rules! lsdef {
    (@req $fn_name:ident, $name:tt) => {
        fn $fn_name(&mut self, params: <lsp_request!($name) as Request>::Params) -> Box<Future<Item=<lsp_request!($name) as Request>::Result, Error=Error>> {
            Box::new(future::err(custom_err("Not implemented")))
        }
    };
    (@notify $fn_name:ident, $name:tt) => {
        fn $fn_name(&mut self, params: <lsp_notification!($name) as Notification>::Params) -> Result<(), Error> {
            Err(custom_err("Not implemented"))
        }
    };
}

#[derive(Deserialize)]
#[serde(untagged)]
pub(crate) enum InitializeOptions {
    Result(InitializeResult),
    Error(InitializeError),
}

impl Request for InitializeOptions {
    type Params = <lsp_request!("initialize") as Request>::Params;
    type Result = Self;
    const METHOD: &'static str = "initialize";
}


/// Trait encapsulating a client to the language server protocol
pub trait LspClient {
    fn initialize(&mut self, params: InitializeParams) -> Box<Future<Item=Result<InitializeResult, InitializeError>, Error=Error>>;

    // lsdef!(@notify $/cancelRequest, "$/cancelRequest");
    // lsdef!(@notify initialized, "initialized");
    lsdef!(@notify exit, "exit");
    // lsdef!(@notify window/showMessage, "window/showMessage");
    // lsdef!(@notify window/logMessage, "window/logMessage");
    // lsdef!(@notify telemetry/event, "telemetry/event");
    lsdef!(@notify did_open_text_document, "textDocument/didOpen");
    lsdef!(@notify did_change_text_document, "textDocument/didChange");
    // lsdef!(@notify textDocument/willSave, "textDocument/willSave");
    lsdef!(@notify did_save_text_document, "textDocument/didSave");
    lsdef!(@notify did_close_text_document, "textDocument/didClose");
    // lsdef!(@notify textDocument/publishDiagnostics, "textDocument/publishDiagnostics");
    lsdef!(@notify did_change_configuration, "workspace/didChangeConfiguration");
    lsdef!(@notify did_change_watched_files, "workspace/didChangeWatchedFiles");

    // lsdef!(@req initialize, "initialize");
    lsdef!(@req shutdown, "shutdown");
    // lsdef!(@req window/showMessageRequest, "window/showMessageRequest");
    // lsdef!(@req client/registerCapability, "client/registerCapability");
    // lsdef!(@req client/unregisterCapability, "client/unregisterCapability");
    lsdef!(@req workspace_symbols, "workspace/symbol");
    // lsdef!(@req workspace/executeCommand, "workspace/executeCommand");
    // lsdef!(@req textDocument/willSaveWaitUntil, "textDocument/willSaveWaitUntil");
    lsdef!(@req completion, "textDocument/completion");
    lsdef!(@req resolve_completion_item, "completionItem/resolve");
    lsdef!(@req hover, "textDocument/hover");
    lsdef!(@req signature_help, "textDocument/signatureHelp");
    lsdef!(@req goto_definition, "textDocument/definition");
    lsdef!(@req references, "textDocument/references");
    lsdef!(@req document_highlight, "textDocument/documentHighlight");
    lsdef!(@req document_symbols, "textDocument/documentSymbol");
    lsdef!(@req code_action, "textDocument/codeAction");
    lsdef!(@req code_lens, "textDocument/codeLens");
    lsdef!(@req code_lens_resolve, "codeLens/resolve");
    lsdef!(@req document_link, "textDocument/documentLink");
    lsdef!(@req document_link_resolve, "documentLink/resolve");
    // lsdef!(@req textDocument/applyEdit, "textDocument/applyEdit");
    lsdef!(@req range_formatting, "textDocument/rangeFormatting");
    lsdef!(@req on_type_formatting, "textDocument/onTypeFormatting");
    lsdef!(@req formatting, "textDocument/formatting");
    lsdef!(@req rename, "textDocument/rename");
}
