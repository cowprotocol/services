use {
    crate::simulation_builder::{
        AccountOverrideRequest,
        Block,
        BuildError,
        EthCallInputs,
        ExecutionAmount,
        MergeConflict,
        Order,
        PriceEncoding,
        SimulationBuilder,
        Solver,
        WrapperConfig,
    },
    alloy_primitives::{Address, B256, Bytes, U256, keccak256},
    alloy_rpc_types::{
        TransactionRequest,
        state::{AccountOverride, StateOverride},
    },
    alloy_sol_types::SolCall,
    app_data::AppDataHash,
    balance_overrides::{BalanceOverrideRequest, BalanceOverriding},
    contracts::GPv2Settlement,
    derive_more::Debug,
    model::{
        interaction::InteractionData,
        order::{BuyTokenDestination, OrderData, OrderKind, SellTokenSource},
        signature::{Signature, SigningScheme},
    },
    number::serialization::HexOrDecimalU256,
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
};

pub type EncodedTrade = (
    U256,    // sellTokenIndex
    U256,    // buyTokenIndex
    Address, // receiver
    U256,    // sellAmount
    U256,    // buyAmount
    u32,     // validTo
    B256,    // appData
    U256,    // feeAmount
    U256,    // flags
    U256,    // executedAmount
    Bytes,   // signature
);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Interactions {
    pub pre: Vec<EncodedInteraction>,
    pub main: Vec<EncodedInteraction>,
    pub post: Vec<EncodedInteraction>,
}

impl Interactions {
    pub fn into_array(self) -> [Vec<EncodedInteraction>; 3] {
        [self.pre, self.main, self.post]
    }
}

impl IntoIterator for Interactions {
    type IntoIter = std::array::IntoIter<Vec<EncodedInteraction>, 3>;
    type Item = Vec<EncodedInteraction>;

    fn into_iter(self) -> Self::IntoIter {
        [self.pre, self.main, self.post].into_iter()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EncodedSettlement {
    pub tokens: Vec<Address>,
    pub clearing_prices: Vec<U256>,
    pub trades: Vec<EncodedTrade>,
    pub interactions: Interactions,
}

impl EncodedSettlement {
    pub fn into_settle_call(&self) -> Bytes {
        GPv2Settlement::GPv2Settlement::settleCall {
            tokens: self.tokens.clone(),
            clearingPrices: self.clearing_prices.clone(),
            interactions: self.interactions.clone().into_array().map(|interactions| {
                interactions
                    .into_iter()
                    .map(|i| GPv2Settlement::GPv2Interaction::Data {
                        target: i.0,
                        value: i.1,
                        callData: i.2.0.into(),
                    })
                    .collect()
            }),
            trades: self
                .trades
                .iter()
                .map(|t| GPv2Settlement::GPv2Trade::Data {
                    sellTokenIndex: t.0,
                    buyTokenIndex: t.1,
                    receiver: t.2,
                    sellAmount: t.3,
                    buyAmount: t.4,
                    validTo: t.5,
                    appData: t.6,
                    feeAmount: t.7,
                    flags: t.8,
                    executedAmount: t.9,
                    signature: t.10.clone(),
                })
                .collect(),
        }
        .abi_encode()
        .into()
    }
}

pub type EncodedInteraction = (
    Address, // target
    U256,    // value
    Bytes,   // callData
);

#[serde_as]
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JitOrder {
    pub buy_token: Address,
    pub sell_token: Address,
    #[serde_as(as = "HexOrDecimalU256")]
    pub sell_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub buy_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub executed_amount: U256,
    pub receiver: Address,
    pub valid_to: u32,
    pub app_data: AppDataHash,
    pub side: Side,
    pub partially_fillable: bool,
    pub sell_token_source: SellTokenSource,
    pub buy_token_destination: BuyTokenDestination,
    #[serde_as(as = "serde_ext::Hex")]
    pub signature: Vec<u8>,
    pub signing_scheme: SigningScheme,
}

#[serde_as]
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    Buy,
    Sell,
}

/// Creates the data which the smart contract's `decodeTrade` expects.
pub fn encode_trade(
    order: &OrderData,
    signature: &Signature,
    owner: Address,
    sell_token_index: usize,
    buy_token_index: usize,
    executed_amount: U256,
) -> EncodedTrade {
    (
        U256::from(sell_token_index),
        U256::from(buy_token_index),
        order.receiver.unwrap_or(Address::ZERO),
        order.sell_amount,
        order.buy_amount,
        order.valid_to,
        B256::new(order.app_data.0),
        order.fee_amount,
        order_flags(order, signature),
        executed_amount,
        Bytes::from(signature.encode_for_settlement(owner)),
    )
}

fn order_flags(order: &OrderData, signature: &Signature) -> U256 {
    let mut result = 0u8;
    // The kind is encoded as 1 bit in position 0.
    result |= match order.kind {
        OrderKind::Sell => 0b0,
        OrderKind::Buy => 0b1,
    };
    // The order fill kind is encoded as 1 bit in position 1.
    result |= (order.partially_fillable as u8) << 1;
    // The order sell token balance is encoded as 2 bits in position 2.
    result |= match order.sell_token_balance {
        SellTokenSource::Erc20 => 0b00,
        SellTokenSource::External => 0b10,
        SellTokenSource::Internal => 0b11,
    } << 2;
    // The order buy token balance is encoded as 1 bit in position 4.
    result |= match order.buy_token_balance {
        BuyTokenDestination::Erc20 => 0b0,
        BuyTokenDestination::Internal => 0b1,
    } << 4;
    // The signing scheme is encoded as a 2 bits in position 5.
    result |= match signature.scheme() {
        SigningScheme::Eip712 => 0b00,
        SigningScheme::EthSign => 0b01,
        SigningScheme::Eip1271 => 0b10,
        SigningScheme::PreSign => 0b11,
    } << 5;
    U256::from(result)
}

/// Data for a raw GPv2 interaction.
#[derive(Clone, PartialEq, Eq, Hash, Default, Serialize, Debug)]
pub struct Interaction {
    pub target: Address,
    pub value: U256,
    #[debug("{}", const_hex::encode_prefixed::<&[u8]>(data.as_ref()))]
    pub data: Vec<u8>,
}

pub trait InteractionEncoding {
    fn encode(&self) -> EncodedInteraction;
}

impl Interaction {
    pub fn to_interaction_data(&self) -> InteractionData {
        InteractionData {
            target: self.target,
            value: self.value,
            call_data: self.data.clone(),
        }
    }
}

impl InteractionEncoding for Interaction {
    fn encode(&self) -> EncodedInteraction {
        (
            self.target,
            self.value,
            Bytes::copy_from_slice(self.data.as_slice()),
        )
    }
}

impl InteractionEncoding for InteractionData {
    fn encode(&self) -> EncodedInteraction {
        (
            self.target,
            self.value,
            Bytes::copy_from_slice(&self.call_data),
        )
    }
}

impl From<InteractionData> for Interaction {
    fn from(interaction: InteractionData) -> Self {
        Self {
            target: interaction.target,
            value: interaction.value,
            data: interaction.call_data,
        }
    }
}

pub fn encode_interactions<'a, I>(
    interactions: impl IntoIterator<Item = &'a I>,
) -> Vec<EncodedInteraction>
where
    I: InteractionEncoding + 'a,
{
    interactions.into_iter().map(|i| i.encode()).collect()
}

#[derive(Clone, Debug)]
pub struct WrapperCall {
    pub address: Address,
    pub data: Bytes,
}

/// Encodes a settlement transaction that uses wrapper contracts.
///
/// Takes the base settlement calldata and wraps it in a wrappedSettleCall
/// with encoded wrapper metadata. Since wrappers are a chain, the wrapper
/// address to call is also processed by this function.
///
/// Returns (first_wrapper_address, wrapped_calldata)
pub fn encode_wrapper_settlement(
    wrappers: &[WrapperCall],
    settle_calldata: Bytes,
) -> Option<(Address, Bytes)> {
    if wrappers.is_empty() {
        return None;
    };
    let wrapper_data = encode_wrapper_data(wrappers);

    // Create wrappedSettleCall
    let calldata = contracts::ICowWrapper::ICowWrapper::wrappedSettleCall {
        settleData: settle_calldata,
        wrapperData: wrapper_data,
    }
    .abi_encode();

    Some((wrappers[0].address, calldata.into()))
}

/// Encodes wrapper metadata for wrapper settlement calls.
/// As wrappers are called, each wrapper reads from wrapper calldata and
/// consumes only their needed portion (however much data that is). Once wrapper
/// is ready to call the settlement contract (or downstream wrapper) it calls
/// the _internalSettle function provided in the CowWrapper abstract contract
///
/// Generally wrappers are encoded with a pair of Address (20 bytes) and then
/// calldata (u16 length + data itself).
///
/// Since the first wrapper's address is the target of the transaction, it is
/// not encoded.
///
/// The encoding format thus is:
/// - The calldata of the first wrapper.
/// - The address and calldata for each subsequent wrapper
///
/// Example: Encoding of 2 wrapper calls, the wrappers are named A, B and are
/// called in the order A -> B
///
/// | A calldata length | A calldata | B address | B calldata length | B calldata |
/// | u16               | &[u8]      | [u8; 20]  | u16               | &[u8]      |
///
/// Any additional wrappers will follow the same scheme: (address, length,
/// calldata)
///
/// More information about wrapper encoding:
/// https://docs.cow.fi/cow-protocol/integrate/wrappers#manual-encoding
pub fn encode_wrapper_data(wrappers: &[WrapperCall]) -> Bytes {
    let mut wrapper_data = Vec::new();

    for (index, w) in wrappers.iter().enumerate() {
        // Skip first wrapper's address (it's the transaction target)
        if index != 0 {
            wrapper_data.extend(w.address.as_slice());
        }

        // Encode data length as u16 in native endian, then the data itself
        wrapper_data.extend((w.data.len() as u16).to_be_bytes().to_vec());
        wrapper_data.extend(w.data.clone());
    }

    wrapper_data.into()
}

pub(crate) async fn finish_simulation_builder(
    mut builder: SimulationBuilder,
) -> Result<EthCallInputs, BuildError> {
    if builder.orders.is_empty() {
        return Err(BuildError::NoOrder);
    }

    let block = match builder.block {
        Block::Latest => builder.simulator.0.current_block.borrow().number,
        Block::Number(n) => n,
    };

    let executed_amounts = futures::future::try_join_all(
        builder
            .orders
            .iter()
            .map(|o| executed_amount(&builder, o, block)),
    )
    .await?;

    // Each order occupies exactly 2 consecutive slots in the token/price
    // vectors: [2*i] = sell_token, [2*i+1] = buy_token.
    // This lets every order be encoded independently without requiring a shared
    // global token list.
    let n = builder.orders.len();
    let mut tokens = Vec::with_capacity(n * 2);
    let mut clearing_prices = Vec::with_capacity(n * 2);
    for order in &builder.orders {
        let (sell_price, buy_price) = match &order.price_encoding {
            PriceEncoding::LimitPrice => (order.data.buy_amount, order.data.sell_amount),
            PriceEncoding::Custom {
                sell_price,
                buy_price,
            } => (*sell_price, *buy_price),
        };
        tokens.push(order.data.sell_token);
        tokens.push(order.data.buy_token);
        clearing_prices.push(sell_price);
        clearing_prices.push(buy_price);
    }

    if builder.provide_buy_tokens {
        let settlement = *builder.simulator.0.settlement.address();
        for (i, (order, &exec)) in builder.orders.iter().zip(&executed_amounts).enumerate() {
            let sell_price = clearing_prices[2 * i];
            let buy_price = clearing_prices[2 * i + 1];
            let amount = match order.data.kind {
                OrderKind::Sell => sell_price
                    .saturating_mul(exec)
                    .checked_div(buy_price)
                    .unwrap_or(U256::MAX),
                OrderKind::Buy => exec,
            }
            // give 1 wei extra to avoid issues with rounding divisions
            .saturating_add(U256::ONE);
            builder
                .account_override_requests
                .push(AccountOverrideRequest::Balance {
                    holder: settlement,
                    token: order.data.buy_token,
                    amount,
                });
        }
    }

    // Encode every order as a trade, then collect all their interactions.
    let mut trades = Vec::with_capacity(n);
    for (i, (order, exec)) in builder.orders.iter().zip(&executed_amounts).enumerate() {
        trades.push(encode_trade(
            &order.data,
            &order.signature,
            order.owner,
            2 * i,
            2 * i + 1,
            *exec,
        ));
    }

    let settlement = EncodedSettlement {
        tokens,
        clearing_prices,
        trades,
        interactions: Interactions {
            pre: encode_interactions(&builder.pre_interactions),
            main: encode_interactions(&builder.main_interactions),
            post: encode_interactions(&builder.post_interactions),
        },
    };

    let settle_calldata = {
        let mut bytes = settlement.into_settle_call().to_vec();
        if let Some(id) = builder.auction_id {
            bytes.extend_from_slice(&id.to_be_bytes());
        }
        bytes.into()
    };

    let wrapper = builder.wrapper;
    let (to, input) = match wrapper {
        WrapperConfig::Custom(wrappers) if !wrappers.is_empty() => {
            encode_wrapper_settlement(&wrappers, settle_calldata).expect("wrappers is non-empty")
        }
        WrapperConfig::Flashloan(loans) => {
            let calldata = contracts::FlashLoanRouter::FlashLoanRouter::flashLoanAndSettleCall {
                loans: loans
                    .into_iter()
                    .map(|l| contracts::FlashLoanRouter::LoanRequest::Data {
                        amount: l.amount,
                        borrower: l.borrower,
                        lender: l.lender,
                        token: l.token,
                    })
                    .collect(),
                settlement: settle_calldata,
            }
            .abi_encode()
            .into();
            (builder.simulator.0.flash_loan_router, calldata)
        }
        _ => (*builder.simulator.0.settlement.address(), settle_calldata),
    };

    let from = match builder.solver {
        Some(Solver::OriginUnaltered(addr)) => addr,
        Some(Solver::Fake(opt)) => {
            let addr = opt.unwrap_or_else(Address::random);
            builder
                .account_override_requests
                .push(AccountOverrideRequest::SufficientEthBalance(addr));
            builder
                .account_override_requests
                .push(AccountOverrideRequest::AuthenticateAsSolver(addr));
            addr
        }
        None => return Err(BuildError::NoSolver),
    };
    let state_overrides = build_final_state_overrides(
        builder.account_override_requests,
        builder.simulator.0.balance_overrides.as_ref(),
        builder.simulator.0.authenticator,
    )
    .await;

    Ok(EthCallInputs {
        request: TransactionRequest {
            from: Some(from),
            to: Some(to.into()),
            input: input.into(),
            gas: Some(builder.simulator.0.max_gas_limit),
            ..Default::default()
        },
        state_overrides,
        block,
        simulator: builder.simulator,
    })
}

/// Computes the exact amount the order should be filled with
/// based on the configured [`ExecutionAmount`].
/// If `Remaining` is configured we look up how much the order
/// was already filled in the settlement contract and deduct
/// that from the full amount.
async fn executed_amount(
    builder: &SimulationBuilder,
    order: &Order,
    block: u64,
) -> Result<U256, BuildError> {
    let full = match order.data.kind {
        OrderKind::Sell => order.data.sell_amount,
        OrderKind::Buy => order.data.buy_amount,
    };

    Ok(match order.executed_amount {
        ExecutionAmount::Full => full,
        ExecutionAmount::Explicit(amount) => amount,
        ExecutionAmount::Remaining => {
            let uid = order
                .data
                .uid(&builder.simulator.0.domain_separator, order.owner);
            let filled_amount = builder
                .simulator
                .0
                .settlement
                .filledAmount(Bytes::from(uid.0))
                .block(block.into())
                .call()
                .await
                .map_err(|err| BuildError::FilledAmountQuery(err.into()))?;
            full.saturating_sub(filled_amount)
        }
    })
}

/// Resolves all [`AccountOverrideRequest`]s concurrently on a best-effort
/// basis. Failures are logged and the corresponding override is skipped rather
/// than aborting the whole build.
async fn build_final_state_overrides(
    requests: Vec<AccountOverrideRequest>,
    balance_overrides: &dyn BalanceOverriding,
    authenticator: Address,
) -> StateOverride {
    let futures = requests.into_iter().map(|request| async move {
        match request {
            AccountOverrideRequest::SufficientEthBalance(addr) => Some((
                addr,
                AccountOverride::default().with_balance(U256::MAX / U256::from(2)),
            )),
            AccountOverrideRequest::AuthenticateAsSolver(addr) => {
                // GPv2AllowListAuthentication stores `mapping(address => bool) managers`
                // at storage slot 1. Solidity mapping key: keccak256(address_padded ++
                // slot_padded).
                // <https://github.com/cowprotocol/contracts/blob/main/src/contracts/GPv2AllowListAuthentication.sol#L22>
                let mut buf = [0u8; 64];
                buf[12..32].copy_from_slice(addr.as_slice());
                buf[32..64].copy_from_slice(&U256::ONE.to_be_bytes::<32>());
                let slot = keccak256(buf);
                Some((
                    authenticator,
                    AccountOverride::default()
                        .with_state_diff(std::iter::once((slot, B256::with_last_byte(1)))),
                ))
            }
            AccountOverrideRequest::Balance {
                holder,
                token,
                amount,
            } => {
                let result = balance_overrides
                    .state_override(BalanceOverrideRequest {
                        token,
                        holder,
                        amount,
                    })
                    .await;
                if result.is_none() {
                    tracing::warn!(%token, %holder, "failed to compute balance state override, skipping");
                }
                result
            }
            AccountOverrideRequest::Code { account, code } => Some((
                account,
                AccountOverride {
                    code: Some(code),
                    ..Default::default()
                },
            )),
            AccountOverrideRequest::Custom { account, state } => Some((account, state)),
        }
    });

    let mut state_overrides = StateOverride::default();
    for (address, account_override) in futures::future::join_all(futures)
        .await
        .into_iter()
        .flatten()
    {
        if let Err(err) = apply_account_override(&mut state_overrides, address, account_override) {
            tracing::warn!(?err, %address, "conflicting state overrides for address, skipping");
        }
    }
    state_overrides
}

/// Merges `new` into `existing` field by field.
///
/// Returns [`MergeConflict`] if both overrides write the same field.
/// Non-conflicting `state_diff` entries are combined into a single map.
fn merge_account_override(
    existing: &mut AccountOverride,
    new: AccountOverride,
) -> Result<(), MergeConflict> {
    if new.balance.is_some() {
        if existing.balance.is_some() {
            return Err(MergeConflict::Balance);
        }
        existing.balance = new.balance;
    }
    if new.nonce.is_some() {
        if existing.nonce.is_some() {
            return Err(MergeConflict::Nonce);
        }
        existing.nonce = new.nonce;
    }
    if new.code.is_some() {
        if existing.code.is_some() {
            return Err(MergeConflict::Code);
        }
        existing.code = new.code;
    }
    match (new.state, new.state_diff) {
        (Some(new_state), None) => {
            if existing.state.is_some() {
                return Err(MergeConflict::State);
            }
            if existing.state_diff.is_some() {
                return Err(MergeConflict::StateAndStateDiff);
            }
            existing.state = Some(new_state);
        }
        (None, Some(new_diff)) => {
            if existing.state.is_some() {
                return Err(MergeConflict::StateAndStateDiff);
            }
            match &mut existing.state_diff {
                None => existing.state_diff = Some(new_diff),
                Some(existing_diff) => {
                    for (slot, value) in new_diff {
                        if existing_diff.contains_key(&slot) {
                            return Err(MergeConflict::StateDiffSlot(slot));
                        }
                        existing_diff.insert(slot, value);
                    }
                }
            }
        }
        (None, None) => {}
        // alloy does not allow both simultaneously, treat as incompatible
        (Some(_), Some(_)) => return Err(MergeConflict::StateAndStateDiff),
    }
    Ok(())
}

/// Applies `new` to the override map for `address`.
///
/// If `address` already has an entry, the overrides are merged via
/// [`merge_account_override`]. Returns an error on conflict.
fn apply_account_override(
    overrides: &mut StateOverride,
    address: Address,
    new: AccountOverride,
) -> Result<(), MergeConflict> {
    if let Some(existing) = overrides.get_mut(&address) {
        merge_account_override(existing, new)
    } else {
        overrides.insert(address, new);
        Ok(())
    }
}
