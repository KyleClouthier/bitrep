# Test data — provenance & license

## `nasa_http_jul95_sizes.csv`

**What.** 6 421 HTTP **response sizes in bytes**, one integer per line — a
real, heavy-tailed web workload (min 0 B, max ≈ 1.27 MB), used by
[`tests/quantile_realdata.rs`](../quantile_realdata.rs) to validate `RelSketch`
quantile accuracy **and byte-identity on genuine real-world data**, hermetically
(the file is embedded with `include_str!`, so the check runs in CI with no
network).

**Source.** The **NASA-HTTP** trace from the *Internet Traffic Archive*: every
HTTP request to the NASA Kennedy Space Center WWW server in Florida over July
1995 (`NASA_access_log_Jul95`, ≈ 1.89 M requests).
<https://ita.ee.lbl.gov/html/contrib/NASA-HTTP.html>

**License / redistribution.** The Internet Traffic Archive states the NASA-HTTP
logs "**may be freely redistributed**." They are provided for research use; this
small derived slice is redistributed here under that permission, with
attribution to the archive and the original collector (Jim Dumoulin, NASA
Kennedy Space Center).

**Exactly how this slice was derived (reproducible).**
1. Download `NASA_access_log_Jul95.gz` (20 676 672 bytes) from the archive URL
   above and gunzip it (205 242 368 bytes, Common Log Format).
2. Keep the trailing byte-count field of every line matching
   `"(GET|POST|HEAD) … " 200 <digits>` — i.e. successful responses with a
   numeric size. That yields 1 701 451 values.
3. Take an even **stride sample**: index `0, 265, 530, …` → 6 421 values, in
   original file order. No shuffling, no filtering by value.

Deterministic result: 6 421 values, LF-terminated, one per line —
`sha256(nasa_http_jul95_sizes.csv)` =
`e72f10b944ee2e20206fd566c6cae6819d984e2d7868aafd3f233b836be0d879`.
(The file is pinned `-text` in `.gitattributes` so the bytes are identical on
every platform checkout, keeping the hash and byte-identity checks stable.)

The raw log itself is **not** committed (20 MB); only this small, license-clean
derived slice is, so the accuracy + reproducibility test is self-contained.
