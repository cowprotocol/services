use {
    crate::{
        boundary::Result,
        domain::{
            eth,
            liquidity::{self, balancer},
        },
    },
    contracts::{BalancerV2Vault, GPv2Settlement},
    shared::{http_solver::model::TokenAmount, price_estimation},
    solver::{
        interactions::allowances::Allowances,
        liquidity::{balancer_v2, WeightedProductOrder},
    },
    std::sync::Arc,
};

pub fn to_domain(id: liquidity::Id, pool: WeightedProductOrder) -> Result<liquidity::Liquidity> {
    Ok(liquidity::Liquidity {
        id,
        gas: price_estimation::gas::GAS_PER_BALANCER_SWAP.into(),
        kind: liquidity::Kind::BalancerV2Weighted(balancer::v2::weighted::Pool {
            vault: vault(&pool),
            id: pool_id(&pool),
            reserves: balancer::v2::weighted::Reserves::new(
                pool.reserves
                    .into_iter()
                    .map(|(token, reserve)| {
                        Ok(balancer::v2::weighted::Reserve {
                            asset: eth::Asset {
                                token: token.into(),
                                amount: reserve.common.balance.into(),
                            },
                            weight: reserve.weight.as_uint256().into(),
                            scale: balancer::v2::ScalingFactor::from_exponent(
                                reserve.common.scaling_exponent,
                            )?,
                        })
                    })
                    .collect::<Result<_>>()?,
            )?,
            fee: pool.fee.as_uint256().into(),
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
    let web3 = shared::ethrpc::dummy::web3();
    let handler = balancer_v2::SettlementHandler::new(
        pool.id.into(),
        // Note that this code assumes `receiver == sender`. This assumption is
        // also baked into the Balancer V2 logic in the `shared` crate, so to
        // change this assumption, we would need to change it there as well.
        GPv2Settlement::at(&web3, receiver.0),
        BalancerV2Vault::at(&web3, pool.vault.into()),
        Arc::new(Allowances::empty(receiver.0)),
    );

    let interaction = handler.swap(
        TokenAmount::new(input.0.token.into(), input.0.amount),
        TokenAmount::new(output.0.token.into(), output.0.amount),
    );

    let (target, value, call_data) = interaction.encode_swap();

    eth::Interaction {
        target: target.into(),
        value: value.into(),
        call_data: call_data.0.into(),
    }
}
