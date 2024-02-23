//! Settlements are scored based on the CIP38 rules.
//! 
//! Score = surplus + protocol fee.

pub type Score = eth::TokenAmount;

// TODO: implement