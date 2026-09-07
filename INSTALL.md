# Installation

Integration of tau-ext-searxng-search into Tau.

## Prerequisites

- Running SearXNG instance with JSON API enabled.
- HTTPS recommended for production deployments.

## SearXNG configuration

Enable JSON format in your SearXNG `settings.yml`:

```yaml
use_default_settings: true

search:
  formats:
    - html
    - json  # REQUIRED for API access
  default_lang: "en"

server:
  secret_key: "random-secret-string"
  limiter: false  # For local/agent use; enable with valkey for public
```

Configure your custom categories and engines:

```yaml
engines:
  - name: "local-docs"
    engine: "solr"
    base_url: "http://internal-search:8983/solr/local_docs"
    categories:
      - local
    timeout: 5.0
    disabled: false
    weight: 2.0
```

Categories are defined under `engines[].categories[]`. The `local` category should include your custom engines for local searches.

## Tau integration

Copy the extension into your Tau installation:

```bash
cp -r /llama/tau/searxng_search /path/to/tau-install/crates/tau-ext-searxng-search
```

Or integrate directly into the tau repository:

```bash
git clone /llama/tau/searxng_search tau/crates/tau-ext-searxng-search
```

Add the extension to Tau's extension list and configure:

```json5
{
  extensions: {
    "std-searxng-search": {
      enable: true,
      config: {
        base_url: "http://localhost:8080",
        timeout_seconds: 15,
        default_categories: ["general", "local"],
      },
    },
  },
}
```

## Verification

Test the SearXNG JSON API:

```bash
curl -s 'http://localhost:8080/search?q=test&format=json' | jq '.results[:2]'
```

Expected output: JSON array of results with title/url/content fields.

## Notes

- Restart Tau after configuration changes.
- Configuration and secret files are not watched; explicit restart required.
