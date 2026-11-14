//! BM25 full-text search index powered by `tantivy` (`0.27.0`).

use std::path::Path;

use serde::{Deserialize, Serialize};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Schema, STORED, STRING, TEXT, Value};
use tantivy::{Index, IndexWriter, TantivyDocument};

use crate::{IndexError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FtsDocument {
    pub doc_id: String,
    pub namespace: String,
    pub language: String,
    pub title: String,
    pub body: String,
    pub source_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FtsHit {
    pub score: f32,
    pub document: FtsDocument,
}

#[derive(Clone, Copy)]
struct FtsFields {
    doc_id: Field,
    namespace: Field,
    language: Field,
    title: Field,
    body: Field,
    source_url: Field,
}

pub struct TantivyFtsIndex {
    index: Index,
    fields: FtsFields,
}

impl TantivyFtsIndex {
    fn build_schema() -> (Schema, FtsFields) {
        let mut builder = Schema::builder();
        let doc_id = builder.add_text_field("doc_id", STRING | STORED);
        let namespace = builder.add_text_field("namespace", STRING | STORED);
        let language = builder.add_text_field("language", STRING | STORED);
        let title = builder.add_text_field("title", TEXT | STORED);
        let body = builder.add_text_field("body", TEXT | STORED);
        let source_url = builder.add_text_field("source_url", STRING | STORED);
        (
            builder.build(),
            FtsFields {
                doc_id,
                namespace,
                language,
                title,
                body,
                source_url,
            },
        )
    }

    pub fn open_in_ram() -> Result<Self> {
        let (schema, fields) = Self::build_schema();
        let index = Index::create_in_ram(schema);
        Ok(Self { index, fields })
    }

    pub fn open_or_create_dir(path: impl AsRef<Path>) -> Result<Self> {
        let dir = path.as_ref();
        std::fs::create_dir_all(dir)
            .map_err(|e| IndexError::Other(format!("tantivy create_dir_all error: {e}")))?;
        let (schema, fields) = Self::build_schema();
        let mmap = tantivy::directory::MmapDirectory::open(dir)
            .map_err(|e| IndexError::Other(format!("tantivy directory error: {e}")))?;
        let index = Index::open_or_create(mmap, schema)
            .map_err(|e| IndexError::Other(format!("tantivy open_or_create error: {e}")))?;
        Ok(Self { index, fields })
    }

    pub fn index_documents(&self, docs: &[FtsDocument]) -> Result<usize> {
        let mut writer: IndexWriter<TantivyDocument> = self
            .index
            .writer(15_000_000)
            .map_err(|e| IndexError::Other(format!("tantivy writer error: {e}")))?;
        for doc in docs {
            let mut entry = TantivyDocument::new();
            entry.add_text(self.fields.doc_id, &doc.doc_id);
            entry.add_text(self.fields.namespace, &doc.namespace);
            entry.add_text(self.fields.language, &doc.language);
            entry.add_text(self.fields.title, &doc.title);
            entry.add_text(self.fields.body, &doc.body);
            entry.add_text(self.fields.source_url, &doc.source_url);
            writer
                .add_document(entry)
                .map_err(|e| IndexError::Other(format!("tantivy add_document error: {e}")))?;
        }
        writer
            .commit()
            .map_err(|e| IndexError::Other(format!("tantivy commit error: {e}")))?;
        Ok(docs.len())
    }

    pub fn search(&self, query_text: &str, limit: usize) -> Result<Vec<FtsHit>> {
        let reader = self
            .index
            .reader()
            .map_err(|e| IndexError::Other(format!("tantivy reader error: {e}")))?;
        let searcher = reader.searcher();
        let parser = QueryParser::for_index(
            &self.index,
            vec![self.fields.title, self.fields.body, self.fields.doc_id],
        );
        let query = parser
            .parse_query(query_text)
            .map_err(|e| IndexError::Other(format!("tantivy query parse error: {e}")))?;
        let collector = TopDocs::with_limit(limit.max(1)).order_by_score();
        let top_docs = searcher
            .search(&query, &collector)
            .map_err(|e| IndexError::Other(format!("tantivy search error: {e}")))?;

        let read_str = |doc: &TantivyDocument, field: Field| -> String {
            doc.get_first(field)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_owned()
        };

        let mut hits = Vec::with_capacity(top_docs.len());
        for (score, addr) in top_docs {
            let retrieved: TantivyDocument = searcher
                .doc(addr)
                .map_err(|e| IndexError::Other(format!("tantivy doc fetch error: {e}")))?;
            hits.push(FtsHit {
                score,
                document: FtsDocument {
                    doc_id: read_str(&retrieved, self.fields.doc_id),
                    namespace: read_str(&retrieved, self.fields.namespace),
                    language: read_str(&retrieved, self.fields.language),
                    title: read_str(&retrieved, self.fields.title),
                    body: read_str(&retrieved, self.fields.body),
                    source_url: read_str(&retrieved, self.fields.source_url),
                },
            });
        }
        Ok(hits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tantivy_indexes_and_ranks_multilingual_entries() {
        let fts = TantivyFtsIndex::open_in_ram().unwrap();
        fts.index_documents(&[
            FtsDocument {
                doc_id: "es-afuro".to_owned(),
                namespace: "wiki".to_owned(),
                language: "es".to_owned(),
                title: "Byron Love (Afuro Terumi)".to_owned(),
                body: "Capitan del Instituto Zeus en Inazuma Eleven.".to_owned(),
                source_url: "https://inazuma.fandom.com/es/wiki/Byron_Love".to_owned(),
            },
            FtsDocument {
                doc_id: "ja-zukan".to_owned(),
                namespace: "zukan".to_owned(),
                language: "ja".to_owned(),
                title: "Afuro Terumi Victory Road".to_owned(),
                body: "Zeus captain God Knows technique".to_owned(),
                source_url: "https://zukan.inazuma.jp/".to_owned(),
            },
        ])
        .unwrap();

        let hits = fts.search("Zeus", 5).unwrap();
        assert_eq!(hits.len(), 2);
        let es_hits = fts.search("Instituto", 5).unwrap();
        assert_eq!(es_hits.len(), 1);
        assert_eq!(es_hits[0].document.language, "es");
    }
}
