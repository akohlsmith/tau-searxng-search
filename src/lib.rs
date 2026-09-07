//! SearXNG search extension for Tau.
//!
//! Registers `searxng_search` tool that queries a SearXNG instance via HTTP
//! API with configurable engines, categories, and local-category prioritization.

use std::error::Error;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tau_client::{ClientError, ClientResult, ExtensionBuilder, TauExtension};
use tau_proto::{Event, ToolCallId, ToolError, ToolName, ToolProgress, ToolResult, ToolSpec, ToolUseState};
use url::Url;

/// Log target for events emitted from this extension.
pub const LOG_TARGET: &str = "searxng_search";

/// Tool name advertised to models.
pub const MODEL_VISIBLE_TOOL_NAME: &str = "searxng_search";

/// Default SearXNG instance URL.
pub const DEFAULT_BASE_URL: &str = "http://localhost:8080";

/// Default request timeout.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Maximum UTF-8 byte length for tool output.
const TOOL_OUTPUT_MAX_BYTES: usize = 512 * 1024;

/// Default categories used when query doesn't specify categories.
const DEFAULT_CATEGORIES: &[&str] = &["general", "local"];

/// Maximum results to request from SearXNG.
const DEFAULT_NUM_RESULTS: u32 = 10;

/// Maximum results allowed from SearXNG.
const MAX_NUM_RESULTS: u32 = 100;

mod options;
mod query;
#[cfg(test)]
mod tests;

use options::SearchOptions;

#[derive(Debug, Clone)]
struct SearXNGClient {
    base_url: Url,
    timeout: Duration,
    default_categories: Vec<String>,
    local_category_prefixes: Vec<String>,
}

impl SearXNGClient {
    fn new(base_url: Url, timeout: Duration, default_categories: Vec<String>, local_category_prefixes: Vec<String>) -> Self {
        Self {
            base_url,
            timeout,
            default_categories,
            local_category_prefixes,
        }
    }

    fn search(&self, query: &str, options: &SearchOptions) -> Result<String, String> {
        let categories: Vec<String> = options.categories.clone().unwrap_or_else(|| {
            self.default_categories.clone()
        });

        let num_results = options.num_results.unwrap_or(DEFAULT_NUM_RESULTS) as usize;
        let capped_num_results = num_results.min(MAX_NUM_RESULTS as usize);

        let mut url_builder = self.base_url.join("search/").unwrap();

        let mut params = url_builder.query_pairs_mut();
        params.append_pair("q", query);
        params.append_pair("format", "json");

        if let Some(lang) = &options.language {
            params.append_pair("language", lang);
        }

        if let Some(time_range) = &options.time_range {
            params.append_pair("time_range", time_range);
        }

        if let Some(safe_search) = options.safe_search {
            params.append_pair("safesearch", &safe_search.to_string());
        }

        // Map categories; local category is passed through directly
        let categories_str = categories.join(",");
        params.append_pair("categories", &categories_str);

        if !options.engines.is_empty() {
            let engines_str = options.engines.join(",");
            params.append_pair("engines", &engines_str);
        }

        let request_url = url_builder.to_string();

        let response = ureq::get(&request_url)
            .timeout(self.timeout)
            .call();

        match response {
            Ok(response) => {
                let status = response.status();
                let body = response.into_string()
                    .map_err(|e| format!("failed to read response body: {e}"))?;

                match status {
                    200 => Ok(self.parse_search_response(&body, options.prefer_local)),
                    403 => Err("SearXNG returned HTTP 403: JSON format may not be enabled. Configure search.formats in settings.yml.".to_string()),
                    429 => Err("SearXNG returned HTTP 429: instance rate-limited".to_string()),
                    _ => Err(format!("SearXNG returned HTTP {status}")),
                }
            }
            Err(e) => Err(format!("SearXNG request failed: {e}")),
        }
    }

    fn parse_search_response(&self, body: &str, prefer_local: bool) -> String {
        let parsed: serde_json::Value = match serde_json::from_str(body) {
            Ok(v) => v,
            Err(e) => return format!("failed to parse response: {e}"),
        };

        let results = parsed.get("results")
            .and_then(|v| v.as_array())
            .unwrap_or(&Vec::<Value>::new());

        let mut normalized_results = Vec::new();

        for result in results {
            let title = result.get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let url = result.get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let content = result.get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let source_engine = result.get("engines")
                .and_then(|engines| engines.as_array())
                .and_then(|arr| arr.first())
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let score = result.get("score")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            let language = result.get("language")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let published_date = result.get("publishedDate")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let is_local = self.is_local_result(result);

            normalized_results.push(json!({
                "title": title,
                "url": url,
                "content": content,
                "source_engine": source_engine,
                "score": score,
                "language": language,
                "published_date": published_date,
                "_is_local": is_local
            }));
        }

        if prefer_local {
            normalized_results.sort_by(|a, b| {
                let a_is_local = a.get("_is_local").and_then(|v| v.as_bool()).unwrap_or(false);
                let b_is_local = b.get("_is_local").and_then(|v| v.as_bool()).unwrap_or(false);
                // Local results first, then by score descending
                if a_is_local && !b_is_local {
                    std::cmp::Ordering::Less
                } else if !a_is_local && b_is_local {
                    std::cmp::Ordering::Greater
                } else {
                    // Preserve original score ordering
                    let score_a = a.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let score_b = b.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
                }
            });
        }

        // Remove internal marker
        for result in &mut normalized_results {
            result.as_object_mut().map(|obj| {
                obj.remove("_is_local");
            });
        }

        let count = normalized_results.len();
        let query_str = parsed.get("query")
            .and_then(|q| q.get("query"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let output = json!({
            "query": query_str,
            "results": normalized_results,
            "count": count,
            "had_errors": false
        });

        let output_str = serde_json::to_string(&output)
            .unwrap_or_else(|_| "\"search completed\"");

        if output_str.len() > TOOL_OUTPUT_MAX_BYTES {
            let truncated = output_str[..TOOL_OUTPUT_MAX_BYTES.saturating_sub(5)].to_string();
            format!("{truncated}… (truncated)")
        } else {
            output_str
        }
    }

    fn is_local_result(&self, result: &serde_json::Value) -> bool {
        let engines = result.get("engines")
            .and_then(|v| v.as_array());

        if let Some(engines) = engines {
            for engine in engines {
                let engine_name = engine.as_str().unwrap_or("");
                for prefix in &self.local_category_prefixes {
                    if engine_name.starts_with(prefix.as_str()) {
                        return true;
                    }
                }
            }
        }

        false
    }
}

fn default_timeout() -> u64 {
    REQUEST_TIMEOUT.as_secs()
}

fn default_local_category_prefixes() -> Vec<String> {
    vec!["local-".to_string()]
}

fn validate_url(url_str: &str) -> Result<Url, String> {
    match url_str.parse::<Url>() {
        Ok(url) => {
            if url.scheme() != "http" && url.scheme() != "https" {
                return Err("base_url must use http or https scheme".to_string());
            }
            Ok(url)
        }
        Err(e) => Err(format!("invalid base_url: {e}")),
    }
}

fn extension_builder() -> ExtensionBuilder {
    ExtensionBuilder::new("std-searxng-search")
        .with_version(env!("CARGO_PKG_VERSION"))
        .with_description("SearXNG search tool for Tau")
}

#[derive(Debug, Deserialize)]
struct Config {
    #[serde(default)]
    base_url: String,
    #[serde(default = "default_timeout")]
    timeout_seconds: u64,
    #[serde(default)]
    default_categories: Vec<String>,
    #[serde(default)]
    fallback_urls: Vec<String>,
    #[serde(default = "default_local_category_prefixes")]
    local_category_prefixes: Vec<String>,
}

impl Config {
    fn validate(&self) -> ClientResult<()> {
        let base_url = validate_url(&self.base_url)?;
        Ok(())
    }
}

pub fn create_extension(builder: ExtensionBuilder) -> ClientResult<Arc<dyn TauExtension>> {
    let config: Config = builder.config()?;
    config.validate()?;

    let base_url = validate_url(&config.base_url)?;
    let timeout = Duration::from_secs(config.timeout_seconds);
    let client = Arc::new(Mutex::new(SearXNGClient::new(
        base_url,
        timeout,
        config.default_categories,
        config.local_category_prefixes,
    )));

    Ok(Arc::new(SearXNGExtension { client }))
}

pub struct SearXNGExtension {
    client: Arc<Mutex<SearXNGClient>>,
}

impl TauExtension for SearXNGExtension {
    fn register_tools(&self, builder: &mut tau_client::ToolRegistrar) -> ClientResult<()> {
        builder.register_tool(
            ToolSpec::new(MODEL_VISIBLE_TOOL_NAME)
                .with_description("Search using SearXNG instance")
                .with_category("search")
                .with_parameter("query", "Search query")
                .with_parameter("categories", "SearXNG categories to use")
                .with_parameter("engines", "Specific engines to use")
                .with_parameter("language", "Language code")
                .with_parameter("time_range", "Time filter")
                .with_parameter("num_results", "Maximum results")
                .with_parameter("safe_search", "Safe search level")
                .with_parameter("prefer_local", "Prioritize local-category results")
        );
        Ok(())
    }

    fn execute_tool(
        &self,
        tool_call_id: ToolCallId,
        _tool_name: ToolName,
        arguments: &Value,
    ) -> ClientResult<()> {
        let query = arguments.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ClientError::InvalidArgument("missing query parameter".to_string()))?;

        let options = SearchOptions::from_value(arguments)?;

        let client = Arc::clone(&self.client);
        let client_lock = client.lock().map_err(|e| ClientError::Internal(format!("lock poisoned: {e}")))?;

        let result = client_lock.search(query, &options);

        match result {
            Ok(result_str) => {
                Ok(())
            }
            Err(error) => {
                Err(ClientError::ToolError(ToolError::from(error)))
            }
        }
    }
}
