mod error;
mod notification;
mod request;
mod response;
mod runner_standard;

use std::{
    path::{Path, PathBuf, absolute},
    str::FromStr,
};

pub use error::*;
use lsp_types::{
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, GotoDefinitionParams,
    GotoDefinitionResponse, InitializeParams, PartialResultParams, Position,
    TextDocumentIdentifier, TextDocumentPositionParams, Uri, WorkDoneProgressParams,
    WorkspaceFolder,
    notification::{DidCloseTextDocument, DidOpenTextDocument, Notification as _},
    request::{GotoDefinition, Initialize, Request as rt},
};
pub use notification::*;
pub use request::*;
pub use response::*;
pub use runner_standard::*;
use serde::Serialize;

pub trait Requester {
    // send sends the request to the LSP and returns the RequestID for the request
    // This enables the application to continue after the request has been made instead
    // of blocking. If you want to block use the provided `request`
    #[allow(async_fn_in_trait)]
    async fn send<S, R>(&mut self, msg: R) -> Result<RequestID>
    where
        S: Serialize,
        R: Into<Request<S>>;

    #[allow(async_fn_in_trait)]
    async fn notify<S, N>(&mut self, msg: N) -> Result<()>
    where
        S: Serialize,
        N: Into<Notification<S>>;
}
// Runner defines the required functions to interact with an LSP
pub trait Runner {
    type Sender: Requester;

    #[allow(async_fn_in_trait)]
    async fn response(&mut self, req_id: RequestID) -> Result<Response>;

    // create a sender for this implementation of the runner
    fn sender(&mut self) -> Result<Self::Sender>;
}

pub type RequestID = u32;

pub struct LSP<R: Runner> {
    runner: R,
    sender: R::Sender,
}

impl<R: Runner> LSP<R> {
    pub async fn new<P: AsRef<Path>>(mut runner: R, workspace: P) -> Result<LSP<R>> {
        let sender = runner.sender()?;
        let mut lsp = LSP { runner, sender };

        lsp.init(absolute(workspace)?).await?;

        Ok(lsp)
    }

    // request handles the request response loop for the cluster
    async fn request<M, T>(&mut self, method: M, obj: T) -> Result<Response>
    where
        M: Into<String>,
        T: Serialize,
    {
        self.runner
            .response(
                self.sender
                    .send(Request::from_serializable(method.into(), obj)?)
                    .await?,
            )
            .await
    }

    async fn notify<M, T>(&mut self, method: M, obj: T) -> Result<()>
    where
        M: Into<String>,
        T: Serialize,
    {
        self.sender
            .notify(Notification::from_serializable(method, obj)?)
            .await
    }

    // initialize the LSP. Allow deprecated since there are parameters that are
    // deprecated but I have to define them
    #[allow(deprecated)]
    async fn init<P: AsRef<Path>>(&mut self, workspace: P) -> Result<()> {
        let _ = self
            .request(
                Initialize::METHOD,
                InitializeParams {
                    process_id: None,
                    root_path: Some(format!("file://{}", workspace.as_ref().to_string_lossy())),
                    root_uri: Some(uri_from_path(&workspace)?),
                    initialization_options: None,
                    capabilities: lsp_types::ClientCapabilities::default(),
                    trace: None,
                    workspace_folders: Some(vec![WorkspaceFolder {
                        uri: uri_from_path(&workspace)?,
                        name: String::from("root"),
                    }]),
                    client_info: None,
                    locale: None,
                    work_done_progress_params: WorkDoneProgressParams {
                        work_done_token: None,
                    },
                },
            )
            .await?;
        Ok(())
    }

    // goto_definition will return
    pub async fn goto_defintion<P: AsRef<Path>>(
        &mut self,
        uri: P,
        line: u32,
        character: u32,
    ) -> Result<GotoDefinitionResponse> {
        self.request(
            GotoDefinition::METHOD,
            GotoDefinitionParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier {
                        uri: uri_from_path(uri)?,
                    },
                    position: Position { line, character },
                },
                work_done_progress_params: WorkDoneProgressParams {
                    work_done_token: None,
                },
                partial_result_params: PartialResultParams {
                    partial_result_token: None,
                },
            },
        )
        .await?
        .result()
    }

    // open_virtual opens a virtual file, that is a made up file, by notifying the
    // LSP that it has opened a text document
    pub async fn did_open<P, S1, S2>(&mut self, uri: P, content: S1, language: S2) -> Result<()>
    where
        P: AsRef<Path>,
        S1: Into<String>,
        S2: Into<String>,
    {
        self.notify(
            DidOpenTextDocument::METHOD,
            DidOpenTextDocumentParams {
                text_document: lsp_types::TextDocumentItem {
                    uri: uri_from_path(uri)?,
                    language_id: language.into(),
                    version: 0,
                    text: content.into(),
                },
            },
        )
        .await
    }

    // did_close will close the file so the LSP stops looking at it
    pub async fn did_close<P: AsRef<Path>>(&mut self, uri: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        self.notify(
            DidCloseTextDocument::METHOD,
            DidCloseTextDocumentParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: uri_from_path(uri)?,
                },
            },
        )
        .await
    }
}

// uri_from_path turns a path into a uri, which is really stupid this isn't already implemented
// somwehere.
fn uri_from_path<P: AsRef<Path>>(p: P) -> Result<Uri> {
    Uri::from_str(&format!("file://{}", p.as_ref().to_string_lossy()))
        .or_else(|err| Err(Error::LSPError(format!("error: {}", err.to_string()))))
}

pub trait AsLocalPath {
    fn as_local_path(self) -> PathBuf;
}

impl AsLocalPath for Uri {
    fn as_local_path(self) -> PathBuf {
        PathBuf::from(self.path().as_estr().decode().into_string_lossy().as_ref())
    }
}
