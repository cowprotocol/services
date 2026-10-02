use {
    alloy_primitives::Address,
    serde::{Deserialize, Deserializer},
};

/// Deserializes a `receiver` field, mapping both a missing field and an
/// explicit `null` to [`Address::ZERO`] (the settlement contract's "pay the
/// owner" sentinel). Combine with `#[serde(default, deserialize_with = …)]`
/// on request-body DTOs so a client that sends `"receiver": null` still
/// parses.
pub fn deserialize_receiver_defaulting_to_zero<'de, D>(d: D) -> Result<Address, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<Address>::deserialize(d)?.unwrap_or(Address::ZERO))
}
