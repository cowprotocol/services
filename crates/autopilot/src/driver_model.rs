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
    #[serde(rename_all = "lowercase")]
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
    #[derive(Clone, Debug, Serialize, Deserialize)]
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
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(rename_all = "lowercase")]
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
    }

    #[derive(Clone, Debug, Default, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct Response {
        pub solutions: Vec<Solution>,
    }
}

pub mod reveal {
    use {
        model::{bytes_hex, order::OrderUid},
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
        pub orders: Vec<OrderUid>,
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
