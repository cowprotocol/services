//! Solve loop: quote each auction order and assemble single-order solutions.

use {
    super::{auction::Auction, solution::Solution},
    crate::dex::{self, Dex},
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
            Solution::single(index as u64, order.uid, &dex_order, swap).ok()
        }
    });
    join_all(candidates).await.into_iter().flatten().collect()
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            config::JupiterConfig,
            domain::{auction, order::OrderUid},
        },
        std::str::FromStr,
    };

    // USDC and wrapped SOL mints for the live test.
    const USDC: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
    const WSOL: &str = "So11111111111111111111111111111111111111112";

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

    /// Live Jupiter API. Needs network. Keyless works, set `JUPITER_API_KEY`
    /// for headroom.
    #[tokio::test]
    #[ignore]
    async fn jupiter_live_solve() {
        let dex = Dex::Jupiter(
            dex::jupiter::Jupiter::new(&JupiterConfig {
                endpoint: "https://api.jup.ag".parse().unwrap(),
                api_key: std::env::var("JUPITER_API_KEY").ok(),
                slippage_bps: 50,
                enable_buy_orders: false,
            })
            .unwrap(),
        );
        let auction = Auction {
            id: 1,
            taker: Pubkey::from_str(WSOL).unwrap(),
            orders: vec![auction::Order {
                uid: OrderUid([7; 32]),
                sell_mint: Pubkey::from_str(USDC).unwrap(),
                buy_mint: Pubkey::from_str(WSOL).unwrap(),
                buy_destination: Pubkey::from_str(WSOL).unwrap(),
                amount: 1_000_000,
                side: dex::Side::Sell,
            }],
        };

        let solutions = solve(&dex, &auction).await;

        assert_eq!(solutions.len(), 1);
        assert!(!solutions[0].interactions.is_empty());
    }
}
