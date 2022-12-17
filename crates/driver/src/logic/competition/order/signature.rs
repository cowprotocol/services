/// Signature over the order data.
#[derive(Debug)]
pub struct Signature {
    pub data: Vec<u8>,
    pub scheme: Scheme,
}

/// The scheme used for signing the order. This is used by the solver and
/// the protocol, the driver does not care about the details of signature
/// verification.
#[derive(Debug)]
pub enum Scheme {
    Eip712,
    EthSign,
    Eip1271,
    PreSign,
}
