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

    /// Raw 20 bytes for signature hashing. Deliberately not an `Address` —
    /// use [`Receiver::resolve`] when you want the effective payout address.
    pub fn raw_bytes(&self) -> &[u8; 20] {
        &self.0.0
    }

    /// Whether this is the zero-address sentinel (= "pay the owner").
    pub fn is_default(self) -> bool {
        self.0.is_zero()
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
    /// Accepts an address string, a missing field (combined with
    /// `#[serde(default)]`), or an explicit `null`. All three forms that
    /// mean "no custom receiver" deserialize to [`Receiver::OWNER`].
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(
            Option::<Address>::deserialize(deserializer)?.unwrap_or(Address::ZERO),
        ))
    }
}
