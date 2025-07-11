use std::hint::black_box;
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use clawspec_utoipa::{CallPath, ParamValue, ParamStyle};

fn benchmark_path_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("path_creation");
    group.sample_size(50);  // Reduce sample size for faster runs
    
    // Test simple path with single parameter
    group.bench_function("simple_path", |b| {
        b.iter(|| {
            let mut path = CallPath::from("/users/{user_id}");
            path.add_param("user_id", ParamValue::new(black_box(123)));
            black_box(path);
        })
    });
    
    // Test path with multiple parameters
    group.bench_function("multiple_params", |b| {
        b.iter(|| {
            let mut path = CallPath::from("/users/{user_id}/posts/{post_id}/comments/{comment_id}");
            path.add_param("user_id", ParamValue::new(black_box(123)));
            path.add_param("post_id", ParamValue::new(black_box("hello-world")));
            path.add_param("comment_id", ParamValue::new(black_box(456)));
            black_box(path);
        })
    });
    
    // Test path with duplicate parameters
    group.bench_function("duplicate_params", |b| {
        b.iter(|| {
            let mut path = CallPath::from("/api/{version}/users/{id}/posts/{id}/comments/{version}");
            path.add_param("version", ParamValue::new(black_box("v1")));
            path.add_param("id", ParamValue::new(black_box(123)));
            black_box(path);
        })
    });
    
    // Test path with array parameters
    group.bench_function("array_params", |b| {
        b.iter(|| {
            let mut path = CallPath::from("/search/{tags}");
            path.add_param("tags", ParamValue::with_style(
                black_box(vec!["rust", "web", "api", "performance", "optimization"]),
                ParamStyle::Simple
            ));
            black_box(path);
        })
    });
    
    // Test path with special characters requiring encoding
    group.bench_function("special_chars", |b| {
        b.iter(|| {
            let mut path = CallPath::from("/search/{query}");
            path.add_param("query", ParamValue::new(black_box("hello world & more @#$%")));
            black_box(path);
        })
    });
    
    group.finish();
}

fn benchmark_path_parameter_styles(c: &mut Criterion) {
    let mut group = c.benchmark_group("parameter_styles");
    
    let test_array = vec!["rust", "web", "api", "performance", "optimization"];
    
    for style in [ParamStyle::Simple, ParamStyle::SpaceDelimited, ParamStyle::PipeDelimited] {
        group.bench_with_input(
            BenchmarkId::new("array_style", format!("{:?}", style)),
            &style,
            |b, &style| {
                b.iter(|| {
                    let mut path = CallPath::from("/search/{tags}");
                    path.add_param("tags", ParamValue::with_style(
                        black_box(test_array.clone()),
                        style
                    ));
                    black_box(path);
                })
            },
        );
    }
    
    group.finish();
}

fn benchmark_regex_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("regex_operations");
    
    let test_paths = vec![
        "/users/{user_id}",
        "/users/{user_id}/posts/{post_id}",
        "/users/{user_id}/posts/{post_id}/comments/{comment_id}",
        "/api/{version}/users/{id}/posts/{id}/comments/{version}",
        "/complex/{param1}/path/{param2}/with/{param3}/many/{param4}/parameters/{param5}",
    ];
    
    for path in test_paths {
        group.bench_with_input(
            BenchmarkId::new("regex_parsing", path.len()),
            &path,
            |b, &path| {
                b.iter(|| {
                    let call_path = CallPath::from(black_box(path));
                    // The regex operation happens during path resolution
                    let _display = format!("{}", call_path);
                })
            },
        );
    }
    
    group.finish();
}

fn benchmark_string_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_operations");
    
    // Benchmark the current string replacement approach
    group.bench_function("current_replacement", |b| {
        b.iter(|| {
            let mut path = String::from("/users/{user_id}/posts/{post_id}");
            let params = vec![("user_id", "123"), ("post_id", "hello-world")];
            
            for (name, value) in params {
                path = path.replace(&format!("{{{name}}}"), black_box(value));
            }
            black_box(path);
        })
    });
    
    // Benchmark URL encoding operations
    group.bench_function("url_encoding", |b| {
        let test_values = vec![
            "simple",
            "hello world",
            "user@example.com",
            "path/with/slashes",
            "special@#$%^&*()chars",
            "very long string with many special characters that need encoding @#$%^&*()",
        ];
        
        b.iter(|| {
            for value in &test_values {
                let _encoded = percent_encoding::utf8_percent_encode(
                    black_box(value),
                    percent_encoding::NON_ALPHANUMERIC
                ).to_string();
            }
        })
    });
    
    group.finish();
}

criterion_group!(
    benches,
    benchmark_path_creation,
    benchmark_path_parameter_styles,
    benchmark_regex_operations,
    benchmark_string_operations
);
criterion_main!(benches);