use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    ToSchema,
    derive_more::Display,
)]
#[display("obs#{_0}")]
pub struct ObservationId(pub(super) usize);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct PartialObservation {
    pub name: String,
    pub position: LngLat,
    pub color: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Observation {
    pub id: ObservationId,

    #[serde(flatten)]
    pub data: PartialObservation,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize, ToSchema)]
pub struct LngLat {
    pub lng: f64,
    pub lat: f64,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, ToSchema)]
pub struct PatchObservation {
    pub name: Option<String>,
    pub position: Option<LngLat>,
    pub color: Option<String>,
    pub notes: Option<String>,
}
