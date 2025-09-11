use core::borrow;
use std::{
    borrow::Borrow,
    path::{Path, PathBuf},
    str::FromStr,
};

use lsp_types::{
    DidChangeTextDocumentParams, DidOpenTextDocumentParams, TextDocumentContentChangeEvent, Uri,
    VersionedTextDocumentIdentifier,
    notification::{DidChangeTextDocument, DidOpenTextDocument, Notification},
};
use rand::{Rng, distr::Alphanumeric};

use super::{Error, Request, Requester, Result};

pub struct VirtualFileBuilder<'a, R: Requester> {
    sender: &'a mut R,
    uri: Option<PathBuf>,
    content: Option<String>,
    language: Option<String>,
}

impl<'a, R: Requester> VirtualFileBuilder<'a, R> {
    pub fn new(sender: &'a mut R) -> Self {
        VirtualFileBuilder {
            sender,
            uri: None,
            content: None,
            language: None,
        }
    }

    pub fn uri<P: AsRef<Path>>(mut self, uri: Option<P>) -> Self {
        self.uri = uri.map(|uri| PathBuf::from(uri.as_ref()));
        self
    }

    pub fn content<S: Borrow<String>>(mut self, content: Option<S>) -> Self {
        self.content = content.map(|content| String::from(content.borrow()));
        self
    }

    pub fn language<S: AsRef<String>>(mut self, language: Option<S>) -> Self {
        self.language = language.map(|language| String::from(language.as_ref()));
        self
    }

    pub async fn build(self) -> Result<VirtualFile<'a, R>> {
        let Self {
            sender,
            uri,
            content,
            language,
        } = self;

        let uri = uri.unwrap_or_else(|| {
            PathBuf::from(
                rand::rng()
                    .sample_iter(&Alphanumeric)
                    .take(7)
                    .map(char::from)
                    .collect::<String>(),
            )
        });

        let vf = VirtualFile {
            sender,
            version: 1,
            path: uri,
            content: content.unwrap_or(String::from("")),
            language: language.unwrap_or(String::from("md")),
        };

        let params = DidOpenTextDocumentParams {
            text_document: lsp_types::TextDocumentItem {
                uri: Uri::from_str(&format!("file://{}", vf.path.as_path().to_string_lossy()))
                    .or_else(|err| Err(Error::LSPError(format!("error: {}", err.to_string()))))?,
                language_id: vf.language.clone(),
                version: vf.version,
                text: vf.content.clone(),
            },
        };
        let req = Request::from_serializable(DidOpenTextDocument::METHOD, params)?;
        let _id = vf.sender.send(req).await?;
        Ok(vf)
    }
}

pub struct VirtualFile<'a, R: Requester> {
    sender: &'a mut R,
    version: i32,
    pub path: PathBuf,
    content: String,
    language: String,
}

impl<'a, R: Requester> VirtualFile<'a, R> {
    //update_content and update the version
    pub async fn update_content(&mut self, content: &str) -> Result<()> {
        self.content.truncate(0);
        self.content.push_str(content);
        self.sync().await
    }

    async fn sync(&mut self) -> Result<()> {
        self.version += 1;
        let params = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: Uri::from_str(&format!("file://{}", self.path.as_path().to_string_lossy()))
                    .or_else(|err| Err(Error::LSPError(format!("error: {}", err.to_string()))))?,
                version: 2,
            },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: self.content.clone(),
            }],
        };
        let req = Request::from_serializable(DidChangeTextDocument::METHOD, params)?;
        let _id = self.sender.send(req).await?;
        Ok(())
    }
}
