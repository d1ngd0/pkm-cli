use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};

use tera::Tera;

use crate::Result;

// ZettelBuilder is used to set the attributes of a zettel and make
// it into an actual file
pub struct ZettelBuilder {
    template: String,
    destination: PathBuf,
    template_dir: PathBuf,
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
        }
    }

    pub fn template(mut self, template: Option<&String>) -> Self {
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

    pub fn build(self, context: &tera::Context) -> Result<()> {
        let Self {
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

        let f = File::create(destination)?;
        tera.render_to(&template, context, &f)?;
        f.sync_all()?;
        Ok(())
    }
}
