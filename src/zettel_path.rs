use chrono::{DateTime, Datelike, TimeZone};
use convert_case::{Case, Casing};
use sha1::{Digest, Sha1};
use std::path::PathBuf;

// ZettelPathBuf is a trait that adds additional functionality to
// the pathbuf to make it easy to create paths for zettels
pub trait ZettelPathBuf {
    // push_year_month_day will add a [year]/[month]/[day] directory chain to the
    // path
    fn push_year_month_day<Tz: TimeZone>(&mut self, current_date: DateTime<Tz>);

    // push_year_month will add a [year]/[month] directory chain to the
    // path
    fn push_year_month<Tz: TimeZone>(&mut self, current_date: DateTime<Tz>);

    // filename_with_hash will create a filename with the following nomenclature
    // [title_as_snakecase]-[8_char_hash].md
    fn filename_with_hash(&mut self, title: &str);

    // will create a filename utilizing the date format ending in `.md`
    fn filename_with_date<Tz: TimeZone>(&mut self, current_date: DateTime<Tz>);
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

    // filename_with_hash will create a filename with the following nomenclature
    // [title_as_snakecase]-[8_char_hash].md
    fn filename_with_hash(&mut self, title: &str) {
        let mut id = title.to_case(Case::Snake);
        id.push_str("-");

        let current_date = chrono::Utc::now();
        let mut hash = Sha1::new();
        hash.update(current_date.to_rfc3339().as_bytes());
        let hash = hex::encode(hash.finalize()).to_string();
        id.push_str(&hash[0..8]);
        id.push_str(".md");
        self.push(&id);
    }

    // will create a filename utilizing the date format ending in `.md`
    fn filename_with_date<Tz: TimeZone>(&mut self, current_date: DateTime<Tz>) {
        let file_path = format!(
            "{:04}-{:02}-{:02}.md",
            current_date.year(),
            current_date.month(),
            current_date.day()
        );
        self.push(file_path);
    }
}
