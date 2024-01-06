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
        crate::domain,
        chrono::{DateTime, Utc},
        model::{
            app_data::AppDataHash,
            bytes_hex::BytesHex,
            order::{BuyTokenDestination, OrderClass, OrderKind, OrderUid, SellTokenSource},
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
        pub class: OrderClass,
        pub app_data: AppDataHash,
        #[serde(flatten)]
        pub signature: Signature,
        /// The types of fees that will be collected by the protocol.
        /// Multiple fees are applied in the order they are listed
        pub fee_policies: Vec<FeePolicy>,
    }

    impl From<domain::auction::order::Order> for Order {
        fn from(order: domain::auction::order::Order) -> Self {
            Self {
                uid: order.uid.into(),
                sell_token: order.sell_token,
                buy_token: order.buy_token,
                sell_amount: order.sell_amount,
                buy_amount: order.buy_amount,
                solver_fee: order.solver_fee,
                user_fee: order.user_fee,
                valid_to: order.valid_to,
                kind: order.kind.into(),
                receiver: order.receiver,
                owner: order.owner,
                partially_fillable: order.partially_fillable,
                executed: order.executed,
                pre_interactions: order.pre_interactions.into_iter().map(Into::into).collect(),
                post_interactions: order
                    .post_interactions
                    .into_iter()
                    .map(Into::into)
                    .collect(),
                sell_token_balance: order.sell_token_balance.into(),
                buy_token_balance: order.buy_token_balance.into(),
                class: order.class.into(),
                app_data: order.app_data.into(),
                signature: order.signature.into(),
                fee_policies: order.fee_policies.into_iter().map(Into::into).collect(),
            }
        }
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

    impl From<domain::auction::order::Interaction> for Interaction {
        fn from(interaction: domain::auction::order::Interaction) -> Self {
            Self {
                target: interaction.target,
                value: interaction.value,
                call_data: interaction.call_data,
            }
        }
    }

    #[derive(Clone, Debug, Serialize)]
    #[serde(rename_all = "camelCase", tag = "kind")]
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

    impl From<domain::fee::Policy> for FeePolicy {
        fn from(policy: domain::fee::Policy) -> Self {
            match policy {
                domain::fee::Policy::PriceImprovement {
                    factor,
                    max_volume_factor,
                } => FeePolicy::PriceImprovement {
                    factor,
                    max_volume_factor,
                },
                domain::fee::Policy::Volume { factor } => FeePolicy::Volume { factor },
            }
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
