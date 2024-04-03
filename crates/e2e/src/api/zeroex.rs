use chrono::{DateTime, NaiveDateTime, Utc};
use ethcontract::common::abi::{encode, Token};
use ethcontract::private::lazy_static;
use hex_literal::hex;
use web3::signing;
use {
    shared::{
        zeroex_api,
        zeroex_api::{OrderRecord, OrdersQuery, ZeroExResponseError},
    },
    std::{collections::HashMap, net::SocketAddr, str::FromStr, sync::Arc},
    warp::{Filter, Reply},
    web3::types::H160,
};
use autopilot::domain::eth::U256;
use driver::domain::eth::H256;
use model::DomainSeparator;
use shared::zeroex_api::{Order, OrderMetadata, ZeroExSignature};
use crate::setup::TestAccount;

type OrdersHandler =
    Arc<dyn Fn(&OrdersQuery) -> Result<Vec<OrderRecord>, ZeroExResponseError> + Send + Sync>;

#[derive(Default)]
pub struct ZeroExApiBuilder {
    orders_handler: Option<OrdersHandler>,
}

impl ZeroExApiBuilder {
    pub fn with_orders_handler(mut self, handler: OrdersHandler) -> Self {
        self.orders_handler = Some(handler);
        self
    }

    pub fn build(&self) -> ZeroExApi {
        ZeroExApi {
            orders_handler: self
                .orders_handler
                .clone()
                .unwrap_or_else(|| self.not_implemented_handler()),
        }
    }

    fn not_implemented_handler(&self) -> OrdersHandler {
        Arc::new(Box::new(|_query: &OrdersQuery| {
            Err(ZeroExResponseError::ServerError(
                "not implemented".to_string(),
            ))
        }))
    }
}

pub struct ZeroExApi {
    orders_handler: OrdersHandler,
}

impl ZeroExApi {
    pub fn builder() -> ZeroExApiBuilder {
        ZeroExApiBuilder::default()
    }

    /// Starts the server and returns the assigned port number.
    pub async fn run(&self) -> u16 {
        let orders_handler = self.orders_handler.clone();

        let orders_route = warp::path!("orderbook" / "v1" / "orders")
            .and(warp::query::<HashMap<String, String>>())
            .map(move |params: HashMap<String, String>| {
                let query = OrdersQuery {
                    taker: params.get("taker").and_then(|t| H160::from_str(t).ok()),
                    sender: params.get("sender").and_then(|s| H160::from_str(s).ok()),
                    verifying_contract: params
                        .get("verifyingContract")
                        .and_then(|vc| H160::from_str(vc).ok()),
                };

                match orders_handler(&query) {
                    Ok(orders) => warp::reply::json(&zeroex_api::OrdersResponse {
                        total: orders.len() as u64,
                        page: 1,
                        per_page: 100,
                        records: orders,
                    })
                    .into_response(),
                    Err(err) => warp::reply::with_status(
                        warp::reply::json(&err.to_string()),
                        warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                    )
                    .into_response(),
                }
            });

        let addr: SocketAddr = ([0, 0, 0, 0], 0).into();
        let server = warp::serve(orders_route);
        let (addr, server) = server.bind_ephemeral(addr);
        let port = addr.port();
        assert!(port > 0, "assigned port must be greater than 0");

        tokio::spawn(async move {
            server.await;
        });

        tracing::info!("Started ZeroEx API server at {}", addr);

        port
    }
}


pub struct Eip712TypedZeroExOrder {
    pub maker_token: H160,
    pub taker_token: H160,
    pub maker_amount: u128,
    pub taker_amount: u128,
    pub taker_token_fee_amount: u128,
    pub maker: H160,
    pub taker: H160,
    pub sender: H160,
    pub fee_recipient: H160,
    pub pool: H256,
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
        verifying_contract: H160,
        signer: TestAccount,
    ) -> OrderRecord {
        OrderRecord {
            metadata: OrderMetadata {
                created_at: DateTime::<Utc>::MIN_UTC,
                order_hash: self.hash_struct().to_vec(),
                remaining_fillable_taker_amount: self.taker_amount,
            },
            order: Order {
                chain_id,
                expiry: NaiveDateTime::MAX.timestamp() as u64,
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
        }
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
        hash_data[44..64].copy_from_slice(self.maker_token.as_fixed_bytes());
        hash_data[76..96].copy_from_slice(self.taker_token.as_fixed_bytes());
        hash_data[112..128].copy_from_slice(&self.maker_amount.to_be_bytes());
        hash_data[144..160].copy_from_slice(&self.taker_amount.to_be_bytes());
        hash_data[176..192].copy_from_slice(&self.taker_token_fee_amount.to_be_bytes());
        hash_data[204..224].copy_from_slice(self.maker.as_fixed_bytes());
        hash_data[236..256].copy_from_slice(self.taker.as_fixed_bytes());
        hash_data[268..288].copy_from_slice(self.sender.as_fixed_bytes());
        hash_data[300..320].copy_from_slice(self.fee_recipient.as_fixed_bytes());
        hash_data[320..352].copy_from_slice(self.pool.as_fixed_bytes());
        hash_data[376..384].copy_from_slice(&self.expiry.to_be_bytes());
        self.salt.to_big_endian(&mut hash_data[384..416]);
        signing::keccak256(&hash_data)
    }
}

struct ZeroExDomainSeparator([u8; 32]);

impl ZeroExDomainSeparator {
    // See <https://github.com/0xProject/protocol/blob/%400x/contracts-zero-ex%400.49.0/contracts/zero-ex/contracts/src/fixins/FixinEIP712.sol>
    pub fn new(chain_id: u64, contract_addr: H160) -> Self {
        lazy_static! {
            /// The EIP-712 domain name used for computing the domain separator.
            static ref DOMAIN_NAME: [u8; 32] = signing::keccak256(b"ZeroEx");

            /// The EIP-712 domain version used for computing the domain separator.
            static ref DOMAIN_VERSION: [u8; 32] = signing::keccak256(b"1.0.0");

            /// The EIP-712 domain type used computing the domain separator.
            static ref DOMAIN_TYPE_HASH: [u8; 32] = signing::keccak256(
                b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
            );
        }
        let abi_encode_string = encode(&[
            Token::FixedBytes((*DOMAIN_TYPE_HASH).into()),
            Token::FixedBytes((*DOMAIN_NAME).into()),
            Token::FixedBytes((*DOMAIN_VERSION).into()),
            Token::Uint(chain_id.into()),
            Token::Address(contract_addr),
        ]);

        Self(signing::keccak256(abi_encode_string.as_slice()))
    }

    pub fn to_domain_separator(&self) -> DomainSeparator {
        DomainSeparator(self.0)
    }
}
