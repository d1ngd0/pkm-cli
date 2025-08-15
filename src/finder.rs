use crate::{Editor, Result};
use std::{
    borrow::Cow,
    fs::read_to_string,
    path::{Path, PathBuf},
    process::ExitStatus,
    sync::Arc,
};

use crossbeam_channel::{Receiver, Sender, unbounded};
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

    pub fn add<F: Into<FinderItem>>(&mut self, item: F) -> Result<()> {
        Ok(self.sender.send(Arc::new(item.into()))?)
    }

    pub fn run(self) -> Result<ExitStatus> {
        let Self {
            repo,
            options,
            receiver,
            sender: _,
        } = self;

        let selections = Skim::run_with(&options, Some(receiver));
        let selections = match selections {
            Some(m) => m,
            None => return Ok(ExitStatus::default()),
        };

        if selections.is_abort {
            return Ok(ExitStatus::default());
        }

        if selections.selected_items.len() == 0 {
            return Ok(ExitStatus::default());
        }

        let mut editor = Editor::new_from_env("EDITOR", repo);

        for f in selections.selected_items {
            editor = editor.file(PathBuf::from(f.text().as_ref()))
        }

        editor.exec()
    }
}

pub struct FinderItem {
    root: PathBuf,
    path: PathBuf,
}

impl FinderItem {
    pub fn new<A: Into<PathBuf>, B: Into<PathBuf>>(root: A, path: B) -> Self {
        Self {
            root: root.into(),
            path: path.into(),
        }
    }
}

impl SkimItem for FinderItem {
    fn text(&self) -> Cow<'_, str> {
        self.path.as_path().to_string_lossy()
    }

    fn preview(&self, _context: skim::PreviewContext) -> skim::ItemPreview {
        let mut full_path = PathBuf::new();
        full_path.push(&self.root);
        full_path.push(&self.path);

        let s = read_to_string(full_path)
            .unwrap_or_else(|err| format!("error getting display: {}", err));
        // change this to item with position
        ItemPreview::Text(s)
    }
}
