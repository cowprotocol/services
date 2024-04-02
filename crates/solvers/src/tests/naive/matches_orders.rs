//! Tests for various permutiations of matching combinatinos of sell and buy
//! orders.

use {crate::tests, serde_json::json};

#[tokio::test]
async fn sell_orders_on_both_sides() {
    let engine = tests::SolverEngine::new(
        "naive",
        tests::Config::String(r#"risk-parameters = [0,0,0,0]"#.to_owned()),
    )
    .await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {},
            "orders": [
                {
                    "uid": "0x0101010101010101010101010101010101010101010101010101010101010101\
                              0101010101010101010101010101010101010101\
                              01010101",
                    "sellToken": "0x000000000000000000000000000000000000000a",
                    "buyToken": "0x000000000000000000000000000000000000000b",
                    "sellAmount": "40000000000000000000",
                    "buyAmount": "30000000000000000000",
                    "feeAmount": "0",
                    "kind": "sell",
                    "partiallyFillable": false,
                    "class": "market",
                    "feePolicies": [],
                },
                {
                    "uid": "0x0202020202020202020202020202020202020202020202020202020202020202\
                              0202020202020202020202020202020202020202\
                              02020202",
                    "sellToken": "0x000000000000000000000000000000000000000b",
                    "buyToken": "0x000000000000000000000000000000000000000a",
                    "sellAmount": "100000000000000000000",
                    "buyAmount": "90000000000000000000",
                    "feeAmount": "0",
                    "kind": "sell",
                    "partiallyFillable": false,
                    "class": "market",
                    "feePolicies": [],
                },
            ],
            "liquidity": [
                {
                    "kind": "constantProduct",
                    "tokens": {
                        "0x000000000000000000000000000000000000000a": {
                            "balance": "1000000000000000000000"
                        },
                        "0x000000000000000000000000000000000000000b": {
                            "balance": "1000000000000000000000"
                        }
                    },
                    "fee": "0.003",
                    "id": "0",
                    "address": "0xffffffffffffffffffffffffffffffffffffffff",
                    "router": "0xffffffffffffffffffffffffffffffffffffffff",
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
            "solutions": [{
                "id": 0,
                "prices": {
                    "0x000000000000000000000000000000000000000a": "57576575881490625723",
                    "0x000000000000000000000000000000000000000b": "54287532963535509684",
                },
                "trades": [
                    {
                        "kind": "fulfillment",
                        "order": "0x0101010101010101010101010101010101010101010101010101010101010101\
                                    0101010101010101010101010101010101010101\
                                    01010101",
                        "executedAmount": "40000000000000000000",
                    },
                    {
                        "kind": "fulfillment",
                        "order": "0x0202020202020202020202020202020202020202020202020202020202020202\
                                    0202020202020202020202020202020202020202\
                                    02020202",
                        "executedAmount": "100000000000000000000",
                    },
                ],
                "interactions": [
                    {
                        "kind": "liquidity",
                        "internalize": false,
                        "id": "0",
                        "inputToken": "0x000000000000000000000000000000000000000b",
                        "outputToken": "0x000000000000000000000000000000000000000a",
                        "inputAmount": "57576575881490625723",
                        "outputAmount": "54287532963535509685"
                    },
                ],
                "gas": 259417,
            }]
        }),
    );
}

#[tokio::test]
async fn sell_orders_on_one_side() {
    let engine = tests::SolverEngine::new(
        "naive",
        tests::Config::String(r#"risk-parameters = [0,0,0,0]"#.to_owned()),
    )
    .await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {},
            "orders": [
                {
                    "uid": "0x0101010101010101010101010101010101010101010101010101010101010101\
                              0101010101010101010101010101010101010101\
                              01010101",
                    "sellToken": "0x000000000000000000000000000000000000000a",
                    "buyToken": "0x000000000000000000000000000000000000000b",
                    "sellAmount": "40000000000000000000",
                    "buyAmount": "30000000000000000000",
                    "feeAmount": "0",
                    "kind": "sell",
                    "partiallyFillable": false,
                    "class": "market",
                    "feePolicies": [],
                },
                {
                    "uid": "0x0202020202020202020202020202020202020202020202020202020202020202\
                              0202020202020202020202020202020202020202\
                              02020202",
                    "sellToken": "0x000000000000000000000000000000000000000a",
                    "buyToken": "0x000000000000000000000000000000000000000b",
                    "sellAmount": "100000000000000000000",
                    "buyAmount": "90000000000000000000",
                    "feeAmount": "0",
                    "kind": "sell",
                    "partiallyFillable": false,
                    "class": "market",
                    "feePolicies": [],
                },
            ],
            "liquidity": [
                {
                    "kind": "constantProduct",
                    "tokens": {
                        "0x000000000000000000000000000000000000000a": {
                            "balance": "1000000000000000000000000"
                        },
                        "0x000000000000000000000000000000000000000b": {
                            "balance": "1000000000000000000000000"
                        }
                    },
                    "fee": "0.003",
                    "id": "0",
                    "address": "0xffffffffffffffffffffffffffffffffffffffff",
                    "router": "0xffffffffffffffffffffffffffffffffffffffff",
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
            "solutions": [{
                "id": 0,
                "prices": {
                    "0x000000000000000000000000000000000000000a": "139560520142598496101",
                    "0x000000000000000000000000000000000000000b": "140000000000000000000",
                },
                "trades": [
                    {
                        "kind": "fulfillment",
                        "order": "0x0101010101010101010101010101010101010101010101010101010101010101\
                                    0101010101010101010101010101010101010101\
                                    01010101",
                        "executedAmount": "40000000000000000000",
                    },
                    {
                        "kind": "fulfillment",
                        "order": "0x0202020202020202020202020202020202020202020202020202020202020202\
                                    0202020202020202020202020202020202020202\
                                    02020202",
                        "executedAmount": "100000000000000000000",
                    },
                ],
                "interactions": [
                    {
                        "kind": "liquidity",
                        "internalize": false,
                        "id": "0",
                        "inputToken": "0x000000000000000000000000000000000000000a",
                        "outputToken": "0x000000000000000000000000000000000000000b",
                        "inputAmount": "140000000000000000000",
                        "outputAmount": "139560520142598496102"
                    },
                ],
                "gas": 259417,
            }]
        }),
    );
}

#[tokio::test]
async fn buy_orders_on_both_sides() {
    let engine = tests::SolverEngine::new(
        "naive",
        tests::Config::String(r#"risk-parameters = [0,0,0,0]"#.to_owned()),
    )
    .await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {},
            "orders": [
                {
                    "uid": "0x0101010101010101010101010101010101010101010101010101010101010101\
                              0101010101010101010101010101010101010101\
                              01010101",
                    "sellToken": "0x000000000000000000000000000000000000000a",
                    "buyToken": "0x000000000000000000000000000000000000000b",
                    "sellAmount": "40000000000000000000",
                    "buyAmount": "30000000000000000000",
                    "feeAmount": "0",
                    "kind": "buy",
                    "partiallyFillable": false,
                    "class": "market",
                    "feePolicies": [],
                },
                {
                    "uid": "0x0202020202020202020202020202020202020202020202020202020202020202\
                              0202020202020202020202020202020202020202\
                              02020202",
                    "sellToken": "0x000000000000000000000000000000000000000b",
                    "buyToken": "0x000000000000000000000000000000000000000a",
                    "sellAmount": "100000000000000000000",
                    "buyAmount": "90000000000000000000",
                    "feeAmount": "0",
                    "kind": "buy",
                    "partiallyFillable": false,
                    "class": "market",
                    "feePolicies": [],
                },
            ],
            "liquidity": [
                {
                    "kind": "constantProduct",
                    "tokens": {
                        "0x000000000000000000000000000000000000000a": {
                            "balance": "1000000000000000000000"
                        },
                        "0x000000000000000000000000000000000000000b": {
                            "balance": "1000000000000000000000"
                        }
                    },
                    "fee": "0.003",
                    "id": "0",
                    "address": "0xffffffffffffffffffffffffffffffffffffffff",
                    "router": "0xffffffffffffffffffffffffffffffffffffffff",
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
            "solutions": [{
                "id": 0,
                "prices": {
                    "0x000000000000000000000000000000000000000a": "66231662019024105282",
                    "0x000000000000000000000000000000000000000b": "61942706346833798925",
                },
                "trades": [
                    {
                        "kind": "fulfillment",
                        "order": "0x0101010101010101010101010101010101010101010101010101010101010101\
                                    0101010101010101010101010101010101010101\
                                    01010101",
                        "executedAmount": "30000000000000000000",
                    },
                    {
                        "kind": "fulfillment",
                        "order": "0x0202020202020202020202020202020202020202020202020202020202020202\
                                    0202020202020202020202020202020202020202\
                                    02020202",
                        "executedAmount": "90000000000000000000",
                    },
                ],
                "interactions": [
                    {
                        "kind": "liquidity",
                        "internalize": false,
                        "id": "0",
                        "inputToken": "0x000000000000000000000000000000000000000b",
                        "outputToken": "0x000000000000000000000000000000000000000a",
                        "inputAmount": "66231662019024105282",
                        "outputAmount": "61942706346833798926"
                    },
                ],
                "gas": 259417,
            }]
        }),
    );
}

#[tokio::test]
async fn buy_and_sell_orders() {
    let engine = tests::SolverEngine::new(
        "naive",
        tests::Config::String(r#"risk-parameters = [0,0,0,0]"#.to_owned()),
    )
    .await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {},
            "orders": [
                {
                    "uid": "0x0101010101010101010101010101010101010101010101010101010101010101\
                              0101010101010101010101010101010101010101\
                              01010101",
                    "sellToken": "0x000000000000000000000000000000000000000a",
                    "buyToken": "0x000000000000000000000000000000000000000b",
                    "sellAmount": "40000000000000000000",
                    "buyAmount": "30000000000000000000",
                    "feeAmount": "0",
                    "kind": "buy",
                    "partiallyFillable": false,
                    "class": "market",
                    "feePolicies": [],
                },
                {
                    "uid": "0x0202020202020202020202020202020202020202020202020202020202020202\
                              0202020202020202020202020202020202020202\
                              02020202",
                    "sellToken": "0x000000000000000000000000000000000000000b",
                    "buyToken": "0x000000000000000000000000000000000000000a",
                    "sellAmount": "100000000000000000000",
                    "buyAmount": "90000000000000000000",
                    "feeAmount": "0",
                    "kind": "sell",
                    "partiallyFillable": false,
                    "class": "market",
                    "feePolicies": [],
                },
            ],
            "liquidity": [
                {
                    "kind": "constantProduct",
                    "tokens": {
                        "0x000000000000000000000000000000000000000a": {
                            "balance": "1000000000000000000000"
                        },
                        "0x000000000000000000000000000000000000000b": {
                            "balance": "1000000000000000000000"
                        }
                    },
                    "fee": "0.003",
                    "id": "0",
                    "address": "0xffffffffffffffffffffffffffffffffffffffff",
                    "router": "0xffffffffffffffffffffffffffffffffffffffff",
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
            "solutions": [{
                "id": 0,
                "prices": {
                    "0x000000000000000000000000000000000000000a": "70000000000000000000",
                    "0x000000000000000000000000000000000000000b": "65237102608923246618",
                },
                "trades": [
                    {
                        "kind": "fulfillment",
                        "order": "0x0101010101010101010101010101010101010101010101010101010101010101\
                                    0101010101010101010101010101010101010101\
                                    01010101",
                        "executedAmount": "30000000000000000000",
                    },
                    {
                        "kind": "fulfillment",
                        "order": "0x0202020202020202020202020202020202020202020202020202020202020202\
                                    0202020202020202020202020202020202020202\
                                    02020202",
                        "executedAmount": "100000000000000000000",
                    },
                ],
                "interactions": [
                    {
                        "kind": "liquidity",
                        "internalize": false,
                        "id": "0",
                        "inputToken": "0x000000000000000000000000000000000000000b",
                        "outputToken": "0x000000000000000000000000000000000000000a",
                        "inputAmount": "70000000000000000000",
                        "outputAmount": "65237102608923246619"
                    },
                ],
                "gas": 259417,
            }]
        }),
    );
}
