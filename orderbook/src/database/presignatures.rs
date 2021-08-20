use crate::conversions::h160_from_vec;
use crate::database::Postgres;
use anyhow::{anyhow, Context, Result};
use ethcontract::H160;
use futures::{future, stream::TryStreamExt};
use model::{order::OrderUid, presignature::PreSignature};
use std::convert::TryInto;

#[async_trait::async_trait]
pub trait PreSignatureRetrieving: Send + Sync {
    async fn presignatures(&self, filter: &PreSignatureFilter) -> Result<Vec<PreSignature>>;
}

/// Any default value means that this field is unfiltered.
#[derive(Debug, Default, PartialEq)]
pub struct PreSignatureFilter {
    pub owner: Option<H160>,
    pub order_uid: Option<OrderUid>,
}

#[async_trait::async_trait]
impl PreSignatureRetrieving for Postgres {
    async fn presignatures(&self, filter: &PreSignatureFilter) -> Result<Vec<PreSignature>> {
        const QUERY: &str = "\
            SELECT DISTINCT ON (t.order_uid) \
                t.block_number, \
                t.log_index, \
                t.owner, \
                t.order_uid, \
                t.signed \
            FROM presignatures t \
            WHERE \
                ($1 IS NULL OR t.owner = $1) \
            AND \
                ($2 IS NULL OR t.order_uid = $2) \
            ORDER BY \
                t.order_uid, t.block_number DESC, t.log_index DESC;";

        sqlx::query_as(QUERY)
            .bind(filter.owner.as_ref().map(|h160| h160.as_bytes()))
            .bind(filter.order_uid.as_ref().map(|uid| uid.0.as_ref()))
            .fetch(&self.pool)
            .err_into()
            .try_filter(|r: &PreSignaturesQueryRow| future::ready(r.signed))
            .and_then(|row: PreSignaturesQueryRow| async move { row.into_presignature() })
            .try_collect()
            .await
    }
}

#[derive(sqlx::FromRow)]
struct PreSignaturesQueryRow {
    block_number: i64,
    log_index: i64,
    owner: Vec<u8>,
    order_uid: Vec<u8>,
    signed: bool,
}

impl PreSignaturesQueryRow {
    fn into_presignature(self) -> Result<PreSignature> {
        let block_number = self
            .block_number
            .try_into()
            .context("block_number is not u32")?;
        let log_index = self.log_index.try_into().context("log_index is not u32")?;
        let owner = h160_from_vec(self.owner)?;
        let order_uid = OrderUid(
            self.order_uid
                .try_into()
                .map_err(|_| anyhow!("order uid has wrong length"))?,
        );
        let signed = self.signed;
        Ok(PreSignature {
            block_number,
            log_index,
            owner,
            order_uid,
            signed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::events::{Event, PreSignature as DbPreSignature};
    use model::presignature::PreSignature;
    use shared::event_handling::EventIndex;
    use std::collections::HashSet;

    async fn add_presignature(
        db: &Postgres,
        owner: H160,
        order_uid: OrderUid,
        event_index: EventIndex,
        signed: bool,
    ) -> PreSignature {
        let presignature = PreSignature {
            block_number: event_index.block_number,
            log_index: event_index.log_index,
            order_uid,
            owner,
            signed,
        };
        db.append_events_(vec![(
            event_index,
            Event::PreSignature(DbPreSignature {
                order_uid,
                owner,
                signed,
            }),
        )])
        .await
        .unwrap();
        presignature
    }

    async fn assert_presignatures(
        db: &Postgres,
        filter: &PreSignatureFilter,
        expected: &[PreSignature],
    ) {
        let filtered = db
            .presignatures(filter)
            .await
            .unwrap()
            .into_iter()
            .collect::<HashSet<_>>();
        let expected = expected.iter().cloned().collect::<HashSet<_>>();
        assert_eq!(filtered, expected);
    }

    fn bump_block_number_by(event_index: &EventIndex, amount: u64) -> EventIndex {
        EventIndex {
            block_number: event_index.block_number + amount,
            log_index: event_index.log_index,
        }
    }

    fn bump_log_index_by(event_index: &EventIndex, amount: u64) -> EventIndex {
        EventIndex {
            block_number: event_index.block_number,
            log_index: event_index.log_index + amount,
        }
    }

    mod postgres_respect_filters {
        use super::*;

        const USER1_UID: OrderUid = OrderUid([42; 56]);
        const USER2_UID: OrderUid = OrderUid([21; 56]);
        const REVOKED_USER_UID: OrderUid = OrderUid([14; 56]);
        const USER1_ADDRESS: H160 = H160([0x42; 20]);
        const USER2_ADDRESS: H160 = H160([0x21; 20]);
        const REVOKED_USER_ADDRESS: H160 = H160([0x14; 20]);

        struct Setup {
            presignature_1: PreSignature,
            presignature_2: PreSignature,
            db: Postgres,
        }

        async fn setup() -> Setup {
            let db = Postgres::new("postgresql://").unwrap();
            db.clear().await.unwrap();
            let event_index = EventIndex {
                block_number: 0,
                log_index: 0,
            };
            let presignature_1 =
                add_presignature(&db, USER1_ADDRESS, USER1_UID, event_index, true).await;
            let presignature_2 = add_presignature(
                &db,
                USER2_ADDRESS,
                USER2_UID,
                bump_block_number_by(&event_index, 1),
                true,
            )
            .await;
            let _presignature_revoked = add_presignature(
                &db,
                REVOKED_USER_ADDRESS,
                REVOKED_USER_UID,
                bump_block_number_by(&event_index, 2),
                false,
            )
            .await;
            Setup {
                presignature_1,
                presignature_2,
                db,
            }
        }

        #[tokio::test]
        #[ignore]
        async fn no_filter() {
            let Setup {
                presignature_1,
                presignature_2,
                db,
            } = setup().await;
            assert_presignatures(
                &db,
                &PreSignatureFilter::default(),
                &[presignature_1, presignature_2],
            )
            .await;
        }

        #[tokio::test]
        #[ignore]
        async fn filter_owner() {
            let Setup {
                presignature_1, db, ..
            } = setup().await;
            assert_presignatures(
                &db,
                &PreSignatureFilter {
                    owner: Some(USER1_ADDRESS),
                    ..PreSignatureFilter::default()
                },
                &[presignature_1],
            )
            .await;
        }

        #[tokio::test]
        #[ignore]
        async fn filter_uid() {
            let Setup {
                presignature_2, db, ..
            } = setup().await;
            assert_presignatures(
                &db,
                &PreSignatureFilter {
                    order_uid: Some(USER2_UID),
                    ..PreSignatureFilter::default()
                },
                &[presignature_2],
            )
            .await;
        }

        #[tokio::test]
        #[ignore]
        async fn both_filters_success() {
            let Setup {
                presignature_1, db, ..
            } = setup().await;
            assert_presignatures(
                &db,
                &PreSignatureFilter {
                    owner: Some(USER1_ADDRESS),
                    order_uid: Some(USER1_UID),
                },
                &[presignature_1],
            )
            .await;
        }

        #[tokio::test]
        #[ignore]
        async fn both_filters_no_entries() {
            let Setup { db, .. } = setup().await;
            assert_presignatures(
                &db,
                &PreSignatureFilter {
                    owner: Some(USER1_ADDRESS),
                    order_uid: Some(USER2_UID),
                },
                &[],
            )
            .await;
        }

        #[tokio::test]
        #[ignore]
        async fn filters_ignore_revoked_entries() {
            let Setup { db, .. } = setup().await;
            assert_presignatures(
                &db,
                &PreSignatureFilter {
                    owner: Some(REVOKED_USER_ADDRESS),
                    ..PreSignatureFilter::default()
                },
                &[],
            )
            .await;
        }
    }

    mod postgres_use_last_presignature_for_each_uid {
        use super::*;

        const OWNER: H160 = H160([0x42; 20]);
        const UID: OrderUid = OrderUid([42; 56]);

        async fn setup() -> (Postgres, EventIndex) {
            let db = Postgres::new("postgresql://").unwrap();
            db.clear().await.unwrap();
            let event_index = EventIndex {
                block_number: 0,
                log_index: 0,
            };
            (db, event_index)
        }

        #[ignore]
        #[tokio::test]
        async fn deleted_in_a_following_block() {
            let (db, event_index) = setup().await;
            add_presignature(&db, OWNER, UID, event_index, true).await;
            add_presignature(
                &db,
                OWNER,
                UID,
                bump_block_number_by(&event_index, 1),
                false,
            )
            .await;
            assert_presignatures(&db, &PreSignatureFilter::default(), &[]).await;
        }

        #[ignore]
        #[tokio::test]
        async fn recreated_in_a_following_block() {
            let (db, event_index) = setup().await;
            add_presignature(&db, OWNER, UID, event_index, false).await;
            let presignature =
                add_presignature(&db, OWNER, UID, bump_block_number_by(&event_index, 1), true)
                    .await;
            assert_presignatures(&db, &PreSignatureFilter::default(), &[presignature]).await;
        }

        #[ignore]
        #[tokio::test]
        async fn deleted_later_in_the_same_block() {
            let (db, event_index) = setup().await;
            add_presignature(&db, OWNER, UID, event_index, true).await;

            add_presignature(&db, OWNER, UID, bump_log_index_by(&event_index, 1), false).await;
            assert_presignatures(&db, &PreSignatureFilter::default(), &[]).await;
        }

        #[ignore]
        #[tokio::test]
        async fn recreated_later_in_the_same_block() {
            let (db, event_index) = setup().await;
            add_presignature(&db, OWNER, UID, event_index, false).await;
            let presignature =
                add_presignature(&db, OWNER, UID, bump_log_index_by(&event_index, 1), true).await;
            assert_presignatures(&db, &PreSignatureFilter::default(), &[presignature]).await;
        }

        #[ignore]
        #[tokio::test]
        async fn created_twice() {
            let (db, event_index) = setup().await;
            add_presignature(&db, OWNER, UID, event_index, true).await;
            let presignature =
                add_presignature(&db, OWNER, UID, bump_block_number_by(&event_index, 1), true)
                    .await;
            assert_presignatures(&db, &PreSignatureFilter::default(), &[presignature]).await;
        }
    }
}
