//! Solution assembly: one quoted swap becomes one single-order solution in the
//! driver's `/solve` DTO.
//!
//! The solver controls only the `interactions` array. Slippage is already
//! baked into the instruction data by the aggregator and the driver applies
//! none to `custom` interactions, so nothing is re-applied here. Compute
//! budget sizing is the driver's job (it derives the CU limit from
//! simulation), so the solution carries no compute-unit estimate and the
//! aggregator's compute-budget instructions are never included. The buy side
//! is funded by the swap output landing in the settlement's per-token buffer
//! (the adapter's `destination_token_account`), and `FinalizeSettle` pushes
//! each order's amount to the user, so the solver emits no transfer or credit
//! interaction.

use {
    crate::dex,
    base64::prelude::*,
    serde::Serialize,
    serde_with::serde_as,
    solana_sdk::{instruction::Instruction, pubkey::Pubkey},
    std::collections::HashMap,
};

/// A single-order solution in the driver DTO. Trades fulfill auction orders
/// only, with no JIT.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Solution {
    pub id: u64,
    /// Uniform clearing prices keyed by mint.
    #[serde_as(as = "HashMap<serde_with::DisplayFromStr, _>")]
    pub prices: HashMap<Pubkey, u64>,
    pub trades: Vec<Trade>,
    pub interactions: Vec<Interaction>,
    /// The address lookup tables the interactions assume, carried through so
    /// the driver can build the v0 transaction around them.
    #[serde_as(as = "Vec<serde_with::DisplayFromStr>")]
    pub address_lookup_tables: Vec<Pubkey>,
}

/// A fulfillment of one auction order.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trade {
    /// The order's 32-byte intent hash.
    #[serde(serialize_with = "serialize_hex")]
    pub order_uid: [u8; 32],
    /// Sell-token units for sell orders, buy-token units for buy orders.
    pub executed_amount: u64,
    /// Fee in sell-token units. Always zero at MVP: the solver prices the
    /// full quoted amounts into the clearing prices instead.
    pub fee: u64,
}

/// A solver-supplied settlement interaction. Only `custom` exists: the
/// dormant liquidity variant of the EVM DTO is never emitted on Solana, so
/// the type does not carry it.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Interaction {
    Custom(CustomInteraction),
}

/// The aggregator's instruction carried verbatim: program ID, full account
/// metas (writable and signer flags), and the instruction data.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomInteraction {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub program_id: Pubkey,
    pub accounts: Vec<AccountMeta>,
    /// Base64, matching the aggregator wire encoding.
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
    /// Sell and buy mint coincide, so a uniform clearing-price map (one price
    /// per mint) cannot represent the trade.
    #[error("sell and buy mint are the same")]
    SameMint,
    /// A clearing price of zero would make the trade worthless downstream.
    #[error("quoted amount is zero")]
    ZeroAmount,
}

impl Solution {
    /// Wraps one quoted swap into a single-order solution.
    ///
    /// Clearing prices derive from the quoted amounts: the sell mint is
    /// priced at the swap's output amount and the buy mint at its input
    /// amount, so `executed × price` matches on both sides. The swap's
    /// instructions are carried verbatim as `custom` interactions, and its
    /// address lookup tables travel along so the driver can build the v0
    /// transaction the instructions assume.
    pub fn single(
        id: u64,
        order_uid: [u8; 32],
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
            prices: HashMap::from([
                (order.sell_mint, swap.out_amount),
                (order.buy_mint, swap.in_amount),
            ]),
            trades: vec![Trade {
                order_uid,
                executed_amount,
                fee: 0,
            }],
            interactions: swap.instructions.iter().map(Interaction::custom).collect(),
            address_lookup_tables: swap.address_lookup_tables,
        })
    }
}

impl Interaction {
    fn custom(instruction: &Instruction) -> Self {
        Self::Custom(CustomInteraction {
            program_id: instruction.program_id,
            accounts: instruction
                .accounts
                .iter()
                .map(|meta| AccountMeta {
                    pubkey: meta.pubkey,
                    is_signer: meta.is_signer,
                    is_writable: meta.is_writable,
                })
                .collect(),
            instruction_data: instruction.data.clone(),
        })
    }
}

fn serialize_base64<S: serde::Serializer>(data: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&BASE64_STANDARD.encode(data))
}

fn serialize_hex<S: serde::Serializer>(data: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&data.map(|byte| format!("{byte:02x}")).concat())
}

#[cfg(test)]
mod tests {
    use {super::*, solana_sdk::instruction::AccountMeta as SdkAccountMeta, std::str::FromStr};

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
            instructions: vec![Instruction {
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
        let solution = Solution::single(42, [8; 32], &order, swap()).unwrap();

        assert_eq!(solution.id, 42);
        // Clearing prices: sell mint priced at the output amount, buy mint at
        // the input amount, so executed × price matches on both sides.
        assert_eq!(solution.prices[&order.sell_mint], 2_000);
        assert_eq!(solution.prices[&order.buy_mint], 1_000);
        assert_eq!(solution.trades.len(), 1);
        assert_eq!(solution.trades[0].order_uid, [8; 32]);
        assert_eq!(solution.trades[0].executed_amount, 1_000);
        assert_eq!(solution.trades[0].fee, 0);
        assert_eq!(solution.address_lookup_tables, vec![pubkey(7)]);

        // The instruction is carried verbatim, flags included.
        let Interaction::Custom(custom) = &solution.interactions[0];
        assert_eq!(custom.program_id, pubkey(9));
        assert_eq!(custom.accounts[0].pubkey, pubkey(4));
        assert!(custom.accounts[0].is_signer);
        assert!(!custom.accounts[0].is_writable);
        assert_eq!(custom.instruction_data, vec![0xde, 0xad]);
    }

    #[test]
    fn buy_swap_executes_in_buy_token_units() {
        let solution = Solution::single(0, [0; 32], &order(dex::Side::Buy), swap()).unwrap();
        assert_eq!(solution.trades[0].executed_amount, 2_000);
    }

    #[test]
    fn same_mint_order_is_rejected() {
        let mut order = order(dex::Side::Sell);
        order.buy_mint = order.sell_mint;
        assert_eq!(
            Solution::single(0, [0; 32], &order, swap()).unwrap_err(),
            Error::SameMint
        );
    }

    #[test]
    fn zero_quoted_amount_is_rejected() {
        let mut swap = swap();
        swap.out_amount = 0;
        assert_eq!(
            Solution::single(0, [0; 32], &order(dex::Side::Sell), swap).unwrap_err(),
            Error::ZeroAmount
        );
    }

    #[test]
    fn wire_format_is_stable() {
        let solution = Solution::single(1, [8; 32], &order(dex::Side::Sell), swap()).unwrap();
        let json = serde_json::to_value(&solution).unwrap();

        assert_eq!(
            json["prices"][pubkey(1).to_string()],
            serde_json::json!(2_000)
        );
        assert_eq!(json["trades"][0]["orderUid"], "08".repeat(32));
        assert_eq!(json["trades"][0]["executedAmount"], 1_000);
        assert_eq!(json["interactions"][0]["kind"], "custom");
        assert_eq!(json["interactions"][0]["programId"], pubkey(9).to_string());
        assert_eq!(
            json["interactions"][0]["instructionData"],
            BASE64_STANDARD.encode([0xde, 0xad])
        );
        assert!(
            json["interactions"][0]["accounts"][0]["isSigner"]
                .as_bool()
                .unwrap()
        );
        assert_eq!(json["addressLookupTables"][0], pubkey(7).to_string());
        // No cu_estimate on the wire: CU sizing is the driver's job.
        assert!(json.get("cuEstimate").is_none());
    }

    #[test]
    fn from_str_roundtrip_for_wire_keys() {
        // Pubkeys serialize as base58 and parse back.
        let key = pubkey(5);
        assert_eq!(Pubkey::from_str(&key.to_string()).unwrap(), key);
    }
}
