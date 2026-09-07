//! Search options parsing for SearXNG search tool.

use serde_json::Value;
use tau_client::ClientError;

#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub categories: Option<Vec<String>>,
    pub engines: Option<Vec<String>>,
    pub language: Option<String>,
    pub time_range: Option<String>,
    pub num_results: Option<u32>,
    pub safe_search: Option<u8>,
    pub prefer_local: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            categories: None,
            engines: None,
            language: None,
            time_range: None,
            num_results: None,
            safe_search: None,
            prefer_local: false,
        }
    }
}

impl SearchOptions {
    pub fn from_value(value: &Value) -> Result<Self, ClientError> {
        let categories = extract_string_list(value, "categories");
        let engines = extract_string_list(value, "engines");
        let language = extract_string(value, "language");
        let time_range = extract_string(value, "time_range");
        let num_results = extract_u32(value, "num_results");
        let safe_search = extract_safe_search(value);
        let prefer_local = extract_bool(value, "prefer_local").unwrap_or(false);

        Ok(Self {
            categories,
            engines,
            language,
            time_range,
            num_results,
            safe_search,
            prefer_local,
        })
    }
}

fn extract_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

fn extract_string_list(value: &Value, key: &str) -> Option<Vec<String>> {
    value.get(key).and_then(|v| match v {
        Value::Array(arr) => {
            let result: Vec<String> = arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
            Some(result)
        },
        _ => None,
    })
}

fn extract_u32(value: &Value, key: &str) -> Option<u32> {
    value.get(key).and_then(|v| v.as_u64()).map(|v| v as u32)
}

fn extract_safe_search(value: &Value) -> Option<u8> {
    value.get("safe_search").and_then(|v| v.as_u64()).map(|v| match v {
        0 | 1 | 2 => v as u8,
        _ => 0,
    })
}

fn extract_bool(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(|v| v.as_bool())
}
