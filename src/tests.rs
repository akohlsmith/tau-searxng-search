//! Tests for SearXNG search extension.

use crate::{options::SearchOptions, SearXNGClient};
use std::time::Duration;
use url::Url;
use serde_json::json;

#[test]
fn test_search_options_from_value_basic() {
    let value = json!({
        "query": "test query",
        "categories": ["local"],
        "num_results": 5,
        "prefer_local": true
    });

    let options = SearchOptions::from_value(&value).unwrap();
    assert_eq!(options.categories, Some(vec!["local".to_string()]));
    assert_eq!(options.num_results, Some(5));
    assert!(options.prefer_local);
}

#[test]
fn test_search_options_default() {
    let options = SearchOptions::default();
    assert!(options.categories.is_none());
    assert!(options.engines.is_none());
    assert!(!options.prefer_local);
}

#[test]
fn test_search_options_prefers_local_false_by_default() {
    let value = json!({
        "query": "test"
    });

    let options = SearchOptions::from_value(&value).unwrap();
    assert!(!options.prefer_local);
}

#[test]
fn test_search_options_safe_search() {
    let value = json!({
        "query": "test",
        "safe_search": 1
    });

    let options = SearchOptions::from_value(&value).unwrap();
    assert_eq!(options.safe_search, Some(1));
}

#[test]
fn test_parse_search_response_tags_extraction() {
    let base_url = Url::parse("http://localhost:8080").unwrap();
    let client = SearXNGClient::new(base_url, Duration::from_secs(15), vec![], vec![]);

    let body = json!({
        "query": { "query": "test query" },
        "results": [
            {
                "title": "Test Result 1",
                "url": "https://example.com/1",
                "content": "Snippet",
                "engines": ["google"],
                "score": 0.8,
                "karakeep_tags": ["rust", "agent"],
                "tags": ["important"]
            },
            {
                "title": "Test Result 2",
                "url": "https://example.com/2",
                "content": "Snippet",
                "engines": ["google"],
                "score": 0.5,
                "karakeep_tags": null,
                "tags": ["tag2"]
            },
            {
                "title": "Test Result 3",
                "url": "https://example.com/3",
                "content": "Snippet",
                "engines": ["google"],
                "score": 0.3,
                "karakeep_tags": [],
                "tags": []
            }
        ]
    })
    .to_string();

    let output = client.parse_search_response(&body, false);

    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let results = parsed.get("results").unwrap().as_array().unwrap();

    // Result 1: both tags present
    let result1 = &results[0];
    assert!(result1.get("karakeep_tags").is_some());
    assert_eq!(
        result1["karakeep_tags"].as_array().unwrap(),
        &["rust", "agent"]
    );
    assert!(result1.get("tags").is_some());
    assert_eq!(
        result1["tags"].as_array().unwrap(),
        &["important"]
    );

    // Result 2: karakeep_tags is null, omitted; tags preserved
    let result2 = &results[1];
    assert!(result2.get("karakeep_tags").is_none());
    assert!(result2.get("tags").is_some());
    assert_eq!(
        result2["tags"].as_array().unwrap(),
        &["tag2"]
    );

    // Result 3: both are empty arrays, both omitted
    let result3 = &results[2];
    assert!(result3.get("karakeep_tags").is_none());
    assert!(result3.get("tags").is_none());
}
