use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// These types are going to be updated once the openapi documentation gets
// merged. Consider them a placeholder.

pub type SolverCompetitionResponse = HashMap<String, SolverSettlement>;

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct SolverSettlement {
    objective_value: f64,
    gas_estimate: u64,
    #[serde(with = "crate::bytes_hex")]
    call_data: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize() {
        let correct = serde_json::json!(
            {
                "objective_value": 1.0,
                "gas_estimate": 1,
                "call_data": "0x01"
            }
        );
        let orig = SolverSettlement {
            objective_value: 1.0,
            gas_estimate: 1,
            call_data: vec![1],
        };
        let serialized = serde_json::to_value(&orig).unwrap();
        assert_eq!(correct, serialized);
        let deserialized: SolverSettlement = serde_json::from_value(correct).unwrap();
        assert_eq!(orig, deserialized);
    }
}
