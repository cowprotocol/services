use {
    super::{eth, swap},
    std::ops::Mul,
};

/// The price between [`swap::FromToken`] and [`swap::ToToken`].
#[derive(Debug, Clone, Copy)]
pub struct Price {
    from: swap::FromAmount,
    to: swap::ToAmount,
}

impl Price {
    pub fn new(from: swap::FromAmount, to: swap::ToAmount) -> Self {
        Self { from, to }
    }
}

/// Apply the price to the [`swap::FromAmount`].
impl Mul<swap::FromAmount> for Price {
    type Output = swap::ToAmount;

    fn mul(self, rhs: swap::FromAmount) -> Self::Output {
        (eth::U256::from(rhs) * eth::U256::from(self.to) / eth::U256::from(self.from)).into()
    }
}

/// Apply the price to the [`swap::ToAmount`].
impl Mul<swap::ToAmount> for Price {
    type Output = swap::FromAmount;

    fn mul(self, rhs: swap::ToAmount) -> Self::Output {
        (eth::U256::from(rhs) * eth::U256::from(self.from) / eth::U256::from(self.to)).into()
    }
}
