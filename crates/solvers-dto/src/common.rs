use utoipa::ToSchema;

// Structs for the utoipa OpenAPI schema generator.

/// Signature bytes.
#[derive(ToSchema)]
#[schema(
    example = "0x0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
)]
#[allow(dead_code)]
pub struct Signature(String);

/// 32 bytes of arbitrary application specific data that can be added to an
/// order. This can also be used to ensure uniqueness between two orders with
/// otherwise the exact same parameters.
#[derive(ToSchema)]
#[schema(example = "0x0000000000000000000000000000000000000000000000000000000000000000")]
#[allow(dead_code)]
pub struct AppData(String);

/// Amount of an ERC20 token. 256 bit unsigned integer in decimal notation.
#[derive(ToSchema)]
#[schema(example = "1234567890")]
#[allow(dead_code)]
pub struct TokenAmount(String);

/// An Ethereum public address.
#[derive(ToSchema)]
#[schema(example = "0x0000000000000000000000000000000000000000")]
#[allow(dead_code)]
pub struct Address(String);

/// An ERC20 token address.
#[derive(ToSchema)]
#[schema(example = "0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB")]
#[allow(dead_code)]
pub struct Token(String);
