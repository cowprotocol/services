//! This test verifies that given a swap with multiple orders, including limit
//! orders, the settlement building does not cause the naive solver to generate
//! a solution that swaps more than the pools reserves.
//!
//! This test verifies a regression that was introduced with limit orders, where
//! the incorrect order amounts were used for computing the final pool swap
//! amounts, causing it buy more from the pool than it actual had.

use {crate::tests, serde_json::json};

#[tokio::test]
async fn test() {
    let engine = tests::SolverEngine::new("naive", tests::Config::None).await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {},
            "orders": [
                {
                    "uid": "0x0101010101010101010101010101010101010101010101010101010101010101\
                              0101010101010101010101010101010101010101\
                              01010101",
                    "sellToken": "0xD533a949740bb3306d119CC777fa900bA034cd52",
                    "buyToken": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
                    "sellAmount": "2161740107040163317224",
                    "fullSellAmount": "2161740107040163317224",
                    "buyAmount": "2146544862",
                    "fullBuyAmount": "2146544862",
                    "feePolicies": [],
                    "validTo": 0,
                    "kind": "sell",
                    "owner": "0x5b1e2c2762667331bc91648052f646d1b0d35984",
                    "partiallyFillable": false,
                    "preInteractions": [],
                    "postInteractions": [],
                    "sellTokenSource": "erc20",
                    "buyTokenDestination": "erc20",
                    "class": "liquidity",
                    "appData": "0x6000000000000000000000000000000000000000000000000000000000000007",
                    "signingScheme": "preSign",
                    "signature": "0x",
                },
                {
                    "uid": "0x0202020202020202020202020202020202020202020202020202020202020202\
                              0202020202020202020202020202020202020202\
                              02020202",
                    "sellToken": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
                    "buyToken": "0xD533a949740bb3306d119CC777fa900bA034cd52",
                    "sellAmount": "495165988",
                    "fullSellAmount": "495165988",
                    "buyAmount": "1428571428571428571428",
                    "fullBuyAmount": "1428571428571428571428",
                    "feePolicies": [],
                    "validTo": 0,
                    "kind": "sell",
                    "owner": "0x5b1e2c2762667331bc91648052f646d1b0d35984",
                    "partiallyFillable": false,
                    "preInteractions": [],
                    "postInteractions": [],
                    "sellTokenSource": "erc20",
                    "buyTokenDestination": "erc20",
                    "class": "limit",
                    "appData": "0x6000000000000000000000000000000000000000000000000000000000000007",
                    "signingScheme": "preSign",
                    "signature": "0x",
                },
            ],
            "liquidity": [
                {
                    "kind": "constantProduct",
                    "tokens": {
                        "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48": {
                            "balance": "32275540"
                        },
                        "0xD533a949740bb3306d119CC777fa900bA034cd52": {
                            "balance": "33308141034569852391"
                        }
                    },
                    "fee": "0.003",
                    "id": "0",
                    "address": "0x210a97ba874a8e279c95b350ae8ba143a143c159",
                    "router": "0x7a250d5630b4cf539739df2c5dacb4c659f2488d",
                    "gasEstimate": "110000"
                },
            ],
            "effectiveGasPrice": "15000000000",
            "deadline": "2106-01-01T00:00:00.000Z",
        }))
        .await;

    assert_eq!(
        solution,
        json!({
            "solutions": []
        }),
    );
}
