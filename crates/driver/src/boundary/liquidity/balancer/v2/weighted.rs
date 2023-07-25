use {
    crate::{
        boundary::Result,
        domain::{eth, liquidity},
    },
    solver::liquidity::WeightedProductOrder,
};

pub fn to_domain(id: liquidity::Id, pool: WeightedProductOrder) -> Result<liquidity::Liquidity> {
    todo!("Balancer V2 weighted pool not yet implemented: {id:?} {pool:?}");
}

pub fn to_interaction(
    pool: &liquidity::balancer::v2::weighted::Pool,
    input: &liquidity::MaxInput,
    output: &liquidity::ExactOutput,
    receiver: &eth::Address,
) -> eth::Interaction {
    todo!(
        "Balancer V2 weighted pool not yet implemented: {pool:?} {input:?} {output:?} {receiver:?}"
    );
}
