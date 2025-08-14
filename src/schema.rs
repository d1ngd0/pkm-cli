use std::fs::read_to_string;

use crate::{Error, Result, find_node};
use markdown::mdast::{Heading, Node};
use tantivy::schema::{IndexRecordOption, Schema, SchemaBuilder, TextFieldIndexing, TextOptions};
use tantivy::{Document, Index as TIndex, IndexWriter};

pub struct Index {
    index: TIndex,
}

impl Index {
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

        let index_dir = PathBuf::new();
        index_dir.push(".index");
        let index = TIndex::open_or_create(&index_dir, schema.build())?;

        Ok(Index { index })
    }

    pub fn doc_indexer<D>(&self) -> Result<DocIndexer<D>>
    where
        D: Document,
    {
        Ok(DocIndexer {
            writer: self.index.writer(1_000_000)?,
        })
    }
}

struct DocIndexer<D: Document> {
    writer: IndexWriter<D>,
}

impl<D: Document> DocIndexer<D> {
    fn process<P>(doc: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let content = read_to_string(doc.as_ref())?;

        let opts = ParseOptions::gfm();
        let ast = markdown::to_mdast(&favorites, &opts)?;
        let title = find_node!(&ast, Node::Heading).ok_or(Error::IndexError(
            tantivy::TantivyError::InvalidArgument(String::from("No title in document")),
        ))?;
        let title = find_node!(&Node::Heading(*title), Node::Text)
            .ok_or(Error::IndexError(tantivy::TantivyError::InvalidArgument(
                String::from("Empty title in document"),
            )))?
            .value;

        Ok(())
    }
}
