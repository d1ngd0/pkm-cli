use crate::{Editor, Error, Highlighting, Result, first_node};
use std::{
    borrow::Cow,
    fs::read_to_string,
    path::{Path, PathBuf},
    sync::Arc,
};

use crossbeam_channel::{Receiver, Sender, unbounded};
use lsp_types::Uri;
use markdown::{ParseOptions, mdast::Node};
use skim::{ItemPreview, Skim, SkimItem, SkimOptions, prelude::SkimOptionsBuilder};

pub struct Finder<P: AsRef<Path>> {
    repo: P,
    options: SkimOptions,
    sender: Sender<Arc<dyn SkimItem>>,
    receiver: Receiver<Arc<dyn SkimItem>>,
}

impl<P: AsRef<Path>> Finder<P> {
    pub fn new(repo: P) -> Finder<P> {
        let options = SkimOptionsBuilder::default()
            .multi(true)
            .preview(Some(String::from("right")))
            .build()
            .expect("you should work");

        let (sender, receiver) = unbounded();
        Finder {
            repo,
            options,
            sender,
            receiver,
        }
    }

    pub fn add_fq_doc(&mut self, path: Uri) -> Result<()> {
        let path_string = path.to_string();
        let path = path_string.strip_prefix("file://").unwrap_or(&path_string);
        let path = path
            .strip_prefix(
                self.repo
                    .as_ref()
                    .to_str()
                    .ok_or_else(|| Error::NotFound(String::from("fuck you fluent_uri")))?,
            )
            .unwrap_or(path);
        let path = path.strip_prefix("/").unwrap_or(path);

        self.add_doc(Path::new(path))
    }

    pub fn add_doc<Q: AsRef<Path>>(&mut self, path: Q) -> Result<()> {
        let mut full_doc_path = PathBuf::new();
        full_doc_path.push(self.repo.as_ref());
        full_doc_path.push(path.as_ref());
        let content = read_to_string(full_doc_path.as_path())?;

        let opts = ParseOptions::gfm();
        let ast = markdown::to_mdast(&content, &opts)?;

        let mut title = None;
        if let Some(header) = first_node!(&ast, Node::Heading) {
            if let Some(Node::Text(header_content)) = header.children.get(0) {
                title = Some(header_content.value.as_str());
            }
        }

        self.add(
            FinderItem::new(path.as_ref())
                .with_display(title)
                .with_preview(Some(content)),
        )
    }

    pub fn add<F: Into<FinderItem>>(&mut self, item: F) -> Result<()> {
        Ok(self.sender.send(Arc::new(item.into()))?)
    }

    // run runs the finder and returns if we ran the editor
    pub fn run(self) -> Result<bool> {
        let Self {
            repo,
            options,
            receiver,
            sender: _,
        } = self;

        let selections = Skim::run_with(&options, Some(receiver));
        let selections = match selections {
            Some(m) => m,
            None => return Ok(false),
        };

        if selections.is_abort {
            return Ok(false);
        }

        if selections.selected_items.len() == 0 {
            return Ok(false);
        }

        let mut editor = Editor::new_from_env("EDITOR", repo);

        for f in selections.selected_items {
            editor = editor.file(PathBuf::from(f.text().as_ref()))
        }

        editor.exec()?;
        Ok(true)
    }
}

pub struct FinderItem {
    path: PathBuf,
    preview: Option<ItemPreview>,
    display: Option<String>,
}

impl FinderItem {
    pub fn new<B: Into<PathBuf>>(path: B) -> Self {
        Self {
            path: path.into(),
            preview: None,
            display: None,
        }
    }

    pub fn with_display<S: Into<String>>(mut self, display: Option<S>) -> Self {
        self.display = display.map(|v| v.into());
        self
    }

    pub fn with_preview<S: Into<String>>(mut self, preview: Option<S>) -> Self {
        self.preview = preview.map(|v| ItemPreview::Text(v.into()));
        self
    }

    pub fn with_syntax_preview(
        mut self,
        content: &str,
        ext: Option<&str>,
        theme: Option<&str>,
    ) -> Result<Self> {
        self.preview = Some(ItemPreview::AnsiText(
            Highlighting::new()
                .syntax(ext)
                .theme(theme)
                .highlight(content)?,
        ));
        Ok(self)
    }
}

impl SkimItem for FinderItem {
    fn text(&self) -> Cow<'_, str> {
        self.path.as_path().to_string_lossy()
    }

    fn display<'a>(&'a self, _context: skim::DisplayContext<'a>) -> skim::AnsiString<'a> {
        let display = self.display.clone();
        display.unwrap_or_else(|| self.text().into()).into()
    }

    fn preview(&self, _context: skim::PreviewContext) -> skim::ItemPreview {
        match self.preview.as_ref() {
            Some(ip) => match ip {
                // wish they would implement clone on ItemPreview
                ItemPreview::Command(s) => ItemPreview::Command(s.clone()),
                ItemPreview::CommandWithPos(s, p) => {
                    ItemPreview::CommandWithPos(s.clone(), p.clone())
                }
                ItemPreview::Text(s) => ItemPreview::Text(s.clone()),
                ItemPreview::TextWithPos(s, p) => ItemPreview::TextWithPos(s.clone(), p.clone()),
                ItemPreview::AnsiText(s) => ItemPreview::AnsiText(s.clone()),
                ItemPreview::AnsiWithPos(s, p) => ItemPreview::AnsiWithPos(s.clone(), p.clone()),
                ItemPreview::Global => ItemPreview::Global,
            },
            _ => ItemPreview::Text(String::from("no display provided")),
        }
    }
}
