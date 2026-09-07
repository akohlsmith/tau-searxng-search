# Specification: tau-ext-searxng-search

## Objective

Provide a SearXNG-based search tool so agents can query a SearXNG instance with configurable engines/categories and receive normalized results.

## Primary goals

- Privacy-preserving search via self-hosted SearXNG.
- Configurable engine selection and categories.
- Consistent result format with diagnostics.
- Standalone tool with optional provider integration.

## Design: tool model

Recommended: dedicated tool (`searxng_search`).

Rationale:
- SearXNG is self-hosted/operator-controlled; failure modes and trust model differ from opaque third-party providers.
- Cleaner separation: explicit configuration, explicit opt-in.
- Avoids awkward failover semantics in default web_search provider pool.

Alternative: add SearXNG as provider backend for `web_search`.
Rationale:
- Consistent with existing provider failover patterns.
- Useful if wrapping hosted SearXNG instances as providers.

Initial implementation: dedicated tool. Provider integration is optional Phase 2.

## Tool specification

Name: `searxng_search`

Parameters:

Required:
- query: string. Search query.

Optional:
- categories: list of strings. SearXNG categories. Default: configured default_categories or ["general", "local"].
- engines: list of strings. Specific engines. Default: all engines for selected categories.
- language: string. Language code (e.g. en-US). Default: instance default.
- time_range: string. Time filter (day/week/month/year). Default: no limit.
- num_results: integer. Max results. Default: 10. Max enforced by instance.
- safe_search: integer. Filter level (0/1/2). Default: instance config.
- prefer_local: boolean. Prioritize local-category results in result ordering. Default: false.

## Result format

Successful results return as:

<tau_web_content adapter="searxng" operation="search">...</tau_web_content>

Content: normalized JSON-like structure:

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
      "tags": "[array of strings or omitted]"
    }
  ],
  "count": <integer>,
  "had_errors": false
}
```

Result fields:

- title: result title from search engine.
- url: result URL.
- content: text snippet or summary.
- source_engine: SearXNG engine that provided the result.
- score: numerical relevance score.
- language: detected result language.
- published_date: publication date string if available.
- tags: combined array of tags from both karakeep_tags and tags fields in search results.
  If only one field exists, its value is used. If both exist, they are merged into a
  single array. The field is omitted when neither exists, or both are absent/null/empty.

## Error handling

HTTP 403: JSON format not enabled → diagnostic error.
HTTP 429: Instance rate-limited → rate-limited error.
HTTP 5xx: Instance error → retry/failover behavior.
Timeout: instance timeout → tool error.
Parse errors: malformed JSON → tool error.
Empty results: empty result set with diagnostics suggesting category or query review.

## Configuration

Required:
- base_url: URL of SearXNG instance.

Optional:
- timeout_seconds: per-request timeout (default 15s).
- basic_auth_secret: secret name for reverse-proxy auth.
- default_categories: default categories for queries without explicit categories.
- fallback_urls: list of URLs for instance failover.
- local_category_prefixes: prefixes identifying local-category results for prefer_local behavior.

## Local category integration

Categories include user-defined categories configured in the SearXNG instance. Default categories include `local` alongside `general` so local matches are discovered by default.

The `prefer_local` parameter allows per-call prioritization:

- When true, results from local-category engines are ranked higher in the result list.
- Does not filter non-local results; only reorders preference.
- Applies only for that tool call; not a global configuration setting.

Implementation: results are reordered by `prefer_local` logic after receiving from SearXNG. The local-category detection uses configured category prefixes or explicit category names.

Behavior examples:
- `searxng_search(query="foo")` → searches default categories including local.
- `searxng_search(query="foo", prefer_local=true)` → same search, local results ranked higher.
- `searxng_search(query="foo", categories=["local"])` → searches only local category.

## Privacy and trust

- Query visibility: self-hosted instance operator sees all queries.
- Instance trust: self-hosted recommended for internal/critical queries.
- Transport: HTTPS recommended between Tau agent and instance.
- Result content_trust: external.
- HTML sanitization: snippets may contain HTML; sanitize before embedding.

## Integration phases

Phase 1: Core tool implementation
- Implement searxng_search tool.
- Configuration parsing and validation.
- HTTP client with timeout and auth.
- Result normalization and error mapping.
- Local category support.

Phase 2: Provider integration (optional)
- Wire SearXNG as optional web_search provider backend.
- Provider failover logic.
- Consistent result format for web_search compatibility.

Phase 3: Hardening (optional)
- Instance health checks and diagnostics.
- Rate limiting and caching.
- Advanced configuration options.
