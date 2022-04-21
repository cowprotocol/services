use crate::debug_bytes;
use anyhow::Result;
use derivative::Derivative;
use ethcontract::{H160, U256};
use model::u256_decimal;
use reqwest::{Client, RequestBuilder, Url};
use serde::{
    de::{DeserializeOwned, Error},
    Deserialize, Deserializer, Serialize,
};
use serde_json::Value;
use thiserror::Error;
use web3::types::Bytes;

const BASE_URL: &str = "https://apiv5.paraswap.io";

/// Mockable implementation of the API for unit test
#[mockall::automock]
#[async_trait::async_trait]
pub trait ParaswapApi: Send + Sync {
    async fn price(&self, query: PriceQuery) -> Result<PriceResponse, ParaswapResponseError>;
    async fn transaction(
        &self,
        query: TransactionBuilderQuery,
    ) -> Result<TransactionBuilderResponse, ParaswapResponseError>;
}

pub struct DefaultParaswapApi {
    pub client: Client,
    pub partner: String,
}

#[async_trait::async_trait]
impl ParaswapApi for DefaultParaswapApi {
    async fn price(&self, query: PriceQuery) -> Result<PriceResponse, ParaswapResponseError> {
        let url = query.into_url(&self.partner);
        tracing::debug!("Querying Paraswap price API: {}", url);
        let response = self.client.get(url).send().await?;
        let status = response.status();
        let text = response.text().await?;
        tracing::debug!(%status, %text, "Response from Paraswap price API");
        parse_paraswap_response_text(&text)
    }
    async fn transaction(
        &self,
        query: TransactionBuilderQuery,
    ) -> Result<TransactionBuilderResponse, ParaswapResponseError> {
        let query = TransactionBuilderQueryWithPartner {
            query,
            partner: &self.partner,
        };
        let response_text = query
            .into_request(&self.client)
            .send()
            .await?
            .text()
            .await?;
        parse_paraswap_response_text(&response_text)
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
// Some Paraswap errors may contain both an error and an Ok response.
// In those cases we should treat the response as an error which is why the error variant
// is declared first (serde will encodes a mixed response as the first matching variant).
pub enum RawResponse<Ok> {
    ResponseErr { error: String },
    ResponseOk(Ok),
}

#[derive(Error, Debug)]
pub enum ParaswapResponseError {
    #[error(transparent)]
    Request(#[from] reqwest::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("insufficient liquidity")]
    InsufficientLiquidity(String),

    #[error("retryable ParaSwap error: {0}")]
    Retryable(String),

    #[error("other ParaSwap error: {0}")]
    Other(String),
}

impl ParaswapResponseError {
    /// Returns true if the error is considered intermittent and the same
    /// ParaSwap request can be retried.
    pub fn is_retryable(&self) -> bool {
        // We don't retry insufficient liquidity errors because it is unlikely a
        // more liquidity will appear by the time we would retry.
        matches!(
            self,
            ParaswapResponseError::Request(_) | ParaswapResponseError::Retryable(_),
        )
    }
}

fn parse_paraswap_response_text<T>(response_text: &str) -> Result<T, ParaswapResponseError>
where
    T: DeserializeOwned,
{
    match serde_json::from_str::<RawResponse<T>>(response_text)? {
        RawResponse::ResponseOk(response) => Ok(response),
        RawResponse::ResponseErr { error: message } => match message.as_str() {
            "ERROR_BUILDING_TRANSACTION"
            | "Error getParaSwapPool"
            | "Server too busy"
            | "Unable to process the transaction"
            | "It seems like the rate has changed, please re-query the latest Price" => {
                Err(ParaswapResponseError::Retryable(message))
            }
            "ESTIMATED_LOSS_GREATER_THAN_MAX_IMPACT"
            | "No routes found with enough liquidity"
            | "Too much slippage on quote, please try again" => {
                Err(ParaswapResponseError::InsufficientLiquidity(message))
            }
            _ => Err(ParaswapResponseError::Other(message)),
        },
    }
}

#[derive(Clone, Debug)]
pub enum Side {
    Buy,
    Sell,
}

/// Paraswap price quote query parameters.
#[derive(Clone, Debug)]
pub struct PriceQuery {
    /// source token address
    pub src_token: H160,
    /// destination token address
    pub dest_token: H160,
    /// decimals of from token (according to API needed  to trade any token)
    pub src_decimals: usize,
    /// decimals of to token (according to API needed to trade any token)
    pub dest_decimals: usize,
    /// amount of source token (in the smallest denomination, e.g. for ETH - 10**18)
    pub amount: U256,
    /// Type of order
    pub side: Side,
    /// The list of DEXs to exclude from the computed price route.
    pub exclude_dexs: Option<Vec<String>>,
}

impl PriceQuery {
    pub fn into_url(self, partner: &str) -> Url {
        let mut url = Url::parse(BASE_URL)
            .expect("invalid base url")
            .join("/prices")
            .expect("unexpectedly invalid URL segment");

        let side = match self.side {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
        };

        url.query_pairs_mut()
            .append_pair("partner", partner)
            .append_pair("srcToken", &format!("{:#x}", self.src_token))
            .append_pair("destToken", &format!("{:#x}", self.dest_token))
            .append_pair("srcDecimals", &self.src_decimals.to_string())
            .append_pair("destDecimals", &self.dest_decimals.to_string())
            .append_pair("amount", &self.amount.to_string())
            .append_pair("side", side)
            .append_pair("network", "1");

        if let Some(dexs) = &self.exclude_dexs {
            url.query_pairs_mut()
                .append_pair("excludeDEXS", &dexs.join(","));
        }

        url
    }
}

/// A Paraswap API price response.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PriceResponse {
    /// Opaque type, which the API expects to get echoed back in the exact form when requesting settlement transaction data
    pub price_route_raw: Value,
    /// The estimated in amount (part of price_route but extracted for type safety & convenience)
    pub src_amount: U256,
    /// The estimated out amount (part of price_route but extracted for type safety & convenience)
    pub dest_amount: U256,
    /// The token transfer proxy address to set an allowance for.
    pub token_transfer_proxy: H160,
    pub gas_cost: u64,
}

impl<'de> Deserialize<'de> for PriceResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ParsedRaw {
            price_route: Value,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct PriceRoute {
            #[serde(with = "u256_decimal")]
            src_amount: U256,
            #[serde(with = "u256_decimal")]
            dest_amount: U256,
            token_transfer_proxy: H160,
            #[serde(with = "serde_with::rust::display_fromstr")]
            gas_cost: u64,
        }

        let parsed = ParsedRaw::deserialize(deserializer)?;
        let PriceRoute {
            src_amount,
            dest_amount,
            token_transfer_proxy,
            gas_cost,
        } = serde_json::from_value::<PriceRoute>(parsed.price_route.clone())
            .map_err(D::Error::custom)?;
        Ok(PriceResponse {
            price_route_raw: parsed.price_route,
            src_amount,
            dest_amount,
            token_transfer_proxy,
            gas_cost,
        })
    }
}

/// Paraswap transaction builder query parameters.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionBuilderQuery {
    /// The sold token
    pub src_token: H160,
    /// The received token
    pub dest_token: H160,
    /// The trade amount amount
    #[serde(flatten)]
    pub trade_amount: TradeAmount,
    /// The maximum slippage in BPS.
    pub slippage: u32,
    /// The decimals of the source token
    pub src_decimals: usize,
    /// The decimals of the destination token
    pub dest_decimals: usize,
    /// priceRoute part from /prices endpoint response (without any change)
    pub price_route: Value,
    /// The address of the signer
    pub user_address: H160,
}

/// The amounts for buying and selling.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum TradeAmount {
    #[serde(rename_all = "camelCase")]
    Sell {
        /// The source amount
        #[serde(with = "u256_decimal")]
        src_amount: U256,
    },
    #[serde(rename_all = "camelCase")]
    Buy {
        /// The destination amount
        #[serde(with = "u256_decimal")]
        dest_amount: U256,
    },
}

/// A helper struct to wrap a `TransactionBuilderQuery` that we get as input from
/// the `ParaswapApi` trait.
///
/// This is done because the `partner` is longer specified in the headersd but
/// instead in the POST body, but we want the API to stay mostly compatible and
/// not require passing it in every time we build a transaction given that the
/// API instance already knows what the `partner` value is.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TransactionBuilderQueryWithPartner<'a> {
    #[serde(flatten)]
    query: TransactionBuilderQuery,
    partner: &'a str,
}

impl TransactionBuilderQueryWithPartner<'_> {
    pub fn into_request(self, client: &Client) -> RequestBuilder {
        let mut url = Url::parse(BASE_URL)
            .expect("invalid base url")
            .join("/transactions/1")
            .expect("unexpectedly invalid URL segment");
        url.query_pairs_mut().append_pair("ignoreChecks", "true");

        tracing::debug!("Paraswap API (transaction) query url: {}", url);
        client.post(url).json(&self)
    }
}

/// Paraswap transaction builder response.
#[derive(Clone, Derivative, Deserialize, Default)]
#[derivative(Debug)]
#[serde(rename_all = "camelCase")]
pub struct TransactionBuilderResponse {
    /// the sender of the built transaction
    pub from: H160,
    /// the target of the built transaction (usually paraswap router)
    pub to: H160,
    /// the chain for which this transaction is valid
    pub chain_id: u64,
    /// the native token value to be set on the transaction
    #[serde(with = "u256_decimal")]
    pub value: U256,
    /// the calldata for the transaction
    #[derivative(Debug(format_with = "debug_bytes"))]
    pub data: Bytes,
    /// the suggested gas price
    #[serde(with = "u256_decimal")]
    pub gas_price: U256,
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::StatusCode;
    use serde_json::json;

    #[tokio::test]
    #[ignore]
    async fn test_api_e2e_sell() {
        let src_token = crate::addr!("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");
        let dest_token = testlib::tokens::GNO;
        let price_query = PriceQuery {
            src_token,
            dest_token,
            src_decimals: 18,
            dest_decimals: 18,
            amount: 135_000_000_000_000_000_000u128.into(),
            side: Side::Sell,
            exclude_dexs: None,
        };

        let price_response: PriceResponse = reqwest::get(price_query.into_url("Test"))
            .await
            .expect("price query failed")
            .json()
            .await
            .expect("Response is not json");

        println!(
            "Price Response: {}",
            serde_json::to_string_pretty(&price_response.price_route_raw).unwrap()
        );

        let transaction_query = TransactionBuilderQueryWithPartner {
            query: TransactionBuilderQuery {
                src_token,
                dest_token,
                trade_amount: TradeAmount::Sell {
                    src_amount: price_response.src_amount,
                },
                slippage: 1000,
                src_decimals: 18,
                dest_decimals: 18,
                price_route: price_response.price_route_raw,
                user_address: crate::addr!("E0B3700e0aadcb18ed8d4BFF648Bc99896a18ad1"),
            },
            partner: "Test",
        };

        println!(
            "Transaction Query: {}",
            serde_json::to_string_pretty(&transaction_query).unwrap()
        );

        let client = Client::new();
        let transaction_response = transaction_query
            .into_request(&client)
            .send()
            .await
            .unwrap();

        let response_status = transaction_response.status();
        let response_text = transaction_response.text().await.unwrap();
        println!("Transaction Response: {}", &response_text);

        assert_eq!(response_status, StatusCode::OK);
        assert!(serde_json::from_str::<TransactionBuilderResponse>(&response_text).is_ok());
    }

    #[tokio::test]
    #[ignore]
    async fn test_api_e2e_buy() {
        let src_token = crate::addr!("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");
        let dest_token = testlib::tokens::GNO;
        let price_query = PriceQuery {
            src_token,
            dest_token,
            src_decimals: 18,
            dest_decimals: 18,
            amount: 1_800_000_000_000_000_000_000u128.into(),
            side: Side::Buy,
            exclude_dexs: Some(vec!["ParaSwapPool4".to_string()]),
        };

        let price_response: PriceResponse = reqwest::get(price_query.into_url("Test"))
            .await
            .expect("price query failed")
            .json()
            .await
            .expect("Response is not json");

        println!(
            "Price Response: {}",
            serde_json::to_string_pretty(&price_response.price_route_raw).unwrap(),
        );

        let transaction_query = TransactionBuilderQueryWithPartner {
            query: TransactionBuilderQuery {
                src_token,
                dest_token,
                trade_amount: TradeAmount::Buy {
                    dest_amount: price_response.dest_amount,
                },
                slippage: 1000,
                src_decimals: 18,
                dest_decimals: 18,
                price_route: price_response.price_route_raw,
                user_address: crate::addr!("E0B3700e0aadcb18ed8d4BFF648Bc99896a18ad1"),
            },
            partner: "Test",
        };

        let client = Client::new();
        let transaction_response = transaction_query
            .into_request(&client)
            .send()
            .await
            .unwrap();

        let response_status = transaction_response.status();
        let response_text = transaction_response.text().await.unwrap();
        println!("Transaction Response: {}", &response_text);

        assert_eq!(response_status, StatusCode::OK);
        assert!(serde_json::from_str::<TransactionBuilderResponse>(&response_text).is_ok());
    }

    #[test]
    fn test_price_query_serialization() {
        let query = PriceQuery {
            src_token: crate::addr!("EeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"),
            dest_token: testlib::tokens::GNO,
            src_decimals: 18,
            dest_decimals: 8,
            amount: 1_000_000_000_000_000_000u128.into(),
            side: Side::Sell,
            exclude_dexs: Some(vec!["Foo".to_string(), "Bar".to_string()]),
        };

        assert_eq!(&query.into_url("Test").to_string(), "https://apiv5.paraswap.io/prices?partner=Test&srcToken=0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee&destToken=0x6810e776880c02933d47db1b9fc05908e5386b96&srcDecimals=18&destDecimals=8&amount=1000000000000000000&side=SELL&network=1&excludeDEXS=Foo%2CBar");
    }

    #[test]
    fn test_price_query_response_deserialization() {
        let result: PriceResponse = serde_json::from_str::<PriceResponse>(
            r#"{
              "priceRoute": {
                "blockNumber": 13036269,
                "network": 1,
                "src": "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE",
                "srcDecimals": 18,
                "srcAmount": "10000000000000000",
                "dest": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
                "destDecimals": 6,
                "destAmount": "32704734",
                "bestRoute": [
                  {
                    "percent": 100,
                    "swaps": [
                      {
                        "src": "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE",
                        "srcDecimals": 18,
                        "dest": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
                        "destDecimals": 6,
                        "swapExchanges": [
                          {
                            "exchange": "UniswapV2",
                            "srcAmount": "10000000000000000",
                            "destAmount": "32704734",
                            "percent": 100,
                            "data": {
                              "router": "0x0000000000000000000000000000000000000000",
                              "path": [
                                "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                                "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
                              ],
                              "factory": "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f",
                              "initCode": "0x96e8ac4277198ff8b6f785478aa9a39f403cb768dd02cbee326c3e7da348845f",
                              "feeFactor": 10000,
                              "pools": [
                                {
                                  "address": "0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc",
                                  "fee": 30,
                                  "direction": false
                                }
                              ],
                              "gasUSD": "9.835332"
                            }
                          }
                        ]
                      }
                    ]
                  }
                ],
                "gasCostUSD": "13.700002",
                "gasCost": "111435",
                "side": "SELL",
                "tokenTransferProxy": "0x0000000000000000000000000000000000000000",
                "contractAddress": "0x0000000000000000000000000000000000000000",
                "contractMethod": "swapOnUniswap",
                "partnerFee": 0,
                "srcUSD": "32.7332000000",
                "destUSD": "32.5799000303",
                "partner": "paraswap",
                "maxImpactReached": false,
                "hmac": "cf2ac4b20f83b6656eb9dd28e26414658430e1d5"
              }
            }"#).unwrap();

        assert_eq!(result.src_amount, 10_000_000_000_000_000_u128.into());
        assert_eq!(result.dest_amount, 32_704_734_u128.into());
    }

    #[test]
    fn test_price_query_response_deserialization_with_error() {
        let result = serde_json::from_str::<RawResponse<PriceResponse>>(
            r#"{
                "error": "ESTIMATED_LOSS_GREATER_THAN_MAX_IMPACT",
                "value": "28.13%",
                "priceRoute": {
                  "bestRoute": [
                    {
                      "exchange": "UniswapV2",
                      "srcAmount": "34020118741679034368",
                      "destAmount": "132586194058791470",
                      "percent": "100.00",
                      "data": {
                        "router": "0x86d3579b043585A97532514016dCF0C2d6C4b6a1",
                        "path": [
                          "0x960b236a07cf122663c4303350609a66a7b288c0",
                          "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                        ],
                        "factory": "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f",
                        "initCode": "0x96e8ac4277198ff8b6f785478aa9a39f403cb768dd02cbee326c3e7da348845f",
                        "tokenFrom": "0x960b236a07cf122663c4303350609a66a7b288c0",
                        "tokenTo": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "gasUSD": "16.343060"
                      },
                      "destAmountFeeDeducted": "132586194058791470"
                    }
                  ],
                  "blockNumber": 13115088,
                  "destAmount": "132586194058791470",
                  "srcAmount": "34020118741679034368",
                  "adapterVersion": "4.0.0",
                  "others": [
                    {
                      "exchange": "UniswapV2",
                      "rate": "132586194058791470",
                      "unit": "3925118665550511",
                      "data": {
                        "router": "0x86d3579b043585A97532514016dCF0C2d6C4b6a1",
                        "path": [
                          "0x960b236a07cf122663c4303350609a66a7b288c0",
                          "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                        ],
                        "factory": "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f",
                        "initCode": "0x96e8ac4277198ff8b6f785478aa9a39f403cb768dd02cbee326c3e7da348845f",
                        "tokenFrom": "0x960b236a07cf122663c4303350609a66a7b288c0",
                        "tokenTo": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "gasUSD": "16.343060"
                      },
                      "rateFeeDeducted": "132586194058791470",
                      "unitFeeDeducted": "3925118665550511"
                    },
                    {
                      "exchange": "Balancer",
                      "rate": "123444215277050183",
                      "unit": "4186233534292740",
                      "data": {
                        "pool": "0x2cf9106faf2c5c8713035d40df655fb1b9b0f9b9",
                        "exchangeProxy": "0x6317c5e82a06e1d8bf200d21f4510ac2c038ac81",
                        "tokenFrom": "0x960b236a07cf122663c4303350609a66a7b288c0",
                        "tokenTo": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "gasUSD": "24.514591"
                      },
                      "rateFeeDeducted": "123444215277050183",
                      "unitFeeDeducted": "4186233534292740"
                    },
                    {
                      "exchange": "SushiSwap",
                      "rate": "2750123947578310",
                      "unit": "1787208779577095",
                      "data": {
                        "router": "0xBc1315CD2671BC498fDAb42aE1214068003DC51e",
                        "path": [
                          "0x960b236a07cf122663c4303350609a66a7b288c0",
                          "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                        ],
                        "factory": "0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac",
                        "initCode": "0xe18a34eb0e04b04f7a0ac29a6e80748dca96319b42c54d679cb821dca90c6303",
                        "tokenFrom": "0x960b236a07cf122663c4303350609a66a7b288c0",
                        "tokenTo": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "gasUSD": "18.385943"
                      },
                      "rateFeeDeducted": "2750123947578310",
                      "unitFeeDeducted": "1787208779577095"
                    },
                    {
                      "exchange": "Kyber",
                      "rate": "137840846740713684",
                      "unit": "4052387741876373",
                      "data": {
                        "exchange": "0x9AAb3f75489902f3a48495025729a0AF77d4b11e",
                        "tokenFrom": "0x960b236a07cf122663c4303350609a66a7b288c0",
                        "tokenTo": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "gasUSD": "57.200711"
                      },
                      "rateFeeDeducted": "137840846740713684",
                      "unitFeeDeducted": "4052387741876373"
                    }
                  ],
                  "side": "SELL",
                  "details": {
                    "tokenFrom": "0x960b236a07cf122663c4303350609a66a7b288c0",
                    "tokenTo": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                    "srcAmount": "34020118741679034368",
                    "destAmount": "132586194058791470"
                  },
                  "bestRouteGas": "189300",
                  "bestRouteGasCostUSD": "38.671767",
                  "contractMethod": "swapOnUniswap",
                  "fromUSD": "599.5691475494",
                  "toUSD": "430.8894557824",
                  "priceWithSlippage": "131260332118203555",
                  "spender": "0xb70Bc06D2c9Bf03b3373799606dc7d39346c06B3",
                  "destAmountFeeDeducted": "132586194058791470",
                  "toUSDFeeDeducted": "430.8894557824",
                  "multiRoute": [],
                  "maxImpactReached": true,
                  "priceID": "385c3f7a-f57b-4295-b5f6-b210c0c7ec1d",
                  "hmac": "59baa2597b5aeebd13eca67fdbe2d6765decc328"
                }
              }"#).unwrap();

        assert!(matches!(result, RawResponse::ResponseErr { error: _ }));
    }

    #[test]
    fn paraswap_response_handling() {
        let parse = |s: &str| parse_paraswap_response_text::<bool>(s);
        assert!(matches!(
            parse("invalid JSON"),
            Err(ParaswapResponseError::Json(_))
        ));

        assert!(matches!(
            parse("{\"error\": \"Never seen this before\"}"),
            Err(ParaswapResponseError::Other(_))
        ));

        for liquidity_error in &[
            "ESTIMATED_LOSS_GREATER_THAN_MAX_IMPACT",
            "No routes found with enough liquidity",
            "Too much slippage on quote, please try again",
        ] {
            assert!(matches!(
                parse(&format!("{{\"error\": \"{}\"}}", liquidity_error)),
                Err(ParaswapResponseError::InsufficientLiquidity(message)) if &message == liquidity_error,
            ));
        }

        for retryable_error in &[
            "ERROR_BUILDING_TRANSACTION",
            "Error getParaSwapPool",
            "Server too busy",
            "Unable to process the transaction",
            "It seems like the rate has changed, please re-query the latest Price",
        ] {
            assert!(matches!(
                parse(&format!("{{\"error\": \"{}\"}}", retryable_error)),
                Err(ParaswapResponseError::Retryable(message)) if &message == retryable_error,
            ));
        }
    }

    #[tokio::test]
    #[ignore]
    async fn transaction_response_error() {
        let src_token = crate::addr!("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");
        let dest_token = testlib::tokens::GNO;
        let price_query = PriceQuery {
            src_token,
            dest_token,
            src_decimals: 18,
            dest_decimals: 18,
            amount: 135_000_000_000_000_000_000u128.into(),
            side: Side::Sell,
            exclude_dexs: None,
        };

        let price_response: PriceResponse = reqwest::get(price_query.into_url("Test"))
            .await
            .expect("price query failed")
            .json()
            .await
            .expect("Response is not json");

        let api = DefaultParaswapApi {
            client: Client::new(),
            partner: "Test".into(),
        };

        let good_query = TransactionBuilderQuery {
            src_token,
            dest_token,
            trade_amount: TradeAmount::Sell {
                src_amount: price_response.src_amount,
            },
            slippage: 1000, // 10%
            src_decimals: 18,
            dest_decimals: 18,
            price_route: price_response.price_route_raw,
            user_address: crate::addr!("E0B3700e0aadcb18ed8d4BFF648Bc99896a18ad1"),
        };

        assert!(api.transaction(good_query).await.is_ok());
    }

    #[test]
    fn transaction_query_serialization() {
        assert_eq!(
            serde_json::to_value(TransactionBuilderQuery {
                src_token: H160([1; 20]),
                dest_token: H160([2; 20]),
                trade_amount: TradeAmount::Sell {
                    src_amount: 1337.into(),
                },
                slippage: 250,
                src_decimals: 18,
                dest_decimals: 18,
                price_route: Value::Null,
                user_address: H160([3; 20]),
            })
            .unwrap(),
            json!({
                "srcToken": H160([1; 20]),
                "destToken": H160([2; 20]),
                "srcAmount": "1337",
                "slippage": 250,
                "srcDecimals": 18,
                "destDecimals": 18,
                "priceRoute": Value::Null,
                "userAddress": H160([3; 20]),
            }),
        );
    }
}
