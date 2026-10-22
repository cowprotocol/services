//! Solution assembly: one quoted swap becomes one solution in the driver's
//! `/solve` DTO, filling one order or a batch of orders of one side on one
//! directed mint pair.

use {
    super::order::OrderUid,
    crate::dex,
    base64::prelude::*,
    serde::Serialize,
    serde_with::serde_as,
    solana_sdk::{instruction::Instruction as SolInstruction, pubkey::Pubkey},
    std::collections::HashMap,
};

/// A solution in the driver's `/solve` DTO. Trades fulfill auction orders, with
/// no JIT.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Solution {
    pub id: u64,
    /// Uniform clearing prices by mint: the executed swap's ratio, the sell
    /// mint mapping to the amount bought and the buy mint to the amount
    /// sold, so `executed_sell * price_sell == executed_buy * price_buy`.
    #[serde_as(as = "HashMap<serde_with::DisplayFromStr, serde_with::DisplayFromStr>")]
    pub prices: HashMap<Pubkey, u64>,
    pub trades: Vec<Trade>,
    pub interactions: Vec<Instruction>,
    /// Optional solver estimate of total settlement compute units.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cu_estimate: Option<u32>,
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
    /// A batch of no orders fills nothing.
    #[error("batch has no orders")]
    NoOrders,
    /// Every order in a batch must share the swap's side, mints and
    /// destination: one swap fixes either its input or its output.
    #[error("batch orders are not all on one side, pair and destination")]
    MixedOrders,
    /// The orders' total amount, or the raised clearing input, does not fit
    /// in a `u64`.
    #[error("batch amounts overflow")]
    TotalOverflow,
    /// The swap must move exactly the orders' total on their side (its
    /// input for sells, its output for buys), or the trades would not add
    /// up to the swap.
    #[error("swap moves {swapped} but the batch trades {total}")]
    AmountMismatch { total: u64, swapped: u64 },
}

impl Solution {
    /// Wraps one quoted swap into a single-order solution.
    ///
    /// The swap's instructions are carried verbatim as interactions, and
    /// its address lookup tables travel along so the driver can build the
    /// v0 transaction the instructions assume.
    pub fn new(
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
        let trades = vec![Trade {
            order_uid,
            executed_amount,
        }];
        let (in_amount, out_amount) = (swap.in_amount, swap.out_amount);
        Ok(Self::assemble(
            id, order, trades, in_amount, out_amount, swap,
        ))
    }

    /// Wraps one quoted swap over the total of `orders`, orders of one side
    /// on one directed mint pair, into a solution filling each of them in
    /// full at the swap's uniform price.
    ///
    /// The swap must move exactly the orders' total on their side: the
    /// input of an `ExactIn` swap for sells, the output of an `ExactOut`
    /// swap for buys. The prices encode [`dex::Swap::clearing_rate`], so the
    /// driver's rounded-down counterpart legs never ask more of the swap
    /// than it moves. Whether that rate clears each order's limit is the
    /// caller's check.
    pub fn batch(
        id: u64,
        orders: &[(OrderUid, dex::Order)],
        swap: dex::Swap,
    ) -> Result<Self, Error> {
        let (_, first) = orders.first().ok_or(Error::NoOrders)?;
        if first.sell_mint == first.buy_mint {
            return Err(Error::SameMint);
        }
        if swap.in_amount == 0 || swap.out_amount == 0 {
            return Err(Error::ZeroAmount);
        }
        let mut total: u64 = 0;
        for (_, order) in orders {
            if (
                order.side,
                order.sell_mint,
                order.buy_mint,
                order.buy_destination,
            ) != (
                first.side,
                first.sell_mint,
                first.buy_mint,
                first.buy_destination,
            ) {
                return Err(Error::MixedOrders);
            }
            total = total
                .checked_add(order.amount())
                .ok_or(Error::TotalOverflow)?;
        }
        let swapped = match first.side {
            dex::Side::Sell => swap.in_amount,
            dex::Side::Buy => swap.out_amount,
        };
        if swapped != total {
            return Err(Error::AmountMismatch { total, swapped });
        }
        let (in_amount, out_amount) = swap
            .clearing_rate(first.side, orders.len())
            .ok_or(Error::TotalOverflow)?;
        let trades = orders
            .iter()
            .map(|(order_uid, order)| Trade {
                order_uid: *order_uid,
                executed_amount: order.amount(),
            })
            .collect();
        Ok(Self::assemble(
            id, first, trades, in_amount, out_amount, swap,
        ))
    }

    /// The solution settling `trades`, all on `order`'s mints, with `swap`
    /// at the rate `in_amount` for `out_amount`: the swap's instructions
    /// carried verbatim as interactions, its lookup tables along so the
    /// driver can build the v0 transaction they assume.
    fn assemble(
        id: u64,
        order: &dex::Order,
        trades: Vec<Trade>,
        in_amount: u64,
        out_amount: u64,
        swap: dex::Swap,
    ) -> Self {
        Self {
            id,
            prices: HashMap::from([(order.sell_mint, out_amount), (order.buy_mint, in_amount)]),
            trades,
            interactions: swap
                .instructions
                .into_iter()
                .map(Instruction::from_sdk)
                .collect(),
            cu_estimate: None,
            address_lookup_tables: swap.address_lookup_tables,
        }
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
            sell_amount: 1_000,
            buy_amount: 2_000,
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
        let solution = Solution::new(42, ORDER_UID, &order, swap()).unwrap();

        assert_eq!(solution.id, 42);
        assert_eq!(solution.trades.len(), 1);
        assert_eq!(solution.trades[0].order_uid, ORDER_UID);
        assert_eq!(solution.trades[0].executed_amount, 1_000);
        // Sell mint prices at the amount bought, buy mint at the amount sold.
        assert_eq!(solution.prices[&order.sell_mint], 2_000);
        assert_eq!(solution.prices[&order.buy_mint], 1_000);
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
        let solution = Solution::new(0, ORDER_UID, &order(dex::Side::Buy), swap()).unwrap();
        assert_eq!(solution.trades[0].executed_amount, 2_000);
    }

    #[test]
    fn same_mint_order_is_rejected() {
        let mut order = order(dex::Side::Sell);
        order.buy_mint = order.sell_mint;
        assert_eq!(
            Solution::new(0, ORDER_UID, &order, swap()).unwrap_err(),
            Error::SameMint
        );
    }

    #[test]
    fn zero_quoted_amount_is_rejected() {
        let mut swap = swap();
        swap.out_amount = 0;
        assert_eq!(
            Solution::new(0, ORDER_UID, &order(dex::Side::Sell), swap).unwrap_err(),
            Error::ZeroAmount
        );
    }

    fn sell_of(sell_amount: u64) -> dex::Order {
        dex::Order {
            sell_amount,
            buy_amount: 0,
            ..order(dex::Side::Sell)
        }
    }

    fn batch_swap(in_amount: u64, out_amount: u64) -> dex::Swap {
        dex::Swap {
            in_amount,
            out_amount,
            ..swap()
        }
    }

    #[test]
    fn batch_fills_each_order_in_full_at_the_swap_ratio() {
        let orders = [
            (OrderUid([1; 32]), sell_of(1_000)),
            (OrderUid([2; 32]), sell_of(3_000)),
        ];
        let solution = Solution::batch(7, &orders, batch_swap(4_000, 10_000)).unwrap();

        assert_eq!(solution.id, 7);
        assert_eq!(
            solution
                .trades
                .iter()
                .map(|trade| (trade.order_uid, trade.executed_amount))
                .collect::<Vec<_>>(),
            vec![(OrderUid([1; 32]), 1_000), (OrderUid([2; 32]), 3_000)]
        );
        assert_eq!(solution.prices[&pubkey(1)], 10_000);
        assert_eq!(solution.prices[&pubkey(2)], 4_000);
        assert_eq!(solution.interactions.len(), 1);
        assert_eq!(solution.address_lookup_tables, vec![pubkey(7)]);
    }

    #[test]
    fn batch_rejects_a_swap_input_off_the_total() {
        let orders = [
            (OrderUid([1; 32]), sell_of(1_000)),
            (OrderUid([2; 32]), sell_of(3_000)),
        ];
        assert_eq!(
            Solution::batch(0, &orders, batch_swap(3_999, 10_000)).unwrap_err(),
            Error::AmountMismatch {
                total: 4_000,
                swapped: 3_999
            }
        );
    }

    #[test]
    fn batch_rejects_mixed_sides() {
        let orders = [
            (OrderUid([1; 32]), sell_of(1_000)),
            (OrderUid([2; 32]), order(dex::Side::Buy)),
        ];
        assert_eq!(
            Solution::batch(0, &orders, batch_swap(2_000, 4_000)).unwrap_err(),
            Error::MixedOrders
        );
    }

    fn buy_of(buy_amount: u64) -> dex::Order {
        dex::Order {
            sell_amount: u64::MAX,
            buy_amount,
            ..order(dex::Side::Buy)
        }
    }

    /// Three buys of 1 that an `ExactOut` swap fills for 5: at the swap's
    /// own rate the driver would pull `floor(5 / 3) = 1` from each, 3 of
    /// the 5 the swap spends. The raised rate pulls `floor(7 / 3) = 2` from
    /// each, 6.
    #[test]
    fn buy_batch_prices_cover_the_swap_input_after_rounding() {
        let orders = [
            (OrderUid([1; 32]), buy_of(1)),
            (OrderUid([2; 32]), buy_of(1)),
            (OrderUid([3; 32]), buy_of(1)),
        ];
        let solution = Solution::batch(0, &orders, batch_swap(5, 3)).unwrap();

        assert_eq!(
            solution
                .trades
                .iter()
                .map(|trade| trade.executed_amount)
                .collect::<Vec<_>>(),
            vec![1, 1, 1]
        );
        let (price_sell, price_buy) = (solution.prices[&pubkey(1)], solution.prices[&pubkey(2)]);
        assert_eq!((price_sell, price_buy), (3, 7));
        // The driver's counterpart leg of a buy: executed buy * price_buy /
        // price_sell, rounded down.
        let pulled: u64 = solution
            .trades
            .iter()
            .map(|trade| trade.executed_amount * price_buy / price_sell)
            .sum();
        assert!(pulled >= 5);
    }

    #[test]
    fn buy_batch_rejects_a_swap_output_off_the_total() {
        let orders = [
            (OrderUid([1; 32]), buy_of(2)),
            (OrderUid([2; 32]), buy_of(2)),
        ];
        assert_eq!(
            Solution::batch(0, &orders, batch_swap(5, 3)).unwrap_err(),
            Error::AmountMismatch {
                total: 4,
                swapped: 3
            }
        );
    }

    #[test]
    fn batch_rejects_orders_off_the_first_ones_pair_or_destination() {
        for off in [
            dex::Order {
                buy_mint: pubkey(5),
                ..sell_of(1_000)
            },
            dex::Order {
                buy_destination: pubkey(5),
                ..sell_of(1_000)
            },
        ] {
            let orders = [
                (OrderUid([1; 32]), sell_of(1_000)),
                (OrderUid([2; 32]), off),
            ];
            assert_eq!(
                Solution::batch(0, &orders, batch_swap(2_000, 4_000)).unwrap_err(),
                Error::MixedOrders
            );
        }
    }

    #[test]
    fn batch_rejects_no_orders_and_an_overflowing_total() {
        assert_eq!(
            Solution::batch(0, &[], batch_swap(1, 1)).unwrap_err(),
            Error::NoOrders
        );
        let orders = [
            (OrderUid([1; 32]), sell_of(u64::MAX)),
            (OrderUid([2; 32]), sell_of(1)),
        ];
        assert_eq!(
            Solution::batch(0, &orders, batch_swap(1, 1)).unwrap_err(),
            Error::TotalOverflow
        );
    }

    #[test]
    fn wire_format_is_stable() {
        let solution = Solution::new(1, ORDER_UID, &order(dex::Side::Sell), swap()).unwrap();
        let json = serde_json::to_value(&solution).unwrap();

        assert_eq!(
            json,
            serde_json::json!({
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
