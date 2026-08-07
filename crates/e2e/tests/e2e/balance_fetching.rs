use {
    ::alloy::{
        primitives::{Address, U256},
        providers::Provider,
        sol_types::SolCall,
    },
    account_balances::{BalanceSimulator, Query},
    balance_overrides::DummyStateOverrider,
    contracts::{ERC20, Multicall3},
    e2e::setup::*,
    ethrpc::{Web3, alloy::CallBuilderExt},
    model::{interaction::InteractionData, order::SellTokenSource},
    number::units::EthUnit,
    std::sync::Arc,
};

/// Comfortably above the batch size the fetcher chunks queries into, so that
/// more than one `Multicall3` call is needed.
const QUERIES_PAST_ONE_BATCH: usize = 25;

#[tokio::test]
#[ignore]
async fn local_node_batched_balances_match_individual_reads() {
    run_test(batched_balances_match_individual_reads).await;
}

/// Reading balances through `Multicall3` must produce exactly what reading each
/// `balanceOf`/`allowance` pair on its own does, including for tokens whose
/// reads fail.
async fn batched_balances_match_individual_reads(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;
    let [token_a, token_b] = onchain.deploy_tokens::<2>(trader.address()).await;

    let vault_relayer = onchain.contracts().allowance;

    // Leave the balance above the allowance on one token and below it on the
    // other, so a result that silently reported only one of the two would not
    // match.
    token_a.mint(trader.address(), 100u64.eth()).await;
    token_a
        .approve(vault_relayer, 40u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    token_b.mint(trader.address(), 30u64.eth()).await;
    token_b
        .approve(vault_relayer, 1_000u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    let query = |owner, token| Query {
        owner,
        token,
        source: SellTokenSource::Erc20,
        interactions: vec![],
        balance_override: None,
    };

    // An address without any code, so the reads for it have to fail without
    // taking the rest of the batch down with them.
    let not_a_token = Address::repeat_byte(0x42);

    // A pre-interaction sends the query down the simulated path instead of the
    // batched one. Reading a balance leaves the outcome unchanged, so the answer
    // still has to be the plain tradable balance.
    let behind_pre_interaction = Query {
        interactions: vec![InteractionData {
            target: *token_a.address(),
            value: U256::ZERO,
            call_data: ERC20::ERC20::balanceOfCall {
                account: trader.address(),
            }
            .abi_encode(),
        }],
        ..query(trader.address(), *token_b.address())
    };
    // Deprecated sources are rejected without ever reaching the node.
    let unsupported_source = Query {
        source: SellTokenSource::External,
        ..query(trader.address(), *token_a.address())
    };

    // The three kinds of query are interleaved on purpose: they are answered by
    // separate code paths and stitched back together afterwards, so a result
    // landing on the wrong query only shows up when they are mixed.
    let mut queries = vec![
        query(trader.address(), *token_a.address()),
        unsupported_source,
        query(trader.address(), *token_b.address()),
        behind_pre_interaction,
        query(trader.address(), not_a_token),
    ];
    let interleaved = queries.len();
    // Owners that never held anything still read fine, and get us past a single
    // batch so that chunking is exercised.
    for i in 0..QUERIES_PAST_ONE_BATCH {
        queries.push(query(Address::repeat_byte(i as u8), *token_a.address()));
    }

    // Without this the fetcher would fall back to reading balances one by one
    // and the test would pass without ever building a `Multicall3` call.
    assert_multicall3_deployed(&web3, onchain.contracts().chain_id).await;

    let balance_fetcher = account_balances::fetcher(
        &web3,
        BalanceSimulator::new(
            onchain.contracts().gp_settlement.clone(),
            onchain.contracts().balances.clone(),
            vault_relayer,
            Arc::new(DummyStateOverrider),
        ),
        onchain.contracts().chain_id,
    );

    let batched = balance_fetcher.get_balances(&queries).await;
    assert_eq!(batched.len(), queries.len());

    // Every query that is a plain read must agree with reading it on its own.
    for (index, (query, batched)) in queries.iter().zip(&batched).enumerate() {
        if query.source != SellTokenSource::Erc20 {
            continue;
        }
        let expected = tradable_balance(&web3, query, vault_relayer).await;
        match (batched, &expected) {
            (Ok(batched), Ok(expected)) => assert_eq!(batched, expected, "query {index}"),
            (Err(_), Err(_)) => (),
            _ => panic!("query {index} disagrees: {batched:?} vs {expected:?}"),
        }
    }

    // Pin the values down too, so that a fetcher answering the wrong query, or
    // returning zero everywhere, could not satisfy the comparison above.
    assert_eq!(
        *batched[0].as_ref().unwrap(),
        40u64.eth(),
        "allowance bound"
    );
    assert!(batched[1].is_err(), "deprecated sell token source");
    assert_eq!(*batched[2].as_ref().unwrap(), 30u64.eth(), "balance bound");
    assert_eq!(
        *batched[3].as_ref().unwrap(),
        30u64.eth(),
        "simulated behind a pre-interaction"
    );
    assert!(batched[4].is_err(), "reads against a non-token must fail");
    for (index, balance) in batched.iter().enumerate().skip(interleaved) {
        assert_eq!(
            *balance.as_ref().unwrap(),
            U256::ZERO,
            "filler query {index}"
        );
    }
}

/// Reads one tradable balance directly, as the oracle to compare the batched
/// results against.
async fn tradable_balance(
    web3: &Web3,
    query: &Query,
    vault_relayer: Address,
) -> anyhow::Result<U256> {
    let token = ERC20::Instance::new(query.token, web3.provider.clone());
    let balance = token.balanceOf(query.owner).call().await?;
    let allowance = token.allowance(query.owner, vault_relayer).call().await?;
    Ok(std::cmp::min(balance, allowance))
}

async fn assert_multicall3_deployed(web3: &Web3, chain_id: u64) {
    let address = Multicall3::deployment_address(&chain_id)
        .unwrap_or_else(|| panic!("no Multicall3 deployment registered for chain {chain_id}"));
    assert!(
        !web3.provider.get_code_at(address).await.unwrap().is_empty(),
        "Multicall3 missing at {address:?}, balances would not be batched"
    );
}
