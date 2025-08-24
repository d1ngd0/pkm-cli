use std::io::Write;
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};

use tera::Tera;

use crate::{Error, Result};

// ZettelBuilder is used to set the attributes of a zettel and make
// it into an actual file
pub struct ZettelBuilder {
    template: String,
    destination: PathBuf,
    template_dir: PathBuf,
    references: Vec<ZettelReference>,
}

struct ZettelReference {
    zettel_path: PathBuf,
    prefix: String,
}

impl ZettelBuilder {
    pub fn new<P>(destination: P, template_dir: P) -> Self
    where
        P: AsRef<Path>,
    {
        Self {
            destination: destination.as_ref().to_path_buf(),
            template: "default".into(),
            template_dir: template_dir.as_ref().to_path_buf(),
            references: vec![],
        }
    }

    pub fn template(mut self, template: Option<&str>) -> Self {
        if let Some(template) = template {
            self.template = template.into();
        }
        self
    }

    pub fn template_dir(mut self, template_dir: Option<&String>) -> Self {
        if let Some(template_dir) = template_dir {
            self.template_dir = template_dir.into();
        }
        self
    }

    // with_reference will make a reference to the zettel somewhere else
    pub fn with_reference<P: AsRef<Path>>(mut self, at: P, prefix: &str) -> Self {
        self.references.push(ZettelReference {
            zettel_path: PathBuf::from(at.as_ref()),
            prefix: prefix.to_string(),
        });
        self
    }

    pub fn build(self, context: &tera::Context) -> Result<()> {
        let Self {
            references,
            destination,
            mut template,
            mut template_dir,
        } = self;

        template_dir.push("**/*.md");
        let tera = Tera::new(
            template_dir
                .as_path()
                .to_str()
                .expect("template is not valid unicode"),
        )?;

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?; // only creates the directories, not the file
        }

        template.push_str(".md");

        let f = File::create(destination.as_path())?;
        tera.render_to(&template, context, &f)?;
        f.sync_all()?;

        for r in references {
            let file = OpenOptions::new()
                .write(true)
                .append(true)
                .open(r.zettel_path.as_path())?;
            let id = destination
                .as_path()
                .file_stem()
                .ok_or_else(|| Error::InvalidZettelID(String::from("Missing zettel id")))?
                .to_string_lossy();

            write!(&file, "\n{} [[{}]]", r.prefix, id)?;
        }
        Ok(())
    }
}
