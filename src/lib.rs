mod editor;
mod error;
mod finder;
mod image;
pub mod lsp;
mod markdown;
mod syntax;
mod zettel;
mod zettel_index;

use std::path::{Path, PathBuf};

use chrono::{DateTime, TimeZone};
use clap::ArgMatches;
pub use editor::*;
pub use error::*;
pub use finder::*;
pub use image::*;
pub use syntax::*;
use tera::{Context, Tera};
pub use zettel::*;
pub use zettel_index::*;

pub struct PKMBuilder<'a> {
    root: &'a Path,
    tmpl_dir: Option<PathBuf>,
    daily_dir: Option<PathBuf>,
    image_dir: Option<PathBuf>,
    zettel_dir: Option<PathBuf>,
}

impl<'a> PKMBuilder<'a> {
    // new creates a new PKMBuilder and defines the root directory
    pub fn new<P>(root: &'a P) -> Self
    where
        P: AsRef<Path>,
    {
        Self {
            root: root.as_ref(),
            tmpl_dir: None,
            daily_dir: None,
            image_dir: None,
            zettel_dir: None,
        }
    }

    // with_tmpl_dir sets the template directory relative to the root directory
    pub fn with_tmpl_dir<P>(mut self, tmpl_dir: Option<&'a P>) -> Self
    where
        P: AsRef<Path>,
    {
        self.tmpl_dir = tmpl_dir.map(|f| {
            let mut path = PathBuf::from(&self.root);
            path.push(f.as_ref());
            path
        });
        self
    }

    pub fn with_daily_dir<P>(mut self, daily_dir: Option<&'a P>) -> Self
    where
        P: AsRef<Path>,
    {
        self.daily_dir = daily_dir.map(|f| {
            let mut path = PathBuf::from(&self.root);
            path.push(f.as_ref());
            path
        });
        self
    }

    pub fn with_image_dir<P>(mut self, image_dir: Option<&'a P>) -> Self
    where
        P: AsRef<Path>,
    {
        self.image_dir = image_dir.map(|f| {
            let mut path = PathBuf::from(&self.root);
            path.push(f.as_ref());
            path
        });
        self
    }

    pub fn with_zettel_dir<P>(mut self, zettel_dir: Option<&'a P>) -> Self
    where
        P: AsRef<Path>,
    {
        self.zettel_dir = zettel_dir.map(|f| {
            let mut path = PathBuf::from(&self.root);
            path.push(f.as_ref());
            path
        });
        self
    }

    pub fn parse_args(self, args: &'a ArgMatches) -> Self {
        self.with_image_dir(args.get_one::<String>("IMG_DIR"))
            .with_tmpl_dir(args.get_one::<String>("TEMPLATE_DIR"))
            .with_daily_dir(args.get_one::<String>("DAILY_DIR"))
            .with_zettel_dir(args.get_one::<String>("ZETTEL_DIR"))
    }

    pub fn build(self) -> Result<PKM> {
        let Self {
            root,
            tmpl_dir,
            daily_dir,
            image_dir,
            zettel_dir,
        } = self;

        let mut tmpl_glob = PathBuf::from(tmpl_dir.ok_or(Error::PKMError(String::from(
            "template directory is a required",
        )))?);
        tmpl_glob.push("**/*.md");
        let tmpl = Tera::new(
            tmpl_glob
                .as_path()
                .to_str()
                .expect("template is not valid unicode"),
        )?;

        Ok(PKM {
            root: root.into(),
            tmpl,
            daily_dir: daily_dir
                .ok_or(Error::PKMError(String::from(
                    "daily directory is a required",
                )))?
                .into(),
            image_dir: image_dir
                .ok_or(Error::PKMError(String::from(
                    "image directory is a required",
                )))?
                .into(),
            zettel_dir: zettel_dir
                .ok_or(Error::PKMError(String::from(
                    "Zettel directory is a required",
                )))?
                .into(),
        })
    }
}

pub struct PKM {
    root: PathBuf,
    pub tmpl: Tera,
    daily_dir: PathBuf,
    image_dir: PathBuf,
    zettel_dir: PathBuf,
}

impl PKM {
    pub fn image(&self) -> ImageBuilder {
        ImageBuilder::new(&self.image_dir)
    }

    pub fn zettel(&self) -> ZettelBuilder {
        ZettelBuilder::new(&self.zettel_dir)
    }

    pub fn daily<Tz: TimeZone>(&self, date: &DateTime<Tz>) -> Result<Zettel> {
        let context = Context::new();
        let id = ZettelIDBuilder::new().date(&date).build()?;
        ZettelBuilder::new(&self.daily_dir)
            .with_year_month(&date)
            .id(id)
            .aquire(&self.tmpl, &context)
    }
}
