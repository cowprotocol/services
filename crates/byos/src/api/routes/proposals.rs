use {
    crate::{
        api::AppState,
        domain::{
            eip712::{self, ProposalData, order_uid_hash},
            proposal::{Interaction, Proposal},
        },
    },
    alloy_primitives::U256,
    axum::{
        Json,
        extract::{Path, State},
        http::StatusCode,
        response::IntoResponse,
    },
    number::serialization::HexOrDecimalU256,
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
    std::sync::Arc,
};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitProposalRequest {
    #[serde_as(as = "serde_ext::Hex")]
    pub order_uid: Vec<u8>,
    #[serde_as(as = "HexOrDecimalU256")]
    pub sell_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub buy_amount: U256,
    pub interactions: Vec<Interaction>,
    pub valid_until: u64,
    #[serde_as(as = "HexOrDecimalU256")]
    pub nonce: U256,
    #[serde_as(as = "serde_ext::Hex")]
    pub signature: Vec<u8>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitProposalResponse {
    pub id: u64,
}

pub async fn submit_proposal(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SubmitProposalRequest>,
) -> impl IntoResponse {
    let order_uid: [u8; 56] = match request.order_uid.try_into() {
        Ok(uid) => uid,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "order_uid must be 56 bytes"})),
            );
        }
    };

    let signature: [u8; 65] = match request.signature.try_into() {
        Ok(sig) => sig,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "signature must be 65 bytes"})),
            );
        }
    };

    let proposal_data = ProposalData {
        orderUidHash: order_uid_hash(&order_uid),
        sellAmount: request.sell_amount,
        buyAmount: request.buy_amount,
        validUntil: U256::from(request.valid_until),
        nonce: request.nonce,
    };

    let solver = match eip712::recover_signer(&proposal_data, &signature, &state.domain) {
        Some(addr) => addr,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid signature"})),
            );
        }
    };

    let proposal = Proposal {
        id: 0, // will be set by store
        order_uid,
        sell_amount: request.sell_amount,
        buy_amount: request.buy_amount,
        interactions: request.interactions,
        solver,
        valid_until: request.valid_until,
        nonce: request.nonce,
    };

    let id = state.store.insert(proposal).await;
    tracing::info!(%id, %solver, "proposal accepted");

    (
        StatusCode::CREATED,
        Json(serde_json::json!(SubmitProposalResponse { id })),
    )
}

pub async fn get_proposals(
    State(state): State<Arc<AppState>>,
    Path(order_uid_hex): Path<String>,
) -> impl IntoResponse {
    let uid_bytes = match const_hex::decode(&order_uid_hex) {
        Ok(bytes) => bytes,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid order_uid hex"})),
            );
        }
    };

    let order_uid: [u8; 56] = match uid_bytes.try_into() {
        Ok(uid) => uid,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "order_uid must be 56 bytes"})),
            );
        }
    };

    match state.store.get_metadata(&order_uid).await {
        Some(meta) => (StatusCode::OK, Json(serde_json::to_value(meta).unwrap())),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "no proposals for this order"})),
        ),
    }
}

pub async fn cancel_proposal(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    if state.store.remove(id).await {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}
