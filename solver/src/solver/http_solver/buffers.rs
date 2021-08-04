use contracts::ERC20;
use ethcontract::{batch::CallBatch, errors::MethodError, H160, U256};
use futures::{future::join_all, join};
use model::order::BUY_ETH_ADDRESS;
use shared::Web3;
use std::collections::HashMap;

const MAX_BATCH_SIZE: usize = 100;

#[derive(Clone)]
/// Computes the amount of "buffer" ERC20 balance that the http solver can use
/// to offset possible rounding errors in computing the amounts in a solution.
pub struct BufferRetriever {
    web3: Web3,
    settlement_contract: H160,
}

impl BufferRetriever {
    pub fn new(web3: Web3, settlement_contract: H160) -> Self {
        Self {
            web3,
            settlement_contract,
        }
    }
}

#[derive(Debug)]
pub enum BufferRetrievalError {
    Eth(web3::Error),
    Erc20(MethodError),
}

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait BufferRetrieving: Send + Sync {
    async fn get_buffers(
        &self,
        tokens: &[H160],
    ) -> HashMap<H160, Result<U256, BufferRetrievalError>>;
}

#[async_trait::async_trait]
impl BufferRetrieving for BufferRetriever {
    async fn get_buffers(
        &self,
        tokens: &[H160],
    ) -> HashMap<H160, Result<U256, BufferRetrievalError>> {
        let mut batch = CallBatch::new(self.web3.transport());
        let tokens_without_eth: Vec<_> = tokens
            .iter()
            .filter(|&&address| address != BUY_ETH_ADDRESS)
            .collect();

        let futures = tokens_without_eth
            .iter()
            .map(|&&address| {
                let erc20 = ERC20::at(&self.web3, address);
                erc20
                    .methods()
                    .balance_of(self.settlement_contract)
                    .batch_call(&mut batch)
            })
            .collect::<Vec<_>>();

        let mut buffers = HashMap::new();

        if tokens_without_eth.len() == tokens.len() {
            batch.execute_all(MAX_BATCH_SIZE).await;
        } else {
            let (_, eth_balance) = join!(
                batch.execute_all(MAX_BATCH_SIZE),
                self.web3.eth().balance(self.settlement_contract, None)
            );
            buffers.insert(
                BUY_ETH_ADDRESS,
                eth_balance.map_err(BufferRetrievalError::Eth),
            );
        }

        buffers
            .into_iter()
            .chain(
                tokens_without_eth
                    .into_iter()
                    .zip(join_all(futures).await.into_iter())
                    .map(|(&address, balance)| {
                        (address, balance.map_err(BufferRetrievalError::Erc20))
                    }),
            )
            .collect()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use contracts::GPv2Settlement;
    use hex_literal::hex;
    use shared::transport::create_test_transport;

    #[tokio::test]
    #[ignore]
    async fn retrieves_buffers_on_rinkeby() {
        let web3 = Web3::new(create_test_transport(
            &std::env::var("NODE_URL_RINKEBY").unwrap(),
        ));
        let settlement_contract = GPv2Settlement::deployed(&web3).await.unwrap();
        let weth = H160(hex!("c778417E063141139Fce010982780140Aa0cD5Ab"));
        let dai = H160(hex!("c7ad46e0b8a400bb3c915120d284aafba8fc4735"));
        let not_a_token = H160(hex!("badbadbadbadbadbadbadbadbadbadbadbadbadb"));

        let buffer_retriever = BufferRetriever::new(web3, settlement_contract.address());
        let buffers = buffer_retriever
            .get_buffers(&[weth, dai, BUY_ETH_ADDRESS, not_a_token])
            .await;
        println!("Buffers: {:#?}", buffers);
        assert!(buffers.get(&weth).unwrap().is_ok());
        assert!(buffers.get(&dai).unwrap().is_ok());
        assert!(buffers.get(&BUY_ETH_ADDRESS).unwrap().is_ok());
        assert!(buffers.get(&not_a_token).unwrap().is_err());
    }

    #[tokio::test]
    #[ignore]
    async fn retrieving_buffers_not_affected_by_eth() {
        let web3 = Web3::new(create_test_transport(
            &std::env::var("NODE_URL_RINKEBY").unwrap(),
        ));
        let settlement_contract = GPv2Settlement::deployed(&web3).await.unwrap();
        let weth = H160(hex!("c778417E063141139Fce010982780140Aa0cD5Ab"));
        let dai = H160(hex!("c7ad46e0b8a400bb3c915120d284aafba8fc4735"));
        let not_a_token = H160(hex!("badbadbadbadbadbadbadbadbadbadbadbadbadb"));

        let buffer_retriever = BufferRetriever::new(web3, settlement_contract.address());
        let buffers_with_eth = buffer_retriever
            .get_buffers(&[weth, dai, not_a_token])
            .await;
        let buffers_without_eth = buffer_retriever
            .get_buffers(&[weth, dai, not_a_token, BUY_ETH_ADDRESS])
            .await;
        assert_eq!(
            buffers_with_eth.get(&weth).unwrap().as_ref().unwrap(),
            buffers_without_eth.get(&weth).unwrap().as_ref().unwrap()
        );
        assert_eq!(
            buffers_with_eth.get(&dai).unwrap().as_ref().unwrap(),
            buffers_without_eth.get(&dai).unwrap().as_ref().unwrap()
        );
        assert!(buffers_with_eth.get(&not_a_token).unwrap().is_err());
        assert!(buffers_without_eth.get(&not_a_token).unwrap().is_err());
    }
}
