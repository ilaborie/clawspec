//! Compatibility pass that strips data the older OpenAPI output cannot carry.

use tracing::{debug, warn};
use utoipa::openapi::path::{Operation, Parameter, ParameterIn, ParameterStyle, PathItem};
use utoipa::openapi::request_body::RequestBody;
use utoipa::openapi::response::Response;
use utoipa::openapi::security::{ApiKey, Flow, SecurityScheme};
use utoipa::openapi::{Content, Header, OpenApi, Paths, RefOr, Server, Tag};

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
        drop_responses_item_fields(&mut components.responses, "components.responses");
        for (name, request_body) in &mut components.request_bodies {
            drop_request_body_item_fields(
                request_body,
                &format!("components.requestBodies.{name}"),
            );
        }
        drop_parameters_item_fields(components.parameters.values_mut(), "components.parameters");
        drop_headers_item_fields(&mut components.headers, "components.headers");
        if !std::mem::take(&mut components.media_types).is_empty() {
            warn!(
                location = "components",
                field = "mediaTypes",
                "dropping field not supported by OpenAPI 3.1"
            );
        }
        for (name, scheme) in &mut components.security_schemes {
            if let RefOr::T(scheme) = scheme {
                downgrade_security_scheme(scheme, &format!("components.securitySchemes.{name}"));
            }
        }
    }
}

pub(in crate::client) fn downgrade_paths_to_31(paths: &mut Paths) {
    for (path, path_item) in &mut paths.paths {
        drop_extra_operations(path_item, path);
        drop_server_names(
            path_item.servers.as_deref_mut(),
            &format!("paths.{path}.servers"),
        );
        drop_parameters_item_fields(
            path_item.parameters.iter_mut().flatten(),
            &format!("paths.{path}.parameters"),
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
            drop_operation_item_fields(operation, &location);
        }
    }
    remove_empty_path_items(paths);
}

fn drop_field<T>(value: &mut Option<T>, location: &str, field: &str) {
    if value.take().is_some() {
        warn!(%location, %field, "dropping field not supported by OpenAPI 3.1");
    }
}

fn drop_extra_operations(path_item: &mut PathItem, path: &str) {
    let location = format!("paths.{path}");
    drop_field(&mut path_item.query, &location, "query");
    for method in std::mem::take(&mut path_item.additional_operations).into_keys() {
        warn!(%location, %method, "dropping additional operation not supported by OpenAPI 3.1");
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
        if tag.description.as_deref().is_none_or(str::is_empty)
            && let Some(summary) = tag.summary.take()
        {
            debug!(%location, "folding tag summary into description");
            tag.description = Some(summary);
        }
        drop_field(&mut tag.summary, &location, "summary");
        drop_field(&mut tag.parent, &location, "parent");
        drop_field(&mut tag.kind, &location, "kind");
    }
}

fn downgrade_security_scheme(scheme: &mut SecurityScheme, location: &str) {
    match scheme {
        SecurityScheme::OAuth2(oauth2) => {
            drop_field(&mut oauth2.deprecated, location, "deprecated");
            drop_field(
                &mut oauth2.oauth2_metadata_url,
                location,
                "oauth2MetadataUrl",
            );
            let flow_count = oauth2.flows.len();
            oauth2
                .flows
                .retain(|_, flow| !matches!(flow, Flow::DeviceAuthorization(_)));
            if oauth2.flows.len() != flow_count {
                warn!(
                    %location,
                    field = "flows.deviceAuthorization",
                    "dropping field not supported by OpenAPI 3.1"
                );
                if oauth2.flows.is_empty() {
                    warn!(%location, "OAuth2 security scheme has no flow left");
                }
            }
        }
        SecurityScheme::ApiKey(
            ApiKey::Header(value) | ApiKey::Query(value) | ApiKey::Cookie(value),
        ) => {
            drop_field(&mut value.deprecated, location, "deprecated");
        }
        SecurityScheme::Http(http) => drop_field(&mut http.deprecated, location, "deprecated"),
        SecurityScheme::OpenIdConnect(oidc) => {
            drop_field(&mut oidc.deprecated, location, "deprecated");
        }
        SecurityScheme::MutualTls { deprecated, .. } => {
            drop_field(deprecated, location, "deprecated");
        }
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

fn drop_operation_item_fields(operation: &mut Operation, location: &str) {
    drop_parameters_item_fields(
        operation.parameters.iter_mut().flatten(),
        &format!("{location}.parameters"),
    );
    if let Some(request_body) = operation.request_body.as_mut() {
        drop_request_body_item_fields(request_body, &format!("{location}.requestBody"));
    }
    drop_responses_item_fields(
        &mut operation.responses.responses,
        &format!("{location}.responses"),
    );
}

fn drop_parameters_item_fields<'a>(
    parameters: impl IntoIterator<Item = &'a mut RefOr<Parameter>>,
    location: &str,
) {
    for parameter in parameters {
        if let RefOr::T(parameter) = parameter {
            let location = format!("{location}.{}", parameter.name);
            drop_content_item_fields(&mut parameter.content, &location);
            convert_querystring_parameter(parameter, &location);
            drop_cookie_style(parameter, &location);
        }
    }
}

fn convert_querystring_parameter(parameter: &mut Parameter, location: &str) {
    if parameter.parameter_in != ParameterIn::QueryString {
        return;
    }
    parameter.parameter_in = ParameterIn::Query;
    let schema = match parameter.content.values().next() {
        Some(RefOr::T(content)) if parameter.content.len() == 1 => content.schema.clone(),
        _ => None,
    };
    let Some(schema) = schema else {
        warn!(%location, "converting querystring parameter to a query parameter, keeping its content");
        return;
    };
    debug!(%location, "converting querystring parameter to an exploded form query parameter");
    parameter.content.clear();
    parameter.schema = Some(schema);
    parameter.style = Some(ParameterStyle::Form);
    parameter.explode = Some(true);
}

fn drop_cookie_style(parameter: &mut Parameter, location: &str) {
    if parameter.style != Some(ParameterStyle::Cookie) {
        return;
    }
    debug!(%location, "dropping cookie style and explode, using the default cookie serialization");
    parameter.style = None;
    parameter.explode = None;
}

fn drop_request_body_item_fields(request_body: &mut RefOr<RequestBody>, location: &str) {
    if let RefOr::T(request_body) = request_body {
        drop_content_item_fields(&mut request_body.content, location);
    }
}

fn drop_responses_item_fields<'a>(
    responses: impl IntoIterator<Item = (&'a String, &'a mut RefOr<Response>)>,
    location: &str,
) {
    for (status, response) in responses {
        if let RefOr::T(response) = response {
            let location = format!("{location}.{status}");
            drop_content_item_fields(&mut response.content, &location);
            drop_headers_item_fields(&mut response.headers, &format!("{location}.headers"));
        }
    }
}

fn drop_headers_item_fields<'a>(
    headers: impl IntoIterator<Item = (&'a String, &'a mut RefOr<Header>)>,
    location: &str,
) {
    for (name, header) in headers {
        if let RefOr::T(header) = header {
            drop_content_item_fields(&mut header.content, &format!("{location}.{name}"));
        }
    }
}

fn drop_content_item_fields<'a>(
    contents: impl IntoIterator<Item = (&'a String, &'a mut RefOr<Content>)>,
    location: &str,
) {
    for (media_type, content) in contents {
        if let RefOr::T(content) = content {
            let location = format!("{location}.content.{media_type}");
            drop_field(&mut content.item_schema, &location, "itemSchema");
            drop_field(&mut content.item_encoding, &location, "itemEncoding");
            if !std::mem::take(&mut content.prefix_encoding).is_empty() {
                warn!(%location, field = "prefixEncoding", "dropping field not supported by OpenAPI 3.1");
            }
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
    use utoipa::openapi::encoding::Encoding;
    use utoipa::openapi::path::{
        HttpMethod, OperationBuilder, ParameterBuilder, ParameterIn, PathItemBuilder,
    };
    use utoipa::openapi::request_body::RequestBodyBuilder;
    use utoipa::openapi::response::{ResponseBuilder, ResponsesBuilder};
    use utoipa::openapi::security::{
        ApiKeyValue, ClientCredentials, DeviceAuthorization, Http, HttpAuthScheme, OAuth2,
        OpenIdConnect, Scopes,
    };
    use utoipa::openapi::tag::TagBuilder;
    use utoipa::openapi::{
        ComponentsBuilder, Deprecated, InfoBuilder, OpenApiBuilder, OpenApiVersion, PathsBuilder,
        Ref, ServerBuilder,
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
    fn should_fold_tag_summary_into_missing_description() {
        let mut openapi = openapi(Paths::new());
        openapi.tags = Some(vec![
            TagBuilder::new().name("pets").summary(Some("Pets")).build(),
            TagBuilder::new()
                .name("stores")
                .summary(Some("Stores"))
                .description(Some(""))
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
          description: Pets
        - name: stores
          description: Stores
        "#);
    }

    #[test]
    fn should_drop_security_scheme_metadata() {
        let mut deprecated_key = ApiKeyValue::new("X-Legacy-Key");
        deprecated_key.deprecated = Some(Deprecated::True);
        let mut bearer = Http::new(HttpAuthScheme::Bearer);
        bearer.deprecated = Some(Deprecated::True);
        let mut oidc = OpenIdConnect::new("https://auth.example.com/.well-known/openid");
        oidc.deprecated = Some(Deprecated::True);
        let mut oauth2 = OAuth2::new([
            Flow::ClientCredentials(ClientCredentials::new(
                "https://auth.example.com/token",
                Scopes::new(),
            )),
            Flow::DeviceAuthorization(DeviceAuthorization::new(
                "https://auth.example.com/device",
                "https://auth.example.com/token",
                Scopes::new(),
            )),
        ])
        .with_metadata_url("https://auth.example.com/.well-known/oauth-authorization-server");
        oauth2.deprecated = Some(Deprecated::True);
        let device_only = OAuth2::new([Flow::DeviceAuthorization(DeviceAuthorization::new(
            "https://auth.example.com/device",
            "https://auth.example.com/token",
            Scopes::new(),
        ))]);
        let mut openapi = openapi(Paths::new());
        openapi.components = Some(
            ComponentsBuilder::new()
                .security_scheme(
                    "apiKey",
                    SecurityScheme::ApiKey(ApiKey::Header(deprecated_key)),
                )
                .security_scheme("bearer", SecurityScheme::Http(bearer))
                .security_scheme("oidc", SecurityScheme::OpenIdConnect(oidc))
                .security_scheme("oauth2", SecurityScheme::OAuth2(oauth2))
                .security_scheme("device", SecurityScheme::OAuth2(device_only))
                .security_scheme(
                    "mtls",
                    SecurityScheme::MutualTls {
                        description: None,
                        deprecated: Some(Deprecated::True),
                        extensions: None,
                    },
                )
                .build(),
        );

        downgrade_to_31(&mut openapi);

        assert_snapshot!(to_yaml(&openapi), @r#"
        openapi: "3.1.0"
        info:
          title: test
          version: "1.0.0"
        paths: {}
        components:
          securitySchemes:
            apiKey:
              type: apiKey
              in: header
              name: X-Legacy-Key
            bearer:
              type: http
              scheme: bearer
            device:
              type: oauth2
              flows: {}
            mtls:
              type: mutualTLS
            oauth2:
              type: oauth2
              flows:
                clientCredentials:
                  tokenUrl: https://auth.example.com/token
                  scopes: {}
            oidc:
              type: openIdConnect
              openIdConnectUrl: https://auth.example.com/.well-known/openid
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
    fn should_drop_query_and_additional_operations() {
        let mut mixed = PathItem::new(HttpMethod::Get, operation());
        mixed.query = Some(operation());
        let mut extra_only = PathItem::default();
        extra_only.query = Some(operation());
        extra_only
            .additional_operations
            .insert("PURGE".to_string(), operation());
        let mut openapi = openapi(
            PathsBuilder::new()
                .path("/items", mixed)
                .path("/search", extra_only)
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
        "#);
    }

    #[test]
    fn should_drop_content_item_fields() {
        let item_content = || {
            Content::builder()
                .item_schema(Some(Ref::from_schema_name("Event")))
                .item_encoding(Some(
                    Encoding::builder().content_type(Some("application/json")),
                ))
                .prefix_encoding([Encoding::builder().content_type(Some("text/plain"))])
                .build()
        };
        let operation = OperationBuilder::new()
            .parameter(
                ParameterBuilder::new()
                    .name("filter")
                    .parameter_in(ParameterIn::Query)
                    .content("application/jsonl", item_content()),
            )
            .request_body(Some(
                RequestBodyBuilder::new()
                    .content("application/json-seq", item_content())
                    .build(),
            ))
            .response(
                "200",
                ResponseBuilder::new()
                    .description("Events")
                    .content("text/event-stream", item_content()),
            )
            .build();
        let mut openapi = openapi(
            PathsBuilder::new()
                .path("/events", PathItem::new(HttpMethod::Post, operation))
                .build(),
        );
        openapi.components = Some(
            ComponentsBuilder::new()
                .response(
                    "Stream",
                    ResponseBuilder::new()
                        .description("Stream")
                        .content("application/x-ndjson", item_content()),
                )
                .media_type("EventStream", item_content())
                .build(),
        );

        downgrade_to_31(&mut openapi);

        assert_snapshot!(to_yaml(&openapi), @r#"
        openapi: "3.1.0"
        info:
          title: test
          version: "1.0.0"
        paths:
          /events:
            post:
              parameters:
              - name: filter
                in: query
                required: false
                content:
                  application/jsonl: {}
              requestBody:
                content:
                  application/json-seq: {}
              responses:
                "200":
                  description: Events
                  content:
                    text/event-stream: {}
        components:
          responses:
            Stream:
              description: Stream
              content:
                application/x-ndjson: {}
        "#);
    }

    #[test]
    fn should_convert_querystring_and_drop_cookie_style() {
        let operation = OperationBuilder::new()
            .parameter(
                ParameterBuilder::new()
                    .name("Filter")
                    .parameter_in(ParameterIn::QueryString)
                    .content(
                        "application/x-www-form-urlencoded",
                        Content::new(Some(Ref::from_schema_name("Filter"))),
                    ),
            )
            .parameter(
                ParameterBuilder::new()
                    .name("session")
                    .parameter_in(ParameterIn::Cookie)
                    .schema(Some(Ref::from_schema_name("Session")))
                    .style(Some(ParameterStyle::Cookie))
                    .explode(Some(false)),
            )
            .build();
        let mut openapi = openapi(
            PathsBuilder::new()
                .path("/items", PathItem::new(HttpMethod::Get, operation))
                .build(),
        );

        downgrade_to_31(&mut openapi);

        assert_snapshot!(to_yaml(&openapi), @r##"
        openapi: "3.1.0"
        info:
          title: test
          version: "1.0.0"
        paths:
          /items:
            get:
              parameters:
              - name: Filter
                in: query
                required: false
                schema:
                  $ref: "#/components/schemas/Filter"
                style: form
                explode: true
              - name: session
                in: cookie
                required: false
                schema:
                  $ref: "#/components/schemas/Session"
              responses: {}
        "##);
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
