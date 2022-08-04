use crate::{
    driver::solver_settlements::RatedSettlement,
    settlement::{external_prices::ExternalPrices, Settlement},
    settlement_access_list::AccessListEstimating,
    settlement_simulation::{settle_method, simulate_and_estimate_gas_at_current_block},
    solver::{SettlementWithError, SettlementWithSolver, Solver},
};
use anyhow::{Context, Result};
use contracts::GPv2Settlement;
use gas_estimation::GasPrice1559;
use itertools::{Either, Itertools};
use num::BigRational;
use shared::Web3;
use std::sync::Arc;
use web3::types::AccessList;

type SolverSettlement = (Arc<dyn Solver>, Settlement);
type RatedSolverSettlement = (Arc<dyn Solver>, RatedSettlement, Option<AccessList>);

#[cfg_attr(any(feature = "mockall", test), mockall::automock)]
#[async_trait::async_trait]
pub trait SettlementRating: Send + Sync {
    async fn rate_settlements(
        &self,
        settlements: Vec<SolverSettlement>,
        prices: &ExternalPrices,
        gas_price: GasPrice1559,
    ) -> Result<(Vec<RatedSolverSettlement>, Vec<SettlementWithError>)>;
}

pub struct SettlementRater {
    pub access_list_estimator: Arc<dyn AccessListEstimating>,
    pub settlement_contract: GPv2Settlement,
    pub web3: Web3,
}

impl SettlementRater {
    async fn append_access_lists(
        &self,
        solver_settlements: Vec<(Arc<dyn Solver>, Settlement)>,
        gas_price: GasPrice1559,
    ) -> Vec<SettlementWithSolver> {
        let txs = solver_settlements
            .iter()
            .map(|(solver, settlement)| {
                settle_method(
                    gas_price,
                    &self.settlement_contract,
                    settlement.clone(),
                    solver.account().clone(),
                )
                .tx
            })
            .collect::<Vec<_>>();

        let mut access_lists = self
            .access_list_estimator
            .estimate_access_lists(&txs)
            .await
            .unwrap_or_default()
            .into_iter();

        solver_settlements
            .into_iter()
            .map(|(solver, settlement)| {
                let access_list = access_lists.next().and_then(|access_list| access_list.ok());
                (solver, settlement, access_list)
            })
            .collect()
    }
}

#[async_trait::async_trait]
impl SettlementRating for SettlementRater {
    /// Rate settlements, ignoring those for which the rating procedure failed.
    async fn rate_settlements(
        &self,
        settlements: Vec<SolverSettlement>,
        prices: &ExternalPrices,
        gas_price: GasPrice1559,
    ) -> Result<(Vec<RatedSolverSettlement>, Vec<SettlementWithError>)> {
        let settlements = self.append_access_lists(settlements, gas_price).await;

        let simulations = simulate_and_estimate_gas_at_current_block(
            settlements.iter().map(|settlement| {
                (
                    settlement.0.account().clone(),
                    settlement.1.clone(),
                    settlement.2.clone(),
                )
            }),
            &self.settlement_contract,
            &self.web3,
            gas_price,
        )
        .await
        .context("failed to simulate settlements")?;

        let gas_price =
            BigRational::from_float(gas_price.effective_gas_price()).expect("Invalid gas price.");

        let rate_settlement = |id, settlement: Settlement, gas_estimate| {
            let surplus = settlement.total_surplus(prices);
            let scaled_solver_fees = settlement.total_scaled_unsubsidized_fees(prices);
            let unscaled_subsidized_fee = settlement.total_unscaled_subsidized_fees(prices);
            RatedSettlement {
                id,
                settlement,
                surplus,
                unscaled_subsidized_fee,
                scaled_unsubsidized_fee: scaled_solver_fees,
                gas_estimate,
                gas_price: gas_price.clone(),
            }
        };

        Ok(
            (settlements.into_iter().zip(simulations).enumerate()).partition_map(
                |(i, ((solver, settlement, access_list), result))| match result {
                    Ok(gas_estimate) => Either::Left((
                        solver.clone(),
                        rate_settlement(i, settlement, gas_estimate),
                        access_list,
                    )),
                    Err(err) => Either::Right((solver, settlement, access_list, err)),
                },
            ),
        )
    }
}
