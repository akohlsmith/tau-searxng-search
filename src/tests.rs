//! Tests for SearXNG search extension.

use crate::options::SearchOptions;
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
