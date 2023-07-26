use {
    crate::{
        boundary::Result,
        domain::{eth, liquidity},
    },
    solver::liquidity::StablePoolOrder,
};

pub fn to_domain(id: liquidity::Id, pool: StablePoolOrder) -> Result<liquidity::Liquidity> {
    todo!("Balancer V2 stable pool not yet implemented: {id:?} {pool:?}");
}

pub fn to_interaction(
    pool: &liquidity::balancer::v2::stable::Pool,
    input: &liquidity::MaxInput,
    output: &liquidity::ExactOutput,
    receiver: &eth::Address,
) -> eth::Interaction {
    todo!(
        "Balancer V2 stable pool not yet implemented: {pool:?} {input:?} {output:?} {receiver:?}"
    );
}
