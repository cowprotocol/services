//! Inbound `/solve` response: the solutions a solver engine returns.
//!
//! The wire format matches `solana-solvers/src/dto/solution.rs`.

use {
    crate::{domain, domain::order_uid::OrderUid, infra::solver::dto::auction::Auction},
    serde::Deserialize,
    serde_with::serde_as,
    solana_sdk::{
        instruction::{AccountMeta as SdkAccountMeta, Instruction as SdkInstruction},
        pubkey::Pubkey,
    },
    std::collections::HashMap,
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
    pub prices: HashMap<Pubkey, u64>,
    pub trades: Vec<Trade>,
    pub interactions: Vec<Instruction>,
    /// Optional solver estimate of total settlement compute units.
    #[serde(default)]
    pub cu_estimate: Option<u64>,
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
        let allowed_orders: HashMap<OrderUid, u64> =
            auction.orders.iter().map(|o| (o.uid, o.amount)).collect();

        self.solutions
            .into_iter()
            .map(|solution| {
                let trades = solution
                    .trades
                    .into_iter()
                    .map(|trade| {
                        let amount = match allowed_orders.get(&trade.order_uid) {
                            Some(&amount) => amount,
                            None => return Err(Error::UnknownOrderUid(trade.order_uid)),
                        };
                        if trade.executed_amount > amount {
                            return Err(Error::ExecutedAmountExceedsOrderAmount(
                                trade.order_uid,
                                trade.executed_amount,
                                amount,
                            ));
                        }
                        Ok(domain::Trade {
                            order_uid: trade.order_uid,
                            executed_amount: trade.executed_amount,
                        })
                    })
                    .collect::<Result<_, _>>()?;

                Ok(domain::Solution {
                    id: solution.id,
                    solver,
                    prices: solution.prices,
                    trades,
                    interactions: solution.interactions.into_iter().map(Into::into).collect(),
                    address_lookup_tables: solution.address_lookup_tables,
                    cu_estimate: solution.cu_estimate,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{domain::Side, util},
        serde_json::json,
        solana_sdk::pubkey::Pubkey,
    };

    fn pubkey(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn sample_auction_dto() -> Auction {
        Auction {
            id: 1,
            taker: pubkey(3),
            orders: vec![super::super::auction::Order {
                uid: OrderUid([8; 32]),
                sell_mint: pubkey(1),
                buy_mint: pubkey(2),
                buy_destination: util::associated_token_address(&pubkey(3), &pubkey(2)),
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
                prices: HashMap::from([(pubkey(1), 2_000), (pubkey(2), 1_000)]),
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
