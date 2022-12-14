use crate::logic::{competition::solution, eth};

#[derive(Debug)]
pub struct Tenderly;

impl Tenderly {
    pub async fn access_list(&self, _settlement: &solution::Settlement) -> eth::AccessList {
        todo!()
    }

    pub async fn gas(
        &self,
        _settlement: &solution::Settlement,
        _access_list: &eth::AccessList,
    ) -> eth::Gas {
        todo!()
    }
}
