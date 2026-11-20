//! The order as a rule sees it: the fields a rule can read, resolved to
//! Rust accessors when the rules compile.
//!
//! Each object is a table of fields. A new field is a row in its object's
//! table and a line in the `order` section of the README.

mod app_data;

pub use app_data::AppData;
use {
    alloy_primitives::{Address, I256, U256},
    model::{order::OrderKind, signature::SigningScheme},
};

/// An order as the host hands it to the engine: the API order plus the facts
/// the host computes for it.
#[derive(Debug, Clone, Copy)]
pub struct OrderView<'a> {
    pub order: &'a model::order::Order,
    /// The order's app data, which the host copies from the parsed document.
    pub app_data: &'a AppData,
    /// The partner fees of the app data. Rules do not see them; the engine
    /// caps them and returns them apart (`Evaluation::partner_fees`).
    pub partner_fees: &'a [crate::PartnerFee],
    /// Whether the limit price is inside the market price of the order's quote;
    /// false when the order has no quote. The engine trusts it as given.
    pub in_market: bool,
}

/// A nested object of the view, such as `order` or `order.appData`.
pub(crate) struct Object {
    pub(crate) name: &'static str,
    /// Whether the object is in this order's view.
    pub(crate) present: fn(&OrderView<'_>) -> bool,
    pub(crate) fields: &'static [Field],
}

impl Object {
    pub(crate) fn field(&self, name: &str) -> Option<&'static Field> {
        self.fields.iter().find(|field| field.name == name)
    }
}

/// A name under an [`Object`].
pub(crate) struct Field {
    pub(crate) name: &'static str,
    pub(crate) kind: FieldKind,
}

/// What a field is: a nested object, or a value with its accessor.
pub(crate) enum FieldKind {
    Object(&'static Object),
    Value(Getter),
}

/// Reads a field; `None` when the order does not have it.
#[derive(Clone, Copy)]
pub(crate) enum Getter {
    Address(fn(&OrderView<'_>) -> Option<Address>),
    UInt(fn(&OrderView<'_>) -> Option<U256>),
    Int(fn(&OrderView<'_>) -> Option<I256>),
    Bool(fn(&OrderView<'_>) -> Option<bool>),
    Text(for<'a> fn(&OrderView<'a>) -> Option<&'a str>),
}

impl Getter {
    pub(crate) fn present(self, view: &OrderView<'_>) -> bool {
        match self {
            Getter::Address(get) => get(view).is_some(),
            Getter::UInt(get) => get(view).is_some(),
            Getter::Int(get) => get(view).is_some(),
            Getter::Bool(get) => get(view).is_some(),
            Getter::Text(get) => get(view).is_some(),
        }
    }
}

const fn value(name: &'static str, getter: Getter) -> Field {
    Field {
        name,
        kind: FieldKind::Value(getter),
    }
}

const fn object(name: &'static str, object: &'static Object) -> Field {
    Field {
        name,
        kind: FieldKind::Object(object),
    }
}

fn kind<'a>(view: &OrderView<'a>) -> Option<&'a str> {
    Some(match view.order.data.kind {
        OrderKind::Buy => "buy",
        OrderKind::Sell => "sell",
    })
}

fn signing_scheme<'a>(view: &OrderView<'a>) -> Option<&'a str> {
    Some(match view.order.signature.scheme() {
        SigningScheme::Eip712 => "eip712",
        SigningScheme::EthSign => "ethsign",
        SigningScheme::Eip1271 => "eip1271",
        SigningScheme::PreSign => "presign",
    })
}

pub(crate) static ORDER: Object = Object {
    name: "order",
    present: |_| true,
    fields: &[
        value(
            "owner",
            Getter::Address(|view| Some(view.order.metadata.owner)),
        ),
        value(
            "creationDate",
            // Unix seconds, as `validTo`.
            Getter::UInt(|view| {
                let seconds = view.order.metadata.creation_date.timestamp();
                Some(U256::from(seconds.max(0).unsigned_abs()))
            }),
        ),
        value(
            "sellToken",
            Getter::Address(|view| Some(view.order.data.sell_token)),
        ),
        value(
            "buyToken",
            Getter::Address(|view| Some(view.order.data.buy_token)),
        ),
        value(
            "receiver",
            Getter::Address(|view| Some(Address::from(*view.order.data.receiver.raw_bytes()))),
        ),
        value(
            "sellAmount",
            Getter::UInt(|view| Some(view.order.data.sell_amount)),
        ),
        value(
            "buyAmount",
            Getter::UInt(|view| Some(view.order.data.buy_amount)),
        ),
        value("kind", Getter::Text(kind)),
        value(
            "partiallyFillable",
            Getter::Bool(|view| Some(view.order.data.partially_fillable)),
        ),
        value("signingScheme", Getter::Text(signing_scheme)),
        value(
            "validTo",
            Getter::UInt(|view| Some(U256::from(view.order.data.valid_to))),
        ),
        value(
            "validFrom",
            Getter::UInt(|view| view.order.metadata.valid_from.map(U256::from)),
        ),
        object("appData", &app_data::APP_DATA),
        value(
            "fastPath",
            Getter::Bool(|view| Some(view.order.metadata.fast_path)),
        ),
        value("inMarket", Getter::Bool(|view| Some(view.in_market))),
        object("ethflowData", &ETHFLOW_DATA),
    ],
};

static ETHFLOW_DATA: Object =
    Object {
        name: "ethflowData",
        present: |view| view.order.metadata.ethflow_data.is_some(),
        fields: &[value(
            "userValidTo",
            Getter::Int(|view| {
                view.order.metadata.ethflow_data.as_ref().map(|ethflow| {
                    I256::try_from(ethflow.user_valid_to).expect("an i64 fits 256 bits")
                })
            }),
        )],
    };
