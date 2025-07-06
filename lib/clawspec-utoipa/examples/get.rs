#![allow(dead_code)]

use clawspec_utoipa::{ApiClient, CallPath, DisplayArg};
use http::uri::Scheme;
use serde::Deserialize;
use utoipa::ToSchema;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().pretty().init();

    // Create a client
    let mut client = ApiClient::builder()
        .scheme(Scheme::HTTP)
        .host("dog.ceo")
        .base_path("/api")?
        .build();

    // Simple get call with no parameters
    let _result = client
        .get("/breeds/list")?
        .exchange()
        .await?
        .as_json::<BreedsList>()
        .await?;

    // Get call with a parameter
    let mut path = CallPath::from("/breed/{breed}/images");
    path.insert_arg("breed", DisplayArg("hound"));

    let _result = client
        .get(path)?
        .exchange()
        .await?
        .as_json::<BreedImages>()
        .await?;

    // extract collected data from client
    let paths = client.collected_openapi().await;
    let out = serde_yaml::to_string(&paths).expect("YAML serialization");
    println!("{out}");

    Ok(())
}

type BreedsList = DogCeoResult<Vec<String>>;

type BreedImages = DogCeoResult<Vec<String>>;

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(tag = "status", rename_all = "lowercase")]
enum DogCeoResult<T> {
    Success { message: T },
    Error { code: u16, message: String },
}
