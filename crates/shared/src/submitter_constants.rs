/// Error messages which suggest that the node is already aware of the submitted
/// tx thus prompting us to increase the replacement gas price.
pub const TX_ALREADY_KNOWN: &[&str] = &[
    "Transaction gas price supplied is too low", //openethereum
    "already known",                             //infura, erigon, eden
    "INTERNAL_ERROR: existing tx with same hash", //erigon
    "replacement transaction underpriced",       //eden
];

/// Error messages suggesting that the transaction we tried to submit has
/// already been mined because its nonce is suddenly too low.
pub const TX_ALREADY_MINED: &[&str] = &[
    "Transaction nonce is too low", //openethereum
    "nonce too low",                //infura, erigon
    "OldNonce",                     //erigon
];
