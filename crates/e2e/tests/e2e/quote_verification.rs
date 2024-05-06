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
async fn forked_node_mainnet_verify_zeroex_quote() {
    run_forked_test_with_block_number(
        forked_mainnet_verify_zeroex_quote,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        FORK_BLOCK_MAINNET,
    )
    .await;
}

/// The block number from which we will fetch state for the forked tests.
const FORK_BLOCK_MAINNET: u64 = 19796077;

/// Tests that quotes based on zeroex RFQ orders that require `tx.origin` to be
/// 0x0000 get verified. Based on an RFQ quote we saw on prod:
/// https://www.tdly.co/shared/simulation/7402de5e-e524-4e24-9af8-50d0a38c105b
async fn forked_mainnet_verify_zeroex_quote(web3: Web3) {
    let block_stream = ethrpc::current_block::current_block_stream(
        Arc::new(web3.clone()),
        std::time::Duration::from_millis(1_000),
    )
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

    let verify_trade = |signature| {
        let verifier = verifier.clone();
        async move {
            let signature_hex = hex::decode(signature).unwrap();
            let arguments_hex = hex::decode("000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000002260fac5e5542a773aa44fbcfedf7c193bc2c599000000000000000000000000000000000000000000000000e357b42c3a9d8ccf0000000000000000000000000000000000000000000000000000000004d0e79e000000000000000000000000a69babef1ca67a37ffaf7a485dfff3382056e78c0000000000000000000000009008d19f58aabd9ed0d60971565aa8510560ab41000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000066360af101ffffffffffffffffffffffffffffffffffffff0f3f47f166360a8d0000003f0000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000001c66b3383f287dd9c85ad90e7c5a576ea4ba1bdf5a001d794a9afa379e6b2517b47e487a1aef32e75af432cbdbd301ada42754eaeac21ec4ca744afd92732f47540000000000000000000000000000000000000000000000000000000004d0c80f").unwrap();
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
                            data: signature_hex.into_iter().chain(arguments_hex).collect(),
                            value: 0.into(),
                        }],
                        solver: H160::from_str("0xe3067c7c27c1038de4e8ad95a83b927d23dfbd99")
                            .unwrap(),
                        tx_origin: Some(H160::zero()),
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

    // trades using `fillRfqOrder()` get verified even if the simulation fails
    // See <https://www.4byte.directory/signatures/?bytes4_signature=0xaa77476c>
    let verification = verify_trade("aa77476c").await;
    assert_eq!(&verification.unwrap(), &verified_quote);

    // trades using `fillOrKillRfqOrder()` get verified even if the simulation fails
    // See <https://www.4byte.directory/signatures/?bytes4_signature=0x438cdfc5>
    let verification = verify_trade("438cdfc5").await;
    assert_eq!(&verification.unwrap(), &verified_quote);

    // trades using any other functions do not get verified when failing to simulate
    let verification = verify_trade("11111111").await;
    assert_eq!(
        verification.unwrap(),
        Estimate {
            verified: false,
            ..verified_quote
        }
    );
}
