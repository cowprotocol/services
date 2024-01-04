use anyhow::Result;

impl super::Postgres {
    pub async fn most_recent_auction(&self) -> Result<Option<dto::AuctionWithId>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["load_most_recent_auction"])
            .start_timer();

        let mut ex = self.pool.acquire().await?;
        let (id, json) = match database::auction::load_most_recent(&mut ex).await? {
            Some(inner) => inner,
            None => return Ok(None),
        };
        let auction: dto::Auction = serde_json::from_value(json)?;
        let auction = dto::AuctionWithId { id, auction };
        Ok(Some(auction))
    }
}

pub mod dto {
    use {
        model::{
            app_data::AppDataHash,
            interaction::InteractionData,
            order::{BuyTokenDestination, OrderClass, OrderKind, OrderUid, SellTokenSource},
            signature::Signature,
        },
        number::serialization::HexOrDecimalU256,
        primitive_types::{H160, U256},
        serde::{Deserialize, Serialize},
        serde_with::serde_as,
        std::collections::BTreeMap,
    };

    #[serde_as]
    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Auction {
        /// The block that this auction is valid for.
        /// The block number for the auction. Orders and prices are guaranteed
        /// to be valid on this block.
        pub block: u64,

        /// The latest block on which a settlement has been processed. This
        /// field is used to tell which orders are still in-flight. See
        /// [`InFlightOrders`].
        ///
        /// Note that under certain conditions it is possible for a settlement
        /// to have been mined as part of [`block`] but not have yet
        /// been processed.
        pub latest_settlement_block: u64,

        /// The solvable orders included in the auction.
        pub orders: Vec<Order>,

        /// The reference prices for all traded tokens in the auction.
        #[serde_as(as = "BTreeMap<_, HexOrDecimalU256>")]
        pub prices: BTreeMap<H160, U256>,
    }

    pub type AuctionId = i64;

    #[serde_as]
    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct AuctionWithId {
        /// Increments whenever the backend updates the auction.
        pub id: AuctionId,
        #[serde(flatten)]
        pub auction: Auction,
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
        pub pre_interactions: Vec<InteractionData>,
        pub post_interactions: Vec<InteractionData>,
        pub sell_token_balance: SellTokenSource,
        pub buy_token_balance: BuyTokenDestination,
        pub class: OrderClass,
        pub app_data: AppDataHash,
        #[serde(flatten)]
        pub signature: Signature,
    }
}
