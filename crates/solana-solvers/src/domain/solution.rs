//! Solution assembly: one quoted swap becomes one single-order solution in the
//! driver's `/solve` DTO.

use {
    super::order::OrderUid,
    crate::dex,
    base64::prelude::*,
    serde::Serialize,
    serde_with::serde_as,
    solana_sdk::{instruction::Instruction as SolInstruction, pubkey::Pubkey},
};

/// A solution in the driver's `/solve` DTO. Trades fulfill auction orders, with
/// no JIT.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Solution {
    pub id: u64,
    pub trades: Vec<Trade>,
    pub interactions: Vec<Instruction>,
    /// Optional solver estimate of total settlement compute units.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cu_estimate: Option<u64>,
    /// The address lookup tables the interactions assume, carried through so
    /// the driver can build the v0 transaction around them.
    #[serde_as(as = "Vec<serde_with::DisplayFromStr>")]
    pub address_lookup_tables: Vec<Pubkey>,
}

/// A fulfillment of one auction order.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trade {
    /// The order's 32-byte intent hash.
    pub order_uid: OrderUid,
    /// Sell-token units for sell orders, buy-token units for buy orders.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub executed_amount: u64,
    /// Fee in sell-token units.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub fee: u64,
}

/// A Solana instruction the solver supplies, carried verbatim.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Instruction {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub program_id: Pubkey,
    pub accounts: Vec<AccountMeta>,
    /// Base64-encoded instruction data.
    #[serde(serialize_with = "serialize_base64")]
    pub instruction_data: Vec<u8>,
}

/// Account meta in the driver DTO shape.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountMeta {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub pubkey: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum Error {
    /// Sell and buy mint coincide, which is not a real trade.
    #[error("sell and buy mint are the same")]
    SameMint,
    /// A zero quoted amount means the swap fills nothing.
    #[error("quoted amount is zero")]
    ZeroAmount,
}

impl Solution {
    /// Wraps one quoted swap into a single-order solution.
    ///
    /// The swap's instructions are carried verbatim as interactions, and
    /// its address lookup tables travel along so the driver can build the
    /// v0 transaction the instructions assume.
    pub fn single(
        id: u64,
        order_uid: OrderUid,
        order: &dex::Order,
        swap: dex::Swap,
    ) -> Result<Self, Error> {
        if order.sell_mint == order.buy_mint {
            return Err(Error::SameMint);
        }
        if swap.in_amount == 0 || swap.out_amount == 0 {
            return Err(Error::ZeroAmount);
        }
        let executed_amount = match order.side {
            dex::Side::Sell => swap.in_amount,
            dex::Side::Buy => swap.out_amount,
        };
        Ok(Self {
            id,
            trades: vec![Trade {
                order_uid,
                executed_amount,
                fee: 0,
            }],
            interactions: swap
                .instructions
                .into_iter()
                .map(Instruction::from_sdk)
                .collect(),
            cu_estimate: None,
            address_lookup_tables: swap.address_lookup_tables,
        })
    }
}

impl Instruction {
    fn from_sdk(instruction: SolInstruction) -> Self {
        Self {
            program_id: instruction.program_id,
            accounts: instruction
                .accounts
                .into_iter()
                .map(|meta| AccountMeta {
                    pubkey: meta.pubkey,
                    is_signer: meta.is_signer,
                    is_writable: meta.is_writable,
                })
                .collect(),
            instruction_data: instruction.data,
        }
    }
}

fn serialize_base64<S: serde::Serializer>(data: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&BASE64_STANDARD.encode(data))
}

#[cfg(test)]
mod tests {
    use {super::*, solana_sdk::instruction::AccountMeta as SdkAccountMeta, std::str::FromStr};

    const ORDER_UID: OrderUid = OrderUid([8; 32]);

    fn pubkey(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn order(side: dex::Side) -> dex::Order {
        dex::Order {
            sell_mint: pubkey(1),
            buy_mint: pubkey(2),
            buy_destination: pubkey(3),
            amount: 1_000,
            side,
        }
    }

    fn swap() -> dex::Swap {
        dex::Swap {
            in_amount: 1_000,
            out_amount: 2_000,
            instructions: vec![SolInstruction {
                program_id: pubkey(9),
                accounts: vec![SdkAccountMeta {
                    pubkey: pubkey(4),
                    is_signer: true,
                    is_writable: false,
                }],
                data: vec![0xde, 0xad],
            }],
            address_lookup_tables: vec![pubkey(7)],
        }
    }

    #[test]
    fn sell_swap_maps_to_single_order_solution() {
        let order = order(dex::Side::Sell);
        let solution = Solution::single(42, ORDER_UID, &order, swap()).unwrap();

        assert_eq!(solution.id, 42);
        assert_eq!(solution.trades.len(), 1);
        assert_eq!(solution.trades[0].order_uid, ORDER_UID);
        assert_eq!(solution.trades[0].executed_amount, 1_000);
        assert_eq!(solution.trades[0].fee, 0);
        assert_eq!(solution.address_lookup_tables, vec![pubkey(7)]);

        // The instruction is carried verbatim, flags included.
        let interaction = &solution.interactions[0];
        assert_eq!(interaction.program_id, pubkey(9));
        assert_eq!(interaction.accounts[0].pubkey, pubkey(4));
        assert!(interaction.accounts[0].is_signer);
        assert!(!interaction.accounts[0].is_writable);
        assert_eq!(interaction.instruction_data, vec![0xde, 0xad]);
    }

    #[test]
    fn buy_swap_executes_in_buy_token_units() {
        let solution = Solution::single(0, ORDER_UID, &order(dex::Side::Buy), swap()).unwrap();
        assert_eq!(solution.trades[0].executed_amount, 2_000);
    }

    #[test]
    fn same_mint_order_is_rejected() {
        let mut order = order(dex::Side::Sell);
        order.buy_mint = order.sell_mint;
        assert_eq!(
            Solution::single(0, ORDER_UID, &order, swap()).unwrap_err(),
            Error::SameMint
        );
    }

    #[test]
    fn zero_quoted_amount_is_rejected() {
        let mut swap = swap();
        swap.out_amount = 0;
        assert_eq!(
            Solution::single(0, ORDER_UID, &order(dex::Side::Sell), swap).unwrap_err(),
            Error::ZeroAmount
        );
    }

    #[test]
    fn wire_format_is_stable() {
        let solution = Solution::single(1, ORDER_UID, &order(dex::Side::Sell), swap()).unwrap();
        let json = serde_json::to_value(&solution).unwrap();

        assert_eq!(
            json,
            serde_json::json!({
                "id": 1,
                "trades": [{
                    "orderUid": format!("0x{}", "08".repeat(32)),
                    "executedAmount": "1000",
                    "fee": "0",
                }],
                "interactions": [{
                    "programId": pubkey(9).to_string(),
                    "accounts": [{
                        "pubkey": pubkey(4).to_string(),
                        "isSigner": true,
                        "isWritable": false,
                    }],
                    "instructionData": BASE64_STANDARD.encode([0xde, 0xad]),
                }],
                "addressLookupTables": [pubkey(7).to_string()],
            })
        );
    }

    #[test]
    fn from_str_roundtrip_for_wire_keys() {
        // Pubkeys serialize as base58 and parse back.
        let key = pubkey(5);
        assert_eq!(Pubkey::from_str(&key.to_string()).unwrap(), key);
    }
}
