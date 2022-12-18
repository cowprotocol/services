use crate::logic::eth;

#[derive(Debug)]
pub(super) struct Ethereum(crate::Ethereum);

impl Ethereum {
    pub async fn simulate(
        &self,
        _tx: &eth::Tx,
        _access_list: &eth::AccessList,
    ) -> super::Simulation {
        todo!()
    }
}
