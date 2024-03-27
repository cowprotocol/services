use crate::domain::eth;

pub trait U256Ext: Sized {
    fn checked_ceil_div(&self, other: &Self) -> Option<Self>;
}

impl U256Ext for eth::U256 {
    fn checked_ceil_div(&self, other: &Self) -> Option<Self> {
        self.checked_add(other.checked_sub(1.into())?)?
            .checked_div(*other)
    }
}
