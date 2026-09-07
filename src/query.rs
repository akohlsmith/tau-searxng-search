//! Query construction and response parsing for SearXNG.

pub fn build_query_url(base_url: &url::Url, query: &str, categories: &[String], engines: &[String], language: Option<&str>, time_range: Option<&str>, safe_search: Option<u8>) -> String {
    let mut url_builder = base_url.join("search/").unwrap();

    let mut params = url_builder.query_pairs_mut();
    params.append_pair("q", query);
    params.append_pair("format", "json");

    if let Some(lang) = language {
        params.append_pair("language", lang);
    }

    if let Some(time_range) = time_range {
        params.append_pair("time_range", time_range);
    }

    if let Some(safe_search) = safe_search {
        params.append_pair("safesearch", &safe_search.to_string());
    }

    if !categories.is_empty() {
        let categories_str = categories.join(",");
        params.append_pair("categories", &categories_str);
    }

    if !engines.is_empty() {
        let engines_str = engines.join(",");
        params.append_pair("engines", &engines_str);
    }

    url_builder.to_string()
}

pub fn parse_response_body(body: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(body).map_err(|e| format!("failed to parse response JSON: {e}"))
}

pub fn extract_results(parsed: &serde_json::Value) -> Vec<serde_json::Value> {
    parsed.get("results")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}
