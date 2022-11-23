pub mod optimize_buffer_usage;
pub mod optimize_unwrapping;

use crate::{
    settlement::Settlement, settlement_simulation::simulate_and_estimate_gas_at_current_block,
    solver::http_solver::buffers::BufferRetriever,
};
use contracts::{GPv2Settlement, WETH9};
use ethcontract::Account;
use gas_estimation::GasPrice1559;
use optimize_buffer_usage::optimize_buffer_usage;
use optimize_unwrapping::optimize_unwrapping;
use primitive_types::H160;
use shared::{
    ethrpc::Web3, http_solver::model::InternalizationStrategy, token_list::AutoUpdatingTokenList,
};

/// Determines whether a settlement would be executed successfully.
#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait SettlementSimulating: Send + Sync {
    async fn settlement_would_succeed(&self, settlement: Settlement) -> bool;
}

pub struct SettlementSimulator {
    settlement_contract: GPv2Settlement,
    gas_price: GasPrice1559,
    solver_account: Account,
    internalization: InternalizationStrategy,
}

#[async_trait::async_trait]
impl SettlementSimulating for SettlementSimulator {
    async fn settlement_would_succeed(&self, settlement: Settlement) -> bool {
        let settlement = settlement.into_encoded(self.internalization);
        let result = simulate_and_estimate_gas_at_current_block(
            std::iter::once((self.solver_account.clone(), settlement, None)),
            &self.settlement_contract,
            self.gas_price,
        )
        .await;
        matches!(result, Ok(results) if results[0].is_ok())
    }
}

#[async_trait::async_trait]
#[mockall::automock]
pub trait PostProcessing: Send + Sync + 'static {
    /// Tries to apply optimizations to a given settlement. If all optimizations fail the original
    /// settlement gets returned.
    async fn optimize_settlement(
        &self,
        settlement: Settlement,
        solver_account: Account,
        gas_price: GasPrice1559,
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
    ) -> Settlement {
        let simulator = SettlementSimulator {
            settlement_contract: self.settlement_contract.clone(),
            gas_price,
            solver_account,
            internalization: InternalizationStrategy::SkipInternalizableInteraction,
        };

        let optimized_solution = optimize_buffer_usage(
            settlement,
            self.market_makable_token_list.clone(),
            &simulator,
        )
        .await;

        // an error will leave the settlement unmodified
        optimize_unwrapping(
            optimized_solution,
            &simulator,
            &self.buffer_retriever,
            &self.weth,
            self.unwrap_factor,
        )
        .await
    }
}
