use crate::Result;
use chrono::{DateTime, Datelike, TimeZone};
use convert_case::{Case, Casing};
use sha1::{Digest, Sha1};
use std::path::PathBuf;

use crate::Error;

// ZettelPathBuf is a trait that adds additional functionality to
// the pathbuf to make it easy to create paths for zettels
pub trait ZettelPathBuf {
    // push_year_month_day will add a [year]/[month]/[day] directory chain to the
    // path
    fn push_year_month_day<Tz: TimeZone>(&mut self, current_date: DateTime<Tz>);

    // push_year_month will add a [year]/[month] directory chain to the
    // path
    fn push_year_month<Tz: TimeZone>(&mut self, current_date: DateTime<Tz>);

    fn push_id(&mut self, id: &str);
}

impl ZettelPathBuf for PathBuf {
    // push_year_month_day will add a [year]/[month]/[day] directory chain to the
    // path
    fn push_year_month_day<Tz: TimeZone>(&mut self, current_date: DateTime<Tz>) {
        self.push(format!("{:02}", current_date.year()));
        self.push(format!("{:02}", current_date.month()));
        self.push(format!("{:02}", current_date.day()));
    }

    // push_year_month will add a [year]/[month]/[day] directory chain to the
    // path
    fn push_year_month<Tz: TimeZone>(&mut self, current_date: DateTime<Tz>) {
        self.push(format!("{:02}", current_date.year()));
        self.push(format!("{:02}", current_date.month()));
    }

    // push_id adds the id as the filename to the path
    fn push_id(&mut self, id: &str) {
        self.push(format!("{}.md", id))
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
    pub fn new(title: Option<&str>) -> Self {
        let mut snake_title = None;

        if let Some(title) = title {
            snake_title = Some(title.to_case(Case::Snake));
        }

        Self {
            title: snake_title,
            prefixes: Vec::new(),
            date: None,
            hash: None,
        }
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
    pub fn date<Tz: TimeZone>(mut self, date: DateTime<Tz>) -> Self {
        self.date = Some(format!(
            "{:04}-{:02}-{:02}",
            date.year(),
            date.month(),
            date.day()
        ));
        self
    }

    // get_date returns the underlying date optional
    pub fn get_date(&self) -> Option<&String> {
        self.date.as_ref()
    }

    // to_string builds the id as a string in the following order
    // [fleeting]-[meeting]-[YYYY-MM-DD]-[title snake case]-[hash]
    pub fn to_string(self) -> Result<String> {
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
            Ok(parts.join("-"))
        }
    }
}
