use {
    crate::setup::TestAccount,
    alloy::primitives::{Address, B256, U256},
    chrono::{DateTime, NaiveDateTime, Utc},
    ethcontract::common::abi::{Token, encode},
    ethrpc::alloy::conversions::IntoLegacy,
    hex_literal::hex,
    model::DomainSeparator,
    shared::zeroex_api::{self, Order, OrderMetadata, OrderRecord, ZeroExSignature},
    std::{net::SocketAddr, sync::LazyLock},
    warp::{Filter, Reply},
    web3::{signing, types::H160},
};

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
        let orders_route = warp::path!("orderbook" / "v1" / "orders").map(move || {
            warp::reply::json(&zeroex_api::OrdersResponse {
                total: self.orders.len() as u64,
                page: 1,
                per_page: 100,
                records: self.orders.clone(),
            })
            .into_response()
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
        verifying_contract: H160,
        signer: TestAccount,
    ) -> OrderRecord {
        OrderRecord::new(
            Order {
                chain_id,
                expiry: NaiveDateTime::MAX.and_utc().timestamp() as u64,
                fee_recipient: self.fee_recipient.into_legacy(),
                maker: self.maker.into_legacy(),
                maker_token: self.maker_token.into_legacy(),
                maker_amount: self.maker_amount,
                pool: self.pool.into_legacy(),
                salt: self.salt.into_legacy(),
                sender: self.sender.into_legacy(),
                taker: self.taker.into_legacy(),
                taker_token: self.taker_token.into_legacy(),
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
        signing::keccak256(&hash_data)
    }
}

struct ZeroExDomainSeparator([u8; 32]);

impl ZeroExDomainSeparator {
    // See <https://github.com/0xProject/protocol/blob/%400x/contracts-zero-ex%400.49.0/contracts/zero-ex/contracts/src/fixins/FixinEIP712.sol>
    pub fn new(chain_id: u64, contract_addr: H160) -> Self {
        /// The EIP-712 domain name used for computing the domain separator.
        static DOMAIN_NAME: LazyLock<[u8; 32]> = LazyLock::new(|| signing::keccak256(b"ZeroEx"));

        /// The EIP-712 domain version used for computing the domain separator.
        static DOMAIN_VERSION: LazyLock<[u8; 32]> = LazyLock::new(|| signing::keccak256(b"1.0.0"));

        /// The EIP-712 domain type used computing the domain separator.
        static DOMAIN_TYPE_HASH: LazyLock<[u8; 32]> = LazyLock::new(|| {
            signing::keccak256(
            b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
        )
        });

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
