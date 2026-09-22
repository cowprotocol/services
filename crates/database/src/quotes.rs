use {
    crate::{Address, orders::OrderKind},
    bigdecimal::BigDecimal,
    sqlx::{
        PgConnection,
        types::chrono::{DateTime, Utc},
    },
    tracing::instrument,
};

pub type QuoteId = i64;

#[derive(Clone, Debug, Default, Eq, PartialEq, sqlx::Type)]
#[sqlx(type_name = "QuoteKind")]
#[sqlx(rename_all = "lowercase")]
pub enum QuoteKind {
    #[default]
    Standard,
    Eip1271OnchainOrder,
    PreSignOnchainOrder,
}

/// One row in the `quotes` table.
#[derive(Clone, Debug, PartialEq, sqlx::FromRow)]
pub struct Quote {
    pub id: QuoteId,
    pub sell_token: Address,
    pub buy_token: Address,
    pub sell_amount: BigDecimal,
    pub buy_amount: BigDecimal,
    pub gas_amount: f64,
    pub gas_price: f64,
    pub sell_token_price: f64,
    pub order_kind: OrderKind,
    pub expiration_timestamp: DateTime<Utc>,
    pub quote_kind: QuoteKind,
    pub solver: Address,
    pub verified: bool,
    pub metadata: serde_json::Value,
}

/// Allocates the id of the next quote from the `quotes` id sequence, so the
/// id is known before the quote is computed and stored.
#[instrument(skip_all)]
pub async fn next_id(ex: &mut PgConnection) -> Result<QuoteId, sqlx::Error> {
    const QUERY: &str = r#"SELECT nextval(pg_get_serial_sequence('quotes', 'id'))::bigint;"#;
    let (id,) = sqlx::query_as(QUERY).fetch_one(ex).await?;
    Ok(id)
}

/// Allocates `n` quote ids from the `quotes` id sequence in one round trip.
#[instrument(skip_all)]
pub async fn next_ids(ex: &mut PgConnection, n: usize) -> Result<Vec<QuoteId>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT nextval(pg_get_serial_sequence('quotes', 'id'))::bigint
FROM generate_series(1, $1);
    "#;
    let ids: Vec<(QuoteId,)> = sqlx::query_as(QUERY)
        .bind(i64::try_from(n).unwrap_or(i64::MAX))
        .fetch_all(ex)
        .await?;
    Ok(ids.into_iter().map(|(id,)| id).collect())
}

/// Stores the quote under its `id` (allocated with [`next_id`] or
/// [`next_ids`]) and returns it.
#[instrument(skip_all)]
pub async fn save(ex: &mut PgConnection, quote: &Quote) -> Result<QuoteId, sqlx::Error> {
    const QUERY: &str = r#"
INSERT INTO quotes (
    id,
    sell_token,
    buy_token,
    sell_amount,
    buy_amount,
    gas_amount,
    gas_price,
    sell_token_price,
    order_kind,
    expiration_timestamp,
    quote_kind,
    solver,
    verified,
    metadata
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
RETURNING id
    "#;
    let (id,) = sqlx::query_as(QUERY)
        .bind(quote.id)
        .bind(quote.sell_token)
        .bind(quote.buy_token)
        .bind(&quote.sell_amount)
        .bind(&quote.buy_amount)
        .bind(quote.gas_amount)
        .bind(quote.gas_price)
        .bind(quote.sell_token_price)
        .bind(quote.order_kind)
        .bind(quote.expiration_timestamp)
        .bind(&quote.quote_kind)
        .bind(quote.solver)
        .bind(quote.verified)
        .bind(&quote.metadata)
        .fetch_one(ex)
        .await?;
    Ok(id)
}

#[instrument(skip_all)]
pub async fn get(ex: &mut PgConnection, id: QuoteId) -> Result<Option<Quote>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT *
FROM quotes
WHERE id = $1
    "#;
    sqlx::query_as(QUERY).bind(id).fetch_optional(ex).await
}

/// Fields for searching stored quotes.
#[derive(Clone)]
pub struct QuoteSearchParameters {
    pub sell_token: Address,
    pub buy_token: Address,
    pub sell_amount_0: BigDecimal,
    pub sell_amount_1: BigDecimal,
    pub buy_amount: BigDecimal,
    pub kind: OrderKind,
    pub expiration: DateTime<Utc>,
    pub quote_kind: QuoteKind,
    pub fast_path: bool,
}

#[instrument(skip_all)]
pub async fn find(
    ex: &mut PgConnection,
    params: &QuoteSearchParameters,
) -> Result<Option<Quote>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT *
FROM quotes q
LEFT JOIN quote_competitions qc ON qc.quote_id = q.id
WHERE
    sell_token = $1 AND
    buy_token = $2 AND
    (
        (order_kind = 'sell' AND sell_amount = $3) OR
        (order_kind = 'sell' AND sell_amount = $4) OR
        (order_kind = 'buy' AND buy_amount = $5)
    ) AND
    order_kind = $6 AND
    expiration_timestamp >= $7 AND
    quote_kind = $8 AND
    -- if a fast path quote is requested the quote competition data must exist
    (qc.quote_id IS NOT NULL) = $9
-- Return the best quote for the user, mirroring the price-estimation
-- competition: prefer verified quotes over unverified ones, then take the
-- highest buy/sell exchange rate net of the sell-token-denominated fee.
ORDER BY
    verified DESC,
    buy_amount / (sell_amount + gas_amount * gas_price / sell_token_price) DESC
LIMIT 1
    "#;
    sqlx::query_as(QUERY)
        .bind(params.sell_token)
        .bind(params.buy_token)
        .bind(&params.sell_amount_0)
        .bind(&params.sell_amount_1)
        .bind(&params.buy_amount)
        .bind(params.kind)
        .bind(params.expiration)
        .bind(&params.quote_kind)
        .bind(params.fast_path)
        .fetch_optional(ex)
        .await
}

#[instrument(skip_all)]
pub async fn remove_expired_quotes(
    ex: &mut PgConnection,
    max_expiry: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
DELETE FROM quotes
WHERE expiration_timestamp < $1
    "#;
    sqlx::query(QUERY)
        .bind(max_expiry)
        .execute(ex)
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::byte_array::ByteArray,
        chrono::Duration,
        sqlx::{Connection, types::chrono::TimeZone},
    };

    /// The postgres database in our CI has different datetime precision than
    /// the `DateTime` uses. This leads to issues comparing round-tripped data.
    /// Work around the issue by created `DateTime`s with lower precision.
    fn low_precision_now() -> DateTime<Utc> {
        Utc.timestamp_opt(Utc::now().timestamp(), 0).unwrap()
    }

    /// Stores `quote` under a freshly allocated id, the way the orderbook
    /// does: the id is minted before the quote is computed, and `save` is
    /// expected to honour it.
    async fn save_with_new_id(db: &mut PgConnection, quote: &mut Quote) -> QuoteId {
        quote.id = next_id(db).await.unwrap();
        let id = save(db, quote).await.unwrap();
        assert_eq!(id, quote.id);
        id
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_save_and_get_quote_by_id() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let now = low_precision_now();
        let mut quote = Quote {
            id: Default::default(),
            sell_token: ByteArray([1; 20]),
            buy_token: ByteArray([2; 20]),
            sell_amount: 3.into(),
            buy_amount: 4.into(),
            gas_amount: 5.,
            gas_price: 6.,
            sell_token_price: 7.,
            order_kind: OrderKind::Sell,
            expiration_timestamp: now,
            quote_kind: QuoteKind::Standard,
            solver: ByteArray([1; 20]),
            verified: false,
            metadata: Default::default(),
        };
        let id = save_with_new_id(&mut db, &mut quote).await;
        quote.id = id;
        assert_eq!(get(&mut db, id).await.unwrap().unwrap(), quote);

        remove_expired_quotes(&mut db, now + Duration::seconds(30))
            .await
            .unwrap();
        assert_eq!(get(&mut db, id).await.unwrap(), None);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_save_and_find_quote() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let now = low_precision_now();
        let token_a = ByteArray([1; 20]);
        let quote_a = Quote {
            id: Default::default(),
            sell_token: token_a,
            buy_token: ByteArray([3; 20]),
            sell_amount: 4.into(),
            buy_amount: 5.into(),
            order_kind: OrderKind::Sell,
            gas_amount: 1.,
            gas_price: 1.,
            sell_token_price: 1.,
            expiration_timestamp: now,
            quote_kind: QuoteKind::Standard,
            solver: ByteArray([1; 20]),
            verified: false,
            metadata: Default::default(),
        };

        let token_b = ByteArray([2; 20]);
        let quote_b = Quote {
            id: Default::default(),
            sell_token: token_b,
            buy_token: token_a,
            sell_amount: 200.into(),
            buy_amount: 100.into(),
            order_kind: OrderKind::Buy,
            gas_amount: 20_000_u32.into(),
            gas_price: 1.,
            sell_token_price: 1.,
            expiration_timestamp: now,
            quote_kind: QuoteKind::Standard,
            solver: ByteArray([2; 20]),
            verified: false,
            metadata: Default::default(),
        };

        // Save two measurements for token_a
        let quotes_a = [
            {
                let mut quote = Quote {
                    expiration_timestamp: now,
                    gas_amount: 100_u32.into(),
                    ..quote_a.clone()
                };
                let id = save_with_new_id(&mut db, &mut quote).await;
                quote.id = id;
                quote
            },
            {
                let mut quote = Quote {
                    expiration_timestamp: now + Duration::seconds(60),
                    gas_amount: 200_u32.into(),
                    ..quote_a.clone()
                };
                let id = save_with_new_id(&mut db, &mut quote).await;
                quote.id = id;
                quote
            },
        ];

        // Save one measurement for token_b
        let quotes_b = [{
            let mut quote = Quote {
                expiration_timestamp: now,
                gas_amount: 10_u32.into(),
                ..quote_b.clone()
            };
            let id = save_with_new_id(&mut db, &mut quote).await;
            quote.id = id;
            quote
        }];

        // Token A has readings valid until now and in 30s
        let search_a = QuoteSearchParameters {
            sell_token: quote_a.sell_token,
            buy_token: quote_a.buy_token,
            sell_amount_0: quote_a.sell_amount.clone(),
            sell_amount_1: quote_a.sell_amount.clone(),
            buy_amount: 1.into(),
            kind: quote_a.order_kind,
            expiration: now,
            quote_kind: QuoteKind::Standard,
            fast_path: false,
        };
        assert_eq!(
            find(&mut db, &search_a).await.unwrap().unwrap(),
            quotes_a[0],
        );
        assert_eq!(
            find(
                &mut db,
                &QuoteSearchParameters {
                    expiration: now + Duration::seconds(30),
                    ..search_a.clone()
                }
            )
            .await
            .unwrap()
            .unwrap(),
            quotes_a[1]
        );

        // Token A has readings for sell + fee amount equal to quoted amount.
        assert_eq!(
            find(
                &mut db,
                &QuoteSearchParameters {
                    sell_amount_0: quote_a.sell_amount.clone() - BigDecimal::from(1),
                    sell_amount_1: quote_a.sell_amount.clone(),
                    ..search_a.clone()
                },
            )
            .await
            .unwrap()
            .unwrap(),
            quotes_a[0],
        );

        // Token A has no reading for wrong filter
        assert_eq!(
            find(
                &mut db,
                &QuoteSearchParameters {
                    sell_amount_0: quote_a.sell_amount.clone() - BigDecimal::from(1),
                    sell_amount_1: quote_a.sell_amount.clone() - BigDecimal::from(1),
                    ..search_a.clone()
                }
            )
            .await
            .unwrap(),
            None
        );

        // Token B only has readings valid until now
        let search_b = QuoteSearchParameters {
            sell_token: quote_b.sell_token,
            buy_token: quote_b.buy_token,
            sell_amount_0: 999.into(),
            sell_amount_1: 999.into(),
            buy_amount: quote_b.buy_amount,
            kind: quote_b.order_kind,
            expiration: now,
            quote_kind: QuoteKind::Standard,
            fast_path: false,
        };
        assert_eq!(
            find(&mut db, &search_b).await.unwrap().unwrap(),
            quotes_b[0],
        );
        assert_eq!(
            find(
                &mut db,
                &QuoteSearchParameters {
                    expiration: now + Duration::seconds(30),
                    ..search_b.clone()
                }
            )
            .await
            .unwrap(),
            None
        );

        // Token B has no reading for wrong filter
        assert_eq!(
            find(
                &mut db,
                &QuoteSearchParameters {
                    buy_amount: 99.into(),
                    ..search_b.clone()
                }
            )
            .await
            .unwrap(),
            None
        );

        // Query that previously succeeded after cleaning up expired
        // measurements.
        remove_expired_quotes(&mut db, now + Duration::seconds(120))
            .await
            .unwrap();
        assert_eq!(find(&mut db, &search_a).await.unwrap(), None);
        assert_eq!(find(&mut db, &search_b).await.unwrap(), None);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_find_quote_picks_best_net_of_fee_rate_sell_order() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let now = low_precision_now();
        // All candidates quote the same sell order (same sell amount); they
        // only differ in buy amount and fee.
        let base = Quote {
            id: Default::default(),
            sell_token: ByteArray([1; 20]),
            buy_token: ByteArray([2; 20]),
            sell_amount: 1000.into(),
            buy_amount: Default::default(),
            gas_amount: 0.,
            gas_price: 1.,
            sell_token_price: 0.1,
            order_kind: OrderKind::Sell,
            expiration_timestamp: now,
            quote_kind: QuoteKind::Standard,
            solver: ByteArray([1; 20]),
            verified: false,
            metadata: Default::default(),
        };

        // Highest absolute buy amount, but an expensive fee.
        // net rate = 210 / (1000 + 100*1/0.1) = 210/2000 ≈ 0.105
        let mut high_buy_high_fee = Quote {
            buy_amount: 210.into(),
            gas_amount: 100.,
            solver: ByteArray([1; 20]),
            ..base.clone()
        };
        save_with_new_id(&mut db, &mut high_buy_high_fee).await;

        // Lower absolute buy amount, but a negligible fee -> best net-of-fee
        // rate. net rate = 200 / (1000 + 1*1/0.1) = 200/1010 ≈ 0.198
        let mut best_rate = Quote {
            buy_amount: 200.into(),
            gas_amount: 1.,
            solver: ByteArray([2; 20]),
            ..base.clone()
        };
        save_with_new_id(&mut db, &mut best_rate).await;

        let search = QuoteSearchParameters {
            sell_token: base.sell_token,
            buy_token: base.buy_token,
            sell_amount_0: base.sell_amount.clone(),
            sell_amount_1: base.sell_amount.clone(),
            buy_amount: Default::default(),
            kind: OrderKind::Sell,
            expiration: now,
            quote_kind: QuoteKind::Standard,
            fast_path: false,
        };
        assert_eq!(find(&mut db, &search).await.unwrap().unwrap(), best_rate);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_find_quote_picks_best_net_of_fee_rate_buy_order() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let now = low_precision_now();
        // All candidates quote the same buy order (same buy amount); they only
        // differ in sell amount and fee.
        let base = Quote {
            id: Default::default(),
            sell_token: ByteArray([1; 20]),
            buy_token: ByteArray([2; 20]),
            sell_amount: Default::default(),
            buy_amount: 100.into(),
            gas_amount: 0.,
            gas_price: 1.,
            sell_token_price: 0.1,
            order_kind: OrderKind::Buy,
            expiration_timestamp: now,
            quote_kind: QuoteKind::Standard,
            solver: ByteArray([1; 20]),
            verified: false,
            metadata: Default::default(),
        };

        // Lowest absolute sell amount, but an expensive fee -> total spend
        // 3000. net rate = 100 / (1000 + 200*1/0.1) = 100/3000 ≈ 0.033
        let mut low_sell_high_fee = Quote {
            sell_amount: 1000.into(),
            gas_amount: 200.,
            solver: ByteArray([1; 20]),
            ..base.clone()
        };
        save_with_new_id(&mut db, &mut low_sell_high_fee).await;

        // Higher absolute sell amount, but a negligible fee -> total spend
        // 1110. net rate = 100 / (1100 + 1*1/0.1) = 100/1110 ≈ 0.090
        let mut high_sell_low_fee = Quote {
            sell_amount: 1100.into(),
            gas_amount: 1.,
            solver: ByteArray([2; 20]),
            ..base.clone()
        };
        save_with_new_id(&mut db, &mut high_sell_low_fee).await;

        let search = QuoteSearchParameters {
            sell_token: base.sell_token,
            buy_token: base.buy_token,
            sell_amount_0: Default::default(),
            sell_amount_1: Default::default(),
            buy_amount: base.buy_amount.clone(),
            kind: OrderKind::Buy,
            expiration: now,
            quote_kind: QuoteKind::Standard,
            fast_path: false,
        };

        assert_eq!(
            find(&mut db, &search).await.unwrap().unwrap(),
            high_sell_low_fee
        );
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_find_quote_prefers_verified_over_better_rate() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let now = low_precision_now();
        // Two candidates for the same sell order, no fee; they differ only in
        // buy amount (rate) and verification status.
        let base = Quote {
            id: Default::default(),
            sell_token: ByteArray([1; 20]),
            buy_token: ByteArray([2; 20]),
            sell_amount: 1000.into(),
            buy_amount: Default::default(),
            gas_amount: 0.,
            gas_price: 1.,
            sell_token_price: 1.,
            order_kind: OrderKind::Sell,
            expiration_timestamp: now,
            quote_kind: QuoteKind::Standard,
            solver: ByteArray([1; 20]),
            verified: false,
            metadata: Default::default(),
        };

        // Unverified but strictly better rate (more buy for the same sell).
        let mut unverified_better = Quote {
            buy_amount: 210.into(),
            verified: false,
            solver: ByteArray([1; 20]),
            ..base.clone()
        };
        save_with_new_id(&mut db, &mut unverified_better).await;

        // Verified with a worse rate -> should still win
        let mut verified_worse = Quote {
            buy_amount: 200.into(),
            verified: true,
            solver: ByteArray([2; 20]),
            ..base.clone()
        };
        save_with_new_id(&mut db, &mut verified_worse).await;

        let search = QuoteSearchParameters {
            sell_token: base.sell_token,
            buy_token: base.buy_token,
            sell_amount_0: base.sell_amount.clone(),
            sell_amount_1: base.sell_amount.clone(),
            buy_amount: Default::default(),
            kind: OrderKind::Sell,
            expiration: now,
            quote_kind: QuoteKind::Standard,
            fast_path: false,
        };
        assert_eq!(
            find(&mut db, &search).await.unwrap().unwrap(),
            verified_worse
        );
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_save_and_find_quote_and_differentiates_by_signing_scheme() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let now = low_precision_now();
        let token_a = ByteArray([1; 20]);
        let quote = {
            let mut quote = Quote {
                id: Default::default(),
                sell_token: token_a,
                buy_token: ByteArray([3; 20]),
                sell_amount: 4.into(),
                buy_amount: 5.into(),
                gas_amount: 1.,
                gas_price: 1.,
                sell_token_price: 1.,
                order_kind: OrderKind::Sell,
                expiration_timestamp: now,
                quote_kind: QuoteKind::Eip1271OnchainOrder,
                solver: ByteArray([1; 20]),
                verified: false,
                metadata: Default::default(),
            };
            let id = save_with_new_id(&mut db, &mut quote).await;
            quote.id = id;
            quote
        };
        // Token A has readings valid until now and in 30s
        let mut search_a = QuoteSearchParameters {
            sell_token: quote.sell_token,
            buy_token: quote.buy_token,
            sell_amount_0: quote.sell_amount.clone(),
            sell_amount_1: quote.sell_amount.clone(),
            buy_amount: quote.buy_amount.clone(),
            kind: quote.order_kind,
            expiration: quote.expiration_timestamp,
            quote_kind: quote.quote_kind.clone(),
            fast_path: false,
        };

        assert_eq!(find(&mut db, &search_a).await.unwrap().unwrap(), quote,);
        search_a.quote_kind = QuoteKind::Standard;
        assert_eq!(find(&mut db, &search_a).await.unwrap(), None,);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_find_quote_differentiates_fast_path() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let now = low_precision_now();
        let base = Quote {
            id: Default::default(),
            sell_token: ByteArray([1; 20]),
            buy_token: ByteArray([2; 20]),
            sell_amount: 100.into(),
            buy_amount: 200.into(),
            gas_amount: 1.,
            gas_price: 1.,
            sell_token_price: 1.,
            order_kind: OrderKind::Sell,
            expiration_timestamp: now,
            quote_kind: QuoteKind::Standard,
            solver: ByteArray([1; 20]),
            verified: false,
            metadata: Default::default(),
        };

        // A regular quote (no staged competition).
        let mut regular = base.clone();
        save_with_new_id(&mut db, &mut regular).await;

        // A fast-path quote (staged competition attached).
        let mut fast_path = base.clone();
        save_with_new_id(&mut db, &mut fast_path).await;
        crate::fast_path::save_competition(&mut db, fast_path.id, serde_json::json!({}))
            .await
            .unwrap();

        let search = QuoteSearchParameters {
            sell_token: base.sell_token,
            buy_token: base.buy_token,
            sell_amount_0: base.sell_amount.clone(),
            sell_amount_1: base.sell_amount.clone(),
            buy_amount: base.buy_amount.clone(),
            kind: base.order_kind,
            expiration: now,
            quote_kind: QuoteKind::Standard,
            fast_path: false,
        };

        // Regular searches skip the quote with a staged competition.
        assert_eq!(find(&mut db, &search).await.unwrap().unwrap(), regular);

        // Fast-path searches only return the quote with a staged competition.
        assert_eq!(
            find(
                &mut db,
                &QuoteSearchParameters {
                    fast_path: true,
                    ..search.clone()
                }
            )
            .await
            .unwrap()
            .unwrap(),
            fast_path,
        );
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_insert_quote_metadata() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let metadata: serde_json::Value = serde_json::from_str(
            r#"{ "version":"1.0", "interactions": [ {
                "target": "0x0102030405060708091011121314151617181920",
                "value": "1",
                "callData": "0x0A0B0C102030"
            },{
            "target": "0xFF02030405060708091011121314151617181920",
            "value": "2",
            "callData": "0xFF0B0C102030"
            }]
        }"#,
        )
        .unwrap();

        let mut quote = Quote {
            id: Default::default(),
            sell_token: ByteArray([1; 20]),
            buy_token: ByteArray([2; 20]),
            sell_amount: 3.into(),
            buy_amount: 4.into(),
            gas_amount: 5.,
            gas_price: 6.,
            sell_token_price: 7.,
            order_kind: OrderKind::Sell,
            expiration_timestamp: low_precision_now(),
            quote_kind: QuoteKind::Standard,
            solver: ByteArray([1; 20]),
            verified: false,
            metadata: metadata.clone(),
        };
        // store quote in database
        let id = save_with_new_id(&mut db, &mut quote).await;

        let stored_quote = get(&mut db, id).await.unwrap().unwrap();
        assert_eq!(stored_quote.metadata, metadata);
    }
}
