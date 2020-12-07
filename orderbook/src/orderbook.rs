use model::{Order, OrderCreation, OrderMetaData, OrderUid};
use primitive_types::{H160, H256};
use tokio::sync::RwLock;

#[derive(Debug, Eq, PartialEq)]
pub enum AddOrderError {
    AlreadyExists,
    InvalidSignature,
    #[allow(dead_code)]
    PastNonce,
    #[allow(dead_code)]
    PastValidTo,
}

#[derive(Debug)]
pub enum RemoveOrderError {
    DoesNotExist,
}

#[derive(Debug, Default)]
pub struct OrderBook {
    // TODO: Store more efficiently (for example HashMap) depending on functionality we need.
    pub orders: RwLock<Vec<Order>>,
}

impl OrderBook {
    pub async fn add_order(&self, order: OrderCreation) -> Result<(), AddOrderError> {
        // TODO: Check order signature, nonce, valid_to.
        let mut orders = self.orders.write().await;
        if orders.iter().any(|x| x.order_creation == order) {
            return Err(AddOrderError::AlreadyExists);
        }
        let order = user_order_to_full_order(order).map_err(|_| AddOrderError::InvalidSignature)?;
        orders.push(order);
        Ok(())
    }

    pub async fn get_orders(&self) -> Vec<Order> {
        self.orders.read().await.clone()
    }

    #[allow(dead_code)]
    pub async fn remove_order(&self, order: &OrderCreation) -> Result<(), RemoveOrderError> {
        let mut orders = self.orders.write().await;
        if let Some(index) = orders.iter().position(|x| x.order_creation == *order) {
            orders.swap_remove(index);
            Ok(())
        } else {
            Err(RemoveOrderError::DoesNotExist)
        }
    }
}

struct InvalidSignatureError {}
fn user_order_to_full_order(user_order: OrderCreation) -> Result<Order, InvalidSignatureError> {
    // TODO: verify signature and extract owner, get orderDigest, and do proper error handling
    let owner = H160::zero();
    let digest = H256::zero();
    let mut uid = OrderUid([0u8; 56]);
    uid.0[0..32].copy_from_slice(digest.as_fixed_bytes());
    uid.0[32..52].copy_from_slice(owner.as_fixed_bytes());
    uid.0[52..56].copy_from_slice(&user_order.valid_to.to_be_bytes());

    Ok(Order {
        order_meta_data: OrderMetaData {
            creation_date: chrono::offset::Utc::now(),
            owner,
            uid,
        },
        order_creation: user_order,
    })
}

#[cfg(test)]
pub mod test_util {
    use super::*;

    #[tokio::test]
    async fn cannot_add_order_twice() {
        let orderbook = OrderBook::default();
        let order = OrderCreation::default();
        orderbook.add_order(order).await.unwrap();
        assert_eq!(orderbook.get_orders().await.len(), 1);
        assert_eq!(
            orderbook.add_order(order).await,
            Err(AddOrderError::AlreadyExists)
        );
    }

    #[tokio::test]
    async fn test_simple_removing_order() {
        let orderbook = OrderBook::default();
        let order = OrderCreation::default();
        orderbook.add_order(order).await.unwrap();
        assert_eq!(orderbook.get_orders().await.len(), 1);
        orderbook.remove_order(&order).await.unwrap();
        assert_eq!(orderbook.get_orders().await.len(), 0);
    }
}
