# Changelog

## [0.2.0] - 2020-07-12

### Changed

* Use [`sailfish`](https://github.com/Kogia-sima/sailfish) instead of [`tera`](https://github.com/Keats/tera).
* Use `dotenv` to get runtime information instead of `clap`.
* Sitemap urls are gathered before user goes to `url/rando`. Resulting in over 100x speedup.
* The above simplified the code base and reduced LOC from 286 to 248 (15% reduction)

### Removed

* Unneeded dependencies
