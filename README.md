# Tantivy Terrible Search Server

This is a search server powered by [tantivy](https://github.com/tantivy-search/tantivy) and [actix web](https://actix.rs/)

## Note on building

Always compile with `cargo build --release`!
It makes a huge difference(I measured a 5-7X increase in throughput here).

### Usage

make a `.env` file in directory like this

```sh
ADDR="127.0.0.1"                  # Address for server
PORT="8080"                       # Port for server
TANTIVY_INDEX="/path/to/index"    # Aboslute path to tantivy-index
SITEMAP="/path/to/sitemap.xml"    # Aboslute path to sitemap
```

`ttss`

Optionally you may create a `sailfish.yml` in the same directory as the `Cargo.toml` if you wish to store your templates somewhere else. Just make a `sailfish.yml`

```yml
template_dir: "/path/to/templates"
```

### Performance

Testing was done with wrk2 with 10 threads 10k connections (with various rates) for 1 minute on a Ryzen 3600X.
I tested the following:

* **Test 1** - Index (no search gets performed) - Rate @ 100k
* **Test 2** - Query with three results - Rate @ 3k
* **Test 3** - Query with no results found - Rate @ 3100
* **Test 4** - Redirect to random page from `sitemap.xml` - Rate @ 100k

With a total of 10 indexed html files

| Tests              | 1 Index   | 2 Qry(3) | 3 Qry(0) | 4 Random  |
| ------------------ | --------- | -------- | -------- | --------- |
| Thread Latency     | 2.58ms    | 183.59ms | 181.47ms | 2.37ms    |
| Thread Requests/s  | 10.64k    | 300.42   | 307.87   | 10.65k    |
| Lat 50%            | 1.89ms    | 115.20ms | 129.73ms | 1.83ms    |
| Lat 75%            | 3.09ms    | 406.27ms | 390ms    | 2.89ms    |
| Lat 90%            | 5.43ms    | 449.79ms | 427.01ms | 4.81ms    |
| Lat 99%            | 10.98ms   | 477.18ms | 451.84ms | 9.01ms    |
| Lat 99.999%        | 23.78ms   | 499.45ms | 467.20ms | 18.37ms   |
| Total Requests/sec | 95,964.95 | 2944.15  | 3042     | 95,692.84 |
