use {
    super::{BadTokenDetecting, TokenQuality, token_owner_finder::TokenOwnerFinding},
    crate::{ethrpc::Web3, trace_many},
    alloy::{
        primitives::{Address, U256, keccak256},
        rpc::{
            json_rpc::ErrorPayload,
            types::{
                TransactionRequest,
                trace::parity::{TraceOutput, TraceResults},
            },
        },
        signers::local::PrivateKeySigner,
        sol_types::SolCall,
        transports::{RpcError, TransportErrorKind},
    },
    anyhow::{Context, Result, bail, ensure},
    contracts::alloy::ERC20,
    model::interaction::InteractionData,
    std::{cmp, sync::Arc},
    tracing::instrument,
};

const METHOD_NOT_FOUND_CODE: i64 = -32601;

/// Detects whether a token is "bad" (works in unexpected ways that are
/// problematic for solving) by simulating several transfers of a token. To find
/// an initial address to transfer from we use the amm pair providers.
/// Tokens are bad if:
/// - we cannot find an amm pool of the token to one of the base tokens
/// - transfer into the settlement contract or back out fails
/// - a transfer loses total balance
pub struct TraceCallDetector {
    inner: TraceCallDetectorRaw,
    finder: Arc<dyn TokenOwnerFinding>,
}

#[async_trait::async_trait]
impl BadTokenDetecting for TraceCallDetector {
    #[instrument(skip_all)]
    async fn detect(&self, token: Address) -> Result<TokenQuality> {
        let quality = self.detect_impl(token).await?;
        tracing::debug!(?token, ?quality, "determined token quality");
        Ok(quality)
    }
}

impl TraceCallDetector {
    pub fn new(web3: Web3, settlement: Address, finder: Arc<dyn TokenOwnerFinding>) -> Self {
        Self {
            inner: TraceCallDetectorRaw::new(web3, settlement),
            finder,
        }
    }

    async fn detect_impl(&self, token: Address) -> Result<TokenQuality> {
        // Arbitrary amount that is large enough that small relative fees should be
        // visible.
        const MIN_AMOUNT: u64 = 100_000;
        let (take_from, amount) = match self
            .finder
            .find_owner(token, U256::from(MIN_AMOUNT))
            .await
            .context("find_owner")?
        {
            Some((address, balance)) => {
                // Don't use the full balance, but instead a portion of it. This
                // makes the trace call less racy and prone to the transfer
                // failing because of a balance change from one block to the
                // next. This can happen because of either:
                // - Block propagation - the trace_callMany is handled by a node that is 1 block
                //   in the past
                // - New block observed - the trace_callMany is executed on a block that came in
                //   since we read the balance
                let amount = cmp::max(balance / U256::from(2), U256::from(MIN_AMOUNT));

                tracing::debug!(?token, ?address, ?amount, "found owner");
                (address, amount)
            }
            None => {
                return Ok(TokenQuality::bad(format!(
                    "Could not find on chain source of the token with at least {MIN_AMOUNT} \
                     balance.",
                )));
            }
        };
        self.inner
            .test_transfer(take_from, token, amount, &[])
            .await
    }
}

/// Detects whether a token is "bad" (works in unexpected ways that are
/// problematic for solving) by simulating several transfers of a token.
#[derive(Debug, Clone)]
pub struct TraceCallDetectorRaw {
    pub web3: Web3,
    pub settlement_contract: Address,
}

impl TraceCallDetectorRaw {
    pub fn new(web3: Web3, settlement: Address) -> Self {
        Self {
            web3,
            settlement_contract: settlement,
        }
    }

    pub async fn test_transfer(
        &self,
        take_from: Address,
        token: Address,
        amount: U256,
        pre_interactions: &[InteractionData],
    ) -> Result<TokenQuality> {
        let mut request: Vec<_> = pre_interactions
            .iter()
            .map(|i| {
                TransactionRequest::default()
                    .to(i.target)
                    .value(i.value)
                    .input(i.call_data.clone().into())
            })
            .collect();
        // We transfer the full available amount of the token from the amm pool into the
        // settlement contract and then to an arbitrary address.
        // Note that gas use can depend on the recipient because for the standard
        // implementation sending to an address that does not have any balance
        // yet (implicitly 0) causes an allocation.
        request.append(&mut self.create_trace_request(token, amount, take_from));
        let traces = match trace_many::trace_many(&self.web3, request).await {
            Ok(result) => result,
            Err(RpcError::UnsupportedFeature(err)) => {
                tracing::warn!(error = ?err, "node does not support trace_callMany");
                return Ok(TokenQuality::Good);
            }
            Err(RpcError::ErrorResp(ErrorPayload {
                code: METHOD_NOT_FOUND_CODE,
                message,
                ..
            })) => {
                tracing::warn!(error = %message, "node does not support trace_callMany");
                return Ok(TokenQuality::Good);
            }
            Err(RpcError::Transport(TransportErrorKind::HttpError(err))) if err.status == 400 => {
                tracing::warn!(
                    error=?err,
                    "unable to perform trace call with configured node, assume good quality"
                );
                return Ok(TokenQuality::Good);
            }
            Err(e) => {
                return Err(e).context("trace_many");
            }
        };
        let relevant_traces = &traces[pre_interactions.len()..];
        Self::handle_response(relevant_traces, amount, take_from)
    }

    // For the out transfer we use an arbitrary address without balance to detect
    // tokens that usually apply fees but not if the the sender or receiver is
    // specifically exempt like their own uniswap pools.
    fn arbitrary_recipient() -> Address {
        PrivateKeySigner::from_bytes(&keccak256(b"moo"))
            .unwrap()
            .address()
    }

    fn create_trace_request(
        &self,
        token: Address,
        amount: U256,
        take_from: Address,
    ) -> Vec<TransactionRequest> {
        let mut requests = Vec::new();
        let recipient = Self::arbitrary_recipient();
        let settlement_contract = self.settlement_contract;

        // 0
        let calldata = ERC20::ERC20::balanceOfCall {
            account: settlement_contract,
        }
        .abi_encode();
        requests.push(
            TransactionRequest::default()
                .to(token)
                .input(calldata.into()),
        );
        // 1
        let calldata = ERC20::ERC20::transferCall {
            recipient: settlement_contract,
            amount,
        }
        .abi_encode();
        requests.push(
            TransactionRequest::default()
                .from(take_from)
                .to(token)
                .input(calldata.into()),
        );
        // 2
        let calldata = ERC20::ERC20::balanceOfCall {
            account: settlement_contract,
        }
        .abi_encode();
        requests.push(
            TransactionRequest::default()
                .to(token)
                .input(calldata.into()),
        );
        // 3
        let calldata = ERC20::ERC20::balanceOfCall { account: recipient }.abi_encode();
        requests.push(
            TransactionRequest::default()
                .to(token)
                .input(calldata.into()),
        );
        // 4
        let calldata = ERC20::ERC20::transferCall { recipient, amount }.abi_encode();
        requests.push(
            TransactionRequest::default()
                .from(self.settlement_contract)
                .to(token)
                .input(calldata.into()),
        );
        // 5
        let calldata = ERC20::ERC20::balanceOfCall {
            account: settlement_contract,
        }
        .abi_encode();
        requests.push(
            TransactionRequest::default()
                .to(token)
                .input(calldata.into()),
        );
        // 6
        let calldata = ERC20::ERC20::balanceOfCall { account: recipient }.abi_encode();
        requests.push(
            TransactionRequest::default()
                .to(token)
                .input(calldata.into()),
        );

        // 7
        let calldata = ERC20::ERC20::approveCall {
            spender: recipient,
            amount: alloy::primitives::U256::MAX,
        }
        .abi_encode();
        requests.push(
            TransactionRequest::default()
                .from(self.settlement_contract)
                .to(token)
                .input(calldata.into()),
        );

        requests
    }

    fn handle_response(
        traces: &[TraceResults],
        amount: U256,
        take_from: Address,
    ) -> Result<TokenQuality> {
        ensure!(traces.len() == 8, "unexpected number of traces");

        let gas_in = match ensure_transaction_ok_and_get_gas(&traces[1])? {
            Ok(gas) => gas,
            Err(reason) => {
                return Ok(TokenQuality::bad(format!(
                    "Transfer of token from on chain source {take_from:?} into settlement \
                     contract failed: {reason}"
                )));
            }
        };
        let arbitrary = Self::arbitrary_recipient();
        let gas_out = match ensure_transaction_ok_and_get_gas(&traces[4])? {
            Ok(gas) => gas,
            Err(reason) => {
                return Ok(TokenQuality::bad(format!(
                    "Transfer token out of settlement contract to arbitrary recipient \
                     {arbitrary:?} failed: {reason}",
                )));
            }
        };

        let message = "\
            Failed to decode the token's balanceOf response because it did not \
            return 32 bytes. A common cause of this is a bug in the Vyper \
            smart contract compiler. See \
            https://github.com/cowprotocol/services/pull/781 for more \
            information.\
        ";
        let bad = TokenQuality::Bad {
            reason: message.to_string(),
        };
        let balance_before_in = match u256_from_be_bytes_strict(&traces[0].output) {
            Some(balance) => balance,
            None => return Ok(bad),
        };
        let balance_after_in = match u256_from_be_bytes_strict(&traces[2].output) {
            Some(balance) => balance,
            None => return Ok(bad),
        };
        let balance_after_out = match u256_from_be_bytes_strict(&traces[5].output) {
            Some(balance) => balance,
            None => return Ok(bad),
        };
        let balance_recipient_before = match u256_from_be_bytes_strict(&traces[3].output) {
            Some(balance) => balance,
            None => return Ok(bad),
        };
        let balance_recipient_after = match u256_from_be_bytes_strict(&traces[6].output) {
            Some(balance) => balance,
            None => return Ok(bad),
        };

        tracing::debug!(%amount, %balance_before_in, %balance_after_in, %balance_after_out);

        let computed_balance_after_in = match balance_before_in.checked_add(amount) {
            Some(amount) => amount,
            None => {
                return Ok(TokenQuality::bad(format!(
                    "Transferring {amount} into settlement contract would overflow its balance."
                )));
            }
        };
        // Allow for a small discrepancy (1 wei) in the balance after the transfer which
        // may come from rounding discrepancies in tokens that track balances
        // with "shares" (e.g. eUSD).
        if balance_after_in < computed_balance_after_in.saturating_sub(U256::ONE) {
            return Ok(TokenQuality::bad(format!(
                "Transferring {amount} into settlement contract was expected to result in a \
                 balance of {computed_balance_after_in} but actually resulted in \
                 {balance_after_in}. A common cause for this is that the token takes a fee on \
                 transfer."
            )));
        }
        if balance_after_out != balance_before_in {
            return Ok(TokenQuality::bad(format!(
                "Transferring {amount} out of settlement contract was expected to result in the \
                 original balance of {balance_before_in} but actually resulted in \
                 {balance_after_out}."
            )));
        }
        let computed_balance_recipient_after = match balance_recipient_before.checked_add(amount) {
            Some(amount) => amount,
            None => {
                return Ok(TokenQuality::bad(format!(
                    "Transferring {amount} into arbitrary recipient {arbitrary:?} would overflow \
                     its balance."
                )));
            }
        };
        // Allow for a small discrepancy (1 wei) in the balance after the transfer
        // which may come from rounding discrepancies in tokens that track
        // balances with "shares" (e.g. eUSD).
        if computed_balance_recipient_after < balance_recipient_after.saturating_sub(U256::ONE) {
            return Ok(TokenQuality::bad(format!(
                "Transferring {amount} into arbitrary recipient {arbitrary:?} was expected to \
                 result in a balance of {computed_balance_recipient_after} but actually resulted \
                 in {balance_recipient_after}. A common cause for this is that the token takes a \
                 fee on transfer."
            )));
        }

        if let Err(err) = ensure_transaction_ok_and_get_gas(&traces[7])? {
            return Ok(TokenQuality::bad(format!(
                "Approval of U256::MAX failed: {err}"
            )));
        }

        let _gas_per_transfer = (gas_in + gas_out) / U256::from(2);
        Ok(TokenQuality::Good)
    }
}

/// The outer result signals communication failure with the node.
/// The inner result is Ok(gas_price) or Err if the transaction failed.
fn ensure_transaction_ok_and_get_gas(trace: &TraceResults) -> Result<Result<U256, String>> {
    let first = trace.trace.first().context("expected at least one trace")?;
    if let Some(error) = &first.error {
        return Ok(Err(format!("transaction failed: {error}")));
    }
    let call_result = match &first.result {
        Some(TraceOutput::Call(call)) => call,
        _ => bail!("no error but also no call result"),
    };
    Ok(Ok(U256::from(call_result.gas_used)))
}

/// Decodes a `U256` from a big-endian encoded slice.
/// The slice's length MUST be 32 bytes.
fn u256_from_be_bytes_strict(b: &[u8]) -> Option<U256> {
    if b.len() != 32 {
        return None;
    }
    U256::try_from_be_slice(b)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            bad_token::token_owner_finder::{
                TokenOwnerFinder,
                blockscout::BlockscoutTokenOwnerFinder,
                liquidity::{
                    BalancerVaultFinder,
                    FeeValues,
                    UniswapLikePairProviderFinder,
                    UniswapV3Finder,
                },
                solvers::{
                    solver_api::SolverConfiguration,
                    solver_finder::AutoUpdatingSolverTokenOwnerFinder,
                },
            },
            sources::{BaselineSource, uniswap_v2},
        },
        alloy::{
            primitives::{Bytes, address},
            providers::Provider,
            rpc::types::trace::parity::{
                Action,
                CallAction,
                CallOutput,
                CallType,
                TransactionTrace,
            },
        },
        chain::Chain,
        contracts::alloy::{BalancerV2Vault, GPv2Settlement, IUniswapV3Factory},
        ethrpc::Web3,
        std::{env, time::Duration},
    };

    #[test]
    fn handle_response_ok() {
        let traces = &[
            TraceResults {
                output: U256::ZERO.to_be_bytes::<32>().to_vec().into(),
                trace: vec![],
                vm_trace: None,
                state_diff: None,
            },
            TraceResults {
                output: Default::default(),
                trace: vec![TransactionTrace {
                    trace_address: Vec::new(),
                    subtraces: 0,
                    action: Action::Call(CallAction {
                        from: Address::ZERO,
                        to: Address::ZERO,
                        value: U256::ZERO,
                        gas: 0,
                        input: Bytes::new(),
                        call_type: CallType::None,
                    }),
                    result: Some(TraceOutput::Call(CallOutput {
                        gas_used: 1,
                        output: Bytes::new(),
                    })),
                    error: None,
                }],
                vm_trace: None,
                state_diff: None,
            },
            TraceResults {
                output: U256::ONE.to_be_bytes::<32>().to_vec().into(),
                trace: vec![],
                vm_trace: None,
                state_diff: None,
            },
            TraceResults {
                output: U256::ZERO.to_be_bytes::<32>().to_vec().into(),
                trace: vec![],
                vm_trace: None,
                state_diff: None,
            },
            TraceResults {
                output: Default::default(),
                trace: vec![TransactionTrace {
                    trace_address: Vec::new(),
                    subtraces: 0,
                    action: Action::Call(CallAction {
                        from: Address::ZERO,
                        to: Address::ZERO,
                        value: U256::ZERO,
                        gas: 0,
                        input: Bytes::new(),
                        call_type: CallType::None,
                    }),
                    result: Some(TraceOutput::Call(CallOutput {
                        gas_used: 3,
                        output: Bytes::new(),
                    })),
                    error: None,
                }],
                vm_trace: None,
                state_diff: None,
            },
            TraceResults {
                output: U256::ZERO.to_be_bytes::<32>().to_vec().into(),
                trace: vec![],
                vm_trace: None,
                state_diff: None,
            },
            TraceResults {
                output: U256::ONE.to_be_bytes::<32>().to_vec().into(),
                trace: vec![],
                vm_trace: None,
                state_diff: None,
            },
            TraceResults {
                output: Default::default(),
                trace: vec![TransactionTrace {
                    trace_address: Vec::new(),
                    subtraces: 0,
                    action: Action::Call(CallAction {
                        from: Address::ZERO,
                        to: Address::ZERO,
                        value: U256::ZERO,
                        gas: 0,
                        input: Bytes::new(),
                        call_type: CallType::None,
                    }),
                    result: Some(TraceOutput::Call(CallOutput {
                        gas_used: 1,
                        output: Bytes::new(),
                    })),
                    error: None,
                }],
                vm_trace: None,
                state_diff: None,
            },
        ];

        let result =
            TraceCallDetectorRaw::handle_response(traces, U256::ONE, Address::ZERO).unwrap();
        let expected = TokenQuality::Good;
        assert_eq!(result, expected);
    }

    // cargo test -p shared mainnet_tokens -- --nocapture --ignored
    #[tokio::test]
    #[ignore]
    async fn mainnet_tokens() {
        // observe::tracing::initialize("orderbook::bad_token=debug,
        // shared::transport=debug", tracing::level_filters::LevelFilter::OFF);
        let web3 = Web3::new_from_env();
        let version = web3.provider.get_chain_id().await.unwrap().to_string();

        let base_tokens = &[
            testlib::tokens::WETH,
            testlib::tokens::DAI,
            testlib::tokens::USDC,
            testlib::tokens::USDT,
            testlib::tokens::COMP,
            testlib::tokens::MKR,
            testlib::tokens::WBTC,
        ];

        // tokens from our deny list
        let bad_tokens = &[
            address!("0027449Bf0887ca3E431D263FFDeFb244D95b555"), // All balances are maxuint256
            address!("0189d31f6629c359007f72b8d5ec8fa1c126f95c"),
            address!("01995786f1435743c42b7f2276c496a610b58612"),
            address!("072c46f392e729c1f0d92a307c2c6dba06b5d078"),
            address!("074545177a36ab81aac783211f25e14f1ed03c2b"),
            address!("07be1ead7aebee544618bdc688fa3cff09857c32"),
            address!("0858a26055d6584e5b47bbecf7f7e8cbc390995b"),
            address!("0aacfbec6a24756c20d41914f2caba817c0d8521"),
            address!("0ba45a8b5d5575935b8158a88c631e9f9c95a2e5"),
            address!("0e69d0a2bbb30abcb7e5cfea0e4fde19c00a8d47"),
            address!("1016f3c0a1939fa27538339da7e2a300031b6f37"),
            address!("106552c11272420aad5d7e94f8acab9095a6c952"),
            address!("106d3c66d22d2dd0446df23d7f5960752994d600"),
            address!("1337DEF18C680aF1f9f45cBcab6309562975b1dD"),
            address!("1341a2257fa7b770420ef70616f888056f90926c"),
            address!("1426cc6d52d1b14e2b3b1cb04d57ea42b39c4c7c"),
            address!("14dd7ebe6cb084cb73ef377e115554d47dc9d61e"),
            address!("15874d65e649880c2614e7a480cb7c9a55787ff6"),
            address!("1681bcb589b3cfcf0c0616b0ce9b19b240643dc1"),
            address!("18bdfc80b97cb97f6b466cce967849ce9cd9d58c"),
            address!("1b9baf2a3edea91ee431f02d449a1044d5726669"),
            address!("2129ff6000b95a973236020bcd2b2006b0d8e019"),
            address!("239dc02a28a0774738463e06245544a72745d5c5"),
            address!("251457b7c5d85251ca1ab384361c821330be2520"),
            address!("25a1de1c3ee658fe034b8914a1d8d34110423af8"),
            address!("26a79bd709a7ef5e5f747b8d8f83326ea044d8cc"),
            address!("289d5488ab09f43471914e572ec9e3651c735af2"),
            address!("298d492e8c1d909d3f63bc4a36c66c64acb3d695"),
            address!("2b1fe2cea92436e8c34b7c215af66aaa2932a8b2"),
            address!("31acf54fae6166dc2f90c4d6f20d379965e96bc1"),
            address!("32c868f6318d6334b2250f323d914bc2239e4eee"),
            address!("33f128394af03db639107473e52d84ff1290499e"),
            address!("37611b28aca5673744161dc337128cfdd2657f69"),
            address!("389999216860ab8e0175387a0c90e5c52522c945"),
            address!("39b8523fa094b0dc045e2c3e5dff34b3f2ca6220"),
            address!("3a6fe4c752eb8d571a660a776be4003d619c30a3"),
            address!("3a9fff453d50d4ac52a6890647b823379ba36b9e"),
            address!("3ea50b7ef6a7eaf7e966e2cb72b519c16557497c"),
            address!("3fca773d13f831753ec3ae9f39ad4a6814ebb695"),
            address!("41933422dc4a1cb8c822e06f12f7b52fa5e7e094"),
            address!("45734927fa2f616fbe19e65f42a0ef3d37d1c80a"),
            address!("45804880de22913dafe09f4980848ece6ecbaf78"),
            address!("48be867b240d2ffaff69e0746130f2c027d8d3d2"),
            address!("4a6be56a211a4c4e0dd4474d524138933c17f3e3"),
            address!("4b86e0295e7d32433ffa6411b82b4f4e56a581e1"),
            address!("4ba6ddd7b89ed838fed25d208d4f644106e34279"),
            address!("4bae380b5d762d543d426331b8437926443ae9ec"),
            address!("4bcddfcfa8cb923952bcf16644b36e5da5ca3184"),
            address!("4c9d5672ae33522240532206ab45508116daf263"),
            address!("4F9254C83EB525f9FCf346490bbb3ed28a81C667"),
            address!("4fab740779c73aa3945a5cf6025bf1b0e7f6349c"),
            address!("51d3e4c0b2c83e62f5d517d250b3e856897d2052"),
            address!("53ba22cb4e5e9c1be0d73913764f572192a71aca"),
            address!("56de8bc61346321d4f2211e3ac3c0a7f00db9b76"),
            address!("576097fa17e1f702bb9167f0f08f2ea0898a3ea5"),
            address!("577e7f9fa80ab33e87a01b701114257c8d9455a8"),
            address!("586c680e9a6d21b81ebecf46d78844dab7b3bcf9"),
            address!("5d0fa08aeb173ade44b0cf7f31d506d8e04f0ac8"),
            address!("62359ed7505efc61ff1d56fef82158ccaffa23d7"),
            address!("63d0eea1d7c0d1e89d7e665708d7e8997c0a9ed6"),
            address!("66d31def9c47b62184d7f57175eed5b5d9b7f038"),
            address!("671ab077497575dcafb68327d2d2329207323e74"),
            address!("685aea4f02e39e5a5bb7f7117e88db1151f38364"),
            address!("68e0a48d3bff6633a31d1d100b70f93c3859218b"),
            address!("69692d3345010a207b759a7d1af6fc7f38b35c5e"),
            address!("6a00b86e30167f73e38be086081b80213e8266aa"),
            address!("6b8e77d3db1faa17f7b24c24242b6a1eb5008a16"),
            address!("6e10aacb89a28d6fa0fe68790777fec7e7f01890"),
            address!("6fcb6408499a7c0f242e32d77eb51ffa1dd28a7e"),
            address!("714599f7604144a3fe1737c440a70fc0fd6503ea"),
            address!("75fef397d74a2d11b64e6915cd847c1e7f8e5520"),
            address!("76851a93977bea9264c32255b6457882035c7501"),
            address!("79ba92dda26fce15e1e9af47d5cfdfd2a093e000"),
            address!("7f0f118d083d5175ab9d2d34c4c8fa4f43c3f47b"),
            address!("7ff4169a6b5122b664c51c95727d87750ec07c84"),
            address!("801ea8c463a776e85344c565e355137b5c3324cd"),
            address!("88ef27e69108b2633f8e1c184cc37940a075cc02"),
            address!("8c7424c3000942e5a93de4a01ce2ec86c06333cb"),
            address!("8eb24319393716668d768dcec29356ae9cffe285"),
            address!("910524678c0b1b23ffb9285a81f99c29c11cbaed"),
            address!("910985ffa7101bf5801dd2e91555c465efd9aab3"),
            address!("925f2c11b99c1a4c46606898ee91ed3d450cfeda"),
            address!("944eee930933be5e23b690c8589021ec8619a301"),
            address!("94987bc8aa5f36cb2461c190134929a29c3df726"),
            address!("97ad070879be5c31a03a1fe7e35dfb7d51d0eef1"),
            address!("97b65710d03e12775189f0d113202cc1443b0aa2"),
            address!("98ecf3d8e21adaafe16c00cc3ff681e72690278b"),
            address!("99043bb680ab9262c7b2ac524e00b215efb7db9b"),
            address!("99ddddd8dfe33905338a073047cfad72e6833c06"),
            address!("9a514389172863f12854ad40090aa4b928028542"),
            address!("9af15d7b8776fa296019979e70a5be53c714a7ec"),
            address!("9ea3b5b4ec044b70375236a281986106457b20ef"),
            address!("9f41da75ab2b8c6f0dcef7173c4bf66bd4f6b36a"),
            address!("a03f1250aa448226ed4066d8d1722ddd8b51df59"),
            address!("a2b4c0af19cc16a6cfacce81f192b024d625817d"),
            address!("a3e059c0b01f07f211c85bf7b4f1d907afb011df"),
            address!("a5959e9412d27041194c3c3bcbe855face2864f7"),
            address!("a9a8377287ea9c6b8b4249dd502e75d34148fc5b"),
            address!("adaa92cba08434c22d036c4115a6b3d7e2b5569b"),
            address!("aee53701e18d5ff6af4964c3a381e7d09b9b9075"),
            address!("b893a8049f250b57efa8c62d51527a22404d7c9a"),
            address!("B96f0e9bb32760091eb2D6B0A5Ca0D2C7b5644B1"),
            address!("ba7435a4b4c747e0101780073eeda872a69bdcd4"),
            address!("bae5f2d8a1299e5c4963eaff3312399253f27ccb"),
            address!("bd36b14c63f483b286c7b49b6eaffb2fe10aabc4"),
            address!("bdea5bb640dbfc4593809deec5cdb8f99b704cd2"),
            address!("bf04e48c5d8880306591ef888cde201d3984eb3e"),
            address!("bf25ea982b4f850dafb4a95367b890eee5a9e8f2"),
            address!("bf494f02ee3fde1f20bee6242bce2d1ed0c15e47"),
            address!("c03841b5135600312707d39eb2af0d2ad5d51a91"),
            address!("c10bbb8fd399d580b740ed31ff5ac94aa78ba9ed"),
            address!("c12d1c73ee7dc3615ba4e37e4abfdbddfa38907e"),
            address!("c40af1e4fecfa05ce6bab79dcd8b373d2e436c4e"),
            address!("c4d586ef7be9ebe80bd5ee4fbd228fe2db5f2c4e"),
            address!("c50ef449171a51fbeafd7c562b064b6471c36caa"),
            address!("c626d951eff8e421448074bd2ad7805c6d585793"),
            address!("c73c167e7a4ba109e4052f70d5466d0c312a344d"),
            address!("c7c24fe893c21e8a4ef46eaf31badcab9f362841"),
            address!("cd7492db29e2ab436e819b249452ee1bbdf52214"),
            address!("cf0c122c6b73ff809c693db761e7baebe62b6a2e"),
            address!("cf2f589bea4645c3ef47f1f33bebf100bee66e05"),
            address!("cf8c23cf17bb5815d5705a15486fa83805415625"),
            address!("d0834d08c83dbe216811aaea0eeffb2349e57634"),
            address!("d0d3ebcad6a20ce69bc3bc0e1ec964075425e533"),
            address!("d1afbccc9a2c2187ea544363b986ea0ab6ef08b5"),
            address!("d375a513692336cf9eebce5e38869b447948016f"),
            address!("d3f6571be1d91ac68b40daaa24075ca7e2f0f72e"),
            address!("d50825f50384bc40d5a10118996ef503b3670afd"),
            address!("d5281bb2d1ee94866b03a0fccdd4e900c8cb5091"),
            address!("da1e53e088023fe4d1dc5a418581748f52cbd1b8"),
            address!("dd339f370bbb18b8f389bd0443329d82ecf4b593"),
            // Should be denied because can't approve more than balance
            address!("decade1c6bf2cd9fb89afad73e4a519c867adcf5"),
            address!("dfdd3459d4f87234751696840092ee20c970fb07"),
            address!("e0bdaafd0aab238c55d68ad54e616305d4a21772"),
            address!("e2d66561b39eadbd488868af8493fb55d4b9d084"),
            address!("e302bf71b1f6f3024e7642f9c824ac86b58436a0"),
            address!("ea319e87cf06203dae107dd8e5672175e3ee976c"),
            address!("ed5e5ab076ae60bdb9c49ac255553e65426a2167"),
            address!("eeee2a622330e6d2036691e983dee87330588603"),
            address!("ef5b32486ed432b804a51d129f4d2fbdf18057ec"),
            address!("f1365ab39e192808b5301bcf6da973830e9e817f"),
            address!("f198B4a2631B7D0B9FAc36f8B546Ed3DCe472A47"),
            address!("fad45e47083e4607302aa43c65fb3106f1cd7607"),
            address!("fcaa8eef70f373e00ac29208023d106c846259ee"),
            address!("ff69e48af1174da7f15d0c771861c33d3f19ed8a"),
        ];

        // Of the deny listed tokens the following are detected as good:
        // - token 0xc12d1c73ee7dc3615ba4e37e4abfdbddfa38907e Has some kind of
        //   "freezing" mechanism where some balance is unusuable. We don't seem to
        //   trigger it.
        // - 0x910524678c0b1b23ffb9285a81f99c29c11cbaed Has some kind of time lock that
        //   we don't encounter.
        // - 0xed5e5ab076ae60bdb9c49ac255553e65426a2167 Not sure why deny listed.
        // - 0x1337def18c680af1f9f45cbcab6309562975b1dd Not sure why deny listed, maybe
        //   the callback that I didn't follow in the SC code.
        // - 0x4f9254c83eb525f9fcf346490bbb3ed28a81c667 Not sure why deny listed.

        let settlement = GPv2Settlement::Instance::deployed(&web3.provider)
            .await
            .unwrap();
        let finder = Arc::new(TokenOwnerFinder {
            web3: web3.clone(),
            settlement_contract: *settlement.address(),
            proposers: vec![
                Arc::new(UniswapLikePairProviderFinder {
                    inner: uniswap_v2::UniV2BaselineSourceParameters::from_baseline_source(
                        BaselineSource::UniswapV2,
                        &version,
                    )
                    .unwrap()
                    .into_source(&web3)
                    .await
                    .unwrap()
                    .pair_provider,
                    base_tokens: base_tokens.to_vec(),
                }),
                Arc::new(UniswapLikePairProviderFinder {
                    inner: uniswap_v2::UniV2BaselineSourceParameters::from_baseline_source(
                        BaselineSource::SushiSwap,
                        &version,
                    )
                    .unwrap()
                    .into_source(&web3)
                    .await
                    .unwrap()
                    .pair_provider,
                    base_tokens: base_tokens.to_vec(),
                }),
                Arc::new(BalancerVaultFinder(
                    BalancerV2Vault::Instance::deployed(&web3.provider)
                        .await
                        .unwrap(),
                )),
                Arc::new(
                    UniswapV3Finder::new(
                        IUniswapV3Factory::Instance::deployed(&web3.provider)
                            .await
                            .unwrap(),
                        base_tokens.to_vec(),
                        FeeValues::Static,
                    )
                    .await
                    .unwrap(),
                ),
                Arc::new(
                    BlockscoutTokenOwnerFinder::with_network(
                        reqwest::Client::new(),
                        &Chain::Mainnet,
                    )
                    .unwrap(),
                ),
            ],
        });
        let token_cache = TraceCallDetector::new(web3, *settlement.address(), finder);

        println!("testing good tokens");
        for &token in base_tokens {
            let result = token_cache.detect(token).await;
            println!("token {token:?} is {result:?}");
        }

        println!("testing bad tokens");
        for &token in bad_tokens {
            let result = token_cache.detect(token).await;
            println!("token {token:?} is {result:?}");
        }
    }

    #[tokio::test]
    #[ignore]
    async fn mainnet_univ3() {
        observe::tracing::initialize(&observe::Config::default().with_env_filter("shared=debug"));
        let web3 = Web3::new_from_env();
        let base_tokens = vec![testlib::tokens::WETH];
        let settlement = GPv2Settlement::Instance::deployed(&web3.provider)
            .await
            .unwrap();
        let factory = IUniswapV3Factory::Instance::deployed(&web3.provider)
            .await
            .unwrap();
        let univ3 = Arc::new(
            UniswapV3Finder::new(factory, base_tokens, FeeValues::Dynamic)
                .await
                .unwrap(),
        );
        let finder = Arc::new(TokenOwnerFinder {
            web3: web3.clone(),
            settlement_contract: *settlement.address(),
            proposers: vec![univ3],
        });
        let token_cache = TraceCallDetector::new(web3, *settlement.address(), finder);

        let result = token_cache.detect(testlib::tokens::USDC).await;
        dbg!(&result);
        assert!(result.unwrap().is_good());

        let only_v3_token = address!("f1b99e3e573a1a9c5e6b2ce818b617f0e664e86b");
        let result = token_cache.detect(only_v3_token).await;
        dbg!(&result);
        assert!(result.unwrap().is_good());
    }

    #[tokio::test]
    #[ignore]
    async fn yearn_vault_tokens() {
        let tokens = [
            address!("1025b1641d1F23C289412Dd5E5701e9810103a93"),
            address!("132d8D2C76Db3812403431fAcB00F3453Fc42125"),
            address!("1635b506a88fBF428465Ad65d00e8d6B6E5846C3"),
            address!("16825039dfe2a5b01F3E1E6a2BBF9a576c6F95c4"),
            address!("1b905331F7dE2748F4D6a0678e1521E20347643F"),
            address!("23D3D0f1c697247d5e0a9efB37d8b0ED0C464f7f"),
            address!("25212Df29073FfFA7A67399AcEfC2dd75a831A1A"),
            address!("27B5739e22ad9033bcBf192059122d163b60349D"),
            address!("2D5D4869381C4Fce34789BC1D38aCCe747E295AE"),
            address!("2DfB14E32e2F8156ec15a2c21c3A6c053af52Be8"),
            address!("2a38B9B0201Ca39B17B460eD2f11e4929559071E"),
            address!("2e5c7e9B1Da0D9Cb2832eBb06241d18552A85400"),
            address!("30FCf7c6cDfC46eC237783D94Fc78553E79d4E9C"),
            address!("341bb10D8f5947f3066502DC8125d9b8949FD3D6"),
            address!("378cb52b00F9D0921cb46dFc099CFf73b42419dC"),
            address!("39CAF13a104FF567f71fd2A4c68C026FDB6E740B"),
            address!("3B27F92C0e212C671EA351827EDF93DB27cc0c65"),
            address!("3B96d491f067912D18563d56858Ba7d6EC67a6fa"),
            address!("3c5DF3077BcF800640B5DAE8c91106575a4826E6"),
            address!("4560b99C904aAD03027B5178CCa81584744AC01f"),
            address!("490bD0886F221A5F79713D3E84404355A9293C50"),
            address!("4B5BfD52124784745c1071dcB244C6688d2533d3"),
            address!("528D50dC9a333f01544177a924893FA1F5b9F748"),
            address!("59518884EeBFb03e90a18ADBAAAB770d4666471e"),
            address!("595a68a8c9D5C230001848B69b1947ee2A607164"),
            address!("5AB64C599FcC59f0f2726A300b03166A395578Da"),
            address!("5a770DbD3Ee6bAF2802D29a901Ef11501C44797A"),
            address!("5c0A86A32c129538D62C106Eb8115a8b02358d57"),
            address!("5e69e8b51B71C8596817fD442849BD44219bb095"),
            address!("5fA5B62c8AF877CB37031e0a3B2f34A78e3C56A6"),
            address!("625b7DF2fa8aBe21B0A976736CDa4775523aeD1E"),
            address!("671a912C10bba0CFA74Cfc2d6Fba9BA1ed9530B2"),
            address!("67e019bfbd5a67207755D04467D6A70c0B75bF60"),
            address!("6A5468752f8DB94134B6508dAbAC54D3b45efCE6"),
            address!("6B5ce31AF687a671a804d8070Ddda99Cab926dfE"),
            address!("6Ede7F19df5df6EF23bD5B9CeDb651580Bdf56Ca"),
            address!("6d765CbE5bC922694afE112C140b8878b9FB0390"),
            address!("7047F90229a057C13BF847C0744D646CFb6c9E1A"),
            address!("718AbE90777F5B778B52D553a5aBaa148DD0dc5D"),
            address!("790a60024bC3aea28385b60480f15a0771f26D09"),
            address!("801Ab06154Bf539dea4385a39f5fa8534fB53073"),
            address!("8414Db07a7F743dEbaFb402070AB01a4E0d2E45e"),
            address!("84E13785B5a27879921D6F685f041421C7F482dA"),
            address!("873fB544277FD7b977B196a826459a69E27eA4ea"),
            address!("8b9C0c24307344B6D7941ab654b2Aeee25347473"),
            address!("8cc94ccd0f3841a468184aCA3Cc478D2148E1757"),
            address!("8ee57c05741aA9DB947A744E713C15d4d19D8822"),
            address!("8fA3A9ecd9EFb07A8CE90A6eb014CF3c0E3B32Ef"),
            address!("9A39f31DD5EDF5919A5C0c2433cE053fAD2E0336"),
            address!("9d409a0A012CFbA9B15F6D4B36Ac57A46966Ab9a"),
            address!("A696a63cc78DfFa1a63E9E50587C197387FF6C7E"),
            address!("A74d4B67b3368E83797a35382AFB776bAAE4F5C8"),
            address!("A9412Ffd7E0866755ae0dda3318470A61F62abe8"),
            address!("B4AdA607B9d6b2c9Ee07A275e9616B84AC560139"),
            address!("BCBB5b54Fa51e7b7Dc920340043B203447842A6b"),
            address!("Bfedbcbe27171C418CDabC2477042554b1904857"),
            address!("C4dAf3b5e2A9e93861c3FBDd25f1e943B8D87417"),
            address!("D6Ea40597Be05c201845c0bFd2e96A60bACde267"),
            address!("E537B5cc158EB71037D4125BDD7538421981E6AA"),
            address!("E5eDcE53e39Cbc6d819E2C340BCF295e0084ff7c"),
            address!("F29AE508698bDeF169B89834F76704C3B205aedf"),
            address!("F59D66c1d593Fb10e2f8c2a6fD2C958792434B9c"),
            address!("F6B9DFE6bc42ed2eaB44D6B829017f7B78B29f88"),
            address!("FBEB78a723b8087fD2ea7Ef1afEc93d35E8Bed42"),
            address!("FD0877d9095789cAF24c98F7CCe092fa8E120775"),
            address!("a258C4606Ca8206D8aA700cE2143D7db854D168c"),
            address!("a354F35829Ae975e850e23e9615b11Da1B3dC4DE"),
            address!("b09F2a67a731466182518fae980feAe96479d80b"),
            address!("b4D1Be44BfF40ad6e506edf43156577a3f8672eC"),
            address!("c5F3D11580c41cD07104e9AF154Fc6428bb93c73"),
            address!("c97232527B62eFb0D8ed38CF3EA103A6CcA4037e"),
            address!("c97511a1dDB162C8742D39FF320CfDCd13fBcf7e"),
            address!("d88dBBA3f9c4391Ee46f5FF548f289054db6E51C"),
            address!("d8C620991b8E626C099eAaB29B1E3eEa279763bb"),
            address!("d9788f3931Ede4D5018184E198699dC6d66C1915"),
            address!("dA816459F1AB5631232FE5e97a05BBBb94970c95"),
            address!("db25cA703181E7484a155DD612b06f57E12Be5F0"),
            address!("e9Dc63083c464d6EDcCFf23444fF3CFc6886f6FB"),
            address!("f2db9a7c0ACd427A680D640F02d90f6186E71725"),
            address!("f8768814b88281DE4F532a3beEfA5b85B69b9324"),
        ];

        let solver_token_finder = Arc::new(AutoUpdatingSolverTokenOwnerFinder::new(
            Box::new(SolverConfiguration {
                url: env::var("SOLVER_TOKEN_OWNERS_URLS")
                    .unwrap()
                    .parse()
                    .unwrap(),
                client: reqwest::Client::new(),
            }),
            Duration::MAX,
            "test".to_owned(),
        ));

        // Force the cache to update at least once.
        solver_token_finder.update().await.unwrap();

        let web3 = Web3::new_from_env();

        let settlement = GPv2Settlement::Instance::deployed(&web3.provider)
            .await
            .unwrap();
        let finder = Arc::new(TokenOwnerFinder {
            web3: web3.clone(),
            proposers: vec![solver_token_finder],
            settlement_contract: *settlement.address(),
        });
        let token_cache = TraceCallDetector::new(web3, *settlement.address(), finder);

        for token in tokens {
            let result = token_cache.detect(token).await;
            println!("token {token:?} is {result:?}");
        }
    }
}
