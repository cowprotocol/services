pub mod detector;

use {
    self::detector::{DetectionError, Detector},
    alloy::{
        primitives::{Address, B256, U256, keccak256, map::AddressMap},
        rpc::types::state::AccountOverride,
    },
    anyhow::Context as _,
    cached::{Cached, SizedCache},
    std::{
        collections::HashMap,
        fmt::{self, Display, Formatter},
        iter,
        str::FromStr,
        sync::{Arc, Mutex},
    },
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

    /// Controls how many storage slots get probed per storage entry point
    /// for automatically detecting how to override the balances of a token.
    #[clap(long, env, action = clap::ArgAction::Set, default_value = "60")]
    pub quote_autodetect_token_balance_overrides_probing_depth: u8,

    /// Controls for how many tokens we store the result of the automatic
    /// balance override detection before evicting less used entries.
    #[clap(long, env, action = clap::ArgAction::Set, default_value = "1000")]
    pub quote_autodetect_token_balance_overrides_cache_size: usize,
}

impl Arguments {
    /// Creates a balance overrides instance from the current configuration.
    pub fn init(&self, web3: ethrpc::Web3) -> Arc<dyn BalanceOverriding> {
        Arc::new(BalanceOverrides {
            hardcoded: self.quote_token_balance_overrides.0.clone(),
            detector: self.quote_autodetect_token_balance_overrides.then(|| {
                (
                    Detector::new(
                        web3,
                        self.quote_autodetect_token_balance_overrides_probing_depth,
                    ),
                    Mutex::new(SizedCache::with_size(
                        self.quote_autodetect_token_balance_overrides_cache_size,
                    )),
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
            quote_autodetect_token_balance_overrides_probing_depth,
            quote_autodetect_token_balance_overrides_cache_size,
        } = self;

        writeln!(
            f,
            "quote_token_balance_overrides: {quote_token_balance_overrides:?}"
        )?;
        writeln!(
            f,
            "quote_autodetect_token_balance_overrides: \
             {quote_autodetect_token_balance_overrides:?}"
        )?;
        writeln!(
            f,
            "quote_autodetect_token_balance_overrides_probing_depth: \
             {quote_autodetect_token_balance_overrides_probing_depth:?}"
        )?;
        writeln!(
            f,
            "quote_autodetect_token_balance_overrides_cache_size: \
             {quote_autodetect_token_balance_overrides_cache_size:?}"
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
                Strategy::SolidityMapping {
                    target_contract,
                    map_slot,
                } => write!(
                    f,
                    "SolidityMapping({addr:?}: {target_contract:?}@{map_slot})"
                ),
                Strategy::SoladyMapping { target_contract } => {
                    write!(f, "SoladyMapping({addr:?}: {target_contract})")
                }
                Strategy::DirectSlot {
                    target_contract,
                    slot,
                } => write!(f, "DirectSlot({addr:?}: {target_contract:?}@{slot})"),
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
                    Strategy::SolidityMapping {
                        target_contract: addr.parse()?,
                        map_slot: slot.parse()?,
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
    async fn state_override(
        &self,
        request: BalanceOverrideRequest,
    ) -> Option<(Address, AccountOverride)>;
}

/// Parameters for computing a balance override request.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct BalanceOverrideRequest {
    /// The token for the override.
    pub token: Address,
    /// The account to override the balance for.
    pub holder: Address,
    /// The token amount (in atoms) to set the balance to.
    pub amount: U256,
}

/// Balance override strategy for a token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Strategy {
    /// Balance override strategy for tokens whose balances are stored in a
    /// direct Solidity mapping from token holder to balance amount in the
    /// form `mapping(address holder => uint256 amount)`.
    ///
    /// The strategy is configured with the storage slot [^1] of the mapping.
    ///
    /// [^1]: <https://docs.soliditylang.org/en/latest/internals/layout_in_storage.html#mappings-and-dynamic-arrays>
    SolidityMapping {
        target_contract: Address,
        map_slot: U256,
    },
    /// Strategy computing storage slot for balances based on the Solady library
    /// [^1].
    ///
    /// [^1]: <https://github.com/Vectorized/solady/blob/6122858a3aed96ee9493b99f70a245237681a95f/src/tokens/ERC20.sol#L75-L81>
    SoladyMapping { target_contract: Address },
    /// Strategy that directly uses the storage slot discovered via
    /// debug_traceCall. This is similar to Foundry's `deal` approach where
    /// we trace a balanceOf call to find which storage slot is accessed for
    /// a given account.
    DirectSlot {
        target_contract: Address,
        slot: B256,
    },
}

impl Strategy {
    /// Computes the storage slot and value to override for a particular token
    /// holder and amount.
    fn state_override(&self, holder: &Address, amount: &U256) -> AddressMap<AccountOverride> {
        let (target_contract, key) = match self {
            Self::SolidityMapping {
                target_contract,
                map_slot,
            } => {
                let mut buf = [0; 64];
                buf[12..32].copy_from_slice(holder.as_slice());
                buf[32..64].copy_from_slice(&map_slot.to_be_bytes::<32>());
                (target_contract, keccak256(buf))
            }
            Self::SoladyMapping { target_contract } => {
                let mut buf = [0; 32];
                buf[0..20].copy_from_slice(holder.as_slice());
                buf[28..32].copy_from_slice(&[0x87, 0xa2, 0x11, 0xa2]);
                (target_contract, keccak256(buf))
            }
            Self::DirectSlot {
                target_contract,
                slot,
            } => (target_contract, *slot),
        };

        let state_override = AccountOverride {
            state_diff: Some(iter::once((key, B256::new(amount.to_be_bytes::<32>()))).collect()),
            ..Default::default()
        };

        iter::once((*target_contract, state_override)).collect()
    }

    fn is_valid_for_all_holders(&self) -> bool {
        matches!(self, Self::DirectSlot { .. })
    }
}

type DetectorCache = Mutex<SizedCache<(Address, Option<Address>), Option<Strategy>>>;

/// The default balance override provider.
#[derive(Debug, Default)]
pub struct BalanceOverrides {
    /// The configured balance override strategies for tokens.
    ///
    /// These take priority over the auto-detection mechanism and are excluded
    /// from the cache in order to prevent them from getting cleaned up by
    /// the caching policy.
    pub hardcoded: HashMap<Address, Strategy>,
    /// The balance override detector and its cache. Set to `None` if
    /// auto-detection is not enabled.
    pub detector: Option<(Detector, DetectorCache)>,
}

impl BalanceOverrides {
    /// Creates a new instance with sensible defaults.
    pub fn new(web3: ethrpc::Web3) -> Self {
        Self {
            hardcoded: Default::default(),
            detector: Some((
                Detector::new(web3, 60),
                Mutex::new(SizedCache::with_size(1000)),
            )),
        }
    }

    pub(crate) async fn cached_detection(
        &self,
        token: Address,
        holder: Address,
    ) -> Option<Strategy> {
        let (detector, cache) = self.detector.as_ref()?;
        tracing::trace!(?token, "attempting to auto-detect");

        {
            let mut cache = cache.lock().unwrap();
            if let Some(strategy) = cache.cache_get(&(token, None)) {
                tracing::trace!(?token, "cache hit (strategy valid for all holders)");
                return strategy.clone();
            }
            if let Some(strategy) = cache.cache_get(&(token, Some(holder))) {
                tracing::trace!(?token, ?holder, "cache hit (holder-specific strategy)");
                return strategy.clone();
            }
        }

        let strategy = detector.detect(token, holder).await;

        // Only cache when we successfully detect the token, or we can't find
        // it. Anything else is likely a temporary simulator (i.e. node) failure
        // which we don't want to cache.
        if matches!(&strategy, Ok(_) | Err(DetectionError::NotFound)) {
            tracing::debug!(?token, ?strategy, "caching auto-detected strategy");
            if let Ok(strategy) = strategy.as_ref() {
                let cache_key = (
                    token,
                    (!strategy.is_valid_for_all_holders()).then_some(holder),
                );
                cache
                    .lock()
                    .unwrap()
                    .cache_set(cache_key, Some(strategy.clone()));
            } else {
                // strategy is Err(DetectionError::NotFound)
                cache.lock().unwrap().cache_set((token, Some(holder)), None);
            }
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
    async fn state_override(
        &self,
        request: BalanceOverrideRequest,
    ) -> Option<(Address, AccountOverride)> {
        let strategy = if let Some(strategy) = self.hardcoded.get(&request.token) {
            tracing::trace!(token = ?request.token, "using pre-configured balance override strategy");
            Some(strategy.clone())
        } else {
            self.cached_detection(request.token, request.holder).await
        }?;

        strategy
            .state_override(&request.holder, &request.amount)
            .into_iter()
            .last()
    }
}

/// Balance overrider that always returns `None`. That can be
/// useful for testing.
pub struct DummyOverrider;

#[async_trait::async_trait]
impl BalanceOverriding for DummyOverrider {
    async fn state_override(
        &self,
        _request: BalanceOverrideRequest,
    ) -> Option<(Address, AccountOverride)> {
        None
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy::primitives::{address, b256},
        ethrpc::mock,
        maplit::hashmap,
    };

    #[tokio::test]
    async fn balance_override_computation() {
        let cow = address!("DEf1CA1fb7FBcDC777520aa7f396b4E015F497aB");
        let balance_overrides = BalanceOverrides {
            hardcoded: hashmap! {
                cow => Strategy::SolidityMapping {
                    target_contract: cow,
                    map_slot: U256::from(0),
                },
            },
            ..Default::default()
        };

        assert_eq!(
            balance_overrides
                .state_override(BalanceOverrideRequest {
                    token: cow,
                    holder: address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"),
                    amount: U256::from(0x42),
                })
                .await,
            Some((
                cow,
                AccountOverride {
                    state_diff: Some(
                        iter::once((
                            b256!(
                                "fca351f4d96129454cfc8ef7930b638ac71fea35eb69ee3b8d959496beb04a33"
                            ),
                            b256!(
                                "0000000000000000000000000000000000000000000000000000000000000042"
                            )
                        ))
                        .collect()
                    ),
                    ..Default::default()
                }
            )),
        );

        // You can verify the state override computation is correct by running:
        // ```
        // curl -X POST $RPC -H 'Content-Type: application/json' --data '{
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
                    token: address!("0000000000000000000000000000000000000000"),
                    holder: address!("0000000000000000000000000000000000000001"),
                    amount: U256::ZERO,
                })
                .await,
            None,
        );
    }

    #[tokio::test]
    async fn balance_override_computation_solady() {
        let token = address!("0000000000c5dc95539589fbd24be07c6c14eca4");
        let balance_overrides = BalanceOverrides {
            hardcoded: hashmap! {
                token => Strategy::SoladyMapping { target_contract: address!("0000000000c5dc95539589fbd24be07c6c14eca4") },
            },
            ..Default::default()
        };

        assert_eq!(
            balance_overrides
                .state_override(BalanceOverrideRequest {
                    token,
                    holder: address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"),
                    amount: U256::from(0x42),
                })
                .await,
            Some((
                token,
                AccountOverride {
                    state_diff: Some({
                        iter::once((
                            b256!(
                                "f6a6656ed2d14bad3cdd3e8871db3f535a136a1b6cd5ae2dced8eb813f3d4e4f"
                            ),
                            b256!(
                                "0000000000000000000000000000000000000000000000000000000000000042"
                            ),
                        ))
                        .collect()
                    }),
                    ..Default::default()
                }
            )),
        );

        // You can verify the state override computation is correct by running:
        // ```
        // curl -X POST $RPC -H 'Content-Type: application/json' --data '{
        //   "jsonrpc": "2.0",
        //   "id": 0,
        //   "method": "eth_call",
        //   "params": [
        //     {
        //       "to": "0x0000000000c5dc95539589fbd24be07c6c14eca4",
        //       "data": "0x70a08231000000000000000000000000d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"
        //     },
        //     "latest",
        //     {
        //       "0x0000000000c5dc95539589fbd24be07c6c14eca4": {
        //         "stateDiff": {
        //           "f6a6656ed2d14bad3cdd3e8871db3f535a136a1b6cd5ae2dced8eb813f3d4e4f":
        //             "0x0000000000000000000000000000000000000000000000000000000000000042"
        //         }
        //       }
        //     }
        //   ]
        // }'
        // ```
    }

    #[tokio::test]
    async fn cached_detection_caches_holder_agnostic_strategies_without_holder() {
        let token = address!("DEf1CA1fb7FBcDC777520aa7f396b4E015F497aB");
        let holder1 = address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045");
        let holder2 = address!("0000000000000000000000000000000000000001");
        let target_contract = address!("0000000000000000000000000000000000000002");

        let strategy = Strategy::SolidityMapping {
            target_contract,
            map_slot: U256::from(3),
        };

        // Create a mock web3 and convert it to the expected type
        let mock_web3 = mock::web3();
        let balance_overrides = BalanceOverrides {
            hardcoded: Default::default(),
            detector: Some((
                Detector::new(mock_web3, 60),
                Mutex::new(SizedCache::with_size(100)),
            )),
        };

        // Manually populate the cache as if detector found this holder-agnostic
        // strategy
        {
            let (_, cache) = balance_overrides.detector.as_ref().unwrap();
            cache
                .lock()
                .unwrap()
                .cache_set((token, None), Some(strategy.clone()));
        }

        // Both holders should retrieve the same cached strategy since it's valid for
        // all holders
        assert_eq!(
            balance_overrides.cached_detection(token, holder1).await,
            Some(strategy.clone())
        );
        assert_eq!(
            balance_overrides.cached_detection(token, holder2).await,
            Some(strategy)
        );
    }

    #[tokio::test]
    async fn cached_detection_caches_holder_specific_strategies_with_holder() {
        let token = address!("DEf1CA1fb7FBcDC777520aa7f396b4E015F497aB");
        let holder1 = address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045");
        let holder2 = address!("0000000000000000000000000000000000000001");
        let target_contract = address!("0000000000000000000000000000000000000002");

        let strategy_h1 = Strategy::DirectSlot {
            target_contract,
            slot: B256::repeat_byte(1),
        };
        let strategy_h2 = Strategy::DirectSlot {
            target_contract,
            slot: B256::repeat_byte(2),
        };

        // Create a mock web3 and convert it to the expected type
        let mock_web3 = mock::web3();
        let balance_overrides = BalanceOverrides {
            hardcoded: Default::default(),
            detector: Some((
                Detector::new(mock_web3, 60),
                Mutex::new(SizedCache::with_size(100)),
            )),
        };

        // Manually populate cache with holder-specific strategies
        {
            let (_, cache) = balance_overrides.detector.as_ref().unwrap();
            cache
                .lock()
                .unwrap()
                .cache_set((token, Some(holder1)), Some(strategy_h1.clone()));
            cache
                .lock()
                .unwrap()
                .cache_set((token, Some(holder2)), Some(strategy_h2.clone()));
        }

        // Each holder should retrieve their specific cached strategy
        assert_eq!(
            balance_overrides.cached_detection(token, holder1).await,
            Some(strategy_h1)
        );
        assert_eq!(
            balance_overrides.cached_detection(token, holder2).await,
            Some(strategy_h2)
        );
    }
}
