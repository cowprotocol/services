use crate::logic::eth;

#[derive(Debug)]
pub(super) struct Ethereum;

impl Ethereum {
    pub async fn simulate(&self, _tx: &eth::Tx, _access_list: &eth::AccessList) -> eth::Simulation {
        todo!()
    }
}
