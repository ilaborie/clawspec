// TODO: Complete implementation - https://github.com/ilaborie/clawspec/issues/28
#[derive(Debug, Clone)]
pub struct CallHeaders {
    // Vec<(name, value)>
    // Or Serializable (See Axum header extractor)
}

impl CallHeaders {
    pub fn merge(self, _other: Self) -> Self {
        todo!() // TODO: Complete implementation - https://github.com/ilaborie/clawspec/issues/28
    }
}
