mod boundary;
mod cases;
mod setup;

pub use setup::setup;

fn hex_address(value: ethcontract::H160) -> String {
    format!("0x{}", hex::encode(value.as_bytes()))
}
