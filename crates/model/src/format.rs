pub fn debug_optional_bytes(
    bytes: &Option<impl AsRef<[u8]>>,
    formatter: &mut std::fmt::Formatter,
) -> Result<(), std::fmt::Error> {
    match bytes {
        Some(bytes) => formatter.write_fmt(format_args!("0x{}", hex::encode(bytes.as_ref()))),
        None => formatter.write_str("None"),
    }
}
