use futures::sync::oneshot::{self, Receiver, Sender};
use jsonrpc::{message, ServerCtl};
use jsonrpc::server::{AbstractServer, ServerChain};
use ls_types::*;

use std::cell::RefCell;
use std::ops::Deref;

use super::*;


/// A wrapper for a RLS client.
///
/// Behaves mostly like a generic `Client`, but has some special
/// behaviour to account for additional functionality.
///
/// For example, waits on the initializing to finish (building/indexing)
/// before sending messages. 
pub struct RlsClient {
    inner: Client,
    pub(crate) init_done: Option<Receiver<()>>,
}

impl RlsClient {
    /// Perform the initialize notification, and provide a future to block 
    /// the client for making more calls until the building/indexing has finished.
    pub fn initialize_and_wait(mut self, params: InitializeParams) -> Box<Future<Item=(Self, Result<InitializeResult, InitializeError>), Error=IoError>> {
        Box::new(self.initialize(params)
        .join(self.init_done.take().expect("attempted to initialize multiple times").map_err(|_e| custom_err("notification handlers cancelled")))
        .and_then(|(resp, _)| {
            Ok((self, resp))
        }))
    }
}


impl RlsClient {
    /// Create a new `Client` with the given connection and run futures on the
    /// provided handle.
    pub fn new<C>(connection: C, handle: &Handle) -> Self
        where
            C: Stream<Item = Parsed, Error = IoError>,
            C: Sink<SinkItem = Message, SinkError = IoError>,
            C: Send + 'static,
    {
        let (server, init_done) = WaitForInit::new();
        Self {
            inner: Client::with_notification_handler(connection, server, handle),
            init_done: Some(init_done),
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
        let (server, init_done) = WaitForInit::new();
        let chain = ServerChain::new(
            vec![
                Box::new(AbstractServer::new(server)),
                Box::new(AbstractServer::new(notification_handler)),
            ]);
        Self {
            inner: Client::with_notification_handler(connection, chain, handle),
            init_done: Some(init_done),
        }
    }
}

/// A `Server' implementation which handles the custom RLS notification, and
/// lets a `Receiver` know when the remote server has finished building the code.
pub struct WaitForInit {
    state: RefCell<RemoteState>,
    sender: RefCell<Option<Sender<()>>>,
    ctl: RefCell<Option<ServerCtl>>,
}

pub enum RemoteState {
    Closed,
    Init,
    Building,
    Indexing,
    Done,
    Unknown,
}

impl WaitForInit {
    pub fn new() -> (Self, Receiver<()>) {
        let (sender, receiver) = oneshot::channel();
        (WaitForInit {
            state: RefCell::new(RemoteState::Closed),
            sender: RefCell::new(Some(sender)),
            ctl: RefCell::new(None),
        },
        receiver)
    }

    fn update_state(&self, state: RemoteState) {
        self.state.replace(state);
    }
}

#[derive(Debug, Deserialize, Serialize)]
enum Phase {
    Building,
    Indexing,
}

#[derive(Debug, Deserialize, Serialize)]
struct WindowProgress {
    id: String,
    pub message: Option<String>,
    pub title: Phase,
    pub done: Option<bool>,
}


impl server::Server for WaitForInit {
    type Success = ();
    type RpcCallResult = Result<(), message::RpcError>;
    type NotificationResult = Result<(), ()>;

    fn initialized(&self, ctl: &ServerCtl) {
        self.update_state(RemoteState::Init);
        self.ctl.replace(Some(ctl.clone()));
    }

    fn notification(&self, _ctl: &ServerCtl, method: &str, params: &Option<serde_json::Value>) -> Option<Self::NotificationResult> {
        if let RemoteState::Done = self.state.borrow().deref() {
            return None;
        }
        if method == "window/progress" {
            if let Some(Ok(params)) = params.clone().map(|p| serde_json::from_value::<WindowProgress>(p)) {
                let state = match (&params.done, &params.title) {
                    (Some(true), Phase::Indexing) => {
                        self.sender.borrow_mut().take().expect("Should not be able to finish twice").send(()).unwrap();
                        RemoteState::Done
                    },
                    (Some(true), Phase::Building) => RemoteState::Indexing,
                    (_, Phase::Indexing) => RemoteState::Indexing,
                    (_, Phase::Building) => RemoteState::Building,
                };
                self.update_state(state);
                self.ctl.replace(None);
                Some(Ok(()))
            } else {
                self.update_state(RemoteState::Unknown);
                Some(Err(()))
            }
        } else {
            None
        }
    }
}


macro_rules! lscall {
    (@req $fn_name:ident, $name:tt) => {
        fn $fn_name(&mut self, params: <lsp_request!($name) as Request>::Params) -> Box<Future<Item=<lsp_request!($name) as Request>::Result, Error=IoError>> {
            self.inner.$fn_name(params)
        }
    };
    (@notify $fn_name:ident, $name:tt) => {
        fn $fn_name(&mut self, params: <lsp_notification!($name) as Notification>::Params) -> Result<(), IoError> {
            self.inner.$fn_name(params)
        }
    };
}

impl LspClient for RlsClient {
    fn initialize(&mut self, params: InitializeParams) -> Box<Future<Item=Result<InitializeResult, InitializeError>, Error=IoError>> {
        self.inner.initialize(params)
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
