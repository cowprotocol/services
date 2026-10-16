//! DTOs for Bitget's `/swap` and `/signBgwSolTx` requests and responses.

use {
    crate::{
        config::Fee,
        dex::{Error, Order, Swap, dto::ApiInstruction},
    },
    serde::{Deserialize, Serialize, de::DeserializeOwned},
    serde_with::{DisplayFromStr, serde_as},
    solana_sdk::{pubkey::Pubkey, transaction::VersionedTransaction},
    spl_associated_token_account_interface::address::get_associated_token_address_with_program_id,
    spl_token_2022_interface::inline_spl_token,
    std::collections::HashSet,
};

/// Bitget's aggregator. Naming it lets `/swap` skip the `/quote` call.
const MARKET: &str = "bgwaggregator";

/// Body of the `/swap` request for a sell order, amounts in atoms.
#[serde_as]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapRequest {
    from_chain: &'static str,
    #[serde_as(as = "DisplayFromStr")]
    from_contract: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    from_amount: u64,
    to_chain: &'static str,
    #[serde_as(as = "DisplayFromStr")]
    to_contract: Pubkey,
    amount_unit: &'static str,
    #[serde_as(as = "DisplayFromStr")]
    from_address: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    to_address: Pubkey,
    market: &'static str,
    /// Percent.
    slippage: f64,
    fee_rate: f64,
    request_mod: &'static str,
    #[serde_as(as = "DisplayFromStr")]
    sol_max_accounts: u16,
}

impl SwapRequest {
    pub fn new(
        order: &Order,
        taker: &Pubkey,
        slippage_bps: u16,
        max_accounts: u16,
        fee: Fee,
    ) -> Self {
        Self {
            from_chain: "sol",
            from_contract: order.sell_mint,
            from_amount: order.sell_amount,
            to_chain: "sol",
            to_contract: order.buy_mint,
            amount_unit: "wei",
            from_address: *taker,
            to_address: *taker,
            market: MARKET,
            slippage: f64::from(slippage_bps) / 100.0,
            fee_rate: match fee {
                Fee::Standard => 0.0001,
                Fee::Exempt => 0.0,
            },
            request_mod: "rich",
            sol_max_accounts: max_accounts,
        }
    }
}

/// Bitget's response wrapper: `status` 0 carries `data`, any other an
/// [`error_code`](https://web3.bitget.com/en/docs/swap-order#error-code-list).
#[derive(Deserialize)]
pub struct Envelope {
    status: i64,
    error_code: Option<i64>,
    msg: Option<String>,
    data: Option<serde_json::Value>,
}

impl Envelope {
    pub fn into_data<T: DeserializeOwned>(self, http_status: u16) -> Result<T, Error> {
        if self.status != 0 {
            let code = self.error_code.unwrap_or_default();
            return Err(match code {
                // No route, or a Bitget-side failure the next auction retries.
                80000 | 80001 | 80004 | 80005 | 80008..=80012 | 80014 | 80020 => Error::NotFound,
                _ => Error::Api {
                    status: http_status,
                    body: format!("error_code {code}: {}", self.msg.unwrap_or_default()),
                },
            });
        }
        let data = self.data.ok_or(Error::NotFound)?;
        serde_json::from_value(data)
            .map_err(|err| Error::BadResponse(format!("response data: {err}")))
    }
}

/// The parts of a rich-mode `/swap` response we need. `minAmount` is the
/// output after Bitget's fee, which its program enforces on chain.
#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponse {
    #[serde_as(as = "DisplayFromStr")]
    min_amount: u64,
    instruction_lists: Vec<ApiInstruction>,
    #[serde_as(as = "Vec<DisplayFromStr>")]
    address_lookup_table_account: Vec<Pubkey>,
}

impl SwapResponse {
    /// Bitget pays the taker's buy-mint ATA, so a transfer of `minAmount` moves
    /// the output on to `order.buy_destination` and positive slippage stays
    /// with the taker. ComputeBudget instructions are dropped: the driver sets
    /// its own. An exempt route may also need Bitget's signature.
    pub fn into_swap(self, order: &Order, taker: &Pubkey, fee: Fee) -> Result<Swap, Error> {
        let mut instructions = Vec::with_capacity(self.instruction_lists.len() + 1);
        for instruction in self.instruction_lists {
            let instruction = instruction.into_instruction()?;
            if instruction.program_id != solana_compute_budget_interface::ID {
                instructions.push(instruction);
            }
        }
        let accounts = || {
            instructions
                .iter()
                .flat_map(|instruction| &instruction.accounts)
        };
        let cosigners: HashSet<_> = accounts()
            .filter(|account| account.is_signer && account.pubkey != *taker)
            .map(|account| account.pubkey)
            .collect();
        let allowed = match fee {
            Fee::Standard => 0,
            Fee::Exempt => 1,
        };
        if cosigners.len() > allowed {
            return Err(Error::BadResponse(format!(
                "route needs {} signers besides the taker",
                cosigners.len()
            )));
        }
        let (token_program, output) = [inline_spl_token::ID, spl_token_2022_interface::ID]
            .into_iter()
            .map(|program| {
                let ata =
                    get_associated_token_address_with_program_id(taker, &order.buy_mint, &program);
                (program, ata)
            })
            .find(|(_, ata)| accounts().any(|account| account.pubkey == *ata))
            .ok_or_else(|| Error::BadResponse("route doesn't pay the taker's ATA".into()))?;
        // Unchecked because the auction carries no decimals. Token-2022 mints
        // with a transfer fee or hook reject it, and simulation drops them.
        #[allow(deprecated)]
        let transfer = spl_token_2022_interface::instruction::transfer(
            &token_program,
            &output,
            &order.buy_destination,
            taker,
            &[],
            self.min_amount,
        )
        .expect("both token programs pass the program check");
        instructions.push(transfer);
        Ok(Swap {
            in_amount: order.sell_amount,
            out_amount: self.min_amount,
            instructions,
            address_lookup_tables: self.address_lookup_table_account,
        })
    }
}

/// Body of the `/signBgwSolTx` request: the transaction as base58 bincode.
#[derive(Serialize)]
pub struct CosignRequest {
    chain: &'static str,
    transaction: String,
}

impl CosignRequest {
    pub fn new(transaction: &VersionedTransaction) -> Self {
        let bytes = bincode::serialize(transaction).expect("a transaction serializes");
        Self {
            chain: "sol",
            transaction: bs58::encode(bytes).into_string(),
        }
    }
}

#[derive(Deserialize)]
pub struct CosignResponse {
    transaction: String,
}

impl CosignResponse {
    pub fn into_transaction(self) -> Result<VersionedTransaction, Error> {
        let bytes = bs58::decode(&self.transaction)
            .into_vec()
            .map_err(|err| Error::BadResponse(format!("co-signed transaction: {err}")))?;
        bincode::deserialize(&bytes)
            .map_err(|err| Error::BadResponse(format!("co-signed transaction: {err}")))
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::dex::Side};

    fn order() -> Order {
        Order {
            sell_mint: Pubkey::new_unique(),
            buy_mint: Pubkey::new_unique(),
            buy_destination: Pubkey::new_unique(),
            sell_amount: 1_000,
            buy_amount: 1,
            side: Side::Sell,
        }
    }

    fn response(
        order: &Order,
        taker: &Pubkey,
        token_program: &Pubkey,
        extra_signers: &[Pubkey],
    ) -> SwapResponse {
        let output =
            get_associated_token_address_with_program_id(taker, &order.buy_mint, token_program);
        let mut accounts = vec![
            serde_json::json!({"pubkey": taker.to_string(), "isSigner": true, "isWritable": true}),
            serde_json::json!({"pubkey": output.to_string(), "isSigner": false, "isWritable": true}),
        ];
        for signer in extra_signers {
            accounts.push(serde_json::json!({"pubkey": signer.to_string(), "isSigner": true, "isWritable": false}));
        }
        serde_json::from_value(serde_json::json!({
            "minAmount": "990",
            "instructionLists": [
                {"programId": solana_compute_budget_interface::ID.to_string(), "accounts": [], "data": "AnBkCAA="},
                {"programId": Pubkey::new_unique().to_string(), "accounts": accounts, "data": "AQ=="},
            ],
            "addressLookupTableAccount": [Pubkey::new_unique().to_string()],
        }))
        .unwrap()
    }

    #[test]
    fn moves_the_minimum_output_into_the_buffer() {
        let taker = Pubkey::new_unique();
        let order = order();
        for program in [inline_spl_token::ID, spl_token_2022_interface::ID] {
            let swap = response(&order, &taker, &program, &[])
                .into_swap(&order, &taker, Fee::Standard)
                .unwrap();

            assert_eq!((swap.in_amount, swap.out_amount), (1_000, 990));
            assert_eq!(swap.address_lookup_tables.len(), 1);
            // ComputeBudget dropped: the route, then the transfer.
            assert_eq!(swap.instructions.len(), 2);
            let transfer = &swap.instructions[1];
            assert_eq!(transfer.program_id, program);
            let paid =
                get_associated_token_address_with_program_id(&taker, &order.buy_mint, &program);
            assert_eq!(transfer.accounts[0].pubkey, paid);
            assert_eq!(transfer.accounts[1].pubkey, order.buy_destination);
            assert_eq!(transfer.accounts[2].pubkey, taker);
            assert_eq!(transfer.data[1..], 990u64.to_le_bytes());
        }
    }

    #[test]
    fn only_a_fee_exempt_route_may_need_bitgets_signature() {
        let taker = Pubkey::new_unique();
        let order = order();
        let response =
            |signers: &[Pubkey]| response(&order, &taker, &inline_spl_token::ID, signers);
        let cosigner = [Pubkey::new_unique()];
        let two = [Pubkey::new_unique(), Pubkey::new_unique()];

        assert!(matches!(
            response(&cosigner).into_swap(&order, &taker, Fee::Standard),
            Err(Error::BadResponse(_))
        ));
        response(&cosigner)
            .into_swap(&order, &taker, Fee::Exempt)
            .unwrap();
        assert!(matches!(
            response(&two).into_swap(&order, &taker, Fee::Exempt),
            Err(Error::BadResponse(_))
        ));
    }
}
