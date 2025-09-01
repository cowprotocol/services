/////////////////////////////////
// Conversions to the alloy types
/////////////////////////////////

pub trait IntoAlloy {
    /// The corresponding Alloy type.
    type To;

    /// Converts the legacy type to the corresponding Alloy type.
    fn into_alloy(self) -> Self::To;
}

impl IntoAlloy for primitive_types::U256 {
    type To = alloy::primitives::U256;

    fn into_alloy(self) -> Self::To {
        let mut buf = [0u8; 32];
        self.to_little_endian(&mut buf);
        alloy::primitives::U256::from_le_bytes(buf)
    }
}

impl IntoAlloy for primitive_types::U512 {
    type To = alloy::primitives::U512;

    fn into_alloy(self) -> Self::To {
        let mut buf = [0u8; 64];
        self.to_little_endian(&mut buf);
        alloy::primitives::U512::from_le_bytes(buf)
    }
}

impl IntoAlloy for primitive_types::H160 {
    type To = alloy::primitives::Address;

    fn into_alloy(self) -> Self::To {
        alloy::primitives::Address(self.0.into())
    }
}

impl IntoAlloy for primitive_types::H256 {
    type To = alloy::primitives::aliases::B256;

    fn into_alloy(self) -> Self::To {
        alloy::primitives::aliases::B256::new(self.0)
    }
}

//////////////////////////////////
// Conversions to the legacy types
//////////////////////////////////

pub trait IntoLegacy {
    /// The corresponding legacy type.
    type To;

    /// Converts the alloy type to the corresponding legacy type.
    fn into_legacy(self) -> Self::To;
}

impl IntoLegacy for alloy::primitives::U256 {
    type To = primitive_types::U256;

    fn into_legacy(self) -> Self::To {
        primitive_types::U256(self.into_limbs())
    }
}

impl IntoLegacy for alloy::primitives::U512 {
    type To = primitive_types::U512;

    fn into_legacy(self) -> Self::To {
        primitive_types::U512(self.into_limbs())
    }
}

impl IntoLegacy for alloy::primitives::Address {
    type To = primitive_types::H160;

    fn into_legacy(self) -> Self::To {
        primitive_types::H160(self.into())
    }
}

impl IntoAlloy for alloy::primitives::aliases::B256 {
    type To = primitive_types::H256;

    fn into_alloy(self) -> Self::To {
        primitive_types::H256(self.into())
    }
}
