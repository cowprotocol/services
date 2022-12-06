use crate::logic::eth;

#[derive(Debug)]
pub struct Node;

impl Node {
    pub async fn access_list(&self, tx: &eth::Tx) -> eth::AccessList {
        todo!()
    }

    pub async fn gas(&self, tx: &eth::Tx, access_list: &eth::AccessList) -> eth::Gas {
        todo!()
    }
}
