use anyhow::{Context, Result};
use derivative::Derivative;
use ethcontract::{H160, U256};
use reqwest::{Client, RequestBuilder, Url};
use serde::{de::Error, Deserialize, Deserializer, Serialize};
use serde_json::Value;

use model::u256_decimal;
use web3::types::Bytes;

const BASE_URL: &str = "https://apiv4.paraswap.io";

/// Mockable implementation of the API for unit test
#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait ParaswapApi {
    async fn price(&self, query: PriceQuery) -> Result<PriceResponse>;
    async fn transaction(
        &self,
        query: TransactionBuilderQuery,
    ) -> Result<TransactionBuilderResponse>;
}

#[derive(Default)]
pub struct DefaultParaswapApi {
    client: Client,
}

#[async_trait::async_trait]
impl ParaswapApi for DefaultParaswapApi {
    async fn price(&self, query: PriceQuery) -> Result<PriceResponse> {
        let text = reqwest::get(query.into_url())
            .await
            .context("PriceQuery failed")?
            .text()
            .await?;

        serde_json::from_str::<PriceResponse>(&text)
            .context(format!("PriceQuery result parsing failed: {}", text))
    }
    async fn transaction(
        &self,
        query: TransactionBuilderQuery,
    ) -> Result<TransactionBuilderResponse> {
        let text = query
            .into_request(&self.client)
            .send()
            .await
            .context("TransactionBuilderQuery failed")?
            .text()
            .await?;
        serde_json::from_str::<TransactionBuilderResponse>(&text).context(format!(
            "TransactionBuilderQuery result parsing failed: {}",
            text
        ))
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
    pub from: H160,
    /// destination token address
    pub to: H160,
    /// decimals of from token (according to API needed  to trade any token)
    pub from_decimals: usize,
    /// decimals of to token (according to API needed to trade any token)
    pub to_decimals: usize,
    /// amount of source token (in the smallest denomination, e.g. for ETH - 10**18)
    pub amount: U256,
    // Type of order
    pub side: Side,
}

impl PriceQuery {
    pub fn into_url(self) -> Url {
        let mut url = Url::parse(BASE_URL)
            .expect("invalid base url")
            .join("v2/prices")
            .expect("unexpectedly invalid URL segment");

        let side = match self.side {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
        };

        url.query_pairs_mut()
            .append_pair("from", &format!("{:#x}", self.from))
            .append_pair("to", &format!("{:#x}", self.to))
            .append_pair("fromDecimals", &self.from_decimals.to_string())
            .append_pair("toDecimals", &self.to_decimals.to_string())
            .append_pair("amount", &self.amount.to_string())
            .append_pair("side", side)
            .append_pair("network", "1");
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
        }

        let parsed = ParsedRaw::deserialize(deserializer)?;
        let PriceRoute {
            src_amount,
            dest_amount,
        } = serde_json::from_value::<PriceRoute>(parsed.price_route.clone())
            .map_err(D::Error::custom)?;
        Ok(PriceResponse {
            price_route_raw: parsed.price_route,
            src_amount,
            dest_amount,
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
    /// The source amount
    #[serde(with = "u256_decimal")]
    pub src_amount: U256,
    /// The amount (from priceRoute) - slippage
    #[serde(with = "u256_decimal")]
    pub dest_amount: U256,
    /// The decimals of the source token
    pub from_decimals: usize,
    /// The decimals of the destination token
    pub to_decimals: usize,
    /// priceRoute part from /prices endpoint response (without any change)
    pub price_route: Value,
    /// The address of the signer
    pub user_address: H160,
    /// partner's referrer string, important if the partner takes fees
    pub referrer: String,
}

impl TransactionBuilderQuery {
    pub fn into_request(self, client: &Client) -> RequestBuilder {
        let mut url = Url::parse(BASE_URL)
            .expect("invalid base url")
            .join("/v2/transactions/1")
            .expect("unexpectedly invalid URL segment");
        url.query_pairs_mut().append_pair("skipChecks", "true");

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

fn debug_bytes(bytes: &Bytes, formatter: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
    formatter.write_fmt(format_args!("0x{}", hex::encode(&bytes.0)))
}

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;

    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_api_e2e() {
        let from = shared::addr!("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");
        let to = shared::addr!("6810e776880c02933d47db1b9fc05908e5386b96");
        let price_query = PriceQuery {
            from,
            to,
            from_decimals: 18,
            to_decimals: 18,
            amount: 135_000_000_000_000_000_000u128.into(),
            side: Side::Sell,
        };

        let price_response: PriceResponse = reqwest::get(price_query.into_url())
            .await
            .expect("price query failed")
            .json()
            .await
            .expect("Response is not json");

        println!("Price Response: {:?}", &price_response,);

        let transaction_query = TransactionBuilderQuery {
            src_token: from,
            dest_token: to,
            src_amount: price_response.src_amount,
            // 10% slippage
            dest_amount: price_response.dest_amount * 90 / 100,
            from_decimals: 18,
            to_decimals: 18,
            price_route: price_response.price_route_raw,
            user_address: shared::addr!("E0B3700e0aadcb18ed8d4BFF648Bc99896a18ad1"),
            referrer: "GPv2".to_string(),
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
            from: shared::addr!("EeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"),
            to: shared::addr!("6810e776880C02933D47DB1b9fc05908e5386b96"),
            from_decimals: 18,
            to_decimals: 8,
            amount: 1_000_000_000_000_000_000u128.into(),
            side: Side::Sell,
        };

        assert_eq!(&query.into_url().to_string(), "https://apiv4.paraswap.io/v2/prices?from=0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee&to=0x6810e776880c02933d47db1b9fc05908e5386b96&fromDecimals=18&toDecimals=8&amount=1000000000000000000&side=SELL&network=1");
    }

    #[test]
    fn test_price_query_response_deserialization() {
        let result: PriceResponse = serde_json::from_str::<PriceResponse>(
            r#"{
                "priceRoute": {
                  "bestRoute": [
                    {
                      "exchange": "UniswapV2",
                      "srcAmount": "100000000000000000",
                      "destAmount": "1444292761374042400",
                      "percent": "100",
                      "data": {
                        "router": "0x86d3579b043585A97532514016dCF0C2d6C4b6a1",
                        "path": [
                          "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                          "0x6810e776880c02933d47db1b9fc05908e5386b96"
                        ],
                        "factory": "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f",
                        "initCode": "0x96e8ac4277198ff8b6f785478aa9a39f403cb768dd02cbee326c3e7da348845f",
                        "tokenFrom": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                        "tokenTo": "0x6810e776880c02933d47db1b9fc05908e5386b96",
                        "gasUSD": "5.473000"
                      },
                      "destAmountFeeDeducted": "1444292761374042400"
                    }
                  ],
                  "blockNumber": 12570470,
                  "destAmount": "1444292761374042400",
                  "srcAmount": "100000000000000000",
                  "adapterVersion": "4.0.0",
                  "others": [
                    {
                      "exchange": "Uniswap",
                      "rate": "1169158453388579682",
                      "unit": "4739285565781337029",
                      "data": {
                        "factory": "0xc0a47dFe034B400B47bDaD5FecDa2621de6c4d95",
                        "tokenFrom": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                        "tokenTo": "0x6810e776880c02933d47db1b9fc05908e5386b96",
                        "gasUSD": "5.473000"
                      },
                      "rateFeeDeducted": "1169158453388579682",
                      "unitFeeDeducted": "4739285565781337029"
                    },
                    {
                      "exchange": "UniswapV2",
                      "rate": "1444292761374042342",
                      "unit": "14437807769106106935",
                      "data": {
                        "router": "0x86d3579b043585A97532514016dCF0C2d6C4b6a1",
                        "path": [
                          "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                          "0x6810e776880c02933d47db1b9fc05908e5386b96"
                        ],
                        "factory": "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f",
                        "initCode": "0x96e8ac4277198ff8b6f785478aa9a39f403cb768dd02cbee326c3e7da348845f",
                        "tokenFrom": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                        "tokenTo": "0x6810e776880c02933d47db1b9fc05908e5386b96",
                        "gasUSD": "5.473000"
                      },
                      "rateFeeDeducted": "1444292761374042342",
                      "unitFeeDeducted": "14437807769106106935"
                    },
                    {
                      "exchange": "Balancer",
                      "rate": "1446394472758668036",
                      "unit": "14458681790856736451",
                      "data": {
                        "pool": "0xdbe29107464d469c64a02afe631aba2e6fabedce",
                        "exchangeProxy": "0x6317c5e82a06e1d8bf200d21f4510ac2c038ac81",
                        "tokenFrom": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                        "tokenTo": "0x6810e776880c02933d47db1b9fc05908e5386b96",
                        "gasUSD": "8.209500"
                      },
                      "rateFeeDeducted": "1446394472758668036",
                      "unitFeeDeducted": "14458681790856736451"
                    },
                    {
                      "exchange": "SushiSwap",
                      "rate": "1430347602573572564",
                      "unit": "14173057789613627150",
                      "data": {
                        "router": "0xBc1315CD2671BC498fDAb42aE1214068003DC51e",
                        "path": [
                          "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                          "0x6810e776880c02933d47db1b9fc05908e5386b96"
                        ],
                        "factory": "0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac",
                        "initCode": "0xe18a34eb0e04b04f7a0ac29a6e80748dca96319b42c54d679cb821dca90c6303",
                        "tokenFrom": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                        "tokenTo": "0x6810e776880c02933d47db1b9fc05908e5386b96",
                        "gasUSD": "6.157125"
                      },
                      "rateFeeDeducted": "1430347602573572564",
                      "unitFeeDeducted": "14173057789613627150"
                    },
                    {
                      "exchange": "UniswapV3",
                      "rate": "1414143411381299064",
                      "unit": "14132797230855578366",
                      "data": {
                        "fee": 10000,
                        "tokenFrom": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                        "tokenTo": "0x6810e776880c02933d47db1b9fc05908e5386b96",
                        "gasUSD": "13.682500"
                      },
                      "rateFeeDeducted": "1414143411381299064",
                      "unitFeeDeducted": "14132797230855578366"
                    }
                  ],
                  "side": "SELL",
                  "details": {
                    "tokenFrom": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                    "tokenTo": "0x6810e776880c02933d47db1b9fc05908e5386b96",
                    "srcAmount": "100000000000000000",
                    "destAmount": "1444292761374042400"
                  },
                  "bestRouteGas": "111435",
                  "bestRouteGasCostUSD": "7.623546",
                  "contractMethod": "swapOnUniswap",
                  "fromUSD": "273.6500000000",
                  "toUSD": "268.2051657871",
                  "priceWithSlippage": "1429849833760301976",
                  "spender": "0xb70Bc06D2c9Bf03b3373799606dc7d39346c06B3",
                  "destAmountFeeDeducted": "1444292761374042400",
                  "toUSDFeeDeducted": "268.2051657871",
                  "multiRoute": [],
                  "maxImpactReached": false,
                  "priceID": "a515b0ec-6cb8-4062-b6d1-b38b33bd05cb",
                  "hmac": "f82acc4c0191938b6eebc6eada0899e53e03d377"
                }
              }"#).unwrap();

        assert_eq!(result.src_amount, 100_000_000_000_000_000u128.into());
        assert_eq!(result.dest_amount, 1_444_292_761_374_042_400u128.into());
    }
}
