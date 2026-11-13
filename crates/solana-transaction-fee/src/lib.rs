//! What a transaction costs its fee payer, read from the transaction itself.
//! The orderbook caps a sponsored creation's priority fee with it and the
//! autopilot charges the creation's fee with it, so both price the same
//! transaction the same way.

#![forbid(unsafe_code)]

use solana_sdk::message::VersionedMessage;

/// The base fee of one signature, in lamports, on every cluster.
pub const LAMPORTS_PER_SIGNATURE: u64 = 5_000;

/// The compute units a transaction can consume at most, whatever it declares.
/// A transaction that names no limit is priced against this, since the units
/// the runtime would otherwise grant depend on which programs each
/// instruction calls.
pub const MAX_COMPUTE_UNIT_LIMIT: u32 = 1_400_000;

/// What the client asked the runtime to charge for priority.
#[derive(Default)]
pub struct ComputeBudget {
    /// Micro-lamports per compute unit.
    pub price: Option<u64>,
    /// Compute units the transaction may consume.
    pub limit: Option<u32>,
}

impl ComputeBudget {
    /// Read one compute-budget instruction. Duplicates of a kind are rejected
    /// because the runtime rejects them too, so accepting one would price the
    /// transaction off a value that never takes effect.
    pub fn read(&mut self, data: &[u8]) -> Result<(), &'static str> {
        // Discriminators of `SetComputeUnitLimit` (u32) and
        // `SetComputeUnitPrice` (u64), both little-endian.
        let (slot_taken, malformed) = match data.split_first() {
            Some((2, limit)) => {
                let limit = limit
                    .try_into()
                    .map(u32::from_le_bytes)
                    .map_err(|_| "malformed compute unit limit");
                (self.limit.is_some(), limit.map(|v| self.limit = Some(v)))
            }
            Some((3, price)) => {
                let price = price
                    .try_into()
                    .map(u64::from_le_bytes)
                    .map_err(|_| "malformed compute unit price");
                (self.price.is_some(), price.map(|v| self.price = Some(v)))
            }
            // Heap frames and data size limits cost the funder nothing.
            _ => return Ok(()),
        };
        if slot_taken {
            return Err("the transaction sets a compute budget twice");
        }
        malformed
    }

    /// The most the transaction could pay in priority fee, in lamports,
    /// rounded up. An undeclared limit is priced at the network ceiling.
    pub fn max_priority_fee_lamports(&self) -> u128 {
        let Some(price) = self.price else {
            return 0;
        };
        let limit = self.limit.unwrap_or(MAX_COMPUTE_UNIT_LIMIT);
        // Micro-lamports per unit times units, rounded up to whole lamports.
        (u128::from(price) * u128::from(limit)).div_ceil(1_000_000)
    }
}

/// The most `message` can cost its fee payer, in lamports: the base fee of
/// every required signature plus the priority fee. Only a legacy or v0
/// message without address lookup tables is priced: its static keys name
/// every program, and its compute budget instructions carry the whole
/// priority fee.
pub fn max_fee_lamports(message: &VersionedMessage) -> Result<u64, &'static str> {
    if matches!(message, VersionedMessage::V1(_)) {
        return Err("a v1 message carries its priority fee in its config");
    }
    if message
        .address_table_lookups()
        .is_some_and(|lookups| !lookups.is_empty())
    {
        return Err("an address lookup table can hide a compute budget instruction");
    }
    let keys = message.static_account_keys();
    let mut budget = ComputeBudget::default();
    for instruction in message.instructions() {
        if keys.get(usize::from(instruction.program_id_index))
            == Some(&solana_compute_budget_interface::ID)
        {
            budget.read(&instruction.data)?;
        }
    }
    let signatures =
        u128::from(message.header().num_required_signatures) * u128::from(LAMPORTS_PER_SIGNATURE);
    u64::try_from(signatures + budget.max_priority_fee_lamports())
        .map_err(|_| "the fee is past u64")
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        solana_sdk::{
            hash::Hash,
            instruction::{AccountMeta, Instruction},
            message::Message,
            pubkey::Pubkey,
        },
    };

    fn set_limit(units: u32) -> Vec<u8> {
        std::iter::once(2u8)
            .chain(units.to_le_bytes())
            .collect::<Vec<_>>()
    }

    fn set_price(micro_lamports: u64) -> Vec<u8> {
        std::iter::once(3u8)
            .chain(micro_lamports.to_le_bytes())
            .collect::<Vec<_>>()
    }

    /// The fee is the product, so a steep price over few units stays cheap
    /// while a modest price over the whole compute ceiling does not.
    #[test]
    fn the_fee_is_the_price_times_the_limit() {
        let mut steep = ComputeBudget::default();
        steep.read(&set_price(1_000_000)).unwrap();
        steep.read(&set_limit(20_000)).unwrap();
        assert_eq!(steep.max_priority_fee_lamports(), 20_000);

        let mut wide = ComputeBudget::default();
        wide.read(&set_price(1_000)).unwrap();
        wide.read(&set_limit(MAX_COMPUTE_UNIT_LIMIT)).unwrap();
        assert_eq!(wide.max_priority_fee_lamports(), 1_400);
    }

    /// No price means no priority fee. A price without a limit is priced at
    /// the ceiling, since the transaction may consume up to it.
    #[test]
    fn an_undeclared_limit_is_priced_at_the_ceiling() {
        assert_eq!(ComputeBudget::default().max_priority_fee_lamports(), 0);

        let mut priced = ComputeBudget::default();
        priced.read(&set_price(1_000_000)).unwrap();
        assert_eq!(priced.max_priority_fee_lamports(), 1_400_000);
    }

    /// The runtime rejects a repeated compute-budget instruction, so pricing
    /// the transaction off the first one would read a value that never runs.
    #[test]
    fn a_repeated_compute_budget_is_rejected() {
        let mut budget = ComputeBudget::default();
        budget.read(&set_price(10)).unwrap();
        assert!(budget.read(&set_price(20)).is_err());

        let mut budget = ComputeBudget::default();
        budget.read(&set_limit(10)).unwrap();
        assert!(budget.read(&set_limit(20)).is_err());
    }

    /// A truncated payload is refused rather than read as a smaller number.
    #[test]
    fn a_malformed_payload_is_rejected() {
        let mut budget = ComputeBudget::default();
        assert!(budget.read(&[3, 1, 2, 3]).is_err());
        assert!(budget.read(&[2, 1]).is_err());
        // Variants that cost the funder nothing are ignored, not parsed.
        assert!(budget.read(&[1, 0, 0, 4, 0]).is_ok());
    }

    /// Two signatures and 375000 micro-lamports over 200000 units, what
    /// Phantom adds to a creation: 10000 lamports of base fee and 75000 of
    /// priority fee. Without a compute budget only the base fee is left.
    #[test]
    fn the_fee_is_the_signatures_plus_the_priority_fee() {
        let (payer, owner) = (Pubkey::new_unique(), Pubkey::new_unique());
        let order = Instruction::new_with_bytes(
            Pubkey::new_unique(),
            &[],
            vec![AccountMeta::new(owner, true)],
        );
        let budget = |data: Vec<u8>| {
            Instruction::new_with_bytes(solana_compute_budget_interface::ID, &data, vec![])
        };
        let fee = |instructions: &[Instruction]| {
            let message = Message::new_with_blockhash(instructions, Some(&payer), &Hash::default());
            max_fee_lamports(&VersionedMessage::Legacy(message))
        };
        assert_eq!(fee(std::slice::from_ref(&order)), Ok(10_000));
        assert_eq!(
            fee(&[
                budget(set_price(375_000)),
                order.clone(),
                budget(set_limit(200_000)),
            ]),
            Ok(85_000)
        );
        assert!(fee(&[budget(set_price(1)), budget(set_price(2)), order]).is_err());
    }
}
