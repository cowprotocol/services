use crate::logic::eth;

#[derive(Debug)]
pub struct Tenderly;

impl Tenderly {
    pub async fn access_list(&self, tx: &eth::Tx) -> eth::AccessList {
        todo!()
    }
}
