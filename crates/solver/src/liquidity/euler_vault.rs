use {
    super::{SettlementHandling},
    crate::{
        liquidity_collector::LiquidityCollecting,
        interactions::{
            allowances::{AllowanceManager, AllowanceManaging, Allowances, Approval}, EulerVaultDepositInteraction, EulerVaultWithdrawInteraction
        },
        liquidity::{AmmOrderExecution, WrappedLiquidityOrder},
        settlement::SettlementEncoder,
    },
    anyhow::Result,
    contracts::{alloy::EulerVault, GPv2Settlement},
    ethrpc::{
        alloy::conversions::IntoLegacy,
        block_stream::{into_stream, CurrentBlockWatcher},
    },
    futures::StreamExt,
    itertools::Itertools,
    model::{order::OrderKind, TokenPair},
    primitive_types::{H160, U256},
    shared::{
        ethrpc::Web3,
        http_solver::model::TokenAmount,
    },
    std::{
        sync::{Arc, Mutex},
    },
    tracing::instrument
};

pub struct EulerVaultLiquidity {
    // todo: remove Arc
    pub vault: Arc<EulerVault::Instance>,
    pub allowance_manager: Box<dyn AllowanceManaging>,
}

impl EulerVaultLiquidity {
    pub async fn new(
        web3: Web3,
        vault: EulerVault::Instance,
        gpv2: GPv2Settlement,
        blocks_stream: CurrentBlockWatcher,
    ) -> Self {
        let gpv2_address = gpv2.address();
        let allowance_manager = AllowanceManager::new(web3, gpv2_address);

        Self {
            vault: Arc::new(vault),
            allowance_manager: Box::new(allowance_manager),
        }
    }
}

#[derive(Clone)]
pub struct EulerSettlementHandler {
    // todo: remove Arc
    pub vault: Arc<EulerVault::Instance>,
    pub base_asset: H160,
    allowances: Arc<Mutex<Allowances>>,
    gpv2_settlement: GPv2Settlement,
}

impl EulerSettlementHandler {
    pub async fn new(
        web3: Web3,
        vault: EulerVault::Instance,
        allowances: Mutex<Allowances>,
        gpv2: GPv2Settlement,
    ) -> Result<Self> {
        // during the async function is when we figure out what the base asset is
        let base_asset = vault.asset().call().await?;

        Ok(Self {
            vault: Arc::new(vault),
            base_asset: base_asset.into_legacy(),
            allowances: Arc::new(allowances),
            gpv2_settlement: gpv2,
        })
    }

    pub fn settle_deposit(
        &self,
        token_amount_in_max: TokenAmount,
    ) -> (Option<Approval>, EulerVaultDepositInteraction) {
        let approval = self
            .allowances
            .lock()
            .expect("Thread holding mutex panicked")
            .approve_token_or_default(token_amount_in_max.clone());

        (
            approval,
            EulerVaultDepositInteraction {
                deposit_amount: token_amount_in_max.amount,
                receiver: self.gpv2_settlement.address(),
                vault: self.vault.clone(),
            },
        )
    }

    pub fn settle_withdraw(
        &self,
        token_amount_in_max: TokenAmount,
    ) -> EulerVaultWithdrawInteraction {
        EulerVaultWithdrawInteraction {
            redeem_amount: token_amount_in_max.amount,
            provider: self.gpv2_settlement.address(),
            receiver: self.gpv2_settlement.address(),
            vault: self.vault.clone(),
        }
    }
}

#[async_trait::async_trait]
impl LiquidityCollecting for EulerVaultLiquidity {
    /// Given a list of offchain orders returns the list of AMM liquidity to be
    /// considered
    #[instrument(name = "uniswap_like_liquidity", skip_all)]
    async fn get_liquidity(
        &self,
        pairs: HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<Vec<Liquidity>> {
        let mut tokens = HashSet::new();
        let mut result = Vec::new();
        for pool in self.pool_fetcher.fetch(pairs, at_block).await? {
            tokens.insert(pool.tokens.get().0);
            tokens.insert(pool.tokens.get().1);

            result.push(Liquidity::ConstantProduct(ConstantProductOrder {
                address: pool.address,
                tokens: pool.tokens,
                reserves: pool.reserves,
                fee: pool.fee,
                settlement_handling: self.inner.clone(),
            }))
        }
        self.cache_allowances(tokens).await?;
        Ok(result)
    }
}


impl SettlementHandling<WrappedLiquidityOrder> for EulerSettlementHandler {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn encode(
        &self,
        execution: AmmOrderExecution,
        encoder: &mut SettlementEncoder,
    ) -> Result<()> {
        if self.is_euler_vault(execution.output.token) {
            let (approval, swap) = self.settle_deposit(execution.input_max);
            if let Some(approval) = approval {
                encoder.append_to_execution_plan_internalizable(
                    Arc::new(approval),
                    execution.internalizable,
                );
            }
        } else {
            // this does not seem internalizable but please let me know if I think about this
            // incorrectly
            encoder.append_to_execution_plan(Arc::new(self.settle_withdraw(execution.input_max)));
        }
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use {
        super::*,
        shared::{
            baseline_solver::BaseTokens,
        },
        std::collections::HashSet
    };

    fn get_relevant_pairs(token_a: H160, token_b: H160) -> HashSet<TokenPair> {
        let base_tokens = Arc::new(BaseTokens::new(H160::zero(), &[]));
        let fake_order = [TokenPair::new(token_a, token_b).unwrap()].into_iter();
        base_tokens.relevant_pairs(fake_order)
    }
}
