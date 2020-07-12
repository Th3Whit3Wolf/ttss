use std::{
    collections::HashMap,
    convert::From,
    fs::File,
    path::{Path, PathBuf},
};

#[macro_use]
extern crate sailfish_macros;
#[macro_use]
extern crate serde_derive;

use actix_web::{web, middleware, App, error::InternalError, http::StatusCode, HttpResponse, HttpServer};
use dotenv::dotenv;
use once_cell::sync::Lazy;
use rand::Rng;
use sailfish::TemplateOnce;
use sitemap::{structs::Location, reader::{SiteMapEntity, SiteMapReader}};
use tantivy::{
    collector::{Count, TopDocs},
    query::QueryParser,
    schema::{Field, FieldType, NamedFieldDocument, Schema, Value},
    tokenizer::*,
    DocAddress, Document, Index as TantivyIndex, IndexReader, Score,
};

static ADDR: Lazy<String> = Lazy::new (||{
    std::env::var("ADDR").expect("ADDR must be set")
});

static PORT: Lazy<u16> = Lazy::new (||{
    std::env::var("PORT").expect("PORT must be set").parse::<u16>().unwrap()
});

static SITEMAP: Lazy<Vec<Location>> = Lazy::new (||{
        let mut urls = Vec::new();
        for entity in SiteMapReader::new(File::open(Path::new(&std::env::var("SITEMAP").expect("SITEMAP must be set"))).unwrap()) {
            if let SiteMapEntity::Url(url_entry) = entity {
                urls.push(url_entry.loc);
            }
        }
        urls
});

static TANTIVY_INDEX: Lazy<String> = Lazy::new (||{
    std::env::var("TANTIVY_INDEX").expect("TANTIVY_INDEX must be set")
});

#[derive(TemplateOnce)]
#[template(path = "search.stpl")]
struct Search<'a> {
    query: &'a str,
    pages: Vec<Page>,
}

#[derive(TemplateOnce)]
#[template(path = "index.stpl")]
struct Index;

async fn index(query: web::Query<HashMap<String, String>>) -> actix_web::Result<HttpResponse> {
    let body = if let Some(query) = query.get("q") {
        let pages = get_search_results(&query);
        Search { query, pages }
        .render_once()
        .map_err(|e| InternalError::new(e, StatusCode::INTERNAL_SERVER_ERROR))?
        } else {
        Index
        .render_once()
        .map_err(|e| InternalError::new(e, StatusCode::INTERNAL_SERVER_ERROR))?
    };

    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(body))
}

async fn random() -> actix_web::Result<HttpResponse> {
    Ok(HttpResponse::Found()
        .header(
            actix_web::http::header::LOCATION,
            SITEMAP[rand::thread_rng().gen_range(0, SITEMAP.len())].get_url().unwrap().as_str(),
        )
        .finish()
        .into_body())
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    
    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .service(web::resource("/").route(web::get().to(index)))
            .service(web::resource("/rando").route(web::get().to(random)))
    })
    .bind((ADDR.as_str(), *PORT))?
    .run()
    .await
}


fn get_search_results(query: &str) -> Vec<Page> {
    let index_directory = PathBuf::from(TANTIVY_INDEX.as_str());
    let index_server = IndexServer::load(&index_directory);
    let serp = index_server.search(query.to_string(), 100).unwrap();
    let scheme = serp.schema;
    let title_field = scheme.get_field("title").unwrap();
    let desc_field = scheme.get_field("description").unwrap();
    let link_field = scheme.get_field("permalink").unwrap();
    let date_field = scheme.get_field("datetime").unwrap();
    let mut v = Vec::with_capacity(10);
    for hit in serp.hits {
        let doc = scheme.convert_named_doc(hit.doc).unwrap();
        let title_val = doc.get_first(title_field).unwrap().clone();
        let desc_val = doc.get_first(desc_field).unwrap().clone();
        let link_val = doc.get_first(link_field).unwrap().clone();
        let date_val = doc.get_first(date_field);

        let title = match title_val {
            Value::Str(x) => Some(x),
            _ => None,
        };
        let desc = match desc_val {
            Value::Str(x) => Some(x),
            _ => None,
        };
        let link = match link_val {
            Value::Str(x) => Some(x),
            _ => None,
        };
        let date = match date_val {
            Some(x) => match x {
                Value::Date(x) => Some(
                    x.to_rfc2822()
                        .split_whitespace()
                        .take(3)
                        .collect::<Vec<&str>>()
                        .join(" "),
                ),
                _ => None,
            },
            _ => None,
        };
        v.push(Page {
            title,
            desc,
            link,
            date,
        })
    }
    v
}

#[derive(Serialize)]
struct Page {
    title: Option<String>,
    desc: Option<String>,
    link: Option<String>,
    date: Option<String>,
}

#[derive(Serialize)]
struct Serp {
    q: String,
    num_hits: usize,
    hits: Vec<Hit>,
    schema: Schema,
}

#[derive(Serialize)]
struct Hit {
    score: Score,
    doc: NamedFieldDocument,
    id: u32,
}

struct IndexServer {
    reader: IndexReader,
    query_parser: QueryParser,
    schema: Schema,
}

impl IndexServer {
    fn load(path: &Path) -> IndexServer {
        let index = TantivyIndex::open_in_dir(path).unwrap();
        index.tokenizers().register(
            "commoncrawl",
            TextAnalyzer::from(SimpleTokenizer)
                .filter(RemoveLongFilter::limit(40))
                .filter(LowerCaser)
                .filter(AlphaNumOnlyFilter)
                .filter(Stemmer::new(Language::English)),
        );
        let schema = index.schema();
        let default_fields: Vec<Field> = schema
            .fields()
            .filter(|&(_, ref field_entry)| match *field_entry.field_type() {
                FieldType::Str(ref text_field_options) => {
                    text_field_options.get_indexing_options().is_some()
                }
                _ => false,
            })
            .map(|(field, _)| field)
            .collect();
        let query_parser =
            QueryParser::new(schema.clone(), default_fields, index.tokenizers().clone());
        let reader = index.reader().unwrap();
        IndexServer {
            reader,
            query_parser,
            schema,
        }
    }

    fn create_hit(&self, score: Score, doc: &Document, doc_address: DocAddress) -> Hit {
        Hit {
            score,
            doc: self.schema.to_named_doc(&doc),
            id: doc_address.doc(),
        }
    }

    fn search(&self, q: String, num_hits: usize) -> tantivy::Result<Serp> {
        let query = self
            .query_parser
            .parse_query(&q)
            .expect("Parsing the query failed");
        let searcher = self.reader.searcher();
        let (top_docs, num_hits) =
            { searcher.search(&query, &(TopDocs::with_limit(num_hits), Count))? };
        let hits: Vec<Hit> = {
            top_docs
                .iter()
                .map(|(score, doc_address)| {
                    let doc: Document = searcher.doc(*doc_address).unwrap();
                    self.create_hit(*score, &doc, *doc_address)
                })
                .collect()
        };
        Ok(Serp {
            q,
            num_hits,
            hits,
            schema: self.schema.clone(),
        })
    }
}
