use {
    alloy_primitives::{Address, Bytes, U256},
    alloy_rpc_types::trace::geth::CallFrame,
    alloy_sol_types::SolCall,
    contracts::{
        ERC20::ERC20,
        FlashLoanRouter::FlashLoanRouter,
        GPv2Settlement::GPv2Settlement,
        ICowWrapper::ICowWrapper,
    },
};

#[derive(Debug, PartialEq, Eq)]
pub struct SimulationReport {
    pub events: Vec<Event>,
    pub returned_bytes: Option<Bytes>,
    pub revert: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    /// Wrapper runs setup code before calling next wrapper or settlement
    /// contract. We also consider the flashloan router a wrapper.
    WrapperEntered {
        wrapper: Address,
    },
    /// The wrapper chain's setup code ran successfully and now we reached the
    /// `settle()` function.
    SettlementEntered,
    /// The trampoline contract catches reverts of user hooks so any revert
    /// inside the hook gets surfaced as a `caught_error`.
    Hook {
        target: Address,
        caught_error: Option<String>,
    },
    /// All orders had valid signatures.
    SignatureCheck,
    /// ERC20 token transfer into or from the settlement contract.
    Transfer {
        token: Address,
        from: Address,
        to: Address,
        amount: U256,
        revert: Option<String>,
    },
    /// `settle()` call terminated.
    SettlementExited {
        revert: Option<String>,
    },
    /// We reached the wrapper's post `settle()` clean up logic.
    WrapperExited {
        wrapper: Address,
        revert: Option<String>,
    },
    EndOfSimulation {
        reason: Option<String>,
    },
}

pub struct Context<'a> {
    pub settlement: &'a Address,
    pub vault_relayer: &'a Address,
    pub trampoline: &'a Address,
}

pub fn generate_settlement_report(context: Context, trace: CallFrame) -> SimulationReport {
    let mut events = Vec::new();
    process_frame(&trace, &context, &mut events);
    events.push(Event::EndOfSimulation {
        reason: trace.revert_reason.clone(),
    });
    SimulationReport {
        events,
        returned_bytes: trace.output,
        revert: trace.revert_reason,
    }
}

fn process_frame(frame: &CallFrame, ctx: &Context, events: &mut Vec<Event>) {
    let to = frame.to.unwrap_or_default();

    if ICowWrapper::wrappedSettleCall::abi_decode(&frame.input).is_ok()
        // since flashloans are just a specific variant of a wrapper we just consider
        // them the same thing
        || FlashLoanRouter::settlementContractCall::abi_decode(&frame.input).is_ok()
    {
        events.push(Event::WrapperEntered { wrapper: to });
        for sub in &frame.calls {
            process_frame(sub, ctx, events);
        }
        events.push(Event::WrapperExited {
            wrapper: to,
            revert: frame.revert_reason.clone(),
        });
    } else if to == *ctx.settlement && GPv2Settlement::settleCall::abi_decode(&frame.input).is_ok()
    {
        events.push(Event::SettlementEntered);
        for sub in &frame.calls {
            process_settle_frame(sub, ctx, events);
        }
        events.push(Event::SettlementExited {
            revert: frame.revert_reason.clone(),
        });
    } else {
        for sub in &frame.calls {
            process_frame(sub, ctx, events);
        }
    }
}

/// This function expects to get the [`CallFrame`] of the `settle()` call to be
/// passed in. Since we are only interested in a relatively high level report of
/// the `settle()` call this function does not traverse the call stack to the
/// end from here.
fn process_settle_frame(frame: &CallFrame, ctx: &Context, events: &mut Vec<Event>) {
    let to = frame.to.unwrap_or_default();

    if to == *ctx.trampoline {
        for sub in &frame.calls {
            events.push(Event::Hook {
                target: sub.to.unwrap_or_default(),
                caught_error: sub.revert_reason.clone(),
            });
        }
    } else if to == *ctx.vault_relayer {
        // Inside `settle()` we only call the vault relayer to transfer funds into
        // the settlement contract. Since this happens AFTER the signature check
        // and signature checks don't always require `CALL`s we use this checkpoint
        // to also signal that the `SignatureCheck` was successful.
        events.push(Event::SignatureCheck);

        for sub in &frame.calls {
            if let Ok(call) = ERC20::transferFromCall::abi_decode(&sub.input) {
                events.push(Event::Transfer {
                    token: sub.to.unwrap_or_default(),
                    from: call.sender,
                    to: call.recipient,
                    amount: call.amount,
                    revert: sub.revert_reason.clone(),
                });
            }
        }
    } else if let Ok(call) = ERC20::transferCall::abi_decode(&frame.input) {
        events.push(Event::Transfer {
            token: to,
            from: frame.from,
            to: call.recipient,
            amount: call.amount,
            revert: frame.revert_reason.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy_primitives::{address, b256, uint},
        alloy_provider::ext::DebugApi,
        alloy_rpc_types::trace::geth::{CallConfig, GethDebugTracingOptions},
        ethrpc::Web3,
    };

    #[ignore]
    #[tokio::test]
    async fn xdai_multiple_wrappers() {
        let provider = Web3::new_from_env().provider;
        let trace = provider
            .debug_trace_transaction_call(
                b256!("0xb355c209672c9289316d1ae41ce1722a16b60e9aaf57576ec6c5a570a4afefc0"),
                GethDebugTracingOptions::call_tracer(CallConfig::default()),
            )
            .await
            .unwrap();
        let context = Context {
            settlement: &address!("0xf553d092b50bdcbdded1a99af2ca29fbe5e2cb13"),
            vault_relayer: &address!("0xc7242d167563352e2bca4d71c043fbe542db8fb2"),
            trampoline: &address!("0xd496f9fcfba14d7bd1e45e4840d38ad85ded14dd"),
        };
        let report = generate_settlement_report(context, trace);
        assert_eq!(
            report,
            SimulationReport {
                events: vec![
                    Event::WrapperEntered {
                        wrapper: address!("0x2e3fdee28d7224ed140b4ea08c57f47546679363"),
                    },
                    Event::WrapperEntered {
                        wrapper: address!("0x531636e6e18f3a52c283accda39d7185e4597a37"),
                    },
                    Event::SettlementEntered,
                    Event::SignatureCheck,
                    Event::Transfer {
                        token: address!("0xe91d153e0b41518a2ce8dd3d7944fa863463a97d"),
                        from: address!("0xcbf50aa2d442548aed93915da99d827e71473dd1"),
                        to: address!("0xf553d092b50bdcbdded1a99af2ca29fbe5e2cb13"),
                        amount: uint!(400000000000000000_U256),
                        revert: None,
                    },
                    Event::Transfer {
                        token: address!("0x6a023ccd1ff6f2045c3309768ead9e68f978f6e1"),
                        from: address!("0xf553d092b50bdcbdded1a99af2ca29fbe5e2cb13"),
                        to: address!("0xcbf50aa2d442548aed93915da99d827e71473dd1"),
                        amount: uint!(237830129172216_U256),
                        revert: None,
                    },
                    Event::SettlementExited { revert: None },
                    Event::WrapperExited {
                        wrapper: address!("0x531636e6e18f3a52c283accda39d7185e4597a37"),
                        revert: None,
                    },
                    Event::WrapperExited {
                        wrapper: address!("0x2e3fdee28d7224ed140b4ea08c57f47546679363"),
                        revert: None,
                    },
                    Event::EndOfSimulation { reason: None },
                ],
                returned_bytes: Some(
                    alloy_primitives::hex!(
                        "0xd20e71e700000000000000000000000000000000000000000000000000000000"
                    )
                    .into(),
                ),
                revert: None,
            }
        );
    }

    #[ignore]
    #[tokio::test]
    async fn mainnet_detects_caught_hook_reverts() {
        let provider = Web3::new_from_env().provider;
        let trace = provider
            .debug_trace_transaction_call(
                b256!("0x642ba5e7461210e1190e4a63c85e7aafdb7fa15b2910cc7eaba4b66016481072"),
                GethDebugTracingOptions::call_tracer(CallConfig::default()),
            )
            .await
            .unwrap();
        let context = Context {
            settlement: &address!("0x9008d19f58aabd9ed0d60971565aa8510560ab41"),
            vault_relayer: &address!("0xc92e8bdf79f0507f65a392b0ab4667716bfe0110"),
            trampoline: &address!("0x60bf78233f48ec42ee3f101b9a05ec7878728006"),
        };
        let report = generate_settlement_report(context, trace);
        assert_eq!(
            report,
            SimulationReport {
                events: vec![
                    Event::SettlementEntered,
                    // tx succeeded despite the hook throwing an error. Also despite
                    // the trampoline contract catching the error we can still see
                    // the revert message.
                    Event::Hook {
                        target: address!("0xc139190f447e929f090edeb554d95abb8b18ac1c"),
                        caught_error: Some("ERC20Permit: invalid signature".to_string()),
                    },
                    Event::SignatureCheck,
                    Event::Transfer {
                        token: address!("0xc139190f447e929f090edeb554d95abb8b18ac1c"),
                        from: address!("0x255a1804125356422354f0252b182b8efad1b329"),
                        to: address!("0x9008d19f58aabd9ed0d60971565aa8510560ab41"),
                        amount: uint!(800000000000000000000_U256),
                        revert: None,
                    },
                    Event::Transfer {
                        token: address!("0x6f40d4a6237c257fff2db00fa0510deeecd303eb"),
                        from: address!("0x9008d19f58aabd9ed0d60971565aa8510560ab41"),
                        to: address!("0x255a1804125356422354f0252b182b8efad1b329"),
                        amount: uint!(792079207920792079208_U256),
                        revert: None,
                    },
                    Event::SettlementExited { revert: None },
                    Event::EndOfSimulation { reason: None },
                ],
                revert: None,
                returned_bytes: None,
            }
        );
    }
}
