# tau-ext-searxng-search

A Tau extension that registers a SearXNG-based search tool. Queries a SearXNG instance via HTTP with configurable engines, categories, and optional local-category prioritization.

## Overview

Provides a standalone `searxng_search` tool so agents can query a SearXNG instance with:

- Configurable engine selection and categories.
- Optional prioritization of custom categories (e.g. user-defined `local` category).
- Per-call parameters rather than global configuration for search behavior.
- Normalized results with diagnostics.

SearXNG is privacy-preserving from user-to-internet perspective: queries route through your instance, not directly to upstream providers. Self-hosted instance recommended for internal queries.

## Tools

- `searxng_search`: Query SearXNG instance via HTTP API.

## Configuration

Enable the extension and configure the SearXNG instance URL:

```yaml
extensions:
  search_searxng:
    enable: true
    config:
      base_url: "http://localhost:8080"
      timeout_seconds: 15
      default_categories:
        - general
        - local
      local_category_prefixes:
        - local-
```

Optional reverse-proxy authentication:

```yaml
extensions:
  search_searxng:
    secrets:
      basic_auth: {}
    config:
      base_url: "http://localhost:8080"
      basic_auth_secret: "basic_auth"
```

Optional instance failover:

```yaml
extensions:
  search_searxng:
    config:
      base_url: "http://primary:8080"
      fallback_urls:
        - "http://secondary:8080"
```

### Disable built-in web search tools

Disable Tau's native and external web search tools to route all searches through SearXNG:

```yaml
agents:
  web_tools:
    search:
      candidates:
        native:
          enable: false
        external:
          enable: false
        search_searxng:
          enable: true
          priority: 10
          kind: tool
          tool: search_searxng
```

Restart Tau after changing configuration.

## Tool parameters

`searxng_search`:

Required:
- query: string. Search query.

Optional:
- categories: list of strings. SearXNG categories to use. Default: configured default_categories or ["general", "local"].
- engines: list of strings. Specific engines to use. Default: all engines for selected categories.
- language: string. Language code (e.g. en-US). Default: instance default.
- time_range: string. Time filter (day, week, month, year). Default: no limit.
- num_results: integer. Maximum results to return. Default: 10.
- safe_search: integer. Safe search filter level (0/1/2). Default: instance config.
- prefer_local: boolean. When true, results from categories matching configured local category are ranked higher in the result list. Applies only for that tool call. Default: false.

## Result format

Successful results return normalized JSON-like content within `<tau_web_content>`:

```json
{
  "query": "<original query>",
  "results": [
    {
      "title": "<result title>",
      "url": "<result url>",
      "content": "<snippet>",
      "source_engine": "<engine name>",
      "score": <float>,
      "language": "<language>",
      "published_date": "<date or null>",
      "tags": "<array of strings, omitted when absent>"
    }
  ],
  "count": <integer>,
  "had_errors": false
}
```

## Local category support

Categories include user-defined categories configured in the SearXNG instance. Default categories include `local` alongside `general` so local matches are discovered by default.

The `prefer_local` parameter allows per-call prioritization of local-category results:

- When `prefer_local=true`, results from local-category engines are ranked higher in the returned result list.
- Does not filter non-local results; only reorders preference.
- Applies only for that tool call; not a global configuration setting.

Behavior examples:
- `searxng_search(query="foo")` → searches default categories including local.
- `searxng_search(query="foo", prefer_local=true)` → same search, local results ranked higher.
- `searxng_search(query="foo", categories=["local"])` → searches only local category.

## Error handling

HTTP 403: JSON format not enabled on instance → diagnostic error.
HTTP 429: Instance rate-limited → rate-limited error.
HTTP 5xx: Instance error → retry/failover behavior.
Timeout: instance timeout → tool error.
Parse errors: malformed JSON → tool error.
Empty results: empty result set with diagnostics suggesting category or query review.

## Privacy and trust

- Query visibility: self-hosted instance operator sees all queries.
- Instance trust: self-hosted recommended for internal/critical queries.
- Transport: HTTPS recommended between Tau agent and instance.
- Result content_trust: external.
- HTML sanitization: snippets may contain HTML; sanitize before embedding.

## Installation

See `INSTALL.md` for integration instructions into Tau.
