use std::future::Future;

#[derive(Clone)]
pub struct RpcMetadata {
    pub method_name: String,
    pub trace_id: Option<String>,
}

tokio::task_local! {
    pub static RPC_METADATA: Vec<RpcMetadata>;
}

/// Tries to read the `rpc_metadata` from this task's storage.
/// Returns an empty `Vec` if task local storage was not initialized or is
/// empty.
pub fn get_rpc_metadata_storage() -> Vec<RpcMetadata> {
    let mut data = Vec::new();
    let _ = RPC_METADATA.try_with(|vec| {
        data.clone_from(vec);
    });
    data
}

/// Sets the tasks's local rpc metadata to the passed in value for the given
/// scope.
pub async fn set_rpc_metadata_storage<F, R>(data: Vec<RpcMetadata>, scope: F) -> R
where
    F: Future<Output = R>,
{
    RPC_METADATA.scope(data, scope).await
}
