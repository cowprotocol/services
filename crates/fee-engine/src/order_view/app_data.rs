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
    pub signer: Option<Address>,
    /// The uid of the order this one replaces.
    pub replaced_order: Option<[u8; 56]>,
    pub flashloan: Option<Flashloan>,
}

#[derive(Debug, Clone)]
pub struct Flashloan {
    pub liquidity_provider: Address,
    pub protocol_adapter: Address,
    pub receiver: Address,
    pub token: Address,
    pub amount: U256,
}

/// The app data under the app-data schema's names, typed like the rest of
/// the view.
pub(super) fn app_data_view(app_data: &AppData) -> Value {
    json!({
        "appCode": app_data.app_code,
        "metadata": {
            "signer": app_data.signer.map(|signer| hex(signer.as_slice())),
            "replacedOrder": app_data.replaced_order.map(|uid| json!({ "uid": hex(&uid) })),
            "flashloan": app_data.flashloan.as_ref().map(|flashloan| json!({
                "liquidityProvider": hex(flashloan.liquidity_provider.as_slice()),
                "protocolAdapter": hex(flashloan.protocol_adapter.as_slice()),
                "receiver": hex(flashloan.receiver.as_slice()),
                "token": hex(flashloan.token.as_slice()),
                "amount": number(flashloan.amount),
            })),
        },
    })
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
            signer: Some(aa),
            replaced_order: Some([0xab; 56]),
            flashloan: Some(Flashloan {
                liquidity_provider: aa,
                protocol_adapter: aa,
                receiver: aa,
                token: aa,
                amount: U256::from(6136714),
            }),
        };
        let aa = "0x00000000000000000000000000000000000000aa";
        assert_eq!(
            view(&app_data),
            json!({
                "appCode": "CoW Swap",
                "metadata": {
                    "signer": aa,
                    "replacedOrder": { "uid": format!("0x{}", "ab".repeat(56)) },
                    "flashloan": {
                        "liquidityProvider": aa,
                        "protocolAdapter": aa,
                        "receiver": aa,
                        "token": aa,
                        "amount": 6136714.0,
                    },
                },
            })
        );
    }

    #[test]
    fn default_app_data_has_an_empty_metadata() {
        assert_eq!(view(&AppData::default()), json!({ "metadata": {} }));
    }
}
