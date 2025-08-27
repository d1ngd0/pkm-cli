use std::ops::Deref;
use std::path::StripPrefixError;
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};

use chrono::{DateTime, Datelike, TimeZone};
use clap::ArgMatches;
use convert_case::{Case, Casing};
use regex::Regex;
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
    tags: Vec<&'a str>,
    date: Option<String>,
    hash: Option<String>,
}

// ZettelFileNameBuilder helps you build a filename for the zettel that is coherent and sensible
impl<'a> ZettelIDBuilder<'a> {
    pub fn new() -> Self {
        Self {
            title: None,
            tags: Vec::new(),
            date: None,
            hash: None,
        }
    }

    pub fn title<S>(mut self, title: Option<S>) -> Self
    where
        S: AsRef<str>,
    {
        self.title = title.map(|v| {
            v.as_ref()
                .replace('\n', "")
                .replace('\r', "")
                .to_case(Case::Train)
        });
        self
    }

    // meeting will put "meeting" at the beginning of the id
    pub fn tag(mut self, prefix: &'a str) -> Self {
        self.tags.push(prefix);
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
            "d{:04}-{:02}-{:02}",
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
            this = this.tag("meeting");
            this = this.date(&date)
        }

        if let Some(true) = args.as_ref().get_one::<bool>("FLEETING") {
            this = this.tag("fleeting")
        }

        this
    }

    // to_string builds the id as a string in the following order
    // [fleeting]-[meeting]-[YYYY-MM-DD]-[title snake case]-[hash]
    pub fn build(self) -> Result<ZettelID> {
        let mut parts = Vec::new();

        let Self {
            title,
            tags,
            date,
            hash,
        } = self;

        if let Some(title) = title.as_ref() {
            parts.push(title.as_str())
        }

        for tag in tags {
            parts.push(tag)
        }

        if let Some(date) = date.as_ref() {
            parts.push(&date)
        }

        if let Some(hash) = hash.as_ref() {
            parts.push(&hash[0..8])
        }

        let id = parts.join("_");

        if id.len() == 0 {
            Err(Error::InvalidZettelID(String::from(
                "zettel id empty, must have title, date or hash",
            )))
        } else {
            Ok(ZettelID(id))
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

    fn parts(&self) -> ZettelIDIter<'_> {
        ZettelIDIter::new(self)
    }

    pub fn title(&self) -> Result<&str> {
        self.parts()
            .filter_map(|f| match f {
                ZettelIDPart::Title(title) => Some(title),
                _ => None,
            })
            .next()
            .ok_or(Error::InvalidZettelID(String::from(
                "No title in zettel ID",
            )))
    }

    pub fn tags(&self) -> impl Iterator<Item = &str> {
        self.parts().filter_map(|f| match f {
            ZettelIDPart::Tag(tag) => Some(tag),
            _ => None,
        })
    }

    pub fn hash(&self) -> Option<&str> {
        self.parts()
            .filter_map(|f| match f {
                ZettelIDPart::Hash(hash) => Some(hash),
                _ => None,
            })
            .next()
    }

    pub fn tag(&self, tag: &str) -> Option<&str> {
        self.tags().filter(|t| *t == tag).next()
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tag(tag).is_some()
    }

    pub fn tag_regex(&self, tag_regex: &Regex) -> Option<&str> {
        self.tags().filter(|t| tag_regex.is_match(t)).next()
    }

    pub fn has_tag_regex(&self, tag_regex: &Regex) -> bool {
        self.tag_regex(tag_regex).is_some()
    }
}

struct ZettelIDIter<'a> {
    title: bool,
    id: &'a ZettelID,
    loc: usize,
}

impl<'a> ZettelIDIter<'a> {
    fn new(id: &'a ZettelID) -> Self {
        Self {
            title: false,
            id,
            loc: 0,
        }
    }
}

impl<'a> Iterator for ZettelIDIter<'a> {
    type Item = ZettelIDPart<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let sub_str = &self.id.0[self.loc..];
        let end = sub_str.find("_");

        if sub_str.len() == 0 {
            None
        } else if end.is_none() {
            self.loc = self.loc + sub_str.len();
            Some(ZettelIDPart::Hash(sub_str))
        } else if !self.title {
            let (left, _) = sub_str.split_at(end.expect("if statement"));
            self.loc = self.loc + left.len() + 1; // +1 skips the _
            Some(ZettelIDPart::Title(left))
        } else {
            let (left, _) = sub_str.split_at(end.expect("if statement"));
            self.loc = self.loc + left.len() + 1; // +1 skips the _
            Some(ZettelIDPart::Tag(left))
        }
    }
}

enum ZettelIDPart<'a> {
    Title(&'a str),
    Tag(&'a str),
    Hash(&'a str),
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
