mod detector;

use {
    self::detector::{DetectionError, Detector},
    crate::code_simulation::CodeSimulating,
    anyhow::Context as _,
    cached::{Cached, SizedCache},
    ethcontract::{Address, H256, U256},
    ethrpc::extensions::StateOverride,
    maplit::hashmap,
    std::{
        collections::HashMap,
        fmt::{self, Display, Formatter},
        str::FromStr,
        sync::{Arc, Mutex},
    },
    web3::signing,
};

/// Balance override configuration arguments.
#[derive(clap::Parser)]
#[group(skip)]
pub struct Arguments {
    /// Token configuration for simulated balances on verified quotes. This
    /// allows the quote verification system to produce verified quotes for
    /// traders without sufficient balance for the configured token pairs.
    ///
    /// The expected format is a comma separated list of `${ADDR}@${SLOT}`,
    /// where `ADDR` is the token address and `SLOT` is the Solidity storage
    /// slot for the balances mapping. For example for WETH:
    /// `0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2@3`.
    #[clap(long, env, default_value_t)]
    pub quote_token_balance_overrides: TokenConfiguration,

    /// Enable automatic detection of token balance overrides. Note that
    /// pre-configured values with the `--quote-token-balance-overrides` flag
    /// will take precedence.
    #[clap(long, env, action = clap::ArgAction::Set, default_value_t)]
    pub quote_autodetect_token_balance_overrides: bool,
}

impl Arguments {
    const CACHE_SIZE: usize = 1000;

    /// Creates a balance overrides instance from the current configuration.
    pub fn init(&self, simulator: Arc<dyn CodeSimulating>) -> Arc<dyn BalanceOverriding> {
        Arc::new(BalanceOverrides {
            hardcoded: self.quote_token_balance_overrides.0.clone(),
            detector: self.quote_autodetect_token_balance_overrides.then(|| {
                (
                    Detector::new(simulator),
                    Mutex::new(SizedCache::with_size(Self::CACHE_SIZE)),
                )
            }),
        })
    }
}

impl Display for Arguments {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let Self {
            quote_token_balance_overrides,
            quote_autodetect_token_balance_overrides,
        } = self;

        writeln!(
            f,
            "quote_token_balance_overrides: {:?}",
            quote_token_balance_overrides
        )?;
        writeln!(
            f,
            "quote_autodetect_token_balance_overrides: {:?}",
            quote_autodetect_token_balance_overrides
        )?;

        Ok(())
    }
}

/// Token configurations for the `BalanceOverriding` component.
#[derive(Clone, Debug, Default)]
pub struct TokenConfiguration(HashMap<Address, Strategy>);

impl Display for TokenConfiguration {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let format_entry =
            |f: &mut Formatter, (addr, strategy): (&Address, &Strategy)| match strategy {
                Strategy::Mapping { slot } => write!(f, "{addr:?}@{slot}"),
            };

        let mut entries = self.0.iter();

        let Some(first) = entries.next() else {
            return Ok(());
        };
        format_entry(f, first)?;

        for entry in entries {
            f.write_str(",")?;
            format_entry(f, entry)?;
        }

        Ok(())
    }
}

impl FromStr for TokenConfiguration {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Ok(Self::default());
        }

        let entries = s
            .split(',')
            .map(|part| -> Result<_, Self::Err> {
                let (addr, slot) = part
                    .split_once('@')
                    .context("expected {addr}@{slot} format")?;
                Ok((
                    addr.parse()?,
                    Strategy::Mapping {
                        slot: slot.parse()?,
                    },
                ))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self(entries))
    }
}

/// A component that can provide balance overrides for tokens.
///
/// This allows a wider range of verified quotes to work, even when balances
/// are not available for the quoter.
#[async_trait::async_trait]
pub trait BalanceOverriding: Send + Sync + 'static {
    async fn state_override(&self, request: BalanceOverrideRequest) -> Option<StateOverride>;
}

/// Parameters for computing a balance override request.
pub struct BalanceOverrideRequest {
    /// The token for the override.
    pub token: Address,
    /// The account to override the balance for.
    pub holder: Address,
    /// The token amount (in atoms) to set the balance to.
    pub amount: U256,
}

/// Balance override strategy for a token.
#[derive(Clone, Debug)]
pub enum Strategy {
    /// Balance override strategy for tokens whose balances are stored in a
    /// direct Solidity mapping from token holder to balance amount in the
    /// form `mapping(address holder => uint256 amount)`.
    ///
    /// The strategy is configured with the storage slot [^1] of the mapping.
    ///
    /// [^1]: <https://docs.soliditylang.org/en/latest/internals/layout_in_storage.html#mappings-and-dynamic-arrays>
    Mapping { slot: U256 },
}

impl Strategy {
    /// Computes the storage slot and value to override for a particular token
    /// holder and amount.
    fn state_override(&self, holder: &Address, amount: &U256) -> (H256, H256) {
        match self {
            Self::Mapping { slot } => {
                let key = {
                    let mut buf = [0; 64];
                    buf[12..32].copy_from_slice(holder.as_fixed_bytes());
                    slot.to_big_endian(&mut buf[32..64]);
                    H256(signing::keccak256(&buf))
                };
                let value = {
                    let mut buf = [0; 32];
                    amount.to_big_endian(&mut buf);
                    H256(buf)
                };
                (key, value)
            }
        }
    }
}

type DetectorCache = Mutex<SizedCache<Address, Option<Strategy>>>;

/// The default balance override provider.
#[derive(Debug, Default)]
pub struct BalanceOverrides {
    /// The configured balance override strategies for tokens.
    ///
    /// These take priority over the auto-detection mechanism and are excluded
    /// from the cache in order to prevent them from getting cleaned up by
    /// the caching policy.
    hardcoded: HashMap<Address, Strategy>,
    /// The balance override detector and its cache. Set to `None` if
    /// auto-detection is not enabled.
    detector: Option<(Detector, DetectorCache)>,
}

impl BalanceOverrides {
    async fn cached_detection(&self, token: Address) -> Option<Strategy> {
        let (detector, cache) = self.detector.as_ref()?;
        tracing::debug!(?token, "attempting to auto-detect");

        {
            let mut cache = cache.lock().unwrap();
            if let Some(strategy) = cache.cache_get(&token) {
                tracing::debug!(?token, "cache hit");
                return strategy.clone();
            }
        }

        let strategy = detector.detect(token).await;

        // Only cache when we successfully detect the token, or we can't find
        // it. Anything else is likely a temporary simulator (i.e. node) failure
        // which we don't want to cache.
        if matches!(&strategy, Ok(_) | Err(DetectionError::NotFound)) {
            tracing::debug!(?token, ?strategy, "caching result");
            let cached_strategy = strategy.as_ref().ok().cloned();
            cache.lock().unwrap().cache_set(token, cached_strategy);
        } else {
            tracing::warn!(
                ?token,
                ?strategy,
                "error auto-detecting token balance override strategy"
            );
        }

        strategy.ok()
    }
}

#[async_trait::async_trait]
impl BalanceOverriding for BalanceOverrides {
    async fn state_override(&self, request: BalanceOverrideRequest) -> Option<StateOverride> {
        let strategy = if let Some(strategy) = self.hardcoded.get(&request.token) {
            tracing::debug!(token = ?request.token, "using pre-configured balance override strategy");
            Some(strategy.clone())
        } else {
            self.cached_detection(request.token).await
        }?;

        let (key, value) = strategy.state_override(&request.holder, &request.amount);
        tracing::debug!(?strategy, ?key, ?value, "overriding token balance");

        Some(StateOverride {
            state_diff: Some(hashmap! { key => value }),
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use {super::*, hex_literal::hex};

    #[tokio::test]
    async fn balance_override_computation() {
        let balance_overrides = BalanceOverrides {
            hardcoded: hashmap! {
                addr!("DEf1CA1fb7FBcDC777520aa7f396b4E015F497aB") => Strategy::Mapping {
                    slot: U256::from(0),
                },
            },
            ..Default::default()
        };

        assert_eq!(
            balance_overrides
                .state_override(BalanceOverrideRequest {
                    token: addr!("DEf1CA1fb7FBcDC777520aa7f396b4E015F497aB"),
                    holder: addr!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"),
                    amount: 0x42_u64.into(),
                })
                .await,
            Some(StateOverride {
                state_diff: Some(hashmap! {
                    H256(hex!("fca351f4d96129454cfc8ef7930b638ac71fea35eb69ee3b8d959496beb04a33")) =>
                        H256(hex!("0000000000000000000000000000000000000000000000000000000000000042")),
                }),
                ..Default::default()
            }),
        );

        // You can verify the state override computation is correct by running:
        // ```
        // curl -X POST $RPC -H 'Content-Type: application/data' --data '{
        //   "jsonrpc": "2.0",
        //   "id": 0,
        //   "method": "eth_call",
        //   "params": [
        //     {
        //       "to": "0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB",
        //       "data": "0x70a08231000000000000000000000000d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"
        //     },
        //     "latest",
        //     {
        //       "0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB": {
        //         "stateDiff": {
        //           "0xfca351f4d96129454cfc8ef7930b638ac71fea35eb69ee3b8d959496beb04a33":
        //             "0x0000000000000000000000000000000000000000000000000000000000000042"
        //         }
        //       }
        //     }
        //   ]
        // }'
        // ```
    }

    #[tokio::test]
    async fn balance_overrides_none_for_unknown_tokens() {
        let balance_overrides = BalanceOverrides::default();
        assert_eq!(
            balance_overrides
                .state_override(BalanceOverrideRequest {
                    token: addr!("0000000000000000000000000000000000000000"),
                    holder: addr!("0000000000000000000000000000000000000001"),
                    amount: U256::zero(),
                })
                .await,
            None,
        );
    }
}
