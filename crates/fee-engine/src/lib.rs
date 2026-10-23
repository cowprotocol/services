//! Rule-based protocol fees.
//!
//! A rules file holds flat named lists, the signers of each partner and an
//! ordered list of rules. A rule is a name, a CEL `when` condition and a CEL
//! `fee` expression. For every order the rules run top to bottom and the first
//! `when` that is true picks the fee, which is then clamped to [`Caps`].
//!
//! ```toml
//! [lists]
//! stablecoins = ["0xa0b8…", "0xdac1…"]
//!
//! [[rule]]
//! name = "Stablecoin swaps"
//! when = 'order.sellToken in stablecoins && order.buyToken in stablecoins'
//! fee  = 'volume(10)'
//!
//! [[rule]]
//! name = "Default"
//! when = 'true'
//! fee  = 'surplus(50, 1)'
//! ```
//!
//! A rule sees `order` (the API order with the parsed app-data document in
//! place of its hash), `partner` (the app code once a registered signer
//! attested the order, else `null`), `now`, `size_usd` and every list.
//!
//! `README.md` documents every name a rule can use, with an example each.

mod engine;
mod fee;
mod file;
mod order_view;
mod partner;

pub use {
    engine::{Auction, Evaluation, Fact, Facts, Field, LoadError, Rule, RuleSet, Settings},
    fee::{Caps, Fee},
    partner::{PartnerAttestation, domain as attestation_domain},
};
