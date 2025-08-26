use std::ops::Deref;
use std::path::StripPrefixError;
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};

use chrono::{DateTime, Datelike, TimeZone};
use clap::ArgMatches;
use convert_case::{Case, Casing};
use sha1::{Digest, Sha1};
use tera::Tera;

use crate::{Error, Result};

// ZettelBuilder is used to set the attributes of a zettel and make
// it into an actual file
pub struct ZettelBuilder {
    path: PathBuf,
    tmpl_name: String,
}

struct ZettelReference {
    zettel_path: PathBuf,
    prefix: String,
}

impl ZettelBuilder {
    pub fn new<P>(repo: P) -> Self
    where
        P: AsRef<Path>,
    {
        Self {
            path: PathBuf::from(repo.as_ref()),
            tmpl_name: "default".into(),
        }
    }

    pub fn with_year_month_day<Tz: TimeZone>(mut self, current_date: &DateTime<Tz>) -> Self {
        self.path.push(format!("{:02}", current_date.year()));
        self.path.push(format!("{:02}", current_date.month()));
        self.path.push(format!("{:02}", current_date.day()));
        self
    }

    // push_year_month will add a [year]/[month]/[day] directory chain to the
    // path
    pub fn with_year_month<Tz: TimeZone>(mut self, current_date: &DateTime<Tz>) -> Self {
        self.path.push(format!("{:02}", current_date.year()));
        self.path.push(format!("{:02}", current_date.month()));
        self
    }

    // push_id adds the id as the filename to the path
    pub fn id(mut self, id: &ZettelID) -> Self {
        self.path.push(id.filename());
        self
    }

    pub fn template(mut self, template: Option<&str>) -> Self {
        if let Some(template) = template {
            self.tmpl_name = template.into();
        }
        self
    }

    pub fn build(self, tmpls: &Tera, context: &tera::Context) -> Result<Zettel> {
        let Self {
            path,
            mut tmpl_name,
        } = self;

        // create the directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?; // only creates the directories, not the file
        }

        tmpl_name.push_str(".md");

        let f = File::create(path.as_path())?;
        tmpls.render_to(&tmpl_name, context, &f)?;
        f.sync_all()?;

        Ok(Zettel { path })
    }
}

// ZettelIDBuilder helps build an id
pub struct ZettelIDBuilder<'a> {
    title: Option<String>,
    prefixes: Vec<&'a str>,
    date: Option<String>,
    hash: Option<String>,
}

// ZettelFileNameBuilder helps you build a filename for the zettel that is coherent and sensible
impl<'a> ZettelIDBuilder<'a> {
    pub fn new() -> Self {
        Self {
            title: None,
            prefixes: Vec::new(),
            date: None,
            hash: None,
        }
    }

    pub fn title<S>(mut self, title: Option<S>) -> Self
    where
        S: AsRef<str>,
    {
        self.title = title.map(|v| v.as_ref().to_case(Case::Snake));
        self
    }

    // meeting will put "meeting" at the beginning of the id
    pub fn prefix(mut self, prefix: &'a str) -> Self {
        self.prefixes.push(prefix);
        self
    }

    // filename_with_hash will create a filename with the following nomenclature
    // [title_as_snakecase]-[8_char_hash].md
    pub fn with_hash(mut self) -> Self {
        let current_date = chrono::Utc::now();
        let mut hash = Sha1::new();
        hash.update(current_date.to_rfc3339().as_bytes());
        let hash = hex::encode(hash.finalize()).to_string();
        self.hash = Some(hash);
        self
    }

    // prefix_date will place the date at the beginning of the id in the
    // format YYYY-MM-DD
    pub fn date<Tz: TimeZone>(mut self, date: &DateTime<Tz>) -> Self {
        self.date = Some(format!(
            "{:04}-{:02}-{:02}",
            date.year(),
            date.month(),
            date.day()
        ));
        self
    }

    pub fn parse_args<M, Tz>(self, args: M, date: &DateTime<Tz>) -> Self
    where
        M: AsRef<ArgMatches>,
        Tz: TimeZone,
    {
        let mut this = self.title(args.as_ref().get_one::<String>("TITLE"));

        if let Some(true) = args.as_ref().get_one::<bool>("DATE") {
            this = this.date(&date)
        }

        if let Some(true) = args.as_ref().get_one::<bool>("MEETING") {
            this = this.prefix("meeting");
            this = this.date(&date)
        }

        if let Some(true) = args.as_ref().get_one::<bool>("FLEETING") {
            this = this.prefix("fleeting")
        }

        this
    }

    // to_string builds the id as a string in the following order
    // [fleeting]-[meeting]-[YYYY-MM-DD]-[title snake case]-[hash]
    pub fn build(self) -> Result<ZettelID> {
        let mut parts = Vec::new();

        let Self {
            title,
            prefixes,
            date: prefix_date,
            hash,
        } = self;

        for prefix in prefixes {
            parts.push(prefix)
        }

        if let Some(date) = prefix_date.as_ref() {
            parts.push(&date)
        }

        if let Some(title) = title.as_ref() {
            parts.push(title)
        }

        if let Some(hash) = hash.as_ref() {
            parts.push(&hash[0..8])
        }

        let id = parts.join("-");

        if id.len() == 0 {
            Err(Error::InvalidZettelID(String::from(
                "zettel id empty, must have title, date or hash",
            )))
        } else {
            Ok(ZettelID(parts.join("-")))
        }
    }
}

pub struct ZettelID(String);

impl From<ZettelID> for String {
    fn from(value: ZettelID) -> Self {
        value.0
    }
}

impl Deref for ZettelID {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ZettelID {
    pub fn filename(&self) -> String {
        format!("{}.md", **self)
    }
}

pub struct Zettel {
    path: PathBuf,
}

impl Zettel {
    pub fn rel_path<P: AsRef<Path>>(
        &self,
        parent: P,
    ) -> std::result::Result<&Path, StripPrefixError> {
        self.path.strip_prefix(parent)
    }
}
