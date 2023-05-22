//! This test ensures that the 1inch solver properly handles cases where no swap
//! was found for the specified order.

use {
    crate::tests::{self, mock},
    serde_json::json,
};

#[tokio::test]
async fn test() {
    let api = mock::http::setup(vec![
        mock::http::Expectation::Get {
            path: mock::http::Path::exact("liquidity-sources"),
            res: json!(
                {
                  "protocols": [
                    {
                      "id": "UNISWAP_V1",
                      "title": "Uniswap V1",
                      "img": "https://cdn.1inch.io/liquidity-sources-logo/uniswap.png",
                      "img_color": "https://cdn.1inch.io/liquidity-sources-logo/uniswap_color.png"
                    },
                    {
                      "id": "UNISWAP_V2",
                      "title": "Uniswap V2",
                      "img": "https://cdn.1inch.io/liquidity-sources-logo/uniswap.png",
                      "img_color": "https://cdn.1inch.io/liquidity-sources-logo/uniswap_color.png"
                    },
                    {
                      "id": "SUSHI",
                      "title": "SushiSwap",
                      "img": "https://cdn.1inch.io/liquidity-sources-logo/sushiswap.png",
                      "img_color": "https://cdn.1inch.io/liquidity-sources-logo/sushiswap_color.png"
                    },
                    {
                      "id": "UNISWAP_V3",
                      "title": "Uniswap V3",
                      "img": "https://cdn.1inch.io/liquidity-sources-logo/uniswap.png",
                      "img_color": "https://cdn.1inch.io/liquidity-sources-logo/uniswap_color.png"
                    },
                  ]
                }
            ),
        },
        mock::http::Expectation::Get {
            path: mock::http::Path::exact("approve/spender"),
            res: json!({ "address": "0x1111111254eeb25477b68fb85ed929f73a960582" }),
        },
        mock::http::Expectation::Get {
            path: mock::http::Path::Any,
            res: json!({
                "statusCode": 400,
                "error": "Bad Request",
                "description": "insufficient liquidity",
                "meta": [{ "type": "toTokenAmount", "value": "0" }],
                "requestId": "82444c15-9092-473a-82da-da3dd1de6d2d"
            }),
        },
    ])
    .await;

    let engine = tests::SolverEngine::new("oneinch", super::config(&api.address)).await;

    let solution = engine
        .solve(json!({
            "id": null,
            "tokens": {},
            "orders": [
                {
                    "uid": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a",
                    "sellToken": "0x1111111111111111111111111111111111111111",
                    "buyToken": "0x2222222222222222222222222222222222222222",
                    "sellAmount": "1000000000000000000",
                    "buyAmount": "1000000000000000000",
                    "feeAmount": "1000000000000000",
                    "kind": "sell",
                    "partiallyFillable": false,
                    "class": "market",
                },
            ],
            "liquidity": [],
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
