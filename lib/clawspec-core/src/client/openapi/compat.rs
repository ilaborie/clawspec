//! Compatibility pass that strips data the older OpenAPI output cannot carry.

use tracing::warn;
use utoipa::openapi::path::{Operation, PathItem};
use utoipa::openapi::response::Response;
use utoipa::openapi::{OpenApi, Paths, RefOr, Server, Tag};

pub(crate) fn iter_operations(path_item: &PathItem) -> impl Iterator<Item = &Operation> {
    [
        path_item.get.as_ref(),
        path_item.put.as_ref(),
        path_item.post.as_ref(),
        path_item.delete.as_ref(),
        path_item.options.as_ref(),
        path_item.head.as_ref(),
        path_item.patch.as_ref(),
        path_item.trace.as_ref(),
        path_item.query.as_ref(),
    ]
    .into_iter()
    .flatten()
    .chain(path_item.additional_operations.values())
}

fn iter_operations_mut(path_item: &mut PathItem) -> impl Iterator<Item = (&str, &mut Operation)> {
    [
        ("get", path_item.get.as_mut()),
        ("put", path_item.put.as_mut()),
        ("post", path_item.post.as_mut()),
        ("delete", path_item.delete.as_mut()),
        ("options", path_item.options.as_mut()),
        ("head", path_item.head.as_mut()),
        ("patch", path_item.patch.as_mut()),
        ("trace", path_item.trace.as_mut()),
        ("query", path_item.query.as_mut()),
    ]
    .into_iter()
    .filter_map(|(method, operation)| operation.map(|operation| (method, operation)))
    .chain(
        path_item
            .additional_operations
            .iter_mut()
            .map(|(method, operation)| (method.as_str(), operation)),
    )
}

pub(in crate::client) fn downgrade_to_31(openapi: &mut OpenApi) {
    drop_server_names(openapi.servers.as_deref_mut(), "servers");
    downgrade_paths_to_31(&mut openapi.paths);
    if let Some(tags) = openapi.tags.as_deref_mut() {
        drop_tag_metadata(tags);
    }
    if let Some(components) = openapi.components.as_mut() {
        drop_response_summaries(&mut components.responses, "components.responses");
    }
}

pub(in crate::client) fn downgrade_paths_to_31(paths: &mut Paths) {
    for (path, path_item) in &mut paths.paths {
        drop_server_names(
            path_item.servers.as_deref_mut(),
            &format!("paths.{path}.servers"),
        );
        for (method, operation) in iter_operations_mut(path_item) {
            let location = format!("paths.{path}.{method}");
            drop_server_names(
                operation.servers.as_deref_mut(),
                &format!("{location}.servers"),
            );
            drop_response_summaries(
                &mut operation.responses.responses,
                &format!("{location}.responses"),
            );
        }
    }
    remove_empty_path_items(paths);
}

fn drop_field<T>(value: &mut Option<T>, location: &str, field: &str) {
    if value.take().is_some() {
        warn!(%location, %field, "dropping field not supported by OpenAPI 3.1");
    }
}

fn drop_server_names(servers: Option<&mut [Server]>, location: &str) {
    for (index, server) in servers.into_iter().flatten().enumerate() {
        drop_field(&mut server.name, &format!("{location}[{index}]"), "name");
    }
}

fn drop_tag_metadata(tags: &mut [Tag]) {
    for tag in tags {
        let location = format!("tags.{}", tag.name);
        drop_field(&mut tag.summary, &location, "summary");
        drop_field(&mut tag.parent, &location, "parent");
        drop_field(&mut tag.kind, &location, "kind");
    }
}

fn drop_response_summaries<'a>(
    responses: impl IntoIterator<Item = (&'a String, &'a mut RefOr<Response>)>,
    location: &str,
) {
    for (status, response) in responses {
        if let RefOr::T(response) = response {
            drop_field(
                &mut response.summary,
                &format!("{location}.{status}"),
                "summary",
            );
        }
    }
}

fn remove_empty_path_items(paths: &mut Paths) {
    paths.paths.retain(|path, path_item| {
        let has_operation = iter_operations(path_item).next().is_some();
        if !has_operation {
            warn!(location = %format!("paths.{path}"), "dropping path item without operation");
        }
        has_operation
    });
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;
    use utoipa::openapi::path::{HttpMethod, OperationBuilder, PathItemBuilder};
    use utoipa::openapi::response::{ResponseBuilder, ResponsesBuilder};
    use utoipa::openapi::tag::TagBuilder;
    use utoipa::openapi::{
        ComponentsBuilder, InfoBuilder, OpenApiBuilder, OpenApiVersion, PathsBuilder, ServerBuilder,
    };

    use super::*;

    fn named_server(url: &str, name: &str) -> Server {
        ServerBuilder::new().url(url).name(Some(name)).build()
    }

    fn summarized_response() -> Response {
        ResponseBuilder::new()
            .summary(Some("Found"))
            .description("The resource")
            .build()
    }

    fn operation() -> Operation {
        OperationBuilder::new()
            .servers(Some(vec![named_server("https://op.example.com", "op")]))
            .responses(
                ResponsesBuilder::new()
                    .response("200", summarized_response())
                    .build(),
            )
            .build()
    }

    fn openapi(paths: Paths) -> OpenApi {
        OpenApiBuilder::new()
            .openapi(OpenApiVersion::Version31)
            .info(InfoBuilder::new().title("test").version("1.0.0").build())
            .paths(paths)
            .build()
    }

    fn to_yaml(openapi: &OpenApi) -> String {
        serde_saphyr::to_string(openapi).expect("should serialize to YAML")
    }

    #[test]
    fn should_iterate_over_all_operations() {
        let mut path_item = PathItem::new(HttpMethod::Get, operation());
        path_item.query = Some(operation());
        path_item
            .additional_operations
            .insert("PURGE".to_string(), operation());

        assert_eq!(iter_operations(&path_item).count(), 3);
        let methods = iter_operations_mut(&mut path_item)
            .map(|(method, _)| method.to_string())
            .collect::<Vec<_>>();
        assert_eq!(methods, ["get", "query", "PURGE"]);
    }

    #[test]
    fn should_drop_server_names() {
        let path_item = PathItemBuilder::new()
            .servers(Some(vec![named_server("https://path.example.com", "path")]))
            .operation(HttpMethod::Get, operation())
            .build();
        let mut openapi = openapi(PathsBuilder::new().path("/items", path_item).build());
        openapi.servers = Some(vec![named_server("https://api.example.com", "prod")]);

        downgrade_to_31(&mut openapi);

        assert_snapshot!(to_yaml(&openapi), @r#"
        openapi: "3.1.0"
        info:
          title: test
          version: "1.0.0"
        servers:
        - url: https://api.example.com
        paths:
          /items:
            servers:
            - url: https://path.example.com
            get:
              responses:
                "200":
                  description: The resource
              servers:
              - url: https://op.example.com
        "#);
    }

    #[test]
    fn should_drop_tag_metadata() {
        let mut openapi = openapi(Paths::new());
        openapi.tags = Some(vec![
            TagBuilder::new()
                .name("pets")
                .summary(Some("Pets"))
                .description(Some("Everything about pets"))
                .parent(Some("animals"))
                .kind(Some("nav"))
                .build(),
        ]);

        downgrade_to_31(&mut openapi);

        assert_snapshot!(to_yaml(&openapi), @r#"
        openapi: "3.1.0"
        info:
          title: test
          version: "1.0.0"
        paths: {}
        tags:
        - name: pets
          description: Everything about pets
        "#);
    }

    #[test]
    fn should_drop_response_summaries() {
        let mut openapi = openapi(
            PathsBuilder::new()
                .path("/items", PathItem::new(HttpMethod::Get, operation()))
                .build(),
        );
        openapi.components = Some(
            ComponentsBuilder::new()
                .response("NotFound", summarized_response())
                .build(),
        );

        downgrade_to_31(&mut openapi);

        assert_snapshot!(to_yaml(&openapi), @r#"
        openapi: "3.1.0"
        info:
          title: test
          version: "1.0.0"
        paths:
          /items:
            get:
              responses:
                "200":
                  description: The resource
              servers:
              - url: https://op.example.com
        components:
          responses:
            NotFound:
              description: The resource
        "#);
    }

    #[test]
    fn should_remove_path_items_without_operation() {
        let mut path_item = PathItem::default();
        path_item.summary = Some("Empty".to_string());
        let mut paths = PathsBuilder::new()
            .path("/empty", path_item)
            .path("/items", PathItem::new(HttpMethod::Get, operation()))
            .build();

        downgrade_paths_to_31(&mut paths);

        assert_eq!(paths.paths.keys().collect::<Vec<_>>(), ["/items"]);
    }
}
