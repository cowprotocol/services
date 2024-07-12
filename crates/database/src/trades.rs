use {
    crate::{fee_policies::FeePolicy, Address, OrderUid, TransactionHash},
    bigdecimal::BigDecimal,
    futures::stream::BoxStream,
    serde_json::Value,
    sqlx::{PgConnection, Row},
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TradesQueryRow {
    pub block_number: i64,
    pub log_index: i64,
    pub order_uid: OrderUid,
    pub buy_amount: BigDecimal,
    pub sell_amount: BigDecimal,
    pub sell_amount_before_fees: BigDecimal,
    pub owner: Address,
    pub buy_token: Address,
    pub sell_token: Address,
    pub tx_hash: Option<TransactionHash>,
    pub fee_policies: Vec<FeePolicy>,
    pub quote: Option<Quote>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Quote {
    pub sell_amount: BigDecimal,
    pub buy_amount: BigDecimal,
    pub fee: f64,
}

impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for TradesQueryRow {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        let fee_policies: Value = row.try_get("fee_policies")?;
        let fee_policies: Vec<FeePolicy> =
            serde_json::from_value(fee_policies).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
        let quote: (Option<BigDecimal>, Option<BigDecimal>, Option<f64>) = (
            row.try_get("quote_sell_amount").ok(),
            row.try_get("quote_buy_amount").ok(),
            row.try_get("quote_fee").ok(),
        );
        let quote = match quote {
            (Some(sell_amount), Some(buy_amount), Some(fee)) => Some(Quote {
                sell_amount,
                buy_amount,
                fee,
            }),
            _ => None,
        };

        Ok(TradesQueryRow {
            block_number: row.try_get("block_number")?,
            log_index: row.try_get("log_index")?,
            order_uid: row.try_get("order_uid")?,
            buy_amount: row.try_get("buy_amount")?,
            sell_amount: row.try_get("sell_amount")?,
            sell_amount_before_fees: row.try_get("sell_amount_before_fees")?,
            owner: row.try_get("owner")?,
            buy_token: row.try_get("buy_token")?,
            sell_token: row.try_get("sell_token")?,
            tx_hash: row.try_get("tx_hash")?,
            fee_policies,
            quote,
        })
    }
}

pub fn trades<'a>(
    ex: &'a mut PgConnection,
    owner_filter: Option<&'a Address>,
    order_uid_filter: Option<&'a OrderUid>,
) -> BoxStream<'a, Result<TradesQueryRow, sqlx::Error>> {
    const COMMON_QUERY: &str = r#"
SELECT
    t.block_number,
    t.log_index,
    t.order_uid,
    t.buy_amount,
    t.sell_amount,
    t.sell_amount - t.fee_amount as sell_amount_before_fees,
    o.owner,
    o.buy_token,
    o.sell_token,
    settlement.tx_hash,
    COALESCE(json_agg(
            json_build_object(
                'auction_id', fp.auction_id,
                'order_uid', fp.order_uid,
                'kind', fp.kind,
                'surplus_factor', fp.surplus_factor,
                'surplus_max_volume_factor', fp.surplus_max_volume_factor,
                'volume_factor', fp.volume_factor,
                'price_improvement_factor', fp.price_improvement_factor,
                'price_improvement_max_volume_factor', fp.price_improvement_max_volume_factor
            ) ORDER BY fp.application_order
        ) FILTER (WHERE fp.auction_id IS NOT NULL), '[]') AS fee_policies,
    oq.sell_amount AS quote_sell_amount,
    oq.buy_amount AS quote_buy_amount,
    (oq.gas_amount * oq.gas_price / oq.sell_token_price) AS quote_fee
FROM trades t
LEFT OUTER JOIN LATERAL (
    SELECT tx_hash, auction_id FROM settlements s
    WHERE s.block_number = t.block_number
    AND   s.log_index > t.log_index
    ORDER BY s.log_index ASC
    LIMIT 1
) AS settlement ON true
JOIN orders o ON o.uid = t.order_uid
LEFT JOIN fee_policies fp ON fp.auction_id = settlement.auction_id AND fp.order_uid = t.order_uid
LEFT JOIN order_quotes oq ON oq.order_uid = t.order_uid"#;
    const QUERY: &str = const_format::concatcp!(
        "WITH combined AS (",
        COMMON_QUERY,
        " WHERE ($1 IS NULL OR o.owner = $1)",
        " AND ($2 IS NULL OR o.uid = $2)",
        " GROUP BY t.block_number, t.log_index, t.order_uid, o.owner, o.buy_token, o.sell_token, \
         settlement.tx_hash, oq.sell_amount, oq.buy_amount, oq.gas_amount, oq.gas_price, \
         oq.sell_token_price ",
        " UNION ALL ",
        COMMON_QUERY,
        " LEFT OUTER JOIN onchain_placed_orders onchain_o",
        " ON onchain_o.uid = t.order_uid",
        " WHERE onchain_o.sender = $1",
        " AND ($2 IS NULL OR o.uid = $2)",
        " GROUP BY t.block_number, t.log_index, t.order_uid, o.owner, o.buy_token, o.sell_token, \
         settlement.tx_hash, oq.sell_amount, oq.buy_amount, oq.gas_amount, oq.gas_price, \
         oq.sell_token_price ",
        ") ",
        "SELECT DISTINCT ON (block_number, log_index, order_uid) * FROM combined ",
        "ORDER BY block_number DESC"
    );

    sqlx::query_as(QUERY)
        .bind(owner_filter)
        .bind(order_uid_filter)
        .fetch(ex)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            auction::AuctionId,
            byte_array::ByteArray,
            events::{Event, EventIndex, Settlement, Trade},
            fee_policies::{FeePolicy, FeePolicyKind},
            onchain_broadcasted_orders::{insert_onchain_order, OnchainOrderPlacement},
            orders::Order,
            PgTransaction,
        },
        futures::TryStreamExt,
        sqlx::Connection,
    };

    async fn generate_owners_and_order_ids(
        num_owners: usize,
        num_orders: usize,
    ) -> (Vec<Address>, Vec<OrderUid>) {
        let owners: Vec<Address> = (0..num_owners).map(|t| ByteArray([t as u8; 20])).collect();
        let order_ids: Vec<OrderUid> = (0..num_orders).map(|i| ByteArray([i as u8; 56])).collect();
        (owners, order_ids)
    }

    async fn add_trade(
        ex: &mut PgTransaction<'_>,
        owner: Address,
        order_uid: OrderUid,
        event_index: EventIndex,
        tx_hash: Option<TransactionHash>,
    ) -> TradesQueryRow {
        crate::events::append(
            ex,
            &[(
                event_index,
                Event::Trade(Trade {
                    order_uid: ByteArray(order_uid.0),
                    ..Default::default()
                }),
            )],
        )
        .await
        .unwrap();
        TradesQueryRow {
            block_number: event_index.block_number,
            log_index: event_index.log_index,
            order_uid,
            owner,
            tx_hash,
            ..Default::default()
        }
    }

    async fn add_order_and_trade(
        ex: &mut PgTransaction<'_>,
        owner: Address,
        order_uid: OrderUid,
        event_index: EventIndex,
        tx_hash: Option<TransactionHash>,
    ) -> TradesQueryRow {
        let order = Order {
            uid: order_uid,
            owner,
            ..Default::default()
        };
        crate::orders::insert_order(ex, &order).await.unwrap();
        add_trade(ex, owner, order_uid, event_index, tx_hash).await
    }

    async fn assert_trades(
        db: &mut PgConnection,
        owner_filter: Option<&Address>,
        order_uid_filter: Option<&OrderUid>,
        expected: &[TradesQueryRow],
    ) {
        let filtered = trades(db, owner_filter, order_uid_filter)
            .try_collect::<Vec<_>>()
            .await
            .unwrap();
        assert_eq!(filtered, expected);
    }

    // Testing trades without corresponding settlement events
    #[tokio::test]
    #[ignore]
    async fn postgres_trades_without_filter() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let (owners, order_ids) = generate_owners_and_order_ids(2, 2).await;
        assert_trades(&mut db, None, None, &[]).await;
        let event_index_a = EventIndex {
            block_number: 0,
            log_index: 0,
        };
        let trade_a =
            add_order_and_trade(&mut db, owners[0], order_ids[0], event_index_a, None).await;
        assert_trades(&mut db, None, None, &[trade_a.clone()]).await;

        let event_index_b = EventIndex {
            block_number: 1,
            log_index: 0,
        };
        let trade_b =
            add_order_and_trade(&mut db, owners[0], order_ids[1], event_index_b, None).await;
        assert_trades(&mut db, None, None, &[trade_b, trade_a]).await;
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trades_with_owner_filter_benchmark_test() {
        // This test can be used for benchmarking. With i in 0..240
        // and j 0..100, the query should be less than 5 ms.
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();
        for i in 0..1u32 {
            let mut owner_bytes = i.to_ne_bytes().to_vec();
            owner_bytes.append(&mut vec![0; 20 - owner_bytes.len()]);
            let owner = ByteArray(owner_bytes.try_into().unwrap());
            for j in 0..1u32 {
                let mut i_as_bytes = i.to_ne_bytes().to_vec();
                let mut j_as_bytes = j.to_ne_bytes().to_vec();
                let mut order_uid_info = vec![0; 56 - i_as_bytes.len() - j_as_bytes.len()];
                order_uid_info.append(&mut j_as_bytes);
                i_as_bytes.append(&mut order_uid_info);
                let event_index_0 = EventIndex {
                    block_number: 0,
                    log_index: 0,
                };
                let order_uid = ByteArray(i_as_bytes.try_into().unwrap());
                insert_onchain_order(
                    &mut db,
                    &event_index_0.clone(),
                    &OnchainOrderPlacement {
                        order_uid,
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
                add_order_and_trade(&mut db, owner, order_uid, event_index_0, None).await;
            }
        }

        let now = std::time::Instant::now();
        trades(&mut db, Some(&ByteArray([2u8; 20])), None)
            .try_collect::<Vec<_>>()
            .await
            .unwrap();
        let elapsed = now.elapsed();
        println!("{elapsed:?}");
        assert!(elapsed < std::time::Duration::from_secs(1));
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trades_with_owner_filter() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let (owners, order_ids) = generate_owners_and_order_ids(4, 4).await;

        let event_index_0 = EventIndex {
            block_number: 0,
            log_index: 0,
        };
        let trade_0 =
            add_order_and_trade(&mut db, owners[0], order_ids[0], event_index_0, None).await;

        let event_index_1 = EventIndex {
            block_number: 0,
            log_index: 1,
        };
        let trade_1 =
            add_order_and_trade(&mut db, owners[1], order_ids[1], event_index_1, None).await;

        assert_trades(&mut db, Some(&owners[0]), None, &[trade_0.clone()]).await;
        assert_trades(&mut db, Some(&owners[1]), None, &[trade_1]).await;
        assert_trades(&mut db, Some(&owners[2]), None, &[]).await;

        let onchain_order = OnchainOrderPlacement {
            order_uid: ByteArray(order_ids[0].0),
            sender: owners[3],
            placement_error: None,
        };
        let event_index = EventIndex::default();
        insert_onchain_order(&mut db, &event_index, &onchain_order)
            .await
            .unwrap();
        assert_trades(&mut db, Some(&owners[3]), None, &[trade_0.clone()]).await;

        add_order_and_trade(&mut db, owners[3], order_ids[3], event_index_1, None).await;
        let onchain_order = OnchainOrderPlacement {
            order_uid: ByteArray(order_ids[3].0),
            sender: owners[3],
            placement_error: None,
        };
        insert_onchain_order(&mut db, &event_index_1, &onchain_order)
            .await
            .unwrap();
        assert_trades(&mut db, Some(&owners[3]), None, &[trade_0]).await;
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trades_with_order_uid_filter() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let (owners, order_ids) = generate_owners_and_order_ids(2, 3).await;

        let event_index_0 = EventIndex {
            block_number: 0,
            log_index: 0,
        };
        let trade_0 =
            add_order_and_trade(&mut db, owners[0], order_ids[0], event_index_0, None).await;

        let event_index_1 = EventIndex {
            block_number: 0,
            log_index: 1,
        };
        let trade_1 =
            add_order_and_trade(&mut db, owners[1], order_ids[1], event_index_1, None).await;

        assert_trades(&mut db, None, Some(&order_ids[0]), &[trade_0]).await;
        assert_trades(&mut db, None, Some(&order_ids[1]), &[trade_1]).await;
        assert_trades(&mut db, None, Some(&order_ids[2]), &[]).await;
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trade_without_matching_order() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let (owners, order_ids) = generate_owners_and_order_ids(1, 1).await;

        let event_index = EventIndex {
            block_number: 0,
            log_index: 0,
        };
        add_trade(&mut db, owners[0], order_ids[0], event_index, None).await;
        // Trade exists in DB but no matching order
        assert_trades(&mut db, None, Some(&order_ids[0]), &[]).await;
        assert_trades(&mut db, Some(&owners[0]), None, &[]).await;
    }

    // Testing Trades with settlements
    async fn add_settlement(
        ex: &mut PgTransaction<'_>,
        event_index: EventIndex,
        solver: Address,
        transaction_hash: TransactionHash,
        auction_id: AuctionId,
    ) -> Settlement {
        let settlement = Settlement {
            solver,
            transaction_hash,
        };
        crate::events::append(ex, &[(event_index, Event::Settlement(settlement))])
            .await
            .unwrap();
        crate::settlements::update_settlement_auction(
            ex,
            event_index.block_number,
            event_index.log_index,
            auction_id,
        )
        .await
        .unwrap();
        settlement
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trades_having_same_settlement_with_and_without_orders() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let (owners, order_ids) = generate_owners_and_order_ids(2, 2).await;
        assert_trades(&mut db, None, None, &[]).await;

        let settlement = add_settlement(
            &mut db,
            EventIndex {
                block_number: 0,
                log_index: 4,
            },
            Default::default(),
            Default::default(),
            1,
        )
        .await;

        let trade_a = add_order_and_trade(
            &mut db,
            owners[0],
            order_ids[0],
            EventIndex {
                block_number: 0,
                log_index: 0,
            },
            Some(settlement.transaction_hash),
        )
        .await;
        assert_trades(&mut db, None, None, &[trade_a.clone()]).await;

        let trade_b = add_order_and_trade(
            &mut db,
            owners[0],
            order_ids[1],
            EventIndex {
                block_number: 0,
                log_index: 1,
            },
            Some(settlement.transaction_hash),
        )
        .await;
        assert_trades(&mut db, None, None, &[trade_a, trade_b]).await;
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trades_with_same_settlement_no_orders() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let (owners, order_ids) = generate_owners_and_order_ids(2, 2).await;
        assert_trades(&mut db, None, None, &[]).await;

        let settlement = add_settlement(
            &mut db,
            EventIndex {
                block_number: 0,
                log_index: 4,
            },
            Default::default(),
            Default::default(),
            1,
        )
        .await;

        add_trade(
            &mut db,
            owners[0],
            order_ids[0],
            EventIndex {
                block_number: 0,
                log_index: 0,
            },
            Some(settlement.transaction_hash),
        )
        .await;

        add_trade(
            &mut db,
            owners[0],
            order_ids[1],
            EventIndex {
                block_number: 0,
                log_index: 1,
            },
            Some(settlement.transaction_hash),
        )
        .await;
        // Trades query returns nothing when there are no corresponding orders.
        assert_trades(&mut db, None, None, &[]).await;
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trades_with_two_settlements_in_same_block() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let (owners, order_ids) = generate_owners_and_order_ids(2, 2).await;
        assert_trades(&mut db, None, None, &[]).await;

        let settlement_a = add_settlement(
            &mut db,
            EventIndex {
                block_number: 0,
                log_index: 1,
            },
            Default::default(),
            Default::default(),
            1,
        )
        .await;
        let settlement_b = add_settlement(
            &mut db,
            EventIndex {
                block_number: 0,
                log_index: 3,
            },
            Default::default(),
            ByteArray([2; 32]),
            1,
        )
        .await;

        let trade_a = add_order_and_trade(
            &mut db,
            owners[0],
            order_ids[0],
            EventIndex {
                block_number: 0,
                log_index: 0,
            },
            Some(settlement_a.transaction_hash),
        )
        .await;
        assert_trades(&mut db, None, None, &[trade_a.clone()]).await;

        let trade_b = add_order_and_trade(
            &mut db,
            owners[0],
            order_ids[1],
            EventIndex {
                block_number: 0,
                log_index: 2,
            },
            Some(settlement_b.transaction_hash),
        )
        .await;
        assert_trades(&mut db, None, None, &[trade_a, trade_b]).await;
    }

    // Testing trades with fee policies
    async fn add_fee_policies(
        ex: &mut PgTransaction<'_>,
        trade: &mut TradesQueryRow,
        fee_policies: Vec<FeePolicy>,
    ) {
        crate::fee_policies::insert_batch(ex, fee_policies.clone())
            .await
            .unwrap();
        trade.fee_policies = fee_policies;
    }

    async fn add_quote(
        ex: &mut PgTransaction<'_>,
        trade: &mut TradesQueryRow,
        quote: crate::orders::Quote,
    ) {
        crate::orders::insert_quote(ex, &quote).await.unwrap();
        let quote = Quote {
            sell_amount: quote.sell_amount,
            buy_amount: quote.buy_amount,
            fee: quote.gas_amount * quote.gas_price / quote.sell_token_price,
        };
        trade.quote = Some(quote);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trades_with_and_without_fee_policies() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let (owners, order_ids) = generate_owners_and_order_ids(2, 2).await;
        assert_trades(&mut db, None, None, &[]).await;

        let settlement = add_settlement(
            &mut db,
            EventIndex {
                block_number: 0,
                log_index: 4,
            },
            Default::default(),
            Default::default(),
            1,
        )
        .await;

        let mut trade_a = add_order_and_trade(
            &mut db,
            owners[0],
            order_ids[0],
            EventIndex {
                block_number: 0,
                log_index: 0,
            },
            Some(settlement.transaction_hash),
        )
        .await;
        add_fee_policies(
            &mut db,
            &mut trade_a,
            vec![
                FeePolicy {
                    auction_id: 1,
                    order_uid: order_ids[0],
                    kind: FeePolicyKind::Surplus,
                    surplus_factor: Some(0.1),
                    surplus_max_volume_factor: Some(0.99999),
                    volume_factor: None,
                    price_improvement_factor: None,
                    price_improvement_max_volume_factor: None,
                },
                FeePolicy {
                    auction_id: 1,
                    order_uid: order_ids[0],
                    kind: FeePolicyKind::Volume,
                    surplus_factor: None,
                    surplus_max_volume_factor: None,
                    volume_factor: Some(0.1),
                    price_improvement_factor: None,
                    price_improvement_max_volume_factor: None,
                },
                FeePolicy {
                    auction_id: 1,
                    order_uid: order_ids[0],
                    kind: FeePolicyKind::PriceImprovement,
                    surplus_factor: None,
                    surplus_max_volume_factor: None,
                    volume_factor: None,
                    price_improvement_factor: Some(0.1),
                    price_improvement_max_volume_factor: Some(0.99999),
                },
            ],
        )
        .await;
        add_quote(
            &mut db,
            &mut trade_a,
            crate::orders::Quote {
                order_uid: order_ids[0],
                gas_amount: 6.0,
                gas_price: 2.0,
                sell_token_price: 4.0,
                sell_amount: BigDecimal::from(100),
                buy_amount: BigDecimal::from(100),
                solver: owners[0],
            },
        )
        .await;
        assert_trades(&mut db, None, None, &[trade_a.clone()]).await;

        let trade_b = add_order_and_trade(
            &mut db,
            owners[0],
            order_ids[1],
            EventIndex {
                block_number: 0,
                log_index: 1,
            },
            Some(settlement.transaction_hash),
        )
        .await;
        assert_trades(&mut db, None, None, &[trade_a, trade_b]).await;
    }
}
