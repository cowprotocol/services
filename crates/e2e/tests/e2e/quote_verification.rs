use {
    e2e::setup::{run_forked_test_with_block_number, OnchainComponents},
    ethcontract::H160,
    ethrpc::Web3,
    model::order::{BuyTokenDestination, OrderKind, SellTokenSource},
    number::nonzero::U256 as NonZeroU256,
    shared::{
        price_estimation::{
            trade_verifier::{PriceQuery, TradeVerifier, TradeVerifying},
            Estimate,
            Verification,
        },
        trade_finding::{Interaction, Trade},
    },
    std::{str::FromStr, sync::Arc},
};

#[tokio::test]
#[ignore]
async fn forked_node_bypass_verification_for_rfq_quotes() {
    run_forked_test_with_block_number(
        test_bypass_verification_for_rfq_quotes,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        FORK_BLOCK_MAINNET,
    )
    .await;
}

/// The block number from which we will fetch state for the forked tests.
const FORK_BLOCK_MAINNET: u64 = 19796077;

/// Tests that quotes requesting `tx_origin: 0x0000` bypass the verification
/// because those are currently used by some solvers to provide market maker
/// integrations. Based on an RFQ quote we saw on prod:
/// https://www.tdly.co/shared/simulation/7402de5e-e524-4e24-9af8-50d0a38c105b
async fn test_bypass_verification_for_rfq_quotes(web3: Web3) {
    let url = std::env::var("FORK_URL_MAINNET")
        .expect("FORK_URL_MAINNET must be set to run forked tests")
        .parse()
        .unwrap();
    let block_stream = ethrpc::current_block::current_block_stream(url)
        .await
        .unwrap();
    let onchain = OnchainComponents::deployed(web3.clone()).await;

    let verifier = TradeVerifier::new(
        web3.clone(),
        Arc::new(web3.clone()),
        Arc::new(web3.clone()),
        block_stream,
        onchain.contracts().gp_settlement.address(),
        onchain.contracts().weth.address(),
        0.0,
    );

    let verify_trade = |tx_origin| {
        let verifier = verifier.clone();
        async move {
            verifier
                .verify(
                    &PriceQuery {
                        sell_token: H160::from_str("0x2260fac5e5542a773aa44fbcfedf7c193bc2c599")
                            .unwrap(),
                        buy_token: H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
                            .unwrap(),
                        kind: OrderKind::Sell,
                        in_amount: NonZeroU256::new(12.into()).unwrap(),
                    },
                    &Verification {
                        from: H160::from_str("0x73688c2b34bf6c09c125fed02fe92d17a94b897a").unwrap(),
                        receiver: H160::from_str("0x73688c2b34bf6c09c125fed02fe92d17a94b897a")
                            .unwrap(),
                        pre_interactions: vec![],
                        post_interactions: vec![],
                        sell_token_source: SellTokenSource::Erc20,
                        buy_token_destination: BuyTokenDestination::Erc20,
                    },
                    Trade {
                        out_amount: 16380122291179526144u128.into(),
                        gas_estimate: Some(225000),
                        interactions: vec![Interaction {
                            target: H160::from_str("0xdef1c0ded9bec7f1a1670819833240f027b25eff")
                                .unwrap(),
                            data: hex::decode("aa77476c000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000002260fac5e5542a773aa44fbcfedf7c193bc2c599000000000000000000000000000000000000000000000000e357b42c3a9d8ccf0000000000000000000000000000000000000000000000000000000004d0e79e000000000000000000000000a69babef1ca67a37ffaf7a485dfff3382056e78c0000000000000000000000009008d19f58aabd9ed0d60971565aa8510560ab41000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000066360af101ffffffffffffffffffffffffffffffffffffff0f3f47f166360a8d0000003f0000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000001c66b3383f287dd9c85ad90e7c5a576ea4ba1bdf5a001d794a9afa379e6b2517b47e487a1aef32e75af432cbdbd301ada42754eaeac21ec4ca744afd92732f47540000000000000000000000000000000000000000000000000000000004d0c80f").unwrap(),
                            value: 0.into(),
                        }],
                        solver: H160::from_str("0xe3067c7c27c1038de4e8ad95a83b927d23dfbd99")
                            .unwrap(),
                        tx_origin,
                    },
                )
                .await
        }
    };

    let verified_quote = Estimate {
        out_amount: 16380122291179526144u128.into(),
        gas: 225000,
        solver: H160::from_str("0xe3067c7c27c1038de4e8ad95a83b927d23dfbd99").unwrap(),
        verified: true,
    };

    // `tx_origin: 0x0000` is currently used to bypass quote verification due to an
    // implementation detail of zeroex RFQ orders.
    // TODO: remove with #2693
    let verification = verify_trade(Some(H160::zero())).await;
    assert_eq!(&verification.unwrap(), &verified_quote);

    // Trades using any other `tx_origin` can not bypass the verification.
    let verification = verify_trade(None).await;
    assert_eq!(
        verification.unwrap(),
        Estimate {
            verified: false,
            ..verified_quote
        }
    );
}
