use crate::logic::eth;

#[derive(Debug)]
pub(super) struct Ethereum(crate::Ethereum);

impl Ethereum {
    pub fn new(eth: crate::Ethereum) -> Self {
        Self(eth)
    }

    pub async fn simulate(&self, _tx: &eth::Tx) -> super::Simulation {
        todo!()
    }
}
