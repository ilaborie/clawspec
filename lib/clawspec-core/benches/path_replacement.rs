use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

/// Original implementation using format! macro
fn original_replace_path_param(path: &str, param_name: &str, value: &str) -> String {
    path.replace(&format!("{{{param_name}}}"), value)
}

/// Optimized implementation that avoids format! macro allocations
/// while maintaining correctness for exact parameter matching
fn optimized_replace_path_param(path: &str, param_name: &str, value: &str) -> String {
    // Use concat to avoid format! macro allocation, but keep str::replace for correctness
    let pattern = ["{", param_name, "}"].concat();
    path.replace(&pattern, value)
}

fn benchmark_path_replacement(c: &mut Criterion) {
    let mut group = c.benchmark_group("path_replacement");

    let test_cases = [
        ("/users/{id}", "id", "123"),
        ("/users/{user_id}/posts/{post_id}", "user_id", "456"),
        ("/users/{user_id}/posts/{post_id}", "post_id", "hello-world"),
        (
            "/api/{version}/users/{id}/posts/{id}/comments/{version}",
            "id",
            "789",
        ),
        (
            "/api/{version}/users/{id}/posts/{id}/comments/{version}",
            "version",
            "v1",
        ),
        (
            "/search/{query}",
            "query",
            "hello world & special chars @#$%",
        ),
    ];

    for (i, (path, param_name, value)) in test_cases.iter().enumerate() {
        group.bench_function(format!("original_{i}"), |b| {
            b.iter(|| {
                let result = original_replace_path_param(
                    black_box(path),
                    black_box(param_name),
                    black_box(value),
                );
                black_box(result);
            })
        });

        group.bench_function(format!("optimized_{i}"), |b| {
            b.iter(|| {
                let result = optimized_replace_path_param(
                    black_box(path),
                    black_box(param_name),
                    black_box(value),
                );
                black_box(result);
            })
        });
    }

    group.finish();
}

fn benchmark_collection_types(c: &mut Criterion) {
    let mut group = c.benchmark_group("collection_types");

    let names = ["id", "user_id", "post_id", "comment_id", "version", "tag"];

    group.bench_function("vec_retain", |b| {
        b.iter(|| {
            let mut names_vec: Vec<String> = names.iter().map(|s| s.to_string()).collect();
            for name in ["id", "user_id", "post_id"] {
                let len = names_vec.len();
                names_vec.retain(|it| it != name);
                black_box(len != names_vec.len());
            }
            black_box(names_vec);
        })
    });

    group.bench_function("hashset_remove", |b| {
        b.iter(|| {
            let mut names_set: std::collections::HashSet<String> =
                names.iter().map(|s| s.to_string()).collect();
            for name in ["id", "user_id", "post_id"] {
                black_box(names_set.remove(name));
            }
            black_box(names_set);
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_path_replacement,
    benchmark_collection_types
);
criterion_main!(benches);
