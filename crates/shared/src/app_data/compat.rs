use {super::ProtocolAppData, model::order::Hooks, serde::Deserialize};

/// The legacy `backend` app data object.
#[derive(Debug, Default, Deserialize)]
pub struct BackendAppData {
    #[serde(default)]
    pub hooks: Hooks,
}

impl From<BackendAppData> for ProtocolAppData {
    fn from(value: BackendAppData) -> Self {
        Self {
            hooks: value.hooks,
            signer: None,
            replaced_order: None,
        }
    }
}
