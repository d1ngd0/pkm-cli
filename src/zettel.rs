use std::borrow::Borrow;
use std::fmt::Display;
use std::io::Write;
use std::ops::Deref;
use std::path::StripPrefixError;
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};

use chrono::{DateTime, Datelike, TimeZone};
use clap::ArgMatches;
use convert_case::{Case, Casing};
use markdown::ParseOptions;
use markdown::mdast::Node;
use regex::Regex;
use sha1::{Digest, Sha1};
use tera::{Context, Tera};

use crate::{Error, Result};

// ZettelBuilder is used to set the attributes of a zettel and make
// it into an actual file
pub struct ZettelBuilder {
    path: PathBuf,
    tmpl_name: String,
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
    pub fn id<Z: AsRef<ZettelID>>(mut self, id: Z) -> Self {
        self.path.push(id.as_ref().filename());
        self
    }

    // parse_args takes arg matches and grabs the following from it
    // TEMPLATE: String
    pub fn parse_args(self, args: &ArgMatches) -> Self {
        self.template(args.get_one::<String>("TEMPLATE"))
    }

    pub fn template<S: AsRef<str>>(mut self, template: Option<S>) -> Self {
        if let Some(template) = template {
            self.tmpl_name = template.as_ref().into();
        }
        self
    }

    // aquire will create the date zettel or return the existing one if it
    // doesn't already exist.
    pub fn aquire<T, C>(self, tmpls: T, context: C) -> Result<Zettel>
    where
        T: Borrow<Tera>,
        C: Borrow<Context>,
    {
        if self.path.exists() {
            Ok(Zettel::new(self.path))
        } else {
            self.build(tmpls, context)
        }
    }

    pub fn build<T, C>(self, tmpls: T, context: C) -> Result<Zettel>
    where
        T: Borrow<Tera>,
        C: Borrow<Context>,
    {
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
        tmpls.borrow().render_to(&tmpl_name, context.borrow(), &f)?;
        f.sync_all()?;

        Ok(Zettel::new(path))
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
                .to_lowercase()
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
            "{:04}-{:02}-{:02}",
            date.year(),
            date.month(),
            date.day()
        ));
        self
    }

    // parse_args takes arg matches and grabs the following from it
    // TITLE: String The title
    // DATE: bool Sets a date tag
    // MEETING: bool Sets the date and `meeting` tag
    // FLEETING: bool Sets the `fleeting` tag
    pub fn parse_args<Tz>(self, args: &ArgMatches, date: &DateTime<Tz>) -> Self
    where
        Tz: TimeZone,
    {
        let mut this = self.title(args.get_one::<String>("TITLE"));

        if let Some(true) = args.get_one::<bool>("DATE") {
            this = this.date(&date);
        }

        if let Some(true) = args.get_one::<bool>("MEETING") {
            this = this.tag("meeting");
            this = this.date(&date)
        }

        if let Some(true) = args.get_one::<bool>("FLEETING") {
            this = this.tag("fleeting");
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

#[derive(Debug)]
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

impl AsRef<ZettelID> for ZettelID {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl Display for ZettelID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
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
    content: Option<String>,
}

impl Zettel {
    pub fn new<P: Into<PathBuf>>(path: P) -> Zettel {
        Zettel {
            path: path.into(),
            content: None,
        }
    }

    pub fn rel_path<P: AsRef<Path>>(
        &self,
        parent: P,
    ) -> std::result::Result<&Path, StripPrefixError> {
        self.path.strip_prefix(parent)
    }

    pub fn content<'a>(&'a mut self) -> Result<ZettelContent<'a>> {
        let content = fs::read_to_string(&self.path)?;
        let child = self.content.insert(content);

        Ok(ZettelContent { child })
    }

    pub fn sync(&mut self) -> Result<()> {
        let content = match self.content.take() {
            Some(v) => v,
            None => return Ok(()),
        };

        let mut file = File::options()
            .truncate(true)
            .create(true)
            .write(true)
            .open(self.path.as_path())?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;

        Ok(())
    }
}

impl AsRef<Zettel> for Zettel {
    fn as_ref(&self) -> &Self {
        self
    }
}

pub struct ZettelContent<'a> {
    child: &'a mut String,
}

impl<'a> ZettelContent<'a> {
    pub fn append(&mut self, child: &str) -> Result<()> {
        self.child.push_str("\n");
        self.child.push_str(child);
        Ok(())
    }
}

impl<'a> ToString for ZettelContent<'a> {
    fn to_string(&self) -> String {
        self.child.to_string()
    }
}

pub struct ZettelReference<'a> {
    id: &'a ZettelID,
    prefix: &'a str,
}

impl<'a> ZettelReference<'a> {
    pub fn new(id: &'a ZettelID, prefix: &'a str) -> ZettelReference<'a> {
        ZettelReference { id, prefix }
    }
}

// From a time in the past when this was going to be possible
// we need to align all on the same thing
// impl From<ZettelReference<'_>> for Node {
//     fn from(value: ZettelReference<'_>) -> Self {
//         let opts = ParseOptions::gfm();
//         markdown::to_mdast(&format!("- {} [[{}]]", value.prefix, value.id), &opts).unwrap()
//     }
// }

impl From<ZettelReference<'_>> for String {
    fn from(value: ZettelReference<'_>) -> Self {
        format!("- {} [[{}]]", value.prefix, value.id)
    }
}
