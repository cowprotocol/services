use super::events::EventIndex;
use crate::{Address, OrderUid, PgTransaction};
use sqlx::{Executor, PgConnection};

#[derive(Clone, Debug, Eq, PartialEq, sqlx::Type)]
#[sqlx(type_name = "OnchainOrderPlacementError", rename_all = "snake_case")]
pub enum OnchainOrderPlacementError {
    QuoteIdNotFound,
    NotAllowedBuyToken,
    NonAcceptedOrderClass,
    ValidToTooFarInFuture,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OnchainOrderPlacement {
    pub order_uid: OrderUid,
    pub sender: Address,
    pub placement_error: Option<OnchainOrderPlacementError>,
}

#[derive(Clone, Debug, Default, sqlx::FromRow, Eq, PartialEq)]
pub struct OnchainOrderPlacementRow {
    pub uid: OrderUid,
    pub sender: Address,
    pub placement_error: Option<OnchainOrderPlacementError>,
    pub is_reorged: bool,
    pub block_number: i64,
    pub log_index: i64,
}

pub async fn last_block(ex: &mut PgConnection) -> Result<i64, sqlx::Error> {
    const QUERY: &str = r#"
        SELECT COALESCE(MAX(block_number), 0) FROM onchain_placed_orders;
    "#;
    sqlx::query_scalar(QUERY).fetch_one(ex).await
}

pub async fn mark_as_reorged(
    ex: &mut PgTransaction<'_>,
    mark_from_block_number: i64,
) -> Result<(), sqlx::Error> {
    const QUERY_ONCHAIN_ORDERS: &str =
        "UPDATE onchain_placed_orders SET is_reorged = true WHERE block_number >= $1;";
    ex.execute(sqlx::query(QUERY_ONCHAIN_ORDERS).bind(mark_from_block_number))
        .await?;
    Ok(())
}

pub async fn append(
    ex: &mut PgTransaction<'_>,
    events: &[(EventIndex, OnchainOrderPlacement)],
) -> Result<(), sqlx::Error> {
    for (index, event) in events {
        insert_onchain_order(ex, index, event).await?;
    }
    Ok(())
}

pub async fn insert_onchain_order(
    ex: &mut PgConnection,
    index: &EventIndex,
    event: &OnchainOrderPlacement,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
        INSERT INTO onchain_placed_orders
            (uid, sender, is_reorged, placement_error, block_number, log_index)
        VALUES ($1, $2, false, $3, $4, $5)
        ON CONFLICT (uid) DO UPDATE SET
            is_reorged = false, sender = $2, placement_error = $3,
            block_number = $4, log_index = $5;
    "#;
    sqlx::query(QUERY)
        .bind(event.order_uid)
        .bind(&event.sender)
        .bind(&event.placement_error)
        .bind(index.block_number)
        .bind(index.log_index)
        .execute(ex)
        .await?;
    Ok(())
}

pub async fn read_order(
    ex: &mut PgConnection,
    id: &OrderUid,
) -> Result<Option<OnchainOrderPlacementRow>, sqlx::Error> {
    const QUERY: &str = r#"
        SELECT * FROM onchain_placed_orders
        WHERE uid = $1
    "#;
    sqlx::query_as(QUERY).bind(id).fetch_optional(ex).await
}

#[cfg(test)]
mod tests {
    use crate::byte_array::ByteArray;

    use super::*;
    use sqlx::Connection;

    #[tokio::test]
    #[ignore]
    async fn postgres_order_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        // round trip with quote id not found error
        crate::clear_DANGER_(&mut db).await.unwrap();

        let order = OnchainOrderPlacement {
            placement_error: Some(OnchainOrderPlacementError::QuoteIdNotFound),
            ..Default::default()
        };
        let event_index = EventIndex::default();
        insert_onchain_order(&mut db, &event_index, &order)
            .await
            .unwrap();
        let row = read_order(&mut db, &order.order_uid)
            .await
            .unwrap()
            .unwrap();
        let expected_row = OnchainOrderPlacementRow {
            uid: order.order_uid,
            sender: order.sender,
            placement_error: Some(OnchainOrderPlacementError::QuoteIdNotFound),
            is_reorged: false,
            block_number: event_index.block_number,
            log_index: event_index.log_index,
        };
        assert_eq!(expected_row, row);
        // round trip with not allowed buy token error
        crate::clear_DANGER_(&mut db).await.unwrap();

        let order = OnchainOrderPlacement {
            placement_error: Some(OnchainOrderPlacementError::NotAllowedBuyToken),
            ..Default::default()
        };
        let event_index = EventIndex::default();
        insert_onchain_order(&mut db, &event_index, &order)
            .await
            .unwrap();
        let row = read_order(&mut db, &order.order_uid)
            .await
            .unwrap()
            .unwrap();
        let expected_row = OnchainOrderPlacementRow {
            uid: order.order_uid,
            sender: order.sender,
            placement_error: Some(OnchainOrderPlacementError::NotAllowedBuyToken),
            is_reorged: false,
            block_number: event_index.block_number,
            log_index: event_index.log_index,
        };
        assert_eq!(expected_row, row);
        // round trip with err valid_to too far in future
        crate::clear_DANGER_(&mut db).await.unwrap();

        let order = OnchainOrderPlacement {
            placement_error: Some(OnchainOrderPlacementError::ValidToTooFarInFuture),
            ..Default::default()
        };
        let event_index = EventIndex::default();
        insert_onchain_order(&mut db, &event_index, &order)
            .await
            .unwrap();
        let row = read_order(&mut db, &order.order_uid)
            .await
            .unwrap()
            .unwrap();
        let expected_row = OnchainOrderPlacementRow {
            uid: order.order_uid,
            sender: order.sender,
            placement_error: Some(OnchainOrderPlacementError::ValidToTooFarInFuture),
            is_reorged: false,
            block_number: event_index.block_number,
            log_index: event_index.log_index,
        };
        assert_eq!(expected_row, row);
        // round trip with not accepted order class error
        crate::clear_DANGER_(&mut db).await.unwrap();

        let order = OnchainOrderPlacement {
            placement_error: Some(OnchainOrderPlacementError::NonAcceptedOrderClass),
            ..Default::default()
        };
        let event_index = EventIndex::default();
        insert_onchain_order(&mut db, &event_index, &order)
            .await
            .unwrap();
        let row = read_order(&mut db, &order.order_uid)
            .await
            .unwrap()
            .unwrap();
        let expected_row = OnchainOrderPlacementRow {
            uid: order.order_uid,
            sender: order.sender,
            placement_error: Some(OnchainOrderPlacementError::NonAcceptedOrderClass),
            is_reorged: false,
            block_number: event_index.block_number,
            log_index: event_index.log_index,
        };
        assert_eq!(expected_row, row);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_last_block() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let event_index = EventIndex {
            block_number: 1,
            log_index: 0,
        };
        append(&mut db, &[(event_index, OnchainOrderPlacement::default())])
            .await
            .unwrap();
        assert_eq!(last_block(&mut db).await.unwrap(), 1);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_sets_is_reorged_to_true() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let event_index_1 = EventIndex {
            block_number: 1,
            log_index: 0,
        };
        let event_index_2 = EventIndex {
            block_number: 2,
            log_index: 0,
        };

        let order_1 = OnchainOrderPlacement {
            order_uid: ByteArray([1; 56]),
            sender: ByteArray([1; 20]),
            placement_error: None,
        };
        let order_2 = OnchainOrderPlacement {
            order_uid: ByteArray([2; 56]),
            sender: ByteArray([2; 20]),
            placement_error: None,
        };
        append(
            &mut db,
            &[
                (event_index_1, order_1.clone()),
                (event_index_2, order_2.clone()),
            ],
        )
        .await
        .unwrap();
        mark_as_reorged(&mut db, 2).await.unwrap();
        let row = read_order(&mut db, &order_1.order_uid)
            .await
            .unwrap()
            .unwrap();
        let expected_row = OnchainOrderPlacementRow {
            uid: order_1.order_uid,
            sender: order_1.sender,
            placement_error: None,
            is_reorged: false,
            block_number: event_index_1.block_number,
            log_index: event_index_1.log_index,
        };
        assert_eq!(expected_row, row);
        let row = read_order(&mut db, &order_2.order_uid)
            .await
            .unwrap()
            .unwrap();
        let expected_row = OnchainOrderPlacementRow {
            uid: order_2.order_uid,
            sender: order_2.sender,
            placement_error: None,
            is_reorged: true, // <-- difference is here
            block_number: event_index_2.block_number,
            log_index: event_index_2.log_index,
        };
        assert_eq!(expected_row, row);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_insert_order_conflict_handling() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let event_index_1 = EventIndex {
            block_number: 1,
            log_index: 0,
        };
        let event_index_2 = EventIndex {
            block_number: 2,
            log_index: 1,
        };
        let order_1 = OnchainOrderPlacement {
            order_uid: ByteArray([1; 56]),
            sender: ByteArray([1; 20]),
            placement_error: None,
        };
        append(&mut db, &[(event_index_1, order_1.clone())])
            .await
            .unwrap();
        mark_as_reorged(&mut db, 1).await.unwrap();
        let row = read_order(&mut db, &order_1.order_uid)
            .await
            .unwrap()
            .unwrap();
        let expected_row = OnchainOrderPlacementRow {
            uid: order_1.order_uid,
            sender: order_1.sender,
            is_reorged: true,
            placement_error: None,
            block_number: event_index_1.block_number,
            log_index: event_index_1.log_index,
        };
        assert_eq!(expected_row, row);
        let reorged_order = OnchainOrderPlacement {
            order_uid: order_1.order_uid,
            sender: ByteArray([2; 20]),
            placement_error: None,
        };
        // Now, we insert the order again and then it should no longer be reorged
        append(&mut db, &[(event_index_2, reorged_order.clone())])
            .await
            .unwrap();
        let row = read_order(&mut db, &order_1.order_uid)
            .await
            .unwrap()
            .unwrap();
        let expected_row = OnchainOrderPlacementRow {
            uid: order_1.order_uid,
            sender: reorged_order.sender,
            is_reorged: false,
            placement_error: None,
            block_number: event_index_2.block_number,
            log_index: event_index_2.log_index,
        };
        assert_eq!(expected_row, row);
    }
}
