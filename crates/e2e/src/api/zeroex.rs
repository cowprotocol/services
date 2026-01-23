use {
    crate::setup::TestAccount,
    alloy::primitives::{Address, B256, U256},
    axum::Json,
    chrono::{DateTime, NaiveDateTime, Utc},
    hex_literal::hex,
    model::DomainSeparator,
    shared::zeroex_api::{self, Order, OrderMetadata, OrderRecord, ZeroExSignature},
    std::{
        net::{Ipv4Addr, SocketAddr},
        sync::Arc,
    },
};

// Mock pagination constants for test API responses
const MOCK_PAGE: u64 = 1;
const MOCK_PER_PAGE: u64 = 100;

#[derive(Clone)]
struct State {
    orders: Arc<Vec<OrderRecord>>,
}

pub struct ZeroExApi {
    orders: Vec<OrderRecord>,
}

impl ZeroExApi {
    /// Creates a new `ZeroExApi` with the given orders to be returned by the
    /// `/orderbook/v1/orders` endpoint.
    pub fn new(orders: Vec<OrderRecord>) -> Self {
        Self { orders }
    }

    /// Starts the server and returns the assigned port number.
    pub async fn run(self) -> u16 {
        let state = State {
            orders: Arc::new(self.orders),
        };

        let app = axum::Router::new()
            .route("/orderbook/v1/orders", axum::routing::get(orders_handler))
            .with_state(state);

        let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0));
        let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let port = addr.port();
        assert!(port > 0, "assigned port must be greater than 0");

        tokio::spawn(async move {
            if let Err(err) = axum::serve(listener, app).await {
                tracing::error!(?err, "ZeroEx API server failed");
                panic!("ZeroEx test server crashed: {}", err);
            }
        });

        tracing::info!("Started ZeroEx API server at {}", addr);

        port
    }
}

async fn orders_handler(
    axum::extract::State(state): axum::extract::State<State>,
) -> Json<zeroex_api::OrdersResponse> {
    Json(zeroex_api::OrdersResponse {
        total: state.orders.len() as u64,
        page: MOCK_PAGE,
        per_page: MOCK_PER_PAGE,
        records: (*state.orders).clone(),
    })
}

pub struct Eip712TypedZeroExOrder {
    pub maker_token: Address,
    pub taker_token: Address,
    pub maker_amount: u128,
    pub taker_amount: u128,
    pub remaining_fillable_taker_amount: u128,
    pub taker_token_fee_amount: u128,
    pub maker: Address,
    pub taker: Address,
    pub sender: Address,
    pub fee_recipient: Address,
    pub pool: B256,
    pub expiry: u64,
    pub salt: U256,
}

impl Eip712TypedZeroExOrder {
    // See <https://github.com/0xProject/protocol/blob/%400x/contracts-zero-ex%400.49.0/contracts/zero-ex/contracts/src/features/libs/LibNativeOrder.sol#L112>
    const ZEROEX_LIMIT_ORDER_TYPEHASH: [u8; 32] =
        hex!("ce918627cb55462ddbb85e73de69a8b322f2bc88f4507c52fcad6d4c33c29d49");

    pub fn to_order_record(
        &self,
        chain_id: u64,
        verifying_contract: Address,
        signer: TestAccount,
    ) -> OrderRecord {
        OrderRecord::new(
            Order {
                chain_id,
                expiry: NaiveDateTime::MAX.and_utc().timestamp() as u64,
                fee_recipient: self.fee_recipient,
                maker: self.maker,
                maker_token: self.maker_token,
                maker_amount: self.maker_amount,
                pool: self.pool,
                salt: self.salt,
                sender: self.sender,
                taker: self.taker,
                taker_token: self.taker_token,
                taker_amount: self.taker_amount,
                taker_token_fee_amount: self.taker_token_fee_amount,
                verifying_contract,
                signature: self.sign(
                    &ZeroExDomainSeparator::new(chain_id, verifying_contract).to_domain_separator(),
                    self.hash_struct(),
                    signer,
                ),
            },
            OrderMetadata {
                created_at: DateTime::<Utc>::MIN_UTC,
                order_hash: self.hash_struct().to_vec(),
                remaining_fillable_taker_amount: self.remaining_fillable_taker_amount,
            },
        )
    }

    fn sign(
        &self,
        domain_separator: &DomainSeparator,
        hash: [u8; 32],
        signer: TestAccount,
    ) -> ZeroExSignature {
        let signature = signer.sign_typed_data(domain_separator, &hash);
        ZeroExSignature {
            r: signature.r,
            s: signature.s,
            v: signature.v,
            // See <https://github.com/0xProject/protocol/blob/%400x/protocol-utils%4011.24.2/packages/protocol-utils/src/signature_utils.ts#L13>
            signature_type: 2,
        }
    }

    // See <https://github.com/0xProject/protocol/blob/%400x/contracts-zero-ex%400.49.0/contracts/zero-ex/contracts/src/features/libs/LibNativeOrder.sol#L166-L195>
    fn hash_struct(&self) -> [u8; 32] {
        let mut hash_data = [0u8; 416];
        hash_data[0..32].copy_from_slice(&Self::ZEROEX_LIMIT_ORDER_TYPEHASH);
        hash_data[44..64].copy_from_slice(self.maker_token.as_slice());
        hash_data[76..96].copy_from_slice(self.taker_token.as_slice());
        hash_data[112..128].copy_from_slice(&self.maker_amount.to_be_bytes());
        hash_data[144..160].copy_from_slice(&self.taker_amount.to_be_bytes());
        hash_data[176..192].copy_from_slice(&self.taker_token_fee_amount.to_be_bytes());
        hash_data[204..224].copy_from_slice(self.maker.as_slice());
        hash_data[236..256].copy_from_slice(self.taker.as_slice());
        hash_data[268..288].copy_from_slice(self.sender.as_slice());
        hash_data[300..320].copy_from_slice(self.fee_recipient.as_slice());
        hash_data[320..352].copy_from_slice(self.pool.as_slice());
        hash_data[376..384].copy_from_slice(&self.expiry.to_be_bytes());
        hash_data[384..416].copy_from_slice(&self.salt.to_be_bytes::<32>());
        alloy::primitives::keccak256(hash_data).into()
    }
}

struct ZeroExDomainSeparator([u8; 32]);

impl ZeroExDomainSeparator {
    // See <https://github.com/0xProject/protocol/blob/%400x/contracts-zero-ex%400.49.0/contracts/zero-ex/contracts/src/fixins/FixinEIP712.sol>
    pub fn new(chain_id: u64, contract_addr: Address) -> Self {
        let domain = alloy::sol_types::eip712_domain! {
            name: "ZeroEx",
            version: "1.0.0",
            chain_id: chain_id,
            verifying_contract: contract_addr,
        };

        Self(domain.separator().into())
    }

    pub fn to_domain_separator(&self) -> DomainSeparator {
        DomainSeparator(self.0)
    }
}
