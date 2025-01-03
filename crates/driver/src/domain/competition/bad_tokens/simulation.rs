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

/// Component to detect tokens which show unusual behavior during
/// transfers. These tokens are likely not supported by less advanced
/// solvers. Checks the behavior on transfer using a `trace_callMany`
/// based simulation.
#[derive(Clone)]
pub struct Detector(Arc<Inner>);

struct Inner {
    cache: Cache,
    detector: TraceCallDetectorRaw,
    sharing: BoxRequestSharing<order::Uid, Quality>,
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

    /// Simulates how the sell token behaves during transfers. Assumes that
    /// the order owner has the required sell token balance and approvals
    /// set.
    pub async fn determine_sell_token_quality(&self, order: &Order, now: Instant) -> Quality {
        let cache = &self.0.cache;
        let quality = cache.get_quality(&order.sell.token, now);
        if quality != Quality::Unknown {
            return quality;
        }

        // The simulation detector gets used by multiple solvers at the same time
        // and therefore will have to handle a lot of duplicate requests. To avoid
        // doing unnecessary work we use the `RequestSharing` component which checks
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
                let sell_amount = match order.partial {
                    order::Partial::Yes { available } => available.0,
                    order::Partial::No => order.sell.amount.0,
                };

                async move {
                    let result = inner
                        .detector
                        .test_transfer(trader, sell_token.0 .0, sell_amount, &pre_interactions)
                        .await;
                    match result {
                        Err(err) => {
                            tracing::debug!(?err, token=?sell_token.0, "failed to determine token quality");
                            Quality::Unknown
                        }
                        Ok(TokenQuality::Good) => {
                            inner
                                .cache
                                .update_quality(sell_token, true, now);
                            Quality::Supported
                        }
                        Ok(TokenQuality::Bad { reason }) => {
                            tracing::debug!(reason, token=?sell_token.0, "cache token as unsupported");
                            inner
                                .cache
                                .update_quality(sell_token, false, now);
                            Quality::Unsupported
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
