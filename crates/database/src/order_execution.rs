use {
    crate::{Address, OrderUid, auction::AuctionId},
    bigdecimal::BigDecimal,
    sqlx::{PgConnection, QueryBuilder},
    std::collections::HashMap,
};

type Execution = (AuctionId, OrderUid);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Asset {
    pub amount: BigDecimal,
    pub token: Address,
}

pub async fn save(
    ex: &mut PgConnection,
    order: &OrderUid,
    auction: AuctionId,
    block_number: i64,
    executed_fee: Asset,
    protocol_fees: &[Asset],
) -> Result<(), sqlx::Error> {
    let (protocol_fee_tokens, protocol_fee_amounts): (Vec<_>, Vec<_>) = protocol_fees
        .iter()
        .map(|entry| (entry.token, entry.amount.clone()))
        .unzip();

    const QUERY: &str = r#"
INSERT INTO order_execution (order_uid, auction_id, reward, executed_fee, executed_fee_token, block_number, protocol_fee_tokens, protocol_fee_amounts)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
ON CONFLICT (order_uid, auction_id)
DO UPDATE SET reward = $3, executed_fee = $4, executed_fee_token = $5, block_number = $6, protocol_fee_tokens = $7, protocol_fee_amounts = $8
;"#;
    sqlx::query(QUERY)
        .bind(order)
        .bind(auction)
        .bind(0.) // reward is deprecated but saved for historical analysis
        .bind(executed_fee.amount)
        .bind(executed_fee.token)
        .bind(block_number)
        .bind(protocol_fee_tokens)
        .bind(protocol_fee_amounts)
        .execute(ex)
        .await?;
    Ok(())
}

/// Fetch protocol fees for all keys in the filter
pub async fn executed_protocol_fees(
    ex: &mut PgConnection,
    keys_filter: &[Execution],
) -> Result<HashMap<Execution, Vec<Asset>>, sqlx::Error> {
    if keys_filter.is_empty() {
        return Ok(HashMap::new());
    }

    let mut query_builder = QueryBuilder::new(
        "SELECT oe.order_uid, oe.auction_id, oe.protocol_fee_tokens, oe.protocol_fee_amounts FROM \
         order_execution oe INNER JOIN (VALUES ",
    );

    for (i, (auction_id, order_uid)) in keys_filter.iter().enumerate() {
        if i > 0 {
            query_builder.push(", ");
        }
        query_builder
            .push("(")
            .push_bind(order_uid)
            .push(", ")
            .push_bind(auction_id)
            .push(")");
    }

    query_builder.push(") AS vals(order_uid, auction_id) ");
    query_builder.push("ON (oe.order_uid, oe.auction_id) = (vals.order_uid, vals.auction_id)");

    #[derive(Clone, Debug, Eq, PartialEq, sqlx::Type, sqlx::FromRow)]
    struct ProtocolFees {
        pub order_uid: OrderUid,
        pub auction_id: AuctionId,
        pub protocol_fee_tokens: Vec<Address>,
        pub protocol_fee_amounts: Vec<BigDecimal>,
    }
    let query = query_builder.build_query_as::<ProtocolFees>();
    let rows: Vec<ProtocolFees> = query.fetch_all(ex).await?;

    let mut fees = HashMap::new();
    for row in rows {
        fees.insert(
            (row.auction_id, row.order_uid),
            row.protocol_fee_tokens
                .into_iter()
                .zip(row.protocol_fee_amounts)
                .map(|(token, amount)| Asset { token, amount })
                .collect(),
        );
    }

    Ok(fees)
}

#[cfg(test)]
mod tests {
    use {super::*, sqlx::Connection};

    #[tokio::test]
    #[ignore]
    async fn postgres_save() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        // save entry with protocol fees
        let protocol_fees = vec![
            Asset {
                amount: BigDecimal::from(1),
                token: Default::default(),
            },
            Asset {
                amount: BigDecimal::from(2),
                token: Default::default(),
            },
        ];
        save(
            &mut db,
            &Default::default(),
            1,
            0,
            Asset {
                amount: BigDecimal::from(3),
                token: Default::default(),
            },
            protocol_fees.as_slice(),
        )
        .await
        .unwrap();

        // save entry for an order without protocol fees
        save(
            &mut db,
            &Default::default(),
            2,
            0,
            Asset {
                amount: BigDecimal::from(3),
                token: Default::default(),
            },
            &[],
        )
        .await
        .unwrap();

        let keys: Vec<(AuctionId, OrderUid)> = vec![
            (1, Default::default()),
            (2, Default::default()),
            (3, Default::default()),
        ];

        let read_protocol_fees = executed_protocol_fees(&mut db, &keys).await.unwrap();
        assert_eq!(read_protocol_fees.len(), 2);
        assert_eq!(read_protocol_fees[&(1, Default::default())], protocol_fees);
    }
}
