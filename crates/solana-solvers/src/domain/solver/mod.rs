//! Solve loop: quote each auction order and assemble single-order solutions.

use {
    crate::{
        dex::{self, Dex},
        dto::{auction::Auction, solution::Solution},
    },
    futures::future::join_all,
    solana_sdk::pubkey::Pubkey,
    std::future::Future,
};

/// Quotes one order into a swap. A seam over [`Dex`] so the loop is testable
/// without the network.
pub trait Quote {
    fn quote(
        &self,
        order: &dex::Order,
        taker: &Pubkey,
    ) -> impl Future<Output = Result<dex::Swap, dex::jupiter::Error>> + Send;
}

impl Quote for Dex {
    fn quote(
        &self,
        order: &dex::Order,
        taker: &Pubkey,
    ) -> impl Future<Output = Result<dex::Swap, dex::jupiter::Error>> + Send {
        self.swap(order, taker)
    }
}

/// Quote every order concurrently and return one single-order solution per
/// routable order. Buys (when disabled) and orders the aggregator cannot route
/// yield no candidate, the rest of the auction still proceeds.
///
/// Order counts are small (bounded by the settlement account budget), so every
/// order is quoted at once.
pub async fn solve<Q: Quote>(quoter: &Q, auction: &Auction) -> Vec<Solution> {
    let candidates = auction.orders.iter().enumerate().map(|(index, order)| {
        let dex_order = order.to_dex_order();
        async move {
            let swap = quoter.quote(&dex_order, &auction.taker).await.ok()?;
            Solution::new(index as u64, order.uid, &dex_order, swap).ok()
        }
    });
    join_all(candidates).await.into_iter().flatten().collect()
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::dto::{auction, order::OrderUid},
    };

    fn pubkey(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn order(uid: u8, side: dex::Side, sell_mint: Pubkey) -> auction::Order {
        auction::Order {
            uid: OrderUid([uid; 32]),
            sell_mint,
            buy_mint: pubkey(2),
            buy_destination: pubkey(3),
            amount: 1_000,
            side,
        }
    }

    /// Routes any sell except the `0xff` sell mint, rejects buys.
    struct MockQuote;

    impl Quote for MockQuote {
        fn quote(
            &self,
            order: &dex::Order,
            _taker: &Pubkey,
        ) -> impl Future<Output = Result<dex::Swap, dex::jupiter::Error>> + Send {
            let result = match order.side {
                dex::Side::Buy => Err(dex::jupiter::Error::OrderNotSupported),
                dex::Side::Sell if order.sell_mint == pubkey(0xff) => {
                    Err(dex::jupiter::Error::NotFound)
                }
                dex::Side::Sell => Ok(dex::Swap {
                    in_amount: 1_000,
                    out_amount: 2_000,
                    instructions: vec![],
                    address_lookup_tables: vec![],
                }),
            };
            async move { result }
        }
    }

    #[tokio::test]
    async fn emits_one_solution_per_routable_order() {
        let auction = Auction {
            id: 1,
            taker: pubkey(1),
            orders: vec![
                order(0x01, dex::Side::Sell, pubkey(0x10)), // routable
                order(0x02, dex::Side::Sell, pubkey(0xff)), // no route
                order(0x03, dex::Side::Buy, pubkey(0x11)),  // buys disabled
            ],
        };

        let solutions = solve(&MockQuote, &auction).await;

        assert_eq!(solutions.len(), 1);
        assert_eq!(solutions[0].trades[0].order_uid, OrderUid([0x01; 32]));
    }

    #[tokio::test]
    async fn empty_auction_yields_no_solutions() {
        let auction = Auction {
            id: 1,
            taker: pubkey(1),
            orders: vec![],
        };
        assert!(solve(&MockQuote, &auction).await.is_empty());
    }
}
