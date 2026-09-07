# Installation

Build as a standalone Tau extension binary, install it somewhere Tau can find it, configure in harness.yaml.

## Prerequisites

- Running SearXNG instance with JSON API enabled.
- Rust toolchain matching Tau's build environment (Rust 1.91+).
- Tau installation with access to harness.yaml configuration.

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

## Build

```bash
cd /llama/tau/searxng_search
cargo build --release
```

Output binary: `target/release/tau-ext-searxng-search`.

Alternatively, build within Tau's workspace:

```bash
cd /llama/tau/tau
cargo build --release -p tau-ext-searxng-search
```

## Install

Copy the binary to a directory in your PATH or a fixed location:

```bash
mkdir -p /usr/local/tau/bin
cp target/release/tau-ext-searxng-search /usr/local/tau/bin/
```

Adjust the path as needed for your deployment. The extension must be executable.

## Tau configuration

Configure the extension in your harness.yaml:

```yaml
extensions:
  std-searxng-search:
    command: ["/usr/local/tau/bin/tau-ext-searxng-search"]
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

Key fields:
- `command`: absolute path to the standalone extension binary.
- `enable`: must be true for the extension to start.
- `config`: forwarded to the extension via LifecycleConfigure.

This configuration runs tau-ext-searxng-search as a supervised child process connected via stdio over the Tau protocol. No Tau rebuild required.

## Verification

Test the SearXNG JSON API:

```bash
curl -s 'http://localhost:8080/search?q=test&format=json' | jq '.results[:2]'
```

Expected output: JSON array of results with title/url/content fields.

Restart Tau after configuration changes. Check logs for extension startup.

## Notes

- Restart Tau after configuration changes.
- Configuration and secret files are not watched; explicit restart required.
- The extension communicates over stdio using Tau's CBOR protocol.
- Extension startup timeout is 2 seconds by default; increase via `startup_timeout_seconds` if needed.
