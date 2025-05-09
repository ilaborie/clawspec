use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::LazyLock;

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
    args: HashMap<String, Box<dyn PathArg>>,
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

// impl FromStr for CallPath {
//     type Err = Infallible;

//     fn from_str(s: &str) -> Result<Self, Self::Err> {
//         let result = Self::from(s.to_string());
//         Ok(result)
//     }
// }

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

            // TODO explore [URI template](https://datatracker.ietf.org/doc/html/rfc6570)
            // See <https://crates.io/crates/iri-string>, <https://crates.io/crates/uri-template-system>
            let value = urlencoding::encode(&value);
            path = path.replace(&format!("{{{name}}}"), &value);

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
    fn as_path_value(&self) -> Option<String>; // TODO Result
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
            // TODO with URI template we could support array and object
            warn!(?value, "expected serialization as String");
            return None;
        };

        Some(value)
    }
}

// TODO dsl path!(""/ object / ""...)

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
        }
        "#);

        let path_resolved = PathResolved::try_from(path).expect("full resolve");

        insta::assert_debug_snapshot!(path_resolved, @r#"
        Done {
            path: "/breed/hound/images",
            params: [],
        }
        "#);
    }
}
