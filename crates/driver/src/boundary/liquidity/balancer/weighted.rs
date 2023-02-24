use {
    crate::{
        boundary,
        domain::{
            eth,
            liquidity::{
                self,
                balancer::weighted::{Bfp, Fee, Id, Pool, TokenState, Weight},
            },
        },
        infra::{self, blockchain::Ethereum},
    },
    anyhow::Result,
    contracts::{BalancerV2Vault, GPv2Settlement},
    shared::{
        current_block::{BlockRetrieving, CurrentBlockStream},
        http_solver::model::TokenAmount,
        interaction::Interaction,
        price_estimation::gas::GAS_PER_BALANCER_SWAP,
        sources::balancer_v2::{
            pool_fetching::{BalancerContracts, BalancerFactoryKind},
            BalancerPoolFetcher,
        },
        token_info::TokenInfoFetching,
    },
    solver::{
        interactions::allowances::Allowances,
        liquidity::{
            balancer_v2::{BalancerV2Liquidity, SettlementHandler},
            WeightedProductOrder,
        },
        liquidity_collector::LiquidityCollecting,
    },
    std::sync::Arc,
};

pub fn to_domain(id: liquidity::Id, pool: WeightedProductOrder) -> liquidity::Liquidity {
    let handler = pool
        .settlement_handling
        .as_any()
        .downcast_ref::<SettlementHandler>()
        .expect("downcast uniswap settlement handler");

    liquidity::Liquidity {
        id,
        // TODO: Not sure if this value is correct here. Also there is only 1 value for balancer
        // but 2 different kinds of supported pools.
        gas: eth::Gas(GAS_PER_BALANCER_SWAP.into()),
        kind: liquidity::Kind::BalancerV2Weighted(Pool {
            id: Id(handler.pool_id),
            swap_fee: Fee(Bfp(pool.fee.0)),
            vault: handler.vault.address().into(),
            tokens: pool
                .reserves
                .iter()
                .map(|(token, state)| TokenState {
                    weight: Weight(Bfp(state.weight.0)),
                    asset: eth::Asset {
                        token: eth::TokenAddress(eth::ContractAddress(*token)),
                        amount: state.common.balance,
                    },
                    scaling_exponent: state.common.scaling_exponent,
                })
                .collect(),
        }),
    }
}

pub fn to_interaction(
    pool: &liquidity::balancer::weighted::Pool,
    input: &liquidity::MaxInput,
    output: &liquidity::ExactOutput,
    receiver: &eth::Address,
) -> eth::Interaction {
    let web3 = shared::ethrpc::dummy::web3();

    let handler = SettlementHandler::new(
        pool.id.0,
        GPv2Settlement::at(&web3, receiver.0),
        BalancerV2Vault::at(&web3, pool.vault.0),
        Arc::new(Allowances::empty(receiver.0)),
    );

    let (_, interaction) = handler.encode(
        TokenAmount::new(input.0.token.into(), input.0.amount),
        TokenAmount::new(output.0.token.into(), output.0.amount),
    );

    interaction
        .encode()
        .into_iter()
        .map(|(target, value, call_data)| eth::Interaction {
            target: eth::Address(target),
            value: eth::Ether(value),
            call_data: call_data.0,
        })
        .next()
        .expect("returns exactly 1 interaction")
}

pub async fn collector(
    eth: &Ethereum,
    block_retriever: Arc<dyn BlockRetrieving>,
    block_stream: CurrentBlockStream,
    token_info_fetcher: Arc<dyn TokenInfoFetching>,
    config: &infra::liquidity::config::BalancerWeighted,
) -> Result<Box<dyn LiquidityCollecting>> {
    let web3 = boundary::web3(eth);

    let balancer_contracts = BalancerContracts::new(
        &web3,
        vec![
            BalancerFactoryKind::Weighted,
            BalancerFactoryKind::Weighted2Token,
            BalancerFactoryKind::LiquidityBootstrapping,
        ],
    )
    .await?;

    let pool_fetcher = Arc::new(
        BalancerPoolFetcher::new(
            eth.chain_id().0.as_u64(),
            block_retriever,
            token_info_fetcher,
            boundary::liquidity::cache_config(),
            block_stream,
            Default::default(),
            web3.clone(),
            &balancer_contracts,
            config.deny_listed_pools.clone(),
        )
        .await
        .unwrap(),
    );

    let vault = BalancerV2Vault::at(&web3, config.vault.into());
    let collector = BalancerV2Liquidity::new(
        web3,
        pool_fetcher,
        eth.contracts().settlement().clone(),
        vault,
    );

    Ok(Box::new(collector))
}
