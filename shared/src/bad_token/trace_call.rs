use super::{BadTokenDetecting, TokenQuality};
use crate::{
    amm_pair_provider::AmmPairProvider, ethcontract_error::EthcontractErrorType, trace_many, Web3,
};
use anyhow::{anyhow, bail, ensure, Result};
use contracts::ERC20;
use ethcontract::{
    batch::CallBatch, dyns::DynTransport, transaction::TransactionBuilder, PrivateKey,
};
use model::TokenPair;
use primitive_types::{H160, U256};
use std::{collections::HashSet, sync::Arc};
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
    pub pools: Vec<Arc<dyn AmmPairProvider>>,
    pub base_tokens: HashSet<H160>,
    pub settlement_contract: H160,
}

#[async_trait::async_trait]
impl BadTokenDetecting for TraceCallDetector {
    async fn detect(&self, token: H160) -> Result<TokenQuality> {
        let quality = self.detect_impl(token).await?;
        tracing::info!("token {:?} quality {:?}", token, quality);
        Ok(quality)
    }
}

impl TraceCallDetector {
    pub async fn detect_impl(&self, token: H160) -> Result<TokenQuality> {
        let instance = ERC20::at(&self.web3, token);
        let decimals = match instance.decimals().call().await {
            Ok(decimals) => decimals,
            Err(err) => {
                return match EthcontractErrorType::classify(&err) {
                    EthcontractErrorType::Node => Err(err.into()),
                    EthcontractErrorType::Contract => {
                        Ok(TokenQuality::bad("failed to get decimals"))
                    }
                }
            }
        };
        let amount = match U256::from(10).checked_pow(decimals.into()) {
            Some(amount) => {
                tracing::debug!("testing token {:?} with amount {}", token, amount);
                amount
            }
            None => return Ok(TokenQuality::bad("decimals too large")),
        };

        let take_from = match self.find_largest_pool_owning_token(token).await? {
            Some((address, balance)) if balance > amount => {
                tracing::debug!("testing token {:?} with pool {:?}", token, address);
                address
            }
            _ => return Ok(TokenQuality::bad("no pool with enough balance")),
        };

        // We transfer one unit (base on decimals) of the token from the amm pool into the
        // settlement contract and then to an arbitrary address.
        // Note that gas use can depend on the recipient because for the standard implementation
        // sending to an address that does not have any balance yet (implicitly 0) causes an
        // allocation.
        let request = self.create_trace_request(token, amount, take_from);
        let traces = trace_many::trace_many(request, &self.web3).await?;
        Self::handle_response(&traces, amount)
    }

    // Based on amm pools find the address with the largest amount of the token.
    // Err if communication with the node failed.
    // Ok(None) if there is no pool or getting the balance fails.
    // Ok(address, balance) for an address that has this amount of balance of the token.
    async fn find_largest_pool_owning_token(&self, token: H160) -> Result<Option<(H160, U256)>> {
        const BATCH_SIZE: usize = 100;

        let pairs = self
            .base_tokens
            .iter()
            .filter_map(move |&base_token| TokenPair::new(base_token, token));
        let candidates = pairs
            .flat_map(|pair| self.pools.iter().map(move |pool| pool.pair_address(&pair)))
            .collect::<HashSet<_>>();

        let instance = ERC20::at(&self.web3, token);
        let mut batch = CallBatch::new(self.web3.transport());
        let futures = candidates
            .iter()
            .map(|&address| {
                let fut = instance.balance_of(address).batch_call(&mut batch);
                async move { (address, fut.await) }
            })
            .collect::<Vec<_>>();
        batch.execute_all(BATCH_SIZE).await;

        let mut biggest_balance = None;
        for future in futures {
            let (address, result) = future.await;
            let balance = match result {
                Ok(balance) => balance,
                Err(err) => {
                    return match EthcontractErrorType::classify(&err) {
                        EthcontractErrorType::Node => Err(err.into()),
                        EthcontractErrorType::Contract => Ok(None),
                    }
                }
            };
            match biggest_balance {
                Some((_, current_biggest)) if current_biggest > balance => (),
                _ => biggest_balance = Some((address, balance)),
            }
        }
        Ok(biggest_balance)
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

        requests
    }

    fn handle_response(traces: &[BlockTrace], amount: U256) -> Result<TokenQuality> {
        ensure!(traces.len() == 7, "unexpected number of traces");

        let gas_in = match ensure_transaction_ok_and_get_gas(&traces[1])? {
            Ok(gas) => gas,
            Err(reason) => return Ok(TokenQuality::bad(reason)),
        };
        let gas_out = match ensure_transaction_ok_and_get_gas(&traces[4])? {
            Ok(gas) => gas,
            Err(reason) => return Ok(TokenQuality::bad(reason)),
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

        let balance_recpient_before = match decode_u256(&traces[3]) {
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

        if balance_after_in != balance_before_in + amount {
            return Ok(TokenQuality::bad(
                "balance after in transfer does not match",
            ));
        }
        if balance_after_out != balance_before_in {
            return Ok(TokenQuality::bad(
                "balance after out transfer does not match",
            ));
        }
        if balance_recpient_before + amount != balance_recipient_after {
            return Ok(TokenQuality::bad("balance of recipient does not match"));
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
    let transaction_traces = trace
        .trace
        .as_ref()
        .ok_or_else(|| anyhow!("trace not set"))?;
    let first = transaction_traces
        .first()
        .ok_or_else(|| anyhow!("expected at least one trace"))?;
    if first.error.is_some() {
        return Ok(Err("transaction failed".to_string()));
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
        amm_pair_provider::{SushiswapPairProvider, UniswapPairProvider},
        transport::LoggingTransport,
    };
    use hex_literal::hex;
    use web3::{
        transports::Http,
        types::{Action, ActionType, Bytes, Call, CallResult, CallType, Res, TransactionTrace},
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
        ];

        let result = TraceCallDetector::handle_response(traces, 1.into()).unwrap();
        let expected = TokenQuality::Good;
        assert_eq!(result, expected);
    }

    #[test]
    fn arbitrary_recipient_() {
        println!("{:?}", TraceCallDetector::arbitrary_recipient());
    }

    // cargo test -p orderbook mainnet_tokens -- --nocapture
    #[tokio::test]
    #[ignore]
    async fn mainnet_tokens() {
        // shared::tracing::initialize("orderbook::bad_token=debug,shared::transport=debug");
        let http = LoggingTransport::new(
            Http::new("https://dev-openethereum.mainnet.gnosisdev.com/").unwrap(),
        );
        let web3 = Web3::new(http);

        let base_tokens = &[
            H160(hex!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")), // weth
            H160(hex!("6B175474E89094C44Da98b954EedeAC495271d0F")), // dai
            H160(hex!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")), // usdc
            H160(hex!("dAC17F958D2ee523a2206206994597C13D831ec7")), // usdt
            H160(hex!("c00e94Cb662C3520282E6f5717214004A7f26888")), // comp
            H160(hex!("9f8F72aA9304c8B593d555F12eF6589cC3A579A2")), // mkr
            H160(hex!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599")), // wbtc
        ];

        // tokens from our deny list
        let bad_tokens = &[
            H160(hex!("C12D1c73eE7DC3615BA4e37E4ABFdbDDFA38907E")),
            H160(hex!("79ba92dda26fce15e1e9af47d5cfdfd2a093e000")),
            H160(hex!("bae5f2d8a1299e5c4963eaff3312399253f27ccb")),
            H160(hex!("4bae380b5d762d543d426331b8437926443ae9ec")),
            H160(hex!("2b1fe2cea92436e8c34b7c215af66aaa2932a8b2")),
            H160(hex!("c7c24fe893c21e8a4ef46eaf31badcab9f362841")),
            H160(hex!("ef5b32486ed432b804a51d129f4d2fbdf18057ec")),
            H160(hex!("79ba92dda26fce15e1e9af47d5cfdfd2a093e000")),
            H160(hex!("bae5f2d8a1299e5c4963eaff3312399253f27ccb")),
            H160(hex!("4bae380b5d762d543d426331b8437926443ae9ec")),
            H160(hex!("a2b4c0af19cc16a6cfacce81f192b024d625817d")),
            H160(hex!("072c46f392e729c1f0d92a307c2c6dba06b5d078")),
            H160(hex!("3a9fff453d50d4ac52a6890647b823379ba36b9e")),
            H160(hex!("62359ed7505efc61ff1d56fef82158ccaffa23d7")),
            H160(hex!("ea319e87cf06203dae107dd8e5672175e3ee976c")),
            H160(hex!("68e0a48d3bff6633a31d1d100b70f93c3859218b")),
            H160(hex!("31acf54fae6166dc2f90c4d6f20d379965e96bc1")),
            H160(hex!("d0d3ebcad6a20ce69bc3bc0e1ec964075425e533")),
            H160(hex!("a9a8377287ea9c6b8b4249dd502e75d34148fc5b")),
            H160(hex!("fad45e47083e4607302aa43c65fb3106f1cd7607")),
            H160(hex!("69692d3345010a207b759a7d1af6fc7f38b35c5e")),
            H160(hex!("298d492e8c1d909d3f63bc4a36c66c64acb3d695")),
            H160(hex!("6e10aacb89a28d6fa0fe68790777fec7e7f01890")),
            H160(hex!("944eee930933be5e23b690c8589021ec8619a301")),
            H160(hex!("d50825f50384bc40d5a10118996ef503b3670afd")),
            H160(hex!("8c7424c3000942e5a93de4a01ce2ec86c06333cb")),
            H160(hex!("106d3c66d22d2dd0446df23d7f5960752994d600")),
            H160(hex!("66d31def9c47b62184d7f57175eed5b5d9b7f038")),
            H160(hex!("bf04e48c5d8880306591ef888cde201d3984eb3e")),
            H160(hex!("88ef27e69108b2633f8e1c184cc37940a075cc02")),
            H160(hex!("9ea3b5b4ec044b70375236a281986106457b20ef")),
            H160(hex!("48be867b240d2ffaff69e0746130f2c027d8d3d2")),
            H160(hex!("4fab740779c73aa3945a5cf6025bf1b0e7f6349c")),
            H160(hex!("910524678c0b1b23ffb9285a81f99c29c11cbaed")),
            H160(hex!("ed5e5ab076ae60bdb9c49ac255553e65426a2167")),
            H160(hex!("14dd7ebe6cb084cb73ef377e115554d47dc9d61e")),
            H160(hex!("e0bdaafd0aab238c55d68ad54e616305d4a21772")),
            H160(hex!("c40af1e4fecfa05ce6bab79dcd8b373d2e436c4e")),
            H160(hex!("75fef397d74a2d11b64e6915cd847c1e7f8e5520")),
            H160(hex!("bf494f02ee3fde1f20bee6242bce2d1ed0c15e47")),
            H160(hex!("1337DEF18C680aF1f9f45cBcab6309562975b1dD")),
            H160(hex!("9aF15D7B8776fa296019979E70a5BE53c714A7ec")),
            H160(hex!("B96f0e9bb32760091eb2D6B0A5Ca0D2C7b5644B1")),
            H160(hex!("7FF4169a6B5122b664c51c95727d87750eC07c84")),
            H160(hex!("d5281bb2d1ee94866b03a0fccdd4e900c8cb5091")),
            H160(hex!("d1afbccc9a2c2187ea544363b986ea0ab6ef08b5")),
            H160(hex!("dd339f370bbb18b8f389bd0443329d82ecf4b593")),
            H160(hex!("577e7f9fa80ab33e87a01b701114257c8d9455a8")),
            H160(hex!("586c680e9a6d21b81ebecf46d78844dab7b3bcf9")),
            H160(hex!("c03841b5135600312707d39eb2af0d2ad5d51a91")),
            H160(hex!("07be1ead7aebee544618bdc688fa3cff09857c32")),
            H160(hex!("0858a26055d6584E5B47bbeCF7f7E8CBC390995b")),
            H160(hex!("0Ba45A8b5d5575935B8158a88C631E9F9C95a2e5")),
            H160(hex!("37611b28aCa5673744161Dc337128cfdD2657F69")),
            H160(hex!("389999216860AB8E0175387A0c90E5c52522C945")),
            H160(hex!("45734927Fa2f616FbE19E65f42A0ef3d37d1c80A")),
            H160(hex!("4F9254C83EB525f9FCf346490bbb3ed28a81C667")),
            H160(hex!("51d3e4C0b2c83E62f5d517D250b3e856897d2052")),
            H160(hex!("925f2C11b99c1A4c46606898ee91eD3D450cFedA")),
            H160(hex!("97b65710D03E12775189F0D113202cc1443b0aa2")),
            H160(hex!("f198B4a2631B7D0B9FAc36f8B546Ed3DCe472A47")),
            H160(hex!("1016f3c0a1939fa27538339da7e2a300031b6f37")),
            H160(hex!("7f0f118d083d5175ab9d2d34c4c8fa4f43c3f47b")),
            H160(hex!("d3f6571be1d91ac68b40daaa24075ca7e2f0f72e")),
            H160(hex!("bf25ea982b4f850dafb4a95367b890eee5a9e8f2")),
            H160(hex!("c73c167e7a4ba109e4052f70d5466d0c312a344d")),
            H160(hex!("1426cc6d52d1b14e2b3b1cb04d57ea42b39c4c7c")),
            H160(hex!("99ddddd8dfe33905338a073047cfad72e6833c06")),
            H160(hex!("4a6be56a211a4c4e0dd4474d524138933c17f3e3")),
            H160(hex!("9f41da75ab2b8c6f0dcef7173c4bf66bd4f6b36a")),
            H160(hex!("239dc02a28a0774738463e06245544a72745d5c5")),
            H160(hex!("801ea8c463a776e85344c565e355137b5c3324cd")),
            H160(hex!("aee53701e18d5ff6af4964c3a381e7d09b9b9075")),
            H160(hex!("3a6fe4c752eb8d571a660a776be4003d619c30a3")),
            H160(hex!("99043bb680ab9262c7b2ac524e00b215efb7db9b")),
            H160(hex!("18bdfc80b97cb97f6b466cce967849ce9cd9d58c")),
            H160(hex!("32c868f6318d6334b2250f323d914bc2239e4eee")),
            H160(hex!("c626d951eff8e421448074bd2ad7805c6d585793")),
            H160(hex!("94987bc8aa5f36cb2461c190134929a29c3df726")),
            H160(hex!("41933422dc4a1cb8c822e06f12f7b52fa5e7e094")),
            H160(hex!("b893a8049f250b57efa8c62d51527a22404d7c9a")),
            H160(hex!("f198b4a2631b7d0b9fac36f8b546ed3dce472a47")),
            H160(hex!("76851a93977bea9264c32255b6457882035c7501")),
            H160(hex!("a5959e9412d27041194c3c3bcbe855face2864f7")),
            H160(hex!("cf8c23cf17bb5815d5705a15486fa83805415625")),
            H160(hex!("63d0eea1d7c0d1e89d7e665708d7e8997c0a9ed6")),
            H160(hex!("98ecf3d8e21adaafe16c00cc3ff681e72690278b")),
            H160(hex!("3ea50b7ef6a7eaf7e966e2cb72b519c16557497c")),
            H160(hex!("2b1fe2cea92436e8c34b7c215af66aaa2932a8b2")),
            H160(hex!("c7c24fe893c21e8a4ef46eaf31badcab9f362841")),
            H160(hex!("ef5b32486ed432b804a51d129f4d2fbdf18057ec")),
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
        let chain_id = web3.eth().chain_id().await.unwrap().as_u64();
        let uniswap = Arc::new(UniswapPairProvider {
            factory: contracts::UniswapV2Factory::deployed(&web3).await.unwrap(),
            chain_id,
        });
        let sushiswap = Arc::new(SushiswapPairProvider {
            factory: contracts::SushiswapV2Factory::deployed(&web3)
                .await
                .unwrap(),
        });
        let token_cache = TraceCallDetector {
            web3,
            settlement_contract: settlement.address(),
            pools: vec![uniswap, sushiswap],
            base_tokens: base_tokens.iter().copied().collect(),
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
}
