//! SearXNG search extension for Tau.
//!
//! Registers `searxng_search` tool that queries a SearXNG instance via HTTP
//! API with configurable engines, categories, and local-category prioritization.

use std::io::{Read, Write};
use std::time::Duration;

use serde::Deserialize;
use serde_json::json;
use tau_client::{ClientError, ClientResult, ExtensionBuilder, TauExtension, TauExtensionRunner, ToolContext};
use tau_proto::{CborValue, ToolName, ToolSpec, ToolType};
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


/// Maximum results to request from SearXNG.
const DEFAULT_NUM_RESULTS: u32 = 10;

/// Maximum results allowed from SearXNG.
const MAX_NUM_RESULTS: u32 = 100;

mod options;
#[cfg(test)]
mod tests;

use options::SearchOptions;

#[derive(Clone)]
pub struct SearXNGClient {
    pub base_url: Url,
    pub timeout: Duration,
    pub default_categories: Vec<String>,
    pub local_category_prefixes: Vec<String>,
}

impl SearXNGClient {
    pub fn new(base_url: Url, timeout: Duration, default_categories: Vec<String>, local_category_prefixes: Vec<String>) -> Self {
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

        let categories_str = if categories.is_empty() {
            self.default_categories.join(",")
        } else {
            categories.join(",")
        };

        let engines_str = match &options.engines {
            Some(engines) if !engines.is_empty() => Some(engines.join(",")),
            _ => None,
        };

        let request_url = self.build_search_url(
            query,
            &categories_str,
            engines_str.as_deref(),
            options.language.as_deref(),
            options.time_range.as_deref(),
            options.safe_search,
            capped_num_results as u32,
        );

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

    fn build_search_url(
        &self,
        query: &str,
        categories: &str,
        engines: Option<&str>,
        language: Option<&str>,
        time_range: Option<&str>,
        safe_search: Option<u8>,
        _num_results: u32,
    ) -> String {
        let mut url_builder = self.base_url
            .join("search")
            .unwrap();

        let mut params = url_builder.query_pairs_mut();
        params.append_pair("q", query);
        params.append_pair("format", "json");
        params.append_pair("categories", categories);

        if let Some(engines) = engines {
            params.append_pair("engines", engines);
        }

        if let Some(lang) = language {
            params.append_pair("language", lang);
        }

        if let Some(tr) = time_range {
            params.append_pair("time_range", tr);
        }

        if let Some(safe) = safe_search {
            params.append_pair("safesearch", &safe.to_string());
        }

        drop(params);
        url_builder.to_string()
    }

    fn parse_search_response(&self, body: &str, prefer_local: bool) -> String {
        let parsed: serde_json::Value = match serde_json::from_str(body) {
            Ok(v) => v,
            Err(e) => return format!("failed to parse response: {e}"),
        };

        let results: Vec<&serde_json::Value> = parsed.get("results")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().collect())
            .unwrap_or_default();

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

            // karakeep_tags: included only if present and non-null and non-empty
            let karakeep_tags = result.get("karakeep_tags")
                .and_then(|v| {
                    if v.is_null() || v.is_array().then_some(v.as_array().unwrap().is_empty()).unwrap_or(false) {
                        None
                    } else {
                        Some(v.clone())
                    }
                });

            // tags: included only if present and non-null and non-empty
            let tags = result.get("tags")
                .and_then(|v| {
                    if v.is_null() || v.is_array().then_some(v.as_array().unwrap().is_empty()).unwrap_or(false) {
                        None
                    } else {
                        Some(v.clone())
                    }
                });

            let is_local = self.is_local_result(result);

            let mut result_obj = json!({
                "title": title,
                "url": url,
                "content": content,
                "source_engine": source_engine,
                "score": score,
                "language": language,
                "published_date": published_date,
                "_is_local": is_local
            });

            if let Some(t) = karakeep_tags {
                result_obj["karakeep_tags"] = t;
            }
            if let Some(t) = tags {
                result_obj["tags"] = t;
            }

            normalized_results.push(result_obj);
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
            .unwrap_or_else(|_| String::from("\"search completed\""));

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

#[derive(Debug, Deserialize)]
struct Config {
    #[serde(default)]
    base_url: String,
    #[serde(default = "default_timeout")]
    timeout_seconds: u64,
    #[serde(default)]
    default_categories: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
    fallback_urls: Vec<String>,
    #[serde(default = "default_local_category_prefixes")]
    local_category_prefixes: Vec<String>,
}



/// Mutable state shared by handlers.
pub struct SearXNGState {
    pub client: SearXNGClient,
}

/// Extension declaration.
pub struct SearXNGExtension;

impl TauExtension for SearXNGExtension {
    type State = SearXNGState;

    fn name(&self) -> &'static str {
        "std-searxng-search"
    }

    fn register(self, builder: &mut ExtensionBuilder<Self::State>) {
        builder
            .configure::<Config>(|cx| {
                let config = &cx.config;
                if let Err(e) = validate_url(&config.base_url) {
                    return Err(ClientError::handler(e));
                }
                let base_url = validate_url(&config.base_url)?;
                let timeout = Duration::from_secs(config.timeout_seconds);
                cx.state.client = SearXNGClient::new(
                    base_url,
                    timeout,
                    config.default_categories.clone(),
                    config.local_category_prefixes.clone(),
                );
                tracing::info!(target: LOG_TARGET, "searxng configured");
                Ok(())
            })
            .tool(
                ToolSpec {
                    name: ToolName::new(MODEL_VISIBLE_TOOL_NAME),
                    model_visible_name: None,
                    description: Some("Search using SearXNG instance".into()),
                    tool_type: ToolType::Function,
                    parameters: Some(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "query": {"type": "string", "description": "Search query"},
                            "categories": {"type": "array", "items": {"type": "string"}, "description": "SearXNG categories to use"},
                            "engines": {"type": "array", "items": {"type": "string"}, "description": "Specific engines to use"},
                            "language": {"type": "string", "description": "Language code"},
                            "time_range": {"type": "string", "description": "Time filter"},
                            "num_results": {"type": "integer", "description": "Maximum results"},
                            "safe_search": {"type": "integer", "description": "Safe search level"},
                            "prefer_local": {"type": "boolean", "description": "Prioritize local-category results"}
                        },
                        "required": ["query"]
                    })),
                    format: None,
                    tags: Vec::new(),
                    enabled_by_default: true,
                    background_support: None,
                    examples: Vec::new(),
                },
                handle_search,
            )
            .ready_message("searxng search ready");
    }
}

fn cbor_to_json(value: &CborValue) -> Result<serde_json::Value, String> {
    match value {
        CborValue::Null => Ok(serde_json::Value::Null),
        CborValue::Bool(b) => Ok(serde_json::Value::Bool(*b)),
        CborValue::Integer(i) => {
            let n: i128 = (*i).into();
            if let Ok(n) = i64::try_from(n) {
                Ok(serde_json::Value::Number(n.into()))
            } else if let Ok(n) = u64::try_from(n) {
                Ok(serde_json::Value::Number(n.into()))
            } else {
                Err("integer argument is outside JSON number range".to_owned())
            }
        }
        CborValue::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .ok_or_else(|| "float argument must be finite".to_owned()),
        CborValue::Text(s) => Ok(serde_json::Value::String(s.clone())),
        CborValue::Bytes(_) => Err("byte string arguments are not supported".to_owned()),
        CborValue::Array(items) => items
            .iter()
            .map(cbor_to_json)
            .collect::<Result<Vec<_>, _>>()
            .map(serde_json::Value::Array),
        CborValue::Map(entries) => {
            let mut map = serde_json::Map::new();
            for (key, value) in entries {
                let CborValue::Text(key) = key else {
                    return Err("argument object keys must be strings".to_owned());
                };
                map.insert(key.clone(), cbor_to_json(value)?);
            }
            Ok(serde_json::Value::Object(map))
        }
        CborValue::Tag(_, inner) => cbor_to_json(inner),
        _ => Err("unsupported CBOR argument value".to_owned()),
    }
}

fn handle_search(cx: ToolContext<'_, SearXNGState>) -> ClientResult<()> {
    let invoke = cx.invoke();

    let json_args = cbor_to_json(&invoke.arguments)
        .map_err(|e| ClientError::handler(format!("invalid arguments: {e}")))?;

    let query = json_args.get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ClientError::handler("missing query parameter"))?;

    let options = SearchOptions::from_value(&json_args)?;

    let result = cx.state.client.search(query, &options);

    match result {
        Ok(result_str) => {
            let result = tau_proto::ToolResult {
                call_id: invoke.call_id.clone(),
                tool_name: invoke.tool_name.clone(),
                tool_type: tau_proto::ToolType::Function,
                presentation: Default::default(),
                result: tau_proto::CborValue::Text(result_str),
                provider_content: Vec::new(),
                kind: tau_proto::ToolResultKind::Final,
                display: None,
                originator: invoke.originator.clone(),
            };
            cx.handle().report_tool_result(result)
        }
        Err(error) => {
            let error_event = tau_proto::ToolError {
                call_id: invoke.call_id.clone(),
                tool_name: invoke.tool_name.clone(),
                tool_type: tau_proto::ToolType::Function,
                presentation: Default::default(),
                display: None,
                message: error,
                details: None,
                originator: invoke.originator.clone(),
            };
            cx.handle().report_tool_error(error_event)
        }
    }
}

/// Run the extension over stdio.
pub fn run_stdio() -> Result<(), Box<dyn std::error::Error>> {
    tau_client::init_logging_for(LOG_TARGET);
    run(std::io::stdin(), std::io::stdout())
}

/// Run the extension over the supplied reader/writer pair.
pub fn run<R, W>(reader: R, writer: W) -> Result<(), Box<dyn std::error::Error>>
where
    R: Read + Send + 'static,
    W: Write + Send + 'static,
{
    let state = SearXNGState {
        client: SearXNGClient::new(
            validate_url(DEFAULT_BASE_URL).expect("default URL"),
            REQUEST_TIMEOUT,
            Vec::new(),
            default_local_category_prefixes(),
        ),
    };

    let mut runtime = match TauExtensionRunner::new(SearXNGExtension)
        .start_manual_loop(reader, writer, state)
    {
        Ok(runtime) => runtime,
        Err(ClientError::InitialConfigureRejected) => return Ok(()),
        Err(error) => return Err(Box::new(error)),
    };

    loop {
        match runtime.try_recv()? {
            tau_client::ManualRuntimePoll::Message(message) => {
                match runtime.dispatch_one(message)? {
                    tau_client::DispatchOutcome::Continue => {}
                    tau_client::DispatchOutcome::StopRequested => break,
                    tau_client::DispatchOutcome::Disconnect(_) => break,
                }
            }
            tau_client::ManualRuntimePoll::InputClosed => break,
            tau_client::ManualRuntimePoll::Empty => runtime.wait_for_wake(),
        }
    }

    let _ = runtime.finish_detached();
    Ok(())
}
