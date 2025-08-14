use std::{
    ffi::OsStr,
    io::Write,
    path::Path,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

use lsp_types::{
    GotoDefinitionParams, PartialResultParams, Position, TextDocumentIdentifier,
    TextDocumentPositionParams, Uri, WorkDoneProgressParams,
    request::{GotoDefinition, Request},
};
use serde::Serialize;
use serde_json::json;

use crate::{Error, Result};

pub struct LSPRuntime {
    lsp_child: Child,
    request_id: AtomicU64,
}

// LSPRuntime is used to run an LSP and run methods against it. It is single threaded, and can only
// handle a single method at a time. Do not send multiple concurrent requests.
impl LSPRuntime {
    pub fn new<A, S, P>(lsp: &str, args: A, wd: P) -> Result<Self>
    where
        A: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
        P: AsRef<Path>,
    {
        let lsp_child = Command::new(lsp)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .current_dir(wd)
            .spawn()?;

        Ok(Self {
            lsp_child,
            request_id: AtomicU64::new(1),
        })
    }

    // goto_defintion returns a response for the GotoDefinition function
    pub fn goto_defintion<U>(&mut self, uri: U, line: u32, character: u32) -> Result<()>
    where
        U: Into<Uri>,
    {
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.into() },
                position: Position { line, character },
            },
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: PartialResultParams {
                partial_result_token: None,
            },
        };

        self.request(GotoDefinition::METHOD, Box::new(params))?;
        Ok(())
    }

    // next_id returns the next request id
    fn next_id(&mut self) -> u64 {
        self.request_id.fetch_add(1, Ordering::SeqCst)
    }

    // request sends a request to the backend
    fn request(&mut self, method: &'static str, params: Box<impl Serialize>) -> Result<()> {
        let body = serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "id": self.next_id(),
            "method": method,
            "params": params
        }))?;

        let stdin = self
            .lsp_child
            .stdin
            .as_mut()
            .ok_or(Error::LSPError(String::from("stdin went away")))?;
        write!(stdin, "Content-Length: {}\r\n\r\n{}", body.len(), body)?;
        Ok(())
    }
}

struct Response {}
