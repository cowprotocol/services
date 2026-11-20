//! What a transaction costs its fee payer, read from the transaction itself.
//! The orderbook caps a sponsored creation's priority fee with it and the
//! autopilot charges the creation's fee with it, so both price the same
//! transaction the same way.

#![forbid(unsafe_code)]

use {
    solana_sdk::{message::VersionedMessage, pubkey::Pubkey},
    solana_sdk_ids::{compute_budget, system_program},
};

/// The base fee of one signature, in lamports, on every cluster.
pub const LAMPORTS_PER_SIGNATURE: u64 = 5_000;

/// What the funder pays for a sponsored creation that sets no priority fee,
/// in lamports: the funder's signature and the owner's. A priority fee is the
/// wallet's choice after the quote, so the quote cannot know it.
pub const SPONSORED_CREATION_FEE: u64 = 2 * LAMPORTS_PER_SIGNATURE;

/// The compute units a transaction can consume at most. The runtime lowers a
/// higher limit to this, declared or default.
pub const MAX_COMPUTE_UNIT_LIMIT: u32 = 1_400_000;

/// The compute units the runtime grants each instruction of a transaction
/// that declares no limit: Agave's `DEFAULT_INSTRUCTION_COMPUTE_UNIT_LIMIT`
/// and, for an instruction of a builtin program,
/// `MAX_BUILTIN_ALLOCATION_COMPUTE_UNIT_LIMIT`.
const DEFAULT_INSTRUCTION_COMPUTE_UNIT_LIMIT: u32 = 200_000;
const BUILTIN_INSTRUCTION_COMPUTE_UNIT_LIMIT: u32 = 3_000;

/// The Agave builtins a creation may name, of the cost map in
/// `solana-builtins-default-costs`: the orderbook admits no other builtin.
const BUILTINS: [Pubkey; 2] = [system_program::ID, compute_budget::ID];

/// What the client asked the runtime to charge for priority.
#[derive(Default)]
pub struct ComputeBudget {
    /// Micro-lamports per compute unit.
    pub price: Option<u64>,
    /// Compute units the transaction may consume.
    pub limit: Option<u32>,
    /// Compute units the runtime grants when `limit` is absent.
    default_limit: u32,
}

impl ComputeBudget {
    /// The compute budget of `message`, whose static keys must name every
    /// program.
    pub fn of(message: &VersionedMessage) -> Result<Self, &'static str> {
        let keys = message.static_account_keys();
        let mut budget = Self::default();
        for instruction in message.instructions() {
            let program = keys.get(usize::from(instruction.program_id_index));
            if program == Some(&compute_budget::ID) {
                budget.read(&instruction.data)?;
            }
            let units = if program.is_some_and(|program| BUILTINS.contains(program)) {
                BUILTIN_INSTRUCTION_COMPUTE_UNIT_LIMIT
            } else {
                DEFAULT_INSTRUCTION_COMPUTE_UNIT_LIMIT
            };
            budget.default_limit = budget.default_limit.saturating_add(units);
        }
        Ok(budget)
    }

    /// Read one compute-budget instruction. Duplicates of a kind are rejected
    /// because the runtime rejects them too, so accepting one would price the
    /// transaction off a value that never takes effect.
    fn read(&mut self, data: &[u8]) -> Result<(), &'static str> {
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

    /// The priority fee the transaction pays, in lamports, rounded up. The
    /// runtime charges the price over the limit, not over the units consumed.
    pub fn max_priority_fee_lamports(&self) -> u128 {
        let Some(price) = self.price else {
            return 0;
        };
        let limit = self
            .limit
            .unwrap_or(self.default_limit)
            .min(MAX_COMPUTE_UNIT_LIMIT);
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
    let budget = ComputeBudget::of(message)?;
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

    /// No price means no priority fee. A price without a limit is charged
    /// over the runtime's default: 3000 units for the price instruction and
    /// 200000 for each of the two program instructions. Eight of those reach
    /// the ceiling.
    #[test]
    fn an_undeclared_limit_is_priced_at_the_runtime_default() {
        let program = || Instruction::new_with_bytes(Pubkey::new_unique(), &[], vec![]);
        let budget = |instructions: &[Instruction]| {
            let message = Message::new(instructions, Some(&Pubkey::new_unique()));
            ComputeBudget::of(&VersionedMessage::Legacy(message)).unwrap()
        };
        let price = Instruction::new_with_bytes(compute_budget::ID, &set_price(700_000), vec![]);

        assert_eq!(budget(&[program()]).max_priority_fee_lamports(), 0);
        assert_eq!(
            budget(&[price.clone(), program(), program()]).max_priority_fee_lamports(),
            282_100
        );
        let mut crowded = vec![price];
        crowded.extend(std::iter::repeat_with(program).take(8));
        assert_eq!(budget(&crowded).max_priority_fee_lamports(), 980_000);
    }

    /// The runtime lowers a declared limit to the ceiling, so the units past
    /// it are never charged.
    #[test]
    fn a_limit_past_the_ceiling_is_priced_at_the_ceiling() {
        let mut budget = ComputeBudget::default();
        budget.read(&set_price(232)).unwrap();
        budget.read(&set_limit(u32::MAX)).unwrap();
        assert_eq!(budget.max_priority_fee_lamports(), 325);
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
        let budget = |data: Vec<u8>| Instruction::new_with_bytes(compute_budget::ID, &data, vec![]);
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
