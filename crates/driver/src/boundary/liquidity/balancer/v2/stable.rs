use {
    crate::{
        boundary::Result,
        domain::{
            eth,
            liquidity::{self, balancer},
        },
    },
    ethrpc::alloy::conversions::{IntoAlloy, IntoLegacy},
    solver::liquidity::{StablePoolOrder, balancer_v2},
};

/// Median gas used per BalancerSwapGivenOutInteraction.
// estimated with https://dune.com/queries/639857
const GAS_PER_SWAP: u64 = 88_892;

pub fn to_domain(id: liquidity::Id, pool: StablePoolOrder) -> Result<liquidity::Liquidity> {
    Ok(liquidity::Liquidity {
        id,
        gas: GAS_PER_SWAP.into(),
        kind: liquidity::Kind::BalancerV2Stable(balancer::v2::stable::Pool {
            vault: vault(&pool),
            id: pool_id(&pool),
            reserves: balancer::v2::stable::Reserves::try_new(
                pool.reserves
                    .into_iter()
                    .map(|(token, reserve)| {
                        Ok(balancer::v2::stable::Reserve {
                            asset: eth::Asset {
                                token: token.into(),
                                amount: reserve.balance.into_alloy().into(),
                            },
                            scale: balancer::v2::ScalingFactor::from_raw(
                                reserve.scaling_factor.as_uint256().into_alloy(),
                            )?,
                        })
                    })
                    .collect::<Result<_>>()?,
            )?,
            amplification_parameter: balancer::v2::stable::AmplificationParameter::new(
                pool.amplification_parameter.factor().into_alloy(),
                pool.amplification_parameter.precision().into_alloy(),
            )?,
            fee: balancer::v2::Fee::from_raw(pool.fee.as_uint256().into_alloy()),
        }),
    })
}

fn vault(pool: &StablePoolOrder) -> eth::ContractAddress {
    (*pool
        .settlement_handling
        .as_any()
        .downcast_ref::<balancer_v2::SettlementHandler>()
        .expect("downcast balancer settlement handler")
        .vault())
    .into()
}

fn pool_id(pool: &StablePoolOrder) -> balancer::v2::Id {
    pool.settlement_handling
        .as_any()
        .downcast_ref::<balancer_v2::SettlementHandler>()
        .expect("downcast balancer settlement handler")
        .pool_id()
        .into_legacy()
        .into()
}

pub fn to_interaction(
    pool: &liquidity::balancer::v2::stable::Pool,
    input: &liquidity::MaxInput,
    output: &liquidity::ExactOutput,
    receiver: &eth::Address,
) -> eth::Interaction {
    super::to_interaction(
        &super::Pool {
            vault: pool.vault,
            id: pool.id,
        },
        input,
        output,
        receiver,
    )
}
