use std::{
    fs::{self, File},
    path::Path,
};

use tera::Tera;

use crate::Result;

// ZettelBuilder is used to set the attributes of a zettel and make
// it into an actual file
pub struct ZettelBuilder<'a> {
    template: String,
    destination: &'a Path,
    template_dir: String,
}

impl<'a> ZettelBuilder<'a> {
    pub fn new(destination: &'a Path) -> Self {
        Self {
            destination,
            template: "default".into(),
            template_dir: ".".into(),
        }
    }

    pub fn template(mut self, template: Option<&String>) -> Self {
        if let Some(template) = template {
            self.template = template.into();
        }
        self
    }

    pub fn build(self, context: &tera::Context) -> Result<()> {
        let Self {
            destination,
            mut template,
            mut template_dir,
        } = self;

        template_dir.push_str("templates/**/*.md");
        template.push_str("_zettel.md");
        let tera = Tera::new(&template_dir)?;

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?; // only creates the directories, not the file
        }

        let f = File::create(destination)?;
        tera.render_to(&template, context, &f)?;
        Ok(())
    }
}
