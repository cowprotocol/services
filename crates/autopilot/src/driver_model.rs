//! Types for communicating with drivers as defined in `crates/driver/openapi.yml`.

pub mod quote {
    use model::u256_decimal;
    use primitive_types::{H160, U256};
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Default, Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Request {
        pub sell_token: H160,
        pub buy_token: H160,
        pub kind: Kind,
        #[serde(with = "u256_decimal")]
        pub amount: U256,
    }

    #[derive(Clone, Debug, Default, Deserialize, Serialize)]
    #[serde(rename_all = "lowercase")]
    pub enum Kind {
        #[default]
        Buy,
        Sell,
    }

    #[derive(Clone, Debug, Deserialize, Serialize)]
    #[serde(untagged, rename_all = "camelCase")]
    pub enum Response {
        Successful {
            #[serde(with = "u256_decimal")]
            sell_amount: U256,
            #[serde(with = "u256_decimal")]
            buy_amount: U256,
            gas: u64,
        },
        Unfillable {
            unfillable_reason: String,
        },
    }
}

pub mod solve {
    use chrono::{DateTime, Utc};
    use primitive_types::{H160, U256};
    use serde::{Deserialize, Serialize};
    use serde_with::{serde_as, DisplayFromStr};
    use std::collections::BTreeMap;

    #[derive(Clone, Debug, Default, Deserialize, Serialize)]
    pub struct Request {
        pub auction: Auction,
        pub deadline: DateTime<Utc>,
    }

    #[serde_as]
    #[derive(Clone, Debug, Default, Deserialize, Serialize)]
    pub struct Auction {
        pub id: i64,
        pub block: u64,
        pub orders: Vec<Order>,
        #[serde_as(as = "BTreeMap<_, DisplayFromStr>")]
        pub prices: BTreeMap<H160, U256>,
    }

    #[derive(Clone, Debug, Default, Deserialize, Serialize)]
    pub struct Order {
        // TODO: what fields? Needs to be documented in openapi too.
    }

    #[derive(Clone, Debug, Default, Deserialize, Serialize)]
    pub struct Response {
        pub objective: f64,
        pub signature: String,
    }
}

pub mod execute {
    use model::{bytes_hex, order::OrderUid, u256_decimal};
    use primitive_types::{H160, U256};
    use serde::{Deserialize, Serialize};
    use serde_with::{serde_as, DisplayFromStr};
    use std::collections::BTreeMap;

    #[derive(Clone, Debug, Default, Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Request {
        pub auction_id: i64,
        #[serde(with = "bytes_hex")]
        pub transaction_identifier: Vec<u8>,
    }

    #[serde_as]
    #[derive(Clone, Debug, Default, Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Response {
        pub account: H160,
        pub nonce: u64,
        #[serde_as(as = "BTreeMap<_, DisplayFromStr>")]
        pub clearing_prices: BTreeMap<H160, U256>,
        pub trades: Vec<Trade>,
        pub internalized_interactions: Vec<InternalizedInteraction>,
        #[serde(with = "bytes_hex")]
        pub calldata: Vec<u8>,
        pub signature: String,
    }

    #[derive(Clone, Debug, Default, Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Trade {
        pub uid: OrderUid,
        #[serde(with = "u256_decimal")]
        pub executed_amount: U256,
    }

    #[serde_as]
    #[derive(Clone, Debug, Default, Deserialize, Serialize)]
    pub struct InternalizedInteraction {
        #[serde(with = "bytes_hex")]
        pub calldata: Vec<u8>,
        #[serde_as(as = "BTreeMap<_, DisplayFromStr>")]
        pub inputs: BTreeMap<H160, U256>,
        #[serde_as(as = "BTreeMap<_, DisplayFromStr>")]
        pub outputs: BTreeMap<H160, U256>,
    }
}
