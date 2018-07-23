/// Example for using the `RlsClient` to get a list of symbols
///
/// Try running with `nc -l -p 50505 -e "rls +nightly" & cargo run --example symbols`

extern crate futures;
extern crate languageserver_types as ls_types;
extern crate serde_json;
extern crate tokio_lsp;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_jsonrpc;
extern crate url;

use futures::Future;
use tokio_core::reactor::Core;
use tokio_core::net::TcpStream;
use tokio_io::AsyncRead;
use tokio_lsp::*;
use tokio_lsp::client::rust::RlsClient;
use url::Url;

use std::env;

fn main() {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let path = env::current_dir().unwrap();
    println!("Path: {:?}", path);

    let request = TcpStream::connect(&"127.0.0.1:50505".parse().expect("Needs an instance of RLS running on port 50505"), &handle)
        .and_then(move |stream| {
            println!("Got stream: {:?}", stream);
            // Create a client on top of the connection
            let client = RlsClient::new(stream.framed(LspCodec), &handle);

            // `RlsClient` has a special method to call the initialize function, and
            // then wait until the building/indexing has finished before sending further messages.
            client.initialize_and_wait(init_params(&format!("file://{}", &path.display())))
            .and_then(move |(mut client, resp)| {
                println!("Received init response: {:#?}", resp);
                client.document_symbols(doc_params(&format!("file://{}/{}", &path.display(), "src/codec.rs")))
            })
            .and_then(|resp| {
                println!("Got document symbols: {:#?}", resp);
                Ok(())
            })
        });

    core.run(request).unwrap();
}

fn init_params(root: &str) -> ls_types::InitializeParams {
    ls_types::InitializeParams { 
        process_id: None, 
        root_uri: Url::parse(root).ok(),
        root_path: None,
        initialization_options: None,
        capabilities: ls_types::ClientCapabilities {
            workspace: None,
            text_document: None,
            experimental: None,
        },
        trace: Some(ls_types::TraceOption::Verbose),
    }
}

fn doc_params(file: &str) -> ls_types::DocumentSymbolParams {
    ls_types::DocumentSymbolParams {
        text_document: ls_types::TextDocumentIdentifier::new(
            Url::parse(file).unwrap(),
        ),
    }
}
