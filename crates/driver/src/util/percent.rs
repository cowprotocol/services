use thiserror::Error;

/// A percentage value. The value is guaranteed to be in the range [0,1].
#[derive(Clone, Copy, Debug)]
pub struct Percent(f64);

impl Percent {
    pub fn get(&self) -> f64 {
        self.0
    }
}

impl TryInto<Percent> for f64 {
    type Error = OutOfRangeError;

    fn try_into(self) -> Result<Percent, Self::Error> {
        if !(0.0..=1.0).contains(&self) {
            return Err(OutOfRangeError);
        }
        Ok(Percent(self))
    }
}

#[derive(Debug, Error)]
#[error("the percentage is out of expected range [0, 100]")]
pub struct OutOfRangeError;
