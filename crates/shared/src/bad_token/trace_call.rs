use super::{token_owner_finder::TokenOwnerFinding, BadTokenDetecting, TokenQuality};
use crate::{ethrpc::Web3, trace_many};
use anyhow::{bail, ensure, Context, Result};
use contracts::ERC20;
use ethcontract::{dyns::DynTransport, transaction::TransactionBuilder, PrivateKey};
use primitive_types::{H160, U256};
use std::{cmp, sync::Arc};
use web3::{
    signing::keccak256,
    types::{BlockTrace, CallRequest, Res},
};

/// Detects whether a token is "bad" (works in unexpected ways that are problematic for solving) by
/// simulating several transfers of a token. To find an initial address to transfer from we use
/// the amm pair providers.
/// Tokens are bad if:
/// - we cannot find an amm pool of the token to one of the base tokens
/// - transfer into the settlement contract or back out fails
/// - a transfer loses total balance
pub struct TraceCallDetector {
    pub web3: Web3,
    pub finder: Arc<dyn TokenOwnerFinding>,
    pub settlement_contract: H160,
}

#[async_trait::async_trait]
impl BadTokenDetecting for TraceCallDetector {
    async fn detect(&self, token: H160) -> Result<TokenQuality> {
        let quality = self.detect_impl(token).await?;
        tracing::debug!("token {:?} quality {:?}", token, quality);
        Ok(quality)
    }
}

impl TraceCallDetector {
    pub async fn detect_impl(&self, token: H160) -> Result<TokenQuality> {
        // Arbitrary amount that is large enough that small relative fees should be visible.
        const MIN_AMOUNT: u64 = 100_000;
        let (take_from, amount) = match self.finder.find_owner(token, MIN_AMOUNT.into()).await? {
            Some((address, balance)) => {
                // Don't use the full balance, but instead a portion of it. This
                // makes the trace call less racy and prone to the transfer
                // failing because of a balance change from one block to the
                // next. This can happen because of either:
                // - Block propagation - the trace_callMany is handled by a node
                //   that is 1 block in the past
                // - New block observed - the trace_callMany is executed on a
                //   block that came in since we read the balance
                let amount = cmp::max(balance / 2, MIN_AMOUNT.into());

                tracing::debug!(?token, ?address, ?amount, "found owner");
                (address, amount)
            }
            None => return Ok(TokenQuality::bad("no pool")),
        };

        // We transfer the full available amount of the token from the amm pool into the
        // settlement contract and then to an arbitrary address.
        // Note that gas use can depend on the recipient because for the standard implementation
        // sending to an address that does not have any balance yet (implicitly 0) causes an
        // allocation.
        let request = self.create_trace_request(token, amount, take_from);
        let traces = trace_many::trace_many(request, &self.web3)
            .await
            .context("failed to trace for bad token detection")?;
        Self::handle_response(&traces, amount)
    }

    // For the out transfer we use an arbitrary address without balance to detect tokens that
    // usually apply fees but not if the the sender or receiver is specifically exempt like
    // their own uniswap pools.
    fn arbitrary_recipient() -> H160 {
        PrivateKey::from_raw(keccak256(b"moo"))
            .unwrap()
            .public_address()
    }

    fn create_trace_request(&self, token: H160, amount: U256, take_from: H160) -> Vec<CallRequest> {
        let instance = ERC20::at(&self.web3, token);

        let mut requests = Vec::new();

        // 0
        let tx = instance.balance_of(self.settlement_contract).m.tx;
        requests.push(call_request(None, token, tx));
        // 1
        let tx = instance.transfer(self.settlement_contract, amount).tx;
        requests.push(call_request(Some(take_from), token, tx));
        // 2
        let tx = instance.balance_of(self.settlement_contract).m.tx;
        requests.push(call_request(None, token, tx));
        // 3
        let recipient = Self::arbitrary_recipient();
        let tx = instance.balance_of(recipient).m.tx;
        requests.push(call_request(None, token, tx));
        // 4
        let tx = instance.transfer(recipient, amount).tx;
        requests.push(call_request(Some(self.settlement_contract), token, tx));
        // 5
        let tx = instance.balance_of(self.settlement_contract).m.tx;
        requests.push(call_request(None, token, tx));
        // 6
        let tx = instance.balance_of(recipient).m.tx;
        requests.push(call_request(None, token, tx));

        // 7
        let tx = instance.approve(recipient, U256::MAX).tx;
        requests.push(call_request(Some(self.settlement_contract), token, tx));

        requests
    }

    fn handle_response(traces: &[BlockTrace], amount: U256) -> Result<TokenQuality> {
        ensure!(traces.len() == 8, "unexpected number of traces");

        let gas_in = match ensure_transaction_ok_and_get_gas(&traces[1])? {
            Ok(gas) => gas,
            Err(reason) => {
                return Ok(TokenQuality::bad(format!(
                    "can't transfer into settlement contract: {reason}"
                )))
            }
        };
        let gas_out = match ensure_transaction_ok_and_get_gas(&traces[4])? {
            Ok(gas) => gas,
            Err(reason) => {
                return Ok(TokenQuality::bad(format!(
                    "can't transfer out of settlement contract: {reason}"
                )))
            }
        };

        let balance_before_in = match decode_u256(&traces[0]) {
            Ok(balance) => balance,
            Err(_) => return Ok(TokenQuality::bad("can't decode initial settlement balance")),
        };
        let balance_after_in = match decode_u256(&traces[2]) {
            Ok(balance) => balance,
            Err(_) => return Ok(TokenQuality::bad("can't decode middle settlement balance")),
        };
        let balance_after_out = match decode_u256(&traces[5]) {
            Ok(balance) => balance,
            Err(_) => return Ok(TokenQuality::bad("can't decode final settlement balance")),
        };

        let balance_recipient_before = match decode_u256(&traces[3]) {
            Ok(balance) => balance,
            Err(_) => return Ok(TokenQuality::bad("can't decode recipient balance before")),
        };

        let balance_recipient_after = match decode_u256(&traces[6]) {
            Ok(balance) => balance,
            Err(_) => return Ok(TokenQuality::bad("can't decode recipient balance after")),
        };

        tracing::debug!(%amount, %balance_before_in, %balance_after_in, %balance_after_out);

        // todo: Maybe do >= checks in case token transfer for whatever reason grants user more than
        // an amount transferred like an anti fee.

        let computed_balance_after_in = match balance_before_in.checked_add(amount) {
            Some(amount) => amount,
            None => {
                return Ok(TokenQuality::bad(
                    "token total supply does not fit a uint256",
                ))
            }
        };
        if balance_after_in != computed_balance_after_in {
            return Ok(TokenQuality::bad(
                "balance after in transfer does not match",
            ));
        }
        if balance_after_out != balance_before_in {
            return Ok(TokenQuality::bad(
                "balance after out transfer does not match",
            ));
        }
        let computed_balance_recipient_after = match balance_recipient_before.checked_add(amount) {
            Some(amount) => amount,
            None => {
                return Ok(TokenQuality::bad(
                    "token total supply does not fit a uint256",
                ))
            }
        };
        if computed_balance_recipient_after != balance_recipient_after {
            return Ok(TokenQuality::bad("balance of recipient does not match"));
        }

        if let Err(err) = ensure_transaction_ok_and_get_gas(&traces[7])? {
            return Ok(TokenQuality::bad(format!(
                "can't approve max amount: {}",
                err
            )));
        }

        let _gas_per_transfer = (gas_in + gas_out) / 2;
        Ok(TokenQuality::Good)
    }
}

fn call_request(
    from: Option<H160>,
    to: H160,
    transaction: TransactionBuilder<DynTransport>,
) -> CallRequest {
    let calldata = transaction.data.unwrap();
    CallRequest {
        from,
        to: Some(to),
        data: Some(calldata),
        ..Default::default()
    }
}

fn decode_u256(trace: &BlockTrace) -> Result<U256> {
    let bytes = trace.output.0.as_slice();
    ensure!(bytes.len() == 32, "invalid length");
    Ok(U256::from_big_endian(bytes))
}

// The outer result signals communication failure with the node.
// The inner result is Ok(gas_price) or Err if the transaction failed.
fn ensure_transaction_ok_and_get_gas(trace: &BlockTrace) -> Result<Result<U256, String>> {
    let transaction_traces = trace.trace.as_ref().context("trace not set")?;
    let first = transaction_traces
        .first()
        .context("expected at least one trace")?;
    if let Some(error) = &first.error {
        return Ok(Err(format!("transaction failed: {error}")));
    }
    let call_result = match &first.result {
        Some(Res::Call(call)) => call,
        _ => bail!("no error but also no call result"),
    };
    Ok(Ok(call_result.gas_used))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        bad_token::token_owner_finder::{
            blockscout::BlockscoutTokenOwnerFinder,
            liquidity::{
                BalancerVaultFinder, FeeValues, UniswapLikePairProviderFinder, UniswapV3Finder,
            },
            solvers::{
                solver_api::SolverConfiguration, solver_finder::AutoUpdatingSolverTokenOwnerFinder,
            },
            TokenOwnerFinder,
        },
        ethrpc::create_env_test_transport,
        sources::{sushiswap, uniswap_v2},
    };
    use contracts::{BalancerV2Vault, IUniswapV3Factory};
    use hex_literal::hex;
    use std::{env, time::Duration};
    use web3::types::{
        Action, ActionType, Bytes, Call, CallResult, CallType, Res, TransactionTrace,
    };

    fn encode_u256(u256: U256) -> Bytes {
        let mut bytes = vec![0u8; 32];
        u256.to_big_endian(&mut bytes);
        Bytes(bytes)
    }

    #[test]
    fn handle_response_ok() {
        let traces = &[
            BlockTrace {
                output: encode_u256(0.into()),
                trace: None,
                vm_trace: None,
                state_diff: None,
                transaction_hash: None,
            },
            BlockTrace {
                output: Default::default(),
                trace: Some(vec![TransactionTrace {
                    trace_address: Vec::new(),
                    subtraces: 0,
                    action: Action::Call(Call {
                        from: H160::zero(),
                        to: H160::zero(),
                        value: 0.into(),
                        gas: 0.into(),
                        input: Bytes(Vec::new()),
                        call_type: CallType::None,
                    }),
                    action_type: ActionType::Call,
                    result: Some(Res::Call(CallResult {
                        gas_used: 1.into(),
                        output: Bytes(Vec::new()),
                    })),
                    error: None,
                }]),
                vm_trace: None,
                state_diff: None,
                transaction_hash: None,
            },
            BlockTrace {
                output: encode_u256(1.into()),
                trace: None,
                vm_trace: None,
                state_diff: None,
                transaction_hash: None,
            },
            BlockTrace {
                output: encode_u256(0.into()),
                trace: None,
                vm_trace: None,
                state_diff: None,
                transaction_hash: None,
            },
            BlockTrace {
                output: Default::default(),
                trace: Some(vec![TransactionTrace {
                    trace_address: Vec::new(),
                    subtraces: 0,
                    action: Action::Call(Call {
                        from: H160::zero(),
                        to: H160::zero(),
                        value: 0.into(),
                        gas: 0.into(),
                        input: Bytes(Vec::new()),
                        call_type: CallType::None,
                    }),
                    action_type: ActionType::Call,
                    result: Some(Res::Call(CallResult {
                        gas_used: 3.into(),
                        output: Bytes(Vec::new()),
                    })),
                    error: None,
                }]),
                vm_trace: None,
                state_diff: None,
                transaction_hash: None,
            },
            BlockTrace {
                output: encode_u256(0.into()),
                trace: None,
                vm_trace: None,
                state_diff: None,
                transaction_hash: None,
            },
            BlockTrace {
                output: encode_u256(1.into()),
                trace: None,
                vm_trace: None,
                state_diff: None,
                transaction_hash: None,
            },
            BlockTrace {
                output: Default::default(),
                trace: Some(vec![TransactionTrace {
                    trace_address: Vec::new(),
                    subtraces: 0,
                    action: Action::Call(Call {
                        from: H160::zero(),
                        to: H160::zero(),
                        value: 0.into(),
                        gas: 0.into(),
                        input: Bytes(Vec::new()),
                        call_type: CallType::None,
                    }),
                    action_type: ActionType::Call,
                    result: Some(Res::Call(CallResult {
                        gas_used: 1.into(),
                        output: Bytes(Vec::new()),
                    })),
                    error: None,
                }]),
                vm_trace: None,
                state_diff: None,
                transaction_hash: None,
            },
        ];

        let result = TraceCallDetector::handle_response(traces, 1.into()).unwrap();
        let expected = TokenQuality::Good;
        assert_eq!(result, expected);
    }

    #[test]
    fn arbitrary_recipient_() {
        println!("{:?}", TraceCallDetector::arbitrary_recipient());
    }

    // cargo test -p shared mainnet_tokens -- --nocapture --ignored
    #[tokio::test]
    #[ignore]
    async fn mainnet_tokens() {
        // shared::tracing::initialize("orderbook::bad_token=debug,shared::transport=debug", tracing::level_filters::LevelFilter::OFF);
        let http = create_env_test_transport();
        let web3 = Web3::new(http);

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
            addr!("0027449Bf0887ca3E431D263FFDeFb244D95b555"), // All balances are maxuint256
            addr!("0189d31f6629c359007f72b8d5ec8fa1c126f95c"),
            addr!("01995786f1435743c42b7f2276c496a610b58612"),
            addr!("072c46f392e729c1f0d92a307c2c6dba06b5d078"),
            addr!("074545177a36ab81aac783211f25e14f1ed03c2b"),
            addr!("07be1ead7aebee544618bdc688fa3cff09857c32"),
            addr!("0858a26055d6584e5b47bbecf7f7e8cbc390995b"),
            addr!("0aacfbec6a24756c20d41914f2caba817c0d8521"),
            addr!("0ba45a8b5d5575935b8158a88c631e9f9c95a2e5"),
            addr!("0e69d0a2bbb30abcb7e5cfea0e4fde19c00a8d47"),
            addr!("1016f3c0a1939fa27538339da7e2a300031b6f37"),
            addr!("106552c11272420aad5d7e94f8acab9095a6c952"),
            addr!("106d3c66d22d2dd0446df23d7f5960752994d600"),
            addr!("1337DEF18C680aF1f9f45cBcab6309562975b1dD"),
            addr!("1341a2257fa7b770420ef70616f888056f90926c"),
            addr!("1426cc6d52d1b14e2b3b1cb04d57ea42b39c4c7c"),
            addr!("14dd7ebe6cb084cb73ef377e115554d47dc9d61e"),
            addr!("15874d65e649880c2614e7a480cb7c9a55787ff6"),
            addr!("1681bcb589b3cfcf0c0616b0ce9b19b240643dc1"),
            addr!("18bdfc80b97cb97f6b466cce967849ce9cd9d58c"),
            addr!("1b9baf2a3edea91ee431f02d449a1044d5726669"),
            addr!("2129ff6000b95a973236020bcd2b2006b0d8e019"),
            addr!("239dc02a28a0774738463e06245544a72745d5c5"),
            addr!("251457b7c5d85251ca1ab384361c821330be2520"),
            addr!("25a1de1c3ee658fe034b8914a1d8d34110423af8"),
            addr!("26a79bd709a7ef5e5f747b8d8f83326ea044d8cc"),
            addr!("289d5488ab09f43471914e572ec9e3651c735af2"),
            addr!("298d492e8c1d909d3f63bc4a36c66c64acb3d695"),
            addr!("2b1fe2cea92436e8c34b7c215af66aaa2932a8b2"),
            addr!("31acf54fae6166dc2f90c4d6f20d379965e96bc1"),
            addr!("32c868f6318d6334b2250f323d914bc2239e4eee"),
            addr!("33f128394af03db639107473e52d84ff1290499e"),
            addr!("37611b28aca5673744161dc337128cfdd2657f69"),
            addr!("389999216860ab8e0175387a0c90e5c52522c945"),
            addr!("39b8523fa094b0dc045e2c3e5dff34b3f2ca6220"),
            addr!("3a6fe4c752eb8d571a660a776be4003d619c30a3"),
            addr!("3a9fff453d50d4ac52a6890647b823379ba36b9e"),
            addr!("3ea50b7ef6a7eaf7e966e2cb72b519c16557497c"),
            addr!("3fca773d13f831753ec3ae9f39ad4a6814ebb695"),
            addr!("41933422dc4a1cb8c822e06f12f7b52fa5e7e094"),
            addr!("45734927fa2f616fbe19e65f42a0ef3d37d1c80a"),
            addr!("45804880de22913dafe09f4980848ece6ecbaf78"),
            addr!("48be867b240d2ffaff69e0746130f2c027d8d3d2"),
            addr!("4a6be56a211a4c4e0dd4474d524138933c17f3e3"),
            addr!("4b86e0295e7d32433ffa6411b82b4f4e56a581e1"),
            addr!("4ba6ddd7b89ed838fed25d208d4f644106e34279"),
            addr!("4bae380b5d762d543d426331b8437926443ae9ec"),
            addr!("4bcddfcfa8cb923952bcf16644b36e5da5ca3184"),
            addr!("4c9d5672ae33522240532206ab45508116daf263"),
            addr!("4F9254C83EB525f9FCf346490bbb3ed28a81C667"),
            addr!("4fab740779c73aa3945a5cf6025bf1b0e7f6349c"),
            addr!("51d3e4c0b2c83e62f5d517d250b3e856897d2052"),
            addr!("53ba22cb4e5e9c1be0d73913764f572192a71aca"),
            addr!("56de8bc61346321d4f2211e3ac3c0a7f00db9b76"),
            addr!("576097fa17e1f702bb9167f0f08f2ea0898a3ea5"),
            addr!("577e7f9fa80ab33e87a01b701114257c8d9455a8"),
            addr!("586c680e9a6d21b81ebecf46d78844dab7b3bcf9"),
            addr!("5d0fa08aeb173ade44b0cf7f31d506d8e04f0ac8"),
            addr!("62359ed7505efc61ff1d56fef82158ccaffa23d7"),
            addr!("63d0eea1d7c0d1e89d7e665708d7e8997c0a9ed6"),
            addr!("66d31def9c47b62184d7f57175eed5b5d9b7f038"),
            addr!("671ab077497575dcafb68327d2d2329207323e74"),
            addr!("685aea4f02e39e5a5bb7f7117e88db1151f38364"),
            addr!("68e0a48d3bff6633a31d1d100b70f93c3859218b"),
            addr!("69692d3345010a207b759a7d1af6fc7f38b35c5e"),
            addr!("6a00b86e30167f73e38be086081b80213e8266aa"),
            addr!("6b8e77d3db1faa17f7b24c24242b6a1eb5008a16"),
            addr!("6e10aacb89a28d6fa0fe68790777fec7e7f01890"),
            addr!("6fcb6408499a7c0f242e32d77eb51ffa1dd28a7e"),
            addr!("714599f7604144a3fe1737c440a70fc0fd6503ea"),
            addr!("75fef397d74a2d11b64e6915cd847c1e7f8e5520"),
            addr!("76851a93977bea9264c32255b6457882035c7501"),
            addr!("79ba92dda26fce15e1e9af47d5cfdfd2a093e000"),
            addr!("7f0f118d083d5175ab9d2d34c4c8fa4f43c3f47b"),
            addr!("7ff4169a6b5122b664c51c95727d87750ec07c84"),
            addr!("801ea8c463a776e85344c565e355137b5c3324cd"),
            addr!("88ef27e69108b2633f8e1c184cc37940a075cc02"),
            addr!("8c7424c3000942e5a93de4a01ce2ec86c06333cb"),
            addr!("8eb24319393716668d768dcec29356ae9cffe285"),
            addr!("910524678c0b1b23ffb9285a81f99c29c11cbaed"),
            addr!("910985ffa7101bf5801dd2e91555c465efd9aab3"),
            addr!("925f2c11b99c1a4c46606898ee91ed3d450cfeda"),
            addr!("944eee930933be5e23b690c8589021ec8619a301"),
            addr!("94987bc8aa5f36cb2461c190134929a29c3df726"),
            addr!("97ad070879be5c31a03a1fe7e35dfb7d51d0eef1"),
            addr!("97b65710d03e12775189f0d113202cc1443b0aa2"),
            addr!("98ecf3d8e21adaafe16c00cc3ff681e72690278b"),
            addr!("99043bb680ab9262c7b2ac524e00b215efb7db9b"),
            addr!("99ddddd8dfe33905338a073047cfad72e6833c06"),
            addr!("9a514389172863f12854ad40090aa4b928028542"),
            addr!("9af15d7b8776fa296019979e70a5be53c714a7ec"),
            addr!("9ea3b5b4ec044b70375236a281986106457b20ef"),
            addr!("9f41da75ab2b8c6f0dcef7173c4bf66bd4f6b36a"),
            addr!("a03f1250aa448226ed4066d8d1722ddd8b51df59"),
            addr!("a2b4c0af19cc16a6cfacce81f192b024d625817d"),
            addr!("a3e059c0b01f07f211c85bf7b4f1d907afb011df"),
            addr!("a5959e9412d27041194c3c3bcbe855face2864f7"),
            addr!("a9a8377287ea9c6b8b4249dd502e75d34148fc5b"),
            addr!("adaa92cba08434c22d036c4115a6b3d7e2b5569b"),
            addr!("aee53701e18d5ff6af4964c3a381e7d09b9b9075"),
            addr!("b893a8049f250b57efa8c62d51527a22404d7c9a"),
            addr!("B96f0e9bb32760091eb2D6B0A5Ca0D2C7b5644B1"),
            addr!("ba7435a4b4c747e0101780073eeda872a69bdcd4"),
            addr!("bae5f2d8a1299e5c4963eaff3312399253f27ccb"),
            addr!("bd36b14c63f483b286c7b49b6eaffb2fe10aabc4"),
            addr!("bdea5bb640dbfc4593809deec5cdb8f99b704cd2"),
            addr!("bf04e48c5d8880306591ef888cde201d3984eb3e"),
            addr!("bf25ea982b4f850dafb4a95367b890eee5a9e8f2"),
            addr!("bf494f02ee3fde1f20bee6242bce2d1ed0c15e47"),
            addr!("c03841b5135600312707d39eb2af0d2ad5d51a91"),
            addr!("c10bbb8fd399d580b740ed31ff5ac94aa78ba9ed"),
            addr!("c12d1c73ee7dc3615ba4e37e4abfdbddfa38907e"),
            addr!("c40af1e4fecfa05ce6bab79dcd8b373d2e436c4e"),
            addr!("c4d586ef7be9ebe80bd5ee4fbd228fe2db5f2c4e"),
            addr!("c50ef449171a51fbeafd7c562b064b6471c36caa"),
            addr!("c626d951eff8e421448074bd2ad7805c6d585793"),
            addr!("c73c167e7a4ba109e4052f70d5466d0c312a344d"),
            addr!("c7c24fe893c21e8a4ef46eaf31badcab9f362841"),
            addr!("cd7492db29e2ab436e819b249452ee1bbdf52214"),
            addr!("cf0c122c6b73ff809c693db761e7baebe62b6a2e"),
            addr!("cf2f589bea4645c3ef47f1f33bebf100bee66e05"),
            addr!("cf8c23cf17bb5815d5705a15486fa83805415625"),
            addr!("d0834d08c83dbe216811aaea0eeffb2349e57634"),
            addr!("d0d3ebcad6a20ce69bc3bc0e1ec964075425e533"),
            addr!("d1afbccc9a2c2187ea544363b986ea0ab6ef08b5"),
            addr!("d375a513692336cf9eebce5e38869b447948016f"),
            addr!("d3f6571be1d91ac68b40daaa24075ca7e2f0f72e"),
            addr!("d50825f50384bc40d5a10118996ef503b3670afd"),
            addr!("d5281bb2d1ee94866b03a0fccdd4e900c8cb5091"),
            addr!("da1e53e088023fe4d1dc5a418581748f52cbd1b8"),
            addr!("dd339f370bbb18b8f389bd0443329d82ecf4b593"),
            addr!("decade1c6bf2cd9fb89afad73e4a519c867adcf5"), // Should be denied because can't approve more than balance
            addr!("dfdd3459d4f87234751696840092ee20c970fb07"),
            addr!("e0bdaafd0aab238c55d68ad54e616305d4a21772"),
            addr!("e2d66561b39eadbd488868af8493fb55d4b9d084"),
            addr!("e302bf71b1f6f3024e7642f9c824ac86b58436a0"),
            addr!("ea319e87cf06203dae107dd8e5672175e3ee976c"),
            addr!("ed5e5ab076ae60bdb9c49ac255553e65426a2167"),
            addr!("eeee2a622330e6d2036691e983dee87330588603"),
            addr!("ef5b32486ed432b804a51d129f4d2fbdf18057ec"),
            addr!("f1365ab39e192808b5301bcf6da973830e9e817f"),
            addr!("f198B4a2631B7D0B9FAc36f8B546Ed3DCe472A47"),
            addr!("fad45e47083e4607302aa43c65fb3106f1cd7607"),
            addr!("fcaa8eef70f373e00ac29208023d106c846259ee"),
            addr!("ff69e48af1174da7f15d0c771861c33d3f19ed8a"),
        ];

        // Of the deny listed tokens the following are detected as good:
        // - token 0xc12d1c73ee7dc3615ba4e37e4abfdbddfa38907e
        //   Has some kind of "freezing" mechanism where some balance is unusuable. We don't seem to
        //   trigger it.
        // - 0x910524678c0b1b23ffb9285a81f99c29c11cbaed
        //   Has some kind of time lock that we don't encounter.
        // - 0xed5e5ab076ae60bdb9c49ac255553e65426a2167
        //   Not sure why deny listed.
        // - 0x1337def18c680af1f9f45cbcab6309562975b1dd
        //   Not sure why deny listed, maybe the callback that I didn't follow in the SC code.
        // - 0x4f9254c83eb525f9fcf346490bbb3ed28a81c667
        //   Not sure why deny listed.

        let settlement = contracts::GPv2Settlement::deployed(&web3).await.unwrap();
        let finder = Arc::new(TokenOwnerFinder {
            web3: web3.clone(),
            proposers: vec![
                Arc::new(UniswapLikePairProviderFinder {
                    inner: uniswap_v2::get_liquidity_source(&web3).await.unwrap().0,
                    base_tokens: base_tokens.to_vec(),
                }),
                Arc::new(UniswapLikePairProviderFinder {
                    inner: sushiswap::get_liquidity_source(&web3).await.unwrap().0,
                    base_tokens: base_tokens.to_vec(),
                }),
                Arc::new(BalancerVaultFinder(
                    BalancerV2Vault::deployed(&web3).await.unwrap(),
                )),
                Arc::new(
                    UniswapV3Finder::new(
                        IUniswapV3Factory::deployed(&web3).await.unwrap(),
                        base_tokens.to_vec(),
                        FeeValues::Dynamic,
                    )
                    .await
                    .unwrap(),
                ),
                Arc::new(
                    BlockscoutTokenOwnerFinder::try_with_network(reqwest::Client::new(), 1)
                        .unwrap(),
                ),
            ],
        });
        let token_cache = TraceCallDetector {
            web3,
            finder,
            settlement_contract: settlement.address(),
        };

        println!("testing good tokens");
        for &token in base_tokens {
            let result = token_cache.detect(token).await;
            println!("token {:?} is {:?}", token, result);
        }

        println!("testing bad tokens");
        for &token in bad_tokens {
            let result = token_cache.detect(token).await;
            println!("token {:?} is {:?}", token, result);
        }
    }

    #[tokio::test]
    #[ignore]
    async fn mainnet_univ3() {
        crate::tracing::initialize_for_tests("shared=debug");
        let http = create_env_test_transport();
        let web3 = Web3::new(http);
        let base_tokens = vec![testlib::tokens::WETH];
        let settlement = contracts::GPv2Settlement::deployed(&web3).await.unwrap();
        let factory = IUniswapV3Factory::deployed(&web3).await.unwrap();
        let univ3 = Arc::new(
            UniswapV3Finder::new(factory, base_tokens, FeeValues::Dynamic)
                .await
                .unwrap(),
        );
        let finder = Arc::new(TokenOwnerFinder {
            web3: web3.clone(),
            proposers: vec![univ3],
        });
        let token_cache = super::TraceCallDetector {
            web3,
            finder,
            settlement_contract: settlement.address(),
        };

        let result = token_cache.detect(testlib::tokens::USDC).await;
        dbg!(&result);
        assert!(result.unwrap().is_good());

        let only_v3_token = H160(hex!("f1b99e3e573a1a9c5e6b2ce818b617f0e664e86b"));
        let result = token_cache.detect(only_v3_token).await;
        dbg!(&result);
        assert!(result.unwrap().is_good());
    }

    #[tokio::test]
    #[ignore]
    async fn yearn_vault_tokens() {
        let tokens = [
            addr!("1025b1641d1F23C289412Dd5E5701e9810103a93"),
            addr!("132d8D2C76Db3812403431fAcB00F3453Fc42125"),
            addr!("1635b506a88fBF428465Ad65d00e8d6B6E5846C3"),
            addr!("16825039dfe2a5b01F3E1E6a2BBF9a576c6F95c4"),
            addr!("1b905331F7dE2748F4D6a0678e1521E20347643F"),
            addr!("23D3D0f1c697247d5e0a9efB37d8b0ED0C464f7f"),
            addr!("25212Df29073FfFA7A67399AcEfC2dd75a831A1A"),
            addr!("27B5739e22ad9033bcBf192059122d163b60349D"),
            addr!("2D5D4869381C4Fce34789BC1D38aCCe747E295AE"),
            addr!("2DfB14E32e2F8156ec15a2c21c3A6c053af52Be8"),
            addr!("2a38B9B0201Ca39B17B460eD2f11e4929559071E"),
            addr!("2e5c7e9B1Da0D9Cb2832eBb06241d18552A85400"),
            addr!("30FCf7c6cDfC46eC237783D94Fc78553E79d4E9C"),
            addr!("341bb10D8f5947f3066502DC8125d9b8949FD3D6"),
            addr!("378cb52b00F9D0921cb46dFc099CFf73b42419dC"),
            addr!("39CAF13a104FF567f71fd2A4c68C026FDB6E740B"),
            addr!("3B27F92C0e212C671EA351827EDF93DB27cc0c65"),
            addr!("3B96d491f067912D18563d56858Ba7d6EC67a6fa"),
            addr!("3c5DF3077BcF800640B5DAE8c91106575a4826E6"),
            addr!("4560b99C904aAD03027B5178CCa81584744AC01f"),
            addr!("490bD0886F221A5F79713D3E84404355A9293C50"),
            addr!("4B5BfD52124784745c1071dcB244C6688d2533d3"),
            addr!("528D50dC9a333f01544177a924893FA1F5b9F748"),
            addr!("59518884EeBFb03e90a18ADBAAAB770d4666471e"),
            addr!("595a68a8c9D5C230001848B69b1947ee2A607164"),
            addr!("5AB64C599FcC59f0f2726A300b03166A395578Da"),
            addr!("5a770DbD3Ee6bAF2802D29a901Ef11501C44797A"),
            addr!("5c0A86A32c129538D62C106Eb8115a8b02358d57"),
            addr!("5e69e8b51B71C8596817fD442849BD44219bb095"),
            addr!("5fA5B62c8AF877CB37031e0a3B2f34A78e3C56A6"),
            addr!("625b7DF2fa8aBe21B0A976736CDa4775523aeD1E"),
            addr!("671a912C10bba0CFA74Cfc2d6Fba9BA1ed9530B2"),
            addr!("67e019bfbd5a67207755D04467D6A70c0B75bF60"),
            addr!("6A5468752f8DB94134B6508dAbAC54D3b45efCE6"),
            addr!("6B5ce31AF687a671a804d8070Ddda99Cab926dfE"),
            addr!("6Ede7F19df5df6EF23bD5B9CeDb651580Bdf56Ca"),
            addr!("6d765CbE5bC922694afE112C140b8878b9FB0390"),
            addr!("7047F90229a057C13BF847C0744D646CFb6c9E1A"),
            addr!("718AbE90777F5B778B52D553a5aBaa148DD0dc5D"),
            addr!("790a60024bC3aea28385b60480f15a0771f26D09"),
            addr!("801Ab06154Bf539dea4385a39f5fa8534fB53073"),
            addr!("8414Db07a7F743dEbaFb402070AB01a4E0d2E45e"),
            addr!("84E13785B5a27879921D6F685f041421C7F482dA"),
            addr!("873fB544277FD7b977B196a826459a69E27eA4ea"),
            addr!("8b9C0c24307344B6D7941ab654b2Aeee25347473"),
            addr!("8cc94ccd0f3841a468184aCA3Cc478D2148E1757"),
            addr!("8ee57c05741aA9DB947A744E713C15d4d19D8822"),
            addr!("8fA3A9ecd9EFb07A8CE90A6eb014CF3c0E3B32Ef"),
            addr!("9A39f31DD5EDF5919A5C0c2433cE053fAD2E0336"),
            addr!("9d409a0A012CFbA9B15F6D4B36Ac57A46966Ab9a"),
            addr!("A696a63cc78DfFa1a63E9E50587C197387FF6C7E"),
            addr!("A74d4B67b3368E83797a35382AFB776bAAE4F5C8"),
            addr!("A9412Ffd7E0866755ae0dda3318470A61F62abe8"),
            addr!("B4AdA607B9d6b2c9Ee07A275e9616B84AC560139"),
            addr!("BCBB5b54Fa51e7b7Dc920340043B203447842A6b"),
            addr!("Bfedbcbe27171C418CDabC2477042554b1904857"),
            addr!("C4dAf3b5e2A9e93861c3FBDd25f1e943B8D87417"),
            addr!("D6Ea40597Be05c201845c0bFd2e96A60bACde267"),
            addr!("E537B5cc158EB71037D4125BDD7538421981E6AA"),
            addr!("E5eDcE53e39Cbc6d819E2C340BCF295e0084ff7c"),
            addr!("F29AE508698bDeF169B89834F76704C3B205aedf"),
            addr!("F59D66c1d593Fb10e2f8c2a6fD2C958792434B9c"),
            addr!("F6B9DFE6bc42ed2eaB44D6B829017f7B78B29f88"),
            addr!("FBEB78a723b8087fD2ea7Ef1afEc93d35E8Bed42"),
            addr!("FD0877d9095789cAF24c98F7CCe092fa8E120775"),
            addr!("a258C4606Ca8206D8aA700cE2143D7db854D168c"),
            addr!("a354F35829Ae975e850e23e9615b11Da1B3dC4DE"),
            addr!("b09F2a67a731466182518fae980feAe96479d80b"),
            addr!("b4D1Be44BfF40ad6e506edf43156577a3f8672eC"),
            addr!("c5F3D11580c41cD07104e9AF154Fc6428bb93c73"),
            addr!("c97232527B62eFb0D8ed38CF3EA103A6CcA4037e"),
            addr!("c97511a1dDB162C8742D39FF320CfDCd13fBcf7e"),
            addr!("d88dBBA3f9c4391Ee46f5FF548f289054db6E51C"),
            addr!("d8C620991b8E626C099eAaB29B1E3eEa279763bb"),
            addr!("d9788f3931Ede4D5018184E198699dC6d66C1915"),
            addr!("dA816459F1AB5631232FE5e97a05BBBb94970c95"),
            addr!("db25cA703181E7484a155DD612b06f57E12Be5F0"),
            addr!("e9Dc63083c464d6EDcCFf23444fF3CFc6886f6FB"),
            addr!("f2db9a7c0ACd427A680D640F02d90f6186E71725"),
            addr!("f8768814b88281DE4F532a3beEfA5b85B69b9324"),
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

        let http = create_env_test_transport();
        let web3 = Web3::new(http);

        let settlement = contracts::GPv2Settlement::deployed(&web3).await.unwrap();
        let finder = Arc::new(TokenOwnerFinder {
            web3: web3.clone(),
            proposers: vec![solver_token_finder],
        });
        let token_cache = TraceCallDetector {
            web3,
            finder,
            settlement_contract: settlement.address(),
        };

        for token in tokens {
            let result = token_cache.detect(token).await;
            println!("token {:?} is {:?}", token, result);
        }
    }
}
