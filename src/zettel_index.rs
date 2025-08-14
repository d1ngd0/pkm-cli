use std::fs::{self, read_to_string};
use std::path::{Path, PathBuf};

use crate::{Error, Result, first_node};
use markdown::ParseOptions;
use markdown::mdast::Node;
use tantivy::directory::MmapDirectory;
use tantivy::schema::{IndexRecordOption, SchemaBuilder, TextFieldIndexing, TextOptions};
use tantivy::{Index, IndexWriter, TantivyDocument, doc};

pub fn path_to_id<P>(path: P) -> String
where
    P: AsRef<Path>,
{
    let id = path
        .as_ref()
        .file_name()
        .expect("why don't you have a filename")
        .to_string_lossy();
    id.trim_end_matches(".md").into()
}

pub struct ZettelIndex<P: AsRef<Path>> {
    parent: P,
    index: Index,
}

impl<P: AsRef<Path>> ZettelIndex<P> {
    pub fn new(dir: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let mut schema = SchemaBuilder::new();

        schema.add_text_field(
            "title",
            TextOptions::default().set_stored().set_indexing_options(
                TextFieldIndexing::default()
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions)
                    .set_tokenizer("en_stem"),
            ),
        );

        schema.add_text_field(
            "content",
            TextOptions::default().set_indexing_options(
                TextFieldIndexing::default()
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions)
                    .set_tokenizer("en_stem"),
            ),
        );

        schema.add_text_field(
            "uri",
            TextOptions::default().set_stored().set_indexing_options(
                TextFieldIndexing::default()
                    .set_index_option(IndexRecordOption::default())
                    .set_tokenizer("raw"),
            ),
        );

        schema.add_text_field(
            "id",
            TextOptions::default().set_stored().set_indexing_options(
                TextFieldIndexing::default()
                    .set_index_option(IndexRecordOption::default())
                    .set_tokenizer("raw"),
            ),
        );

        let mut index_dir = PathBuf::new();
        index_dir.push(dir.as_ref());
        index_dir.push(".index");

        // create the directory if it doesn't exist
        if !fs::exists(index_dir.as_path())? {
            fs::create_dir(index_dir.as_path())?;
        }

        let index_dir = MmapDirectory::open(index_dir.as_path())?;
        let index = Index::open_or_create(index_dir, schema.build())?;

        Ok(Self { index, parent: dir })
    }

    pub fn doc_indexer(&self) -> Result<DocIndexer<P>> {
        Ok(DocIndexer {
            index: self,
            writer: self.index.writer(15_000_000)?,
        })
    }
}

pub struct DocIndexer<'a, P: AsRef<Path>> {
    index: &'a ZettelIndex<P>,
    writer: IndexWriter<TantivyDocument>,
}

impl<'a, P: AsRef<Path>> DocIndexer<'a, P> {
    pub fn clear(&mut self) -> Result<()> {
        self.writer.delete_all_documents()?;
        Ok(())
    }

    pub fn process<Q>(&mut self, id: &str, doc: Q) -> Result<()>
    where
        Q: AsRef<Path>,
    {
        let mut full_doc_path = PathBuf::new();
        full_doc_path.push(self.index.parent.as_ref());
        full_doc_path.push(doc.as_ref());
        let content = read_to_string(full_doc_path.as_path())?;

        let opts = ParseOptions::gfm();
        let ast = markdown::to_mdast(&content, &opts)?;
        let header = first_node!(&ast, Node::Heading).ok_or(Error::IndexError(
            tantivy::TantivyError::InvalidArgument(String::from("No title in document")),
        ))?;

        let mut title = None;
        if let Some(Node::Text(header_content)) = header.children.get(0) {
            title = Some(header_content.value.as_str());
        }

        let title = title.ok_or(Error::IndexError(tantivy::TantivyError::InvalidArgument(
            String::from("Title must be supplied"),
        )))?;

        self.writer.add_document(doc!(
            self.writer.index().schema().get_field("title").expect("title not in schema") => title,
            self.writer.index().schema().get_field("content").expect("content not in schema")  => content,
            self.writer.index().schema().get_field("uri").expect("uri not in schema")  => *doc.as_ref().to_string_lossy(),
            self.writer.index().schema().get_field("uri").expect("id not in schema")  => id,
        ))?;

        Ok(())
    }

    pub fn commit(mut self) -> Result<()> {
        self.writer.commit()?;
        Ok(())
    }
}
