//! The order's app data as a rule sees it.

use super::{Getter, Input, Object, object, value};

/// The app data a rule can read. The host copies it from the protocol's
/// parsed document; `AppData::default()` when the order has none.
#[derive(Debug, Clone, Default)]
pub struct AppData {
    pub app_code: Option<String>,
    /// Whether the document asks for a flashloan.
    pub flashloan: bool,
}

fn app_code<'a>(input: &Input<'a>) -> Option<&'a str> {
    input.order.app_data.app_code.as_deref()
}

pub(super) static APP_DATA: Object = Object {
    name: "appData",
    present: |_| true,
    fields: &[
        value("appCode", Getter::Text(app_code)),
        object("metadata", &METADATA),
    ],
};

static METADATA: Object = Object {
    name: "metadata",
    present: |_| true,
    fields: &[value(
        "flashloan",
        Getter::Bool(|i| i.order.app_data.flashloan.then_some(true)),
    )],
};
