mod error;
mod request;
mod response;
mod runner_standard;

use std::{path::Path, str::FromStr};

pub use error::*;
use lsp_types::{
    DidOpenTextDocumentParams, GotoDefinitionParams, GotoDefinitionResponse, InitializeParams,
    PartialResultParams, Position, TextDocumentIdentifier, TextDocumentPositionParams, Uri,
    WorkDoneProgressParams, WorkspaceFolder,
    notification::{DidOpenTextDocument, Notification as _},
    request::{GotoDefinition, Initialize, Request as rt},
};
pub use request::*;
pub use response::*;
pub use runner_standard::*;
use serde::Serialize;

pub trait Requester {
    // send sends the request to the LSP and returns the RequestID for the request
    // This enables the application to continue after the request has been made instead
    // of blocking. If you want to block use the provided `request`
    async fn send<S, R>(&mut self, msg: R) -> Result<RequestID>
    where
        S: Serialize,
        R: Into<Request<S>>;
}
// Runner defines the required functions to interact with an LSP
pub trait Runner {
    type Sender: Requester;

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

        lsp.init(workspace).await?;

        Ok(lsp)
    }

    // initialize the LSP. Allow deprecated since there are parameters that are
    // deprecated but I have to define them
    #[allow(deprecated)]
    async fn init<P: AsRef<Path>>(&mut self, workspace: P) -> Result<()> {
        let workspace = workspace.as_ref().to_string_lossy();
        let init = InitializeParams {
            process_id: None,
            root_path: Some(format!("file://{}", workspace.as_ref())),
            root_uri: Some(
                Uri::from_str(&format!("file://{}", workspace.as_ref()))
                    .or_else(|err| Err(Error::LSPError(format!("error: {}", err.to_string()))))?,
            ),
            initialization_options: None,
            capabilities: lsp_types::ClientCapabilities::default(),
            trace: None,
            workspace_folders: Some(vec![WorkspaceFolder {
                uri: Uri::from_str(&format!("file://{}", workspace.as_ref()))
                    .or_else(|err| Err(Error::LSPError(format!("error: {}", err.to_string()))))?,
                name: String::from("root"),
            }]),
            client_info: None,
            locale: None,
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
        };

        let req = Request::from_serializable(Initialize::METHOD, init)?;
        let id = self.sender.send(req).await?;
        self.runner.response(id).await?;
        Ok(())
    }

    pub async fn goto_defintion<P: AsRef<Path>>(
        &mut self,
        uri: P,
        line: u32,
        character: u32,
    ) -> Result<GotoDefinitionResponse> {
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: Uri::from_str(&format!("file://{}", uri.as_ref().to_string_lossy()))
                        .or_else(|err| {
                            Err(Error::LSPError(format!("error: {}", err.to_string())))
                        })?,
                },
                position: Position { line, character },
            },
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: PartialResultParams {
                partial_result_token: None,
            },
        };
        let req = Request::from_serializable(GotoDefinition::METHOD, params)?;
        let id = self.sender.send(req).await?;

        self.runner.response(id).await?.result()
    }

    pub async fn open_virtual<P: AsRef<Path>>(
        &mut self,
        uri: P,
        content: String,
        language: String,
    ) -> Result<()> {
        let params = DidOpenTextDocumentParams {
            text_document: lsp_types::TextDocumentItem {
                uri: Uri::from_str(&format!("file://{}", uri.as_ref().to_string_lossy()))
                    .or_else(|err| Err(Error::LSPError(format!("error: {}", err.to_string()))))?,
                language_id: language,
                version: 1,
                text: content,
            },
        };
        let req = Request::from_serializable(DidOpenTextDocument::METHOD, params)?;
        let _id = self.sender.send(req).await?;
        Ok(())
    }
}
