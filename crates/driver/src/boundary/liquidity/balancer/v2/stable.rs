use {
    crate::{
        boundary::Result,
        domain::{
            eth,
            liquidity::{self, balancer},
        },
    },
    shared::price_estimation,
    solver::liquidity::{balancer_v2, StablePoolOrder},
};

pub fn to_domain(id: liquidity::Id, pool: StablePoolOrder) -> Result<liquidity::Liquidity> {
    Ok(liquidity::Liquidity {
        id,
        gas: price_estimation::gas::GAS_PER_BALANCER_SWAP.into(),
        kind: liquidity::Kind::BalancerV2Stable(balancer::v2::stable::Pool {
            vault: vault(&pool),
            id: pool_id(&pool),
            reserves: balancer::v2::stable::Reserves::new(
                pool.reserves
                    .into_iter()
                    .map(|(token, reserve)| {
                        Ok(balancer::v2::stable::Reserve {
                            asset: eth::Asset {
                                token: token.into(),
                                amount: reserve.balance.into(),
                            },
                            scale: balancer::v2::ScalingFactor::from_exponent(
                                reserve.scaling_exponent,
                            )?,
                        })
                    })
                    .collect::<Result<_>>()?,
            )?,
            amplification_parameter: balancer::v2::stable::AmplificationParameter::new(
                pool.amplification_parameter.factor(),
                pool.amplification_parameter.precision(),
            )?,
            fee: pool.fee.as_uint256().into(),
        }),
    })
}

fn vault(pool: &StablePoolOrder) -> eth::ContractAddress {
    pool.settlement_handling
        .as_any()
        .downcast_ref::<balancer_v2::SettlementHandler>()
        .expect("downcast balancer settlement handler")
        .vault()
        .address()
        .into()
}

fn pool_id(pool: &StablePoolOrder) -> balancer::v2::Id {
    pool.settlement_handling
        .as_any()
        .downcast_ref::<balancer_v2::SettlementHandler>()
        .expect("downcast balancer settlement handler")
        .pool_id()
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
