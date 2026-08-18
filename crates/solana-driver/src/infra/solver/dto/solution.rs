//! Inbound `/solve` response: the solutions a solver engine returns.
//!
//! The wire format matches `solana-solvers/src/dto/solution.rs`.

use {
    crate::{domain, domain::order_uid::OrderUid, infra::solver::dto::auction::Auction},
    serde::Deserialize,
    serde_with::serde_as,
    solana_sdk::{
        instruction::{AccountMeta as SdkAccountMeta, Instruction},
        pubkey::Pubkey,
    },
    std::collections::HashSet,
};

/// The solutions one engine returned for one auction. This wrapper owns the
/// conversion into domain solutions.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Solutions {
    solutions: Vec<Solution>,
}

/// A solution in the driver's `/solve` DTO.
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Solution {
    pub id: u64,
    pub trades: Vec<Trade>,
    pub interactions: Vec<InstructionDto>,
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
#[derive(Debug, Deserialize)]
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
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstructionDto {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub program_id: Pubkey,
    pub accounts: Vec<AccountMetaDto>,
    #[serde_as(as = "serde_with::base64::Base64")]
    pub instruction_data: Vec<u8>,
}

/// Account meta in the driver DTO shape.
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountMetaDto {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub pubkey: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
}

impl From<AccountMetaDto> for SdkAccountMeta {
    fn from(value: AccountMetaDto) -> Self {
        Self {
            pubkey: value.pubkey,
            is_signer: value.is_signer,
            is_writable: value.is_writable,
        }
    }
}

impl From<InstructionDto> for Instruction {
    fn from(value: InstructionDto) -> Self {
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
        let allowed_uids: HashSet<OrderUid> = auction.orders.iter().map(|o| o.uid).collect();

        self.solutions
            .into_iter()
            .map(|solution| {
                let trades = solution
                    .trades
                    .into_iter()
                    .map(|trade| {
                        if !allowed_uids.contains(&trade.order_uid) {
                            return Err(Error::UnknownOrderUid(trade.order_uid));
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
        }
    }

    #[test]
    fn rejects_unknown_order_uid() {
        let bad = json!({
            "solutions": [{
                "id": 1,
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
}
