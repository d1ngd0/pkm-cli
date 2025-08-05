use chrono::Datelike;
use convert_case::{Case, Casing};
use sha1::{Digest, Sha1};
use std::path::PathBuf;

// ZettelPathBuf is a trait that adds additional functionality to
// the pathbuf to make it easy to create paths for zettels
pub trait ZettelPathBuf {
    // push_date_path will add a [year]/[month]/[day] directory chain to the
    // path
    fn push_date_path(&mut self);

    // filename_with_hash will create a filename with the following nomenclature
    // [title_as_snakecase]-[8_char_hash].md
    fn filename_with_hash(&mut self, title: &str);
}

impl ZettelPathBuf for PathBuf {
    // push_date_path will add a [year]/[month]/[day] directory chain to the
    // path
    fn push_date_path(&mut self) {
        let current_date = chrono::Utc::now();
        self.push(format!("{:02}", current_date.year()));
        self.push(format!("{:02}", current_date.month()));
        self.push(format!("{:02}", current_date.day()));
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
}
