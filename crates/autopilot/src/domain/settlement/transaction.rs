/// Call data in a format expected by the settlement contract.
#[derive(Debug)]
pub struct CallData(pub crate::util::Bytes<Vec<u8>>);
