use std::borrow::Cow;
use std::fmt::Debug;
use std::sync::LazyLock;

use indexmap::IndexMap;
use percent_encoding::NON_ALPHANUMERIC;
use regex::Regex;
use serde::Serialize;
use tracing::warn;
use utoipa::openapi::RefOr;
use utoipa::openapi::schema::Schema;
use utoipa::{PartialSchema, ToSchema};

use super::{ApiClientError, Schemas};

static RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{(?<name>\w*)}").expect("a valid regex"));

#[derive(Debug, Default, derive_more::Display)]
#[display("{path}")]
pub struct CallPath {
    pub(super) path: String,
    args: IndexMap<String, Box<dyn PathArg>>,
    schemas: Schemas,
}

impl CallPath {
    pub fn insert_arg<A>(&mut self, name: impl Into<String>, arg: A)
    where
        A: PathArg + ToSchema + 'static,
    {
        let example = arg.as_path_value();
        self.args.insert(name.into(), Box::new(arg));
        self.schemas.add_example::<A>(example);
    }
}

impl From<&str> for CallPath {
    fn from(value: &str) -> Self {
        Self::from(value.to_string())
    }
}

impl From<String> for CallPath {
    fn from(value: String) -> Self {
        let path = value;
        let args = Default::default();
        let schemas = Schemas::default();
        Self {
            path,
            args,
            schemas,
        }
    }
}

#[derive(Debug, derive_more::Error, derive_more::Display)]
pub enum PathError {
    #[display("missing parameters: {names:?}")]
    MissingParameters { path: String, names: Vec<String> },
}

#[derive(Debug)]
pub(super) struct PathParam(pub(super) String);

#[derive(Debug)]
pub(super) struct PathResolved {
    pub(super) path: String,
    pub(super) params: Vec<PathParam>,
    pub(super) schemas: Schemas,
}

// Build concrete
impl TryFrom<CallPath> for PathResolved {
    type Error = ApiClientError;

    fn try_from(value: CallPath) -> Result<Self, Self::Error> {
        let CallPath {
            mut path,
            args,
            schemas,
        } = value;

        let mut names = RE
            .captures_iter(&path)
            .filter_map(|caps| caps.name("name"))
            .map(|m| m.as_str().to_string())
            .collect::<Vec<_>>();

        let mut params = vec![];

        if names.is_empty() {
            return Ok(Self {
                path,
                params,
                schemas,
            });
        }

        for (name, arg) in args {
            let Some(idx) = names.iter().position(|it| it == &name) else {
                warn!(?name, "argument name not found");
                continue;
            };

            let Some(value) = arg.as_path_value() else {
                warn!("cannot provide argument value");
                continue;
            };

            names.remove(idx);

            // TODO explore [URI template](https://datatracker.ietf.org/doc/html/rfc6570) - https://github.com/ilaborie/clawspec/issues/21
            // See <https://crates.io/crates/iri-string>, <https://crates.io/crates/uri-template-system>
            let value = percent_encoding::utf8_percent_encode(&value, NON_ALPHANUMERIC).to_string();
            path = path.replace(&format!("{{{name}}}"), &value); // TODO: Optimize string allocations - https://github.com/ilaborie/clawspec/issues/31

            params.push(PathParam(name.to_string()));

            if names.is_empty() {
                return Ok(Self {
                    path,
                    params,
                    schemas,
                });
            }
        }

        Err(ApiClientError::PathUnresolved {
            path,
            missings: names,
        })
    }
}

// Args

pub trait PathArg: Debug {
    fn as_path_value(&self) -> Option<String>; // TODO Result - https://github.com/ilaborie/clawspec/issues/22
}

#[derive(Debug)]
pub struct DisplayArg<T>(pub T);
impl<T: ToSchema> ToSchema for DisplayArg<T> {
    fn name() -> Cow<'static, str> {
        T::name()
    }
}
impl<T: ToSchema> PartialSchema for DisplayArg<T> {
    fn schema() -> RefOr<Schema> {
        T::schema()
    }
}

impl<T> PathArg for DisplayArg<T>
where
    T: ToString + Debug,
{
    fn as_path_value(&self) -> Option<String> {
        Some(self.0.to_string())
    }
}

#[derive(Debug)]
pub struct StringSerializeArg<T>(pub T);

impl<T: ToSchema> ToSchema for StringSerializeArg<T> {
    fn name() -> Cow<'static, str> {
        T::name()
    }
}
impl<T: ToSchema> PartialSchema for StringSerializeArg<T> {
    fn schema() -> RefOr<Schema> {
        T::schema()
    }
}

impl<T> PathArg for StringSerializeArg<T>
where
    T: Serialize + Debug,
{
    fn as_path_value(&self) -> Option<String> {
        let value = match serde_json::to_value(&self.0) {
            Ok(value) => value,
            Err(error) => {
                warn!(?error, "fail to serialize value");
                return None;
            }
        };

        let serde_json::Value::String(value) = value else {
            // TODO with URI template we could support array and object - https://github.com/ilaborie/clawspec/issues/21
            warn!(?value, "expected serialization as String");
            return None;
        };

        Some(value)
    }
}

// TODO dsl path!(""/ object / ""...) - https://github.com/ilaborie/clawspec/issues/21

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_build_call_path() {
        let mut path = CallPath::from("/breed/{breed}/images");
        path.insert_arg("breed", DisplayArg("hound"));

        insta::assert_debug_snapshot!(path, @r#"
        CallPath {
            path: "/breed/{breed}/images",
            args: {
                "breed": DisplayArg(
                    "hound",
                ),
            },
            schemas: Schemas(
                [
                    "clawspec_utoipa::client::path::DisplayArg<&str>",
                ],
            ),
        }
        "#);

        let path_resolved = PathResolved::try_from(path).expect("full resolve");

        insta::assert_debug_snapshot!(path_resolved, @r#"
        PathResolved {
            path: "/breed/hound/images",
            params: [
                PathParam(
                    "breed",
                ),
            ],
            schemas: Schemas(
                [
                    "clawspec_utoipa::client::path::DisplayArg<&str>",
                ],
            ),
        }
        "#);
    }

    #[test]
    fn test_path_resolved_with_multiple_parameters() {
        let mut path = CallPath::from("/users/{user_id}/posts/{post_id}");
        path.insert_arg("user_id", DisplayArg(123));
        path.insert_arg("post_id", DisplayArg("abc"));

        let resolved = PathResolved::try_from(path).expect("should resolve");

        insta::assert_debug_snapshot!(resolved, @r#"
        PathResolved {
            path: "/users/123/posts/abc",
            params: [
                PathParam(
                    "user_id",
                ),
                PathParam(
                    "post_id",
                ),
            ],
            schemas: Schemas(
                [
                    "clawspec_utoipa::client::path::DisplayArg<i32>",
                    "clawspec_utoipa::client::path::DisplayArg<&str>",
                ],
            ),
        }
        "#);
    }

    #[test]
    fn test_path_resolved_with_missing_parameters() {
        let mut path = CallPath::from("/users/{user_id}/posts/{post_id}");
        path.insert_arg("user_id", DisplayArg(123));
        // Missing post_id parameter

        let result = PathResolved::try_from(path);
        assert!(result.is_err());
    }

    #[test]
    fn test_path_resolved_with_url_encoding() {
        let mut path = CallPath::from("/search/{query}");
        path.insert_arg("query", DisplayArg("hello world"));

        let resolved = PathResolved::try_from(path).expect("should resolve");

        assert_eq!(resolved.path, "/search/hello%20world");
    }

    #[test]
    fn test_path_resolved_with_special_characters() {
        let mut path = CallPath::from("/items/{name}");
        path.insert_arg("name", DisplayArg("test@example.com"));

        let resolved = PathResolved::try_from(path).expect("should resolve");

        insta::assert_snapshot!(resolved.path, @"/items/test%40example%2Ecom");
    }

    #[test]
    fn test_path_with_duplicate_parameter_names() {
        let mut path = CallPath::from("/test/{id}/{id}");
        path.insert_arg("id", DisplayArg(123));

        // This will actually fail because the algorithm doesn't handle duplicates properly
        // The replace() replaces all occurrences but names.remove() only removes one from the list
        let result = PathResolved::try_from(path);

        // This demonstrates the current behavior - should fail with missing parameter
        assert!(result.is_err());
    }

    #[test]
    fn test_insert_arg_overwrites_existing() {
        let mut path = CallPath::from("/test/{id}");
        path.insert_arg("id", DisplayArg(123));
        path.insert_arg("id", DisplayArg(456)); // Overwrite

        let resolved = PathResolved::try_from(path).expect("should resolve");
        assert_eq!(resolved.path, "/test/456");
    }
}
