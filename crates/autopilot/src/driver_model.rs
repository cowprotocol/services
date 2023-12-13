//! Types for communicating with drivers as defined in
//! `crates/driver/openapi.yml`.

// TODO: parse proper error type with kind and description, that driver returns.

pub mod quote {
    use {
        number::serialization::HexOrDecimalU256,
        primitive_types::{H160, U256},
        serde::{Deserialize, Serialize},
        serde_with::serde_as,
    };

    #[serde_as]
    #[derive(Clone, Debug, Default, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Request {
        pub sell_token: H160,
        pub buy_token: H160,
        pub kind: Kind,
        #[serde_as(as = "HexOrDecimalU256")]
        pub amount: U256,
    }

    #[derive(Clone, Debug, Default, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub enum Kind {
        #[default]
        Buy,
        Sell,
    }

    #[serde_as]
    #[derive(Clone, Debug, Deserialize)]
    #[serde(untagged, rename_all = "camelCase", deny_unknown_fields)]
    pub enum Response {
        Successful {
            #[serde_as(as = "HexOrDecimalU256")]
            sell_amount: U256,
            #[serde_as(as = "HexOrDecimalU256")]
            buy_amount: U256,
            gas: u64,
        },
        Unfillable {
            unfillable_reason: String,
        },
    }
}

pub mod solve {
    use {
        crate::arguments,
        chrono::{DateTime, Utc},
        model::{
            app_data::AppDataHash,
            bytes_hex::BytesHex,
            order::{BuyTokenDestination, OrderKind, OrderUid, SellTokenSource},
            signature::Signature,
        },
        number::serialization::HexOrDecimalU256,
        primitive_types::{H160, U256},
        serde::{Deserialize, Serialize},
        serde_with::{serde_as, DisplayFromStr},
        std::collections::HashMap,
    };

    #[serde_as]
    #[derive(Clone, Debug, Default, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Request {
        #[serde_as(as = "DisplayFromStr")]
        pub id: i64,
        pub tokens: Vec<Token>,
        pub orders: Vec<Order>,
        pub deadline: DateTime<Utc>,
        #[serde_as(as = "HexOrDecimalU256")]
        pub score_cap: U256,
    }

    #[serde_as]
    #[derive(Clone, Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Token {
        pub address: H160,
        #[serde_as(as = "Option<HexOrDecimalU256>")]
        pub price: Option<U256>,
        pub trusted: bool,
    }

    #[serde_as]
    #[derive(Clone, Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Order {
        pub uid: OrderUid,
        pub sell_token: H160,
        pub buy_token: H160,
        #[serde_as(as = "HexOrDecimalU256")]
        pub sell_amount: U256,
        #[serde_as(as = "HexOrDecimalU256")]
        pub buy_amount: U256,
        #[serde_as(as = "HexOrDecimalU256")]
        pub solver_fee: U256,
        #[serde_as(as = "HexOrDecimalU256")]
        pub user_fee: U256,
        pub valid_to: u32,
        pub kind: OrderKind,
        pub receiver: Option<H160>,
        pub owner: H160,
        pub partially_fillable: bool,
        #[serde_as(as = "HexOrDecimalU256")]
        pub executed: U256,
        pub pre_interactions: Vec<Interaction>,
        pub post_interactions: Vec<Interaction>,
        pub sell_token_balance: SellTokenSource,
        pub buy_token_balance: BuyTokenDestination,
        pub class: Class,
        pub app_data: AppDataHash,
        #[serde(flatten)]
        pub signature: Signature,
        /// The types of fees that will be collected by the protocol.
        /// Multiple fees are applied in the order they are listed
        pub fee_policies: Vec<FeePolicy>,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub enum Class {
        Market,
        Limit,
        Liquidity,
    }

    #[serde_as]
    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Interaction {
        pub target: H160,
        #[serde_as(as = "HexOrDecimalU256")]
        pub value: U256,
        #[serde_as(as = "BytesHex")]
        pub call_data: Vec<u8>,
    }

    #[derive(Clone, Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub enum FeePolicy {
        /// If the order receives more than expected (positive deviation from
        /// quoted amounts) pay the protocol a factor of the achieved
        /// improvement. The fee is taken in `sell` token for `buy`
        /// orders and in `buy` token for `sell` orders.
        #[serde(rename_all = "camelCase")]
        PriceImprovement {
            /// Factor of price improvement the protocol charges as a fee.
            /// Price improvement is the difference between executed price and
            /// limit price or quoted price (whichever is better)
            ///
            /// E.g. if a user received 2000USDC for 1ETH while having been
            /// quoted 1990USDC, their price improvement is 10USDC.
            /// A factor of 0.5 requires the solver to pay 5USDC to
            /// the protocol for settling this order.
            factor: f64,
            /// Cap protocol fee with a percentage of the order's volume.
            max_volume_factor: f64,
        },
        /// How much of the order's volume should be taken as a protocol fee.
        /// The fee is taken in `sell` token for `sell` orders and in `buy`
        /// token for `buy` orders.
        #[serde(rename_all = "camelCase")]
        Volume {
            /// Percentage of the order's volume should be taken as a protocol
            /// fee.
            factor: f64,
        },
    }

    pub fn fee_policy_to_dto(fee_policy: &arguments::FeePolicy) -> FeePolicy {
        match fee_policy.fee_policy_kind {
            arguments::FeePolicyKind::PriceImprovement {
                factor: price_improvement_factor,
                max_volume_factor,
            } => FeePolicy::PriceImprovement {
                factor: price_improvement_factor,
                max_volume_factor,
            },
            arguments::FeePolicyKind::Volume { factor } => FeePolicy::Volume { factor },
        }
    }

    #[serde_as]
    #[derive(Clone, Debug, Default, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct TradedAmounts {
        /// The effective amount that left the user's wallet including all fees.
        #[serde_as(as = "HexOrDecimalU256")]
        pub sell_amount: U256,
        /// The effective amount the user received after all fees.
        #[serde_as(as = "HexOrDecimalU256")]
        pub buy_amount: U256,
    }

    #[serde_as]
    #[derive(Clone, Debug, Default, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct Solution {
        /// Unique ID of the solution (per driver competition), used to identify
        /// it in subsequent requests (reveal, settle).
        #[serde_as(as = "serde_with::DisplayFromStr")]
        pub solution_id: u64,
        #[serde_as(as = "HexOrDecimalU256")]
        pub score: U256,
        /// Address used by the driver to submit the settlement onchain.
        pub submission_address: H160,
        pub orders: HashMap<OrderUid, TradedAmounts>,
        #[serde_as(as = "HashMap<_, HexOrDecimalU256>")]
        pub clearing_prices: HashMap<H160, U256>,
    }

    #[derive(Clone, Debug, Default, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct Response {
        pub solutions: Vec<Solution>,
    }
}

pub mod reveal {
    use {
        model::bytes_hex,
        serde::{Deserialize, Serialize},
        serde_with::serde_as,
    };

    #[serde_as]
    #[derive(Clone, Debug, Default, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Request {
        /// Unique ID of the solution (per driver competition), to reveal.
        #[serde_as(as = "serde_with::DisplayFromStr")]
        pub solution_id: u64,
    }

    #[serde_as]
    #[derive(Clone, Debug, Default, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct Calldata {
        #[serde(with = "bytes_hex")]
        pub internalized: Vec<u8>,
        #[serde(with = "bytes_hex")]
        pub uninternalized: Vec<u8>,
    }

    #[derive(Clone, Debug, Default, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct Response {
        pub calldata: Calldata,
    }
}

pub mod settle {
    use {
        model::bytes_hex,
        primitive_types::H256,
        serde::{Deserialize, Serialize},
        serde_with::serde_as,
    };

    #[serde_as]
    #[derive(Clone, Debug, Default, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Request {
        /// Unique ID of the solution (per driver competition), to settle.
        #[serde_as(as = "serde_with::DisplayFromStr")]
        pub solution_id: u64,
    }

    #[serde_as]
    #[derive(Clone, Debug, Default, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct Response {
        pub calldata: Calldata,
        pub tx_hash: H256,
    }

    #[serde_as]
    #[derive(Clone, Debug, Default, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct Calldata {
        #[serde(with = "bytes_hex")]
        pub internalized: Vec<u8>,
        #[serde(with = "bytes_hex")]
        pub uninternalized: Vec<u8>,
    }
}
