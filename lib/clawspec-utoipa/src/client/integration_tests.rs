//! Integration tests for query parameter functionality
//!
//! These tests verify the complete integration of query parameters
//! from creation through URL serialization and OpenAPI generation.

use super::{CallQuery, DisplayQuery, QueryStyle, SerializableQuery};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_parameter_full_integration() {
        // Simulate a real-world API query with multiple parameter types
        let query = CallQuery::new()
            .add_param("search", DisplayQuery("machine learning"))
            .add_param("page", DisplayQuery(1))
            .add_param("per_page", DisplayQuery(20))
            .add_param("active", DisplayQuery(true))
            .add_param("tags", SerializableQuery::new(vec!["ai", "ml", "data"]))
            .add_param(
                "categories",
                SerializableQuery::with_style(
                    vec!["research", "applications"],
                    QueryStyle::SpaceDelimited,
                ),
            )
            .add_param(
                "include_fields",
                SerializableQuery::with_style(
                    vec!["title", "summary", "author"],
                    QueryStyle::PipeDelimited,
                ),
            );

        // Test URL serialization
        let query_string = query
            .to_query_string()
            .expect("Query serialization should succeed");

        insta::assert_debug_snapshot!(query_string, @r#""search=machine+learning&page=1&per_page=20&active=true&tags=ai&tags=ml&tags=data&categories=research+applications&include_fields=title%7Csummary%7Cauthor""#);

        // Test OpenAPI parameter generation
        let parameters: Vec<_> = query.to_parameters().collect();
        assert_eq!(parameters.len(), 7);

        // Create a summary for snapshot testing
        let param_summary: Vec<_> = parameters
            .iter()
            .map(|p| format!("{}: {:?}", p.name, p.style.as_ref().unwrap()))
            .collect();

        insta::assert_debug_snapshot!(param_summary, @r#"
        [
            "search: Form",
            "page: Form",
            "per_page: Form",
            "active: Form",
            "tags: Form",
            "categories: SpaceDelimited",
            "include_fields: PipeDelimited",
        ]
        "#);
    }

    #[test]
    fn test_edge_cases_comprehensive() {
        // Test various edge cases
        let query = CallQuery::new()
            .add_param("empty_string", DisplayQuery(""))
            .add_param("zero", DisplayQuery(0))
            .add_param("false_bool", DisplayQuery(false))
            .add_param(
                "null_value",
                SerializableQuery::new(serde_json::Value::Null),
            )
            .add_param("empty_array", SerializableQuery::new(Vec::<String>::new()))
            .add_param("single_item_array", SerializableQuery::new(vec!["single"]));

        let query_string = query
            .to_query_string()
            .expect("Edge case serialization should succeed");

        insta::assert_debug_snapshot!(query_string, @r#""empty_string=&zero=0&false_bool=false&null_value=&single_item_array=single""#);
    }

    #[test]
    fn test_url_encoding_compliance() {
        // Test characters that require URL encoding
        let query = CallQuery::new()
            .add_param("spaces", DisplayQuery("hello world"))
            .add_param("special_chars", DisplayQuery("a&b=c?d#e"))
            .add_param("unicode", DisplayQuery("José's café"))
            .add_param("reserved", DisplayQuery("100% guaranteed!"))
            .add_param(
                "mixed_array",
                SerializableQuery::new(vec!["a&b", "c=d", "e?f"]),
            );

        let query_string = query
            .to_query_string()
            .expect("URL encoding test should succeed");

        insta::assert_debug_snapshot!(query_string, @r#""spaces=hello+world&special_chars=a%26b%3Dc%3Fd%23e&unicode=Jos%C3%A9%27s+caf%C3%A9&reserved=100%25+guaranteed%21&mixed_array=a%26b&mixed_array=c%3Dd&mixed_array=e%3Ff""#);
    }

    #[test]
    fn test_different_array_styles_side_by_side() {
        let items = vec!["apple", "banana", "cherry"];
        // Same data with different styles
        let query = CallQuery::new()
            .add_param("form_style", SerializableQuery::new(items.clone()))
            .add_param(
                "space_style",
                SerializableQuery::with_style(items.clone(), QueryStyle::SpaceDelimited),
            )
            .add_param(
                "pipe_style",
                SerializableQuery::with_style(items.clone(), QueryStyle::PipeDelimited),
            );

        let query_string = query
            .to_query_string()
            .expect("Array styles test should succeed");

        insta::assert_debug_snapshot!(query_string, @r#""form_style=apple&form_style=banana&form_style=cherry&space_style=apple+banana+cherry&pipe_style=apple%7Cbanana%7Ccherry""#);
    }

    #[test]
    fn test_complex_nested_data_rejection() {
        use serde_json::json;

        // This should work - simple values
        let query = CallQuery::new()
            .add_param("simple", SerializableQuery::new("hello"))
            .add_param("number", SerializableQuery::new(42))
            // This should fail - nested object
            .add_param(
                "complex",
                SerializableQuery::new(json!({
                    "nested": {
                        "data": "value"
                    }
                })),
            );

        let result = query.to_query_string();
        assert!(result.is_err(), "Complex nested objects should be rejected");

        // Verify the error type
        match result {
            Err(crate::client::ApiClientError::UnsupportedQueryParameterValue { .. }) => {
                // Expected error type
            }
            _ => panic!("Expected UnsupportedQueryParameterValue error"),
        }
    }
}
