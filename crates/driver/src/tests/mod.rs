mod boundary;
mod cases;
mod setup;

pub use setup::setup;

fn hex_address(value: ethcontract::H160) -> String {
    const_hex::encode_prefixed(value.as_bytes())
}
