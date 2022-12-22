pub mod auction;
pub mod solution;

pub use {
    auction::Auction,
    solution::{solve, Score, Solution},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

/// UID of an order.
#[derive(Debug, Clone, Copy)]
pub struct OrderUid(pub [u8; 56]);

impl From<[u8; 56]> for OrderUid {
    fn from(inner: [u8; 56]) -> Self {
        Self(inner)
    }
}

impl From<OrderUid> for [u8; 56] {
    fn from(uid: OrderUid) -> Self {
        uid.0
    }
}

/// This is a hash allowing arbitrary user data to be associated with an order.
/// While this type holds the hash, the data itself is uploaded to IPFS. This
/// hash is signed along with the order.
#[derive(Debug, Clone, Copy)]
pub struct AppData(pub [u8; 32]);

impl From<[u8; 32]> for AppData {
    fn from(inner: [u8; 32]) -> Self {
        Self(inner)
    }
}

impl From<AppData> for [u8; 32] {
    fn from(app_data: AppData) -> Self {
        app_data.0
    }
}
