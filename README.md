# Tantivy Tera Search Server

This is a search server powered by [tantivy](https://github.com/tantivy-search/tantivy) and [actix web](https://actix.rs/)

## Note on building

Always compile with `cargo build --release`!
It makes a huge difference(I measured a 5-7X increase in throughput here).

### Usage

`ttss 127.0.0.1 10011 templates ~/zola/tantivy-index ~/zola/public/sitemap.xml`

### Performance

Testing was done with wrk with 10 threads 10k connections for 1 minute.
I tested the following

* **Test 1** - Index (no search gets performed)
* **Test 2** - Query with three results
* **Test 3** - Query with no results found
* **Test 4** - Redirect to random page from `sitemap.xml`

With a total of 22 indexed html files

| Test | Thread Latency          | Thread Requests/s   | Lat 50%  | Lat 75%  | Lat 90%  | Lat 99%  | Total Requests/sec |
| :--- | :---------------------: | :-----------------: | :------: | :------: | :------: | :------: | :----------------: | 
|   1  | 4.85 ms (+/- 8.06ms)    | 20.19k (+/- 10.05k) | 2.32ms   | 3.39ms   | 16.68ms  | 40.87ms  | 200,840.26         |
|   2  | 347.07ms (+/- 17.31ms)  | 317.98 (+/- 180.40) | 347.83ms | 353.25ms | 355.09ms | 368.66ms | 2,899.75           |
|   3  |  330.83ms (+/- 15.41ms) | 316.17 (+/- 205.29) | 330.92ms | 334.05ms | 338.35ms | 344.95ms | 3,040.79            |
| 4    | 454.06 ms (+/- 35.41ms)    | 223.72 (+/- 120.35) | 451.95ms   | 466.66ms   | 487.82ms  | 536.80ms  | 2,212.33         |
