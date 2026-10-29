//! The order's app data as a rule sees it.

use {
    super::{hex, number},
    alloy_primitives::{Address, U256},
    serde_json::{Value, json},
};

/// The app data a rule can read. The host copies it from the protocol's
/// parsed document; `AppData::default()` when the order has none.
#[derive(Debug, Clone, Default)]
pub struct AppData {
    pub app_code: Option<String>,
    pub hooks: Hooks,
    pub signer: Option<Address>,
    /// The uid of the order this one replaces.
    pub replaced_order: Option<[u8; 56]>,
    pub partner_fees: Vec<PartnerFee>,
    pub flashloan: Option<Flashloan>,
    pub wrappers: Vec<WrapperCall>,
}

#[derive(Debug, Clone, Default)]
pub struct Hooks {
    pub pre: Vec<Hook>,
    pub post: Vec<Hook>,
}

#[derive(Debug, Clone)]
pub struct Hook {
    pub target: Address,
    pub call_data: Vec<u8>,
    pub gas_limit: u64,
}

#[derive(Debug, Clone)]
pub struct PartnerFee {
    pub policy: PartnerFeePolicy,
    pub recipient: Address,
}

#[derive(Debug, Clone)]
pub enum PartnerFeePolicy {
    Volume { bps: u64 },
    Surplus { bps: u64, max_volume_bps: u64 },
    PriceImprovement { bps: u64, max_volume_bps: u64 },
}

#[derive(Debug, Clone)]
pub struct Flashloan {
    pub liquidity_provider: Address,
    pub protocol_adapter: Address,
    pub receiver: Address,
    pub token: Address,
    pub amount: U256,
}

#[derive(Debug, Clone)]
pub struct WrapperCall {
    pub address: Address,
    pub data: Vec<u8>,
    pub is_omittable: bool,
}

/// The app data under the app-data schema's names, typed like the rest of
/// the view.
pub(super) fn app_data_view(app_data: &AppData) -> Value {
    json!({
        "appCode": app_data.app_code,
        "metadata": {
            "hooks": {
                "pre": app_data.hooks.pre.iter().map(hook).collect::<Vec<_>>(),
                "post": app_data.hooks.post.iter().map(hook).collect::<Vec<_>>(),
            },
            "signer": app_data.signer.map(|signer| hex(signer.as_slice())),
            "replacedOrder": app_data.replaced_order.map(|uid| json!({ "uid": hex(&uid) })),
            "partnerFee": app_data.partner_fees.iter().map(partner_fee).collect::<Vec<_>>(),
            "flashloan": app_data.flashloan.as_ref().map(|flashloan| json!({
                "liquidityProvider": hex(flashloan.liquidity_provider.as_slice()),
                "protocolAdapter": hex(flashloan.protocol_adapter.as_slice()),
                "receiver": hex(flashloan.receiver.as_slice()),
                "token": hex(flashloan.token.as_slice()),
                "amount": number(flashloan.amount),
            })),
            "wrappers": app_data.wrappers.iter().map(|wrapper| json!({
                "address": hex(wrapper.address.as_slice()),
                "data": hex(&wrapper.data),
                "isOmittable": wrapper.is_omittable,
            })).collect::<Vec<_>>(),
        },
    })
}

fn hook(hook: &Hook) -> Value {
    json!({
        "target": hex(hook.target.as_slice()),
        "callData": hex(&hook.call_data),
        "gasLimit": hook.gas_limit,
    })
}

/// One partner fee under the app-data schema's names.
fn partner_fee(fee: &PartnerFee) -> Value {
    let mut view = match fee.policy {
        PartnerFeePolicy::Volume { bps } => json!({ "volumeBps": bps }),
        PartnerFeePolicy::Surplus {
            bps,
            max_volume_bps,
        } => json!({ "surplusBps": bps, "maxVolumeBps": max_volume_bps }),
        PartnerFeePolicy::PriceImprovement {
            bps,
            max_volume_bps,
        } => json!({ "priceImprovementBps": bps, "maxVolumeBps": max_volume_bps }),
    };
    view["recipient"] = hex(fee.recipient.as_slice()).into();
    view
}

#[cfg(test)]
mod tests {
    use {super::*, alloy_primitives::address};

    /// The view as the order view holds it: no `null` fields.
    fn view(app_data: &AppData) -> Value {
        let mut view = app_data_view(app_data);
        crate::order_view::remove_nulls(&mut view);
        view
    }

    #[test]
    fn app_data_view_types_every_field() {
        let aa = address!("0x00000000000000000000000000000000000000Aa");
        let app_data = AppData {
            app_code: Some("CoW Swap".to_owned()),
            hooks: Hooks {
                pre: vec![Hook {
                    target: aa,
                    call_data: vec![0xab],
                    gas_limit: 21000,
                }],
                post: Vec::new(),
            },
            signer: Some(aa),
            replaced_order: Some([0xab; 56]),
            partner_fees: vec![
                PartnerFee {
                    policy: PartnerFeePolicy::Volume { bps: 25 },
                    recipient: aa,
                },
                PartnerFee {
                    policy: PartnerFeePolicy::Surplus {
                        bps: 50,
                        max_volume_bps: 100,
                    },
                    recipient: aa,
                },
                PartnerFee {
                    policy: PartnerFeePolicy::PriceImprovement {
                        bps: 30,
                        max_volume_bps: 90,
                    },
                    recipient: aa,
                },
            ],
            flashloan: Some(Flashloan {
                liquidity_provider: aa,
                protocol_adapter: aa,
                receiver: aa,
                token: aa,
                amount: U256::from(6136714),
            }),
            wrappers: vec![WrapperCall {
                address: aa,
                data: vec![0xab],
                is_omittable: true,
            }],
        };
        let aa = "0x00000000000000000000000000000000000000aa";
        assert_eq!(
            view(&app_data),
            json!({
                "appCode": "CoW Swap",
                "metadata": {
                    "hooks": {
                        "pre": [{ "target": aa, "callData": "0xab", "gasLimit": 21000 }],
                        "post": [],
                    },
                    "signer": aa,
                    "replacedOrder": { "uid": format!("0x{}", "ab".repeat(56)) },
                    "partnerFee": [
                        { "volumeBps": 25, "recipient": aa },
                        { "surplusBps": 50, "maxVolumeBps": 100, "recipient": aa },
                        { "priceImprovementBps": 30, "maxVolumeBps": 90, "recipient": aa },
                    ],
                    "flashloan": {
                        "liquidityProvider": aa,
                        "protocolAdapter": aa,
                        "receiver": aa,
                        "token": aa,
                        "amount": 6136714.0,
                    },
                    "wrappers": [{ "address": aa, "data": "0xab", "isOmittable": true }],
                },
            })
        );
    }

    #[test]
    fn default_app_data_has_only_the_empty_lists() {
        // The lists are there to read without `has`.
        assert_eq!(
            view(&AppData::default()),
            json!({ "metadata": {
                "hooks": { "pre": [], "post": [] },
                "partnerFee": [],
                "wrappers": [],
            } })
        );
    }
}
