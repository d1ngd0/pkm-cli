use std::path::{Path, PathBuf, absolute};
use std::time::Duration;

use crate::lsp::{AsLocalPath, LSP, Runner, StandardRunner, StandardRunnerBuilder};
use crate::{ImageBuilder, Result, Zettel, ZettelBuilder, ZettelIDBuilder};
use chrono::{DateTime, Local};
use clap::ArgMatches;
use lsp_types::GotoDefinitionResponse;
use tera::{Context, Tera};

pub const DEFAULT_IMAGE_DIR: &str = "imgs";
pub const DEFAULT_TEMPLATE_DIR: &str = "tmpl";
pub const DEFAULT_ZETTEL_DIR: &str = "zettels";
pub const DEFAULT_DAILY_DIR: &str = "daily";

pub struct PKMBuilder {
    root: PathBuf,
    tmpl_dir: Option<PathBuf>,
    daily_dir: Option<PathBuf>,
    image_dir: Option<PathBuf>,
    zettel_dir: Option<PathBuf>,
}

impl PKMBuilder {
    // new creates a new PKMBuilder and defines the root directory
    pub fn new<P>(root: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        Ok(Self {
            root: absolute(root.as_ref())?,
            tmpl_dir: None,
            daily_dir: None,
            image_dir: None,
            zettel_dir: None,
        })
    }

    // with_tmpl_dir sets the template directory relative to the root directory
    pub fn with_tmpl_dir<P>(mut self, tmpl_dir: Option<P>) -> Self
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

    pub fn with_daily_dir<P>(mut self, daily_dir: Option<P>) -> Self
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

    pub fn with_image_dir<P>(mut self, image_dir: Option<P>) -> Self
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

    pub fn with_zettel_dir<P>(mut self, zettel_dir: Option<P>) -> Self
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

    pub fn parse_args(self, args: &ArgMatches) -> Self {
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

        let tmpl_dir = tmpl_dir.unwrap_or_else(|| {
            let mut tmpl_dir = PathBuf::from(&root);
            tmpl_dir.push(DEFAULT_TEMPLATE_DIR);
            tmpl_dir
        });

        let mut tmpl = if tmpl_dir.is_dir() {
            let mut glob = PathBuf::from(tmpl_dir);
            glob.push("**/*.md");
            Tera::new(glob.to_string_lossy().as_ref())?
        } else {
            Tera::default()
        };

        if tmpl
            .get_template("daily.md")
            .is_err_and(|v| matches!(v.kind, tera::ErrorKind::TemplateNotFound(_)))
        {
            tmpl.add_raw_template("daily.md", "# {{ date }}")?;
        }

        if tmpl
            .get_template("default.md")
            .is_err_and(|v| matches!(v.kind, tera::ErrorKind::TemplateNotFound(_)))
        {
            tmpl.add_raw_template("default.md", "# {{ title }}")?;
        }
        log::debug!("{:?}", tmpl);

        Ok(PKM {
            root: root.clone(),
            tmpl,
            daily_dir: daily_dir
                .unwrap_or_else(|| {
                    let mut daily = PathBuf::from(&root);
                    daily.push(DEFAULT_DAILY_DIR);
                    daily
                })
                .into(),
            image_dir: image_dir
                .unwrap_or_else(|| {
                    let mut daily = PathBuf::from(&root);
                    daily.push(DEFAULT_IMAGE_DIR);
                    daily
                })
                .into(),
            zettel_dir: zettel_dir
                .unwrap_or_else(|| {
                    let mut daily = PathBuf::from(&root);
                    daily.push(DEFAULT_ZETTEL_DIR);
                    daily
                })
                .into(),
        })
    }
}

pub struct PKM {
    pub root: PathBuf,
    pub tmpl: Tera,
    pub daily_dir: PathBuf,
    pub image_dir: PathBuf,
    pub zettel_dir: PathBuf,
}

impl PKM {
    pub fn image(&self) -> ImageBuilder {
        ImageBuilder::new(&self.image_dir)
    }

    pub fn zettel(&self) -> ZettelBuilder {
        ZettelBuilder::new(&self.zettel_dir)
    }

    pub fn daily(&self, date: &DateTime<Local>) -> Result<Zettel> {
        let mut context = Context::new();
        context.insert("date", &format!("{}", date.format("%A, %B %d, %Y")));
        let id = ZettelIDBuilder::new().date(&date).build()?;
        ZettelBuilder::new(&self.daily_dir)
            .with_year_month(&date)
            .id(id)
            .template(Some("daily"))
            .aquire(&self.tmpl, &context)
    }

    pub async fn lsp(&self) -> Result<LSP<StandardRunner>> {
        let runner = StandardRunnerBuilder::new("markdown-oxide")
            .working_dir(self.root.as_path())
            .spawn()?;
        Ok(LSP::new(runner, self.root.as_path()).await?)
    }

    // resolve_path will resolve the specified zettel id. It is possible there
    // is more than one zettel with the same name, so it returns a vector of them
    pub async fn resolve_path<R: Runner>(
        &self,
        id: &str,
        lsp: &mut LSP<R>,
    ) -> Result<Vec<PathBuf>> {
        let path = PathBuf::from("/__resolve_path.md");
        lsp.did_open(&path, format!("[[{}]]", id), "markdown")
            .await?;
        tokio::time::sleep(Duration::from_millis(100)).await;

        let locations = match lsp.goto_defintion(&path, 0, 2).await? {
            GotoDefinitionResponse::Scalar(location) => vec![location.uri.as_local_path()],
            GotoDefinitionResponse::Array(locations) => locations
                .into_iter()
                .map(|l| l.uri.as_local_path())
                .collect(),
            GotoDefinitionResponse::Link(location_links) => location_links
                .into_iter()
                .map(|ll| ll.target_uri.as_local_path())
                .collect(),
        };

        Ok(locations
            .into_iter()
            .filter(|buf| {
                buf.starts_with(absolute(&self.zettel_dir).unwrap_or(self.zettel_dir.clone()))
            })
            .collect())
    }
}
