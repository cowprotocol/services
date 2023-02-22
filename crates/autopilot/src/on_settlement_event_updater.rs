//! This module is responsible for updating the database, for each settlement event
//! that is emitted by the settlement contract.

use {
    crate::{database::Postgres, decoded_settlement::DecodedSettlement},
    anyhow::{anyhow, Context, Result},
    contracts::GPv2Settlement,
    primitive_types::{H160, H256},
    shared::{
        current_block::CurrentBlockStream,
        ethrpc::Web3,
        event_handling::MAX_REORG_BLOCK_COUNT,
        external_prices::ExternalPrices,
    },
    std::time::Duration,
    web3::types::TransactionId,
};

pub struct OnSettlementEventUpdater {
    pub web3: Web3,
    pub contract: GPv2Settlement,
    pub native_token: H160,
    pub db: Postgres,
    pub current_block: CurrentBlockStream,
}

impl OnSettlementEventUpdater {
    pub async fn run_forever(self) -> ! {
        loop {
            match self.update().await {
                Ok(true) => (),
                Ok(false) => tokio::time::sleep(Duration::from_secs(10)).await,
                Err(err) => {
                    tracing::error!(?err, "auction transaction update task failed");
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            }
        }
    }

    /// Update a single settlement event.
    ///
    /// Returns whether an update was performed.
    async fn update(&self) -> Result<bool> {
        let current_block = self.current_block.borrow().number;
        let reorg_safe_block: u64 = current_block
            .checked_sub(MAX_REORG_BLOCK_COUNT)
            .context("no reorg safe block")?;
        let reorg_safe_block: i64 = reorg_safe_block.try_into().context("convert block")?;
        let event = match self
            .db
            .get_settlement_event_without_tx_info(reorg_safe_block)
            .await
            .context("get_settlement_event_without_tx_info")?
        {
            Some(event) => event,
            None => return Ok(false),
        };
        let hash = H256(event.tx_hash.0);
        tracing::trace!(?hash);

        // 1. Associating auction ids with transaction hashes.

        // see database/sql/V037__auction_transaction.sql
        //
        // When we put settlement transactions on chain there is no reliable way to
        // know the transaction hash because we can create multiple transactions with
        // different gas prices. What we do know is the account and nonce that the
        // transaction will have which is enough to uniquely identify it.
        //
        // We build an association between account-nonce and tx hash by backfilling
        // settlement events with the account and nonce of their tx hash. This happens
        // in an always running background task.
        //
        // Alternatively we could change the event insertion code to do this but I (vk)
        // would like to keep that code as fast as possible to not slow down event
        // insertion which also needs to deal with reorgs. It is also nicer from a code
        // organization standpoint.

        let transaction = self
            .web3
            .eth()
            .transaction(TransactionId::Hash(hash))
            .await
            .with_context(|| format!("get tx {hash:?}"))?
            .with_context(|| format!("no tx {hash:?}"))?;
        let from = transaction
            .from
            .with_context(|| format!("no from {hash:?}"))?;
        let nonce: i64 = transaction
            .nonce
            .try_into()
            .map_err(|err| anyhow!("{}", err))
            .with_context(|| format!("convert nonce {hash:?}"))?;

        self.db
            .update_settlement_tx_info(event.block_number, event.log_index, from, nonce)
            .await
            .context("update_settlement_tx_info")?;

        // 2. Inserting settlement observations
        //
        // see database/sql/V048__create_settlement_rewards.sql
        //
        // Surplus and fees calculation is based solely on the mined transaction call
        // data and the auction external prices. Auction external prices for all tokens
        // for all solvable orders are stored in the database (auction_prices table) in
        // autopilot before competition. After a transaction is mined we calculate the
        // surplus and fees for each transaction and insert them into the database
        // (settlement_observations table). Now we know which tokens were used in the
        // transaction so we update the auction_prices table with the actual prices
        // that were used.

        // TODO how to detect missed settlements? need to populate settlement
        // observation on event insertition.

        let tx_receipt = self
            .web3
            .eth()
            .transaction_receipt(hash)
            .await?
            .with_context(|| format!("no receipt {hash:?}"))?;
        let gas_used = tx_receipt
            .gas_used
            .with_context(|| format!("no gas used {hash:?}"))?;
        let effective_gas_price = tx_receipt
            .effective_gas_price
            .with_context(|| format!("no effective gas price {hash:?}"))?;
        let fee = primitive_types::U256::default(); // TODO
        let settlement = DecodedSettlement::new(&self.contract, &transaction.input.0)?;
        let auction_id = self
            .db
            .get_auction_id(from, nonce)
            .await?
            .context(format!("no auction id for tx {hash:?}"))?;
        let auction_external_prices =
            self.db
                .fetch_auction_prices(auction_id)
                .await
                .context(format!(
                    "no external prices for auction id {auction_id:?} and tx {hash:?}"
                ))?;
        let external_prices = ExternalPrices::try_from_auction_prices(
            self.native_token,
            auction_external_prices.clone(),
        )?;
        let surplus = settlement.total_surplus(&external_prices);

        self.db
            .insert_settlement_observation(
                event.block_number,
                event.log_index,
                gas_used,
                effective_gas_price,
                surplus,
                fee,
            )
            .await
            .context("insert_settlement_observation")?;

        // reduce external prices in `auction_prices` table to only include used tokens
        // this is done to reduce the amount of data we store in the database
        let reduced_external_prices = settlement
            .trades
            .iter()
            .filter_map(|trade| {
                let buy_token = settlement
                    .tokens
                    .get(trade.buy_token_index.as_u64() as usize)
                    .unwrap(); // TODO can I unwrap here? Is it guaranteed by the settlement contract?
                let sell_token = settlement
                    .tokens
                    .get(trade.sell_token_index.as_u64() as usize)
                    .unwrap(); // TODO can I unwrap here? Is it guaranteed by the settlement contract?
                let buy_token_price = auction_external_prices.get(buy_token);
                let sell_token_price = auction_external_prices.get(sell_token);
                match (buy_token_price, sell_token_price) {
                    (Some(buy_token_price), Some(sell_token_price)) => Some(vec![
                        (buy_token.clone(), buy_token_price.clone()),
                        (sell_token.clone(), sell_token_price.clone()),
                    ]),
                    _ => {
                        tracing::error!("settlement used token that was not in auction");
                        None
                    }
                }
            })
            .flatten()
            .collect::<std::collections::BTreeMap<_, _>>();
        self.db
            .insert_auction_prices(auction_id, &reduced_external_prices)
            .await?;

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, sqlx::Executor, std::sync::Arc};

    #[tokio::test]
    #[ignore]
    async fn manual_node_test() {
        // TODO update test
        shared::tracing::initialize_reentrant("autopilot=trace");
        let db = Postgres::new("postgresql://").await.unwrap();
        database::clear_DANGER(&db.0).await.unwrap();
        //let transport = shared::ethrpc::create_env_test_transport();
        let transport = shared::ethrpc::create_test_transport(
            "https://mainnet.infura.io/v3/3b497b3196e4468288eb5c7f239e86f4",
        );
        let web3 = Web3::new(transport);
        let current_block = shared::current_block::current_block_stream(
            Arc::new(web3.clone()),
            Duration::from_secs(1),
        )
        .await
        .unwrap();

        let contract = contracts::GPv2Settlement::deployed(&web3).await.unwrap();
        let native_token = contracts::WETH9::deployed(&web3).await.unwrap().address();
        let updater = OnSettlementEventUpdater {
            web3,
            db,
            native_token,
            current_block,
            contract,
        };

        assert!(!updater.update().await.unwrap());

        let query = r#"
INSERT INTO settlements (block_number, log_index, solver, tx_hash, tx_from, tx_nonce)
VALUES (15875801, 405, '\x', '\x0e9d0f4ea243ac0f02e1d3ecab3fea78108d83bfca632b30e9bc4acb22289c5a', NULL, NULL)
    ;"#;
        updater.db.0.execute(query).await.unwrap();

        let query = r#"
INSERT INTO solver_competitions (id, tx_hash)
VALUES (0, '\x0e9d0f4ea243ac0f02e1d3ecab3fea78108d83bfca632b30e9bc4acb22289c5a')
    ;"#;
        updater.db.0.execute(query).await.unwrap();

        assert!(updater.update().await.unwrap());

        let query = r#"
SELECT tx_from, tx_nonce
FROM settlements
WHERE block_number = 15875801 AND log_index = 405
        ;"#;
        let (tx_from, tx_nonce): (Vec<u8>, i64) = sqlx::query_as(query)
            .fetch_one(&updater.db.0)
            .await
            .unwrap();
        assert_eq!(
            tx_from,
            hex_literal::hex!("a21740833858985e4d801533a808786d3647fb83")
        );
        assert_eq!(tx_nonce, 4701);

        let query = r#"
SELECT auction_id, tx_from, tx_nonce
FROM auction_transaction
        ;"#;
        let (auction_id, tx_from, tx_nonce): (i64, Vec<u8>, i64) = sqlx::query_as(query)
            .fetch_one(&updater.db.0)
            .await
            .unwrap();
        assert_eq!(auction_id, 0);
        assert_eq!(
            tx_from,
            hex_literal::hex!("a21740833858985e4d801533a808786d3647fb83")
        );
        assert_eq!(tx_nonce, 4701);

        assert!(!updater.update().await.unwrap());
    }
}
