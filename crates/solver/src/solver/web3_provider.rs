use ethcontract::web3::{Web3, Transport};

#[derive(Debug)]
pub struct Web3Provider<T: Transport> {
    web3: Web3<T>,
}

impl<T: Transport> Web3Provider<T> {
    pub fn new(web3: Web3<T>) -> Self {
        Self { web3 }
    }

    pub fn web3(&self) -> &Web3<T> {
        &self.web3
    }
} 