use model::{Order, OrderCreation, OrderMetaData};
use tokio::sync::RwLock;

#[derive(Debug, Eq, PartialEq)]
pub enum AddOrderError {
    DuplicatedOrder,
    InvalidSignature,
    #[allow(dead_code)]
    Forbidden,
    #[allow(dead_code)]
    MissingOrderData,
    #[allow(dead_code)]
    PastValidTo,
    #[allow(dead_code)]
    InsufficientFunds,
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
            return Err(AddOrderError::DuplicatedOrder);
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
    Ok(Order {
        order_meta_data: OrderMetaData {
            creation_date: chrono::offset::Utc::now(),
            owner: user_order.order_owner(),
            uid: user_order.order_uid(),
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
            Err(AddOrderError::DuplicatedOrder)
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
