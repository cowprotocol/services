//! The order's app data as a rule sees it.

use {
    super::{map, serialized, text},
    cel::common::value::Val,
};

/// The app data a rule can read. The host copies it from the protocol's
/// parsed document; `AppData::default()` when the order has none.
#[derive(Debug, Clone, Default)]
pub struct AppData {
    pub app_code: Option<String>,
    /// Whether the document asks for a flashloan.
    pub flashloan: bool,
}

/// The app data under the app-data schema's names, typed like the rest of
/// the view. A flashloan shows as `true`; a rule reads only its presence.
pub(super) fn app_data_view(app_data: &AppData) -> Box<dyn Val> {
    map([
        ("appCode", app_data.app_code.clone().and_then(text)),
        (
            "metadata",
            Some(map([(
                "flashloan",
                app_data.flashloan.then(|| serialized(true)).flatten(),
            )])),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        serde_json::{Value, json},
    };

    /// The view as JSON.
    fn view(app_data: &AppData) -> Value {
        crate::order_view::json(app_data_view(app_data).as_ref())
    }

    #[test]
    fn app_data_view_types_every_field() {
        let app_data = AppData {
            app_code: Some("CoW Swap".to_owned()),
            flashloan: true,
        };
        assert_eq!(
            view(&app_data),
            json!({ "appCode": "CoW Swap", "metadata": { "flashloan": true } })
        );
    }

    #[test]
    fn default_app_data_has_an_empty_metadata() {
        assert_eq!(view(&AppData::default()), json!({ "metadata": {} }));
    }
}
