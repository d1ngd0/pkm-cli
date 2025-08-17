mod error;
mod request;
mod response;
mod runner_standard;

use std::{path::Path, str::FromStr, thread, time::Duration};

pub use error::*;
use lsp_types::{
    GotoDefinitionParams, GotoDefinitionResponse, InitializeParams, PartialResultParams, Position,
    TextDocumentIdentifier, TextDocumentPositionParams, Uri, WorkDoneProgressParams,
    WorkspaceFolder,
    request::{GotoDefinition, Initialize, Request as rt},
};
pub use request::*;
pub use response::*;
pub use runner_standard::*;
use serde::Serialize;

pub trait Sender {
    // send sends the request to the LSP and returns the RequestID for the request
    // This enables the application to continue after the request has been made instead
    // of blocking. If you want to block use the provided `request`
    fn send<S, R>(&mut self, msg: R) -> Result<RequestID>
    where
        S: Serialize,
        R: Into<Request<S>>;
}
// Runner defines the required functions to interact with an LSP
pub trait Runner {
    type Sender: Sender;
    // try_response will try to get the response from the endpoint, if it can't
    // it must return a NotReady error to let the caller know we aren't ready yet
    fn try_response(&mut self, req_id: RequestID) -> Result<Response>;

    fn response(&mut self, req_id: RequestID) -> Result<Response> {
        let mut resp = self.try_response(req_id);
        loop {
            match resp {
                Ok(r) => return Ok(r),
                Err(Error::NotReady) => {
                    thread::sleep(Duration::from_millis(10));
                    resp = self.try_response(req_id.into());
                }
                Err(err) => return Err(err),
            }
        }
    }

    // create a sender for this implementation of the runner
    fn sender(&mut self) -> Result<Self::Sender>;
}

pub type RequestID = u32;

pub struct LSP<R: Runner> {
    runner: R,
    sender: R::Sender,
}

impl<R: Runner> LSP<R> {
    pub fn new<P: AsRef<Path>>(mut runner: R, workspace: P) -> Result<LSP<R>> {
        let sender = runner.sender()?;
        let mut lsp = LSP { runner, sender };

        lsp.init(workspace)?;

        Ok(lsp)
    }

    // initialize the LSP
    fn init<P: AsRef<Path>>(&mut self, workspace: P) -> Result<()> {
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
        let id = self.sender.send(req)?;
        self.runner.response(id)?;
        Ok(())
    }

    pub fn goto_defintion<P: AsRef<Path>>(
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
        let id = self.sender.send(req)?;

        self.runner.response(id)?.result()
    }
}
