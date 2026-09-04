//! Inbound `/solve` response: the solutions a solver engine returns.
//!
//! The wire format matches `solana-solvers/src/dto/solution.rs`.

use {
    crate::{
        domain,
        domain::{Side, order_uid::OrderUid},
        infra::solver::dto::auction::{Auction, Order},
    },
    serde::Deserialize,
    serde_with::serde_as,
    solana_sdk::{
        instruction::{AccountMeta as SdkAccountMeta, Instruction as SdkInstruction},
        pubkey::Pubkey,
    },
    std::{collections::HashMap, num::NonZero},
};

/// The solutions one engine returned for one auction. This wrapper owns the
/// conversion into domain solutions.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Solutions {
    solutions: Vec<Solution>,
}

/// A solution in the driver's `/solve` DTO.
#[serde_as]
#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Solution {
    pub id: u64,
    /// Uniform clearing prices by mint: the sell mint maps to the amount
    /// bought and the buy mint to the amount sold, so for every trade
    /// `executed_sell * price_sell == executed_buy * price_buy`. The engine
    /// currently only produces single-order solutions, so the pair is the
    /// executed swap's ratio.
    #[serde_as(as = "HashMap<serde_with::DisplayFromStr, serde_with::DisplayFromStr>")]
    pub prices: HashMap<Pubkey, NonZero<u64>>,
    pub trades: Vec<Trade>,
    pub interactions: Vec<Instruction>,
    /// Optional solver estimate of total settlement compute units.
    #[serde(default)]
    pub cu_estimate: Option<u32>,
    /// The address lookup tables the interactions assume.
    #[serde(default)]
    #[serde_as(as = "Vec<serde_with::DisplayFromStr>")]
    pub address_lookup_tables: Vec<Pubkey>,
}

/// A fulfillment of one auction order.
#[serde_as]
#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trade {
    /// The order's 32-byte intent hash.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub order_uid: OrderUid,
    /// Sell-token units for sell orders, buy-token units for buy orders.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub executed_amount: u64,
}

impl Trade {
    /// Convert this wire trade into a domain trade, deriving the counterpart
    /// leg from the clearing prices.
    fn into_domain(
        self,
        order: &Order,
        price_sell: NonZero<u64>,
        price_buy: NonZero<u64>,
    ) -> Result<domain::Trade, Error> {
        if self.executed_amount > order.amount {
            return Err(Error::ExecutedAmountExceedsOrderAmount(
                self.order_uid,
                self.executed_amount,
                order.amount,
            ));
        }

        // The engine reports one executed amount, on the order's own side, plus
        // uniform clearing prices per mint. Derive the counterpart leg from the
        // prices so the domain trade carries a real amount on both sides.
        let counterpart = match order.side {
            Side::Sell => Self::counterpart(self.executed_amount, price_sell, price_buy),
            Side::Buy => Self::counterpart(self.executed_amount, price_buy, price_sell),
        }
        .ok_or(Error::InvalidClearingPrice(self.order_uid))?;

        let (executed_sell, executed_buy) = match order.side {
            Side::Sell => (self.executed_amount, counterpart),
            Side::Buy => (counterpart, self.executed_amount),
        };

        Ok(domain::Trade {
            order_uid: self.order_uid,
            executed_sell,
            executed_buy,
        })
    }

    /// Derive the counterpart leg from the executed amount and the two clearing
    /// prices. Both prices are `NonZero`, and the u128 product and quotient
    /// always fit, so only the shrink back to u64 can fail.
    fn counterpart(
        executed: u64,
        price_own: NonZero<u64>,
        price_other: NonZero<u64>,
    ) -> Option<u64> {
        u64::try_from((executed as u128) * (price_own.get() as u128) / (price_other.get() as u128))
            .ok()
    }
}

/// A Solana instruction the solver supplies, carried verbatim.
#[serde_as]
#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instruction {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub program_id: Pubkey,
    pub accounts: Vec<AccountMeta>,
    #[serde_as(as = "serde_with::base64::Base64")]
    pub instruction_data: Vec<u8>,
}

/// Account meta in the driver DTO shape.
#[serde_as]
#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountMeta {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub pubkey: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
}

impl From<AccountMeta> for SdkAccountMeta {
    fn from(value: AccountMeta) -> Self {
        Self {
            pubkey: value.pubkey,
            is_signer: value.is_signer,
            is_writable: value.is_writable,
        }
    }
}

impl From<Instruction> for SdkInstruction {
    fn from(value: Instruction) -> Self {
        Self {
            program_id: value.program_id,
            accounts: value.accounts.into_iter().map(Into::into).collect(),
            data: value.instruction_data,
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum Error {
    /// A trade references an order that was not in the sent auction.
    #[error("trade references unknown order UID {0}")]
    UnknownOrderUid(OrderUid),
    /// A trade executes more than the order amount.
    #[error("trade {0} executes {1} but order amount is {2}")]
    ExecutedAmountExceedsOrderAmount(OrderUid, u64, u64),
    /// The engine did not report a clearing price for a mint a trade touches.
    #[error("trade {0} has no clearing price for mint {1}")]
    MissingClearingPrice(OrderUid, Pubkey),
    /// The clearing prices do not yield a valid counterpart amount.
    #[error("trade {0} has invalid clearing prices")]
    InvalidClearingPrice(OrderUid),
}

impl Solutions {
    /// Convert the wire solutions into domain solutions.
    ///
    /// Each trade must reference an order from the auction the driver sent.
    /// Any trade referencing an unknown order UID rejects the entire engine
    /// response.
    pub fn into_domain(
        self,
        auction: &Auction,
        solver: Pubkey,
    ) -> Result<Vec<domain::Solution>, Error> {
        let allowed_orders: HashMap<OrderUid, &Order> =
            auction.orders.iter().map(|o| (o.uid, o)).collect();

        self.solutions
            .into_iter()
            .map(|solution| {
                let Solution {
                    id,
                    prices,
                    trades,
                    interactions,
                    cu_estimate,
                    address_lookup_tables,
                } = solution;
                let trades = trades
                    .into_iter()
                    .map(|trade| {
                        let order = allowed_orders
                            .get(&trade.order_uid)
                            .copied()
                            .ok_or(Error::UnknownOrderUid(trade.order_uid))?;
                        let price_sell = prices.get(&order.sell_mint).copied().ok_or(
                            Error::MissingClearingPrice(trade.order_uid, order.sell_mint),
                        )?;
                        let price_buy = prices
                            .get(&order.buy_mint)
                            .copied()
                            .ok_or(Error::MissingClearingPrice(trade.order_uid, order.buy_mint))?;
                        trade.into_domain(order, price_sell, price_buy)
                    })
                    .collect::<Result<_, _>>()?;

                Ok(domain::Solution {
                    id,
                    solver,
                    prices,
                    trades,
                    interactions: interactions.into_iter().map(Into::into).collect(),
                    address_lookup_tables,
                    cu_estimate,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{domain::Side, infra::blockchain::associated_token_address},
        serde_json::json,
        solana_sdk::pubkey::Pubkey,
    };

    fn pubkey(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn nz(value: u64) -> NonZero<u64> {
        NonZero::new(value).unwrap()
    }

    fn sample_auction_dto() -> Auction {
        Auction {
            id: 1,
            taker: pubkey(3),
            orders: vec![super::super::auction::Order {
                uid: OrderUid([8; 32]),
                sell_mint: pubkey(1),
                buy_mint: pubkey(2),
                buy_destination: associated_token_address(&pubkey(3), &pubkey(2)),
                amount: 1_000,
                side: Side::Sell,
            }],
            deadline: chrono::Utc::now() + chrono::Duration::seconds(60),
        }
    }

    #[test]
    fn rejects_executed_amount_exceeding_order() {
        let bad = json!({
            "solutions": [{
                "id": 1,
                "prices": {
                    (pubkey(1).to_string()): "2000",
                    (pubkey(2).to_string()): "1000",
                },
                "trades": [{
                    "orderUid": format!("0x{}", "08".repeat(32)),
                    "executedAmount": "1001",
                }],
                "interactions": [],
                "addressLookupTables": [],
            }],
        });
        let solutions: Solutions = serde_json::from_value(bad).unwrap();
        let err = solutions
            .into_domain(&sample_auction_dto(), pubkey(6))
            .unwrap_err();
        assert_eq!(
            err,
            Error::ExecutedAmountExceedsOrderAmount(OrderUid([8; 32]), 1001, 1000)
        );
    }

    #[test]
    fn rejects_unknown_order_uid() {
        let bad = json!({
            "solutions": [{
                "id": 1,
                "prices": {
                    (pubkey(1).to_string()): "2000",
                    (pubkey(2).to_string()): "1000",
                },
                "trades": [{
                    "orderUid": format!("0x{}", "ff".repeat(32)),
                    "executedAmount": "1000",
                }],
                "interactions": [],
                "addressLookupTables": [],
            }],
        });
        let solutions: Solutions = serde_json::from_value(bad).unwrap();
        let err = solutions
            .into_domain(&sample_auction_dto(), pubkey(6))
            .unwrap_err();
        assert_eq!(err, Error::UnknownOrderUid(OrderUid([0xff; 32])));
    }

    /// A sell order fills `executed_sell` and derives `executed_buy` from the
    /// clearing prices. A buy order does the reverse.
    #[test]
    fn derives_both_legs_from_clearing_prices() {
        let solutions = Solutions {
            solutions: vec![Solution {
                id: 1,
                prices: HashMap::from([(pubkey(1), nz(2_000)), (pubkey(2), nz(1_000))]),
                trades: vec![Trade {
                    order_uid: OrderUid([8; 32]),
                    executed_amount: 1_000,
                }],
                interactions: vec![],
                cu_estimate: None,
                address_lookup_tables: vec![],
            }],
        };
        let domain = solutions
            .into_domain(&sample_auction_dto(), pubkey(6))
            .unwrap();
        let trade = &domain[0].trades[0];
        // Sell order: 1000 sold, 1000 * 2000 / 1000 = 2000 bought.
        assert_eq!(trade.executed_sell, 1_000);
        assert_eq!(trade.executed_buy, 2_000);
    }

    /// A trade whose mint has no clearing price rejects the whole response.
    #[test]
    fn rejects_missing_clearing_price() {
        let solutions = Solutions {
            solutions: vec![Solution {
                id: 1,
                prices: HashMap::from([(pubkey(1), nz(2_000))]),
                trades: vec![Trade {
                    order_uid: OrderUid([8; 32]),
                    executed_amount: 1_000,
                }],
                interactions: vec![],
                cu_estimate: None,
                address_lookup_tables: vec![],
            }],
        };
        let err = solutions
            .into_domain(&sample_auction_dto(), pubkey(6))
            .unwrap_err();
        assert_eq!(
            err,
            Error::MissingClearingPrice(OrderUid([8; 32]), pubkey(2))
        );
    }

    /// The intermediate product of two u64s can overflow u64 even when the
    /// final result fits back in u64; the u128 math must still succeed.
    #[test]
    fn derives_counterpart_that_overflows_u64_product() {
        let auction = Auction {
            id: 1,
            taker: pubkey(3),
            orders: vec![Order {
                uid: OrderUid([8; 32]),
                sell_mint: pubkey(1),
                buy_mint: pubkey(2),
                buy_destination: associated_token_address(&pubkey(3), &pubkey(2)),
                amount: u64::MAX,
                side: Side::Sell,
            }],
            deadline: chrono::Utc::now() + chrono::Duration::seconds(60),
        };
        let solutions = Solutions {
            solutions: vec![Solution {
                id: 1,
                // price_sell = 2, price_buy = 2: u64::MAX * 2 overflows u64 but
                // fits in u128, and / 2 lands back on u64::MAX.
                prices: HashMap::from([(pubkey(1), nz(2)), (pubkey(2), nz(2))]),
                trades: vec![Trade {
                    order_uid: OrderUid([8; 32]),
                    executed_amount: u64::MAX,
                }],
                interactions: vec![],
                cu_estimate: None,
                address_lookup_tables: vec![],
            }],
        };
        let domain = solutions.into_domain(&auction, pubkey(6)).unwrap();
        let trade = &domain[0].trades[0];
        assert_eq!(trade.executed_sell, u64::MAX);
        assert_eq!(trade.executed_buy, u64::MAX);
    }

    /// A zero clearing price is invalid: it would settle a leg against
    /// nothing, so the whole response fails to parse.
    #[test]
    fn rejects_zero_clearing_price() {
        let bad = json!({
            "solutions": [{
                "id": 1,
                "prices": {
                    (pubkey(1).to_string()): "0",
                    (pubkey(2).to_string()): "1000",
                },
                "trades": [{
                    "orderUid": format!("0x{}", "08".repeat(32)),
                    "executedAmount": "1000",
                }],
                "interactions": [],
            }],
        });
        assert!(
            serde_json::from_value::<Solutions>(bad).is_err(),
            "a zero clearing price must be rejected at parse time"
        );
    }

    /// Pins the inbound `/solve` response shape against the literal the
    /// `solana-solvers` `Solution` serializes.
    #[test]
    fn wire_format_is_stable() {
        let json = json!({
            "solutions": [{
                "id": 1,
                "prices": {
                    (pubkey(1).to_string()): "2000",
                    (pubkey(2).to_string()): "1000",
                },
                "trades": [{
                    "orderUid": format!("0x{}", "08".repeat(32)),
                    "executedAmount": "1000",
                }],
                "interactions": [{
                    "programId": pubkey(9).to_string(),
                    "accounts": [{
                        "pubkey": pubkey(4).to_string(),
                        "isSigner": true,
                        "isWritable": false,
                    }],
                    "instructionData": "3q0=",
                }],
                "addressLookupTables": [pubkey(7).to_string()],
            }]
        });

        let expected = Solutions {
            solutions: vec![Solution {
                id: 1,
                prices: HashMap::from([(pubkey(1), nz(2_000)), (pubkey(2), nz(1_000))]),
                trades: vec![Trade {
                    order_uid: OrderUid([8; 32]),
                    executed_amount: 1_000,
                }],
                interactions: vec![Instruction {
                    program_id: pubkey(9),
                    accounts: vec![AccountMeta {
                        pubkey: pubkey(4),
                        is_signer: true,
                        is_writable: false,
                    }],
                    instruction_data: vec![0xde, 0xad],
                }],
                cu_estimate: None,
                address_lookup_tables: vec![pubkey(7)],
            }],
        };

        let actual: Solutions = serde_json::from_value(json).unwrap();
        assert_eq!(actual, expected);
    }
}
