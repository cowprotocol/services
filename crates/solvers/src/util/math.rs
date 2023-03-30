use ethereum_types::U256;

/// Perform a ceiled U256 integer division.
///
/// Returns `None` when dividing by `0`.
pub fn div_ceil(q: U256, d: U256) -> Option<U256> {
    if d.is_zero() {
        return None;
    }

    let (r, rem) = q.div_mod(d);
    if rem.is_zero() {
        Some(r)
    } else {
        Some(
            r.checked_add(U256::one())
                .expect("unexpected ceiled division overflow"),
        )
    }
}
