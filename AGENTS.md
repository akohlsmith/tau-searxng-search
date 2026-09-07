# tau-ext-searxng-search

SearXNG search tool extension for Tau.

## Architecture

- Implements `searxng_search` tool.
- HTTP client queries SearXNG instance via `/search` endpoint with JSON format.
- Normalizes results to consistent output format.
- Supports configurable categories including custom `local` category.
- `prefer_local` parameter enables per-call prioritization of local-category results.

## Build

Standard Cargo workspace build:

```bash
cargo build --release
```

Run tests:

```bash
cargo test
```

## Configuration

See `INSTALL.md` for integration instructions and `config/example.json5` for configuration examples.

## Testing

Unit tests in `src/tests.rs`. Integration tests require running SearXNG instance.

## Files

- `src/lib.rs`: Core extension and tool implementation.
- `src/options.rs`: Search options parsing from tool arguments.
- `src/query.rs`: Query URL building and response parsing utilities.
- `src/tests.rs`: Unit tests for options parsing.
- `specs/SPEC-searxng-search.md`: Project specification.
