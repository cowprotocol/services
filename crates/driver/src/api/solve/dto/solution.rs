use {crate::logic, serde::Serialize, serde_with::serde_as};

impl From<logic::competition::Score> for Solution {
    fn from(score: logic::competition::Score) -> Self {
        Self {
            objective: score.into(),
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
pub struct Solution {
    objective: f64,
}
