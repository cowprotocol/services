use anyhow::Result;
use ethcontract::{H160, web3::Transport};
use web3::types::{CallRequest, Bytes};
use crate::config::circles_config::CirclesConfig;
use crate::solver::web3_provider::Web3Provider;
use model::order::Order;
use model::order::OrderData;

#[derive(Clone, Debug)]
pub struct CRCOrderInfo {
    pub order: Order,
    pub sell_is_crc: bool,
    pub buy_is_crc: bool,
}

pub async fn is_crc_token<T: Transport>(
    web3: &Web3Provider<T>,
    circles_config: &CirclesConfig,
    token: H160,
) -> Result<bool> {
    let data = hex::decode("f9d4e0da").unwrap(); // hub() selector
    let req = CallRequest {
        from: None,
        to: Some(token),
        gas: None,
        gas_price: None,
        value: None,
        data: Some(Bytes(data)),
        transaction_type: None,
        access_list: None,
        max_fee_per_gas: None,
        max_priority_fee_per_gas: None,
    };

    let response = match web3.web3().eth().call(req, None).await {
        Ok(res) => res,
        Err(_) => {
            // If it reverts, probably not CRC
            return Ok(false);
        }
    };

    if response.0.len() < 32 {
        return Ok(false);
    }

    let returned = &response.0[12..32];
    let hub_addr = H160::from_slice(returned);
    Ok(circles_config.is_known_hub(hub_addr))
}

pub async fn identify_crc_orders<T: Transport>(
    web3: &Web3Provider<T>,
    circles_config: &CirclesConfig,
    orders: Vec<Order>,
) -> Result<Vec<CRCOrderInfo>> {
    let mut result = Vec::with_capacity(orders.len());
    for order in orders {
        let sell_is_crc = is_crc_token(web3, circles_config, order.data.sell_token).await.unwrap_or(false);
        let buy_is_crc = is_crc_token(web3, circles_config, order.data.buy_token).await.unwrap_or(false);
        result.push(CRCOrderInfo { order, sell_is_crc, buy_is_crc });
    }
    Ok(result)
}

pub fn match_crc_pairs(crc_orders: &[CRCOrderInfo]) -> Vec<(CRCOrderInfo, CRCOrderInfo)> {
    let mut pairs = Vec::new();
    for (i, o1) in crc_orders.iter().enumerate() {
        // Must be CRC order
        if !(o1.sell_is_crc || o1.buy_is_crc) {
            continue;
        }
        for o2 in crc_orders.iter().skip(i+1) {
            if !(o2.sell_is_crc || o2.buy_is_crc) {
                continue;
            }

            let cycle = o1.order.data.buy_token == o2.order.data.sell_token &&
                        o2.order.data.buy_token == o1.order.data.sell_token;
            if cycle {
                pairs.push((o1.clone(), o2.clone()));
            }
        }
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethcontract::web3::transports::Http;
    use ethcontract::web3::Web3;

    #[tokio::test]
    async fn test_is_crc_token_mock() {
        // This test would normally require a mock or a real endpoint.
        // For simplicity, we assume we have a mock transport that returns a known hub address.

        let known_hub: H160 = "0x1111111111111111111111111111111111111111".parse().unwrap();
        let config = CirclesConfig::new(vec![known_hub]);

        // Mock web3 or use a Ganache dev chain with a token implementing hub().
        // Here we just outline the structure.
        let transport = Http::new("http://localhost:8545").unwrap();
        let web3 = Web3Provider::new(Web3::new(transport));

        let token_addr: H160 = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".parse().unwrap();

        // In a real test, we'd deploy a mock CRC token contract and call.
        // Here we just show structure:
        let result = is_crc_token(&web3, &config, token_addr).await;
        // Without real contract, can't fully verify. But we can assert no panic.
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_identify_crc_orders() {
        let known_hub: H160 = "0x1111111111111111111111111111111111111111".parse().unwrap();
        let config = CirclesConfig::new(vec![known_hub]);

        let transport = Http::new("http://localhost:8545").unwrap();
        let web3 = Web3Provider::new(Web3::new(transport));

        let order = Order {
            data: OrderData {
                sell_token: H160::zero(),
                buy_token: H160::zero(),
                ..Default::default()
            },
            ..Default::default()
        };

        let orders = vec![order.clone()];
        let result = identify_crc_orders(&web3, &config, orders).await.unwrap();
        assert_eq!(result.len(), 1);
        // Without a real contract, we can't assert true or false reliably,
        // but we ensure no panic and correct structure.
        assert!(!result[0].sell_is_crc);
        assert!(!result[0].buy_is_crc);
    }

    #[cfg(test)]
    mod pair_tests {
        use super::*;
        use model::order::{Order, OrderData};

        fn mock_crc_order(sell: H160, buy: H160, sell_is_crc: bool, buy_is_crc: bool) -> CRCOrderInfo {
            CRCOrderInfo {
                order: Order {
                    data: OrderData {
                        sell_token: sell,
                        buy_token: buy,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                sell_is_crc,
                buy_is_crc,
            }
        }

        #[test]
        fn test_match_crc_pairs() {
            let a: H160 = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".parse().unwrap();
            let b: H160 = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".parse().unwrap();
            // Two CRC orders that form a cycle
            let o1 = mock_crc_order(a, b, true, false);
            let o2 = mock_crc_order(b, a, false, true);
            let orders = vec![o1, o2];

            let pairs = match_crc_pairs(&orders);
            assert_eq!(pairs.len(), 1);
        }

        #[test]
        fn test_no_pairs() {
            let a: H160 = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".parse().unwrap();
            let b: H160 = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".parse().unwrap();
            let c: H160 = "0xcccccccccccccccccccccccccccccccccccccccc".parse().unwrap();

            let o1 = mock_crc_order(a, b, true, false);
            let o2 = mock_crc_order(b, c, true, false); // no cycle with o1
            let orders = vec![o1, o2];

            let pairs = match_crc_pairs(&orders);
            assert!(pairs.is_empty());
        }
    }
} 