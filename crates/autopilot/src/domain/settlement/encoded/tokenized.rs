use ethcontract::{Address, Bytes, U256};

// Original type for input of `GPv2Settlement.settle` function.
pub type Settlement = (
    Vec<Token>,
    Vec<ClearingPrice>,
    Vec<Trade>,
    [Vec<Interaction>; 3],
);

pub type Token = Address;
pub type ClearingPrice = U256;
pub type Trade = (
    U256,            // sellTokenIndex
    U256,            // buyTokenIndex
    Address,         // receiver
    U256,            // sellAmount
    U256,            // buyAmount
    u32,             // validTo
    Bytes<[u8; 32]>, // appData
    U256,            // feeAmount
    U256,            // flags
    U256,            // executedAmount
    Bytes<Vec<u8>>,  // signature
);
pub type Interaction = (Address, U256, Bytes<Vec<u8>>);
