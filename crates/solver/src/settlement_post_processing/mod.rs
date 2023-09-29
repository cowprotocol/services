mod optimize_buffer_usage;
mod optimize_score;
mod optimize_unwrapping;

use {
    crate::{
        settlement::Settlement,
        settlement_simulation::simulate_and_estimate_gas_at_current_block,
        solver::{http_solver::buffers::BufferRetriever, risk_computation::RiskCalculator},
    },
    anyhow::{Context, Result},
    contracts::{GPv2Settlement, WETH9},
    ethcontract::{Account, U256},
    gas_estimation::GasPrice1559,
    optimize_buffer_usage::optimize_buffer_usage,
    optimize_score::compute_success_probability,
    optimize_unwrapping::optimize_unwrapping,
    primitive_types::H160,
    shared::{
        ethrpc::Web3,
        http_solver::{self, model::InternalizationStrategy},
        token_list::AutoUpdatingTokenList,
    },
};

/// Determines whether a settlement would be executed successfully.
/// If the settlement would succeed, the gas estimate is returned.
#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait SettlementSimulating: Send + Sync {
    async fn estimate_gas(&self, settlement: Settlement) -> Result<U256>;
}

pub struct SettlementSimulator {
    settlement_contract: GPv2Settlement,
    gas_price: GasPrice1559,
    solver_account: Account,
    internalization: InternalizationStrategy,
}

#[async_trait::async_trait]
impl SettlementSimulating for SettlementSimulator {
    async fn estimate_gas(&self, settlement: Settlement) -> Result<U256> {
        let settlement = settlement.encode(self.internalization);
        simulate_and_estimate_gas_at_current_block(
            std::iter::once((self.solver_account.clone(), settlement, None)),
            &self.settlement_contract,
            self.gas_price,
        )
        .await?
        .pop()
        .context("empty result")?
        .map_err(Into::into)
    }
}

#[async_trait::async_trait]
#[mockall::automock]
pub trait PostProcessing: Send + Sync + 'static {
    /// Tries to apply optimizations to a given settlement. If all optimizations
    /// fail the original settlement gets returned.
    async fn optimize_settlement(
        &self,
        settlement: Settlement,
        solver_account: Account,
        gas_price: GasPrice1559,
        risk_calculator: Option<&RiskCalculator>,
    ) -> Settlement;
}

pub struct PostProcessingPipeline {
    settlement_contract: GPv2Settlement,
    unwrap_factor: f64,
    weth: WETH9,
    buffer_retriever: BufferRetriever,
    market_makable_token_list: AutoUpdatingTokenList,
}

impl PostProcessingPipeline {
    pub fn new(
        native_token: H160,
        web3: Web3,
        unwrap_factor: f64,
        settlement_contract: GPv2Settlement,
        market_makable_token_list: AutoUpdatingTokenList,
    ) -> Self {
        let weth = WETH9::at(&web3, native_token);
        let buffer_retriever = BufferRetriever::new(web3, settlement_contract.address());

        Self {
            settlement_contract,
            unwrap_factor,
            weth,
            buffer_retriever,
            market_makable_token_list,
        }
    }
}

#[async_trait::async_trait]
impl PostProcessing for PostProcessingPipeline {
    async fn optimize_settlement(
        &self,
        settlement: Settlement,
        solver_account: Account,
        gas_price: GasPrice1559,
        risk_calculator: Option<&RiskCalculator>,
    ) -> Settlement {
        let simulator = SettlementSimulator {
            settlement_contract: self.settlement_contract.clone(),
            gas_price,
            solver_account: solver_account.clone(),
            internalization: InternalizationStrategy::SkipInternalizableInteraction,
        };

        let optimized_solution = optimize_buffer_usage(
            settlement,
            self.market_makable_token_list.clone(),
            &simulator,
        )
        .await;

        // an error will leave the settlement unmodified
        let optimized_solution = optimize_unwrapping(
            optimized_solution,
            &simulator,
            &self.buffer_retriever,
            &self.weth,
            self.unwrap_factor,
        )
        .await;

        // although some solvers provided success probability, protocol will
        // override the success probability if it has risk parameters for the solver.
        // this is currently done for naive, baseline, gnosis solvers
        // TODO: once we eliminate naive and baseline this logic should be moved to
        // SingleOrderSettlement::into_settlement
        match (optimized_solution.score, risk_calculator) {
            (http_solver::model::Score::RiskAdjusted { gas_amount, .. }, Some(risk_calculator)) => {
                match compute_success_probability(
                    &optimized_solution,
                    &simulator,
                    risk_calculator,
                    gas_price,
                    &solver_account.address(),
                )
                .await
                {
                    Ok(success_probability) => Settlement {
                        score: http_solver::model::Score::RiskAdjusted {
                            success_probability,
                            gas_amount,
                        },
                        ..optimized_solution
                    },
                    Err(err) => {
                        tracing::warn!(?err, "failed to compute success probability");
                        optimized_solution
                    }
                }
            }
            _ => optimized_solution,
        }
    }
}
