//! The autopilot's HTTP API: the native price cache, served to the orderbook.

use {
    crate::infra::prices::{NativePrices, RateLimited},
    axum::{
        Json,
        Router,
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Response},
        routing::get,
    },
    observe::tracing::distributed::axum::{make_span, record_trace_id},
    serde::Serialize,
    serde_with::{DisplayFromStr, serde_as},
    solana_sdk::pubkey::Pubkey,
    tokio::net::TcpListener,
};

/// The `GET /native_price/{mint}` body: lamports per 10^9 atoms of the mint,
/// the unit the auctions carry.
#[serde_as]
#[derive(Debug, Serialize)]
struct NativePrice {
    #[serde_as(as = "DisplayFromStr")]
    price: u64,
}

/// Serve the API on `listener` until the task is dropped.
pub async fn serve(listener: TcpListener, prices: NativePrices) -> std::io::Result<()> {
    let app = Router::new()
        .route("/native_price/{mint}", get(native_price))
        .with_state(prices)
        .layer(
            tower::ServiceBuilder::new()
                .layer(tower_http::trace::TraceLayer::new_for_http().make_span_with(make_span))
                .map_request(record_trace_id),
        );
    tracing::info!(addr = ?listener.local_addr()?, "serving HTTP API");
    axum::serve(listener, app).await
}

/// Handle `GET /native_price/{mint}`: 200 with the price, 404 for a mint no
/// estimator prices, 429 while the sources are rate limited.
async fn native_price(Path(mint): Path<String>, State(prices): State<NativePrices>) -> Response {
    let Ok(mint) = mint.parse::<Pubkey>() else {
        return (StatusCode::BAD_REQUEST, "Invalid mint").into_response();
    };
    match prices.price(mint).await {
        Ok(Some(price)) => Json(NativePrice { price }).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "No liquidity").into_response(),
        Err(err) if err.is::<RateLimited>() => {
            (StatusCode::TOO_MANY_REQUESTS, "Rate limited").into_response()
        }
        Err(err) => {
            tracing::warn!(?err, %mint, "native price lookup failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, std::net::SocketAddr};

    async fn spawn(prices: NativePrices) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { serve(listener, prices).await.unwrap() });
        addr
    }

    async fn fetch(addr: SocketAddr, mint: &str) -> (reqwest::StatusCode, String) {
        let response = reqwest::get(format!("http://{addr}/native_price/{mint}"))
            .await
            .unwrap();
        (response.status(), response.text().await.unwrap())
    }

    /// A priced mint answers its price as a decimal string, an unpriced one
    /// 404, and a path that is no mint 400.
    #[tokio::test]
    async fn serves_the_price_cache() {
        let priced = Pubkey::new_unique();
        let unpriced = Pubkey::new_unique();
        let addr = spawn(NativePrices::seeded([
            (priced, Some(5_000_000_000)),
            (unpriced, None),
        ]))
        .await;

        assert_eq!(
            fetch(addr, &priced.to_string()).await,
            (
                reqwest::StatusCode::OK,
                r#"{"price":"5000000000"}"#.to_owned()
            )
        );
        assert_eq!(
            fetch(addr, &unpriced.to_string()).await,
            (reqwest::StatusCode::NOT_FOUND, "No liquidity".to_owned())
        );
        assert_eq!(
            fetch(addr, "not-a-mint").await.0,
            reqwest::StatusCode::BAD_REQUEST
        );
    }

    /// Without estimators every mint prices at the denominator.
    #[tokio::test]
    async fn prices_at_the_denominator_without_sources() {
        let addr = spawn(NativePrices::Denominated).await;
        assert_eq!(
            fetch(addr, &Pubkey::new_unique().to_string()).await,
            (
                reqwest::StatusCode::OK,
                r#"{"price":"1000000000"}"#.to_owned()
            )
        );
    }

    /// The sources' spent quota answers 429, so the orderbook can tell it
    /// from a mint nobody prices.
    #[tokio::test]
    async fn rate_limited_sources_answer_429() {
        let app = Router::new().route(
            "/simple/token_price/solana",
            get(|| async { StatusCode::TOO_MANY_REQUESTS }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let coingecko = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let mint = solana_testlib::classic_mint(6);
        let rpc = cow_solana_rpc::SolanaRPC::new_mock_with_mocks(cow_solana_rpc::Mocks::from([(
            cow_solana_rpc::RpcRequest::GetMultipleAccounts,
            solana_testlib::multiple_accounts_json([solana_testlib::account_json(&mint)]),
        )]));
        let prices = NativePrices::new(
            &crate::infra::config::NativePrices {
                estimators: vec![crate::infra::config::NativePriceEstimator::CoinGecko {
                    endpoint: format!("http://{coingecko}/simple/token_price")
                        .parse()
                        .unwrap(),
                    api_key: String::new(),
                }],
                ttl: std::time::Duration::from_secs(60),
                driver_probe_lamports: 100_000_000,
            },
            rpc,
            Pubkey::new_unique(),
        );
        let addr = spawn(prices).await;

        assert_eq!(
            fetch(addr, &Pubkey::new_unique().to_string()).await,
            (
                reqwest::StatusCode::TOO_MANY_REQUESTS,
                "Rate limited".to_owned()
            )
        );
    }
}
