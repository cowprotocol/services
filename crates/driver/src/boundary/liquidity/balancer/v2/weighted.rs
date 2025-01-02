use {
    crate::{
        boundary::Result,
        domain::{
            eth,
            liquidity::{self, balancer},
        },
    },
    shared::sources::balancer_v2::pool_fetching::WeightedPoolVersion,
    solver::liquidity::{balancer_v2, WeightedProductOrder},
};

/// Median gas used per BalancerSwapGivenOutInteraction.
// estimated with https://dune.com/queries/639857
const GAS_PER_SWAP: u64 = 88_892;

pub fn to_domain(id: liquidity::Id, pool: WeightedProductOrder) -> Result<liquidity::Liquidity> {
    Ok(liquidity::Liquidity {
        id,
        gas: GAS_PER_SWAP.into(),
        kind: liquidity::Kind::BalancerV2Weighted(balancer::v2::weighted::Pool {
            vault: vault(&pool),
            id: pool_id(&pool),
            reserves: balancer::v2::weighted::Reserves::try_new(
                pool.reserves
                    .into_iter()
                    .map(|(token, reserve)| {
                        Ok(balancer::v2::weighted::Reserve {
                            asset: eth::Asset {
                                token: token.into(),
                                amount: reserve.common.balance.into(),
                            },
                            weight: balancer::v2::weighted::Weight::from_raw(
                                reserve.weight.as_uint256(),
                            ),
                            scale: balancer::v2::ScalingFactor::from_raw(
                                reserve.common.scaling_factor.as_uint256(),
                            )?,
                        })
                    })
                    .collect::<Result<_>>()?,
            )?,
            fee: balancer::v2::Fee::from_raw(pool.fee.as_uint256()),
            version: match pool.version {
                WeightedPoolVersion::V0 => balancer::v2::weighted::Version::V0,
                WeightedPoolVersion::V3Plus => balancer::v2::weighted::Version::V3Plus,
            },
        }),
    })
}

fn vault(pool: &WeightedProductOrder) -> eth::ContractAddress {
    pool.settlement_handling
        .as_any()
        .downcast_ref::<balancer_v2::SettlementHandler>()
        .expect("downcast balancer settlement handler")
        .vault()
        .address()
        .into()
}

fn pool_id(pool: &WeightedProductOrder) -> balancer::v2::Id {
    pool.settlement_handling
        .as_any()
        .downcast_ref::<balancer_v2::SettlementHandler>()
        .expect("downcast balancer settlement handler")
        .pool_id()
        .into()
}

pub fn to_interaction(
    pool: &liquidity::balancer::v2::weighted::Pool,
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
