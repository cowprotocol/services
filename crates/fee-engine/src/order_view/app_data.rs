//! The order's app data as a rule sees it.

use {
    super::{hex, map, number, text},
    alloy_primitives::{Address, U256},
    cel::common::value::Val,
    model::order::OrderUid,
};

/// The app data a rule can read. The host copies it from the protocol's
/// parsed document; `AppData::default()` when the order has none.
#[derive(Debug, Clone, Default)]
pub struct AppData {
    pub app_code: Option<String>,
    pub signer: Option<Address>,
    pub replaced_order: Option<OrderUid>,
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
pub(super) fn app_data_view(app_data: &AppData) -> Box<dyn Val> {
    let address = |address: Address| text(hex(address.as_slice()));
    map([
        ("appCode", app_data.app_code.clone().and_then(text)),
        (
            "metadata",
            Some(map([
                ("signer", app_data.signer.and_then(address)),
                (
                    "replacedOrder",
                    app_data
                        .replaced_order
                        .map(|uid| map([("uid", text(hex(&uid.0)))])),
                ),
                (
                    "flashloan",
                    app_data.flashloan.as_ref().map(|flashloan| {
                        map([
                            ("liquidityProvider", address(flashloan.liquidity_provider)),
                            ("protocolAdapter", address(flashloan.protocol_adapter)),
                            ("receiver", address(flashloan.receiver)),
                            ("token", address(flashloan.token)),
                            ("amount", number(flashloan.amount)),
                        ])
                    }),
                ),
            ])),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy_primitives::address,
        serde_json::{Value, json},
    };

    /// The view as JSON.
    fn view(app_data: &AppData) -> Value {
        crate::order_view::json(app_data_view(app_data).as_ref())
    }

    #[test]
    fn app_data_view_types_every_field() {
        let aa = address!("0x00000000000000000000000000000000000000Aa");
        let app_data = AppData {
            app_code: Some("CoW Swap".to_owned()),
            signer: Some(aa),
            replaced_order: Some(OrderUid([0xab; 56])),
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
