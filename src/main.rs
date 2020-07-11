use std::{
    collections::HashMap,
    convert::From,
    fs::File,
    path::{Path, PathBuf},
};

#[macro_use]
extern crate serde_derive;

use actix_web::client::Client;
use actix_web::{error as AnotherError, http, middleware, web, App, Error as ActixError, HttpResponse, HttpServer};
use clap::{value_t, Arg};
use rand::Rng;
use sitemap::{structs::Location, reader::{SiteMapEntity, SiteMapReader}};
use once_cell::sync::OnceCell;

use tantivy::{
    collector::{Count, TopDocs},
    query::QueryParser,
    schema::{Field, FieldType, NamedFieldDocument, Schema, Value},
    tokenizer::*,
    DocAddress, Document, Index, IndexReader, Score,
};
use tera::Tera;

// store tera template in application state
async fn index(
    tmpl: web::Data<tera::Tera>,
    query: web::Query<HashMap<String, String>>,
    tantivy_index: web::Data<PathBuf>
) -> Result<HttpResponse, ActixError> {
    let mut ctx = tera::Context::new();
    let s = if let Some(query) = query.get("q") {
        // submitted form
        let pages = get_search_results(&query, tantivy_index);
        if pages.is_empty() {
            ctx.insert("query", &query);
            tmpl.render("noresults.html", &ctx)
                .map_err(|_| AnotherError::ErrorInternalServerError("Template error for noresults"))?
        } else {
            ctx.insert("query", &query);
            ctx.insert("pages", &pages);
            tmpl.render("search.html", &ctx)
                .map_err(|_| AnotherError::ErrorInternalServerError("Template error for search"))?
        }
    } else {
        tmpl.render("index.html", &ctx)
            .map_err(|_| AnotherError::ErrorInternalServerError("Template error for index"))?
    };
    Ok(HttpResponse::Ok().content_type("text/html").body(s))
}

async fn random(site_map: web::Data<Vec<Location>>) -> HttpResponse {
    HttpResponse::Found()
        .header(
            http::header::LOCATION,
            site_map[rand::thread_rng().gen_range(0, site_map.len())].get_url().unwrap().as_str(),
        )
        .finish()
        .into_body()
}


#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    let matches = clap::App::new("HTTP Proxy")
        .arg(
            Arg::with_name("listen_addr")
                .takes_value(true)
                .value_name("LISTEN ADDR")
                .index(1)
                .required(true),
        )
        .arg(
            Arg::with_name("listen_port")
                .takes_value(true)
                .value_name("LISTEN PORT")
                .index(2)
                .required(true),
        )
        .arg(
            Arg::with_name("tera_templates")
                .takes_value(true)
                .value_name("PATH TO TERA TEMPLATES")
                .index(3)
                .required(true),
        )
        .arg(
            Arg::with_name("tantivy_index")
                .takes_value(true)
                .value_name("PATH TO TANTIVY INDEX")
                .index(4)
                .required(true),
        )
        .arg(
            Arg::with_name("sitemap")
                .takes_value(true)
                .value_name("PATH TO SITEMAP")
                .index(5)
                .required(true),
        )
        .get_matches();

    let listen_addr = matches.value_of("listen_addr").unwrap();
    let listen_port = value_t!(matches, "listen_port", u16).unwrap_or_else(|e| e.exit());

    let tera_templates = value_t!(matches, "tera_templates", PathBuf).unwrap_or_else(|e| e.exit());
    let tera = Tera::new(&(tera_templates.as_os_str().to_str().unwrap().to_string() + "/**/*")).unwrap();
    let tantivy_index = value_t!(matches, "tantivy_index", PathBuf).unwrap_or_else(|e| e.exit());
    //let sitemap = value_t!(matches, "sitemap", String).unwrap_or_else(|e| e.exit());

    static SITEMAP: OnceCell<Vec<Location>> = OnceCell::new();
    SITEMAP.get_or_init(|| {
        let sitemap = value_t!(matches, "sitemap", String).unwrap_or_else(|e| e.exit());
        let mut urls = Vec::new();
        for entity in SiteMapReader::new(File::open(Path::new(&sitemap)).unwrap()) {
            if let SiteMapEntity::Url(url_entry) = entity {
                urls.push(url_entry.loc);
            }
        }
        urls
    });

    HttpServer::new(move || {
        App::new()
            .data(Client::new())
            .data(tera.clone())
            .data(tantivy_index.clone())
            .data(SITEMAP.clone())
            .wrap(middleware::Logger::default())
            .service(web::resource("/").route(web::get().to(index)))
            .service(web::resource("/rando").route(web::get().to(random)))
    })
    .bind((listen_addr, listen_port))?
    .system_exit()
    .run()
    .await
}

fn get_search_results(query: &str, tantivy_index: web::Data<PathBuf>) -> Vec<Page> {
    let index_directory = PathBuf::from(tantivy_index.get_ref());
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
        let index = Index::open_in_dir(path).unwrap();
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
