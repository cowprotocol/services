//! Winner selection over the chain-generic arbitrator.

use {
    crate::{
        domain::{
            auction::Auction,
            cycle::{Ranking, SolanaCycle, Solution},
        },
        run_loop::WinnerSelection,
    },
    chain_types::{
        ChainTypes,
        solana::{Pubkey, Solana},
    },
    winner_selection::{Arbitrator, AuctionContext},
};

/// Runs the generic fair-combinatorial arbitrator over the drivers'
/// solutions.
pub struct SolanaArbitrator {
    inner: Arbitrator<Solana>,
}

impl SolanaArbitrator {
    pub fn new(max_winners: usize, wrapped_native: Pubkey) -> Self {
        Self {
            inner: Arbitrator {
                max_winners,
                wrapped_native,
            },
        }
    }
}

impl WinnerSelection<SolanaCycle> for SolanaArbitrator {
    fn arbitrate(&self, solutions: Vec<Solution>, auction: &Auction) -> Ranking {
        let drivers = solutions
            .iter()
            .map(|solution| {
                (
                    (solution.inner.solver(), solution.inner.id()),
                    solution.driver_index,
                )
            })
            .collect();

        // An empty fee-policy list marks an order as part of the auction, the
        // arbitrator only scores such orders. The auction carries no protocol
        // fees.
        // TODO: real fee policies arrive with the protocol-fee support
        // (post-MVP).
        let fee_policies = auction
            .orders
            .iter()
            .map(|order| (order.uid, Vec::new()))
            .collect();
        // Every auction token is priced at the native denominator, i.e. 1:1
        // to lamports, so scores compare raw surplus. Ranking within one
        // token pair is exact, comparisons across pairs are not.
        // TODO: replace with native price estimation.
        let native_prices = auction
            .orders
            .iter()
            .flat_map(|order| [order.sell_token, order.buy_token])
            .map(|token| (token, Solana::NATIVE_PRICE_DENOMINATOR))
            .collect();
        let context = AuctionContext::<Solana> {
            fee_policies,
            surplus_capturing_jit_order_owners: Default::default(),
            native_prices,
        };

        let inner = self.inner.arbitrate(
            solutions
                .into_iter()
                .map(|solution| solution.inner)
                .collect(),
            &context,
        );
        Ranking { inner, drivers }
    }
}
