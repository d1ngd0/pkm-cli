use chrono::{DateTime, Datelike, TimeZone};
use image::{
    ImageReader,
    imageops::{self, FilterType::Gaussian},
};

use crate::{Result, ZettelIDBuilder};
use std::{
    fs::{self, File},
    path::{Path, PathBuf, StripPrefixError},
};

pub struct ImageBuilder {
    base: PathBuf,
    max_width: Option<u32>,
    max_height: Option<u32>,
}

impl ImageBuilder {
    pub fn new<P: AsRef<Path>>(base: P) -> Self {
        Self {
            base: PathBuf::from(base.as_ref()),
            max_width: None,
            max_height: None,
        }
    }

    pub fn subdirectory<P: AsRef<Path>>(mut self, subdir: P) -> Self {
        self.base.push(subdir.as_ref());
        self
    }

    // with_date_directory will add the path [year]/[month]/[day] to the base
    // when creating the image
    pub fn with_date_directory<Tz: TimeZone>(mut self, date: &DateTime<Tz>) -> Self {
        self.base.push(format!("{:02}", date.year()));
        self.base.push(format!("{:02}", date.month()));
        self.base.push(format!("{:02}", date.day()));
        self
    }

    pub fn max_width(mut self, width: Option<u32>) -> Self {
        self.max_width = width;
        self
    }

    pub fn max_height(mut self, height: Option<u32>) -> Self {
        self.max_height = height;
        self
    }

    pub fn build<P>(self, path: P) -> Result<Image>
    where
        P: AsRef<Path>,
    {
        let Self {
            base,
            max_width,
            max_height,
        } = self;

        let img = ImageReader::open(path.as_ref())?.decode()?;
        let img = img.to_rgb8();
        let mut width = img.width();
        let mut height = img.height();

        // if max width is set make sure to adjust things
        if let Some(max_width) = max_width {
            if max_width < width {
                let ratio = width / max_width;
                width = max_width;
                height = height * ratio;
            }
        }

        // if height width is set make sure to adjust things
        if let Some(max_height) = max_height {
            if max_height < height {
                let ratio = height / max_height;
                height = max_height;
                width = width * ratio;
            }
        }

        let img = imageops::resize(&img, width, height, Gaussian);

        // Create the directory for the thing to live in
        fs::create_dir_all(base.as_path())?; // only creates the directories, not the file

        let mut id = ZettelIDBuilder::new(None).with_hash().to_string()?;
        id.push_str(".jpg");

        let mut path = PathBuf::from(base);
        path.push(id);

        let mut image_file = File::create(path.as_path())?;
        img.write_to(&mut image_file, image::ImageFormat::Jpeg)?;
        image_file.sync_all()?;

        Ok(Image { path })
    }
}

pub struct Image {
    pub path: PathBuf,
}

impl Image {
    pub fn rel_path<P: AsRef<Path>>(
        &self,
        parent: P,
    ) -> std::result::Result<&Path, StripPrefixError> {
        self.path.strip_prefix(parent)
    }
}
