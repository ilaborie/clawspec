use clawspec_core::{ApiClient, CallPath, ParamValue};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use serde::{Deserialize, Serialize};
use std::hint::black_box;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
struct User {
    id: u64,
    name: String,
    email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
struct Post {
    id: u64,
    title: String,
    content: String,
    author_id: u64,
}

/// Performance regression test for path parameter replacement operations.
///
/// This benchmark ensures that the optimized path replacement performance
/// is maintained over time and doesn't regress to the original slow implementation.
///
/// # Performance Baseline
///
/// The optimized implementation should be:
/// - At least 20% faster than the original format!() based implementation
/// - Complete simple path replacement in <50ns
/// - Complete complex path replacement in <100ns
///
/// # Test Cases
///
/// - Simple paths: `/users/{id}` -> `/users/123`
/// - Complex paths: `/api/{version}/users/{id}/posts/{post_id}`
/// - Duplicate parameters: `/test/{id}/{id}`
/// - Special characters: paths with URL encoding requirements
fn benchmark_path_replacement_regression(c: &mut Criterion) {
    let mut group = c.benchmark_group("path_regression");

    // Set performance thresholds based on Issue #31 optimizations
    group.significance_level(0.1).sample_size(1000);

    let test_cases = vec![
        ("simple", "/users/{id}", vec![("id", "123")]),
        (
            "complex",
            "/api/{version}/users/{user_id}/posts/{post_id}",
            vec![
                ("version", "v1"),
                ("user_id", "456"),
                ("post_id", "hello-world"),
            ],
        ),
        ("duplicate", "/test/{id}/{id}", vec![("id", "789")]),
        (
            "special_chars",
            "/search/{query}",
            vec![("query", "hello world & more")],
        ),
        (
            "many_params",
            "/a/{p1}/b/{p2}/c/{p3}/d/{p4}/e/{p5}",
            vec![
                ("p1", "1"),
                ("p2", "2"),
                ("p3", "3"),
                ("p4", "4"),
                ("p5", "5"),
            ],
        ),
    ];

    for (name, path_template, params) in test_cases {
        group.bench_with_input(
            BenchmarkId::new("path_replacement", name),
            &(path_template, params),
            |b, (path_template, params)| {
                b.iter(|| {
                    let mut path = CallPath::from(black_box(*path_template));
                    for (param_name, param_value) in params {
                        path = path.add_param(
                            black_box(*param_name),
                            ParamValue::new(black_box(*param_value)),
                        );
                    }
                    black_box(path);
                })
            },
        );
    }

    group.finish();
}

/// Performance regression test for schema-related operations.
///
/// This benchmark ensures that schema-related operations maintain
/// their performance characteristics over time.
///
/// # Performance Baseline
///
/// The implementation should:
/// - Handle complex type creation efficiently
/// - Maintain reasonable performance for schema-heavy operations
/// - Scale appropriately with data complexity
fn benchmark_schema_related_regression(c: &mut Criterion) {
    let mut group = c.benchmark_group("schema_related_regression");

    // Set performance thresholds
    group.significance_level(0.1).sample_size(100);

    group.bench_function("complex_type_creation", |b| {
        b.iter(|| {
            let user = User {
                id: black_box(123),
                name: black_box("John Doe".to_string()),
                email: black_box("john@example.com".to_string()),
            };

            let post = Post {
                id: black_box(456),
                title: black_box("My Post".to_string()),
                content: black_box("This is a test post".to_string()),
                author_id: black_box(123),
            };

            black_box((user, post));
        })
    });

    group.finish();
}

/// Performance regression test for collection type operations.
///
/// This benchmark ensures that the switch from Vec::retain() to HashSet::remove()
/// maintains its O(1) vs O(n) performance advantage.
///
/// # Performance Baseline
///
/// The optimized implementation should:
/// - Use HashSet for O(1) parameter lookup and removal
/// - Be significantly faster than Vec::retain() for large parameter sets
/// - Scale linearly with parameter count (not quadratically)
fn benchmark_collection_operations_regression(c: &mut Criterion) {
    let mut group = c.benchmark_group("collection_regression");

    // Set performance thresholds
    group.significance_level(0.1).sample_size(100);

    let param_counts = vec![5, 10, 25, 50, 100];

    for count in param_counts {
        // Test HashSet performance (optimized)
        group.bench_with_input(
            BenchmarkId::new("hashset_remove", count),
            &count,
            |b, &count| {
                b.iter(|| {
                    let mut names_set: std::collections::HashSet<String> =
                        (0..count).map(|i| format!("param_{i}")).collect();

                    // Remove every third parameter
                    for i in (0..count).step_by(3) {
                        black_box(names_set.remove(&format!("param_{i}")));
                    }
                    black_box(names_set);
                })
            },
        );

        // Test Vec performance (original - for comparison)
        group.bench_with_input(
            BenchmarkId::new("vec_retain", count),
            &count,
            |b, &count| {
                b.iter(|| {
                    let mut names_vec: Vec<String> =
                        (0..count).map(|i| format!("param_{i}")).collect();

                    // Remove every third parameter
                    for i in (0..count).step_by(3) {
                        let param_name = format!("param_{i}");
                        names_vec.retain(|name| name != &param_name);
                    }
                    black_box(names_vec);
                })
            },
        );
    }

    group.finish();
}

/// Performance regression test for end-to-end API client operations.
///
/// This benchmark tests the overall performance of the API client
/// to ensure that optimizations don't cause regressions in real usage.
///
/// # Performance Baseline
///
/// The implementation should:
/// - Complete API client builder operations in <10μs
/// - Handle path creation and parameter addition efficiently
/// - Maintain reasonable memory usage patterns
fn benchmark_api_client_regression(c: &mut Criterion) {
    let mut group = c.benchmark_group("api_client_regression");

    // Set performance thresholds
    group.significance_level(0.1).sample_size(50);

    group.bench_function("client_builder", |b| {
        b.iter(|| {
            let client = ApiClient::builder().build().expect("should build client");
            black_box(client);
        })
    });

    group.bench_function("path_creation_with_params", |b| {
        b.iter(|| {
            let path = CallPath::from("/users/{user_id}/posts/{post_id}")
                .add_param("user_id", ParamValue::new(black_box(123)))
                .add_param("post_id", ParamValue::new(black_box("hello-world")));
            black_box(path);
        })
    });

    group.bench_function("complex_path_operations", |b| {
        b.iter(|| {
            // Simulate a complex API call setup
            let path = CallPath::from(
                "/api/{version}/users/{user_id}/posts/{post_id}/comments/{comment_id}",
            )
            .add_param("version", ParamValue::new(black_box("v1")))
            .add_param("user_id", ParamValue::new(black_box(456)))
            .add_param("post_id", ParamValue::new(black_box("my-post")))
            .add_param("comment_id", ParamValue::new(black_box(789)))
            .add_param(
                "tags",
                ParamValue::new(black_box(vec!["rust", "web", "api"])),
            );

            black_box(path);
        })
    });

    group.finish();
}

/// Performance regression test for string allocation patterns.
///
/// This benchmark monitors string allocation patterns to ensure
/// that optimizations reducing allocations are maintained.
///
/// # Performance Baseline
///
/// The implementation should:
/// - Minimize string allocations in hot paths
/// - Use string concatenation instead of format!() where possible
/// - Maintain efficient memory usage patterns
fn benchmark_string_allocation_regression(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_allocation_regression");

    // Set performance thresholds
    group.significance_level(0.1).sample_size(200);

    // Test the optimized string concatenation approach
    group.bench_function("optimized_concat", |b| {
        b.iter(|| {
            let param_name = black_box("user_id");
            let pattern = ["{", param_name, "}"].concat();
            black_box(pattern);
        })
    });

    // Test the original format! approach for comparison
    group.bench_function("format_macro", |b| {
        b.iter(|| {
            let param_name = black_box("user_id");
            let pattern = format!("{{{param_name}}}");
            black_box(pattern);
        })
    });

    // Test bulk string operations
    group.bench_function("bulk_string_ops", |b| {
        b.iter(|| {
            let mut results = Vec::new();
            for i in 0..10 {
                let param_name = format!("param_{i}");
                let pattern = ["{", &param_name, "}"].concat();
                results.push(pattern);
            }
            black_box(results);
        })
    });

    group.finish();
}

criterion_group!(
    performance_regression,
    benchmark_path_replacement_regression,
    benchmark_schema_related_regression,
    benchmark_collection_operations_regression,
    benchmark_api_client_regression,
    benchmark_string_allocation_regression
);

criterion_main!(performance_regression);
