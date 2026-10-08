use {
    alloy_primitives::Address,
    serde::{Deserialize, Deserializer, Serialize},
};

/// Address that will receive the `buy_token` of an order.
///
/// The settlement contract treats the all-zero address as a sentinel meaning
/// "pay the order's owner".
///
/// - [`Receiver::resolve`] — returns the actual payout address, substituting
///   `owner` when the sentinel is set.
/// - [`Receiver::raw_bytes`] — returns the 20 raw bytes. Reserved for signature
///   hashing (EIP‑712); deliberately not an [`Address`] so callers can't bypass
///   [`resolve`] by accident.
///
/// `Deserialize` tolerates both a missing field and an explicit `null`
/// (combine with `#[serde(default)]` on request-body DTOs) — see the inline
/// `impl` for details.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Receiver(Address);

impl Receiver {
    /// Sentinel value meaning "pay the owner".
    pub const OWNER: Self = Self(Address::ZERO);

    pub const fn new(addr: Address) -> Self {
        Self(addr)
    }

    /// Effective payout address: the inner value when a custom receiver was
    /// set, otherwise `owner` (because the settlement contract treats
    /// `0x0000…` as "pay the owner").
    pub fn resolve(self, owner: Address) -> Address {
        if self.is_default() { owner } else { self.0 }
    }

    /// Raw 20 bytes the user actually signed - use this when persisting the
    /// receiver somewhere, computing the order hash or encoding calldata.
    /// Deliberately not an `Address` — use [`Receiver::resolve`] when you
    /// want the effective payout address.
    pub fn raw_bytes(&self) -> &[u8; 20] {
        &self.0.0
    }

    /// Whether this is the zero-address sentinel (= "pay the owner").
    pub fn is_default(self) -> bool {
        self == Self::OWNER
    }

    /// The address the user explicitly specified as a custom receiver, or
    /// `None` for the zero-sentinel (= "pay the owner"). Use this when you
    /// need to act on a *custom* receiver.
    pub fn as_custom(self) -> Option<Address> {
        (!self.is_default()).then_some(self.0)
    }
}

impl From<Address> for Receiver {
    fn from(addr: Address) -> Self {
        Self(addr)
    }
}

impl<'de> Deserialize<'de> for Receiver {
    /// Accepts an address string, a missing field  or an explicit `null`.
    /// All three forms that mean "no custom receiver" deserialize to
    /// [`Receiver::OWNER`].
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(
            Option::<Address>::deserialize(deserializer)?.unwrap_or(Address::ZERO),
        ))
    }
}

#[cfg(test)]
mod tests {
    use {super::*, serde::Deserialize, serde_json::json};

    /// Container without `#[serde(default)]` on `receiver`.
    #[derive(Debug, Deserialize)]
    struct Strict {
        receiver: Receiver,
    }

    /// Same shape with `#[serde(default)]`: a missing field falls back to
    /// `Receiver::default()` (= `Receiver::OWNER`) without even calling the
    /// custom deserializer.
    #[derive(Debug, Deserialize)]
    struct Defaulting {
        #[serde(default)]
        receiver: Receiver,
    }

    #[test]
    fn explicit_address_parses() {
        let v = json!({ "receiver": "0x3333333333333333333333333333333333333333" });
        let strict: Strict = serde_json::from_value(v.clone()).unwrap();
        let defaulting: Defaulting = serde_json::from_value(v).unwrap();
        assert_eq!(strict.receiver, Receiver::new(Address::repeat_byte(0x33)));
        assert_eq!(
            defaulting.receiver,
            Receiver::new(Address::repeat_byte(0x33))
        );
    }

    #[test]
    fn explicit_null_parses_as_owner() {
        let v = json!({ "receiver": null });
        let strict: Strict = serde_json::from_value(v.clone()).unwrap();
        let defaulting: Defaulting = serde_json::from_value(v).unwrap();
        assert_eq!(strict.receiver, Receiver::OWNER);
        assert_eq!(defaulting.receiver, Receiver::OWNER);
    }

    /// A missing field is tolerated **without** `#[serde(default)]` only
    /// because `Receiver::deserialize` internally calls
    /// `Option::<Address>::deserialize`, and serde-json routes a missing
    /// key through `deserialize_option`, which `Option` happily accepts as
    /// `None`. Other data formats (CBOR, bincode, …) raise "missing field"
    /// instead — so DTOs that need to be format-agnostic (and anything we
    /// expose over HTTP via JSON) should still annotate the field with
    /// `#[serde(default)]`.
    #[test]
    fn missing_field_is_tolerated_by_serde_json() {
        // Via `from_value` (works because serde-json routes missing keys
        // through `Option`).
        let strict: Strict = serde_json::from_value(json!({})).unwrap();
        assert_eq!(strict.receiver, Receiver::OWNER);

        // Same via `from_str` for completeness.
        let strict: Strict = serde_json::from_str("{}").unwrap();
        assert_eq!(strict.receiver, Receiver::OWNER);

        // `#[serde(default)]` is the belt-and-braces option; it works
        // regardless of what the data format does with missing keys.
        let defaulting: Defaulting = serde_json::from_str("{}").unwrap();
        assert_eq!(defaulting.receiver, Receiver::OWNER);
    }
}
