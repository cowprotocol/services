use {
    crate::{
        domain::{
            competition::{
                bad_tokens::{cache::Cache, Quality},
                order,
                Order,
            },
            eth,
        },
        infra,
    },
    futures::FutureExt,
    model::interaction::InteractionData,
    shared::{
        bad_token::{trace_call::TraceCallDetectorRaw, TokenQuality},
        request_sharing::BoxRequestSharing,
    },
    std::{
        sync::Arc,
        time::{Duration, Instant},
    },
};

#[derive(Clone)]
pub struct Detector(Arc<Inner>);

struct Inner {
    cache: Cache,
    detector: TraceCallDetectorRaw,
    sharing: BoxRequestSharing<order::Uid, Option<Quality>>,
}

impl Detector {
    pub fn new(max_age: Duration, eth: &infra::Ethereum) -> Self {
        let detector =
            TraceCallDetectorRaw::new(eth.web3().clone(), eth.contracts().settlement().address());
        Self(Arc::new(Inner {
            cache: Cache::new(max_age),
            detector,
            sharing: BoxRequestSharing::labelled("bad_tokens".into()),
        }))
    }

    pub async fn determine_sell_token_quality(
        &self,
        order: &Order,
        now: Instant,
    ) -> Option<Quality> {
        let cache = &self.0.cache;
        if let Some(quality) = cache.get_quality(order.sell.token, now) {
            return Some(quality);
        }

        // The simulation detector gets used by multiple solvers at the same time
        // and therefore will have to handle a lot of duplicate requests. To avoid
        // doing duplicate work we use the `RequestSharing` component which checks
        // if an equivalent request is already in-flight and awaits that instead of
        // creating a new one.
        let uid = order.uid;
        self.0
            .sharing
            .shared_or_else(uid, move |_uid| {
                let inner = self.0.clone();
                let sell_token = order.sell.token;
                let pre_interactions: Vec<_> = order
                    .pre_interactions
                    .iter()
                    .map(|i| InteractionData {
                        target: i.target.0,
                        value: i.value.0,
                        call_data: i.call_data.0.clone(),
                    })
                    .collect();
                let trader = eth::Address::from(order.trader()).0;
                let sell_amount = order.sell.amount.0;

                async move {
                    let result = inner
                        .detector
                        .test_transfer(trader, sell_token.0 .0, sell_amount, &pre_interactions)
                        .await;
                    match result {
                        Err(err) => {
                            tracing::debug!(?err, "failed to determine token quality");
                            None
                        }
                        Ok(TokenQuality::Good) => {
                            inner
                                .cache
                                .update_quality(sell_token, Quality::Supported, now);
                            Some(Quality::Supported)
                        }
                        Ok(TokenQuality::Bad { reason }) => {
                            tracing::debug!(reason, "cache token as unsupported");
                            inner
                                .cache
                                .update_quality(sell_token, Quality::Unsupported, now);
                            Some(Quality::Unsupported)
                        }
                    }
                }
                .boxed()
            })
            .await
    }
}

impl std::ops::Deref for Detector {
    type Target = Cache;

    fn deref(&self) -> &Self::Target {
        &self.0.cache
    }
}
