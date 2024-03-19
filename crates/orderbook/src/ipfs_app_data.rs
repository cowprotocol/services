use {
    crate::ipfs::Ipfs,
    anyhow::Result,
    app_data::AppDataHash,
    cached::{Cached, TimedSizedCache},
    std::sync::Mutex,
};

pub struct IpfsAppData {
    ipfs: Ipfs,
    cache: Mutex<TimedSizedCache<AppDataHash, Option<String>>>,
    metrics: &'static Metrics,
}

#[derive(prometheus_metric_storage::MetricStorage, Clone, Debug)]
#[metric(subsystem = "ipfs")]
struct Metrics {
    /// Number of completed IPFS app data fetches.
    #[metric(labels("outcome", "source"))]
    app_data: prometheus::IntCounterVec,

    /// Timing of IPFS app data fetches.
    fetches: prometheus::Histogram,
}

impl IpfsAppData {
    pub fn new(ipfs: Ipfs) -> Self {
        let metrics = Metrics::instance(observe::metrics::get_storage_registry()).unwrap();
        // Initialize metrics.
        for outcome in &["error", "found", "missing"] {
            for source in &["cache", "node"] {
                metrics.app_data.with_label_values(&[outcome, source]);
            }
        }
        Self {
            ipfs,
            cache: Mutex::new(TimedSizedCache::with_size_and_lifespan_and_refresh(
                1000, 600, false,
            )),
            metrics,
        }
    }

    /// Tries to find full app data corresponding to the contract app data on
    /// IPFS.
    ///
    /// A return value of `Some` indicates that either the old or new CID format
    /// was found on IPFS and points to valid utf-8.
    ///
    /// A return value of `None` indicates that neither CID was found. This
    /// might be a temporary condition as IPFS is a decentralized network.
    ///
    /// A return value of `Err` indicates an error communication with the IPFS
    /// gateway.
    async fn fetch_raw(&self, contract_app_data: &AppDataHash) -> Result<Option<String>> {
        let old = old_app_data_cid(contract_app_data);
        let new = new_app_data_cid(contract_app_data);
        let fetch = |cid: String| async move {
            let result = self.ipfs.fetch(&cid).await;
            let result = match result {
                Ok(Some(result)) => {
                    tracing::debug!(?contract_app_data, %cid, "found full app data");
                    result
                }
                Ok(None) => {
                    tracing::debug!(?contract_app_data, %cid,"no full app data");
                    return Ok(None);
                }
                Err(err) => {
                    tracing::warn!(?contract_app_data, %cid, ?err, "failed full app data");
                    return Err(err);
                }
            };
            match String::from_utf8(result) {
                Ok(result) => Ok(Some(result)),
                Err(err) => {
                    tracing::debug!(?err, %cid, "CID doesn't point to utf-8");
                    Ok(None)
                }
            }
        };
        futures::future::select_ok([std::pin::pin!(fetch(old)), std::pin::pin!(fetch(new))])
            .await
            .map(|(ok, _rest)| ok)
    }

    pub async fn fetch(&self, contract_app_data: &AppDataHash) -> Result<Option<String>> {
        let outcome = |data: &Option<String>| if data.is_some() { "found" } else { "missing" };

        let metric = &self.metrics.app_data;
        if let Some(cached) = self
            .cache
            .lock()
            .unwrap()
            .cache_get(contract_app_data)
            .cloned()
        {
            metric.with_label_values(&[outcome(&cached), "cache"]).inc();
            return Ok(cached);
        }

        let fetched = {
            let _timer = self.metrics.fetches.start_timer();
            self.fetch_raw(contract_app_data).await
        };
        let result = match fetched {
            Ok(result) => result,
            Err(err) => {
                metric.with_label_values(&["error", "node"]).inc();
                return Err(err);
            }
        };

        self.cache
            .lock()
            .unwrap()
            .cache_set(*contract_app_data, result.clone());
        metric.with_label_values(&[outcome(&result), "node"]).inc();
        Ok(result)
    }
}

fn new_app_data_cid(contract_app_data: &AppDataHash) -> String {
    let raw_cid = app_data_hash::create_ipfs_cid(&contract_app_data.0);
    multibase::encode(multibase::Base::Base32Lower, raw_cid)
}

fn old_app_data_cid(contract_app_data: &AppDataHash) -> String {
    let mut raw_cid = [0u8; 4 + 32];
    raw_cid[0] = 1; // cid version
    raw_cid[1] = 0x70; // dag-pb
    raw_cid[2] = 0x12; // sha2-256
    raw_cid[3] = 32; // hash length
    raw_cid[4..].copy_from_slice(&contract_app_data.0);
    multibase::encode(multibase::Base::Base32Lower, raw_cid)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Can be compared with CID explorer to make sure CIDs encode the right data.
    #[test]
    fn cid() {
        let hash = AppDataHash(hex_literal::hex!(
            "8af4e8c9973577b08ac21d17d331aade86c11ebcc5124744d621ca8365ec9424"
        ));
        let cid = new_app_data_cid(&hash);
        println!("{cid}");

        let hash = AppDataHash(hex_literal::hex!(
            "AE16F2D8B960FFE3DE70E074CECFB24441D4CDC67E8A68566B9E6CE3037CB41D"
        ));
        let cid = old_app_data_cid(&hash);
        println!("{cid}");
    }

    #[ignore]
    #[tokio::test]
    async fn fetch() {
        let ipfs = Ipfs::new(Default::default(), "https://ipfs.io".parse().unwrap(), None);
        let ipfs = IpfsAppData::new(ipfs);
        let hash = AppDataHash::default();
        let result = ipfs.fetch(&hash).await;
        let _ = dbg!(result);
        let hash = AppDataHash(hex_literal::hex!(
            "8af4e8c9973577b08ac21d17d331aade86c11ebcc5124744d621ca8365ec9424"
        ));
        let result = ipfs.fetch(&hash).await;
        let _ = dbg!(result);
    }
}
