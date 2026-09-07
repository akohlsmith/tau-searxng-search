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
fn test_parse_search_response_tags_combined() {
    let base_url = Url::parse("http://localhost:8080").unwrap();
    let client = SearXNGClient::new(base_url, Duration::from_secs(15), vec![], vec![]);

    let body = json!({
        "query": { "query": "test query" },
        "results": [
            {
                "title": "Both fields present",
                "url": "https://example.com/1",
                "content": "Snippet",
                "engines": ["google"],
                "score": 0.8,
                "karakeep_tags": ["rust", "agent"],
                "tags": ["important"]
            },
            {
                "title": "Only karakeep_tags",
                "url": "https://example.com/2",
                "content": "Snippet",
                "engines": ["google"],
                "score": 0.5,
                "karakeep_tags": ["only_kara"]
            },
            {
                "title": "Only tags",
                "url": "https://example.com/3",
                "content": "Snippet",
                "engines": ["google"],
                "score": 0.4,
                "tags": ["only_tags"]
            },
            {
                "title": "karakeep_tags null, tags present",
                "url": "https://example.com/4",
                "content": "Snippet",
                "engines": ["google"],
                "score": 0.3,
                "karakeep_tags": null,
                "tags": ["tag2"]
            },
            {
                "title": "Both empty arrays",
                "url": "https://example.com/5",
                "content": "Snippet",
                "engines": ["google"],
                "score": 0.2,
                "karakeep_tags": [],
                "tags": []
            },
            {
                "title": "No tags at all",
                "url": "https://example.com/6",
                "content": "Snippet",
                "engines": ["google"],
                "score": 0.1
            }
        ]
    })
    .to_string();

    let output = client.parse_search_response(&body, false);

    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let results = parsed.get("results").unwrap().as_array().unwrap();

    // Result 1: both tags present -> merged into single tags field
    let result1 = &results[0];
    assert!(result1.get("karakeep_tags").is_none(), "karakeep_tags field should not exist");
    assert!(result1.get("tags").is_some());
    assert_eq!(
        result1["tags"].as_array().unwrap(),
        &["rust", "agent", "important"]
    );

    // Result 2: only karakeep_tags -> renamed to tags
    let result2 = &results[1];
    assert!(result2.get("karakeep_tags").is_none());
    assert!(result2.get("tags").is_some());
    assert_eq!(
        result2["tags"].as_array().unwrap(),
        &["only_kara"]
    );

    // Result 3: only tags -> preserved
    let result3 = &results[2];
    assert!(result3.get("karakeep_tags").is_none());
    assert!(result3.get("tags").is_some());
    assert_eq!(
        result3["tags"].as_array().unwrap(),
        &["only_tags"]
    );

    // Result 4: karakeep_tags is null, tags preserved
    let result4 = &results[3];
    assert!(result4.get("karakeep_tags").is_none());
    assert!(result4.get("tags").is_some());
    assert_eq!(
        result4["tags"].as_array().unwrap(),
        &["tag2"]
    );

    // Result 5: both empty arrays -> tags field omitted
    let result5 = &results[4];
    assert!(result5.get("karakeep_tags").is_none());
    assert!(result5.get("tags").is_none());

    // Result 6: no tags fields -> omitted
    let result6 = &results[5];
    assert!(result6.get("karakeep_tags").is_none());
    assert!(result6.get("tags").is_none());
}
