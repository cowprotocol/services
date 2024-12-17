use {
    crate::{
        domain::{
            competition::{
                bad_tokens::{cache::Cache, Quality},
                Order,
            },
            eth,
        },
        infra,
    },
    model::interaction::InteractionData,
    shared::bad_token::{trace_call::TraceCallDetectorRaw, TokenQuality},
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
}

impl Detector {
    pub fn new(max_age: Duration, eth: &infra::Ethereum) -> Self {
        let detector =
            TraceCallDetectorRaw::new(eth.web3().clone(), eth.contracts().settlement().address());
        Self(Arc::new(Inner {
            cache: Cache::new(max_age),
            detector,
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

        let token = order.sell.token;
        let pre_interactions: Vec<_> = order
            .pre_interactions
            .iter()
            .map(|i| InteractionData {
                target: i.target.0,
                value: i.value.0,
                call_data: i.call_data.0.clone(),
            })
            .collect();

        match self
            .0
            .detector
            .test_transfer(
                eth::Address::from(order.trader()).0,
                token.0 .0,
                order.sell.amount.0,
                &pre_interactions,
            )
            .await
        {
            Err(err) => {
                tracing::debug!(?err, "failed to determine token quality");
                None
            }
            Ok(TokenQuality::Good) => {
                cache.update_quality(token, Quality::Supported, now);
                Some(Quality::Supported)
            }
            Ok(TokenQuality::Bad { reason }) => {
                tracing::debug!(reason, "cache token as unsupported");
                cache.update_quality(token, Quality::Unsupported, now);
                Some(Quality::Unsupported)
            }
        }
    }
}

impl std::ops::Deref for Detector {
    type Target = Cache;

    fn deref(&self) -> &Self::Target {
        &self.0.cache
    }
}
