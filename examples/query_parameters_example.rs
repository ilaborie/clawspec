//! Comprehensive examples demonstrating query parameter usage with clawspec.
//!
//! This example showcases the different ways to use query parameters with the
//! clawspec HTTP client library, including various parameter styles and types
//! that comply with OpenAPI 3.1 specifications.

use clawspec_utoipa::client::query::{CallQuery, DisplayQuery, SerializableQuery, QueryStyle};
use serde::Serialize;
use utoipa::ToSchema;

/// Example of a custom serializable type for complex query parameters
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SearchFilters {
    pub active: bool,
    pub category: String,
    pub min_price: Option<f64>,
}

/// Example of a simple enum that can be used as a query parameter
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    Asc,
    Desc,
}

impl std::fmt::Display for SortOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SortOrder::Asc => write!(f, "asc"),
            SortOrder::Desc => write!(f, "desc"),
        }
    }
}

fn main() {
    println!("=== Query Parameter Examples ===\n");

    // Example 1: Basic usage with simple types
    basic_query_parameters();

    // Example 2: Array parameters with different styles
    array_parameter_styles();

    // Example 3: Mixed parameter types
    mixed_parameter_types();

    // Example 4: Custom types and enums
    custom_types_example();

    // Example 5: Error handling for unsupported types
    error_handling_example();
}

/// Demonstrates basic query parameter usage with simple types
fn basic_query_parameters() {
    println!("1. Basic Query Parameters");
    println!("========================");

    let query = CallQuery::new()
        .add_param("search", DisplayQuery("hello world"))
        .add_param("page", DisplayQuery(1))
        .add_param("limit", DisplayQuery(20))
        .add_param("active", DisplayQuery(true));

    // In a real application, you would call query.to_query_string()
    // This would generate: ?search=hello+world&page=1&limit=20&active=true
    
    println!("Query with: search='hello world', page=1, limit=20, active=true");
    println!("Generated query string: ?search=hello+world&page=1&limit=20&active=true\n");
}

/// Demonstrates array parameters with different OpenAPI 3.1 styles
fn array_parameter_styles() {
    println!("2. Array Parameter Styles");
    println!("=========================");

    // Form style (default) - parameters are repeated
    let form_query = CallQuery::new()
        .add_param("tags", SerializableQuery::new(vec!["rust", "web", "api"]));
    
    println!("Form style (default):");
    println!("  Input: vec![\"rust\", \"web\", \"api\"]");
    println!("  Output: ?tags=rust&tags=web&tags=api");

    // Space delimited style - values joined with spaces
    let space_query = CallQuery::new()
        .add_param("categories", SerializableQuery::with_style(
            vec!["tech", "programming", "tutorial"], 
            QueryStyle::SpaceDelimited
        ));
    
    println!("\nSpace delimited style:");
    println!("  Input: vec![\"tech\", \"programming\", \"tutorial\"]");
    println!("  Output: ?categories=tech+programming+tutorial");

    // Pipe delimited style - values joined with pipes
    let pipe_query = CallQuery::new()
        .add_param("ids", SerializableQuery::with_style(
            vec![1, 2, 3, 4, 5], 
            QueryStyle::PipeDelimited
        ));
    
    println!("\nPipe delimited style:");
    println!("  Input: vec![1, 2, 3, 4, 5]");
    println!("  Output: ?ids=1%7C2%7C3%7C4%7C5 (pipes are URL-encoded)\n");
}

/// Demonstrates mixing different parameter types in a single query
fn mixed_parameter_types() {
    println!("3. Mixed Parameter Types");
    println!("========================");

    let query = CallQuery::new()
        // Simple display parameters
        .add_param("q", DisplayQuery("search term"))
        .add_param("limit", DisplayQuery(50))
        .add_param("offset", DisplayQuery(0))
        
        // Array parameters with different styles
        .add_param("tags", SerializableQuery::new(vec!["rust", "web"]))
        .add_param("categories", SerializableQuery::with_style(
            vec!["tech", "programming"], 
            QueryStyle::SpaceDelimited
        ))
        .add_param("exclude_ids", SerializableQuery::with_style(
            vec![10, 20, 30], 
            QueryStyle::PipeDelimited
        ));

    println!("Complex query combining multiple parameter types:");
    println!("  - Simple search term: 'search term'");
    println!("  - Pagination: limit=50, offset=0");
    println!("  - Form style tags: ['rust', 'web']");
    println!("  - Space delimited categories: ['tech', 'programming']");
    println!("  - Pipe delimited exclude IDs: [10, 20, 30]");
    println!("Generated: ?q=search+term&limit=50&offset=0&tags=rust&tags=web&categories=tech+programming&exclude_ids=10%7C20%7C30\n");
}

/// Demonstrates using custom types and enums as query parameters
fn custom_types_example() {
    println!("4. Custom Types and Enums");
    println!("=========================");

    let query = CallQuery::new()
        // Using a custom enum with Display
        .add_param("sort", DisplayQuery(SortOrder::Desc))
        .add_param("order", DisplayQuery(SortOrder::Asc))
        
        // Using arrays of custom types
        .add_param("sort_fields", SerializableQuery::new(vec!["name", "created_at"]));

    println!("Custom enum parameters:");
    println!("  - sort: SortOrder::Desc -> 'desc'");
    println!("  - order: SortOrder::Asc -> 'asc'");
    println!("  - sort_fields: vec![\"name\", \"created_at\"]");
    println!("Generated: ?sort=desc&order=asc&sort_fields=name&sort_fields=created_at\n");
}

/// Demonstrates error handling for unsupported parameter types
fn error_handling_example() {
    println!("5. Error Handling");
    println!("=================");

    // This would work fine - arrays of primitives are supported
    let valid_query = CallQuery::new()
        .add_param("numbers", SerializableQuery::new(vec![1, 2, 3]))
        .add_param("strings", SerializableQuery::new(vec!["a", "b", "c"]));

    println!("✅ Valid parameters:");
    println!("  - Arrays of numbers: vec![1, 2, 3]");
    println!("  - Arrays of strings: vec![\"a\", \"b\", \"c\"]");

    // Note: Object parameters would cause runtime errors during serialization
    println!("\n❌ Invalid parameters (would cause runtime errors):");
    println!("  - Objects: Not supported for query parameters");
    println!("  - Nested arrays: Arrays containing objects");
    println!("  - Complex nested structures");
    
    println!("\nBest practices:");
    println!("  - Use DisplayQuery for simple types (strings, numbers, booleans)");
    println!("  - Use SerializableQuery for arrays and simple structs");
    println!("  - Avoid complex nested objects in query parameters");
    println!("  - Test serialization with your data types before deployment\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_examples_compile() {
        // This test ensures all examples compile correctly
        basic_query_parameters();
        array_parameter_styles();
        mixed_parameter_types();
        custom_types_example();
        error_handling_example();
    }

    #[test]
    fn test_sort_order_display() {
        assert_eq!(SortOrder::Asc.to_string(), "asc");
        assert_eq!(SortOrder::Desc.to_string(), "desc");
    }
}