#![allow(unused_imports, unused_attributes, clippy::all, rustdoc::all, non_snake_case)]
//! Auto-generated contract bindings. Do not edit.
///Module containing a contract's types and functions.
/**

```solidity
library IVault {
    type PoolSpecialization is uint8;
    type SwapKind is uint8;
    type UserBalanceOpKind is uint8;
    struct BatchSwapStep { bytes32 poolId; uint256 assetInIndex; uint256 assetOutIndex; uint256 amount; bytes userData; }
    struct FundManagement { address sender; bool fromInternalBalance; address recipient; bool toInternalBalance; }
    struct SingleSwap { bytes32 poolId; SwapKind kind; address assetIn; address assetOut; uint256 amount; bytes userData; }
    struct UserBalanceOp { UserBalanceOpKind kind; address asset; uint256 amount; address sender; address recipient; }
}
```*/
#[allow(
    non_camel_case_types,
    non_snake_case,
    clippy::pub_underscore_fields,
    clippy::style,
    clippy::empty_structs_with_brackets
)]
pub mod IVault {
    use super::*;
    use alloy_sol_types as alloy_sol_types;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct PoolSpecialization(u8);
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<PoolSpecialization> for u8 {
            #[inline]
            fn stv_to_tokens(
                &self,
            ) -> <alloy_sol_types::sol_data::Uint<
                8,
            > as alloy_sol_types::SolType>::Token<'_> {
                alloy_sol_types::private::SolTypeValue::<
                    alloy_sol_types::sol_data::Uint<8>,
                >::stv_to_tokens(self)
            }
            #[inline]
            fn stv_eip712_data_word(&self) -> alloy_sol_types::Word {
                <alloy_sol_types::sol_data::Uint<
                    8,
                > as alloy_sol_types::SolType>::tokenize(self)
                    .0
            }
            #[inline]
            fn stv_abi_encode_packed_to(
                &self,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                <alloy_sol_types::sol_data::Uint<
                    8,
                > as alloy_sol_types::SolType>::abi_encode_packed_to(self, out)
            }
            #[inline]
            fn stv_abi_packed_encoded_size(&self) -> usize {
                <alloy_sol_types::sol_data::Uint<
                    8,
                > as alloy_sol_types::SolType>::abi_encoded_size(self)
            }
        }
        impl PoolSpecialization {
            /// The Solidity type name.
            pub const NAME: &'static str = stringify!(@ name);
            /// Convert from the underlying value type.
            #[inline]
            pub const fn from_underlying(value: u8) -> Self {
                Self(value)
            }
            /// Return the underlying value.
            #[inline]
            pub const fn into_underlying(self) -> u8 {
                self.0
            }
            /// Return the single encoding of this value, delegating to the
            /// underlying type.
            #[inline]
            pub fn abi_encode(&self) -> alloy_sol_types::private::Vec<u8> {
                <Self as alloy_sol_types::SolType>::abi_encode(&self.0)
            }
            /// Return the packed encoding of this value, delegating to the
            /// underlying type.
            #[inline]
            pub fn abi_encode_packed(&self) -> alloy_sol_types::private::Vec<u8> {
                <Self as alloy_sol_types::SolType>::abi_encode_packed(&self.0)
            }
        }
        #[automatically_derived]
        impl From<u8> for PoolSpecialization {
            fn from(value: u8) -> Self {
                Self::from_underlying(value)
            }
        }
        #[automatically_derived]
        impl From<PoolSpecialization> for u8 {
            fn from(value: PoolSpecialization) -> Self {
                value.into_underlying()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolType for PoolSpecialization {
            type RustType = u8;
            type Token<'a> = <alloy_sol_types::sol_data::Uint<
                8,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SOL_NAME: &'static str = Self::NAME;
            const ENCODED_SIZE: Option<usize> = <alloy_sol_types::sol_data::Uint<
                8,
            > as alloy_sol_types::SolType>::ENCODED_SIZE;
            const PACKED_ENCODED_SIZE: Option<usize> = <alloy_sol_types::sol_data::Uint<
                8,
            > as alloy_sol_types::SolType>::PACKED_ENCODED_SIZE;
            #[inline]
            fn valid_token(token: &Self::Token<'_>) -> bool {
                Self::type_check(token).is_ok()
            }
            #[inline]
            fn type_check(token: &Self::Token<'_>) -> alloy_sol_types::Result<()> {
                <alloy_sol_types::sol_data::Uint<
                    8,
                > as alloy_sol_types::SolType>::type_check(token)
            }
            #[inline]
            fn detokenize(token: Self::Token<'_>) -> Self::RustType {
                <alloy_sol_types::sol_data::Uint<
                    8,
                > as alloy_sol_types::SolType>::detokenize(token)
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for PoolSpecialization {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                <alloy_sol_types::sol_data::Uint<
                    8,
                > as alloy_sol_types::EventTopic>::topic_preimage_length(rust)
            }
            #[inline]
            fn encode_topic_preimage(
                rust: &Self::RustType,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                <alloy_sol_types::sol_data::Uint<
                    8,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(rust, out)
            }
            #[inline]
            fn encode_topic(
                rust: &Self::RustType,
            ) -> alloy_sol_types::abi::token::WordToken {
                <alloy_sol_types::sol_data::Uint<
                    8,
                > as alloy_sol_types::EventTopic>::encode_topic(rust)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct SwapKind(u8);
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<SwapKind> for u8 {
            #[inline]
            fn stv_to_tokens(
                &self,
            ) -> <alloy_sol_types::sol_data::Uint<
                8,
            > as alloy_sol_types::SolType>::Token<'_> {
                alloy_sol_types::private::SolTypeValue::<
                    alloy_sol_types::sol_data::Uint<8>,
                >::stv_to_tokens(self)
            }
            #[inline]
            fn stv_eip712_data_word(&self) -> alloy_sol_types::Word {
                <alloy_sol_types::sol_data::Uint<
                    8,
                > as alloy_sol_types::SolType>::tokenize(self)
                    .0
            }
            #[inline]
            fn stv_abi_encode_packed_to(
                &self,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                <alloy_sol_types::sol_data::Uint<
                    8,
                > as alloy_sol_types::SolType>::abi_encode_packed_to(self, out)
            }
            #[inline]
            fn stv_abi_packed_encoded_size(&self) -> usize {
                <alloy_sol_types::sol_data::Uint<
                    8,
                > as alloy_sol_types::SolType>::abi_encoded_size(self)
            }
        }
        impl SwapKind {
            /// The Solidity type name.
            pub const NAME: &'static str = stringify!(@ name);
            /// Convert from the underlying value type.
            #[inline]
            pub const fn from_underlying(value: u8) -> Self {
                Self(value)
            }
            /// Return the underlying value.
            #[inline]
            pub const fn into_underlying(self) -> u8 {
                self.0
            }
            /// Return the single encoding of this value, delegating to the
            /// underlying type.
            #[inline]
            pub fn abi_encode(&self) -> alloy_sol_types::private::Vec<u8> {
                <Self as alloy_sol_types::SolType>::abi_encode(&self.0)
            }
            /// Return the packed encoding of this value, delegating to the
            /// underlying type.
            #[inline]
            pub fn abi_encode_packed(&self) -> alloy_sol_types::private::Vec<u8> {
                <Self as alloy_sol_types::SolType>::abi_encode_packed(&self.0)
            }
        }
        #[automatically_derived]
        impl From<u8> for SwapKind {
            fn from(value: u8) -> Self {
                Self::from_underlying(value)
            }
        }
        #[automatically_derived]
        impl From<SwapKind> for u8 {
            fn from(value: SwapKind) -> Self {
                value.into_underlying()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolType for SwapKind {
            type RustType = u8;
            type Token<'a> = <alloy_sol_types::sol_data::Uint<
                8,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SOL_NAME: &'static str = Self::NAME;
            const ENCODED_SIZE: Option<usize> = <alloy_sol_types::sol_data::Uint<
                8,
            > as alloy_sol_types::SolType>::ENCODED_SIZE;
            const PACKED_ENCODED_SIZE: Option<usize> = <alloy_sol_types::sol_data::Uint<
                8,
            > as alloy_sol_types::SolType>::PACKED_ENCODED_SIZE;
            #[inline]
            fn valid_token(token: &Self::Token<'_>) -> bool {
                Self::type_check(token).is_ok()
            }
            #[inline]
            fn type_check(token: &Self::Token<'_>) -> alloy_sol_types::Result<()> {
                <alloy_sol_types::sol_data::Uint<
                    8,
                > as alloy_sol_types::SolType>::type_check(token)
            }
            #[inline]
            fn detokenize(token: Self::Token<'_>) -> Self::RustType {
                <alloy_sol_types::sol_data::Uint<
                    8,
                > as alloy_sol_types::SolType>::detokenize(token)
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for SwapKind {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                <alloy_sol_types::sol_data::Uint<
                    8,
                > as alloy_sol_types::EventTopic>::topic_preimage_length(rust)
            }
            #[inline]
            fn encode_topic_preimage(
                rust: &Self::RustType,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                <alloy_sol_types::sol_data::Uint<
                    8,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(rust, out)
            }
            #[inline]
            fn encode_topic(
                rust: &Self::RustType,
            ) -> alloy_sol_types::abi::token::WordToken {
                <alloy_sol_types::sol_data::Uint<
                    8,
                > as alloy_sol_types::EventTopic>::encode_topic(rust)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct UserBalanceOpKind(u8);
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<UserBalanceOpKind> for u8 {
            #[inline]
            fn stv_to_tokens(
                &self,
            ) -> <alloy_sol_types::sol_data::Uint<
                8,
            > as alloy_sol_types::SolType>::Token<'_> {
                alloy_sol_types::private::SolTypeValue::<
                    alloy_sol_types::sol_data::Uint<8>,
                >::stv_to_tokens(self)
            }
            #[inline]
            fn stv_eip712_data_word(&self) -> alloy_sol_types::Word {
                <alloy_sol_types::sol_data::Uint<
                    8,
                > as alloy_sol_types::SolType>::tokenize(self)
                    .0
            }
            #[inline]
            fn stv_abi_encode_packed_to(
                &self,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                <alloy_sol_types::sol_data::Uint<
                    8,
                > as alloy_sol_types::SolType>::abi_encode_packed_to(self, out)
            }
            #[inline]
            fn stv_abi_packed_encoded_size(&self) -> usize {
                <alloy_sol_types::sol_data::Uint<
                    8,
                > as alloy_sol_types::SolType>::abi_encoded_size(self)
            }
        }
        impl UserBalanceOpKind {
            /// The Solidity type name.
            pub const NAME: &'static str = stringify!(@ name);
            /// Convert from the underlying value type.
            #[inline]
            pub const fn from_underlying(value: u8) -> Self {
                Self(value)
            }
            /// Return the underlying value.
            #[inline]
            pub const fn into_underlying(self) -> u8 {
                self.0
            }
            /// Return the single encoding of this value, delegating to the
            /// underlying type.
            #[inline]
            pub fn abi_encode(&self) -> alloy_sol_types::private::Vec<u8> {
                <Self as alloy_sol_types::SolType>::abi_encode(&self.0)
            }
            /// Return the packed encoding of this value, delegating to the
            /// underlying type.
            #[inline]
            pub fn abi_encode_packed(&self) -> alloy_sol_types::private::Vec<u8> {
                <Self as alloy_sol_types::SolType>::abi_encode_packed(&self.0)
            }
        }
        #[automatically_derived]
        impl From<u8> for UserBalanceOpKind {
            fn from(value: u8) -> Self {
                Self::from_underlying(value)
            }
        }
        #[automatically_derived]
        impl From<UserBalanceOpKind> for u8 {
            fn from(value: UserBalanceOpKind) -> Self {
                value.into_underlying()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolType for UserBalanceOpKind {
            type RustType = u8;
            type Token<'a> = <alloy_sol_types::sol_data::Uint<
                8,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SOL_NAME: &'static str = Self::NAME;
            const ENCODED_SIZE: Option<usize> = <alloy_sol_types::sol_data::Uint<
                8,
            > as alloy_sol_types::SolType>::ENCODED_SIZE;
            const PACKED_ENCODED_SIZE: Option<usize> = <alloy_sol_types::sol_data::Uint<
                8,
            > as alloy_sol_types::SolType>::PACKED_ENCODED_SIZE;
            #[inline]
            fn valid_token(token: &Self::Token<'_>) -> bool {
                Self::type_check(token).is_ok()
            }
            #[inline]
            fn type_check(token: &Self::Token<'_>) -> alloy_sol_types::Result<()> {
                <alloy_sol_types::sol_data::Uint<
                    8,
                > as alloy_sol_types::SolType>::type_check(token)
            }
            #[inline]
            fn detokenize(token: Self::Token<'_>) -> Self::RustType {
                <alloy_sol_types::sol_data::Uint<
                    8,
                > as alloy_sol_types::SolType>::detokenize(token)
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for UserBalanceOpKind {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                <alloy_sol_types::sol_data::Uint<
                    8,
                > as alloy_sol_types::EventTopic>::topic_preimage_length(rust)
            }
            #[inline]
            fn encode_topic_preimage(
                rust: &Self::RustType,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                <alloy_sol_types::sol_data::Uint<
                    8,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(rust, out)
            }
            #[inline]
            fn encode_topic(
                rust: &Self::RustType,
            ) -> alloy_sol_types::abi::token::WordToken {
                <alloy_sol_types::sol_data::Uint<
                    8,
                > as alloy_sol_types::EventTopic>::encode_topic(rust)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**```solidity
struct BatchSwapStep { bytes32 poolId; uint256 assetInIndex; uint256 assetOutIndex; uint256 amount; bytes userData; }
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct BatchSwapStep {
        #[allow(missing_docs)]
        pub poolId: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub assetInIndex: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub assetOutIndex: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub amount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub userData: alloy_sol_types::private::Bytes,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        #[doc(hidden)]
        #[allow(dead_code)]
        type UnderlyingSolTuple<'a> = (
            alloy_sol_types::sol_data::FixedBytes<32>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Bytes,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::FixedBytes<32>,
            alloy_sol_types::private::primitives::aliases::U256,
            alloy_sol_types::private::primitives::aliases::U256,
            alloy_sol_types::private::primitives::aliases::U256,
            alloy_sol_types::private::Bytes,
        );
        #[cfg(test)]
        #[allow(dead_code, unreachable_patterns)]
        fn _type_assertion(
            _t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>,
        ) {
            match _t {
                alloy_sol_types::private::AssertTypeEq::<
                    <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                >(_) => {}
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<BatchSwapStep> for UnderlyingRustTuple<'_> {
            fn from(value: BatchSwapStep) -> Self {
                (
                    value.poolId,
                    value.assetInIndex,
                    value.assetOutIndex,
                    value.amount,
                    value.userData,
                )
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for BatchSwapStep {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    poolId: tuple.0,
                    assetInIndex: tuple.1,
                    assetOutIndex: tuple.2,
                    amount: tuple.3,
                    userData: tuple.4,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for BatchSwapStep {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for BatchSwapStep {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self.poolId),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.assetInIndex),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.assetOutIndex),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.amount),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.userData,
                    ),
                )
            }
            #[inline]
            fn stv_abi_encoded_size(&self) -> usize {
                if let Some(size) = <Self as alloy_sol_types::SolType>::ENCODED_SIZE {
                    return size;
                }
                let tuple = <UnderlyingRustTuple<
                    '_,
                > as ::core::convert::From<Self>>::from(self.clone());
                <UnderlyingSolTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_encoded_size(&tuple)
            }
            #[inline]
            fn stv_eip712_data_word(&self) -> alloy_sol_types::Word {
                <Self as alloy_sol_types::SolStruct>::eip712_hash_struct(self)
            }
            #[inline]
            fn stv_abi_encode_packed_to(
                &self,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                let tuple = <UnderlyingRustTuple<
                    '_,
                > as ::core::convert::From<Self>>::from(self.clone());
                <UnderlyingSolTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_encode_packed_to(&tuple, out)
            }
            #[inline]
            fn stv_abi_packed_encoded_size(&self) -> usize {
                if let Some(size) = <Self as alloy_sol_types::SolType>::PACKED_ENCODED_SIZE {
                    return size;
                }
                let tuple = <UnderlyingRustTuple<
                    '_,
                > as ::core::convert::From<Self>>::from(self.clone());
                <UnderlyingSolTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_packed_encoded_size(&tuple)
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolType for BatchSwapStep {
            type RustType = Self;
            type Token<'a> = <UnderlyingSolTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SOL_NAME: &'static str = <Self as alloy_sol_types::SolStruct>::NAME;
            const ENCODED_SIZE: Option<usize> = <UnderlyingSolTuple<
                '_,
            > as alloy_sol_types::SolType>::ENCODED_SIZE;
            const PACKED_ENCODED_SIZE: Option<usize> = <UnderlyingSolTuple<
                '_,
            > as alloy_sol_types::SolType>::PACKED_ENCODED_SIZE;
            #[inline]
            fn valid_token(token: &Self::Token<'_>) -> bool {
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::valid_token(token)
            }
            #[inline]
            fn detokenize(token: Self::Token<'_>) -> Self::RustType {
                let tuple = <UnderlyingSolTuple<
                    '_,
                > as alloy_sol_types::SolType>::detokenize(token);
                <Self as ::core::convert::From<UnderlyingRustTuple<'_>>>::from(tuple)
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolStruct for BatchSwapStep {
            const NAME: &'static str = "BatchSwapStep";
            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "BatchSwapStep(bytes32 poolId,uint256 assetInIndex,uint256 assetOutIndex,uint256 amount,bytes userData)",
                )
            }
            #[inline]
            fn eip712_components() -> alloy_sol_types::private::Vec<
                alloy_sol_types::private::Cow<'static, str>,
            > {
                alloy_sol_types::private::Vec::new()
            }
            #[inline]
            fn eip712_encode_type() -> alloy_sol_types::private::Cow<'static, str> {
                <Self as alloy_sol_types::SolStruct>::eip712_root_type()
            }
            #[inline]
            fn eip712_encode_data(&self) -> alloy_sol_types::private::Vec<u8> {
                [
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.poolId)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.assetInIndex)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.assetOutIndex)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.amount)
                        .0,
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::eip712_data_word(
                            &self.userData,
                        )
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for BatchSwapStep {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.poolId,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.assetInIndex,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.assetOutIndex,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.amount,
                    )
                    + <alloy_sol_types::sol_data::Bytes as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.userData,
                    )
            }
            #[inline]
            fn encode_topic_preimage(
                rust: &Self::RustType,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                out.reserve(
                    <Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust),
                );
                <alloy_sol_types::sol_data::FixedBytes<
                    32,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.poolId,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.assetInIndex,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.assetOutIndex,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.amount,
                    out,
                );
                <alloy_sol_types::sol_data::Bytes as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.userData,
                    out,
                );
            }
            #[inline]
            fn encode_topic(
                rust: &Self::RustType,
            ) -> alloy_sol_types::abi::token::WordToken {
                let mut out = alloy_sol_types::private::Vec::new();
                <Self as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    rust,
                    &mut out,
                );
                alloy_sol_types::abi::token::WordToken(
                    alloy_sol_types::private::keccak256(out),
                )
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**```solidity
struct FundManagement { address sender; bool fromInternalBalance; address recipient; bool toInternalBalance; }
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct FundManagement {
        #[allow(missing_docs)]
        pub sender: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub fromInternalBalance: bool,
        #[allow(missing_docs)]
        pub recipient: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub toInternalBalance: bool,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        #[doc(hidden)]
        #[allow(dead_code)]
        type UnderlyingSolTuple<'a> = (
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Bool,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Bool,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::Address,
            bool,
            alloy_sol_types::private::Address,
            bool,
        );
        #[cfg(test)]
        #[allow(dead_code, unreachable_patterns)]
        fn _type_assertion(
            _t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>,
        ) {
            match _t {
                alloy_sol_types::private::AssertTypeEq::<
                    <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                >(_) => {}
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<FundManagement> for UnderlyingRustTuple<'_> {
            fn from(value: FundManagement) -> Self {
                (
                    value.sender,
                    value.fromInternalBalance,
                    value.recipient,
                    value.toInternalBalance,
                )
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for FundManagement {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    sender: tuple.0,
                    fromInternalBalance: tuple.1,
                    recipient: tuple.2,
                    toInternalBalance: tuple.3,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for FundManagement {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for FundManagement {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.sender,
                    ),
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::tokenize(
                        &self.fromInternalBalance,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.recipient,
                    ),
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::tokenize(
                        &self.toInternalBalance,
                    ),
                )
            }
            #[inline]
            fn stv_abi_encoded_size(&self) -> usize {
                if let Some(size) = <Self as alloy_sol_types::SolType>::ENCODED_SIZE {
                    return size;
                }
                let tuple = <UnderlyingRustTuple<
                    '_,
                > as ::core::convert::From<Self>>::from(self.clone());
                <UnderlyingSolTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_encoded_size(&tuple)
            }
            #[inline]
            fn stv_eip712_data_word(&self) -> alloy_sol_types::Word {
                <Self as alloy_sol_types::SolStruct>::eip712_hash_struct(self)
            }
            #[inline]
            fn stv_abi_encode_packed_to(
                &self,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                let tuple = <UnderlyingRustTuple<
                    '_,
                > as ::core::convert::From<Self>>::from(self.clone());
                <UnderlyingSolTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_encode_packed_to(&tuple, out)
            }
            #[inline]
            fn stv_abi_packed_encoded_size(&self) -> usize {
                if let Some(size) = <Self as alloy_sol_types::SolType>::PACKED_ENCODED_SIZE {
                    return size;
                }
                let tuple = <UnderlyingRustTuple<
                    '_,
                > as ::core::convert::From<Self>>::from(self.clone());
                <UnderlyingSolTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_packed_encoded_size(&tuple)
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolType for FundManagement {
            type RustType = Self;
            type Token<'a> = <UnderlyingSolTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SOL_NAME: &'static str = <Self as alloy_sol_types::SolStruct>::NAME;
            const ENCODED_SIZE: Option<usize> = <UnderlyingSolTuple<
                '_,
            > as alloy_sol_types::SolType>::ENCODED_SIZE;
            const PACKED_ENCODED_SIZE: Option<usize> = <UnderlyingSolTuple<
                '_,
            > as alloy_sol_types::SolType>::PACKED_ENCODED_SIZE;
            #[inline]
            fn valid_token(token: &Self::Token<'_>) -> bool {
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::valid_token(token)
            }
            #[inline]
            fn detokenize(token: Self::Token<'_>) -> Self::RustType {
                let tuple = <UnderlyingSolTuple<
                    '_,
                > as alloy_sol_types::SolType>::detokenize(token);
                <Self as ::core::convert::From<UnderlyingRustTuple<'_>>>::from(tuple)
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolStruct for FundManagement {
            const NAME: &'static str = "FundManagement";
            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "FundManagement(address sender,bool fromInternalBalance,address recipient,bool toInternalBalance)",
                )
            }
            #[inline]
            fn eip712_components() -> alloy_sol_types::private::Vec<
                alloy_sol_types::private::Cow<'static, str>,
            > {
                alloy_sol_types::private::Vec::new()
            }
            #[inline]
            fn eip712_encode_type() -> alloy_sol_types::private::Cow<'static, str> {
                <Self as alloy_sol_types::SolStruct>::eip712_root_type()
            }
            #[inline]
            fn eip712_encode_data(&self) -> alloy_sol_types::private::Vec<u8> {
                [
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.sender,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::eip712_data_word(
                            &self.fromInternalBalance,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.recipient,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::eip712_data_word(
                            &self.toInternalBalance,
                        )
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for FundManagement {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.sender,
                    )
                    + <alloy_sol_types::sol_data::Bool as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.fromInternalBalance,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.recipient,
                    )
                    + <alloy_sol_types::sol_data::Bool as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.toInternalBalance,
                    )
            }
            #[inline]
            fn encode_topic_preimage(
                rust: &Self::RustType,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                out.reserve(
                    <Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust),
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.sender,
                    out,
                );
                <alloy_sol_types::sol_data::Bool as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.fromInternalBalance,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.recipient,
                    out,
                );
                <alloy_sol_types::sol_data::Bool as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.toInternalBalance,
                    out,
                );
            }
            #[inline]
            fn encode_topic(
                rust: &Self::RustType,
            ) -> alloy_sol_types::abi::token::WordToken {
                let mut out = alloy_sol_types::private::Vec::new();
                <Self as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    rust,
                    &mut out,
                );
                alloy_sol_types::abi::token::WordToken(
                    alloy_sol_types::private::keccak256(out),
                )
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**```solidity
struct SingleSwap { bytes32 poolId; SwapKind kind; address assetIn; address assetOut; uint256 amount; bytes userData; }
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct SingleSwap {
        #[allow(missing_docs)]
        pub poolId: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub kind: <SwapKind as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub assetIn: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub assetOut: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub amount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub userData: alloy_sol_types::private::Bytes,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        #[doc(hidden)]
        #[allow(dead_code)]
        type UnderlyingSolTuple<'a> = (
            alloy_sol_types::sol_data::FixedBytes<32>,
            SwapKind,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Bytes,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::FixedBytes<32>,
            <SwapKind as alloy_sol_types::SolType>::RustType,
            alloy_sol_types::private::Address,
            alloy_sol_types::private::Address,
            alloy_sol_types::private::primitives::aliases::U256,
            alloy_sol_types::private::Bytes,
        );
        #[cfg(test)]
        #[allow(dead_code, unreachable_patterns)]
        fn _type_assertion(
            _t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>,
        ) {
            match _t {
                alloy_sol_types::private::AssertTypeEq::<
                    <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                >(_) => {}
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<SingleSwap> for UnderlyingRustTuple<'_> {
            fn from(value: SingleSwap) -> Self {
                (
                    value.poolId,
                    value.kind,
                    value.assetIn,
                    value.assetOut,
                    value.amount,
                    value.userData,
                )
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for SingleSwap {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    poolId: tuple.0,
                    kind: tuple.1,
                    assetIn: tuple.2,
                    assetOut: tuple.3,
                    amount: tuple.4,
                    userData: tuple.5,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for SingleSwap {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for SingleSwap {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self.poolId),
                    <SwapKind as alloy_sol_types::SolType>::tokenize(&self.kind),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.assetIn,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.assetOut,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.amount),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.userData,
                    ),
                )
            }
            #[inline]
            fn stv_abi_encoded_size(&self) -> usize {
                if let Some(size) = <Self as alloy_sol_types::SolType>::ENCODED_SIZE {
                    return size;
                }
                let tuple = <UnderlyingRustTuple<
                    '_,
                > as ::core::convert::From<Self>>::from(self.clone());
                <UnderlyingSolTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_encoded_size(&tuple)
            }
            #[inline]
            fn stv_eip712_data_word(&self) -> alloy_sol_types::Word {
                <Self as alloy_sol_types::SolStruct>::eip712_hash_struct(self)
            }
            #[inline]
            fn stv_abi_encode_packed_to(
                &self,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                let tuple = <UnderlyingRustTuple<
                    '_,
                > as ::core::convert::From<Self>>::from(self.clone());
                <UnderlyingSolTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_encode_packed_to(&tuple, out)
            }
            #[inline]
            fn stv_abi_packed_encoded_size(&self) -> usize {
                if let Some(size) = <Self as alloy_sol_types::SolType>::PACKED_ENCODED_SIZE {
                    return size;
                }
                let tuple = <UnderlyingRustTuple<
                    '_,
                > as ::core::convert::From<Self>>::from(self.clone());
                <UnderlyingSolTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_packed_encoded_size(&tuple)
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolType for SingleSwap {
            type RustType = Self;
            type Token<'a> = <UnderlyingSolTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SOL_NAME: &'static str = <Self as alloy_sol_types::SolStruct>::NAME;
            const ENCODED_SIZE: Option<usize> = <UnderlyingSolTuple<
                '_,
            > as alloy_sol_types::SolType>::ENCODED_SIZE;
            const PACKED_ENCODED_SIZE: Option<usize> = <UnderlyingSolTuple<
                '_,
            > as alloy_sol_types::SolType>::PACKED_ENCODED_SIZE;
            #[inline]
            fn valid_token(token: &Self::Token<'_>) -> bool {
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::valid_token(token)
            }
            #[inline]
            fn detokenize(token: Self::Token<'_>) -> Self::RustType {
                let tuple = <UnderlyingSolTuple<
                    '_,
                > as alloy_sol_types::SolType>::detokenize(token);
                <Self as ::core::convert::From<UnderlyingRustTuple<'_>>>::from(tuple)
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolStruct for SingleSwap {
            const NAME: &'static str = "SingleSwap";
            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "SingleSwap(bytes32 poolId,uint8 kind,address assetIn,address assetOut,uint256 amount,bytes userData)",
                )
            }
            #[inline]
            fn eip712_components() -> alloy_sol_types::private::Vec<
                alloy_sol_types::private::Cow<'static, str>,
            > {
                alloy_sol_types::private::Vec::new()
            }
            #[inline]
            fn eip712_encode_type() -> alloy_sol_types::private::Cow<'static, str> {
                <Self as alloy_sol_types::SolStruct>::eip712_root_type()
            }
            #[inline]
            fn eip712_encode_data(&self) -> alloy_sol_types::private::Vec<u8> {
                [
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.poolId)
                        .0,
                    <SwapKind as alloy_sol_types::SolType>::eip712_data_word(&self.kind)
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.assetIn,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.assetOut,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.amount)
                        .0,
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::eip712_data_word(
                            &self.userData,
                        )
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for SingleSwap {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.poolId,
                    )
                    + <SwapKind as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.kind,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.assetIn,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.assetOut,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.amount,
                    )
                    + <alloy_sol_types::sol_data::Bytes as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.userData,
                    )
            }
            #[inline]
            fn encode_topic_preimage(
                rust: &Self::RustType,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                out.reserve(
                    <Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust),
                );
                <alloy_sol_types::sol_data::FixedBytes<
                    32,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.poolId,
                    out,
                );
                <SwapKind as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.kind,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.assetIn,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.assetOut,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.amount,
                    out,
                );
                <alloy_sol_types::sol_data::Bytes as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.userData,
                    out,
                );
            }
            #[inline]
            fn encode_topic(
                rust: &Self::RustType,
            ) -> alloy_sol_types::abi::token::WordToken {
                let mut out = alloy_sol_types::private::Vec::new();
                <Self as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    rust,
                    &mut out,
                );
                alloy_sol_types::abi::token::WordToken(
                    alloy_sol_types::private::keccak256(out),
                )
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**```solidity
struct UserBalanceOp { UserBalanceOpKind kind; address asset; uint256 amount; address sender; address recipient; }
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct UserBalanceOp {
        #[allow(missing_docs)]
        pub kind: <UserBalanceOpKind as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub asset: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub amount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub sender: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub recipient: alloy_sol_types::private::Address,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        #[doc(hidden)]
        #[allow(dead_code)]
        type UnderlyingSolTuple<'a> = (
            UserBalanceOpKind,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Address,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            <UserBalanceOpKind as alloy_sol_types::SolType>::RustType,
            alloy_sol_types::private::Address,
            alloy_sol_types::private::primitives::aliases::U256,
            alloy_sol_types::private::Address,
            alloy_sol_types::private::Address,
        );
        #[cfg(test)]
        #[allow(dead_code, unreachable_patterns)]
        fn _type_assertion(
            _t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>,
        ) {
            match _t {
                alloy_sol_types::private::AssertTypeEq::<
                    <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                >(_) => {}
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UserBalanceOp> for UnderlyingRustTuple<'_> {
            fn from(value: UserBalanceOp) -> Self {
                (value.kind, value.asset, value.amount, value.sender, value.recipient)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for UserBalanceOp {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    kind: tuple.0,
                    asset: tuple.1,
                    amount: tuple.2,
                    sender: tuple.3,
                    recipient: tuple.4,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for UserBalanceOp {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for UserBalanceOp {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <UserBalanceOpKind as alloy_sol_types::SolType>::tokenize(
                        &self.kind,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.asset,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.amount),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.sender,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.recipient,
                    ),
                )
            }
            #[inline]
            fn stv_abi_encoded_size(&self) -> usize {
                if let Some(size) = <Self as alloy_sol_types::SolType>::ENCODED_SIZE {
                    return size;
                }
                let tuple = <UnderlyingRustTuple<
                    '_,
                > as ::core::convert::From<Self>>::from(self.clone());
                <UnderlyingSolTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_encoded_size(&tuple)
            }
            #[inline]
            fn stv_eip712_data_word(&self) -> alloy_sol_types::Word {
                <Self as alloy_sol_types::SolStruct>::eip712_hash_struct(self)
            }
            #[inline]
            fn stv_abi_encode_packed_to(
                &self,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                let tuple = <UnderlyingRustTuple<
                    '_,
                > as ::core::convert::From<Self>>::from(self.clone());
                <UnderlyingSolTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_encode_packed_to(&tuple, out)
            }
            #[inline]
            fn stv_abi_packed_encoded_size(&self) -> usize {
                if let Some(size) = <Self as alloy_sol_types::SolType>::PACKED_ENCODED_SIZE {
                    return size;
                }
                let tuple = <UnderlyingRustTuple<
                    '_,
                > as ::core::convert::From<Self>>::from(self.clone());
                <UnderlyingSolTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_packed_encoded_size(&tuple)
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolType for UserBalanceOp {
            type RustType = Self;
            type Token<'a> = <UnderlyingSolTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SOL_NAME: &'static str = <Self as alloy_sol_types::SolStruct>::NAME;
            const ENCODED_SIZE: Option<usize> = <UnderlyingSolTuple<
                '_,
            > as alloy_sol_types::SolType>::ENCODED_SIZE;
            const PACKED_ENCODED_SIZE: Option<usize> = <UnderlyingSolTuple<
                '_,
            > as alloy_sol_types::SolType>::PACKED_ENCODED_SIZE;
            #[inline]
            fn valid_token(token: &Self::Token<'_>) -> bool {
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::valid_token(token)
            }
            #[inline]
            fn detokenize(token: Self::Token<'_>) -> Self::RustType {
                let tuple = <UnderlyingSolTuple<
                    '_,
                > as alloy_sol_types::SolType>::detokenize(token);
                <Self as ::core::convert::From<UnderlyingRustTuple<'_>>>::from(tuple)
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolStruct for UserBalanceOp {
            const NAME: &'static str = "UserBalanceOp";
            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "UserBalanceOp(uint8 kind,address asset,uint256 amount,address sender,address recipient)",
                )
            }
            #[inline]
            fn eip712_components() -> alloy_sol_types::private::Vec<
                alloy_sol_types::private::Cow<'static, str>,
            > {
                alloy_sol_types::private::Vec::new()
            }
            #[inline]
            fn eip712_encode_type() -> alloy_sol_types::private::Cow<'static, str> {
                <Self as alloy_sol_types::SolStruct>::eip712_root_type()
            }
            #[inline]
            fn eip712_encode_data(&self) -> alloy_sol_types::private::Vec<u8> {
                [
                    <UserBalanceOpKind as alloy_sol_types::SolType>::eip712_data_word(
                            &self.kind,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.asset,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.amount)
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.sender,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.recipient,
                        )
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for UserBalanceOp {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <UserBalanceOpKind as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.kind,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.asset,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.amount,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.sender,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.recipient,
                    )
            }
            #[inline]
            fn encode_topic_preimage(
                rust: &Self::RustType,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                out.reserve(
                    <Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust),
                );
                <UserBalanceOpKind as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.kind,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.asset,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.amount,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.sender,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.recipient,
                    out,
                );
            }
            #[inline]
            fn encode_topic(
                rust: &Self::RustType,
            ) -> alloy_sol_types::abi::token::WordToken {
                let mut out = alloy_sol_types::private::Vec::new();
                <Self as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    rust,
                    &mut out,
                );
                alloy_sol_types::abi::token::WordToken(
                    alloy_sol_types::private::keccak256(out),
                )
            }
        }
    };
    use alloy_contract as alloy_contract;
    /**Creates a new wrapper around an on-chain [`IVault`](self) contract instance.

See the [wrapper's documentation](`IVaultInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> IVaultInstance<P, N> {
        IVaultInstance::<P, N>::new(address, __provider)
    }
    /**A [`IVault`](self) instance.

Contains type-safe methods for interacting with an on-chain instance of the
[`IVault`](self) contract located at a given `address`, using a given
provider `P`.

If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
documentation on how to provide it), the `deploy` and `deploy_builder` methods can
be used to deploy a new instance of the contract.

See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct IVaultInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for IVaultInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("IVaultInstance").field(&self.address).finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    > IVaultInstance<P, N> {
        /**Creates a new wrapper around an on-chain [`IVault`](self) contract instance.

See the [wrapper's documentation](`IVaultInstance`) for more details.*/
        #[inline]
        pub const fn new(
            address: alloy_sol_types::private::Address,
            __provider: P,
        ) -> Self {
            Self {
                address,
                provider: __provider,
                _network: ::core::marker::PhantomData,
            }
        }
        /// Returns a reference to the address.
        #[inline]
        pub const fn address(&self) -> &alloy_sol_types::private::Address {
            &self.address
        }
        /// Sets the address.
        #[inline]
        pub fn set_address(&mut self, address: alloy_sol_types::private::Address) {
            self.address = address;
        }
        /// Sets the address and returns `self`.
        pub fn at(mut self, address: alloy_sol_types::private::Address) -> Self {
            self.set_address(address);
            self
        }
        /// Returns a reference to the provider.
        #[inline]
        pub const fn provider(&self) -> &P {
            &self.provider
        }
    }
    impl<P: ::core::clone::Clone, N> IVaultInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned provider.
        #[inline]
        pub fn with_cloned_provider(self) -> IVaultInstance<P, N> {
            IVaultInstance {
                address: self.address,
                provider: ::core::clone::Clone::clone(&self.provider),
                _network: ::core::marker::PhantomData,
            }
        }
    }
    /// Function calls.
    impl<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    > IVaultInstance<P, N> {
        /// Creates a new call builder using this contract instance's provider and address.
        ///
        /// Note that the call can be any function call, not just those defined in this
        /// contract. Prefer using the other methods for building type-safe contract calls.
        pub fn call_builder<C: alloy_sol_types::SolCall>(
            &self,
            call: &C,
        ) -> alloy_contract::SolCallBuilder<&P, C, N> {
            alloy_contract::SolCallBuilder::new_sol(&self.provider, &self.address, call)
        }
    }
    /// Event filters.
    impl<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    > IVaultInstance<P, N> {
        /// Creates a new event filter using this contract instance's provider and address.
        ///
        /// Note that the type can be any event, not just those defined in this contract.
        /// Prefer using the other methods for building type-safe event filters.
        pub fn event_filter<E: alloy_sol_types::SolEvent>(
            &self,
        ) -> alloy_contract::Event<&P, E, N> {
            alloy_contract::Event::new_sol(&self.provider, &self.address)
        }
    }
}
/**

Generated by the following Solidity interface...
```solidity
library IVault {
    type PoolSpecialization is uint8;
    type SwapKind is uint8;
    type UserBalanceOpKind is uint8;
    struct BatchSwapStep {
        bytes32 poolId;
        uint256 assetInIndex;
        uint256 assetOutIndex;
        uint256 amount;
        bytes userData;
    }
    struct FundManagement {
        address sender;
        bool fromInternalBalance;
        address payable recipient;
        bool toInternalBalance;
    }
    struct SingleSwap {
        bytes32 poolId;
        SwapKind kind;
        address assetIn;
        address assetOut;
        uint256 amount;
        bytes userData;
    }
    struct UserBalanceOp {
        UserBalanceOpKind kind;
        address asset;
        uint256 amount;
        address sender;
        address payable recipient;
    }
}

interface BalancerV2Vault {
    event AuthorizerChanged(address indexed newAuthorizer);
    event ExternalBalanceTransfer(address indexed token, address indexed sender, address recipient, uint256 amount);
    event FlashLoan(address indexed recipient, address indexed token, uint256 amount, uint256 feeAmount);
    event InternalBalanceChanged(address indexed user, address indexed token, int256 delta);
    event PausedStateChanged(bool paused);
    event PoolBalanceChanged(bytes32 indexed poolId, address indexed liquidityProvider, address[] tokens, int256[] deltas, uint256[] protocolFeeAmounts);
    event PoolBalanceManaged(bytes32 indexed poolId, address indexed assetManager, address indexed token, int256 cashDelta, int256 managedDelta);
    event PoolRegistered(bytes32 indexed poolId, address indexed poolAddress, IVault.PoolSpecialization specialization);
    event RelayerApprovalChanged(address indexed relayer, address indexed sender, bool approved);
    event Swap(bytes32 indexed poolId, address indexed tokenIn, address indexed tokenOut, uint256 amountIn, uint256 amountOut);
    event TokensDeregistered(bytes32 indexed poolId, address[] tokens);
    event TokensRegistered(bytes32 indexed poolId, address[] tokens, address[] assetManagers);

    constructor(address authorizer, address weth, uint256 pauseWindowDuration, uint256 bufferPeriodDuration);

    receive() external payable;

    function WETH() external view returns (address);
    function batchSwap(IVault.SwapKind kind, IVault.BatchSwapStep[] memory swaps, address[] memory assets, IVault.FundManagement memory funds, int256[] memory limits, uint256 deadline) external payable returns (int256[] memory assetDeltas);
    function flashLoan(address recipient, address[] memory tokens, uint256[] memory amounts, bytes memory userData) external;
    function getInternalBalance(address user, address[] memory tokens) external view returns (uint256[] memory balances);
    function getPausedState() external view returns (bool paused, uint256 pauseWindowEndTime, uint256 bufferPeriodEndTime);
    function getPool(bytes32 poolId) external view returns (address, IVault.PoolSpecialization);
    function getPoolTokens(bytes32 poolId) external view returns (address[] memory tokens, uint256[] memory balances, uint256 lastChangeBlock);
    function hasApprovedRelayer(address user, address relayer) external view returns (bool);
    function manageUserBalance(IVault.UserBalanceOp[] memory ops) external payable;
    function setRelayerApproval(address sender, address relayer, bool approved) external;
    function swap(IVault.SingleSwap memory singleSwap, IVault.FundManagement memory funds, uint256 limit, uint256 deadline) external payable returns (uint256 amountCalculated);
}
```

...which was generated by the following JSON ABI:
```json
[
  {
    "type": "constructor",
    "inputs": [
      {
        "name": "authorizer",
        "type": "address",
        "internalType": "contract IAuthorizer"
      },
      {
        "name": "weth",
        "type": "address",
        "internalType": "contract IWETH"
      },
      {
        "name": "pauseWindowDuration",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "bufferPeriodDuration",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "stateMutability": "nonpayable"
  },
  {
    "type": "receive",
    "stateMutability": "payable"
  },
  {
    "type": "function",
    "name": "WETH",
    "inputs": [],
    "outputs": [
      {
        "name": "",
        "type": "address",
        "internalType": "contract IWETH"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "batchSwap",
    "inputs": [
      {
        "name": "kind",
        "type": "uint8",
        "internalType": "enum IVault.SwapKind"
      },
      {
        "name": "swaps",
        "type": "tuple[]",
        "internalType": "struct IVault.BatchSwapStep[]",
        "components": [
          {
            "name": "poolId",
            "type": "bytes32",
            "internalType": "bytes32"
          },
          {
            "name": "assetInIndex",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "assetOutIndex",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "amount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "userData",
            "type": "bytes",
            "internalType": "bytes"
          }
        ]
      },
      {
        "name": "assets",
        "type": "address[]",
        "internalType": "contract IAsset[]"
      },
      {
        "name": "funds",
        "type": "tuple",
        "internalType": "struct IVault.FundManagement",
        "components": [
          {
            "name": "sender",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "fromInternalBalance",
            "type": "bool",
            "internalType": "bool"
          },
          {
            "name": "recipient",
            "type": "address",
            "internalType": "address payable"
          },
          {
            "name": "toInternalBalance",
            "type": "bool",
            "internalType": "bool"
          }
        ]
      },
      {
        "name": "limits",
        "type": "int256[]",
        "internalType": "int256[]"
      },
      {
        "name": "deadline",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "outputs": [
      {
        "name": "assetDeltas",
        "type": "int256[]",
        "internalType": "int256[]"
      }
    ],
    "stateMutability": "payable"
  },
  {
    "type": "function",
    "name": "flashLoan",
    "inputs": [
      {
        "name": "recipient",
        "type": "address",
        "internalType": "contract IFlashLoanRecipient"
      },
      {
        "name": "tokens",
        "type": "address[]",
        "internalType": "contract IERC20[]"
      },
      {
        "name": "amounts",
        "type": "uint256[]",
        "internalType": "uint256[]"
      },
      {
        "name": "userData",
        "type": "bytes",
        "internalType": "bytes"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "getInternalBalance",
    "inputs": [
      {
        "name": "user",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "tokens",
        "type": "address[]",
        "internalType": "contract IERC20[]"
      }
    ],
    "outputs": [
      {
        "name": "balances",
        "type": "uint256[]",
        "internalType": "uint256[]"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "getPausedState",
    "inputs": [],
    "outputs": [
      {
        "name": "paused",
        "type": "bool",
        "internalType": "bool"
      },
      {
        "name": "pauseWindowEndTime",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "bufferPeriodEndTime",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "getPool",
    "inputs": [
      {
        "name": "poolId",
        "type": "bytes32",
        "internalType": "bytes32"
      }
    ],
    "outputs": [
      {
        "name": "",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "",
        "type": "uint8",
        "internalType": "enum IVault.PoolSpecialization"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "getPoolTokens",
    "inputs": [
      {
        "name": "poolId",
        "type": "bytes32",
        "internalType": "bytes32"
      }
    ],
    "outputs": [
      {
        "name": "tokens",
        "type": "address[]",
        "internalType": "contract IERC20[]"
      },
      {
        "name": "balances",
        "type": "uint256[]",
        "internalType": "uint256[]"
      },
      {
        "name": "lastChangeBlock",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "hasApprovedRelayer",
    "inputs": [
      {
        "name": "user",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "relayer",
        "type": "address",
        "internalType": "address"
      }
    ],
    "outputs": [
      {
        "name": "",
        "type": "bool",
        "internalType": "bool"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "manageUserBalance",
    "inputs": [
      {
        "name": "ops",
        "type": "tuple[]",
        "internalType": "struct IVault.UserBalanceOp[]",
        "components": [
          {
            "name": "kind",
            "type": "uint8",
            "internalType": "enum IVault.UserBalanceOpKind"
          },
          {
            "name": "asset",
            "type": "address",
            "internalType": "contract IAsset"
          },
          {
            "name": "amount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "sender",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "recipient",
            "type": "address",
            "internalType": "address payable"
          }
        ]
      }
    ],
    "outputs": [],
    "stateMutability": "payable"
  },
  {
    "type": "function",
    "name": "setRelayerApproval",
    "inputs": [
      {
        "name": "sender",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "relayer",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "approved",
        "type": "bool",
        "internalType": "bool"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "swap",
    "inputs": [
      {
        "name": "singleSwap",
        "type": "tuple",
        "internalType": "struct IVault.SingleSwap",
        "components": [
          {
            "name": "poolId",
            "type": "bytes32",
            "internalType": "bytes32"
          },
          {
            "name": "kind",
            "type": "uint8",
            "internalType": "enum IVault.SwapKind"
          },
          {
            "name": "assetIn",
            "type": "address",
            "internalType": "contract IAsset"
          },
          {
            "name": "assetOut",
            "type": "address",
            "internalType": "contract IAsset"
          },
          {
            "name": "amount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "userData",
            "type": "bytes",
            "internalType": "bytes"
          }
        ]
      },
      {
        "name": "funds",
        "type": "tuple",
        "internalType": "struct IVault.FundManagement",
        "components": [
          {
            "name": "sender",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "fromInternalBalance",
            "type": "bool",
            "internalType": "bool"
          },
          {
            "name": "recipient",
            "type": "address",
            "internalType": "address payable"
          },
          {
            "name": "toInternalBalance",
            "type": "bool",
            "internalType": "bool"
          }
        ]
      },
      {
        "name": "limit",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "deadline",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "outputs": [
      {
        "name": "amountCalculated",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "stateMutability": "payable"
  },
  {
    "type": "event",
    "name": "AuthorizerChanged",
    "inputs": [
      {
        "name": "newAuthorizer",
        "type": "address",
        "indexed": true,
        "internalType": "contract IAuthorizer"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "ExternalBalanceTransfer",
    "inputs": [
      {
        "name": "token",
        "type": "address",
        "indexed": true,
        "internalType": "contract IERC20"
      },
      {
        "name": "sender",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "recipient",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "amount",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "FlashLoan",
    "inputs": [
      {
        "name": "recipient",
        "type": "address",
        "indexed": true,
        "internalType": "contract IFlashLoanRecipient"
      },
      {
        "name": "token",
        "type": "address",
        "indexed": true,
        "internalType": "contract IERC20"
      },
      {
        "name": "amount",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "feeAmount",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "InternalBalanceChanged",
    "inputs": [
      {
        "name": "user",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "token",
        "type": "address",
        "indexed": true,
        "internalType": "contract IERC20"
      },
      {
        "name": "delta",
        "type": "int256",
        "indexed": false,
        "internalType": "int256"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "PausedStateChanged",
    "inputs": [
      {
        "name": "paused",
        "type": "bool",
        "indexed": false,
        "internalType": "bool"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "PoolBalanceChanged",
    "inputs": [
      {
        "name": "poolId",
        "type": "bytes32",
        "indexed": true,
        "internalType": "bytes32"
      },
      {
        "name": "liquidityProvider",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "tokens",
        "type": "address[]",
        "indexed": false,
        "internalType": "contract IERC20[]"
      },
      {
        "name": "deltas",
        "type": "int256[]",
        "indexed": false,
        "internalType": "int256[]"
      },
      {
        "name": "protocolFeeAmounts",
        "type": "uint256[]",
        "indexed": false,
        "internalType": "uint256[]"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "PoolBalanceManaged",
    "inputs": [
      {
        "name": "poolId",
        "type": "bytes32",
        "indexed": true,
        "internalType": "bytes32"
      },
      {
        "name": "assetManager",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "token",
        "type": "address",
        "indexed": true,
        "internalType": "contract IERC20"
      },
      {
        "name": "cashDelta",
        "type": "int256",
        "indexed": false,
        "internalType": "int256"
      },
      {
        "name": "managedDelta",
        "type": "int256",
        "indexed": false,
        "internalType": "int256"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "PoolRegistered",
    "inputs": [
      {
        "name": "poolId",
        "type": "bytes32",
        "indexed": true,
        "internalType": "bytes32"
      },
      {
        "name": "poolAddress",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "specialization",
        "type": "uint8",
        "indexed": false,
        "internalType": "enum IVault.PoolSpecialization"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "RelayerApprovalChanged",
    "inputs": [
      {
        "name": "relayer",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "sender",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "approved",
        "type": "bool",
        "indexed": false,
        "internalType": "bool"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "Swap",
    "inputs": [
      {
        "name": "poolId",
        "type": "bytes32",
        "indexed": true,
        "internalType": "bytes32"
      },
      {
        "name": "tokenIn",
        "type": "address",
        "indexed": true,
        "internalType": "contract IERC20"
      },
      {
        "name": "tokenOut",
        "type": "address",
        "indexed": true,
        "internalType": "contract IERC20"
      },
      {
        "name": "amountIn",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "amountOut",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "TokensDeregistered",
    "inputs": [
      {
        "name": "poolId",
        "type": "bytes32",
        "indexed": true,
        "internalType": "bytes32"
      },
      {
        "name": "tokens",
        "type": "address[]",
        "indexed": false,
        "internalType": "contract IERC20[]"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "TokensRegistered",
    "inputs": [
      {
        "name": "poolId",
        "type": "bytes32",
        "indexed": true,
        "internalType": "bytes32"
      },
      {
        "name": "tokens",
        "type": "address[]",
        "indexed": false,
        "internalType": "contract IERC20[]"
      },
      {
        "name": "assetManagers",
        "type": "address[]",
        "indexed": false,
        "internalType": "address[]"
      }
    ],
    "anonymous": false
  }
]
```*/
#[allow(
    non_camel_case_types,
    non_snake_case,
    clippy::pub_underscore_fields,
    clippy::style,
    clippy::empty_structs_with_brackets
)]
pub mod BalancerV2Vault {
    use super::*;
    use alloy_sol_types as alloy_sol_types;
    /// The creation / init bytecode of the contract.
    ///
    /// ```text
    ///0x6101806040523480156200001257600080fd5b5060405162006ed638038062006ed6833981016040819052620000359162000253565b8382826040518060400160405280601181526020017010985b185b98d95c88158c8815985d5b1d607a1b81525080604051806040016040528060018152602001603160f81b815250306001600160a01b031660001b89806001600160a01b03166080816001600160a01b031660601b815250505030604051620000b89062000245565b620000c491906200029f565b604051809103906000f080158015620000e1573d6000803e3d6000fd5b5060601b6001600160601b03191660a052600160005560c052815160209283012060e052805191012061010052507f8b73c3c69bb8fe3d512ecc4cf759cc79239f7b179b0ffacaa9a75d522b39400f61012052620001486276a70083111561019462000181565b6200015c62278d0082111561019562000181565b429091016101408190520161016052620001768162000196565b5050505050620002cc565b8162000192576200019281620001f2565b5050565b6040516001600160a01b038216907f94b979b6831a51293e2641426f97747feed46f17779fed9cd18d1ecefcfe92ef90600090a2600380546001600160a01b0390921661010002610100600160a81b0319909216919091179055565b62461bcd60e51b6000908152602060045260076024526642414c23000030600a808404818106603090810160081b95839006959095019082900491820690940160101b939093010160c81b604452606490fd5b610be680620062f083390190565b6000806000806080858703121562000269578384fd5b84516200027681620002b3565b60208601519094506200028981620002b3565b6040860151606090960151949790965092505050565b6001600160a01b0391909116815260200190565b6001600160a01b0381168114620002c957600080fd5b50565b60805160601c60a05160601c60c05160e05161010051610120516101405161016051615fc06200033060003980611aed525080611ac952508061289f5250806128e15250806128c05250806110fd5250806113b15250806105285250615fc06000f3fe6080604052600436106101a55760003560e01c8063945bcec9116100e1578063e6c460921161008a578063f84d066e11610064578063f84d066e1461048a578063f94d4668146104aa578063fa6e671d146104d9578063fec90d72146104f9576101d3565b8063e6c4609214610427578063ed24911d14610447578063f6c009271461045c576101d3565b8063b05f8e48116100bb578063b05f8e48146103cf578063b95cac28146103ff578063d2946c2b14610412576101d3565b8063945bcec914610385578063aaabadc514610398578063ad5c4648146103ba576101d3565b806352bbbe291161014e5780637d3aeb96116101285780637d3aeb9614610305578063851c1bb3146103255780638bdb39131461034557806390193b7c14610365576101d3565b806352bbbe29146102b25780635c38449e146102c557806366a9c7d2146102e5576101d3565b80630f5a6efa1161017f5780630f5a6efa1461024157806316c38b3c1461026e5780631c0de0511461028e576101d3565b8063058a628f146101d857806309b2760f146101f85780630e8e3e841461022e576101d3565b366101d3576101d16101b5610526565b6001600160a01b0316336001600160a01b03161461020661054b565b005b600080fd5b3480156101e457600080fd5b506101d16101f3366004615157565b61055d565b34801561020457600080fd5b506102186102133660046156e6565b610581565b6040516102259190615d3e565b60405180910390f35b6101d161023c36600461531e565b610634565b34801561024d57600080fd5b5061026161025c3660046151f5565b610770565b6040516102259190615d08565b34801561027a57600080fd5b506101d161028936600461545c565b610806565b34801561029a57600080fd5b506102a361081f565b60405161022593929190615d26565b6102186102c036600461588f565b610848565b3480156102d157600080fd5b506101d16102e036600461565b565b6109e9565b3480156102f157600080fd5b506101d1610300366004615545565b610e06565b34801561031157600080fd5b506101d1610320366004615516565b610fa5565b34801561033157600080fd5b50610218610340366004615633565b6110f9565b34801561035157600080fd5b506101d16103603660046154ac565b61114b565b34801561037157600080fd5b50610218610380366004615157565b611161565b610261610393366004615786565b61117c565b3480156103a457600080fd5b506103ad6112b0565b6040516102259190615b63565b3480156103c657600080fd5b506103ad6112c4565b3480156103db57600080fd5b506103ef6103ea36600461560f565b6112d3565b6040516102259493929190615eb9565b6101d161040d3660046154ac565b611396565b34801561041e57600080fd5b506103ad6113af565b34801561043357600080fd5b506101d1610442366004615243565b6113d3565b34801561045357600080fd5b506102186114ef565b34801561046857600080fd5b5061047c610477366004615494565b6114f9565b604051610225929190615b9b565b34801561049657600080fd5b506102616104a5366004615702565b611523565b3480156104b657600080fd5b506104ca6104c5366004615494565b611620565b60405161022593929190615cd2565b3480156104e557600080fd5b506101d16104f43660046151ab565b611654565b34801561050557600080fd5b50610519610514366004615173565b6116e6565b6040516102259190615d1b565b7f00000000000000000000000000000000000000000000000000000000000000005b90565b8161055957610559816116fb565b5050565b610565611768565b61056d611781565b610576816117af565b61057e611822565b50565b600061058b611768565b610593611829565b60006105a2338460065461183e565b6000818152600560205260409020549091506105c49060ff16156101f461054b565b60008181526005602052604090819020805460ff1916600190811790915560068054909101905551339082907f3c13bc30b8e878c53fd2a36b679409c073afd75950be43d8858768e956fbc20e9061061d908790615e3a565b60405180910390a3905061062f611822565b919050565b61063c611768565b6000806000805b845181101561075b5760008060008060006106718a878151811061066357fe5b60200260200101518961187d565b9c50939850919650945092509050600185600381111561068d57fe5b14156106a45761069f848383866118f5565b61074a565b866106b6576106b1611829565b600196505b60008560038111156106c457fe5b14156106f5576106d684838386611918565b6106df84611938565b1561069f576106ee8984611945565b985061074a565b61070a61070185611938565b1561020761054b565b600061071585610548565b9050600286600381111561072557fe5b141561073c5761073781848487611957565b610748565b61074881848487611970565b505b505060019093019250610643915050565b50610765836119de565b50505061057e611822565b6060815167ffffffffffffffff8111801561078a57600080fd5b506040519080825280602002602001820160405280156107b4578160200160208202803683370190505b50905060005b82518110156107ff576107e0848483815181106107d357fe5b6020026020010151611a01565b8282815181106107ec57fe5b60209081029190910101526001016107ba565b5092915050565b61080e611768565b610816611781565b61057681611a2c565b600080600061082c611aaa565b159250610837611ac7565b9150610841611aeb565b9050909192565b6000610852611768565b61085a611829565b835161086581611b0f565b610874834211156101fc61054b565b61088760008760800151116101fe61054b565b60006108968760400151611b41565b905060006108a78860600151611b41565b90506108ca816001600160a01b0316836001600160a01b031614156101fd61054b565b6108d2614ce1565b885160808201526020890151819060018111156108eb57fe5b908160018111156108f857fe5b9052506001600160a01b03808416602083015282811660408084019190915260808b0151606084015260a08b01516101008401528951821660c08401528901511660e082015260008061094a83611b66565b9198509250905061098160008c60200151600181111561096657fe5b146109745789831115610979565b898210155b6101fb61054b565b6109998b60400151838c600001518d60200151611c5a565b6109b18b60600151828c604001518d60600151611d38565b6109d36109c18c60400151611938565b6109cc5760006109ce565b825b6119de565b5050505050506109e1611822565b949350505050565b6109f1611768565b6109f9611829565b610a0583518351611e12565b6060835167ffffffffffffffff81118015610a1f57600080fd5b50604051908082528060200260200182016040528015610a49578160200160208202803683370190505b5090506060845167ffffffffffffffff81118015610a6657600080fd5b50604051908082528060200260200182016040528015610a90578160200160208202803683370190505b5090506000805b8651811015610c09576000878281518110610aae57fe5b602002602001015190506000878381518110610ac657fe5b60200260200101519050610b11846001600160a01b0316836001600160a01b03161160006001600160a01b0316846001600160a01b031614610b09576066610b0c565b60685b61054b565b819350816001600160a01b03166370a08231306040518263ffffffff1660e01b8152600401610b409190615b63565b60206040518083038186803b158015610b5857600080fd5b505afa158015610b6c573d6000803e3d6000fd5b505050506040513d601f19601f82011682018060405250810190610b909190615968565b858481518110610b9c57fe5b602002602001018181525050610bb181611e1f565b868481518110610bbd57fe5b602002602001018181525050610beb81868581518110610bd957fe5b6020026020010151101561021061054b565b610bff6001600160a01b0383168b83611ea6565b5050600101610a97565b506040517ff04f27070000000000000000000000000000000000000000000000000000000081526001600160a01b0388169063f04f270790610c55908990899088908a90600401615c85565b600060405180830381600087803b158015610c6f57600080fd5b505af1158015610c83573d6000803e3d6000fd5b5050505060005b8651811015610df4576000878281518110610ca157fe5b602002602001015190506000848381518110610cb957fe5b602002602001015190506000826001600160a01b03166370a08231306040518263ffffffff1660e01b8152600401610cf19190615b63565b60206040518083038186803b158015610d0957600080fd5b505afa158015610d1d573d6000803e3d6000fd5b505050506040513d601f19601f82011682018060405250810190610d419190615968565b9050610d528282101561020361054b565b60008282039050610d7b888681518110610d6857fe5b602002602001015182101561025a61054b565b610d858482611f11565b836001600160a01b03168c6001600160a01b03167f0d7d75e01ab95780d3cd1c8ec0dd6c2ce19e3a20427eec8bf53283b6fb8e95f08c8881518110610dc657fe5b602002602001015184604051610ddd929190615e4d565b60405180910390a350505050806001019050610c8a565b50505050610e00611822565b50505050565b610e0e611768565b610e16611829565b82610e2081611f33565b610e2c83518351611e12565b60005b8351811015610eca576000848281518110610e4657fe5b60200260200101519050610e7260006001600160a01b0316826001600160a01b0316141561013561054b565b838281518110610e7e57fe5b6020908102919091018101516000888152600a835260408082206001600160a01b0395861683529093529190912080546001600160a01b03191692909116919091179055600101610e2f565b506000610ed685611f64565b90506002816002811115610ee657fe5b1415610f3457610efc845160021461020c61054b565b610f2f8585600081518110610f0d57fe5b602002602001015186600181518110610f2257fe5b6020026020010151611f7e565b610f5c565b6001816002811115610f4257fe5b1415610f5257610f2f858561202a565b610f5c8585612082565b847ff5847d3f2197b16cdcd2098ec95d0905cd1abdaf415f07bb7cef2bba8ac5dec48585604051610f8e929190615bed565b60405180910390a25050610fa0611822565b505050565b610fad611768565b610fb5611829565b81610fbf81611f33565b6000610fca84611f64565b90506002816002811115610fda57fe5b141561102857610ff0835160021461020c61054b565b611023848460008151811061100157fe5b60200260200101518560018151811061101657fe5b60200260200101516120d7565b611050565b600181600281111561103657fe5b1415611046576110238484612145565b61105084846121ff565b60005b83518110156110b657600a6000868152602001908152602001600020600085838151811061107d57fe5b6020908102919091018101516001600160a01b0316825281019190915260400160002080546001600160a01b0319169055600101611053565b50837f7dcdc6d02ef40c7c1a7046a011b058bd7f988fa14e20a66344f9d4e60657d610846040516110e79190615bda565b60405180910390a25050610559611822565b60007f00000000000000000000000000000000000000000000000000000000000000008260405160200161112e929190615ac2565b604051602081830303815290604052805190602001209050919050565b610e00600185858561115c86612262565b61226e565b6001600160a01b031660009081526002602052604090205490565b6060611186611768565b61118e611829565b835161119981611b0f565b6111a8834211156101fc61054b565b6111b486518551611e12565b6111c08787878b6123f4565b91506000805b87518110156112925760008882815181106111dd57fe5b6020026020010151905060008583815181106111f557fe5b6020026020010151905061122188848151811061120e57fe5b60200260200101518213156101fb61054b565b600081131561126157885160208a015182916112409185918491611c5a565b61124983611938565b1561125b576112588582611945565b94505b50611288565b600081121561128857600081600003905061128683828c604001518d60600151611d38565b505b50506001016111c6565b5061129c816119de565b50506112a6611822565b9695505050505050565b60035461010090046001600160a01b031690565b60006112ce610526565b905090565b600080600080856112e381612683565b6000806112ef89611f64565b905060028160028111156112ff57fe5b14156113165761130f89896126a1565b9150611341565b600181600281111561132457fe5b14156113345761130f898961271b565b61133e8989612789565b91505b61134a826127a1565b9650611355826127b4565b9550611360826127ca565b6000998a52600a60209081526040808c206001600160a01b039b8c168d5290915290992054969995989796909616955050505050565b61139e611829565b610e00600085858561115c86612262565b7f000000000000000000000000000000000000000000000000000000000000000090565b6113db611768565b6113e3611829565b6113eb614d31565b60005b82518110156114e55782818151811061140357fe5b6020026020010151915060008260200151905061141f81612683565b604083015161143961143183836127d0565b61020961054b565b6000828152600a602090815260408083206001600160a01b03858116855292529091205461146c911633146101f661054b565b835160608501516000806114828487878661282c565b91509150846001600160a01b0316336001600160a01b0316877f6edcaf6241105b4c94c2efdbf3a6b12458eb3d07be3a0e81d24b13c44045fe7a85856040516114cc929190615e4d565b60405180910390a45050505050508060010190506113ee565b505061057e611822565b60006112ce61289b565b6000808261150681612683565b61150f84612938565b61151885611f64565b925092505b50915091565b60603330146115f6576000306001600160a01b0316600036604051611549929190615ada565b6000604051808303816000865af19150503d8060008114611586576040519150601f19603f3d011682016040523d82523d6000602084013e61158b565b606091505b50509050806000811461159a57fe5b60046000803e6000516001600160e01b0319167ffa61cc120000000000000000000000000000000000000000000000000000000081146115de573d6000803e3d6000fd5b50602060005260043d0380600460203e602081016000f35b6060611604858585896123f4565b9050602081510263fa61cc126020830352600482036024820181fd5b60608060008361162f81612683565b606061163a8661293e565b9095509050611648816129a0565b95979096509350505050565b61165c611768565b611664611829565b8261166e81611b0f565b6001600160a01b0384811660008181526004602090815260408083209488168084529490915290819020805460ff1916861515179055519091907f46961fdb4502b646d5095fba7600486a8ac05041d55cdf0f16ed677180b5cad8906116d5908690615d1b565b60405180910390a350610fa0611822565b60006116f28383612a4f565b90505b92915050565b7f08c379a0000000000000000000000000000000000000000000000000000000006000908152602060045260076024526642414c23000030600a808404818106603090810160081b95839006959095019082900491820690940160101b939093010160c81b604452606490fd5b61177a6002600054141561019061054b565b6002600055565b60006117986000356001600160e01b0319166110f9565b905061057e6117a78233612a7d565b61019161054b565b6040516001600160a01b038216907f94b979b6831a51293e2641426f97747feed46f17779fed9cd18d1ecefcfe92ef90600090a2600380546001600160a01b03909216610100027fffffffffffffffffffffff0000000000000000000000000000000000000000ff909216919091179055565b6001600055565b61183c611834611aaa565b61019261054b565b565b600069ffffffffffffffffffff8216605084600281111561185b57fe5b901b17606085901b6bffffffffffffffffffffffff19161790505b9392505050565b600080600080600080600088606001519050336001600160a01b0316816001600160a01b0316146118cf57876118ba576118b5611781565b600197505b6118cf6118c78233612a4f565b6101f761054b565b885160208a015160408b01516080909b0151919b909a9992985090965090945092505050565b61190a8361190286611b41565b836000612b20565b50610e008482846000611d38565b61192b8261192586611b41565b83612b76565b610e008482856000611c5a565b6001600160a01b03161590565b60008282016116f2848210158361054b565b6119648385836000612b20565b50610e00828583612b76565b8015610e005761198b6001600160a01b038516848484612ba6565b826001600160a01b0316846001600160a01b03167f540a1a3f28340caec336c81d8d7b3df139ee5cdc1839a4f283d7ebb7eaae2d5c84846040516119d0929190615bc1565b60405180910390a350505050565b6119ed8134101561020461054b565b348190038015610559576105593382612bc7565b6001600160a01b039182166000908152600b6020908152604080832093909416825291909152205490565b8015611a4c57611a47611a3d611ac7565b421061019361054b565b611a61565b611a61611a57611aeb565b42106101a961054b565b6003805460ff19168215151790556040517f9e3a5e37224532dea67b89face185703738a228a6e8a23dee546960180d3be6490611a9f908390615d1b565b60405180910390a150565b6000611ab4611aeb565b4211806112ce57505060035460ff161590565b7f000000000000000000000000000000000000000000000000000000000000000090565b7f000000000000000000000000000000000000000000000000000000000000000090565b336001600160a01b0382161461057e57611b27611781565b611b318133612a4f565b61057e5761057e816101f7612c41565b6000611b4c82611938565b611b5e57611b5982610548565b6116f5565b6116f5610526565b600080600080611b798560800151612938565b90506000611b8a8660800151611f64565b90506002816002811115611b9a57fe5b1415611bb157611baa8683612c75565b9450611bdc565b6001816002811115611bbf57fe5b1415611bcf57611baa8683612d25565b611bd98683612db8565b94505b611bef8660000151876060015187612ff7565b809450819550505085604001516001600160a01b031686602001516001600160a01b031687608001517f2170c741c41531aec20e7c107c24eecfdd15e69c9bb0a8dd37b1840b9e0b207b8787604051611c49929190615e4d565b60405180910390a450509193909250565b82611c6457610e00565b611c6d84611938565b15611cee57611c7f811561020261054b565b611c8e8347101561020461054b565b611c96610526565b6001600160a01b031663d0e30db0846040518263ffffffff1660e01b81526004016000604051808303818588803b158015611cd057600080fd5b505af1158015611ce4573d6000803e3d6000fd5b5050505050610e00565b6000611cf985610548565b90508115611d16576000611d108483876001612b20565b90940393505b8315611d3157611d316001600160a01b038216843087612ba6565b5050505050565b82611d4257610e00565b611d4b84611938565b15611ddb57611d5d811561020261054b565b611d65610526565b6001600160a01b0316632e1a7d4d846040518263ffffffff1660e01b8152600401611d909190615d3e565b600060405180830381600087803b158015611daa57600080fd5b505af1158015611dbe573d6000803e3d6000fd5b50611dd6925050506001600160a01b03831684612bc7565b610e00565b6000611de685610548565b90508115611dfe57611df9838286612b76565b611d31565b611d316001600160a01b0382168486611ea6565b610559818314606761054b565b600080611e2a6113af565b6001600160a01b031663d877845c6040518163ffffffff1660e01b815260040160206040518083038186803b158015611e6257600080fd5b505afa158015611e76573d6000803e3d6000fd5b505050506040513d601f19601f82011682018060405250810190611e9a9190615968565b90506118768382613025565b610fa08363a9059cbb60e01b8484604051602401611ec5929190615bc1565b60408051601f198184030181529190526020810180517bffffffffffffffffffffffffffffffffffffffffffffffffffffffff166001600160e01b031990931692909217909152613072565b801561055957610559611f226113af565b6001600160a01b0384169083611ea6565b611f3c81612683565b61057e611f4882612938565b6001600160a01b0316336001600160a01b0316146101f561054b565b600061ffff605083901c166116f5600382106101f461054b565b611f9f816001600160a01b0316836001600160a01b0316141561020a61054b565b611fbe816001600160a01b0316836001600160a01b031610606661054b565b60008381526009602052604090208054611ffb906001600160a01b0316158015611ff3575060018201546001600160a01b0316155b61020b61054b565b80546001600160a01b039384166001600160a01b03199182161782556001909101805492909316911617905550565b6000828152600860205260408120905b8251811015610e0057600061206b84838151811061205457fe5b60200260200101518461311290919063ffffffff16565b90506120798161020a61054b565b5060010161203a565b6000828152600160205260408120905b8251811015610e005760006120c08483815181106120ac57fe5b602090810291909101015184906000613175565b90506120ce8161020a61054b565b50600101612092565b60008060006120e7868686613222565b9250925092506121116120f9846132e9565b80156121095750612109836132e9565b61020d61054b565b600095865260096020526040862080546001600160a01b031990811682556001909101805490911690559490945550505050565b6000828152600860205260408120905b8251811015610e0057600083828151811061216c57fe5b602002602001015190506121b8612109600760008881526020019081526020016000206000846001600160a01b03166001600160a01b03168152602001908152602001600020546132e9565b60008581526007602090815260408083206001600160a01b038516845290915281208190556121e7848361330b565b90506121f58161020961054b565b5050600101612155565b6000828152600160205260408120905b8251811015610e0057600083828151811061222657fe5b60200260200101519050600061223c8483613412565b905061224a612109826132e9565b6122548483613421565b50505080600101905061220f565b61226a614d5a565b5090565b612276611768565b8361228081612683565b8361228a81611b0f565b61229e836000015151846020015151611e12565b60606122ad84600001516134c3565b905060606122bb8883613552565b905060608060606122d08c8c8c8c8c896135e3565b92509250925060006122e18c611f64565b905060028160028111156122f157fe5b1415612359576123548c8760008151811061230857fe5b60200260200101518660008151811061231d57fe5b60200260200101518960018151811061233257fe5b60200260200101518860018151811061234757fe5b60200260200101516137a8565b612382565b600181600281111561236757fe5b1415612378576123548c87866137e7565b6123828c85613854565b6000808e600181111561239157fe5b1490508b6001600160a01b03168d7fe5ce249087ce04f05a957192435400fd97868dba0e6a4b4c049abf8af80dae78896123cb888661389d565b876040516123db93929190615c4c565b60405180910390a3505050505050505050611d31611822565b6060835167ffffffffffffffff8111801561240e57600080fd5b50604051908082528060200260200182016040528015612438578160200160208202803683370190505b509050612443614d84565b61244b614ce1565b60008060005b89518110156126765789818151811061246657fe5b6020026020010151945060008951866020015110801561248a575089518660400151105b905061249781606461054b565b60006124b98b8860200151815181106124ac57fe5b6020026020010151611b41565b905060006124d08c8960400151815181106124ac57fe5b90506124f3816001600160a01b0316836001600160a01b031614156101fd61054b565b60608801516125435761250b600085116101fe61054b565b60006125188b8484613945565b6001600160a01b0316876001600160a01b031614905061253a816101ff61054b565b50606088018590525b87516080880152868a600181111561255757fe5b9081600181111561256457fe5b9052506001600160a01b0380831660208901528181166040808a01919091526060808b0151908a015260808a01516101008a01528c51821660c08a01528c01511660e08801526000806125b689611b66565b919850925090506125c88c8585613967565b97506125fc6125d683613981565b8c8c60200151815181106125e657fe5b60200260200101516139b190919063ffffffff16565b8b8b602001518151811061260c57fe5b60200260200101818152505061264a61262482613981565b8c8c604001518151811061263457fe5b60200260200101516139e590919063ffffffff16565b8b8b604001518151811061265a57fe5b6020026020010181815250505050505050806001019050612451565b5050505050949350505050565b60008181526005602052604090205461057e9060ff166101f461054b565b60008060008060006126b287613a19565b945094509450945050836001600160a01b0316866001600160a01b031614156126e157829450505050506116f5565b816001600160a01b0316866001600160a01b031614156127065793506116f592505050565b6127116102096116fb565b5050505092915050565b60008281526007602090815260408083206001600160a01b03851684529091528120548161274882613a8f565b80612766575060008581526008602052604090206127669085613aa1565b9050806127815761277685612683565b6127816102096116fb565b509392505050565b60008281526001602052604081206109e18184613412565b6dffffffffffffffffffffffffffff1690565b60701c6dffffffffffffffffffffffffffff1690565b60e01c90565b6000806127dc84611f64565b905060028160028111156127ec57fe5b1415612804576127fc8484613ac2565b9150506116f5565b600181600281111561281257fe5b1415612822576127fc8484613b13565b6127fc8484613b2b565b600080600061283a86611f64565b9050600087600281111561284a57fe5b14156128665761285c86828787613b43565b9250925050612892565b600187600281111561287457fe5b14156128865761285c86828787613bbe565b61285c86828787613c3a565b94509492505050565b60007f00000000000000000000000000000000000000000000000000000000000000007f00000000000000000000000000000000000000000000000000000000000000007f0000000000000000000000000000000000000000000000000000000000000000612908613c9d565b3060405160200161291d959493929190615df0565b60405160208183030381529060405280519060200120905090565b60601c90565b606080600061294c84611f64565b9050600281600281111561295c57fe5b14156129755761296b84613ca1565b925092505061299b565b600181600281111561298357fe5b14156129925761296b84613dd6565b61296b84613efd565b915091565b60606000825167ffffffffffffffff811180156129bc57600080fd5b506040519080825280602002602001820160405280156129e6578160200160208202803683370190505b5091506000905060005b825181101561151d576000848281518110612a0757fe5b60200260200101519050612a1a81613ff9565b848381518110612a2657fe5b602002602001018181525050612a4483612a3f836127ca565b614014565b9250506001016129f0565b6001600160a01b03918216600090815260046020908152604080832093909416825291909152205460ff1690565b6003546040517f9be2a88400000000000000000000000000000000000000000000000000000000815260009161010090046001600160a01b031690639be2a88490612ad090869086903090600401615d47565b60206040518083038186803b158015612ae857600080fd5b505afa158015612afc573d6000803e3d6000fd5b505050506040513d601f19601f820116820180604052508101906116f29190615478565b600080612b2d8686611a01565b9050612b468380612b3e5750848210155b61020161054b565b612b50818561402b565b9150818103612b6c878783612b6487613981565b60000361403a565b5050949350505050565b6000612b828484611a01565b90506000612b908284611945565b9050611d31858583612ba187613981565b61403a565b610e00846323b872dd60e01b858585604051602401611ec593929190615b77565b612bd6814710156101a361054b565b6000826001600160a01b031682604051612bef90610548565b60006040518083038185875af1925050503d8060008114612c2c576040519150601f19603f3d011682016040523d82523d6000602084013e612c31565b606091505b50509050610fa0816101a461054b565b6001600160a01b0382166000908152600260205260409020805460018101909155610fa0612c6f8483614095565b8361054b565b600080600080612c92866080015187602001518860400151613222565b92509250925060008087604001516001600160a01b031688602001516001600160a01b03161015612cc7575083905082612ccd565b50829050835b612cd9888884846141bb565b60408b015160208c01519199509294509092506001600160a01b03918216911610612d0d57612d0881836142d1565b612d17565b612d1782826142d1565b909255509295945050505050565b600080612d3a8460800151856020015161271b565b90506000612d508560800151866040015161271b565b9050612d5e858584846141bb565b6080880180516000908152600760208181526040808420828e01516001600160a01b03908116865290835281852098909855935183529081528282209a830151909516815298909352919096209590955550929392505050565b60808201516000908152600160209081526040822090840151829182918290612de290839061430c565b90506000612dfd88604001518461430c90919063ffffffff16565b9050811580612e0a575080155b15612e2757612e1c8860800151612683565b612e276102096116fb565b60001991820191016000612e3a8461432b565b905060608167ffffffffffffffff81118015612e5557600080fd5b50604051908082528060200260200182016040528015612e7f578160200160208202803683370190505b50600060a08c018190529091505b82811015612eff576000612ea1878361432f565b9050612eac81613ff9565b838381518110612eb857fe5b602002602001018181525050612ed58c60a00151612a3f836127ca565b60a08d015281861415612eea57809850612ef6565b84821415612ef6578097505b50600101612e8d565b506040517f01ec954a0000000000000000000000000000000000000000000000000000000081526001600160a01b038a16906301ec954a90612f4b908d90859089908990600401615e5b565b602060405180830381600087803b158015612f6557600080fd5b505af1158015612f79573d6000803e3d6000fd5b505050506040513d601f19601f82011682018060405250810190612f9d9190615968565b9750600080612fb58c600001518d606001518c612ff7565b9092509050612fc48983614345565b9850612fd08882614376565b9750612fdd87878b61438c565b612fe887868a61438c565b50505050505050505092915050565b6000808085600181111561300757fe5b141561301757508290508161301d565b50819050825b935093915050565b600082820261304984158061304257508385838161303f57fe5b04145b600361054b565b806130585760009150506116f5565b670de0b6b3a76400006000198201046001019150506116f5565b60006060836001600160a01b03168360405161308e9190615aea565b6000604051808303816000865af19150503d80600081146130cb576040519150601f19603f3d011682016040523d82523d6000602084013e6130d0565b606091505b509150915060008214156130e8573d6000803e3d6000fd5b610e0081516000148061310a57508180602001905181019061310a9190615478565b6101a261054b565b600061311e8383613aa1565b61316d57508154600180820184556000848152602080822090930180546001600160a01b0319166001600160a01b038616908117909155855490825282860190935260409020919091556116f5565b5060006116f5565b6001600160a01b03821660009081526002840160205260408120548061320257505082546040805180820182526001600160a01b03858116808352602080840187815260008781526001808c018452878220965187546001600160a01b03191696169590951786559051948401949094559482018089559083526002880190945291902091909155611876565b600019016000908152600180860160205260408220018390559050611876565b600080600080600061323487876143a4565b91509150600061324483836143d5565b60008a81526009602090815260408083208484526002019091528120805460018201549197509293509061327783613a8f565b80613286575061328682613a8f565b806132a757506132968c87613ac2565b80156132a757506132a78c86613ac2565b9050806132c2576132b78c612683565b6132c26102096116fb565b6132cc8383614408565b98506132d8838361442d565b975050505050505093509350939050565b7bffffffffffffffffffffffffffffffffffffffffffffffffffffffff161590565b6001600160a01b03811660009081526001830160205260408120548015613408578354600019808301919081019060009087908390811061334857fe5b60009182526020909120015487546001600160a01b039091169150819088908590811061337157fe5b600091825260208083209190910180546001600160a01b0319166001600160a01b039485161790559183168152600189810190925260409020908401905586548790806133ba57fe5b60008281526020808220830160001990810180546001600160a01b03191690559092019092556001600160a01b03881682526001898101909152604082209190915594506116f59350505050565b60009150506116f5565b60006116f28383610209614444565b6001600160a01b0381166000908152600283016020526040812054801561340857835460001990810160008181526001878101602090815260408084209587018452808420865481546001600160a01b03199081166001600160a01b0392831617835588860180549387019390935588548216875260028d018086528488209a909a5588541690975584905593895593871682529390925281205590506116f5565b606080825167ffffffffffffffff811180156134de57600080fd5b50604051908082528060200260200182016040528015613508578160200160208202803683370190505b50905060005b83518110156107ff576135268482815181106124ac57fe5b82828151811061353257fe5b6001600160a01b039092166020928302919091019091015260010161350e565b60608060606135608561293e565b9150915061357082518551611e12565b613580600083511161020f61054b565b60005b82518110156135da576135d285828151811061359b57fe5b60200260200101516001600160a01b03168483815181106135b857fe5b60200260200101516001600160a01b03161461020861054b565b600101613583565b50949350505050565b60608060608060006135f4866129a0565b9150915060006136038b612938565b905060008c600181111561361357fe5b146136b657806001600160a01b03166374f3b0098c8c8c8787613634614481565b8f604001516040518863ffffffff1660e01b815260040161365b9796959493929190615d66565b600060405180830381600087803b15801561367557600080fd5b505af1158015613689573d6000803e3d6000fd5b505050506040513d6000823e601f3d908101601f191682016040526136b19190810190615405565b61374f565b806001600160a01b031663d5c096c48c8c8c87876136d2614481565b8f604001516040518863ffffffff1660e01b81526004016136f99796959493929190615d66565b600060405180830381600087803b15801561371357600080fd5b505af1158015613727573d6000803e3d6000fd5b505050506040513d6000823e601f3d908101601f1916820160405261374f9190810190615405565b80955081965050506137658751865186516144fb565b60008c600181111561377357fe5b1461378a576137858989898888614513565b613797565b6137978a8989888861465a565b955050505096509650969350505050565b60006137b485846143d5565b600087815260096020908152604080832084845260020190915290209091506137dd85846142d1565b9055505050505050565b60005b8251811015610e00578181815181106137ff57fe5b602002602001015160076000868152602001908152602001600020600085848151811061382857fe5b6020908102919091018101516001600160a01b03168252810191909152604001600020556001016137ea565b6000828152600160205260408120905b8251811015610e00576138958184838151811061387d57fe5b60200260200101518461438c9092919063ffffffff16565b600101613864565b6060825167ffffffffffffffff811180156138b757600080fd5b506040519080825280602002602001820160405280156138e1578160200160208202803683370190505b50905060005b83518110156107ff57826139115783818151811061390157fe5b6020026020010151600003613926565b83818151811061391d57fe5b60200260200101515b82828151811061393257fe5b60209081029190910101526001016138e7565b60008084600181111561395457fe5b1461395f57816109e1565b509092915050565b60008084600181111561397657fe5b146107ff57826109e1565b600061226a7f800000000000000000000000000000000000000000000000000000000000000083106101a561054b565b60008282016116f28284128015906139c95750848212155b806139de57506000841280156139de57508482125b600061054b565b60008183036116f28284128015906139fd5750848213155b80613a125750600084128015613a1257508482135b600161054b565b6000818152600960205260408120805460018201546001600160a01b0391821692849290911690829081613a4d86856143d5565b6000818152600284016020526040902080546001820154919950919250613a748282614408565b9650613a80828261442d565b94505050505091939590929450565b6000613a9a826132e9565b1592915050565b6001600160a01b031660009081526001919091016020526040902054151590565b600082815260096020526040812080546001600160a01b0384811691161480613afa575060018101546001600160a01b038481169116145b80156109e1575050506001600160a01b03161515919050565b60008281526008602052604081206109e18184613aa1565b60008281526001602052604081206109e181846147d0565b6000806002856002811115613b5457fe5b1415613b6a57613b658685856147f1565b613b94565b6001856002811115613b7857fe5b1415613b8957613b658685856147ff565b613b9486858561480d565b8215613bae57613bae6001600160a01b0385163385611ea6565b5050600081900394909350915050565b6000806002856002811115613bcf57fe5b1415613be557613be086858561481b565b613c0f565b6001856002811115613bf357fe5b1415613c0457613be0868585614829565b613c0f868585614837565b8215613c2a57613c2a6001600160a01b038516333086612ba6565b5090946000869003945092505050565b6000806002856002811115613c4b57fe5b1415613c6357613c5c868585614845565b9050613c90565b6001856002811115613c7157fe5b1415613c8257613c5c868585614855565b613c8d868585614865565b90505b6000915094509492505050565b4690565b606080600080600080613cb387613a19565b92975090955093509150506001600160a01b0384161580613cdb57506001600160a01b038216155b15613d04575050604080516000808252602082019081528183019092529450925061299b915050565b60408051600280825260608201835290916020830190803683370190505095508386600081518110613d3257fe5b60200260200101906001600160a01b031690816001600160a01b0316815250508186600181518110613d6057fe5b6001600160a01b03929092166020928302919091018201526040805160028082526060820183529092909190830190803683370190505094508285600081518110613da757fe5b6020026020010181815250508085600181518110613dc157fe5b60200260200101818152505050505050915091565b60008181526008602052604090206060908190613df28161432b565b67ffffffffffffffff81118015613e0857600080fd5b50604051908082528060200260200182016040528015613e32578160200160208202803683370190505b509250825167ffffffffffffffff81118015613e4d57600080fd5b50604051908082528060200260200182016040528015613e77578160200160208202803683370190505b50915060005b8351811015613ef6576000613e928383614875565b905080858381518110613ea157fe5b6001600160a01b03928316602091820292909201810191909152600088815260078252604080822093851682529290915220548451859084908110613ee257fe5b602090810291909101015250600101613e7d565b5050915091565b60008181526001602052604090206060908190613f198161432b565b67ffffffffffffffff81118015613f2f57600080fd5b50604051908082528060200260200182016040528015613f59578160200160208202803683370190505b509250825167ffffffffffffffff81118015613f7457600080fd5b50604051908082528060200260200182016040528015613f9e578160200160208202803683370190505b50915060005b8351811015613ef657613fb782826148a2565b858381518110613fc357fe5b60200260200101858481518110613fd657fe5b60209081029190910101919091526001600160a01b039091169052600101613fa4565b6000614004826127b4565b61400d836127a1565b0192915050565b60008183101561402457816116f2565b5090919050565b600081831061402457816116f2565b6001600160a01b038085166000818152600b602090815260408083209488168084529490915290819020859055517f18e1ea4139e68413d7d08aa752e71568e36b2c5bf940893314c2c5b01eaa0c42906119d0908590615d3e565b6000806140a06148c6565b9050428110156140b45760009150506116f5565b60006140be6148d2565b9050806140d0576000925050506116f5565b6000816140db6149e3565b80516020918201206040516140f7939233918a91899101615dc4565b604051602081830303815290604052805190602001209050600061411a82614a32565b90506000806000614129614a4e565b9250925092506000600185858585604051600081526020016040526040516141549493929190615e1c565b6020604051602081039080840390855afa158015614176573d6000803e3d6000fd5b5050604051601f1901519150506001600160a01b038116158015906141ac57508a6001600160a01b0316816001600160a01b0316145b9b9a5050505050505050505050565b6000806000806141ca86613ff9565b905060006141d786613ff9565b90506141ee6141e5886127ca565b612a3f886127ca565b60a08a01526040517f9d2c110c0000000000000000000000000000000000000000000000000000000081526001600160a01b03891690639d2c110c9061423c908c9086908690600401615e94565b602060405180830381600087803b15801561425657600080fd5b505af115801561426a573d6000803e3d6000fd5b505050506040513d601f19601f8201168201806040525081019061428e9190615968565b92506000806142a68b600001518c6060015187612ff7565b90925090506142b58983614345565b96506142c18882614376565b9550505050509450945094915050565b6000806142e96142e0856127ca565b612a3f856127ca565b90506109e16142f7856127a1565b614300856127a1565b8363ffffffff16614a75565b6001600160a01b03166000908152600291909101602052604090205490565b5490565b6000908152600191820160205260409020015490565b60008061435b83614355866127a1565b90611945565b90506000614368856127b4565b9050436112a6838383614a83565b60008061435b83614386866127a1565b90614abc565b60009182526001928301602052604090912090910155565b600080826001600160a01b0316846001600160a01b0316106143c75782846143ca565b83835b915091509250929050565b600082826040516020016143ea929190615b06565b60405160208183030381529060405280519060200120905092915050565b60006116f2614416846127a1565b61441f846127a1565b614428866127ca565b614a83565b60006116f261443b846127b4565b61441f846127b4565b6001600160a01b038216600090815260028401602052604081205461446b8115158461054b565b614478856001830361432f565b95945050505050565b600061448b6113af565b6001600160a01b03166355c676286040518163ffffffff1660e01b815260040160206040518083038186803b1580156144c357600080fd5b505afa1580156144d7573d6000803e3d6000fd5b505050506040513d601f19601f820116820180604052508101906112ce9190615968565b610fa0828414801561450c57508183145b606761054b565b6060835167ffffffffffffffff8111801561452d57600080fd5b50604051908082528060200260200182016040528015614557578160200160208202803683370190505b50905060005b85515181101561465057600084828151811061457557fe5b602002602001015190506145a58760200151838151811061459257fe5b60200260200101518210156101f961054b565b6000876000015183815181106145b757fe5b602002602001015190506145d181838b8b60600151611d38565b60008584815181106145df57fe5b602002602001015190506145fb6145f583611b41565b82611f11565b61462a6146088483611945565b89868151811061461457fe5b602002602001015161437690919063ffffffff16565b85858151811061463657fe5b60200260200101818152505050505080600101905061455d565b5095945050505050565b60606000845167ffffffffffffffff8111801561467657600080fd5b506040519080825280602002602001820160405280156146a0578160200160208202803683370190505b50915060005b8651518110156147c65760008582815181106146be57fe5b602002602001015190506146ee886020015183815181106146db57fe5b60200260200101518211156101fa61054b565b60008860000151838151811061470057fe5b6020026020010151905061471a81838c8c60600151611c5a565b61472381611938565b15614735576147328483611945565b93505b600086848151811061474357fe5b602002602001015190506147596145f583611b41565b80831015614778576147738382038a868151811061461457fe5b6147a0565b6147a08184038a868151811061478a57fe5b602002602001015161434590919063ffffffff16565b8685815181106147ac57fe5b6020026020010181815250505050508060010190506146a6565b50614650816119de565b6001600160a01b031660009081526002919091016020526040902054151590565b610e008383614ad284614b0d565b610e008383614ad284614bb8565b610e008383614ad284614c13565b610e008383614c6284614b0d565b610e008383614c6284614bb8565b610e008383614c6284614c13565b60006109e18484614c8385614b0d565b60006109e18484614c8385614bb8565b60006109e18484614c8385614c13565b600082600001828154811061488657fe5b6000918252602090912001546001600160a01b03169392505050565b600090815260019182016020526040902080549101546001600160a01b0390911691565b60006112ce6000614c9d565b6000803560e01c8063b95cac28811461491a57638bdb39138114614942576352bbbe29811461496a5763945bcec981146149925763fa6e671d81146149ba57600092506149de565b7f3f7b71252bd19113ff48c19c6e004a9bcfcca320a0d74d58e85877cbd7dcae5892506149de565b7f8bbc57f66ea936902f50a71ce12b92c43f3c5340bb40c27c4e90ab84eeae335392506149de565b7fe192dcbc143b1e244ad73b813fd3c097b832ad260a157340b4e5e5beda067abe92506149de565b7f9bfc43a4d98313c6766986ffd7c916c7481566d9f224c6819af0a53388aced3a92506149de565b7fa3f865aa351e51cfeb40f5178d1564bb629fe9030b83caf6361d1baaf5b90b5a92505b505090565b60606000368080601f0160208091040260200160405190810160405280939291908181526020018383808284376000920191909152505082519293505050608010156105485760803603815290565b6000614a3c61289b565b8260405160200161112e929190615b2d565b6000806000614a5d6020614c9d565b9250614a696040614c9d565b91506108416060614c9d565b60e01b60709190911b010190565b6000838301614ab1858210801590614aa957506e01000000000000000000000000000082105b61020e61054b565b614478858585614a75565b6000614acc83831115600161054b565b50900390565b600080614ae283614386866127a1565b90506000614af384614355876127b4565b90506000614b00866127ca565b90506112a6838383614a83565b6000806000806000614b1e89613a19565b9450509350935093506000836001600160a01b0316896001600160a01b03161415614b69576000614b5384898b63ffffffff16565b9050614b5f8185614ca7565b9093509050614b8b565b6000614b7983898b63ffffffff16565b9050614b858184614ca7565b90925090505b614b9583836142d1565b8555614ba18383614cc3565b600190950194909455509192505050949350505050565b600080614bc5868661271b565b90506000614bd782858763ffffffff16565b60008881526007602090815260408083206001600160a01b038b16845290915290208190559050614c088183614ca7565b979650505050505050565b600084815260016020526040812081614c2c8287613412565b90506000614c3e82868863ffffffff16565b9050614c4b838883613175565b50614c568183614ca7565b98975050505050505050565b600080614c7283614355866127a1565b90506000614af384614386876127b4565b600080614c8f846127a1565b905043614478828583614a83565b3601607f19013590565b6000614cb2826127b4565b614cbb846127b4565b039392505050565b60006116f2614cd1846127b4565b614cda846127b4565b6000614a75565b60408051610120810190915280600081526000602082018190526040820181905260608083018290526080830182905260a0830182905260c0830182905260e08301919091526101009091015290565b604080516080810190915280600081526000602082018190526040820181905260609091015290565b60405180608001604052806060815260200160608152602001606081526020016000151581525090565b6040518060a0016040528060008019168152602001600081526020016000815260200160008152602001606081525090565b80356116f581615f5a565b600082601f830112614dd1578081fd5b8135614de4614ddf82615f04565b615edd565b818152915060208083019084810181840286018201871015614e0557600080fd5b60005b84811015614e2d578135614e1b81615f5a565b84529282019290820190600101614e08565b505050505092915050565b600082601f830112614e48578081fd5b8135614e56614ddf82615f04565b818152915060208083019084810160005b84811015614e2d578135870160a080601f19838c03011215614e8857600080fd5b614e9181615edd565b85830135815260408084013587830152606080850135828401526080915081850135818401525082840135925067ffffffffffffffff831115614ed357600080fd5b614ee18c8885870101614fc0565b90820152865250509282019290820190600101614e67565b600082601f830112614f09578081fd5b8135614f17614ddf82615f04565b818152915060208083019084810181840286018201871015614f3857600080fd5b60005b84811015614e2d57813584529282019290820190600101614f3b565b600082601f830112614f67578081fd5b8151614f75614ddf82615f04565b818152915060208083019084810181840286018201871015614f9657600080fd5b60005b84811015614e2d57815184529282019290820190600101614f99565b80356116f581615f6f565b600082601f830112614fd0578081fd5b813567ffffffffffffffff811115614fe6578182fd5b614ff9601f8201601f1916602001615edd565b915080825283602082850101111561501057600080fd5b8060208401602084013760009082016020015292915050565b80356116f581615f7d565b8035600281106116f557600080fd5b8035600481106116f557600080fd5b600060808284031215615063578081fd5b61506d6080615edd565b9050813567ffffffffffffffff8082111561508757600080fd5b61509385838601614dc1565b835260208401359150808211156150a957600080fd5b6150b585838601614ef9565b602084015260408401359150808211156150ce57600080fd5b506150db84828501614fc0565b6040830152506150ee8360608401614fb5565b606082015292915050565b60006080828403121561510a578081fd5b6151146080615edd565b9050813561512181615f5a565b8152602082013561513181615f6f565b6020820152604082013561514481615f5a565b604082015260608201356150ee81615f6f565b600060208284031215615168578081fd5b81356116f281615f5a565b60008060408385031215615185578081fd5b823561519081615f5a565b915060208301356151a081615f5a565b809150509250929050565b6000806000606084860312156151bf578081fd5b83356151ca81615f5a565b925060208401356151da81615f5a565b915060408401356151ea81615f6f565b809150509250925092565b60008060408385031215615207578182fd5b823561521281615f5a565b9150602083013567ffffffffffffffff81111561522d578182fd5b61523985828601614dc1565b9150509250929050565b60006020808385031215615255578182fd5b823567ffffffffffffffff81111561526b578283fd5b8301601f8101851361527b578283fd5b8035615289614ddf82615f04565b818152838101908385016080808502860187018a10156152a7578788fd5b8795505b848610156153105780828b0312156152c1578788fd5b6152ca81615edd565b6152d48b84615029565b8152878301358882015260406152ec8c828601614db6565b908201526060838101359082015284526001959095019492860192908101906152ab565b509098975050505050505050565b60006020808385031215615330578182fd5b823567ffffffffffffffff811115615346578283fd5b8301601f81018513615356578283fd5b8035615364614ddf82615f04565b8181528381019083850160a0808502860187018a1015615382578788fd5b8795505b848610156153105780828b03121561539c578788fd5b6153a581615edd565b6153af8b84615043565b81526153bd8b898501614db6565b818901526040838101359082015260606153d98c828601614db6565b9082015260806153eb8c858301614db6565b908201528452600195909501949286019290810190615386565b60008060408385031215615417578182fd5b825167ffffffffffffffff8082111561542e578384fd5b61543a86838701614f57565b9350602085015191508082111561544f578283fd5b5061523985828601614f57565b60006020828403121561546d578081fd5b81356116f281615f6f565b600060208284031215615489578081fd5b81516116f281615f6f565b6000602082840312156154a5578081fd5b5035919050565b600080600080608085870312156154c1578182fd5b8435935060208501356154d381615f5a565b925060408501356154e381615f5a565b9150606085013567ffffffffffffffff8111156154fe578182fd5b61550a87828801615052565b91505092959194509250565b60008060408385031215615528578182fd5b82359150602083013567ffffffffffffffff81111561522d578182fd5b600080600060608486031215615559578081fd5b8335925060208085013567ffffffffffffffff80821115615578578384fd5b61558488838901614dc1565b94506040870135915080821115615599578384fd5b508501601f810187136155aa578283fd5b80356155b8614ddf82615f04565b81815283810190838501858402850186018b10156155d4578687fd5b8694505b838510156155ff5780356155eb81615f5a565b8352600194909401939185019185016155d8565b5080955050505050509250925092565b60008060408385031215615621578182fd5b8235915060208301356151a081615f5a565b600060208284031215615644578081fd5b81356001600160e01b0319811681146116f2578182fd5b60008060008060808587031215615670578182fd5b843561567b81615f5a565b9350602085013567ffffffffffffffff80821115615697578384fd5b6156a388838901614dc1565b945060408701359150808211156156b8578384fd5b6156c488838901614ef9565b935060608701359150808211156156d9578283fd5b5061550a87828801614fc0565b6000602082840312156156f7578081fd5b81356116f281615f7d565b60008060008060e08587031215615717578182fd5b6157218686615034565b9350602085013567ffffffffffffffff8082111561573d578384fd5b61574988838901614e38565b9450604087013591508082111561575e578384fd5b5061576b87828801614dc1565b92505061577b86606087016150f9565b905092959194509250565b600080600080600080610120878903121561579f578384fd5b6157a98888615034565b955060208088013567ffffffffffffffff808211156157c6578687fd5b6157d28b838c01614e38565b975060408a01359150808211156157e7578687fd5b6157f38b838c01614dc1565b96506158028b60608c016150f9565b955060e08a0135915080821115615817578485fd5b508801601f81018a13615828578384fd5b8035615836614ddf82615f04565b81815283810190838501858402850186018e1015615852578788fd5b8794505b83851015615874578035835260019490940193918501918501615856565b50809650505050505061010087013590509295509295509295565b60008060008060e085870312156158a4578182fd5b843567ffffffffffffffff808211156158bb578384fd5b9086019060c082890312156158ce578384fd5b6158d860c0615edd565b823581526158e98960208501615034565b602082015260408301356158fc81615f5a565b604082015261590e8960608501614db6565b60608201526080830135608082015260a08301358281111561592e578586fd5b61593a8a828601614fc0565b60a08301525080965050505061595386602087016150f9565b939693955050505060a08201359160c0013590565b600060208284031215615979578081fd5b5051919050565b6001600160a01b03169052565b6000815180845260208085019450808401835b838110156159c55781516001600160a01b0316875295820195908201906001016159a0565b509495945050505050565b6000815180845260208085019450808401835b838110156159c5578151875295820195908201906001016159e3565b60008151808452615a17816020860160208601615f24565b601f01601f19169290920160200192915050565b6000610120825160028110615a3c57fe5b808552506020830151615a526020860182615980565b506040830151615a656040860182615980565b50606083015160608501526080830151608085015260a083015160a085015260c0830151615a9660c0860182615980565b5060e0830151615aa960e0860182615980565b506101008084015182828701526112a6838701826159ff565b9182526001600160e01b031916602082015260240190565b6000828483379101908152919050565b60008251615afc818460208701615f24565b9190910192915050565b6bffffffffffffffffffffffff19606093841b811682529190921b16601482015260280190565b7f190100000000000000000000000000000000000000000000000000000000000081526002810192909252602282015260420190565b6001600160a01b0391909116815260200190565b6001600160a01b039384168152919092166020820152604081019190915260600190565b6001600160a01b038316815260408101615bb483615f50565b8260208301529392505050565b6001600160a01b03929092168252602082015260400190565b6000602082526116f2602083018461598d565b600060408252615c00604083018561598d565b828103602084810191909152845180835285820192820190845b81811015615c3f5784516001600160a01b031683529383019391830191600101615c1a565b5090979650505050505050565b600060608252615c5f606083018661598d565b8281036020840152615c7181866159d0565b905082810360408401526112a681856159d0565b600060808252615c98608083018761598d565b8281036020840152615caa81876159d0565b90508281036040840152615cbe81866159d0565b90508281036060840152614c0881856159ff565b600060608252615ce5606083018661598d565b8281036020840152615cf781866159d0565b915050826040830152949350505050565b6000602082526116f260208301846159d0565b901515815260200190565b92151583526020830191909152604082015260600190565b90815260200190565b9283526001600160a01b03918216602084015216604082015260600190565b60008882526001600160a01b03808916602084015280881660408401525060e06060830152615d9860e08301876159d0565b8560808401528460a084015282810360c0840152615db681856159ff565b9a9950505050505050505050565b94855260208501939093526001600160a01b039190911660408401526060830152608082015260a00190565b9485526020850193909352604084019190915260608301526001600160a01b0316608082015260a00190565b93845260ff9290921660208401526040830152606082015260800190565b60208101615e4783615f50565b91905290565b918252602082015260400190565b600060808252615e6e6080830187615a2b565b8281036020840152615e8081876159d0565b604084019590955250506060015292915050565b600060608252615ea76060830186615a2b565b60208301949094525060400152919050565b938452602084019290925260408301526001600160a01b0316606082015260800190565b60405181810167ffffffffffffffff81118282101715615efc57600080fd5b604052919050565b600067ffffffffffffffff821115615f1a578081fd5b5060209081020190565b60005b83811015615f3f578181015183820152602001615f27565b83811115610e005750506000910152565b6003811061057e57fe5b6001600160a01b038116811461057e57600080fd5b801515811461057e57600080fd5b6003811061057e57600080fdfea2646970667358221220201e4f926e390fed8dd5318c58846af735c2bebc61b80693ae936a5fe76dcf1464736f6c6343000701003360c060405234801561001057600080fd5b50604051610be6380380610be683398101604081905261002f9161004d565b30608052600160005560601b6001600160601b03191660a05261007b565b60006020828403121561005e578081fd5b81516001600160a01b0381168114610074578182fd5b9392505050565b60805160a05160601c610b406100a66000398061041352806105495250806102a75250610b406000f3fe608060405234801561001057600080fd5b50600436106100a35760003560e01c8063851c1bb311610076578063d877845c1161005b578063d877845c14610129578063e42abf3514610131578063fbfa77cf14610151576100a3565b8063851c1bb314610101578063aaabadc514610114576100a3565b806338e9922e146100a857806355c67628146100bd5780636b6b9f69146100db5780636daefab6146100ee575b600080fd5b6100bb6100b636600461099c565b610159565b005b6100c56101b8565b6040516100d29190610aa6565b60405180910390f35b6100bb6100e936600461099c565b6101be565b6100bb6100fc3660046107d1565b610211565b6100c561010f366004610924565b6102a3565b61011c6102f5565b6040516100d29190610a35565b6100c5610304565b61014461013f366004610852565b61030a565b6040516100d29190610a62565b61011c610411565b610161610435565b6101786706f05b59d3b2000082111561025861047e565b60018190556040517fa9ba3ffe0b6c366b81232caab38605a0699ad5398d6cce76f91ee809e322dafc906101ad908390610aa6565b60405180910390a150565b60015490565b6101c6610435565b6101dc662386f26fc1000082111561025961047e565b60028190556040517f5a0b7386237e7f07fa741efc64e59c9387d2cccafec760efed4d53387f20e19a906101ad908390610aa6565b610219610490565b610221610435565b61022b84836104a9565b60005b8481101561029357600086868381811061024457fe5b90506020020160208101906102599190610980565b9050600085858481811061026957fe5b6020029190910135915061028990506001600160a01b03831685836104b6565b505060010161022e565b5061029c61053e565b5050505050565b60007f0000000000000000000000000000000000000000000000000000000000000000826040516020016102d89291906109cc565b604051602081830303815290604052805190602001209050919050565b60006102ff610545565b905090565b60025490565b6060815167ffffffffffffffff8111801561032457600080fd5b5060405190808252806020026020018201604052801561034e578160200160208202803683370190505b50905060005b825181101561040b5782818151811061036957fe5b60200260200101516001600160a01b03166370a08231306040518263ffffffff1660e01b815260040161039c9190610a35565b60206040518083038186803b1580156103b457600080fd5b505afa1580156103c8573d6000803e3d6000fd5b505050506040513d601f19601f820116820180604052508101906103ec91906109b4565b8282815181106103f857fe5b6020908102919091010152600101610354565b50919050565b7f000000000000000000000000000000000000000000000000000000000000000081565b60006104646000357fffffffff00000000000000000000000000000000000000000000000000000000166102a3565b905061047b61047382336105d8565b61019161047e565b50565b8161048c5761048c8161066a565b5050565b6104a26002600054141561019061047e565b6002600055565b61048c818314606761047e565b6105398363a9059cbb60e01b84846040516024016104d5929190610a49565b60408051601f198184030181529190526020810180517bffffffffffffffffffffffffffffffffffffffffffffffffffffffff167fffffffff00000000000000000000000000000000000000000000000000000000909316929092179091526106d7565b505050565b6001600055565b60007f00000000000000000000000000000000000000000000000000000000000000006001600160a01b031663aaabadc56040518163ffffffff1660e01b815260040160206040518083038186803b1580156105a057600080fd5b505afa1580156105b4573d6000803e3d6000fd5b505050506040513d601f19601f820116820180604052508101906102ff9190610964565b60006105e2610545565b6001600160a01b0316639be2a8848484306040518463ffffffff1660e01b815260040161061193929190610aaf565b60206040518083038186803b15801561062957600080fd5b505afa15801561063d573d6000803e3d6000fd5b505050506040513d601f19601f8201168201806040525081019061066191906108fd565b90505b92915050565b7f08c379a0000000000000000000000000000000000000000000000000000000006000908152602060045260076024526642414c23000030600a808404818106603090810160081b95839006959095019082900491820690940160101b939093010160c81b604452606490fd5b60006060836001600160a01b0316836040516106f391906109fc565b6000604051808303816000865af19150503d8060008114610730576040519150601f19603f3d011682016040523d82523d6000602084013e610735565b606091505b5091509150600082141561074d573d6000803e3d6000fd5b61077781516000148061076f57508180602001905181019061076f91906108fd565b6101a261047e565b50505050565b60008083601f84011261078e578182fd5b50813567ffffffffffffffff8111156107a5578182fd5b60208301915083602080830285010111156107bf57600080fd5b9250929050565b803561066481610af5565b6000806000806000606086880312156107e8578081fd5b853567ffffffffffffffff808211156107ff578283fd5b61080b89838a0161077d565b90975095506020880135915080821115610823578283fd5b506108308882890161077d565b909450925050604086013561084481610af5565b809150509295509295909350565b60006020808385031215610864578182fd5b823567ffffffffffffffff8082111561087b578384fd5b818501915085601f83011261088e578384fd5b81358181111561089c578485fd5b83810291506108ac848301610ace565b8181528481019084860184860187018a10156108c6578788fd5b8795505b838610156108f0576108dc8a826107c6565b8352600195909501949186019186016108ca565b5098975050505050505050565b60006020828403121561090e578081fd5b8151801515811461091d578182fd5b9392505050565b600060208284031215610935578081fd5b81357fffffffff000000000000000000000000000000000000000000000000000000008116811461091d578182fd5b600060208284031215610975578081fd5b815161091d81610af5565b600060208284031215610991578081fd5b813561091d81610af5565b6000602082840312156109ad578081fd5b5035919050565b6000602082840312156109c5578081fd5b5051919050565b9182527fffffffff0000000000000000000000000000000000000000000000000000000016602082015260240190565b60008251815b81811015610a1c5760208186018101518583015201610a02565b81811115610a2a5782828501525b509190910192915050565b6001600160a01b0391909116815260200190565b6001600160a01b03929092168252602082015260400190565b6020808252825182820181905260009190848201906040850190845b81811015610a9a57835183529284019291840191600101610a7e565b50909695505050505050565b90815260200190565b9283526001600160a01b03918216602084015216604082015260600190565b60405181810167ffffffffffffffff81118282101715610aed57600080fd5b604052919050565b6001600160a01b038116811461047b57600080fdfea2646970667358221220be72bdf8e7a3c38606c5f954fbe2d77798347aaa1cfb76fe77ec2f6c245d24bc64736f6c63430007010033
    /// ```
    #[rustfmt::skip]
    #[allow(clippy::all)]
    pub static BYTECODE: alloy_sol_types::private::Bytes = alloy_sol_types::private::Bytes::from_static(
        b"a\x01\x80`@R4\x80\x15b\0\0\x12W`\0\x80\xFD[P`@Qb\0n\xD68\x03\x80b\0n\xD6\x839\x81\x01`@\x81\x90Rb\0\x005\x91b\0\x02SV[\x83\x82\x82`@Q\x80`@\x01`@R\x80`\x11\x81R` \x01p\x10\x98[\x18[\x98\xD9\\\x88\x15\x8C\x88\x15\x98][\x1D`z\x1B\x81RP\x80`@Q\x80`@\x01`@R\x80`\x01\x81R` \x01`1`\xF8\x1B\x81RP0`\x01`\x01`\xA0\x1B\x03\x16`\0\x1B\x89\x80`\x01`\x01`\xA0\x1B\x03\x16`\x80\x81`\x01`\x01`\xA0\x1B\x03\x16``\x1B\x81RPPP0`@Qb\0\0\xB8\x90b\0\x02EV[b\0\0\xC4\x91\x90b\0\x02\x9FV[`@Q\x80\x91\x03\x90`\0\xF0\x80\x15\x80\x15b\0\0\xE1W=`\0\x80>=`\0\xFD[P``\x1B`\x01`\x01``\x1B\x03\x19\x16`\xA0R`\x01`\0U`\xC0R\x81Q` \x92\x83\x01 `\xE0R\x80Q\x91\x01 a\x01\0RP\x7F\x8Bs\xC3\xC6\x9B\xB8\xFE=Q.\xCCL\xF7Y\xCCy#\x9F{\x17\x9B\x0F\xFA\xCA\xA9\xA7]R+9@\x0Fa\x01 Rb\0\x01Hbv\xA7\0\x83\x11\x15a\x01\x94b\0\x01\x81V[b\0\x01\\b'\x8D\0\x82\x11\x15a\x01\x95b\0\x01\x81V[B\x90\x91\x01a\x01@\x81\x90R\x01a\x01`Rb\0\x01v\x81b\0\x01\x96V[PPPPPb\0\x02\xCCV[\x81b\0\x01\x92Wb\0\x01\x92\x81b\0\x01\xF2V[PPV[`@Q`\x01`\x01`\xA0\x1B\x03\x82\x16\x90\x7F\x94\xB9y\xB6\x83\x1AQ)>&ABo\x97t\x7F\xEE\xD4o\x17w\x9F\xED\x9C\xD1\x8D\x1E\xCE\xFC\xFE\x92\xEF\x90`\0\x90\xA2`\x03\x80T`\x01`\x01`\xA0\x1B\x03\x90\x92\x16a\x01\0\x02a\x01\0`\x01`\xA8\x1B\x03\x19\x90\x92\x16\x91\x90\x91\x17\x90UV[bF\x1B\xCD`\xE5\x1B`\0\x90\x81R` `\x04R`\x07`$RfBAL#\0\x000`\n\x80\x84\x04\x81\x81\x06`0\x90\x81\x01`\x08\x1B\x95\x83\x90\x06\x95\x90\x95\x01\x90\x82\x90\x04\x91\x82\x06\x90\x94\x01`\x10\x1B\x93\x90\x93\x01\x01`\xC8\x1B`DR`d\x90\xFD[a\x0B\xE6\x80b\0b\xF0\x839\x01\x90V[`\0\x80`\0\x80`\x80\x85\x87\x03\x12\x15b\0\x02iW\x83\x84\xFD[\x84Qb\0\x02v\x81b\0\x02\xB3V[` \x86\x01Q\x90\x94Pb\0\x02\x89\x81b\0\x02\xB3V[`@\x86\x01Q``\x90\x96\x01Q\x94\x97\x90\x96P\x92PPPV[`\x01`\x01`\xA0\x1B\x03\x91\x90\x91\x16\x81R` \x01\x90V[`\x01`\x01`\xA0\x1B\x03\x81\x16\x81\x14b\0\x02\xC9W`\0\x80\xFD[PV[`\x80Q``\x1C`\xA0Q``\x1C`\xC0Q`\xE0Qa\x01\0Qa\x01 Qa\x01@Qa\x01`Qa_\xC0b\0\x030`\09\x80a\x1A\xEDRP\x80a\x1A\xC9RP\x80a(\x9FRP\x80a(\xE1RP\x80a(\xC0RP\x80a\x10\xFDRP\x80a\x13\xB1RP\x80a\x05(RPa_\xC0`\0\xF3\xFE`\x80`@R`\x046\x10a\x01\xA5W`\x005`\xE0\x1C\x80c\x94[\xCE\xC9\x11a\0\xE1W\x80c\xE6\xC4`\x92\x11a\0\x8AW\x80c\xF8M\x06n\x11a\0dW\x80c\xF8M\x06n\x14a\x04\x8AW\x80c\xF9MFh\x14a\x04\xAAW\x80c\xFAng\x1D\x14a\x04\xD9W\x80c\xFE\xC9\rr\x14a\x04\xF9Wa\x01\xD3V[\x80c\xE6\xC4`\x92\x14a\x04'W\x80c\xED$\x91\x1D\x14a\x04GW\x80c\xF6\xC0\t'\x14a\x04\\Wa\x01\xD3V[\x80c\xB0_\x8EH\x11a\0\xBBW\x80c\xB0_\x8EH\x14a\x03\xCFW\x80c\xB9\\\xAC(\x14a\x03\xFFW\x80c\xD2\x94l+\x14a\x04\x12Wa\x01\xD3V[\x80c\x94[\xCE\xC9\x14a\x03\x85W\x80c\xAA\xAB\xAD\xC5\x14a\x03\x98W\x80c\xAD\\FH\x14a\x03\xBAWa\x01\xD3V[\x80cR\xBB\xBE)\x11a\x01NW\x80c}:\xEB\x96\x11a\x01(W\x80c}:\xEB\x96\x14a\x03\x05W\x80c\x85\x1C\x1B\xB3\x14a\x03%W\x80c\x8B\xDB9\x13\x14a\x03EW\x80c\x90\x19;|\x14a\x03eWa\x01\xD3V[\x80cR\xBB\xBE)\x14a\x02\xB2W\x80c\\8D\x9E\x14a\x02\xC5W\x80cf\xA9\xC7\xD2\x14a\x02\xE5Wa\x01\xD3V[\x80c\x0FZn\xFA\x11a\x01\x7FW\x80c\x0FZn\xFA\x14a\x02AW\x80c\x16\xC3\x8B<\x14a\x02nW\x80c\x1C\r\xE0Q\x14a\x02\x8EWa\x01\xD3V[\x80c\x05\x8Ab\x8F\x14a\x01\xD8W\x80c\t\xB2v\x0F\x14a\x01\xF8W\x80c\x0E\x8E>\x84\x14a\x02.Wa\x01\xD3V[6a\x01\xD3Wa\x01\xD1a\x01\xB5a\x05&V[`\x01`\x01`\xA0\x1B\x03\x163`\x01`\x01`\xA0\x1B\x03\x16\x14a\x02\x06a\x05KV[\0[`\0\x80\xFD[4\x80\x15a\x01\xE4W`\0\x80\xFD[Pa\x01\xD1a\x01\xF36`\x04aQWV[a\x05]V[4\x80\x15a\x02\x04W`\0\x80\xFD[Pa\x02\x18a\x02\x136`\x04aV\xE6V[a\x05\x81V[`@Qa\x02%\x91\x90a]>V[`@Q\x80\x91\x03\x90\xF3[a\x01\xD1a\x02<6`\x04aS\x1EV[a\x064V[4\x80\x15a\x02MW`\0\x80\xFD[Pa\x02aa\x02\\6`\x04aQ\xF5V[a\x07pV[`@Qa\x02%\x91\x90a]\x08V[4\x80\x15a\x02zW`\0\x80\xFD[Pa\x01\xD1a\x02\x896`\x04aT\\V[a\x08\x06V[4\x80\x15a\x02\x9AW`\0\x80\xFD[Pa\x02\xA3a\x08\x1FV[`@Qa\x02%\x93\x92\x91\x90a]&V[a\x02\x18a\x02\xC06`\x04aX\x8FV[a\x08HV[4\x80\x15a\x02\xD1W`\0\x80\xFD[Pa\x01\xD1a\x02\xE06`\x04aV[V[a\t\xE9V[4\x80\x15a\x02\xF1W`\0\x80\xFD[Pa\x01\xD1a\x03\x006`\x04aUEV[a\x0E\x06V[4\x80\x15a\x03\x11W`\0\x80\xFD[Pa\x01\xD1a\x03 6`\x04aU\x16V[a\x0F\xA5V[4\x80\x15a\x031W`\0\x80\xFD[Pa\x02\x18a\x03@6`\x04aV3V[a\x10\xF9V[4\x80\x15a\x03QW`\0\x80\xFD[Pa\x01\xD1a\x03`6`\x04aT\xACV[a\x11KV[4\x80\x15a\x03qW`\0\x80\xFD[Pa\x02\x18a\x03\x806`\x04aQWV[a\x11aV[a\x02aa\x03\x936`\x04aW\x86V[a\x11|V[4\x80\x15a\x03\xA4W`\0\x80\xFD[Pa\x03\xADa\x12\xB0V[`@Qa\x02%\x91\x90a[cV[4\x80\x15a\x03\xC6W`\0\x80\xFD[Pa\x03\xADa\x12\xC4V[4\x80\x15a\x03\xDBW`\0\x80\xFD[Pa\x03\xEFa\x03\xEA6`\x04aV\x0FV[a\x12\xD3V[`@Qa\x02%\x94\x93\x92\x91\x90a^\xB9V[a\x01\xD1a\x04\r6`\x04aT\xACV[a\x13\x96V[4\x80\x15a\x04\x1EW`\0\x80\xFD[Pa\x03\xADa\x13\xAFV[4\x80\x15a\x043W`\0\x80\xFD[Pa\x01\xD1a\x04B6`\x04aRCV[a\x13\xD3V[4\x80\x15a\x04SW`\0\x80\xFD[Pa\x02\x18a\x14\xEFV[4\x80\x15a\x04hW`\0\x80\xFD[Pa\x04|a\x04w6`\x04aT\x94V[a\x14\xF9V[`@Qa\x02%\x92\x91\x90a[\x9BV[4\x80\x15a\x04\x96W`\0\x80\xFD[Pa\x02aa\x04\xA56`\x04aW\x02V[a\x15#V[4\x80\x15a\x04\xB6W`\0\x80\xFD[Pa\x04\xCAa\x04\xC56`\x04aT\x94V[a\x16 V[`@Qa\x02%\x93\x92\x91\x90a\\\xD2V[4\x80\x15a\x04\xE5W`\0\x80\xFD[Pa\x01\xD1a\x04\xF46`\x04aQ\xABV[a\x16TV[4\x80\x15a\x05\x05W`\0\x80\xFD[Pa\x05\x19a\x05\x146`\x04aQsV[a\x16\xE6V[`@Qa\x02%\x91\x90a]\x1BV[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0[\x90V[\x81a\x05YWa\x05Y\x81a\x16\xFBV[PPV[a\x05ea\x17hV[a\x05ma\x17\x81V[a\x05v\x81a\x17\xAFV[a\x05~a\x18\"V[PV[`\0a\x05\x8Ba\x17hV[a\x05\x93a\x18)V[`\0a\x05\xA23\x84`\x06Ta\x18>V[`\0\x81\x81R`\x05` R`@\x90 T\x90\x91Pa\x05\xC4\x90`\xFF\x16\x15a\x01\xF4a\x05KV[`\0\x81\x81R`\x05` R`@\x90\x81\x90 \x80T`\xFF\x19\x16`\x01\x90\x81\x17\x90\x91U`\x06\x80T\x90\x91\x01\x90UQ3\x90\x82\x90\x7F<\x13\xBC0\xB8\xE8x\xC5?\xD2\xA3kg\x94\t\xC0s\xAF\xD7YP\xBEC\xD8\x85\x87h\xE9V\xFB\xC2\x0E\x90a\x06\x1D\x90\x87\x90a^:V[`@Q\x80\x91\x03\x90\xA3\x90Pa\x06/a\x18\"V[\x91\x90PV[a\x06<a\x17hV[`\0\x80`\0\x80[\x84Q\x81\x10\x15a\x07[W`\0\x80`\0\x80`\0a\x06q\x8A\x87\x81Q\x81\x10a\x06cW\xFE[` \x02` \x01\x01Q\x89a\x18}V[\x9CP\x93\x98P\x91\x96P\x94P\x92P\x90P`\x01\x85`\x03\x81\x11\x15a\x06\x8DW\xFE[\x14\x15a\x06\xA4Wa\x06\x9F\x84\x83\x83\x86a\x18\xF5V[a\x07JV[\x86a\x06\xB6Wa\x06\xB1a\x18)V[`\x01\x96P[`\0\x85`\x03\x81\x11\x15a\x06\xC4W\xFE[\x14\x15a\x06\xF5Wa\x06\xD6\x84\x83\x83\x86a\x19\x18V[a\x06\xDF\x84a\x198V[\x15a\x06\x9FWa\x06\xEE\x89\x84a\x19EV[\x98Pa\x07JV[a\x07\na\x07\x01\x85a\x198V[\x15a\x02\x07a\x05KV[`\0a\x07\x15\x85a\x05HV[\x90P`\x02\x86`\x03\x81\x11\x15a\x07%W\xFE[\x14\x15a\x07<Wa\x077\x81\x84\x84\x87a\x19WV[a\x07HV[a\x07H\x81\x84\x84\x87a\x19pV[P[PP`\x01\x90\x93\x01\x92Pa\x06C\x91PPV[Pa\x07e\x83a\x19\xDEV[PPPa\x05~a\x18\"V[``\x81Qg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x80\x15a\x07\x8AW`\0\x80\xFD[P`@Q\x90\x80\x82R\x80` \x02` \x01\x82\x01`@R\x80\x15a\x07\xB4W\x81` \x01` \x82\x02\x806\x837\x01\x90P[P\x90P`\0[\x82Q\x81\x10\x15a\x07\xFFWa\x07\xE0\x84\x84\x83\x81Q\x81\x10a\x07\xD3W\xFE[` \x02` \x01\x01Qa\x1A\x01V[\x82\x82\x81Q\x81\x10a\x07\xECW\xFE[` \x90\x81\x02\x91\x90\x91\x01\x01R`\x01\x01a\x07\xBAV[P\x92\x91PPV[a\x08\x0Ea\x17hV[a\x08\x16a\x17\x81V[a\x05v\x81a\x1A,V[`\0\x80`\0a\x08,a\x1A\xAAV[\x15\x92Pa\x087a\x1A\xC7V[\x91Pa\x08Aa\x1A\xEBV[\x90P\x90\x91\x92V[`\0a\x08Ra\x17hV[a\x08Za\x18)V[\x83Qa\x08e\x81a\x1B\x0FV[a\x08t\x83B\x11\x15a\x01\xFCa\x05KV[a\x08\x87`\0\x87`\x80\x01Q\x11a\x01\xFEa\x05KV[`\0a\x08\x96\x87`@\x01Qa\x1BAV[\x90P`\0a\x08\xA7\x88``\x01Qa\x1BAV[\x90Pa\x08\xCA\x81`\x01`\x01`\xA0\x1B\x03\x16\x83`\x01`\x01`\xA0\x1B\x03\x16\x14\x15a\x01\xFDa\x05KV[a\x08\xD2aL\xE1V[\x88Q`\x80\x82\x01R` \x89\x01Q\x81\x90`\x01\x81\x11\x15a\x08\xEBW\xFE[\x90\x81`\x01\x81\x11\x15a\x08\xF8W\xFE[\x90RP`\x01`\x01`\xA0\x1B\x03\x80\x84\x16` \x83\x01R\x82\x81\x16`@\x80\x84\x01\x91\x90\x91R`\x80\x8B\x01Q``\x84\x01R`\xA0\x8B\x01Qa\x01\0\x84\x01R\x89Q\x82\x16`\xC0\x84\x01R\x89\x01Q\x16`\xE0\x82\x01R`\0\x80a\tJ\x83a\x1BfV[\x91\x98P\x92P\x90Pa\t\x81`\0\x8C` \x01Q`\x01\x81\x11\x15a\tfW\xFE[\x14a\ttW\x89\x83\x11\x15a\tyV[\x89\x82\x10\x15[a\x01\xFBa\x05KV[a\t\x99\x8B`@\x01Q\x83\x8C`\0\x01Q\x8D` \x01Qa\x1CZV[a\t\xB1\x8B``\x01Q\x82\x8C`@\x01Q\x8D``\x01Qa\x1D8V[a\t\xD3a\t\xC1\x8C`@\x01Qa\x198V[a\t\xCCW`\0a\t\xCEV[\x82[a\x19\xDEV[PPPPPPa\t\xE1a\x18\"V[\x94\x93PPPPV[a\t\xF1a\x17hV[a\t\xF9a\x18)V[a\n\x05\x83Q\x83Qa\x1E\x12V[``\x83Qg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x80\x15a\n\x1FW`\0\x80\xFD[P`@Q\x90\x80\x82R\x80` \x02` \x01\x82\x01`@R\x80\x15a\nIW\x81` \x01` \x82\x02\x806\x837\x01\x90P[P\x90P``\x84Qg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x80\x15a\nfW`\0\x80\xFD[P`@Q\x90\x80\x82R\x80` \x02` \x01\x82\x01`@R\x80\x15a\n\x90W\x81` \x01` \x82\x02\x806\x837\x01\x90P[P\x90P`\0\x80[\x86Q\x81\x10\x15a\x0C\tW`\0\x87\x82\x81Q\x81\x10a\n\xAEW\xFE[` \x02` \x01\x01Q\x90P`\0\x87\x83\x81Q\x81\x10a\n\xC6W\xFE[` \x02` \x01\x01Q\x90Pa\x0B\x11\x84`\x01`\x01`\xA0\x1B\x03\x16\x83`\x01`\x01`\xA0\x1B\x03\x16\x11`\0`\x01`\x01`\xA0\x1B\x03\x16\x84`\x01`\x01`\xA0\x1B\x03\x16\x14a\x0B\tW`fa\x0B\x0CV[`h[a\x05KV[\x81\x93P\x81`\x01`\x01`\xA0\x1B\x03\x16cp\xA0\x8210`@Q\x82c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01a\x0B@\x91\x90a[cV[` `@Q\x80\x83\x03\x81\x86\x80;\x15\x80\x15a\x0BXW`\0\x80\xFD[PZ\xFA\x15\x80\x15a\x0BlW=`\0\x80>=`\0\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x0B\x90\x91\x90aYhV[\x85\x84\x81Q\x81\x10a\x0B\x9CW\xFE[` \x02` \x01\x01\x81\x81RPPa\x0B\xB1\x81a\x1E\x1FV[\x86\x84\x81Q\x81\x10a\x0B\xBDW\xFE[` \x02` \x01\x01\x81\x81RPPa\x0B\xEB\x81\x86\x85\x81Q\x81\x10a\x0B\xD9W\xFE[` \x02` \x01\x01Q\x10\x15a\x02\x10a\x05KV[a\x0B\xFF`\x01`\x01`\xA0\x1B\x03\x83\x16\x8B\x83a\x1E\xA6V[PP`\x01\x01a\n\x97V[P`@Q\x7F\xF0O'\x07\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x88\x16\x90c\xF0O'\x07\x90a\x0CU\x90\x89\x90\x89\x90\x88\x90\x8A\x90`\x04\x01a\\\x85V[`\0`@Q\x80\x83\x03\x81`\0\x87\x80;\x15\x80\x15a\x0CoW`\0\x80\xFD[PZ\xF1\x15\x80\x15a\x0C\x83W=`\0\x80>=`\0\xFD[PPPP`\0[\x86Q\x81\x10\x15a\r\xF4W`\0\x87\x82\x81Q\x81\x10a\x0C\xA1W\xFE[` \x02` \x01\x01Q\x90P`\0\x84\x83\x81Q\x81\x10a\x0C\xB9W\xFE[` \x02` \x01\x01Q\x90P`\0\x82`\x01`\x01`\xA0\x1B\x03\x16cp\xA0\x8210`@Q\x82c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01a\x0C\xF1\x91\x90a[cV[` `@Q\x80\x83\x03\x81\x86\x80;\x15\x80\x15a\r\tW`\0\x80\xFD[PZ\xFA\x15\x80\x15a\r\x1DW=`\0\x80>=`\0\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\rA\x91\x90aYhV[\x90Pa\rR\x82\x82\x10\x15a\x02\x03a\x05KV[`\0\x82\x82\x03\x90Pa\r{\x88\x86\x81Q\x81\x10a\rhW\xFE[` \x02` \x01\x01Q\x82\x10\x15a\x02Za\x05KV[a\r\x85\x84\x82a\x1F\x11V[\x83`\x01`\x01`\xA0\x1B\x03\x16\x8C`\x01`\x01`\xA0\x1B\x03\x16\x7F\r}u\xE0\x1A\xB9W\x80\xD3\xCD\x1C\x8E\xC0\xDDl,\xE1\x9E: B~\xEC\x8B\xF52\x83\xB6\xFB\x8E\x95\xF0\x8C\x88\x81Q\x81\x10a\r\xC6W\xFE[` \x02` \x01\x01Q\x84`@Qa\r\xDD\x92\x91\x90a^MV[`@Q\x80\x91\x03\x90\xA3PPPP\x80`\x01\x01\x90Pa\x0C\x8AV[PPPPa\x0E\0a\x18\"V[PPPPV[a\x0E\x0Ea\x17hV[a\x0E\x16a\x18)V[\x82a\x0E \x81a\x1F3V[a\x0E,\x83Q\x83Qa\x1E\x12V[`\0[\x83Q\x81\x10\x15a\x0E\xCAW`\0\x84\x82\x81Q\x81\x10a\x0EFW\xFE[` \x02` \x01\x01Q\x90Pa\x0Er`\0`\x01`\x01`\xA0\x1B\x03\x16\x82`\x01`\x01`\xA0\x1B\x03\x16\x14\x15a\x015a\x05KV[\x83\x82\x81Q\x81\x10a\x0E~W\xFE[` \x90\x81\x02\x91\x90\x91\x01\x81\x01Q`\0\x88\x81R`\n\x83R`@\x80\x82 `\x01`\x01`\xA0\x1B\x03\x95\x86\x16\x83R\x90\x93R\x91\x90\x91 \x80T`\x01`\x01`\xA0\x1B\x03\x19\x16\x92\x90\x91\x16\x91\x90\x91\x17\x90U`\x01\x01a\x0E/V[P`\0a\x0E\xD6\x85a\x1FdV[\x90P`\x02\x81`\x02\x81\x11\x15a\x0E\xE6W\xFE[\x14\x15a\x0F4Wa\x0E\xFC\x84Q`\x02\x14a\x02\x0Ca\x05KV[a\x0F/\x85\x85`\0\x81Q\x81\x10a\x0F\rW\xFE[` \x02` \x01\x01Q\x86`\x01\x81Q\x81\x10a\x0F\"W\xFE[` \x02` \x01\x01Qa\x1F~V[a\x0F\\V[`\x01\x81`\x02\x81\x11\x15a\x0FBW\xFE[\x14\x15a\x0FRWa\x0F/\x85\x85a *V[a\x0F\\\x85\x85a \x82V[\x84\x7F\xF5\x84}?!\x97\xB1l\xDC\xD2\t\x8E\xC9]\t\x05\xCD\x1A\xBD\xAFA_\x07\xBB|\xEF+\xBA\x8A\xC5\xDE\xC4\x85\x85`@Qa\x0F\x8E\x92\x91\x90a[\xEDV[`@Q\x80\x91\x03\x90\xA2PPa\x0F\xA0a\x18\"V[PPPV[a\x0F\xADa\x17hV[a\x0F\xB5a\x18)V[\x81a\x0F\xBF\x81a\x1F3V[`\0a\x0F\xCA\x84a\x1FdV[\x90P`\x02\x81`\x02\x81\x11\x15a\x0F\xDAW\xFE[\x14\x15a\x10(Wa\x0F\xF0\x83Q`\x02\x14a\x02\x0Ca\x05KV[a\x10#\x84\x84`\0\x81Q\x81\x10a\x10\x01W\xFE[` \x02` \x01\x01Q\x85`\x01\x81Q\x81\x10a\x10\x16W\xFE[` \x02` \x01\x01Qa \xD7V[a\x10PV[`\x01\x81`\x02\x81\x11\x15a\x106W\xFE[\x14\x15a\x10FWa\x10#\x84\x84a!EV[a\x10P\x84\x84a!\xFFV[`\0[\x83Q\x81\x10\x15a\x10\xB6W`\n`\0\x86\x81R` \x01\x90\x81R` \x01`\0 `\0\x85\x83\x81Q\x81\x10a\x10}W\xFE[` \x90\x81\x02\x91\x90\x91\x01\x81\x01Q`\x01`\x01`\xA0\x1B\x03\x16\x82R\x81\x01\x91\x90\x91R`@\x01`\0 \x80T`\x01`\x01`\xA0\x1B\x03\x19\x16\x90U`\x01\x01a\x10SV[P\x83\x7F}\xCD\xC6\xD0.\xF4\x0C|\x1ApF\xA0\x11\xB0X\xBD\x7F\x98\x8F\xA1N \xA6cD\xF9\xD4\xE6\x06W\xD6\x10\x84`@Qa\x10\xE7\x91\x90a[\xDAV[`@Q\x80\x91\x03\x90\xA2PPa\x05Ya\x18\"V[`\0\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x82`@Q` \x01a\x11.\x92\x91\x90aZ\xC2V[`@Q` \x81\x83\x03\x03\x81R\x90`@R\x80Q\x90` \x01 \x90P\x91\x90PV[a\x0E\0`\x01\x85\x85\x85a\x11\\\x86a\"bV[a\"nV[`\x01`\x01`\xA0\x1B\x03\x16`\0\x90\x81R`\x02` R`@\x90 T\x90V[``a\x11\x86a\x17hV[a\x11\x8Ea\x18)V[\x83Qa\x11\x99\x81a\x1B\x0FV[a\x11\xA8\x83B\x11\x15a\x01\xFCa\x05KV[a\x11\xB4\x86Q\x85Qa\x1E\x12V[a\x11\xC0\x87\x87\x87\x8Ba#\xF4V[\x91P`\0\x80[\x87Q\x81\x10\x15a\x12\x92W`\0\x88\x82\x81Q\x81\x10a\x11\xDDW\xFE[` \x02` \x01\x01Q\x90P`\0\x85\x83\x81Q\x81\x10a\x11\xF5W\xFE[` \x02` \x01\x01Q\x90Pa\x12!\x88\x84\x81Q\x81\x10a\x12\x0EW\xFE[` \x02` \x01\x01Q\x82\x13\x15a\x01\xFBa\x05KV[`\0\x81\x13\x15a\x12aW\x88Q` \x8A\x01Q\x82\x91a\x12@\x91\x85\x91\x84\x91a\x1CZV[a\x12I\x83a\x198V[\x15a\x12[Wa\x12X\x85\x82a\x19EV[\x94P[Pa\x12\x88V[`\0\x81\x12\x15a\x12\x88W`\0\x81`\0\x03\x90Pa\x12\x86\x83\x82\x8C`@\x01Q\x8D``\x01Qa\x1D8V[P[PP`\x01\x01a\x11\xC6V[Pa\x12\x9C\x81a\x19\xDEV[PPa\x12\xA6a\x18\"V[\x96\x95PPPPPPV[`\x03Ta\x01\0\x90\x04`\x01`\x01`\xA0\x1B\x03\x16\x90V[`\0a\x12\xCEa\x05&V[\x90P\x90V[`\0\x80`\0\x80\x85a\x12\xE3\x81a&\x83V[`\0\x80a\x12\xEF\x89a\x1FdV[\x90P`\x02\x81`\x02\x81\x11\x15a\x12\xFFW\xFE[\x14\x15a\x13\x16Wa\x13\x0F\x89\x89a&\xA1V[\x91Pa\x13AV[`\x01\x81`\x02\x81\x11\x15a\x13$W\xFE[\x14\x15a\x134Wa\x13\x0F\x89\x89a'\x1BV[a\x13>\x89\x89a'\x89V[\x91P[a\x13J\x82a'\xA1V[\x96Pa\x13U\x82a'\xB4V[\x95Pa\x13`\x82a'\xCAV[`\0\x99\x8AR`\n` \x90\x81R`@\x80\x8C `\x01`\x01`\xA0\x1B\x03\x9B\x8C\x16\x8DR\x90\x91R\x90\x99 T\x96\x99\x95\x98\x97\x96\x90\x96\x16\x95PPPPPV[a\x13\x9Ea\x18)V[a\x0E\0`\0\x85\x85\x85a\x11\\\x86a\"bV[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90V[a\x13\xDBa\x17hV[a\x13\xE3a\x18)V[a\x13\xEBaM1V[`\0[\x82Q\x81\x10\x15a\x14\xE5W\x82\x81\x81Q\x81\x10a\x14\x03W\xFE[` \x02` \x01\x01Q\x91P`\0\x82` \x01Q\x90Pa\x14\x1F\x81a&\x83V[`@\x83\x01Qa\x149a\x141\x83\x83a'\xD0V[a\x02\ta\x05KV[`\0\x82\x81R`\n` \x90\x81R`@\x80\x83 `\x01`\x01`\xA0\x1B\x03\x85\x81\x16\x85R\x92R\x90\x91 Ta\x14l\x91\x163\x14a\x01\xF6a\x05KV[\x83Q``\x85\x01Q`\0\x80a\x14\x82\x84\x87\x87\x86a(,V[\x91P\x91P\x84`\x01`\x01`\xA0\x1B\x03\x163`\x01`\x01`\xA0\x1B\x03\x16\x87\x7Fn\xDC\xAFbA\x10[L\x94\xC2\xEF\xDB\xF3\xA6\xB1$X\xEB=\x07\xBE:\x0E\x81\xD2K\x13\xC4@E\xFEz\x85\x85`@Qa\x14\xCC\x92\x91\x90a^MV[`@Q\x80\x91\x03\x90\xA4PPPPPP\x80`\x01\x01\x90Pa\x13\xEEV[PPa\x05~a\x18\"V[`\0a\x12\xCEa(\x9BV[`\0\x80\x82a\x15\x06\x81a&\x83V[a\x15\x0F\x84a)8V[a\x15\x18\x85a\x1FdV[\x92P\x92P[P\x91P\x91V[``30\x14a\x15\xF6W`\x000`\x01`\x01`\xA0\x1B\x03\x16`\x006`@Qa\x15I\x92\x91\x90aZ\xDAV[`\0`@Q\x80\x83\x03\x81`\0\x86Z\xF1\x91PP=\x80`\0\x81\x14a\x15\x86W`@Q\x91P`\x1F\x19`?=\x01\x16\x82\x01`@R=\x82R=`\0` \x84\x01>a\x15\x8BV[``\x91P[PP\x90P\x80`\0\x81\x14a\x15\x9AW\xFE[`\x04`\0\x80>`\0Q`\x01`\x01`\xE0\x1B\x03\x19\x16\x7F\xFAa\xCC\x12\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81\x14a\x15\xDEW=`\0\x80>=`\0\xFD[P` `\0R`\x04=\x03\x80`\x04` >` \x81\x01`\0\xF3[``a\x16\x04\x85\x85\x85\x89a#\xF4V[\x90P` \x81Q\x02c\xFAa\xCC\x12` \x83\x03R`\x04\x82\x03`$\x82\x01\x81\xFD[``\x80`\0\x83a\x16/\x81a&\x83V[``a\x16:\x86a)>V[\x90\x95P\x90Pa\x16H\x81a)\xA0V[\x95\x97\x90\x96P\x93PPPPV[a\x16\\a\x17hV[a\x16da\x18)V[\x82a\x16n\x81a\x1B\x0FV[`\x01`\x01`\xA0\x1B\x03\x84\x81\x16`\0\x81\x81R`\x04` \x90\x81R`@\x80\x83 \x94\x88\x16\x80\x84R\x94\x90\x91R\x90\x81\x90 \x80T`\xFF\x19\x16\x86\x15\x15\x17\x90UQ\x90\x91\x90\x7FF\x96\x1F\xDBE\x02\xB6F\xD5\t_\xBAv\0Hj\x8A\xC0PA\xD5\\\xDF\x0F\x16\xEDgq\x80\xB5\xCA\xD8\x90a\x16\xD5\x90\x86\x90a]\x1BV[`@Q\x80\x91\x03\x90\xA3Pa\x0F\xA0a\x18\"V[`\0a\x16\xF2\x83\x83a*OV[\x90P[\x92\x91PPV[\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\0\x90\x81R` `\x04R`\x07`$RfBAL#\0\x000`\n\x80\x84\x04\x81\x81\x06`0\x90\x81\x01`\x08\x1B\x95\x83\x90\x06\x95\x90\x95\x01\x90\x82\x90\x04\x91\x82\x06\x90\x94\x01`\x10\x1B\x93\x90\x93\x01\x01`\xC8\x1B`DR`d\x90\xFD[a\x17z`\x02`\0T\x14\x15a\x01\x90a\x05KV[`\x02`\0UV[`\0a\x17\x98`\x005`\x01`\x01`\xE0\x1B\x03\x19\x16a\x10\xF9V[\x90Pa\x05~a\x17\xA7\x823a*}V[a\x01\x91a\x05KV[`@Q`\x01`\x01`\xA0\x1B\x03\x82\x16\x90\x7F\x94\xB9y\xB6\x83\x1AQ)>&ABo\x97t\x7F\xEE\xD4o\x17w\x9F\xED\x9C\xD1\x8D\x1E\xCE\xFC\xFE\x92\xEF\x90`\0\x90\xA2`\x03\x80T`\x01`\x01`\xA0\x1B\x03\x90\x92\x16a\x01\0\x02\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\xFF\x90\x92\x16\x91\x90\x91\x17\x90UV[`\x01`\0UV[a\x18<a\x184a\x1A\xAAV[a\x01\x92a\x05KV[V[`\0i\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x16`P\x84`\x02\x81\x11\x15a\x18[W\xFE[\x90\x1B\x17``\x85\x90\x1Bk\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x19\x16\x17\x90P[\x93\x92PPPV[`\0\x80`\0\x80`\0\x80`\0\x88``\x01Q\x90P3`\x01`\x01`\xA0\x1B\x03\x16\x81`\x01`\x01`\xA0\x1B\x03\x16\x14a\x18\xCFW\x87a\x18\xBAWa\x18\xB5a\x17\x81V[`\x01\x97P[a\x18\xCFa\x18\xC7\x823a*OV[a\x01\xF7a\x05KV[\x88Q` \x8A\x01Q`@\x8B\x01Q`\x80\x90\x9B\x01Q\x91\x9B\x90\x9A\x99\x92\x98P\x90\x96P\x90\x94P\x92PPPV[a\x19\n\x83a\x19\x02\x86a\x1BAV[\x83`\0a+ V[Pa\x0E\0\x84\x82\x84`\0a\x1D8V[a\x19+\x82a\x19%\x86a\x1BAV[\x83a+vV[a\x0E\0\x84\x82\x85`\0a\x1CZV[`\x01`\x01`\xA0\x1B\x03\x16\x15\x90V[`\0\x82\x82\x01a\x16\xF2\x84\x82\x10\x15\x83a\x05KV[a\x19d\x83\x85\x83`\0a+ V[Pa\x0E\0\x82\x85\x83a+vV[\x80\x15a\x0E\0Wa\x19\x8B`\x01`\x01`\xA0\x1B\x03\x85\x16\x84\x84\x84a+\xA6V[\x82`\x01`\x01`\xA0\x1B\x03\x16\x84`\x01`\x01`\xA0\x1B\x03\x16\x7FT\n\x1A?(4\x0C\xAE\xC36\xC8\x1D\x8D{=\xF19\xEE\\\xDC\x189\xA4\xF2\x83\xD7\xEB\xB7\xEA\xAE-\\\x84\x84`@Qa\x19\xD0\x92\x91\x90a[\xC1V[`@Q\x80\x91\x03\x90\xA3PPPPV[a\x19\xED\x814\x10\x15a\x02\x04a\x05KV[4\x81\x90\x03\x80\x15a\x05YWa\x05Y3\x82a+\xC7V[`\x01`\x01`\xA0\x1B\x03\x91\x82\x16`\0\x90\x81R`\x0B` \x90\x81R`@\x80\x83 \x93\x90\x94\x16\x82R\x91\x90\x91R T\x90V[\x80\x15a\x1ALWa\x1AGa\x1A=a\x1A\xC7V[B\x10a\x01\x93a\x05KV[a\x1AaV[a\x1Aaa\x1AWa\x1A\xEBV[B\x10a\x01\xA9a\x05KV[`\x03\x80T`\xFF\x19\x16\x82\x15\x15\x17\x90U`@Q\x7F\x9E:^7\"E2\xDE\xA6{\x89\xFA\xCE\x18W\x03s\x8A\"\x8An\x8A#\xDE\xE5F\x96\x01\x80\xD3\xBEd\x90a\x1A\x9F\x90\x83\x90a]\x1BV[`@Q\x80\x91\x03\x90\xA1PV[`\0a\x1A\xB4a\x1A\xEBV[B\x11\x80a\x12\xCEWPP`\x03T`\xFF\x16\x15\x90V[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90V[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90V[3`\x01`\x01`\xA0\x1B\x03\x82\x16\x14a\x05~Wa\x1B'a\x17\x81V[a\x1B1\x813a*OV[a\x05~Wa\x05~\x81a\x01\xF7a,AV[`\0a\x1BL\x82a\x198V[a\x1B^Wa\x1BY\x82a\x05HV[a\x16\xF5V[a\x16\xF5a\x05&V[`\0\x80`\0\x80a\x1By\x85`\x80\x01Qa)8V[\x90P`\0a\x1B\x8A\x86`\x80\x01Qa\x1FdV[\x90P`\x02\x81`\x02\x81\x11\x15a\x1B\x9AW\xFE[\x14\x15a\x1B\xB1Wa\x1B\xAA\x86\x83a,uV[\x94Pa\x1B\xDCV[`\x01\x81`\x02\x81\x11\x15a\x1B\xBFW\xFE[\x14\x15a\x1B\xCFWa\x1B\xAA\x86\x83a-%V[a\x1B\xD9\x86\x83a-\xB8V[\x94P[a\x1B\xEF\x86`\0\x01Q\x87``\x01Q\x87a/\xF7V[\x80\x94P\x81\x95PPP\x85`@\x01Q`\x01`\x01`\xA0\x1B\x03\x16\x86` \x01Q`\x01`\x01`\xA0\x1B\x03\x16\x87`\x80\x01Q\x7F!p\xC7A\xC4\x151\xAE\xC2\x0E|\x10|$\xEE\xCF\xDD\x15\xE6\x9C\x9B\xB0\xA8\xDD7\xB1\x84\x0B\x9E\x0B {\x87\x87`@Qa\x1CI\x92\x91\x90a^MV[`@Q\x80\x91\x03\x90\xA4PP\x91\x93\x90\x92PV[\x82a\x1CdWa\x0E\0V[a\x1Cm\x84a\x198V[\x15a\x1C\xEEWa\x1C\x7F\x81\x15a\x02\x02a\x05KV[a\x1C\x8E\x83G\x10\x15a\x02\x04a\x05KV[a\x1C\x96a\x05&V[`\x01`\x01`\xA0\x1B\x03\x16c\xD0\xE3\r\xB0\x84`@Q\x82c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01`\0`@Q\x80\x83\x03\x81\x85\x88\x80;\x15\x80\x15a\x1C\xD0W`\0\x80\xFD[PZ\xF1\x15\x80\x15a\x1C\xE4W=`\0\x80>=`\0\xFD[PPPPPa\x0E\0V[`\0a\x1C\xF9\x85a\x05HV[\x90P\x81\x15a\x1D\x16W`\0a\x1D\x10\x84\x83\x87`\x01a+ V[\x90\x94\x03\x93P[\x83\x15a\x1D1Wa\x1D1`\x01`\x01`\xA0\x1B\x03\x82\x16\x840\x87a+\xA6V[PPPPPV[\x82a\x1DBWa\x0E\0V[a\x1DK\x84a\x198V[\x15a\x1D\xDBWa\x1D]\x81\x15a\x02\x02a\x05KV[a\x1Dea\x05&V[`\x01`\x01`\xA0\x1B\x03\x16c.\x1A}M\x84`@Q\x82c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01a\x1D\x90\x91\x90a]>V[`\0`@Q\x80\x83\x03\x81`\0\x87\x80;\x15\x80\x15a\x1D\xAAW`\0\x80\xFD[PZ\xF1\x15\x80\x15a\x1D\xBEW=`\0\x80>=`\0\xFD[Pa\x1D\xD6\x92PPP`\x01`\x01`\xA0\x1B\x03\x83\x16\x84a+\xC7V[a\x0E\0V[`\0a\x1D\xE6\x85a\x05HV[\x90P\x81\x15a\x1D\xFEWa\x1D\xF9\x83\x82\x86a+vV[a\x1D1V[a\x1D1`\x01`\x01`\xA0\x1B\x03\x82\x16\x84\x86a\x1E\xA6V[a\x05Y\x81\x83\x14`ga\x05KV[`\0\x80a\x1E*a\x13\xAFV[`\x01`\x01`\xA0\x1B\x03\x16c\xD8w\x84\\`@Q\x81c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01` `@Q\x80\x83\x03\x81\x86\x80;\x15\x80\x15a\x1EbW`\0\x80\xFD[PZ\xFA\x15\x80\x15a\x1EvW=`\0\x80>=`\0\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x1E\x9A\x91\x90aYhV[\x90Pa\x18v\x83\x82a0%V[a\x0F\xA0\x83c\xA9\x05\x9C\xBB`\xE0\x1B\x84\x84`@Q`$\x01a\x1E\xC5\x92\x91\x90a[\xC1V[`@\x80Q`\x1F\x19\x81\x84\x03\x01\x81R\x91\x90R` \x81\x01\x80Q{\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16`\x01`\x01`\xE0\x1B\x03\x19\x90\x93\x16\x92\x90\x92\x17\x90\x91Ra0rV[\x80\x15a\x05YWa\x05Ya\x1F\"a\x13\xAFV[`\x01`\x01`\xA0\x1B\x03\x84\x16\x90\x83a\x1E\xA6V[a\x1F<\x81a&\x83V[a\x05~a\x1FH\x82a)8V[`\x01`\x01`\xA0\x1B\x03\x163`\x01`\x01`\xA0\x1B\x03\x16\x14a\x01\xF5a\x05KV[`\0a\xFF\xFF`P\x83\x90\x1C\x16a\x16\xF5`\x03\x82\x10a\x01\xF4a\x05KV[a\x1F\x9F\x81`\x01`\x01`\xA0\x1B\x03\x16\x83`\x01`\x01`\xA0\x1B\x03\x16\x14\x15a\x02\na\x05KV[a\x1F\xBE\x81`\x01`\x01`\xA0\x1B\x03\x16\x83`\x01`\x01`\xA0\x1B\x03\x16\x10`fa\x05KV[`\0\x83\x81R`\t` R`@\x90 \x80Ta\x1F\xFB\x90`\x01`\x01`\xA0\x1B\x03\x16\x15\x80\x15a\x1F\xF3WP`\x01\x82\x01T`\x01`\x01`\xA0\x1B\x03\x16\x15[a\x02\x0Ba\x05KV[\x80T`\x01`\x01`\xA0\x1B\x03\x93\x84\x16`\x01`\x01`\xA0\x1B\x03\x19\x91\x82\x16\x17\x82U`\x01\x90\x91\x01\x80T\x92\x90\x93\x16\x91\x16\x17\x90UPV[`\0\x82\x81R`\x08` R`@\x81 \x90[\x82Q\x81\x10\x15a\x0E\0W`\0a k\x84\x83\x81Q\x81\x10a TW\xFE[` \x02` \x01\x01Q\x84a1\x12\x90\x91\x90c\xFF\xFF\xFF\xFF\x16V[\x90Pa y\x81a\x02\na\x05KV[P`\x01\x01a :V[`\0\x82\x81R`\x01` R`@\x81 \x90[\x82Q\x81\x10\x15a\x0E\0W`\0a \xC0\x84\x83\x81Q\x81\x10a \xACW\xFE[` \x90\x81\x02\x91\x90\x91\x01\x01Q\x84\x90`\0a1uV[\x90Pa \xCE\x81a\x02\na\x05KV[P`\x01\x01a \x92V[`\0\x80`\0a \xE7\x86\x86\x86a2\"V[\x92P\x92P\x92Pa!\x11a \xF9\x84a2\xE9V[\x80\x15a!\tWPa!\t\x83a2\xE9V[a\x02\ra\x05KV[`\0\x95\x86R`\t` R`@\x86 \x80T`\x01`\x01`\xA0\x1B\x03\x19\x90\x81\x16\x82U`\x01\x90\x91\x01\x80T\x90\x91\x16\x90U\x94\x90\x94UPPPPV[`\0\x82\x81R`\x08` R`@\x81 \x90[\x82Q\x81\x10\x15a\x0E\0W`\0\x83\x82\x81Q\x81\x10a!lW\xFE[` \x02` \x01\x01Q\x90Pa!\xB8a!\t`\x07`\0\x88\x81R` \x01\x90\x81R` \x01`\0 `\0\x84`\x01`\x01`\xA0\x1B\x03\x16`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01\x90\x81R` \x01`\0 Ta2\xE9V[`\0\x85\x81R`\x07` \x90\x81R`@\x80\x83 `\x01`\x01`\xA0\x1B\x03\x85\x16\x84R\x90\x91R\x81 \x81\x90Ua!\xE7\x84\x83a3\x0BV[\x90Pa!\xF5\x81a\x02\ta\x05KV[PP`\x01\x01a!UV[`\0\x82\x81R`\x01` R`@\x81 \x90[\x82Q\x81\x10\x15a\x0E\0W`\0\x83\x82\x81Q\x81\x10a\"&W\xFE[` \x02` \x01\x01Q\x90P`\0a\"<\x84\x83a4\x12V[\x90Pa\"Ja!\t\x82a2\xE9V[a\"T\x84\x83a4!V[PPP\x80`\x01\x01\x90Pa\"\x0FV[a\"jaMZV[P\x90V[a\"va\x17hV[\x83a\"\x80\x81a&\x83V[\x83a\"\x8A\x81a\x1B\x0FV[a\"\x9E\x83`\0\x01QQ\x84` \x01QQa\x1E\x12V[``a\"\xAD\x84`\0\x01Qa4\xC3V[\x90P``a\"\xBB\x88\x83a5RV[\x90P``\x80``a\"\xD0\x8C\x8C\x8C\x8C\x8C\x89a5\xE3V[\x92P\x92P\x92P`\0a\"\xE1\x8Ca\x1FdV[\x90P`\x02\x81`\x02\x81\x11\x15a\"\xF1W\xFE[\x14\x15a#YWa#T\x8C\x87`\0\x81Q\x81\x10a#\x08W\xFE[` \x02` \x01\x01Q\x86`\0\x81Q\x81\x10a#\x1DW\xFE[` \x02` \x01\x01Q\x89`\x01\x81Q\x81\x10a#2W\xFE[` \x02` \x01\x01Q\x88`\x01\x81Q\x81\x10a#GW\xFE[` \x02` \x01\x01Qa7\xA8V[a#\x82V[`\x01\x81`\x02\x81\x11\x15a#gW\xFE[\x14\x15a#xWa#T\x8C\x87\x86a7\xE7V[a#\x82\x8C\x85a8TV[`\0\x80\x8E`\x01\x81\x11\x15a#\x91W\xFE[\x14\x90P\x8B`\x01`\x01`\xA0\x1B\x03\x16\x8D\x7F\xE5\xCE$\x90\x87\xCE\x04\xF0Z\x95q\x92CT\0\xFD\x97\x86\x8D\xBA\x0EjKL\x04\x9A\xBF\x8A\xF8\r\xAEx\x89a#\xCB\x88\x86a8\x9DV[\x87`@Qa#\xDB\x93\x92\x91\x90a\\LV[`@Q\x80\x91\x03\x90\xA3PPPPPPPPPa\x1D1a\x18\"V[``\x83Qg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x80\x15a$\x0EW`\0\x80\xFD[P`@Q\x90\x80\x82R\x80` \x02` \x01\x82\x01`@R\x80\x15a$8W\x81` \x01` \x82\x02\x806\x837\x01\x90P[P\x90Pa$CaM\x84V[a$KaL\xE1V[`\0\x80`\0[\x89Q\x81\x10\x15a&vW\x89\x81\x81Q\x81\x10a$fW\xFE[` \x02` \x01\x01Q\x94P`\0\x89Q\x86` \x01Q\x10\x80\x15a$\x8AWP\x89Q\x86`@\x01Q\x10[\x90Pa$\x97\x81`da\x05KV[`\0a$\xB9\x8B\x88` \x01Q\x81Q\x81\x10a$\xACW\xFE[` \x02` \x01\x01Qa\x1BAV[\x90P`\0a$\xD0\x8C\x89`@\x01Q\x81Q\x81\x10a$\xACW\xFE[\x90Pa$\xF3\x81`\x01`\x01`\xA0\x1B\x03\x16\x83`\x01`\x01`\xA0\x1B\x03\x16\x14\x15a\x01\xFDa\x05KV[``\x88\x01Qa%CWa%\x0B`\0\x85\x11a\x01\xFEa\x05KV[`\0a%\x18\x8B\x84\x84a9EV[`\x01`\x01`\xA0\x1B\x03\x16\x87`\x01`\x01`\xA0\x1B\x03\x16\x14\x90Pa%:\x81a\x01\xFFa\x05KV[P``\x88\x01\x85\x90R[\x87Q`\x80\x88\x01R\x86\x8A`\x01\x81\x11\x15a%WW\xFE[\x90\x81`\x01\x81\x11\x15a%dW\xFE[\x90RP`\x01`\x01`\xA0\x1B\x03\x80\x83\x16` \x89\x01R\x81\x81\x16`@\x80\x8A\x01\x91\x90\x91R``\x80\x8B\x01Q\x90\x8A\x01R`\x80\x8A\x01Qa\x01\0\x8A\x01R\x8CQ\x82\x16`\xC0\x8A\x01R\x8C\x01Q\x16`\xE0\x88\x01R`\0\x80a%\xB6\x89a\x1BfV[\x91\x98P\x92P\x90Pa%\xC8\x8C\x85\x85a9gV[\x97Pa%\xFCa%\xD6\x83a9\x81V[\x8C\x8C` \x01Q\x81Q\x81\x10a%\xE6W\xFE[` \x02` \x01\x01Qa9\xB1\x90\x91\x90c\xFF\xFF\xFF\xFF\x16V[\x8B\x8B` \x01Q\x81Q\x81\x10a&\x0CW\xFE[` \x02` \x01\x01\x81\x81RPPa&Ja&$\x82a9\x81V[\x8C\x8C`@\x01Q\x81Q\x81\x10a&4W\xFE[` \x02` \x01\x01Qa9\xE5\x90\x91\x90c\xFF\xFF\xFF\xFF\x16V[\x8B\x8B`@\x01Q\x81Q\x81\x10a&ZW\xFE[` \x02` \x01\x01\x81\x81RPPPPPPP\x80`\x01\x01\x90Pa$QV[PPPPP\x94\x93PPPPV[`\0\x81\x81R`\x05` R`@\x90 Ta\x05~\x90`\xFF\x16a\x01\xF4a\x05KV[`\0\x80`\0\x80`\0a&\xB2\x87a:\x19V[\x94P\x94P\x94P\x94PP\x83`\x01`\x01`\xA0\x1B\x03\x16\x86`\x01`\x01`\xA0\x1B\x03\x16\x14\x15a&\xE1W\x82\x94PPPPPa\x16\xF5V[\x81`\x01`\x01`\xA0\x1B\x03\x16\x86`\x01`\x01`\xA0\x1B\x03\x16\x14\x15a'\x06W\x93Pa\x16\xF5\x92PPPV[a'\x11a\x02\ta\x16\xFBV[PPPP\x92\x91PPV[`\0\x82\x81R`\x07` \x90\x81R`@\x80\x83 `\x01`\x01`\xA0\x1B\x03\x85\x16\x84R\x90\x91R\x81 T\x81a'H\x82a:\x8FV[\x80a'fWP`\0\x85\x81R`\x08` R`@\x90 a'f\x90\x85a:\xA1V[\x90P\x80a'\x81Wa'v\x85a&\x83V[a'\x81a\x02\ta\x16\xFBV[P\x93\x92PPPV[`\0\x82\x81R`\x01` R`@\x81 a\t\xE1\x81\x84a4\x12V[m\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90V[`p\x1Cm\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90V[`\xE0\x1C\x90V[`\0\x80a'\xDC\x84a\x1FdV[\x90P`\x02\x81`\x02\x81\x11\x15a'\xECW\xFE[\x14\x15a(\x04Wa'\xFC\x84\x84a:\xC2V[\x91PPa\x16\xF5V[`\x01\x81`\x02\x81\x11\x15a(\x12W\xFE[\x14\x15a(\"Wa'\xFC\x84\x84a;\x13V[a'\xFC\x84\x84a;+V[`\0\x80`\0a(:\x86a\x1FdV[\x90P`\0\x87`\x02\x81\x11\x15a(JW\xFE[\x14\x15a(fWa(\\\x86\x82\x87\x87a;CV[\x92P\x92PPa(\x92V[`\x01\x87`\x02\x81\x11\x15a(tW\xFE[\x14\x15a(\x86Wa(\\\x86\x82\x87\x87a;\xBEV[a(\\\x86\x82\x87\x87a<:V[\x94P\x94\x92PPPV[`\0\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0a)\x08a<\x9DV[0`@Q` \x01a)\x1D\x95\x94\x93\x92\x91\x90a]\xF0V[`@Q` \x81\x83\x03\x03\x81R\x90`@R\x80Q\x90` \x01 \x90P\x90V[``\x1C\x90V[``\x80`\0a)L\x84a\x1FdV[\x90P`\x02\x81`\x02\x81\x11\x15a)\\W\xFE[\x14\x15a)uWa)k\x84a<\xA1V[\x92P\x92PPa)\x9BV[`\x01\x81`\x02\x81\x11\x15a)\x83W\xFE[\x14\x15a)\x92Wa)k\x84a=\xD6V[a)k\x84a>\xFDV[\x91P\x91V[```\0\x82Qg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x80\x15a)\xBCW`\0\x80\xFD[P`@Q\x90\x80\x82R\x80` \x02` \x01\x82\x01`@R\x80\x15a)\xE6W\x81` \x01` \x82\x02\x806\x837\x01\x90P[P\x91P`\0\x90P`\0[\x82Q\x81\x10\x15a\x15\x1DW`\0\x84\x82\x81Q\x81\x10a*\x07W\xFE[` \x02` \x01\x01Q\x90Pa*\x1A\x81a?\xF9V[\x84\x83\x81Q\x81\x10a*&W\xFE[` \x02` \x01\x01\x81\x81RPPa*D\x83a*?\x83a'\xCAV[a@\x14V[\x92PP`\x01\x01a)\xF0V[`\x01`\x01`\xA0\x1B\x03\x91\x82\x16`\0\x90\x81R`\x04` \x90\x81R`@\x80\x83 \x93\x90\x94\x16\x82R\x91\x90\x91R T`\xFF\x16\x90V[`\x03T`@Q\x7F\x9B\xE2\xA8\x84\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\0\x91a\x01\0\x90\x04`\x01`\x01`\xA0\x1B\x03\x16\x90c\x9B\xE2\xA8\x84\x90a*\xD0\x90\x86\x90\x86\x900\x90`\x04\x01a]GV[` `@Q\x80\x83\x03\x81\x86\x80;\x15\x80\x15a*\xE8W`\0\x80\xFD[PZ\xFA\x15\x80\x15a*\xFCW=`\0\x80>=`\0\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x16\xF2\x91\x90aTxV[`\0\x80a+-\x86\x86a\x1A\x01V[\x90Pa+F\x83\x80a+>WP\x84\x82\x10\x15[a\x02\x01a\x05KV[a+P\x81\x85a@+V[\x91P\x81\x81\x03a+l\x87\x87\x83a+d\x87a9\x81V[`\0\x03a@:V[PP\x94\x93PPPPV[`\0a+\x82\x84\x84a\x1A\x01V[\x90P`\0a+\x90\x82\x84a\x19EV[\x90Pa\x1D1\x85\x85\x83a+\xA1\x87a9\x81V[a@:V[a\x0E\0\x84c#\xB8r\xDD`\xE0\x1B\x85\x85\x85`@Q`$\x01a\x1E\xC5\x93\x92\x91\x90a[wV[a+\xD6\x81G\x10\x15a\x01\xA3a\x05KV[`\0\x82`\x01`\x01`\xA0\x1B\x03\x16\x82`@Qa+\xEF\x90a\x05HV[`\0`@Q\x80\x83\x03\x81\x85\x87Z\xF1\x92PPP=\x80`\0\x81\x14a,,W`@Q\x91P`\x1F\x19`?=\x01\x16\x82\x01`@R=\x82R=`\0` \x84\x01>a,1V[``\x91P[PP\x90Pa\x0F\xA0\x81a\x01\xA4a\x05KV[`\x01`\x01`\xA0\x1B\x03\x82\x16`\0\x90\x81R`\x02` R`@\x90 \x80T`\x01\x81\x01\x90\x91Ua\x0F\xA0a,o\x84\x83a@\x95V[\x83a\x05KV[`\0\x80`\0\x80a,\x92\x86`\x80\x01Q\x87` \x01Q\x88`@\x01Qa2\"V[\x92P\x92P\x92P`\0\x80\x87`@\x01Q`\x01`\x01`\xA0\x1B\x03\x16\x88` \x01Q`\x01`\x01`\xA0\x1B\x03\x16\x10\x15a,\xC7WP\x83\x90P\x82a,\xCDV[P\x82\x90P\x83[a,\xD9\x88\x88\x84\x84aA\xBBV[`@\x8B\x01Q` \x8C\x01Q\x91\x99P\x92\x94P\x90\x92P`\x01`\x01`\xA0\x1B\x03\x91\x82\x16\x91\x16\x10a-\rWa-\x08\x81\x83aB\xD1V[a-\x17V[a-\x17\x82\x82aB\xD1V[\x90\x92UP\x92\x95\x94PPPPPV[`\0\x80a-:\x84`\x80\x01Q\x85` \x01Qa'\x1BV[\x90P`\0a-P\x85`\x80\x01Q\x86`@\x01Qa'\x1BV[\x90Pa-^\x85\x85\x84\x84aA\xBBV[`\x80\x88\x01\x80Q`\0\x90\x81R`\x07` \x81\x81R`@\x80\x84 \x82\x8E\x01Q`\x01`\x01`\xA0\x1B\x03\x90\x81\x16\x86R\x90\x83R\x81\x85 \x98\x90\x98U\x93Q\x83R\x90\x81R\x82\x82 \x9A\x83\x01Q\x90\x95\x16\x81R\x98\x90\x93R\x91\x90\x96 \x95\x90\x95UP\x92\x93\x92PPPV[`\x80\x82\x01Q`\0\x90\x81R`\x01` \x90\x81R`@\x82 \x90\x84\x01Q\x82\x91\x82\x91\x82\x90a-\xE2\x90\x83\x90aC\x0CV[\x90P`\0a-\xFD\x88`@\x01Q\x84aC\x0C\x90\x91\x90c\xFF\xFF\xFF\xFF\x16V[\x90P\x81\x15\x80a.\nWP\x80\x15[\x15a.'Wa.\x1C\x88`\x80\x01Qa&\x83V[a.'a\x02\ta\x16\xFBV[`\0\x19\x91\x82\x01\x91\x01`\0a.:\x84aC+V[\x90P``\x81g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x80\x15a.UW`\0\x80\xFD[P`@Q\x90\x80\x82R\x80` \x02` \x01\x82\x01`@R\x80\x15a.\x7FW\x81` \x01` \x82\x02\x806\x837\x01\x90P[P`\0`\xA0\x8C\x01\x81\x90R\x90\x91P[\x82\x81\x10\x15a.\xFFW`\0a.\xA1\x87\x83aC/V[\x90Pa.\xAC\x81a?\xF9V[\x83\x83\x81Q\x81\x10a.\xB8W\xFE[` \x02` \x01\x01\x81\x81RPPa.\xD5\x8C`\xA0\x01Qa*?\x83a'\xCAV[`\xA0\x8D\x01R\x81\x86\x14\x15a.\xEAW\x80\x98Pa.\xF6V[\x84\x82\x14\x15a.\xF6W\x80\x97P[P`\x01\x01a.\x8DV[P`@Q\x7F\x01\xEC\x95J\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x8A\x16\x90c\x01\xEC\x95J\x90a/K\x90\x8D\x90\x85\x90\x89\x90\x89\x90`\x04\x01a^[V[` `@Q\x80\x83\x03\x81`\0\x87\x80;\x15\x80\x15a/eW`\0\x80\xFD[PZ\xF1\x15\x80\x15a/yW=`\0\x80>=`\0\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a/\x9D\x91\x90aYhV[\x97P`\0\x80a/\xB5\x8C`\0\x01Q\x8D``\x01Q\x8Ca/\xF7V[\x90\x92P\x90Pa/\xC4\x89\x83aCEV[\x98Pa/\xD0\x88\x82aCvV[\x97Pa/\xDD\x87\x87\x8BaC\x8CV[a/\xE8\x87\x86\x8AaC\x8CV[PPPPPPPPP\x92\x91PPV[`\0\x80\x80\x85`\x01\x81\x11\x15a0\x07W\xFE[\x14\x15a0\x17WP\x82\x90P\x81a0\x1DV[P\x81\x90P\x82[\x93P\x93\x91PPV[`\0\x82\x82\x02a0I\x84\x15\x80a0BWP\x83\x85\x83\x81a0?W\xFE[\x04\x14[`\x03a\x05KV[\x80a0XW`\0\x91PPa\x16\xF5V[g\r\xE0\xB6\xB3\xA7d\0\0`\0\x19\x82\x01\x04`\x01\x01\x91PPa\x16\xF5V[`\0``\x83`\x01`\x01`\xA0\x1B\x03\x16\x83`@Qa0\x8E\x91\x90aZ\xEAV[`\0`@Q\x80\x83\x03\x81`\0\x86Z\xF1\x91PP=\x80`\0\x81\x14a0\xCBW`@Q\x91P`\x1F\x19`?=\x01\x16\x82\x01`@R=\x82R=`\0` \x84\x01>a0\xD0V[``\x91P[P\x91P\x91P`\0\x82\x14\x15a0\xE8W=`\0\x80>=`\0\xFD[a\x0E\0\x81Q`\0\x14\x80a1\nWP\x81\x80` \x01\x90Q\x81\x01\x90a1\n\x91\x90aTxV[a\x01\xA2a\x05KV[`\0a1\x1E\x83\x83a:\xA1V[a1mWP\x81T`\x01\x80\x82\x01\x84U`\0\x84\x81R` \x80\x82 \x90\x93\x01\x80T`\x01`\x01`\xA0\x1B\x03\x19\x16`\x01`\x01`\xA0\x1B\x03\x86\x16\x90\x81\x17\x90\x91U\x85T\x90\x82R\x82\x86\x01\x90\x93R`@\x90 \x91\x90\x91Ua\x16\xF5V[P`\0a\x16\xF5V[`\x01`\x01`\xA0\x1B\x03\x82\x16`\0\x90\x81R`\x02\x84\x01` R`@\x81 T\x80a2\x02WPP\x82T`@\x80Q\x80\x82\x01\x82R`\x01`\x01`\xA0\x1B\x03\x85\x81\x16\x80\x83R` \x80\x84\x01\x87\x81R`\0\x87\x81R`\x01\x80\x8C\x01\x84R\x87\x82 \x96Q\x87T`\x01`\x01`\xA0\x1B\x03\x19\x16\x96\x16\x95\x90\x95\x17\x86U\x90Q\x94\x84\x01\x94\x90\x94U\x94\x82\x01\x80\x89U\x90\x83R`\x02\x88\x01\x90\x94R\x91\x90 \x91\x90\x91Ua\x18vV[`\0\x19\x01`\0\x90\x81R`\x01\x80\x86\x01` R`@\x82 \x01\x83\x90U\x90Pa\x18vV[`\0\x80`\0\x80`\0a24\x87\x87aC\xA4V[\x91P\x91P`\0a2D\x83\x83aC\xD5V[`\0\x8A\x81R`\t` \x90\x81R`@\x80\x83 \x84\x84R`\x02\x01\x90\x91R\x81 \x80T`\x01\x82\x01T\x91\x97P\x92\x93P\x90a2w\x83a:\x8FV[\x80a2\x86WPa2\x86\x82a:\x8FV[\x80a2\xA7WPa2\x96\x8C\x87a:\xC2V[\x80\x15a2\xA7WPa2\xA7\x8C\x86a:\xC2V[\x90P\x80a2\xC2Wa2\xB7\x8Ca&\x83V[a2\xC2a\x02\ta\x16\xFBV[a2\xCC\x83\x83aD\x08V[\x98Pa2\xD8\x83\x83aD-V[\x97PPPPPPP\x93P\x93P\x93\x90PV[{\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x15\x90V[`\x01`\x01`\xA0\x1B\x03\x81\x16`\0\x90\x81R`\x01\x83\x01` R`@\x81 T\x80\x15a4\x08W\x83T`\0\x19\x80\x83\x01\x91\x90\x81\x01\x90`\0\x90\x87\x90\x83\x90\x81\x10a3HW\xFE[`\0\x91\x82R` \x90\x91 \x01T\x87T`\x01`\x01`\xA0\x1B\x03\x90\x91\x16\x91P\x81\x90\x88\x90\x85\x90\x81\x10a3qW\xFE[`\0\x91\x82R` \x80\x83 \x91\x90\x91\x01\x80T`\x01`\x01`\xA0\x1B\x03\x19\x16`\x01`\x01`\xA0\x1B\x03\x94\x85\x16\x17\x90U\x91\x83\x16\x81R`\x01\x89\x81\x01\x90\x92R`@\x90 \x90\x84\x01\x90U\x86T\x87\x90\x80a3\xBAW\xFE[`\0\x82\x81R` \x80\x82 \x83\x01`\0\x19\x90\x81\x01\x80T`\x01`\x01`\xA0\x1B\x03\x19\x16\x90U\x90\x92\x01\x90\x92U`\x01`\x01`\xA0\x1B\x03\x88\x16\x82R`\x01\x89\x81\x01\x90\x91R`@\x82 \x91\x90\x91U\x94Pa\x16\xF5\x93PPPPV[`\0\x91PPa\x16\xF5V[`\0a\x16\xF2\x83\x83a\x02\taDDV[`\x01`\x01`\xA0\x1B\x03\x81\x16`\0\x90\x81R`\x02\x83\x01` R`@\x81 T\x80\x15a4\x08W\x83T`\0\x19\x90\x81\x01`\0\x81\x81R`\x01\x87\x81\x01` \x90\x81R`@\x80\x84 \x95\x87\x01\x84R\x80\x84 \x86T\x81T`\x01`\x01`\xA0\x1B\x03\x19\x90\x81\x16`\x01`\x01`\xA0\x1B\x03\x92\x83\x16\x17\x83U\x88\x86\x01\x80T\x93\x87\x01\x93\x90\x93U\x88T\x82\x16\x87R`\x02\x8D\x01\x80\x86R\x84\x88 \x9A\x90\x9AU\x88T\x16\x90\x97U\x84\x90U\x93\x89U\x93\x87\x16\x82R\x93\x90\x92R\x81 U\x90Pa\x16\xF5V[``\x80\x82Qg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x80\x15a4\xDEW`\0\x80\xFD[P`@Q\x90\x80\x82R\x80` \x02` \x01\x82\x01`@R\x80\x15a5\x08W\x81` \x01` \x82\x02\x806\x837\x01\x90P[P\x90P`\0[\x83Q\x81\x10\x15a\x07\xFFWa5&\x84\x82\x81Q\x81\x10a$\xACW\xFE[\x82\x82\x81Q\x81\x10a52W\xFE[`\x01`\x01`\xA0\x1B\x03\x90\x92\x16` \x92\x83\x02\x91\x90\x91\x01\x90\x91\x01R`\x01\x01a5\x0EV[``\x80``a5`\x85a)>V[\x91P\x91Pa5p\x82Q\x85Qa\x1E\x12V[a5\x80`\0\x83Q\x11a\x02\x0Fa\x05KV[`\0[\x82Q\x81\x10\x15a5\xDAWa5\xD2\x85\x82\x81Q\x81\x10a5\x9BW\xFE[` \x02` \x01\x01Q`\x01`\x01`\xA0\x1B\x03\x16\x84\x83\x81Q\x81\x10a5\xB8W\xFE[` \x02` \x01\x01Q`\x01`\x01`\xA0\x1B\x03\x16\x14a\x02\x08a\x05KV[`\x01\x01a5\x83V[P\x94\x93PPPPV[``\x80``\x80`\0a5\xF4\x86a)\xA0V[\x91P\x91P`\0a6\x03\x8Ba)8V[\x90P`\0\x8C`\x01\x81\x11\x15a6\x13W\xFE[\x14a6\xB6W\x80`\x01`\x01`\xA0\x1B\x03\x16ct\xF3\xB0\t\x8C\x8C\x8C\x87\x87a64aD\x81V[\x8F`@\x01Q`@Q\x88c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01a6[\x97\x96\x95\x94\x93\x92\x91\x90a]fV[`\0`@Q\x80\x83\x03\x81`\0\x87\x80;\x15\x80\x15a6uW`\0\x80\xFD[PZ\xF1\x15\x80\x15a6\x89W=`\0\x80>=`\0\xFD[PPPP`@Q=`\0\x82>`\x1F=\x90\x81\x01`\x1F\x19\x16\x82\x01`@Ra6\xB1\x91\x90\x81\x01\x90aT\x05V[a7OV[\x80`\x01`\x01`\xA0\x1B\x03\x16c\xD5\xC0\x96\xC4\x8C\x8C\x8C\x87\x87a6\xD2aD\x81V[\x8F`@\x01Q`@Q\x88c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01a6\xF9\x97\x96\x95\x94\x93\x92\x91\x90a]fV[`\0`@Q\x80\x83\x03\x81`\0\x87\x80;\x15\x80\x15a7\x13W`\0\x80\xFD[PZ\xF1\x15\x80\x15a7'W=`\0\x80>=`\0\xFD[PPPP`@Q=`\0\x82>`\x1F=\x90\x81\x01`\x1F\x19\x16\x82\x01`@Ra7O\x91\x90\x81\x01\x90aT\x05V[\x80\x95P\x81\x96PPPa7e\x87Q\x86Q\x86QaD\xFBV[`\0\x8C`\x01\x81\x11\x15a7sW\xFE[\x14a7\x8AWa7\x85\x89\x89\x89\x88\x88aE\x13V[a7\x97V[a7\x97\x8A\x89\x89\x88\x88aFZV[\x95PPPP\x96P\x96P\x96\x93PPPPV[`\0a7\xB4\x85\x84aC\xD5V[`\0\x87\x81R`\t` \x90\x81R`@\x80\x83 \x84\x84R`\x02\x01\x90\x91R\x90 \x90\x91Pa7\xDD\x85\x84aB\xD1V[\x90UPPPPPPV[`\0[\x82Q\x81\x10\x15a\x0E\0W\x81\x81\x81Q\x81\x10a7\xFFW\xFE[` \x02` \x01\x01Q`\x07`\0\x86\x81R` \x01\x90\x81R` \x01`\0 `\0\x85\x84\x81Q\x81\x10a8(W\xFE[` \x90\x81\x02\x91\x90\x91\x01\x81\x01Q`\x01`\x01`\xA0\x1B\x03\x16\x82R\x81\x01\x91\x90\x91R`@\x01`\0 U`\x01\x01a7\xEAV[`\0\x82\x81R`\x01` R`@\x81 \x90[\x82Q\x81\x10\x15a\x0E\0Wa8\x95\x81\x84\x83\x81Q\x81\x10a8}W\xFE[` \x02` \x01\x01Q\x84aC\x8C\x90\x92\x91\x90c\xFF\xFF\xFF\xFF\x16V[`\x01\x01a8dV[``\x82Qg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x80\x15a8\xB7W`\0\x80\xFD[P`@Q\x90\x80\x82R\x80` \x02` \x01\x82\x01`@R\x80\x15a8\xE1W\x81` \x01` \x82\x02\x806\x837\x01\x90P[P\x90P`\0[\x83Q\x81\x10\x15a\x07\xFFW\x82a9\x11W\x83\x81\x81Q\x81\x10a9\x01W\xFE[` \x02` \x01\x01Q`\0\x03a9&V[\x83\x81\x81Q\x81\x10a9\x1DW\xFE[` \x02` \x01\x01Q[\x82\x82\x81Q\x81\x10a92W\xFE[` \x90\x81\x02\x91\x90\x91\x01\x01R`\x01\x01a8\xE7V[`\0\x80\x84`\x01\x81\x11\x15a9TW\xFE[\x14a9_W\x81a\t\xE1V[P\x90\x92\x91PPV[`\0\x80\x84`\x01\x81\x11\x15a9vW\xFE[\x14a\x07\xFFW\x82a\t\xE1V[`\0a\"j\x7F\x80\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x83\x10a\x01\xA5a\x05KV[`\0\x82\x82\x01a\x16\xF2\x82\x84\x12\x80\x15\x90a9\xC9WP\x84\x82\x12\x15[\x80a9\xDEWP`\0\x84\x12\x80\x15a9\xDEWP\x84\x82\x12[`\0a\x05KV[`\0\x81\x83\x03a\x16\xF2\x82\x84\x12\x80\x15\x90a9\xFDWP\x84\x82\x13\x15[\x80a:\x12WP`\0\x84\x12\x80\x15a:\x12WP\x84\x82\x13[`\x01a\x05KV[`\0\x81\x81R`\t` R`@\x81 \x80T`\x01\x82\x01T`\x01`\x01`\xA0\x1B\x03\x91\x82\x16\x92\x84\x92\x90\x91\x16\x90\x82\x90\x81a:M\x86\x85aC\xD5V[`\0\x81\x81R`\x02\x84\x01` R`@\x90 \x80T`\x01\x82\x01T\x91\x99P\x91\x92Pa:t\x82\x82aD\x08V[\x96Pa:\x80\x82\x82aD-V[\x94PPPPP\x91\x93\x95\x90\x92\x94PV[`\0a:\x9A\x82a2\xE9V[\x15\x92\x91PPV[`\x01`\x01`\xA0\x1B\x03\x16`\0\x90\x81R`\x01\x91\x90\x91\x01` R`@\x90 T\x15\x15\x90V[`\0\x82\x81R`\t` R`@\x81 \x80T`\x01`\x01`\xA0\x1B\x03\x84\x81\x16\x91\x16\x14\x80a:\xFAWP`\x01\x81\x01T`\x01`\x01`\xA0\x1B\x03\x84\x81\x16\x91\x16\x14[\x80\x15a\t\xE1WPPP`\x01`\x01`\xA0\x1B\x03\x16\x15\x15\x91\x90PV[`\0\x82\x81R`\x08` R`@\x81 a\t\xE1\x81\x84a:\xA1V[`\0\x82\x81R`\x01` R`@\x81 a\t\xE1\x81\x84aG\xD0V[`\0\x80`\x02\x85`\x02\x81\x11\x15a;TW\xFE[\x14\x15a;jWa;e\x86\x85\x85aG\xF1V[a;\x94V[`\x01\x85`\x02\x81\x11\x15a;xW\xFE[\x14\x15a;\x89Wa;e\x86\x85\x85aG\xFFV[a;\x94\x86\x85\x85aH\rV[\x82\x15a;\xAEWa;\xAE`\x01`\x01`\xA0\x1B\x03\x85\x163\x85a\x1E\xA6V[PP`\0\x81\x90\x03\x94\x90\x93P\x91PPV[`\0\x80`\x02\x85`\x02\x81\x11\x15a;\xCFW\xFE[\x14\x15a;\xE5Wa;\xE0\x86\x85\x85aH\x1BV[a<\x0FV[`\x01\x85`\x02\x81\x11\x15a;\xF3W\xFE[\x14\x15a<\x04Wa;\xE0\x86\x85\x85aH)V[a<\x0F\x86\x85\x85aH7V[\x82\x15a<*Wa<*`\x01`\x01`\xA0\x1B\x03\x85\x1630\x86a+\xA6V[P\x90\x94`\0\x86\x90\x03\x94P\x92PPPV[`\0\x80`\x02\x85`\x02\x81\x11\x15a<KW\xFE[\x14\x15a<cWa<\\\x86\x85\x85aHEV[\x90Pa<\x90V[`\x01\x85`\x02\x81\x11\x15a<qW\xFE[\x14\x15a<\x82Wa<\\\x86\x85\x85aHUV[a<\x8D\x86\x85\x85aHeV[\x90P[`\0\x91P\x94P\x94\x92PPPV[F\x90V[``\x80`\0\x80`\0\x80a<\xB3\x87a:\x19V[\x92\x97P\x90\x95P\x93P\x91PP`\x01`\x01`\xA0\x1B\x03\x84\x16\x15\x80a<\xDBWP`\x01`\x01`\xA0\x1B\x03\x82\x16\x15[\x15a=\x04WPP`@\x80Q`\0\x80\x82R` \x82\x01\x90\x81R\x81\x83\x01\x90\x92R\x94P\x92Pa)\x9B\x91PPV[`@\x80Q`\x02\x80\x82R``\x82\x01\x83R\x90\x91` \x83\x01\x90\x806\x837\x01\x90PP\x95P\x83\x86`\0\x81Q\x81\x10a=2W\xFE[` \x02` \x01\x01\x90`\x01`\x01`\xA0\x1B\x03\x16\x90\x81`\x01`\x01`\xA0\x1B\x03\x16\x81RPP\x81\x86`\x01\x81Q\x81\x10a=`W\xFE[`\x01`\x01`\xA0\x1B\x03\x92\x90\x92\x16` \x92\x83\x02\x91\x90\x91\x01\x82\x01R`@\x80Q`\x02\x80\x82R``\x82\x01\x83R\x90\x92\x90\x91\x90\x83\x01\x90\x806\x837\x01\x90PP\x94P\x82\x85`\0\x81Q\x81\x10a=\xA7W\xFE[` \x02` \x01\x01\x81\x81RPP\x80\x85`\x01\x81Q\x81\x10a=\xC1W\xFE[` \x02` \x01\x01\x81\x81RPPPPPP\x91P\x91V[`\0\x81\x81R`\x08` R`@\x90 ``\x90\x81\x90a=\xF2\x81aC+V[g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x80\x15a>\x08W`\0\x80\xFD[P`@Q\x90\x80\x82R\x80` \x02` \x01\x82\x01`@R\x80\x15a>2W\x81` \x01` \x82\x02\x806\x837\x01\x90P[P\x92P\x82Qg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x80\x15a>MW`\0\x80\xFD[P`@Q\x90\x80\x82R\x80` \x02` \x01\x82\x01`@R\x80\x15a>wW\x81` \x01` \x82\x02\x806\x837\x01\x90P[P\x91P`\0[\x83Q\x81\x10\x15a>\xF6W`\0a>\x92\x83\x83aHuV[\x90P\x80\x85\x83\x81Q\x81\x10a>\xA1W\xFE[`\x01`\x01`\xA0\x1B\x03\x92\x83\x16` \x91\x82\x02\x92\x90\x92\x01\x81\x01\x91\x90\x91R`\0\x88\x81R`\x07\x82R`@\x80\x82 \x93\x85\x16\x82R\x92\x90\x91R T\x84Q\x85\x90\x84\x90\x81\x10a>\xE2W\xFE[` \x90\x81\x02\x91\x90\x91\x01\x01RP`\x01\x01a>}V[PP\x91P\x91V[`\0\x81\x81R`\x01` R`@\x90 ``\x90\x81\x90a?\x19\x81aC+V[g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x80\x15a?/W`\0\x80\xFD[P`@Q\x90\x80\x82R\x80` \x02` \x01\x82\x01`@R\x80\x15a?YW\x81` \x01` \x82\x02\x806\x837\x01\x90P[P\x92P\x82Qg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x80\x15a?tW`\0\x80\xFD[P`@Q\x90\x80\x82R\x80` \x02` \x01\x82\x01`@R\x80\x15a?\x9EW\x81` \x01` \x82\x02\x806\x837\x01\x90P[P\x91P`\0[\x83Q\x81\x10\x15a>\xF6Wa?\xB7\x82\x82aH\xA2V[\x85\x83\x81Q\x81\x10a?\xC3W\xFE[` \x02` \x01\x01\x85\x84\x81Q\x81\x10a?\xD6W\xFE[` \x90\x81\x02\x91\x90\x91\x01\x01\x91\x90\x91R`\x01`\x01`\xA0\x1B\x03\x90\x91\x16\x90R`\x01\x01a?\xA4V[`\0a@\x04\x82a'\xB4V[a@\r\x83a'\xA1V[\x01\x92\x91PPV[`\0\x81\x83\x10\x15a@$W\x81a\x16\xF2V[P\x90\x91\x90PV[`\0\x81\x83\x10a@$W\x81a\x16\xF2V[`\x01`\x01`\xA0\x1B\x03\x80\x85\x16`\0\x81\x81R`\x0B` \x90\x81R`@\x80\x83 \x94\x88\x16\x80\x84R\x94\x90\x91R\x90\x81\x90 \x85\x90UQ\x7F\x18\xE1\xEAA9\xE6\x84\x13\xD7\xD0\x8A\xA7R\xE7\x15h\xE3k,[\xF9@\x893\x14\xC2\xC5\xB0\x1E\xAA\x0CB\x90a\x19\xD0\x90\x85\x90a]>V[`\0\x80a@\xA0aH\xC6V[\x90PB\x81\x10\x15a@\xB4W`\0\x91PPa\x16\xF5V[`\0a@\xBEaH\xD2V[\x90P\x80a@\xD0W`\0\x92PPPa\x16\xF5V[`\0\x81a@\xDBaI\xE3V[\x80Q` \x91\x82\x01 `@Qa@\xF7\x93\x923\x91\x8A\x91\x89\x91\x01a]\xC4V[`@Q` \x81\x83\x03\x03\x81R\x90`@R\x80Q\x90` \x01 \x90P`\0aA\x1A\x82aJ2V[\x90P`\0\x80`\0aA)aJNV[\x92P\x92P\x92P`\0`\x01\x85\x85\x85\x85`@Q`\0\x81R` \x01`@R`@QaAT\x94\x93\x92\x91\x90a^\x1CV[` `@Q` \x81\x03\x90\x80\x84\x03\x90\x85Z\xFA\x15\x80\x15aAvW=`\0\x80>=`\0\xFD[PP`@Q`\x1F\x19\x01Q\x91PP`\x01`\x01`\xA0\x1B\x03\x81\x16\x15\x80\x15\x90aA\xACWP\x8A`\x01`\x01`\xA0\x1B\x03\x16\x81`\x01`\x01`\xA0\x1B\x03\x16\x14[\x9B\x9APPPPPPPPPPPV[`\0\x80`\0\x80aA\xCA\x86a?\xF9V[\x90P`\0aA\xD7\x86a?\xF9V[\x90PaA\xEEaA\xE5\x88a'\xCAV[a*?\x88a'\xCAV[`\xA0\x8A\x01R`@Q\x7F\x9D,\x11\x0C\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x89\x16\x90c\x9D,\x11\x0C\x90aB<\x90\x8C\x90\x86\x90\x86\x90`\x04\x01a^\x94V[` `@Q\x80\x83\x03\x81`\0\x87\x80;\x15\x80\x15aBVW`\0\x80\xFD[PZ\xF1\x15\x80\x15aBjW=`\0\x80>=`\0\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90aB\x8E\x91\x90aYhV[\x92P`\0\x80aB\xA6\x8B`\0\x01Q\x8C``\x01Q\x87a/\xF7V[\x90\x92P\x90PaB\xB5\x89\x83aCEV[\x96PaB\xC1\x88\x82aCvV[\x95PPPPP\x94P\x94P\x94\x91PPV[`\0\x80aB\xE9aB\xE0\x85a'\xCAV[a*?\x85a'\xCAV[\x90Pa\t\xE1aB\xF7\x85a'\xA1V[aC\0\x85a'\xA1V[\x83c\xFF\xFF\xFF\xFF\x16aJuV[`\x01`\x01`\xA0\x1B\x03\x16`\0\x90\x81R`\x02\x91\x90\x91\x01` R`@\x90 T\x90V[T\x90V[`\0\x90\x81R`\x01\x91\x82\x01` R`@\x90 \x01T\x90V[`\0\x80aC[\x83aCU\x86a'\xA1V[\x90a\x19EV[\x90P`\0aCh\x85a'\xB4V[\x90PCa\x12\xA6\x83\x83\x83aJ\x83V[`\0\x80aC[\x83aC\x86\x86a'\xA1V[\x90aJ\xBCV[`\0\x91\x82R`\x01\x92\x83\x01` R`@\x90\x91 \x90\x91\x01UV[`\0\x80\x82`\x01`\x01`\xA0\x1B\x03\x16\x84`\x01`\x01`\xA0\x1B\x03\x16\x10aC\xC7W\x82\x84aC\xCAV[\x83\x83[\x91P\x91P\x92P\x92\x90PV[`\0\x82\x82`@Q` \x01aC\xEA\x92\x91\x90a[\x06V[`@Q` \x81\x83\x03\x03\x81R\x90`@R\x80Q\x90` \x01 \x90P\x92\x91PPV[`\0a\x16\xF2aD\x16\x84a'\xA1V[aD\x1F\x84a'\xA1V[aD(\x86a'\xCAV[aJ\x83V[`\0a\x16\xF2aD;\x84a'\xB4V[aD\x1F\x84a'\xB4V[`\x01`\x01`\xA0\x1B\x03\x82\x16`\0\x90\x81R`\x02\x84\x01` R`@\x81 TaDk\x81\x15\x15\x84a\x05KV[aDx\x85`\x01\x83\x03aC/V[\x95\x94PPPPPV[`\0aD\x8Ba\x13\xAFV[`\x01`\x01`\xA0\x1B\x03\x16cU\xC6v(`@Q\x81c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01` `@Q\x80\x83\x03\x81\x86\x80;\x15\x80\x15aD\xC3W`\0\x80\xFD[PZ\xFA\x15\x80\x15aD\xD7W=`\0\x80>=`\0\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x12\xCE\x91\x90aYhV[a\x0F\xA0\x82\x84\x14\x80\x15aE\x0CWP\x81\x83\x14[`ga\x05KV[``\x83Qg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x80\x15aE-W`\0\x80\xFD[P`@Q\x90\x80\x82R\x80` \x02` \x01\x82\x01`@R\x80\x15aEWW\x81` \x01` \x82\x02\x806\x837\x01\x90P[P\x90P`\0[\x85QQ\x81\x10\x15aFPW`\0\x84\x82\x81Q\x81\x10aEuW\xFE[` \x02` \x01\x01Q\x90PaE\xA5\x87` \x01Q\x83\x81Q\x81\x10aE\x92W\xFE[` \x02` \x01\x01Q\x82\x10\x15a\x01\xF9a\x05KV[`\0\x87`\0\x01Q\x83\x81Q\x81\x10aE\xB7W\xFE[` \x02` \x01\x01Q\x90PaE\xD1\x81\x83\x8B\x8B``\x01Qa\x1D8V[`\0\x85\x84\x81Q\x81\x10aE\xDFW\xFE[` \x02` \x01\x01Q\x90PaE\xFBaE\xF5\x83a\x1BAV[\x82a\x1F\x11V[aF*aF\x08\x84\x83a\x19EV[\x89\x86\x81Q\x81\x10aF\x14W\xFE[` \x02` \x01\x01QaCv\x90\x91\x90c\xFF\xFF\xFF\xFF\x16V[\x85\x85\x81Q\x81\x10aF6W\xFE[` \x02` \x01\x01\x81\x81RPPPPP\x80`\x01\x01\x90PaE]V[P\x95\x94PPPPPV[```\0\x84Qg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x80\x15aFvW`\0\x80\xFD[P`@Q\x90\x80\x82R\x80` \x02` \x01\x82\x01`@R\x80\x15aF\xA0W\x81` \x01` \x82\x02\x806\x837\x01\x90P[P\x91P`\0[\x86QQ\x81\x10\x15aG\xC6W`\0\x85\x82\x81Q\x81\x10aF\xBEW\xFE[` \x02` \x01\x01Q\x90PaF\xEE\x88` \x01Q\x83\x81Q\x81\x10aF\xDBW\xFE[` \x02` \x01\x01Q\x82\x11\x15a\x01\xFAa\x05KV[`\0\x88`\0\x01Q\x83\x81Q\x81\x10aG\0W\xFE[` \x02` \x01\x01Q\x90PaG\x1A\x81\x83\x8C\x8C``\x01Qa\x1CZV[aG#\x81a\x198V[\x15aG5WaG2\x84\x83a\x19EV[\x93P[`\0\x86\x84\x81Q\x81\x10aGCW\xFE[` \x02` \x01\x01Q\x90PaGYaE\xF5\x83a\x1BAV[\x80\x83\x10\x15aGxWaGs\x83\x82\x03\x8A\x86\x81Q\x81\x10aF\x14W\xFE[aG\xA0V[aG\xA0\x81\x84\x03\x8A\x86\x81Q\x81\x10aG\x8AW\xFE[` \x02` \x01\x01QaCE\x90\x91\x90c\xFF\xFF\xFF\xFF\x16V[\x86\x85\x81Q\x81\x10aG\xACW\xFE[` \x02` \x01\x01\x81\x81RPPPPP\x80`\x01\x01\x90PaF\xA6V[PaFP\x81a\x19\xDEV[`\x01`\x01`\xA0\x1B\x03\x16`\0\x90\x81R`\x02\x91\x90\x91\x01` R`@\x90 T\x15\x15\x90V[a\x0E\0\x83\x83aJ\xD2\x84aK\rV[a\x0E\0\x83\x83aJ\xD2\x84aK\xB8V[a\x0E\0\x83\x83aJ\xD2\x84aL\x13V[a\x0E\0\x83\x83aLb\x84aK\rV[a\x0E\0\x83\x83aLb\x84aK\xB8V[a\x0E\0\x83\x83aLb\x84aL\x13V[`\0a\t\xE1\x84\x84aL\x83\x85aK\rV[`\0a\t\xE1\x84\x84aL\x83\x85aK\xB8V[`\0a\t\xE1\x84\x84aL\x83\x85aL\x13V[`\0\x82`\0\x01\x82\x81T\x81\x10aH\x86W\xFE[`\0\x91\x82R` \x90\x91 \x01T`\x01`\x01`\xA0\x1B\x03\x16\x93\x92PPPV[`\0\x90\x81R`\x01\x91\x82\x01` R`@\x90 \x80T\x91\x01T`\x01`\x01`\xA0\x1B\x03\x90\x91\x16\x91V[`\0a\x12\xCE`\0aL\x9DV[`\0\x805`\xE0\x1C\x80c\xB9\\\xAC(\x81\x14aI\x1AWc\x8B\xDB9\x13\x81\x14aIBWcR\xBB\xBE)\x81\x14aIjWc\x94[\xCE\xC9\x81\x14aI\x92Wc\xFAng\x1D\x81\x14aI\xBAW`\0\x92PaI\xDEV[\x7F?{q%+\xD1\x91\x13\xFFH\xC1\x9Cn\0J\x9B\xCF\xCC\xA3 \xA0\xD7MX\xE8Xw\xCB\xD7\xDC\xAEX\x92PaI\xDEV[\x7F\x8B\xBCW\xF6n\xA96\x90/P\xA7\x1C\xE1+\x92\xC4?<S@\xBB@\xC2|N\x90\xAB\x84\xEE\xAE3S\x92PaI\xDEV[\x7F\xE1\x92\xDC\xBC\x14;\x1E$J\xD7;\x81?\xD3\xC0\x97\xB82\xAD&\n\x15s@\xB4\xE5\xE5\xBE\xDA\x06z\xBE\x92PaI\xDEV[\x7F\x9B\xFCC\xA4\xD9\x83\x13\xC6vi\x86\xFF\xD7\xC9\x16\xC7H\x15f\xD9\xF2$\xC6\x81\x9A\xF0\xA53\x88\xAC\xED:\x92PaI\xDEV[\x7F\xA3\xF8e\xAA5\x1EQ\xCF\xEB@\xF5\x17\x8D\x15d\xBBb\x9F\xE9\x03\x0B\x83\xCA\xF66\x1D\x1B\xAA\xF5\xB9\x0BZ\x92P[PP\x90V[```\x006\x80\x80`\x1F\x01` \x80\x91\x04\x02` \x01`@Q\x90\x81\x01`@R\x80\x93\x92\x91\x90\x81\x81R` \x01\x83\x83\x80\x82\x847`\0\x92\x01\x91\x90\x91RPP\x82Q\x92\x93PPP`\x80\x10\x15a\x05HW`\x806\x03\x81R\x90V[`\0aJ<a(\x9BV[\x82`@Q` \x01a\x11.\x92\x91\x90a[-V[`\0\x80`\0aJ]` aL\x9DV[\x92PaJi`@aL\x9DV[\x91Pa\x08A``aL\x9DV[`\xE0\x1B`p\x91\x90\x91\x1B\x01\x01\x90V[`\0\x83\x83\x01aJ\xB1\x85\x82\x10\x80\x15\x90aJ\xA9WPn\x01\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x82\x10[a\x02\x0Ea\x05KV[aDx\x85\x85\x85aJuV[`\0aJ\xCC\x83\x83\x11\x15`\x01a\x05KV[P\x90\x03\x90V[`\0\x80aJ\xE2\x83aC\x86\x86a'\xA1V[\x90P`\0aJ\xF3\x84aCU\x87a'\xB4V[\x90P`\0aK\0\x86a'\xCAV[\x90Pa\x12\xA6\x83\x83\x83aJ\x83V[`\0\x80`\0\x80`\0aK\x1E\x89a:\x19V[\x94PP\x93P\x93P\x93P`\0\x83`\x01`\x01`\xA0\x1B\x03\x16\x89`\x01`\x01`\xA0\x1B\x03\x16\x14\x15aKiW`\0aKS\x84\x89\x8Bc\xFF\xFF\xFF\xFF\x16V[\x90PaK_\x81\x85aL\xA7V[\x90\x93P\x90PaK\x8BV[`\0aKy\x83\x89\x8Bc\xFF\xFF\xFF\xFF\x16V[\x90PaK\x85\x81\x84aL\xA7V[\x90\x92P\x90P[aK\x95\x83\x83aB\xD1V[\x85UaK\xA1\x83\x83aL\xC3V[`\x01\x90\x95\x01\x94\x90\x94UP\x91\x92PPP\x94\x93PPPPV[`\0\x80aK\xC5\x86\x86a'\x1BV[\x90P`\0aK\xD7\x82\x85\x87c\xFF\xFF\xFF\xFF\x16V[`\0\x88\x81R`\x07` \x90\x81R`@\x80\x83 `\x01`\x01`\xA0\x1B\x03\x8B\x16\x84R\x90\x91R\x90 \x81\x90U\x90PaL\x08\x81\x83aL\xA7V[\x97\x96PPPPPPPV[`\0\x84\x81R`\x01` R`@\x81 \x81aL,\x82\x87a4\x12V[\x90P`\0aL>\x82\x86\x88c\xFF\xFF\xFF\xFF\x16V[\x90PaLK\x83\x88\x83a1uV[PaLV\x81\x83aL\xA7V[\x98\x97PPPPPPPPV[`\0\x80aLr\x83aCU\x86a'\xA1V[\x90P`\0aJ\xF3\x84aC\x86\x87a'\xB4V[`\0\x80aL\x8F\x84a'\xA1V[\x90PCaDx\x82\x85\x83aJ\x83V[6\x01`\x7F\x19\x015\x90V[`\0aL\xB2\x82a'\xB4V[aL\xBB\x84a'\xB4V[\x03\x93\x92PPPV[`\0a\x16\xF2aL\xD1\x84a'\xB4V[aL\xDA\x84a'\xB4V[`\0aJuV[`@\x80Qa\x01 \x81\x01\x90\x91R\x80`\0\x81R`\0` \x82\x01\x81\x90R`@\x82\x01\x81\x90R``\x80\x83\x01\x82\x90R`\x80\x83\x01\x82\x90R`\xA0\x83\x01\x82\x90R`\xC0\x83\x01\x82\x90R`\xE0\x83\x01\x91\x90\x91Ra\x01\0\x90\x91\x01R\x90V[`@\x80Q`\x80\x81\x01\x90\x91R\x80`\0\x81R`\0` \x82\x01\x81\x90R`@\x82\x01\x81\x90R``\x90\x91\x01R\x90V[`@Q\x80`\x80\x01`@R\x80``\x81R` \x01``\x81R` \x01``\x81R` \x01`\0\x15\x15\x81RP\x90V[`@Q\x80`\xA0\x01`@R\x80`\0\x80\x19\x16\x81R` \x01`\0\x81R` \x01`\0\x81R` \x01`\0\x81R` \x01``\x81RP\x90V[\x805a\x16\xF5\x81a_ZV[`\0\x82`\x1F\x83\x01\x12aM\xD1W\x80\x81\xFD[\x815aM\xE4aM\xDF\x82a_\x04V[a^\xDDV[\x81\x81R\x91P` \x80\x83\x01\x90\x84\x81\x01\x81\x84\x02\x86\x01\x82\x01\x87\x10\x15aN\x05W`\0\x80\xFD[`\0[\x84\x81\x10\x15aN-W\x815aN\x1B\x81a_ZV[\x84R\x92\x82\x01\x92\x90\x82\x01\x90`\x01\x01aN\x08V[PPPPP\x92\x91PPV[`\0\x82`\x1F\x83\x01\x12aNHW\x80\x81\xFD[\x815aNVaM\xDF\x82a_\x04V[\x81\x81R\x91P` \x80\x83\x01\x90\x84\x81\x01`\0[\x84\x81\x10\x15aN-W\x815\x87\x01`\xA0\x80`\x1F\x19\x83\x8C\x03\x01\x12\x15aN\x88W`\0\x80\xFD[aN\x91\x81a^\xDDV[\x85\x83\x015\x81R`@\x80\x84\x015\x87\x83\x01R``\x80\x85\x015\x82\x84\x01R`\x80\x91P\x81\x85\x015\x81\x84\x01RP\x82\x84\x015\x92Pg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x83\x11\x15aN\xD3W`\0\x80\xFD[aN\xE1\x8C\x88\x85\x87\x01\x01aO\xC0V[\x90\x82\x01R\x86RPP\x92\x82\x01\x92\x90\x82\x01\x90`\x01\x01aNgV[`\0\x82`\x1F\x83\x01\x12aO\tW\x80\x81\xFD[\x815aO\x17aM\xDF\x82a_\x04V[\x81\x81R\x91P` \x80\x83\x01\x90\x84\x81\x01\x81\x84\x02\x86\x01\x82\x01\x87\x10\x15aO8W`\0\x80\xFD[`\0[\x84\x81\x10\x15aN-W\x815\x84R\x92\x82\x01\x92\x90\x82\x01\x90`\x01\x01aO;V[`\0\x82`\x1F\x83\x01\x12aOgW\x80\x81\xFD[\x81QaOuaM\xDF\x82a_\x04V[\x81\x81R\x91P` \x80\x83\x01\x90\x84\x81\x01\x81\x84\x02\x86\x01\x82\x01\x87\x10\x15aO\x96W`\0\x80\xFD[`\0[\x84\x81\x10\x15aN-W\x81Q\x84R\x92\x82\x01\x92\x90\x82\x01\x90`\x01\x01aO\x99V[\x805a\x16\xF5\x81a_oV[`\0\x82`\x1F\x83\x01\x12aO\xD0W\x80\x81\xFD[\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15aO\xE6W\x81\x82\xFD[aO\xF9`\x1F\x82\x01`\x1F\x19\x16` \x01a^\xDDV[\x91P\x80\x82R\x83` \x82\x85\x01\x01\x11\x15aP\x10W`\0\x80\xFD[\x80` \x84\x01` \x84\x017`\0\x90\x82\x01` \x01R\x92\x91PPV[\x805a\x16\xF5\x81a_}V[\x805`\x02\x81\x10a\x16\xF5W`\0\x80\xFD[\x805`\x04\x81\x10a\x16\xF5W`\0\x80\xFD[`\0`\x80\x82\x84\x03\x12\x15aPcW\x80\x81\xFD[aPm`\x80a^\xDDV[\x90P\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15aP\x87W`\0\x80\xFD[aP\x93\x85\x83\x86\x01aM\xC1V[\x83R` \x84\x015\x91P\x80\x82\x11\x15aP\xA9W`\0\x80\xFD[aP\xB5\x85\x83\x86\x01aN\xF9V[` \x84\x01R`@\x84\x015\x91P\x80\x82\x11\x15aP\xCEW`\0\x80\xFD[PaP\xDB\x84\x82\x85\x01aO\xC0V[`@\x83\x01RPaP\xEE\x83``\x84\x01aO\xB5V[``\x82\x01R\x92\x91PPV[`\0`\x80\x82\x84\x03\x12\x15aQ\nW\x80\x81\xFD[aQ\x14`\x80a^\xDDV[\x90P\x815aQ!\x81a_ZV[\x81R` \x82\x015aQ1\x81a_oV[` \x82\x01R`@\x82\x015aQD\x81a_ZV[`@\x82\x01R``\x82\x015aP\xEE\x81a_oV[`\0` \x82\x84\x03\x12\x15aQhW\x80\x81\xFD[\x815a\x16\xF2\x81a_ZV[`\0\x80`@\x83\x85\x03\x12\x15aQ\x85W\x80\x81\xFD[\x825aQ\x90\x81a_ZV[\x91P` \x83\x015aQ\xA0\x81a_ZV[\x80\x91PP\x92P\x92\x90PV[`\0\x80`\0``\x84\x86\x03\x12\x15aQ\xBFW\x80\x81\xFD[\x835aQ\xCA\x81a_ZV[\x92P` \x84\x015aQ\xDA\x81a_ZV[\x91P`@\x84\x015aQ\xEA\x81a_oV[\x80\x91PP\x92P\x92P\x92V[`\0\x80`@\x83\x85\x03\x12\x15aR\x07W\x81\x82\xFD[\x825aR\x12\x81a_ZV[\x91P` \x83\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15aR-W\x81\x82\xFD[aR9\x85\x82\x86\x01aM\xC1V[\x91PP\x92P\x92\x90PV[`\0` \x80\x83\x85\x03\x12\x15aRUW\x81\x82\xFD[\x825g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15aRkW\x82\x83\xFD[\x83\x01`\x1F\x81\x01\x85\x13aR{W\x82\x83\xFD[\x805aR\x89aM\xDF\x82a_\x04V[\x81\x81R\x83\x81\x01\x90\x83\x85\x01`\x80\x80\x85\x02\x86\x01\x87\x01\x8A\x10\x15aR\xA7W\x87\x88\xFD[\x87\x95P[\x84\x86\x10\x15aS\x10W\x80\x82\x8B\x03\x12\x15aR\xC1W\x87\x88\xFD[aR\xCA\x81a^\xDDV[aR\xD4\x8B\x84aP)V[\x81R\x87\x83\x015\x88\x82\x01R`@aR\xEC\x8C\x82\x86\x01aM\xB6V[\x90\x82\x01R``\x83\x81\x015\x90\x82\x01R\x84R`\x01\x95\x90\x95\x01\x94\x92\x86\x01\x92\x90\x81\x01\x90aR\xABV[P\x90\x98\x97PPPPPPPPV[`\0` \x80\x83\x85\x03\x12\x15aS0W\x81\x82\xFD[\x825g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15aSFW\x82\x83\xFD[\x83\x01`\x1F\x81\x01\x85\x13aSVW\x82\x83\xFD[\x805aSdaM\xDF\x82a_\x04V[\x81\x81R\x83\x81\x01\x90\x83\x85\x01`\xA0\x80\x85\x02\x86\x01\x87\x01\x8A\x10\x15aS\x82W\x87\x88\xFD[\x87\x95P[\x84\x86\x10\x15aS\x10W\x80\x82\x8B\x03\x12\x15aS\x9CW\x87\x88\xFD[aS\xA5\x81a^\xDDV[aS\xAF\x8B\x84aPCV[\x81RaS\xBD\x8B\x89\x85\x01aM\xB6V[\x81\x89\x01R`@\x83\x81\x015\x90\x82\x01R``aS\xD9\x8C\x82\x86\x01aM\xB6V[\x90\x82\x01R`\x80aS\xEB\x8C\x85\x83\x01aM\xB6V[\x90\x82\x01R\x84R`\x01\x95\x90\x95\x01\x94\x92\x86\x01\x92\x90\x81\x01\x90aS\x86V[`\0\x80`@\x83\x85\x03\x12\x15aT\x17W\x81\x82\xFD[\x82Qg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15aT.W\x83\x84\xFD[aT:\x86\x83\x87\x01aOWV[\x93P` \x85\x01Q\x91P\x80\x82\x11\x15aTOW\x82\x83\xFD[PaR9\x85\x82\x86\x01aOWV[`\0` \x82\x84\x03\x12\x15aTmW\x80\x81\xFD[\x815a\x16\xF2\x81a_oV[`\0` \x82\x84\x03\x12\x15aT\x89W\x80\x81\xFD[\x81Qa\x16\xF2\x81a_oV[`\0` \x82\x84\x03\x12\x15aT\xA5W\x80\x81\xFD[P5\x91\x90PV[`\0\x80`\0\x80`\x80\x85\x87\x03\x12\x15aT\xC1W\x81\x82\xFD[\x845\x93P` \x85\x015aT\xD3\x81a_ZV[\x92P`@\x85\x015aT\xE3\x81a_ZV[\x91P``\x85\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15aT\xFEW\x81\x82\xFD[aU\n\x87\x82\x88\x01aPRV[\x91PP\x92\x95\x91\x94P\x92PV[`\0\x80`@\x83\x85\x03\x12\x15aU(W\x81\x82\xFD[\x825\x91P` \x83\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15aR-W\x81\x82\xFD[`\0\x80`\0``\x84\x86\x03\x12\x15aUYW\x80\x81\xFD[\x835\x92P` \x80\x85\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15aUxW\x83\x84\xFD[aU\x84\x88\x83\x89\x01aM\xC1V[\x94P`@\x87\x015\x91P\x80\x82\x11\x15aU\x99W\x83\x84\xFD[P\x85\x01`\x1F\x81\x01\x87\x13aU\xAAW\x82\x83\xFD[\x805aU\xB8aM\xDF\x82a_\x04V[\x81\x81R\x83\x81\x01\x90\x83\x85\x01\x85\x84\x02\x85\x01\x86\x01\x8B\x10\x15aU\xD4W\x86\x87\xFD[\x86\x94P[\x83\x85\x10\x15aU\xFFW\x805aU\xEB\x81a_ZV[\x83R`\x01\x94\x90\x94\x01\x93\x91\x85\x01\x91\x85\x01aU\xD8V[P\x80\x95PPPPPP\x92P\x92P\x92V[`\0\x80`@\x83\x85\x03\x12\x15aV!W\x81\x82\xFD[\x825\x91P` \x83\x015aQ\xA0\x81a_ZV[`\0` \x82\x84\x03\x12\x15aVDW\x80\x81\xFD[\x815`\x01`\x01`\xE0\x1B\x03\x19\x81\x16\x81\x14a\x16\xF2W\x81\x82\xFD[`\0\x80`\0\x80`\x80\x85\x87\x03\x12\x15aVpW\x81\x82\xFD[\x845aV{\x81a_ZV[\x93P` \x85\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15aV\x97W\x83\x84\xFD[aV\xA3\x88\x83\x89\x01aM\xC1V[\x94P`@\x87\x015\x91P\x80\x82\x11\x15aV\xB8W\x83\x84\xFD[aV\xC4\x88\x83\x89\x01aN\xF9V[\x93P``\x87\x015\x91P\x80\x82\x11\x15aV\xD9W\x82\x83\xFD[PaU\n\x87\x82\x88\x01aO\xC0V[`\0` \x82\x84\x03\x12\x15aV\xF7W\x80\x81\xFD[\x815a\x16\xF2\x81a_}V[`\0\x80`\0\x80`\xE0\x85\x87\x03\x12\x15aW\x17W\x81\x82\xFD[aW!\x86\x86aP4V[\x93P` \x85\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15aW=W\x83\x84\xFD[aWI\x88\x83\x89\x01aN8V[\x94P`@\x87\x015\x91P\x80\x82\x11\x15aW^W\x83\x84\xFD[PaWk\x87\x82\x88\x01aM\xC1V[\x92PPaW{\x86``\x87\x01aP\xF9V[\x90P\x92\x95\x91\x94P\x92PV[`\0\x80`\0\x80`\0\x80a\x01 \x87\x89\x03\x12\x15aW\x9FW\x83\x84\xFD[aW\xA9\x88\x88aP4V[\x95P` \x80\x88\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15aW\xC6W\x86\x87\xFD[aW\xD2\x8B\x83\x8C\x01aN8V[\x97P`@\x8A\x015\x91P\x80\x82\x11\x15aW\xE7W\x86\x87\xFD[aW\xF3\x8B\x83\x8C\x01aM\xC1V[\x96PaX\x02\x8B``\x8C\x01aP\xF9V[\x95P`\xE0\x8A\x015\x91P\x80\x82\x11\x15aX\x17W\x84\x85\xFD[P\x88\x01`\x1F\x81\x01\x8A\x13aX(W\x83\x84\xFD[\x805aX6aM\xDF\x82a_\x04V[\x81\x81R\x83\x81\x01\x90\x83\x85\x01\x85\x84\x02\x85\x01\x86\x01\x8E\x10\x15aXRW\x87\x88\xFD[\x87\x94P[\x83\x85\x10\x15aXtW\x805\x83R`\x01\x94\x90\x94\x01\x93\x91\x85\x01\x91\x85\x01aXVV[P\x80\x96PPPPPPa\x01\0\x87\x015\x90P\x92\x95P\x92\x95P\x92\x95V[`\0\x80`\0\x80`\xE0\x85\x87\x03\x12\x15aX\xA4W\x81\x82\xFD[\x845g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15aX\xBBW\x83\x84\xFD[\x90\x86\x01\x90`\xC0\x82\x89\x03\x12\x15aX\xCEW\x83\x84\xFD[aX\xD8`\xC0a^\xDDV[\x825\x81RaX\xE9\x89` \x85\x01aP4V[` \x82\x01R`@\x83\x015aX\xFC\x81a_ZV[`@\x82\x01RaY\x0E\x89``\x85\x01aM\xB6V[``\x82\x01R`\x80\x83\x015`\x80\x82\x01R`\xA0\x83\x015\x82\x81\x11\x15aY.W\x85\x86\xFD[aY:\x8A\x82\x86\x01aO\xC0V[`\xA0\x83\x01RP\x80\x96PPPPaYS\x86` \x87\x01aP\xF9V[\x93\x96\x93\x95PPPP`\xA0\x82\x015\x91`\xC0\x015\x90V[`\0` \x82\x84\x03\x12\x15aYyW\x80\x81\xFD[PQ\x91\x90PV[`\x01`\x01`\xA0\x1B\x03\x16\x90RV[`\0\x81Q\x80\x84R` \x80\x85\x01\x94P\x80\x84\x01\x83[\x83\x81\x10\x15aY\xC5W\x81Q`\x01`\x01`\xA0\x1B\x03\x16\x87R\x95\x82\x01\x95\x90\x82\x01\x90`\x01\x01aY\xA0V[P\x94\x95\x94PPPPPV[`\0\x81Q\x80\x84R` \x80\x85\x01\x94P\x80\x84\x01\x83[\x83\x81\x10\x15aY\xC5W\x81Q\x87R\x95\x82\x01\x95\x90\x82\x01\x90`\x01\x01aY\xE3V[`\0\x81Q\x80\x84RaZ\x17\x81` \x86\x01` \x86\x01a_$V[`\x1F\x01`\x1F\x19\x16\x92\x90\x92\x01` \x01\x92\x91PPV[`\0a\x01 \x82Q`\x02\x81\x10aZ<W\xFE[\x80\x85RP` \x83\x01QaZR` \x86\x01\x82aY\x80V[P`@\x83\x01QaZe`@\x86\x01\x82aY\x80V[P``\x83\x01Q``\x85\x01R`\x80\x83\x01Q`\x80\x85\x01R`\xA0\x83\x01Q`\xA0\x85\x01R`\xC0\x83\x01QaZ\x96`\xC0\x86\x01\x82aY\x80V[P`\xE0\x83\x01QaZ\xA9`\xE0\x86\x01\x82aY\x80V[Pa\x01\0\x80\x84\x01Q\x82\x82\x87\x01Ra\x12\xA6\x83\x87\x01\x82aY\xFFV[\x91\x82R`\x01`\x01`\xE0\x1B\x03\x19\x16` \x82\x01R`$\x01\x90V[`\0\x82\x84\x837\x91\x01\x90\x81R\x91\x90PV[`\0\x82QaZ\xFC\x81\x84` \x87\x01a_$V[\x91\x90\x91\x01\x92\x91PPV[k\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x19``\x93\x84\x1B\x81\x16\x82R\x91\x90\x92\x1B\x16`\x14\x82\x01R`(\x01\x90V[\x7F\x19\x01\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x02\x81\x01\x92\x90\x92R`\"\x82\x01R`B\x01\x90V[`\x01`\x01`\xA0\x1B\x03\x91\x90\x91\x16\x81R` \x01\x90V[`\x01`\x01`\xA0\x1B\x03\x93\x84\x16\x81R\x91\x90\x92\x16` \x82\x01R`@\x81\x01\x91\x90\x91R``\x01\x90V[`\x01`\x01`\xA0\x1B\x03\x83\x16\x81R`@\x81\x01a[\xB4\x83a_PV[\x82` \x83\x01R\x93\x92PPPV[`\x01`\x01`\xA0\x1B\x03\x92\x90\x92\x16\x82R` \x82\x01R`@\x01\x90V[`\0` \x82Ra\x16\xF2` \x83\x01\x84aY\x8DV[`\0`@\x82Ra\\\0`@\x83\x01\x85aY\x8DV[\x82\x81\x03` \x84\x81\x01\x91\x90\x91R\x84Q\x80\x83R\x85\x82\x01\x92\x82\x01\x90\x84[\x81\x81\x10\x15a\\?W\x84Q`\x01`\x01`\xA0\x1B\x03\x16\x83R\x93\x83\x01\x93\x91\x83\x01\x91`\x01\x01a\\\x1AV[P\x90\x97\x96PPPPPPPV[`\0``\x82Ra\\_``\x83\x01\x86aY\x8DV[\x82\x81\x03` \x84\x01Ra\\q\x81\x86aY\xD0V[\x90P\x82\x81\x03`@\x84\x01Ra\x12\xA6\x81\x85aY\xD0V[`\0`\x80\x82Ra\\\x98`\x80\x83\x01\x87aY\x8DV[\x82\x81\x03` \x84\x01Ra\\\xAA\x81\x87aY\xD0V[\x90P\x82\x81\x03`@\x84\x01Ra\\\xBE\x81\x86aY\xD0V[\x90P\x82\x81\x03``\x84\x01RaL\x08\x81\x85aY\xFFV[`\0``\x82Ra\\\xE5``\x83\x01\x86aY\x8DV[\x82\x81\x03` \x84\x01Ra\\\xF7\x81\x86aY\xD0V[\x91PP\x82`@\x83\x01R\x94\x93PPPPV[`\0` \x82Ra\x16\xF2` \x83\x01\x84aY\xD0V[\x90\x15\x15\x81R` \x01\x90V[\x92\x15\x15\x83R` \x83\x01\x91\x90\x91R`@\x82\x01R``\x01\x90V[\x90\x81R` \x01\x90V[\x92\x83R`\x01`\x01`\xA0\x1B\x03\x91\x82\x16` \x84\x01R\x16`@\x82\x01R``\x01\x90V[`\0\x88\x82R`\x01`\x01`\xA0\x1B\x03\x80\x89\x16` \x84\x01R\x80\x88\x16`@\x84\x01RP`\xE0``\x83\x01Ra]\x98`\xE0\x83\x01\x87aY\xD0V[\x85`\x80\x84\x01R\x84`\xA0\x84\x01R\x82\x81\x03`\xC0\x84\x01Ra]\xB6\x81\x85aY\xFFV[\x9A\x99PPPPPPPPPPV[\x94\x85R` \x85\x01\x93\x90\x93R`\x01`\x01`\xA0\x1B\x03\x91\x90\x91\x16`@\x84\x01R``\x83\x01R`\x80\x82\x01R`\xA0\x01\x90V[\x94\x85R` \x85\x01\x93\x90\x93R`@\x84\x01\x91\x90\x91R``\x83\x01R`\x01`\x01`\xA0\x1B\x03\x16`\x80\x82\x01R`\xA0\x01\x90V[\x93\x84R`\xFF\x92\x90\x92\x16` \x84\x01R`@\x83\x01R``\x82\x01R`\x80\x01\x90V[` \x81\x01a^G\x83a_PV[\x91\x90R\x90V[\x91\x82R` \x82\x01R`@\x01\x90V[`\0`\x80\x82Ra^n`\x80\x83\x01\x87aZ+V[\x82\x81\x03` \x84\x01Ra^\x80\x81\x87aY\xD0V[`@\x84\x01\x95\x90\x95RPP``\x01R\x92\x91PPV[`\0``\x82Ra^\xA7``\x83\x01\x86aZ+V[` \x83\x01\x94\x90\x94RP`@\x01R\x91\x90PV[\x93\x84R` \x84\x01\x92\x90\x92R`@\x83\x01R`\x01`\x01`\xA0\x1B\x03\x16``\x82\x01R`\x80\x01\x90V[`@Q\x81\x81\x01g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x82\x82\x10\x17\x15a^\xFCW`\0\x80\xFD[`@R\x91\x90PV[`\0g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11\x15a_\x1AW\x80\x81\xFD[P` \x90\x81\x02\x01\x90V[`\0[\x83\x81\x10\x15a_?W\x81\x81\x01Q\x83\x82\x01R` \x01a_'V[\x83\x81\x11\x15a\x0E\0WPP`\0\x91\x01RV[`\x03\x81\x10a\x05~W\xFE[`\x01`\x01`\xA0\x1B\x03\x81\x16\x81\x14a\x05~W`\0\x80\xFD[\x80\x15\x15\x81\x14a\x05~W`\0\x80\xFD[`\x03\x81\x10a\x05~W`\0\x80\xFD\xFE\xA2dipfsX\"\x12  \x1EO\x92n9\x0F\xED\x8D\xD51\x8CX\x84j\xF75\xC2\xBE\xBCa\xB8\x06\x93\xAE\x93j_\xE7m\xCF\x14dsolcC\0\x07\x01\x003`\xC0`@R4\x80\x15a\0\x10W`\0\x80\xFD[P`@Qa\x0B\xE68\x03\x80a\x0B\xE6\x839\x81\x01`@\x81\x90Ra\0/\x91a\0MV[0`\x80R`\x01`\0U``\x1B`\x01`\x01``\x1B\x03\x19\x16`\xA0Ra\0{V[`\0` \x82\x84\x03\x12\x15a\0^W\x80\x81\xFD[\x81Q`\x01`\x01`\xA0\x1B\x03\x81\x16\x81\x14a\0tW\x81\x82\xFD[\x93\x92PPPV[`\x80Q`\xA0Q``\x1Ca\x0B@a\0\xA6`\09\x80a\x04\x13R\x80a\x05IRP\x80a\x02\xA7RPa\x0B@`\0\xF3\xFE`\x80`@R4\x80\x15a\0\x10W`\0\x80\xFD[P`\x046\x10a\0\xA3W`\x005`\xE0\x1C\x80c\x85\x1C\x1B\xB3\x11a\0vW\x80c\xD8w\x84\\\x11a\0[W\x80c\xD8w\x84\\\x14a\x01)W\x80c\xE4*\xBF5\x14a\x011W\x80c\xFB\xFAw\xCF\x14a\x01QWa\0\xA3V[\x80c\x85\x1C\x1B\xB3\x14a\x01\x01W\x80c\xAA\xAB\xAD\xC5\x14a\x01\x14Wa\0\xA3V[\x80c8\xE9\x92.\x14a\0\xA8W\x80cU\xC6v(\x14a\0\xBDW\x80ckk\x9Fi\x14a\0\xDBW\x80cm\xAE\xFA\xB6\x14a\0\xEEW[`\0\x80\xFD[a\0\xBBa\0\xB66`\x04a\t\x9CV[a\x01YV[\0[a\0\xC5a\x01\xB8V[`@Qa\0\xD2\x91\x90a\n\xA6V[`@Q\x80\x91\x03\x90\xF3[a\0\xBBa\0\xE96`\x04a\t\x9CV[a\x01\xBEV[a\0\xBBa\0\xFC6`\x04a\x07\xD1V[a\x02\x11V[a\0\xC5a\x01\x0F6`\x04a\t$V[a\x02\xA3V[a\x01\x1Ca\x02\xF5V[`@Qa\0\xD2\x91\x90a\n5V[a\0\xC5a\x03\x04V[a\x01Da\x01?6`\x04a\x08RV[a\x03\nV[`@Qa\0\xD2\x91\x90a\nbV[a\x01\x1Ca\x04\x11V[a\x01aa\x045V[a\x01xg\x06\xF0[Y\xD3\xB2\0\0\x82\x11\x15a\x02Xa\x04~V[`\x01\x81\x90U`@Q\x7F\xA9\xBA?\xFE\x0Bl6k\x81#,\xAA\xB3\x86\x05\xA0i\x9A\xD59\x8Dl\xCEv\xF9\x1E\xE8\t\xE3\"\xDA\xFC\x90a\x01\xAD\x90\x83\x90a\n\xA6V[`@Q\x80\x91\x03\x90\xA1PV[`\x01T\x90V[a\x01\xC6a\x045V[a\x01\xDCf#\x86\xF2o\xC1\0\0\x82\x11\x15a\x02Ya\x04~V[`\x02\x81\x90U`@Q\x7FZ\x0Bs\x86#~\x7F\x07\xFAt\x1E\xFCd\xE5\x9C\x93\x87\xD2\xCC\xCA\xFE\xC7`\xEF\xEDMS8\x7F \xE1\x9A\x90a\x01\xAD\x90\x83\x90a\n\xA6V[a\x02\x19a\x04\x90V[a\x02!a\x045V[a\x02+\x84\x83a\x04\xA9V[`\0[\x84\x81\x10\x15a\x02\x93W`\0\x86\x86\x83\x81\x81\x10a\x02DW\xFE[\x90P` \x02\x01` \x81\x01\x90a\x02Y\x91\x90a\t\x80V[\x90P`\0\x85\x85\x84\x81\x81\x10a\x02iW\xFE[` \x02\x91\x90\x91\x015\x91Pa\x02\x89\x90P`\x01`\x01`\xA0\x1B\x03\x83\x16\x85\x83a\x04\xB6V[PP`\x01\x01a\x02.V[Pa\x02\x9Ca\x05>V[PPPPPV[`\0\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x82`@Q` \x01a\x02\xD8\x92\x91\x90a\t\xCCV[`@Q` \x81\x83\x03\x03\x81R\x90`@R\x80Q\x90` \x01 \x90P\x91\x90PV[`\0a\x02\xFFa\x05EV[\x90P\x90V[`\x02T\x90V[``\x81Qg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x80\x15a\x03$W`\0\x80\xFD[P`@Q\x90\x80\x82R\x80` \x02` \x01\x82\x01`@R\x80\x15a\x03NW\x81` \x01` \x82\x02\x806\x837\x01\x90P[P\x90P`\0[\x82Q\x81\x10\x15a\x04\x0BW\x82\x81\x81Q\x81\x10a\x03iW\xFE[` \x02` \x01\x01Q`\x01`\x01`\xA0\x1B\x03\x16cp\xA0\x8210`@Q\x82c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01a\x03\x9C\x91\x90a\n5V[` `@Q\x80\x83\x03\x81\x86\x80;\x15\x80\x15a\x03\xB4W`\0\x80\xFD[PZ\xFA\x15\x80\x15a\x03\xC8W=`\0\x80>=`\0\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x03\xEC\x91\x90a\t\xB4V[\x82\x82\x81Q\x81\x10a\x03\xF8W\xFE[` \x90\x81\x02\x91\x90\x91\x01\x01R`\x01\x01a\x03TV[P\x91\x90PV[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[`\0a\x04d`\x005\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16a\x02\xA3V[\x90Pa\x04{a\x04s\x823a\x05\xD8V[a\x01\x91a\x04~V[PV[\x81a\x04\x8CWa\x04\x8C\x81a\x06jV[PPV[a\x04\xA2`\x02`\0T\x14\x15a\x01\x90a\x04~V[`\x02`\0UV[a\x04\x8C\x81\x83\x14`ga\x04~V[a\x059\x83c\xA9\x05\x9C\xBB`\xE0\x1B\x84\x84`@Q`$\x01a\x04\xD5\x92\x91\x90a\nIV[`@\x80Q`\x1F\x19\x81\x84\x03\x01\x81R\x91\x90R` \x81\x01\x80Q{\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90\x93\x16\x92\x90\x92\x17\x90\x91Ra\x06\xD7V[PPPV[`\x01`\0UV[`\0\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\x01`\x01`\xA0\x1B\x03\x16c\xAA\xAB\xAD\xC5`@Q\x81c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01` `@Q\x80\x83\x03\x81\x86\x80;\x15\x80\x15a\x05\xA0W`\0\x80\xFD[PZ\xFA\x15\x80\x15a\x05\xB4W=`\0\x80>=`\0\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x02\xFF\x91\x90a\tdV[`\0a\x05\xE2a\x05EV[`\x01`\x01`\xA0\x1B\x03\x16c\x9B\xE2\xA8\x84\x84\x840`@Q\x84c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01a\x06\x11\x93\x92\x91\x90a\n\xAFV[` `@Q\x80\x83\x03\x81\x86\x80;\x15\x80\x15a\x06)W`\0\x80\xFD[PZ\xFA\x15\x80\x15a\x06=W=`\0\x80>=`\0\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x06a\x91\x90a\x08\xFDV[\x90P[\x92\x91PPV[\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\0\x90\x81R` `\x04R`\x07`$RfBAL#\0\x000`\n\x80\x84\x04\x81\x81\x06`0\x90\x81\x01`\x08\x1B\x95\x83\x90\x06\x95\x90\x95\x01\x90\x82\x90\x04\x91\x82\x06\x90\x94\x01`\x10\x1B\x93\x90\x93\x01\x01`\xC8\x1B`DR`d\x90\xFD[`\0``\x83`\x01`\x01`\xA0\x1B\x03\x16\x83`@Qa\x06\xF3\x91\x90a\t\xFCV[`\0`@Q\x80\x83\x03\x81`\0\x86Z\xF1\x91PP=\x80`\0\x81\x14a\x070W`@Q\x91P`\x1F\x19`?=\x01\x16\x82\x01`@R=\x82R=`\0` \x84\x01>a\x075V[``\x91P[P\x91P\x91P`\0\x82\x14\x15a\x07MW=`\0\x80>=`\0\xFD[a\x07w\x81Q`\0\x14\x80a\x07oWP\x81\x80` \x01\x90Q\x81\x01\x90a\x07o\x91\x90a\x08\xFDV[a\x01\xA2a\x04~V[PPPPV[`\0\x80\x83`\x1F\x84\x01\x12a\x07\x8EW\x81\x82\xFD[P\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x07\xA5W\x81\x82\xFD[` \x83\x01\x91P\x83` \x80\x83\x02\x85\x01\x01\x11\x15a\x07\xBFW`\0\x80\xFD[\x92P\x92\x90PV[\x805a\x06d\x81a\n\xF5V[`\0\x80`\0\x80`\0``\x86\x88\x03\x12\x15a\x07\xE8W\x80\x81\xFD[\x855g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a\x07\xFFW\x82\x83\xFD[a\x08\x0B\x89\x83\x8A\x01a\x07}V[\x90\x97P\x95P` \x88\x015\x91P\x80\x82\x11\x15a\x08#W\x82\x83\xFD[Pa\x080\x88\x82\x89\x01a\x07}V[\x90\x94P\x92PP`@\x86\x015a\x08D\x81a\n\xF5V[\x80\x91PP\x92\x95P\x92\x95\x90\x93PV[`\0` \x80\x83\x85\x03\x12\x15a\x08dW\x81\x82\xFD[\x825g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a\x08{W\x83\x84\xFD[\x81\x85\x01\x91P\x85`\x1F\x83\x01\x12a\x08\x8EW\x83\x84\xFD[\x815\x81\x81\x11\x15a\x08\x9CW\x84\x85\xFD[\x83\x81\x02\x91Pa\x08\xAC\x84\x83\x01a\n\xCEV[\x81\x81R\x84\x81\x01\x90\x84\x86\x01\x84\x86\x01\x87\x01\x8A\x10\x15a\x08\xC6W\x87\x88\xFD[\x87\x95P[\x83\x86\x10\x15a\x08\xF0Wa\x08\xDC\x8A\x82a\x07\xC6V[\x83R`\x01\x95\x90\x95\x01\x94\x91\x86\x01\x91\x86\x01a\x08\xCAV[P\x98\x97PPPPPPPPV[`\0` \x82\x84\x03\x12\x15a\t\x0EW\x80\x81\xFD[\x81Q\x80\x15\x15\x81\x14a\t\x1DW\x81\x82\xFD[\x93\x92PPPV[`\0` \x82\x84\x03\x12\x15a\t5W\x80\x81\xFD[\x815\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81\x16\x81\x14a\t\x1DW\x81\x82\xFD[`\0` \x82\x84\x03\x12\x15a\tuW\x80\x81\xFD[\x81Qa\t\x1D\x81a\n\xF5V[`\0` \x82\x84\x03\x12\x15a\t\x91W\x80\x81\xFD[\x815a\t\x1D\x81a\n\xF5V[`\0` \x82\x84\x03\x12\x15a\t\xADW\x80\x81\xFD[P5\x91\x90PV[`\0` \x82\x84\x03\x12\x15a\t\xC5W\x80\x81\xFD[PQ\x91\x90PV[\x91\x82R\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16` \x82\x01R`$\x01\x90V[`\0\x82Q\x81[\x81\x81\x10\x15a\n\x1CW` \x81\x86\x01\x81\x01Q\x85\x83\x01R\x01a\n\x02V[\x81\x81\x11\x15a\n*W\x82\x82\x85\x01R[P\x91\x90\x91\x01\x92\x91PPV[`\x01`\x01`\xA0\x1B\x03\x91\x90\x91\x16\x81R` \x01\x90V[`\x01`\x01`\xA0\x1B\x03\x92\x90\x92\x16\x82R` \x82\x01R`@\x01\x90V[` \x80\x82R\x82Q\x82\x82\x01\x81\x90R`\0\x91\x90\x84\x82\x01\x90`@\x85\x01\x90\x84[\x81\x81\x10\x15a\n\x9AW\x83Q\x83R\x92\x84\x01\x92\x91\x84\x01\x91`\x01\x01a\n~V[P\x90\x96\x95PPPPPPV[\x90\x81R` \x01\x90V[\x92\x83R`\x01`\x01`\xA0\x1B\x03\x91\x82\x16` \x84\x01R\x16`@\x82\x01R``\x01\x90V[`@Q\x81\x81\x01g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x82\x82\x10\x17\x15a\n\xEDW`\0\x80\xFD[`@R\x91\x90PV[`\x01`\x01`\xA0\x1B\x03\x81\x16\x81\x14a\x04{W`\0\x80\xFD\xFE\xA2dipfsX\"\x12 \xBEr\xBD\xF8\xE7\xA3\xC3\x86\x06\xC5\xF9T\xFB\xE2\xD7w\x984z\xAA\x1C\xFBv\xFEw\xEC/l$]$\xBCdsolcC\0\x07\x01\x003",
    );
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `AuthorizerChanged(address)` and selector `0x94b979b6831a51293e2641426f97747feed46f17779fed9cd18d1ecefcfe92ef`.
```solidity
event AuthorizerChanged(address indexed newAuthorizer);
```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct AuthorizerChanged {
        #[allow(missing_docs)]
        pub newAuthorizer: alloy_sol_types::private::Address,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::SolEvent for AuthorizerChanged {
            type DataTuple<'a> = ();
            type DataToken<'a> = <Self::DataTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "AuthorizerChanged(address)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 = alloy_sol_types::private::B256::new([
                148u8, 185u8, 121u8, 182u8, 131u8, 26u8, 81u8, 41u8, 62u8, 38u8, 65u8,
                66u8, 111u8, 151u8, 116u8, 127u8, 238u8, 212u8, 111u8, 23u8, 119u8,
                159u8, 237u8, 156u8, 209u8, 141u8, 30u8, 206u8, 252u8, 254u8, 146u8,
                239u8,
            ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self { newAuthorizer: topics.1 }
            }
            #[inline]
            fn check_signature(
                topics: &<Self::TopicList as alloy_sol_types::SolType>::RustType,
            ) -> alloy_sol_types::Result<()> {
                if topics.0 != Self::SIGNATURE_HASH {
                    return Err(
                        alloy_sol_types::Error::invalid_event_signature_hash(
                            Self::SIGNATURE,
                            topics.0,
                            Self::SIGNATURE_HASH,
                        ),
                    );
                }
                Ok(())
            }
            #[inline]
            fn tokenize_body(&self) -> Self::DataToken<'_> {
                ()
            }
            #[inline]
            fn topics(&self) -> <Self::TopicList as alloy_sol_types::SolType>::RustType {
                (Self::SIGNATURE_HASH.into(), self.newAuthorizer.clone())
            }
            #[inline]
            fn encode_topics_raw(
                &self,
                out: &mut [alloy_sol_types::abi::token::WordToken],
            ) -> alloy_sol_types::Result<()> {
                if out.len() < <Self::TopicList as alloy_sol_types::TopicList>::COUNT {
                    return Err(alloy_sol_types::Error::Overrun);
                }
                out[0usize] = alloy_sol_types::abi::token::WordToken(
                    Self::SIGNATURE_HASH,
                );
                out[1usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.newAuthorizer,
                );
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for AuthorizerChanged {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&AuthorizerChanged> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &AuthorizerChanged) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `ExternalBalanceTransfer(address,address,address,uint256)` and selector `0x540a1a3f28340caec336c81d8d7b3df139ee5cdc1839a4f283d7ebb7eaae2d5c`.
```solidity
event ExternalBalanceTransfer(address indexed token, address indexed sender, address recipient, uint256 amount);
```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct ExternalBalanceTransfer {
        #[allow(missing_docs)]
        pub token: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub sender: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub recipient: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub amount: alloy_sol_types::private::primitives::aliases::U256,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::SolEvent for ExternalBalanceTransfer {
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type DataToken<'a> = <Self::DataTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "ExternalBalanceTransfer(address,address,address,uint256)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 = alloy_sol_types::private::B256::new([
                84u8, 10u8, 26u8, 63u8, 40u8, 52u8, 12u8, 174u8, 195u8, 54u8, 200u8,
                29u8, 141u8, 123u8, 61u8, 241u8, 57u8, 238u8, 92u8, 220u8, 24u8, 57u8,
                164u8, 242u8, 131u8, 215u8, 235u8, 183u8, 234u8, 174u8, 45u8, 92u8,
            ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    token: topics.1,
                    sender: topics.2,
                    recipient: data.0,
                    amount: data.1,
                }
            }
            #[inline]
            fn check_signature(
                topics: &<Self::TopicList as alloy_sol_types::SolType>::RustType,
            ) -> alloy_sol_types::Result<()> {
                if topics.0 != Self::SIGNATURE_HASH {
                    return Err(
                        alloy_sol_types::Error::invalid_event_signature_hash(
                            Self::SIGNATURE,
                            topics.0,
                            Self::SIGNATURE_HASH,
                        ),
                    );
                }
                Ok(())
            }
            #[inline]
            fn tokenize_body(&self) -> Self::DataToken<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.recipient,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.amount),
                )
            }
            #[inline]
            fn topics(&self) -> <Self::TopicList as alloy_sol_types::SolType>::RustType {
                (Self::SIGNATURE_HASH.into(), self.token.clone(), self.sender.clone())
            }
            #[inline]
            fn encode_topics_raw(
                &self,
                out: &mut [alloy_sol_types::abi::token::WordToken],
            ) -> alloy_sol_types::Result<()> {
                if out.len() < <Self::TopicList as alloy_sol_types::TopicList>::COUNT {
                    return Err(alloy_sol_types::Error::Overrun);
                }
                out[0usize] = alloy_sol_types::abi::token::WordToken(
                    Self::SIGNATURE_HASH,
                );
                out[1usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.token,
                );
                out[2usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.sender,
                );
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for ExternalBalanceTransfer {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&ExternalBalanceTransfer> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(
                this: &ExternalBalanceTransfer,
            ) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `FlashLoan(address,address,uint256,uint256)` and selector `0x0d7d75e01ab95780d3cd1c8ec0dd6c2ce19e3a20427eec8bf53283b6fb8e95f0`.
```solidity
event FlashLoan(address indexed recipient, address indexed token, uint256 amount, uint256 feeAmount);
```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct FlashLoan {
        #[allow(missing_docs)]
        pub recipient: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub token: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub amount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub feeAmount: alloy_sol_types::private::primitives::aliases::U256,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::SolEvent for FlashLoan {
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type DataToken<'a> = <Self::DataTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "FlashLoan(address,address,uint256,uint256)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 = alloy_sol_types::private::B256::new([
                13u8, 125u8, 117u8, 224u8, 26u8, 185u8, 87u8, 128u8, 211u8, 205u8, 28u8,
                142u8, 192u8, 221u8, 108u8, 44u8, 225u8, 158u8, 58u8, 32u8, 66u8, 126u8,
                236u8, 139u8, 245u8, 50u8, 131u8, 182u8, 251u8, 142u8, 149u8, 240u8,
            ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    recipient: topics.1,
                    token: topics.2,
                    amount: data.0,
                    feeAmount: data.1,
                }
            }
            #[inline]
            fn check_signature(
                topics: &<Self::TopicList as alloy_sol_types::SolType>::RustType,
            ) -> alloy_sol_types::Result<()> {
                if topics.0 != Self::SIGNATURE_HASH {
                    return Err(
                        alloy_sol_types::Error::invalid_event_signature_hash(
                            Self::SIGNATURE,
                            topics.0,
                            Self::SIGNATURE_HASH,
                        ),
                    );
                }
                Ok(())
            }
            #[inline]
            fn tokenize_body(&self) -> Self::DataToken<'_> {
                (
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.amount),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.feeAmount),
                )
            }
            #[inline]
            fn topics(&self) -> <Self::TopicList as alloy_sol_types::SolType>::RustType {
                (Self::SIGNATURE_HASH.into(), self.recipient.clone(), self.token.clone())
            }
            #[inline]
            fn encode_topics_raw(
                &self,
                out: &mut [alloy_sol_types::abi::token::WordToken],
            ) -> alloy_sol_types::Result<()> {
                if out.len() < <Self::TopicList as alloy_sol_types::TopicList>::COUNT {
                    return Err(alloy_sol_types::Error::Overrun);
                }
                out[0usize] = alloy_sol_types::abi::token::WordToken(
                    Self::SIGNATURE_HASH,
                );
                out[1usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.recipient,
                );
                out[2usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.token,
                );
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for FlashLoan {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&FlashLoan> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &FlashLoan) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `InternalBalanceChanged(address,address,int256)` and selector `0x18e1ea4139e68413d7d08aa752e71568e36b2c5bf940893314c2c5b01eaa0c42`.
```solidity
event InternalBalanceChanged(address indexed user, address indexed token, int256 delta);
```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct InternalBalanceChanged {
        #[allow(missing_docs)]
        pub user: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub token: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub delta: alloy_sol_types::private::primitives::aliases::I256,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::SolEvent for InternalBalanceChanged {
            type DataTuple<'a> = (alloy_sol_types::sol_data::Int<256>,);
            type DataToken<'a> = <Self::DataTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "InternalBalanceChanged(address,address,int256)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 = alloy_sol_types::private::B256::new([
                24u8, 225u8, 234u8, 65u8, 57u8, 230u8, 132u8, 19u8, 215u8, 208u8, 138u8,
                167u8, 82u8, 231u8, 21u8, 104u8, 227u8, 107u8, 44u8, 91u8, 249u8, 64u8,
                137u8, 51u8, 20u8, 194u8, 197u8, 176u8, 30u8, 170u8, 12u8, 66u8,
            ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    user: topics.1,
                    token: topics.2,
                    delta: data.0,
                }
            }
            #[inline]
            fn check_signature(
                topics: &<Self::TopicList as alloy_sol_types::SolType>::RustType,
            ) -> alloy_sol_types::Result<()> {
                if topics.0 != Self::SIGNATURE_HASH {
                    return Err(
                        alloy_sol_types::Error::invalid_event_signature_hash(
                            Self::SIGNATURE,
                            topics.0,
                            Self::SIGNATURE_HASH,
                        ),
                    );
                }
                Ok(())
            }
            #[inline]
            fn tokenize_body(&self) -> Self::DataToken<'_> {
                (
                    <alloy_sol_types::sol_data::Int<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.delta),
                )
            }
            #[inline]
            fn topics(&self) -> <Self::TopicList as alloy_sol_types::SolType>::RustType {
                (Self::SIGNATURE_HASH.into(), self.user.clone(), self.token.clone())
            }
            #[inline]
            fn encode_topics_raw(
                &self,
                out: &mut [alloy_sol_types::abi::token::WordToken],
            ) -> alloy_sol_types::Result<()> {
                if out.len() < <Self::TopicList as alloy_sol_types::TopicList>::COUNT {
                    return Err(alloy_sol_types::Error::Overrun);
                }
                out[0usize] = alloy_sol_types::abi::token::WordToken(
                    Self::SIGNATURE_HASH,
                );
                out[1usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.user,
                );
                out[2usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.token,
                );
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for InternalBalanceChanged {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&InternalBalanceChanged> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &InternalBalanceChanged) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `PausedStateChanged(bool)` and selector `0x9e3a5e37224532dea67b89face185703738a228a6e8a23dee546960180d3be64`.
```solidity
event PausedStateChanged(bool paused);
```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct PausedStateChanged {
        #[allow(missing_docs)]
        pub paused: bool,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::SolEvent for PausedStateChanged {
            type DataTuple<'a> = (alloy_sol_types::sol_data::Bool,);
            type DataToken<'a> = <Self::DataTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);
            const SIGNATURE: &'static str = "PausedStateChanged(bool)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 = alloy_sol_types::private::B256::new([
                158u8, 58u8, 94u8, 55u8, 34u8, 69u8, 50u8, 222u8, 166u8, 123u8, 137u8,
                250u8, 206u8, 24u8, 87u8, 3u8, 115u8, 138u8, 34u8, 138u8, 110u8, 138u8,
                35u8, 222u8, 229u8, 70u8, 150u8, 1u8, 128u8, 211u8, 190u8, 100u8,
            ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self { paused: data.0 }
            }
            #[inline]
            fn check_signature(
                topics: &<Self::TopicList as alloy_sol_types::SolType>::RustType,
            ) -> alloy_sol_types::Result<()> {
                if topics.0 != Self::SIGNATURE_HASH {
                    return Err(
                        alloy_sol_types::Error::invalid_event_signature_hash(
                            Self::SIGNATURE,
                            topics.0,
                            Self::SIGNATURE_HASH,
                        ),
                    );
                }
                Ok(())
            }
            #[inline]
            fn tokenize_body(&self) -> Self::DataToken<'_> {
                (
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::tokenize(
                        &self.paused,
                    ),
                )
            }
            #[inline]
            fn topics(&self) -> <Self::TopicList as alloy_sol_types::SolType>::RustType {
                (Self::SIGNATURE_HASH.into(),)
            }
            #[inline]
            fn encode_topics_raw(
                &self,
                out: &mut [alloy_sol_types::abi::token::WordToken],
            ) -> alloy_sol_types::Result<()> {
                if out.len() < <Self::TopicList as alloy_sol_types::TopicList>::COUNT {
                    return Err(alloy_sol_types::Error::Overrun);
                }
                out[0usize] = alloy_sol_types::abi::token::WordToken(
                    Self::SIGNATURE_HASH,
                );
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for PausedStateChanged {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&PausedStateChanged> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &PausedStateChanged) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `PoolBalanceChanged(bytes32,address,address[],int256[],uint256[])` and selector `0xe5ce249087ce04f05a957192435400fd97868dba0e6a4b4c049abf8af80dae78`.
```solidity
event PoolBalanceChanged(bytes32 indexed poolId, address indexed liquidityProvider, address[] tokens, int256[] deltas, uint256[] protocolFeeAmounts);
```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct PoolBalanceChanged {
        #[allow(missing_docs)]
        pub poolId: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub liquidityProvider: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub tokens: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
        #[allow(missing_docs)]
        pub deltas: alloy_sol_types::private::Vec<
            alloy_sol_types::private::primitives::aliases::I256,
        >,
        #[allow(missing_docs)]
        pub protocolFeeAmounts: alloy_sol_types::private::Vec<
            alloy_sol_types::private::primitives::aliases::U256,
        >,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::SolEvent for PoolBalanceChanged {
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Int<256>>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
            );
            type DataToken<'a> = <Self::DataTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "PoolBalanceChanged(bytes32,address,address[],int256[],uint256[])";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 = alloy_sol_types::private::B256::new([
                229u8, 206u8, 36u8, 144u8, 135u8, 206u8, 4u8, 240u8, 90u8, 149u8, 113u8,
                146u8, 67u8, 84u8, 0u8, 253u8, 151u8, 134u8, 141u8, 186u8, 14u8, 106u8,
                75u8, 76u8, 4u8, 154u8, 191u8, 138u8, 248u8, 13u8, 174u8, 120u8,
            ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    poolId: topics.1,
                    liquidityProvider: topics.2,
                    tokens: data.0,
                    deltas: data.1,
                    protocolFeeAmounts: data.2,
                }
            }
            #[inline]
            fn check_signature(
                topics: &<Self::TopicList as alloy_sol_types::SolType>::RustType,
            ) -> alloy_sol_types::Result<()> {
                if topics.0 != Self::SIGNATURE_HASH {
                    return Err(
                        alloy_sol_types::Error::invalid_event_signature_hash(
                            Self::SIGNATURE,
                            topics.0,
                            Self::SIGNATURE_HASH,
                        ),
                    );
                }
                Ok(())
            }
            #[inline]
            fn tokenize_body(&self) -> Self::DataToken<'_> {
                (
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Address,
                    > as alloy_sol_types::SolType>::tokenize(&self.tokens),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Int<256>,
                    > as alloy_sol_types::SolType>::tokenize(&self.deltas),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::SolType>::tokenize(&self.protocolFeeAmounts),
                )
            }
            #[inline]
            fn topics(&self) -> <Self::TopicList as alloy_sol_types::SolType>::RustType {
                (
                    Self::SIGNATURE_HASH.into(),
                    self.poolId.clone(),
                    self.liquidityProvider.clone(),
                )
            }
            #[inline]
            fn encode_topics_raw(
                &self,
                out: &mut [alloy_sol_types::abi::token::WordToken],
            ) -> alloy_sol_types::Result<()> {
                if out.len() < <Self::TopicList as alloy_sol_types::TopicList>::COUNT {
                    return Err(alloy_sol_types::Error::Overrun);
                }
                out[0usize] = alloy_sol_types::abi::token::WordToken(
                    Self::SIGNATURE_HASH,
                );
                out[1usize] = <alloy_sol_types::sol_data::FixedBytes<
                    32,
                > as alloy_sol_types::EventTopic>::encode_topic(&self.poolId);
                out[2usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.liquidityProvider,
                );
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for PoolBalanceChanged {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&PoolBalanceChanged> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &PoolBalanceChanged) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `PoolBalanceManaged(bytes32,address,address,int256,int256)` and selector `0x6edcaf6241105b4c94c2efdbf3a6b12458eb3d07be3a0e81d24b13c44045fe7a`.
```solidity
event PoolBalanceManaged(bytes32 indexed poolId, address indexed assetManager, address indexed token, int256 cashDelta, int256 managedDelta);
```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct PoolBalanceManaged {
        #[allow(missing_docs)]
        pub poolId: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub assetManager: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub token: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub cashDelta: alloy_sol_types::private::primitives::aliases::I256,
        #[allow(missing_docs)]
        pub managedDelta: alloy_sol_types::private::primitives::aliases::I256,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::SolEvent for PoolBalanceManaged {
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::Int<256>,
                alloy_sol_types::sol_data::Int<256>,
            );
            type DataToken<'a> = <Self::DataTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "PoolBalanceManaged(bytes32,address,address,int256,int256)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 = alloy_sol_types::private::B256::new([
                110u8, 220u8, 175u8, 98u8, 65u8, 16u8, 91u8, 76u8, 148u8, 194u8, 239u8,
                219u8, 243u8, 166u8, 177u8, 36u8, 88u8, 235u8, 61u8, 7u8, 190u8, 58u8,
                14u8, 129u8, 210u8, 75u8, 19u8, 196u8, 64u8, 69u8, 254u8, 122u8,
            ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    poolId: topics.1,
                    assetManager: topics.2,
                    token: topics.3,
                    cashDelta: data.0,
                    managedDelta: data.1,
                }
            }
            #[inline]
            fn check_signature(
                topics: &<Self::TopicList as alloy_sol_types::SolType>::RustType,
            ) -> alloy_sol_types::Result<()> {
                if topics.0 != Self::SIGNATURE_HASH {
                    return Err(
                        alloy_sol_types::Error::invalid_event_signature_hash(
                            Self::SIGNATURE,
                            topics.0,
                            Self::SIGNATURE_HASH,
                        ),
                    );
                }
                Ok(())
            }
            #[inline]
            fn tokenize_body(&self) -> Self::DataToken<'_> {
                (
                    <alloy_sol_types::sol_data::Int<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.cashDelta),
                    <alloy_sol_types::sol_data::Int<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.managedDelta),
                )
            }
            #[inline]
            fn topics(&self) -> <Self::TopicList as alloy_sol_types::SolType>::RustType {
                (
                    Self::SIGNATURE_HASH.into(),
                    self.poolId.clone(),
                    self.assetManager.clone(),
                    self.token.clone(),
                )
            }
            #[inline]
            fn encode_topics_raw(
                &self,
                out: &mut [alloy_sol_types::abi::token::WordToken],
            ) -> alloy_sol_types::Result<()> {
                if out.len() < <Self::TopicList as alloy_sol_types::TopicList>::COUNT {
                    return Err(alloy_sol_types::Error::Overrun);
                }
                out[0usize] = alloy_sol_types::abi::token::WordToken(
                    Self::SIGNATURE_HASH,
                );
                out[1usize] = <alloy_sol_types::sol_data::FixedBytes<
                    32,
                > as alloy_sol_types::EventTopic>::encode_topic(&self.poolId);
                out[2usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.assetManager,
                );
                out[3usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.token,
                );
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for PoolBalanceManaged {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&PoolBalanceManaged> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &PoolBalanceManaged) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `PoolRegistered(bytes32,address,uint8)` and selector `0x3c13bc30b8e878c53fd2a36b679409c073afd75950be43d8858768e956fbc20e`.
```solidity
event PoolRegistered(bytes32 indexed poolId, address indexed poolAddress, IVault.PoolSpecialization specialization);
```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct PoolRegistered {
        #[allow(missing_docs)]
        pub poolId: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub poolAddress: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub specialization: <IVault::PoolSpecialization as alloy_sol_types::SolType>::RustType,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::SolEvent for PoolRegistered {
            type DataTuple<'a> = (IVault::PoolSpecialization,);
            type DataToken<'a> = <Self::DataTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "PoolRegistered(bytes32,address,uint8)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 = alloy_sol_types::private::B256::new([
                60u8, 19u8, 188u8, 48u8, 184u8, 232u8, 120u8, 197u8, 63u8, 210u8, 163u8,
                107u8, 103u8, 148u8, 9u8, 192u8, 115u8, 175u8, 215u8, 89u8, 80u8, 190u8,
                67u8, 216u8, 133u8, 135u8, 104u8, 233u8, 86u8, 251u8, 194u8, 14u8,
            ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    poolId: topics.1,
                    poolAddress: topics.2,
                    specialization: data.0,
                }
            }
            #[inline]
            fn check_signature(
                topics: &<Self::TopicList as alloy_sol_types::SolType>::RustType,
            ) -> alloy_sol_types::Result<()> {
                if topics.0 != Self::SIGNATURE_HASH {
                    return Err(
                        alloy_sol_types::Error::invalid_event_signature_hash(
                            Self::SIGNATURE,
                            topics.0,
                            Self::SIGNATURE_HASH,
                        ),
                    );
                }
                Ok(())
            }
            #[inline]
            fn tokenize_body(&self) -> Self::DataToken<'_> {
                (
                    <IVault::PoolSpecialization as alloy_sol_types::SolType>::tokenize(
                        &self.specialization,
                    ),
                )
            }
            #[inline]
            fn topics(&self) -> <Self::TopicList as alloy_sol_types::SolType>::RustType {
                (
                    Self::SIGNATURE_HASH.into(),
                    self.poolId.clone(),
                    self.poolAddress.clone(),
                )
            }
            #[inline]
            fn encode_topics_raw(
                &self,
                out: &mut [alloy_sol_types::abi::token::WordToken],
            ) -> alloy_sol_types::Result<()> {
                if out.len() < <Self::TopicList as alloy_sol_types::TopicList>::COUNT {
                    return Err(alloy_sol_types::Error::Overrun);
                }
                out[0usize] = alloy_sol_types::abi::token::WordToken(
                    Self::SIGNATURE_HASH,
                );
                out[1usize] = <alloy_sol_types::sol_data::FixedBytes<
                    32,
                > as alloy_sol_types::EventTopic>::encode_topic(&self.poolId);
                out[2usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.poolAddress,
                );
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for PoolRegistered {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&PoolRegistered> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &PoolRegistered) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `RelayerApprovalChanged(address,address,bool)` and selector `0x46961fdb4502b646d5095fba7600486a8ac05041d55cdf0f16ed677180b5cad8`.
```solidity
event RelayerApprovalChanged(address indexed relayer, address indexed sender, bool approved);
```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct RelayerApprovalChanged {
        #[allow(missing_docs)]
        pub relayer: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub sender: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub approved: bool,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::SolEvent for RelayerApprovalChanged {
            type DataTuple<'a> = (alloy_sol_types::sol_data::Bool,);
            type DataToken<'a> = <Self::DataTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "RelayerApprovalChanged(address,address,bool)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 = alloy_sol_types::private::B256::new([
                70u8, 150u8, 31u8, 219u8, 69u8, 2u8, 182u8, 70u8, 213u8, 9u8, 95u8,
                186u8, 118u8, 0u8, 72u8, 106u8, 138u8, 192u8, 80u8, 65u8, 213u8, 92u8,
                223u8, 15u8, 22u8, 237u8, 103u8, 113u8, 128u8, 181u8, 202u8, 216u8,
            ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    relayer: topics.1,
                    sender: topics.2,
                    approved: data.0,
                }
            }
            #[inline]
            fn check_signature(
                topics: &<Self::TopicList as alloy_sol_types::SolType>::RustType,
            ) -> alloy_sol_types::Result<()> {
                if topics.0 != Self::SIGNATURE_HASH {
                    return Err(
                        alloy_sol_types::Error::invalid_event_signature_hash(
                            Self::SIGNATURE,
                            topics.0,
                            Self::SIGNATURE_HASH,
                        ),
                    );
                }
                Ok(())
            }
            #[inline]
            fn tokenize_body(&self) -> Self::DataToken<'_> {
                (
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::tokenize(
                        &self.approved,
                    ),
                )
            }
            #[inline]
            fn topics(&self) -> <Self::TopicList as alloy_sol_types::SolType>::RustType {
                (Self::SIGNATURE_HASH.into(), self.relayer.clone(), self.sender.clone())
            }
            #[inline]
            fn encode_topics_raw(
                &self,
                out: &mut [alloy_sol_types::abi::token::WordToken],
            ) -> alloy_sol_types::Result<()> {
                if out.len() < <Self::TopicList as alloy_sol_types::TopicList>::COUNT {
                    return Err(alloy_sol_types::Error::Overrun);
                }
                out[0usize] = alloy_sol_types::abi::token::WordToken(
                    Self::SIGNATURE_HASH,
                );
                out[1usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.relayer,
                );
                out[2usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.sender,
                );
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for RelayerApprovalChanged {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&RelayerApprovalChanged> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &RelayerApprovalChanged) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `Swap(bytes32,address,address,uint256,uint256)` and selector `0x2170c741c41531aec20e7c107c24eecfdd15e69c9bb0a8dd37b1840b9e0b207b`.
```solidity
event Swap(bytes32 indexed poolId, address indexed tokenIn, address indexed tokenOut, uint256 amountIn, uint256 amountOut);
```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct Swap {
        #[allow(missing_docs)]
        pub poolId: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub tokenIn: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub tokenOut: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub amountIn: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub amountOut: alloy_sol_types::private::primitives::aliases::U256,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::SolEvent for Swap {
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type DataToken<'a> = <Self::DataTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "Swap(bytes32,address,address,uint256,uint256)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 = alloy_sol_types::private::B256::new([
                33u8, 112u8, 199u8, 65u8, 196u8, 21u8, 49u8, 174u8, 194u8, 14u8, 124u8,
                16u8, 124u8, 36u8, 238u8, 207u8, 221u8, 21u8, 230u8, 156u8, 155u8, 176u8,
                168u8, 221u8, 55u8, 177u8, 132u8, 11u8, 158u8, 11u8, 32u8, 123u8,
            ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    poolId: topics.1,
                    tokenIn: topics.2,
                    tokenOut: topics.3,
                    amountIn: data.0,
                    amountOut: data.1,
                }
            }
            #[inline]
            fn check_signature(
                topics: &<Self::TopicList as alloy_sol_types::SolType>::RustType,
            ) -> alloy_sol_types::Result<()> {
                if topics.0 != Self::SIGNATURE_HASH {
                    return Err(
                        alloy_sol_types::Error::invalid_event_signature_hash(
                            Self::SIGNATURE,
                            topics.0,
                            Self::SIGNATURE_HASH,
                        ),
                    );
                }
                Ok(())
            }
            #[inline]
            fn tokenize_body(&self) -> Self::DataToken<'_> {
                (
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.amountIn),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.amountOut),
                )
            }
            #[inline]
            fn topics(&self) -> <Self::TopicList as alloy_sol_types::SolType>::RustType {
                (
                    Self::SIGNATURE_HASH.into(),
                    self.poolId.clone(),
                    self.tokenIn.clone(),
                    self.tokenOut.clone(),
                )
            }
            #[inline]
            fn encode_topics_raw(
                &self,
                out: &mut [alloy_sol_types::abi::token::WordToken],
            ) -> alloy_sol_types::Result<()> {
                if out.len() < <Self::TopicList as alloy_sol_types::TopicList>::COUNT {
                    return Err(alloy_sol_types::Error::Overrun);
                }
                out[0usize] = alloy_sol_types::abi::token::WordToken(
                    Self::SIGNATURE_HASH,
                );
                out[1usize] = <alloy_sol_types::sol_data::FixedBytes<
                    32,
                > as alloy_sol_types::EventTopic>::encode_topic(&self.poolId);
                out[2usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.tokenIn,
                );
                out[3usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.tokenOut,
                );
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for Swap {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&Swap> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &Swap) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `TokensDeregistered(bytes32,address[])` and selector `0x7dcdc6d02ef40c7c1a7046a011b058bd7f988fa14e20a66344f9d4e60657d610`.
```solidity
event TokensDeregistered(bytes32 indexed poolId, address[] tokens);
```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct TokensDeregistered {
        #[allow(missing_docs)]
        pub poolId: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub tokens: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::SolEvent for TokensDeregistered {
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
            );
            type DataToken<'a> = <Self::DataTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::FixedBytes<32>,
            );
            const SIGNATURE: &'static str = "TokensDeregistered(bytes32,address[])";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 = alloy_sol_types::private::B256::new([
                125u8, 205u8, 198u8, 208u8, 46u8, 244u8, 12u8, 124u8, 26u8, 112u8, 70u8,
                160u8, 17u8, 176u8, 88u8, 189u8, 127u8, 152u8, 143u8, 161u8, 78u8, 32u8,
                166u8, 99u8, 68u8, 249u8, 212u8, 230u8, 6u8, 87u8, 214u8, 16u8,
            ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    poolId: topics.1,
                    tokens: data.0,
                }
            }
            #[inline]
            fn check_signature(
                topics: &<Self::TopicList as alloy_sol_types::SolType>::RustType,
            ) -> alloy_sol_types::Result<()> {
                if topics.0 != Self::SIGNATURE_HASH {
                    return Err(
                        alloy_sol_types::Error::invalid_event_signature_hash(
                            Self::SIGNATURE,
                            topics.0,
                            Self::SIGNATURE_HASH,
                        ),
                    );
                }
                Ok(())
            }
            #[inline]
            fn tokenize_body(&self) -> Self::DataToken<'_> {
                (
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Address,
                    > as alloy_sol_types::SolType>::tokenize(&self.tokens),
                )
            }
            #[inline]
            fn topics(&self) -> <Self::TopicList as alloy_sol_types::SolType>::RustType {
                (Self::SIGNATURE_HASH.into(), self.poolId.clone())
            }
            #[inline]
            fn encode_topics_raw(
                &self,
                out: &mut [alloy_sol_types::abi::token::WordToken],
            ) -> alloy_sol_types::Result<()> {
                if out.len() < <Self::TopicList as alloy_sol_types::TopicList>::COUNT {
                    return Err(alloy_sol_types::Error::Overrun);
                }
                out[0usize] = alloy_sol_types::abi::token::WordToken(
                    Self::SIGNATURE_HASH,
                );
                out[1usize] = <alloy_sol_types::sol_data::FixedBytes<
                    32,
                > as alloy_sol_types::EventTopic>::encode_topic(&self.poolId);
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for TokensDeregistered {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&TokensDeregistered> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &TokensDeregistered) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `TokensRegistered(bytes32,address[],address[])` and selector `0xf5847d3f2197b16cdcd2098ec95d0905cd1abdaf415f07bb7cef2bba8ac5dec4`.
```solidity
event TokensRegistered(bytes32 indexed poolId, address[] tokens, address[] assetManagers);
```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct TokensRegistered {
        #[allow(missing_docs)]
        pub poolId: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub tokens: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
        #[allow(missing_docs)]
        pub assetManagers: alloy_sol_types::private::Vec<
            alloy_sol_types::private::Address,
        >,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::SolEvent for TokensRegistered {
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
            );
            type DataToken<'a> = <Self::DataTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::FixedBytes<32>,
            );
            const SIGNATURE: &'static str = "TokensRegistered(bytes32,address[],address[])";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 = alloy_sol_types::private::B256::new([
                245u8, 132u8, 125u8, 63u8, 33u8, 151u8, 177u8, 108u8, 220u8, 210u8, 9u8,
                142u8, 201u8, 93u8, 9u8, 5u8, 205u8, 26u8, 189u8, 175u8, 65u8, 95u8, 7u8,
                187u8, 124u8, 239u8, 43u8, 186u8, 138u8, 197u8, 222u8, 196u8,
            ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    poolId: topics.1,
                    tokens: data.0,
                    assetManagers: data.1,
                }
            }
            #[inline]
            fn check_signature(
                topics: &<Self::TopicList as alloy_sol_types::SolType>::RustType,
            ) -> alloy_sol_types::Result<()> {
                if topics.0 != Self::SIGNATURE_HASH {
                    return Err(
                        alloy_sol_types::Error::invalid_event_signature_hash(
                            Self::SIGNATURE,
                            topics.0,
                            Self::SIGNATURE_HASH,
                        ),
                    );
                }
                Ok(())
            }
            #[inline]
            fn tokenize_body(&self) -> Self::DataToken<'_> {
                (
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Address,
                    > as alloy_sol_types::SolType>::tokenize(&self.tokens),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Address,
                    > as alloy_sol_types::SolType>::tokenize(&self.assetManagers),
                )
            }
            #[inline]
            fn topics(&self) -> <Self::TopicList as alloy_sol_types::SolType>::RustType {
                (Self::SIGNATURE_HASH.into(), self.poolId.clone())
            }
            #[inline]
            fn encode_topics_raw(
                &self,
                out: &mut [alloy_sol_types::abi::token::WordToken],
            ) -> alloy_sol_types::Result<()> {
                if out.len() < <Self::TopicList as alloy_sol_types::TopicList>::COUNT {
                    return Err(alloy_sol_types::Error::Overrun);
                }
                out[0usize] = alloy_sol_types::abi::token::WordToken(
                    Self::SIGNATURE_HASH,
                );
                out[1usize] = <alloy_sol_types::sol_data::FixedBytes<
                    32,
                > as alloy_sol_types::EventTopic>::encode_topic(&self.poolId);
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for TokensRegistered {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&TokensRegistered> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &TokensRegistered) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    /**Constructor`.
```solidity
constructor(address authorizer, address weth, uint256 pauseWindowDuration, uint256 bufferPeriodDuration);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct constructorCall {
        #[allow(missing_docs)]
        pub authorizer: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub weth: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub pauseWindowDuration: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub bufferPeriodDuration: alloy_sol_types::private::primitives::aliases::U256,
    }
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
                alloy_sol_types::private::Address,
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::primitives::aliases::U256,
            );
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(
                _t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>,
            ) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<constructorCall> for UnderlyingRustTuple<'_> {
                fn from(value: constructorCall) -> Self {
                    (
                        value.authorizer,
                        value.weth,
                        value.pauseWindowDuration,
                        value.bufferPeriodDuration,
                    )
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for constructorCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        authorizer: tuple.0,
                        weth: tuple.1,
                        pauseWindowDuration: tuple.2,
                        bufferPeriodDuration: tuple.3,
                    }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolConstructor for constructorCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }
            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.authorizer,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.weth,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.pauseWindowDuration),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.bufferPeriodDuration),
                )
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `WETH()` and selector `0xad5c4648`.
```solidity
function WETH() external view returns (address);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct WETHCall;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`WETH()`](WETHCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct WETHReturn {
        #[allow(missing_docs)]
        pub _0: alloy_sol_types::private::Address,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = ();
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = ();
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(
                _t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>,
            ) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<WETHCall> for UnderlyingRustTuple<'_> {
                fn from(value: WETHCall) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for WETHCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (alloy_sol_types::sol_data::Address,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (alloy_sol_types::private::Address,);
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(
                _t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>,
            ) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<WETHReturn> for UnderlyingRustTuple<'_> {
                fn from(value: WETHReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for WETHReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for WETHCall {
            type Parameters<'a> = ();
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::Address;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "WETH()";
            const SELECTOR: [u8; 4] = [173u8, 92u8, 70u8, 72u8];
            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }
            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                ()
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        ret,
                    ),
                )
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence(data)
                    .map(|r| {
                        let r: WETHReturn = r.into();
                        r._0
                    })
            }
            #[inline]
            fn abi_decode_returns_validate(
                data: &[u8],
            ) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence_validate(data)
                    .map(|r| {
                        let r: WETHReturn = r.into();
                        r._0
                    })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `batchSwap(uint8,(bytes32,uint256,uint256,uint256,bytes)[],address[],(address,bool,address,bool),int256[],uint256)` and selector `0x945bcec9`.
```solidity
function batchSwap(IVault.SwapKind kind, IVault.BatchSwapStep[] memory swaps, address[] memory assets, IVault.FundManagement memory funds, int256[] memory limits, uint256 deadline) external payable returns (int256[] memory assetDeltas);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct batchSwapCall {
        #[allow(missing_docs)]
        pub kind: <IVault::SwapKind as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub swaps: alloy_sol_types::private::Vec<
            <IVault::BatchSwapStep as alloy_sol_types::SolType>::RustType,
        >,
        #[allow(missing_docs)]
        pub assets: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
        #[allow(missing_docs)]
        pub funds: <IVault::FundManagement as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub limits: alloy_sol_types::private::Vec<
            alloy_sol_types::private::primitives::aliases::I256,
        >,
        #[allow(missing_docs)]
        pub deadline: alloy_sol_types::private::primitives::aliases::U256,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`batchSwap(uint8,(bytes32,uint256,uint256,uint256,bytes)[],address[],(address,bool,address,bool),int256[],uint256)`](batchSwapCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct batchSwapReturn {
        #[allow(missing_docs)]
        pub assetDeltas: alloy_sol_types::private::Vec<
            alloy_sol_types::private::primitives::aliases::I256,
        >,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                IVault::SwapKind,
                alloy_sol_types::sol_data::Array<IVault::BatchSwapStep>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                IVault::FundManagement,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Int<256>>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                <IVault::SwapKind as alloy_sol_types::SolType>::RustType,
                alloy_sol_types::private::Vec<
                    <IVault::BatchSwapStep as alloy_sol_types::SolType>::RustType,
                >,
                alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
                <IVault::FundManagement as alloy_sol_types::SolType>::RustType,
                alloy_sol_types::private::Vec<
                    alloy_sol_types::private::primitives::aliases::I256,
                >,
                alloy_sol_types::private::primitives::aliases::U256,
            );
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(
                _t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>,
            ) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<batchSwapCall> for UnderlyingRustTuple<'_> {
                fn from(value: batchSwapCall) -> Self {
                    (
                        value.kind,
                        value.swaps,
                        value.assets,
                        value.funds,
                        value.limits,
                        value.deadline,
                    )
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for batchSwapCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        kind: tuple.0,
                        swaps: tuple.1,
                        assets: tuple.2,
                        funds: tuple.3,
                        limits: tuple.4,
                        deadline: tuple.5,
                    }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Int<256>>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Vec<
                    alloy_sol_types::private::primitives::aliases::I256,
                >,
            );
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(
                _t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>,
            ) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<batchSwapReturn> for UnderlyingRustTuple<'_> {
                fn from(value: batchSwapReturn) -> Self {
                    (value.assetDeltas,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for batchSwapReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { assetDeltas: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for batchSwapCall {
            type Parameters<'a> = (
                IVault::SwapKind,
                alloy_sol_types::sol_data::Array<IVault::BatchSwapStep>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                IVault::FundManagement,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Int<256>>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::Vec<
                alloy_sol_types::private::primitives::aliases::I256,
            >;
            type ReturnTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Int<256>>,
            );
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "batchSwap(uint8,(bytes32,uint256,uint256,uint256,bytes)[],address[],(address,bool,address,bool),int256[],uint256)";
            const SELECTOR: [u8; 4] = [148u8, 91u8, 206u8, 201u8];
            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }
            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <IVault::SwapKind as alloy_sol_types::SolType>::tokenize(&self.kind),
                    <alloy_sol_types::sol_data::Array<
                        IVault::BatchSwapStep,
                    > as alloy_sol_types::SolType>::tokenize(&self.swaps),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Address,
                    > as alloy_sol_types::SolType>::tokenize(&self.assets),
                    <IVault::FundManagement as alloy_sol_types::SolType>::tokenize(
                        &self.funds,
                    ),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Int<256>,
                    > as alloy_sol_types::SolType>::tokenize(&self.limits),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.deadline),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                (
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Int<256>,
                    > as alloy_sol_types::SolType>::tokenize(ret),
                )
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence(data)
                    .map(|r| {
                        let r: batchSwapReturn = r.into();
                        r.assetDeltas
                    })
            }
            #[inline]
            fn abi_decode_returns_validate(
                data: &[u8],
            ) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence_validate(data)
                    .map(|r| {
                        let r: batchSwapReturn = r.into();
                        r.assetDeltas
                    })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `flashLoan(address,address[],uint256[],bytes)` and selector `0x5c38449e`.
```solidity
function flashLoan(address recipient, address[] memory tokens, uint256[] memory amounts, bytes memory userData) external;
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct flashLoanCall {
        #[allow(missing_docs)]
        pub recipient: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub tokens: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
        #[allow(missing_docs)]
        pub amounts: alloy_sol_types::private::Vec<
            alloy_sol_types::private::primitives::aliases::U256,
        >,
        #[allow(missing_docs)]
        pub userData: alloy_sol_types::private::Bytes,
    }
    ///Container type for the return parameters of the [`flashLoan(address,address[],uint256[],bytes)`](flashLoanCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct flashLoanReturn {}
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
                alloy_sol_types::sol_data::Bytes,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
                alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
                alloy_sol_types::private::Vec<
                    alloy_sol_types::private::primitives::aliases::U256,
                >,
                alloy_sol_types::private::Bytes,
            );
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(
                _t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>,
            ) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<flashLoanCall> for UnderlyingRustTuple<'_> {
                fn from(value: flashLoanCall) -> Self {
                    (value.recipient, value.tokens, value.amounts, value.userData)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for flashLoanCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        recipient: tuple.0,
                        tokens: tuple.1,
                        amounts: tuple.2,
                        userData: tuple.3,
                    }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = ();
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = ();
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(
                _t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>,
            ) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<flashLoanReturn> for UnderlyingRustTuple<'_> {
                fn from(value: flashLoanReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for flashLoanReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl flashLoanReturn {
            fn _tokenize(
                &self,
            ) -> <flashLoanCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for flashLoanCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
                alloy_sol_types::sol_data::Bytes,
            );
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = flashLoanReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "flashLoan(address,address[],uint256[],bytes)";
            const SELECTOR: [u8; 4] = [92u8, 56u8, 68u8, 158u8];
            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }
            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.recipient,
                    ),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Address,
                    > as alloy_sol_types::SolType>::tokenize(&self.tokens),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::SolType>::tokenize(&self.amounts),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.userData,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                flashLoanReturn::_tokenize(ret)
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence(data)
                    .map(Into::into)
            }
            #[inline]
            fn abi_decode_returns_validate(
                data: &[u8],
            ) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence_validate(data)
                    .map(Into::into)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `getInternalBalance(address,address[])` and selector `0x0f5a6efa`.
```solidity
function getInternalBalance(address user, address[] memory tokens) external view returns (uint256[] memory balances);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct getInternalBalanceCall {
        #[allow(missing_docs)]
        pub user: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub tokens: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`getInternalBalance(address,address[])`](getInternalBalanceCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct getInternalBalanceReturn {
        #[allow(missing_docs)]
        pub balances: alloy_sol_types::private::Vec<
            alloy_sol_types::private::primitives::aliases::U256,
        >,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
                alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
            );
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(
                _t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>,
            ) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<getInternalBalanceCall>
            for UnderlyingRustTuple<'_> {
                fn from(value: getInternalBalanceCall) -> Self {
                    (value.user, value.tokens)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for getInternalBalanceCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        user: tuple.0,
                        tokens: tuple.1,
                    }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Vec<
                    alloy_sol_types::private::primitives::aliases::U256,
                >,
            );
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(
                _t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>,
            ) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<getInternalBalanceReturn>
            for UnderlyingRustTuple<'_> {
                fn from(value: getInternalBalanceReturn) -> Self {
                    (value.balances,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for getInternalBalanceReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { balances: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for getInternalBalanceCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
            );
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::Vec<
                alloy_sol_types::private::primitives::aliases::U256,
            >;
            type ReturnTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
            );
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "getInternalBalance(address,address[])";
            const SELECTOR: [u8; 4] = [15u8, 90u8, 110u8, 250u8];
            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }
            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.user,
                    ),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Address,
                    > as alloy_sol_types::SolType>::tokenize(&self.tokens),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                (
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::SolType>::tokenize(ret),
                )
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence(data)
                    .map(|r| {
                        let r: getInternalBalanceReturn = r.into();
                        r.balances
                    })
            }
            #[inline]
            fn abi_decode_returns_validate(
                data: &[u8],
            ) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence_validate(data)
                    .map(|r| {
                        let r: getInternalBalanceReturn = r.into();
                        r.balances
                    })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `getPausedState()` and selector `0x1c0de051`.
```solidity
function getPausedState() external view returns (bool paused, uint256 pauseWindowEndTime, uint256 bufferPeriodEndTime);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct getPausedStateCall;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`getPausedState()`](getPausedStateCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct getPausedStateReturn {
        #[allow(missing_docs)]
        pub paused: bool,
        #[allow(missing_docs)]
        pub pauseWindowEndTime: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub bufferPeriodEndTime: alloy_sol_types::private::primitives::aliases::U256,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = ();
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = ();
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(
                _t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>,
            ) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<getPausedStateCall> for UnderlyingRustTuple<'_> {
                fn from(value: getPausedStateCall) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for getPausedStateCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Bool,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                bool,
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::primitives::aliases::U256,
            );
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(
                _t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>,
            ) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<getPausedStateReturn>
            for UnderlyingRustTuple<'_> {
                fn from(value: getPausedStateReturn) -> Self {
                    (value.paused, value.pauseWindowEndTime, value.bufferPeriodEndTime)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for getPausedStateReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        paused: tuple.0,
                        pauseWindowEndTime: tuple.1,
                        bufferPeriodEndTime: tuple.2,
                    }
                }
            }
        }
        impl getPausedStateReturn {
            fn _tokenize(
                &self,
            ) -> <getPausedStateCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                (
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::tokenize(
                        &self.paused,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.pauseWindowEndTime),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.bufferPeriodEndTime),
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for getPausedStateCall {
            type Parameters<'a> = ();
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = getPausedStateReturn;
            type ReturnTuple<'a> = (
                alloy_sol_types::sol_data::Bool,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "getPausedState()";
            const SELECTOR: [u8; 4] = [28u8, 13u8, 224u8, 81u8];
            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }
            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                ()
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                getPausedStateReturn::_tokenize(ret)
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence(data)
                    .map(Into::into)
            }
            #[inline]
            fn abi_decode_returns_validate(
                data: &[u8],
            ) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence_validate(data)
                    .map(Into::into)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `getPool(bytes32)` and selector `0xf6c00927`.
```solidity
function getPool(bytes32 poolId) external view returns (address, IVault.PoolSpecialization);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct getPoolCall {
        #[allow(missing_docs)]
        pub poolId: alloy_sol_types::private::FixedBytes<32>,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`getPool(bytes32)`](getPoolCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct getPoolReturn {
        #[allow(missing_docs)]
        pub _0: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub _1: <IVault::PoolSpecialization as alloy_sol_types::SolType>::RustType,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (alloy_sol_types::sol_data::FixedBytes<32>,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (alloy_sol_types::private::FixedBytes<32>,);
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(
                _t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>,
            ) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<getPoolCall> for UnderlyingRustTuple<'_> {
                fn from(value: getPoolCall) -> Self {
                    (value.poolId,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for getPoolCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { poolId: tuple.0 }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                IVault::PoolSpecialization,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
                <IVault::PoolSpecialization as alloy_sol_types::SolType>::RustType,
            );
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(
                _t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>,
            ) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<getPoolReturn> for UnderlyingRustTuple<'_> {
                fn from(value: getPoolReturn) -> Self {
                    (value._0, value._1)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for getPoolReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0, _1: tuple.1 }
                }
            }
        }
        impl getPoolReturn {
            fn _tokenize(
                &self,
            ) -> <getPoolCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self._0,
                    ),
                    <IVault::PoolSpecialization as alloy_sol_types::SolType>::tokenize(
                        &self._1,
                    ),
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for getPoolCall {
            type Parameters<'a> = (alloy_sol_types::sol_data::FixedBytes<32>,);
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = getPoolReturn;
            type ReturnTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                IVault::PoolSpecialization,
            );
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "getPool(bytes32)";
            const SELECTOR: [u8; 4] = [246u8, 192u8, 9u8, 39u8];
            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }
            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self.poolId),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                getPoolReturn::_tokenize(ret)
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence(data)
                    .map(Into::into)
            }
            #[inline]
            fn abi_decode_returns_validate(
                data: &[u8],
            ) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence_validate(data)
                    .map(Into::into)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `getPoolTokens(bytes32)` and selector `0xf94d4668`.
```solidity
function getPoolTokens(bytes32 poolId) external view returns (address[] memory tokens, uint256[] memory balances, uint256 lastChangeBlock);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct getPoolTokensCall {
        #[allow(missing_docs)]
        pub poolId: alloy_sol_types::private::FixedBytes<32>,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`getPoolTokens(bytes32)`](getPoolTokensCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct getPoolTokensReturn {
        #[allow(missing_docs)]
        pub tokens: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
        #[allow(missing_docs)]
        pub balances: alloy_sol_types::private::Vec<
            alloy_sol_types::private::primitives::aliases::U256,
        >,
        #[allow(missing_docs)]
        pub lastChangeBlock: alloy_sol_types::private::primitives::aliases::U256,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (alloy_sol_types::sol_data::FixedBytes<32>,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (alloy_sol_types::private::FixedBytes<32>,);
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(
                _t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>,
            ) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<getPoolTokensCall> for UnderlyingRustTuple<'_> {
                fn from(value: getPoolTokensCall) -> Self {
                    (value.poolId,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for getPoolTokensCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { poolId: tuple.0 }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
                alloy_sol_types::private::Vec<
                    alloy_sol_types::private::primitives::aliases::U256,
                >,
                alloy_sol_types::private::primitives::aliases::U256,
            );
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(
                _t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>,
            ) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<getPoolTokensReturn> for UnderlyingRustTuple<'_> {
                fn from(value: getPoolTokensReturn) -> Self {
                    (value.tokens, value.balances, value.lastChangeBlock)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for getPoolTokensReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        tokens: tuple.0,
                        balances: tuple.1,
                        lastChangeBlock: tuple.2,
                    }
                }
            }
        }
        impl getPoolTokensReturn {
            fn _tokenize(
                &self,
            ) -> <getPoolTokensCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                (
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Address,
                    > as alloy_sol_types::SolType>::tokenize(&self.tokens),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::SolType>::tokenize(&self.balances),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.lastChangeBlock),
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for getPoolTokensCall {
            type Parameters<'a> = (alloy_sol_types::sol_data::FixedBytes<32>,);
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = getPoolTokensReturn;
            type ReturnTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "getPoolTokens(bytes32)";
            const SELECTOR: [u8; 4] = [249u8, 77u8, 70u8, 104u8];
            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }
            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self.poolId),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                getPoolTokensReturn::_tokenize(ret)
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence(data)
                    .map(Into::into)
            }
            #[inline]
            fn abi_decode_returns_validate(
                data: &[u8],
            ) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence_validate(data)
                    .map(Into::into)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `hasApprovedRelayer(address,address)` and selector `0xfec90d72`.
```solidity
function hasApprovedRelayer(address user, address relayer) external view returns (bool);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct hasApprovedRelayerCall {
        #[allow(missing_docs)]
        pub user: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub relayer: alloy_sol_types::private::Address,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`hasApprovedRelayer(address,address)`](hasApprovedRelayerCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct hasApprovedRelayerReturn {
        #[allow(missing_docs)]
        pub _0: bool,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
                alloy_sol_types::private::Address,
            );
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(
                _t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>,
            ) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<hasApprovedRelayerCall>
            for UnderlyingRustTuple<'_> {
                fn from(value: hasApprovedRelayerCall) -> Self {
                    (value.user, value.relayer)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for hasApprovedRelayerCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        user: tuple.0,
                        relayer: tuple.1,
                    }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (alloy_sol_types::sol_data::Bool,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (bool,);
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(
                _t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>,
            ) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<hasApprovedRelayerReturn>
            for UnderlyingRustTuple<'_> {
                fn from(value: hasApprovedRelayerReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for hasApprovedRelayerReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for hasApprovedRelayerCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
            );
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = bool;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Bool,);
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "hasApprovedRelayer(address,address)";
            const SELECTOR: [u8; 4] = [254u8, 201u8, 13u8, 114u8];
            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }
            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.user,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.relayer,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                (
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::tokenize(
                        ret,
                    ),
                )
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence(data)
                    .map(|r| {
                        let r: hasApprovedRelayerReturn = r.into();
                        r._0
                    })
            }
            #[inline]
            fn abi_decode_returns_validate(
                data: &[u8],
            ) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence_validate(data)
                    .map(|r| {
                        let r: hasApprovedRelayerReturn = r.into();
                        r._0
                    })
            }
        }
    };
    #[derive()]
    /**Function with signature `manageUserBalance((uint8,address,uint256,address,address)[])` and selector `0x0e8e3e84`.
```solidity
function manageUserBalance(IVault.UserBalanceOp[] memory ops) external payable;
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct manageUserBalanceCall {
        #[allow(missing_docs)]
        pub ops: alloy_sol_types::private::Vec<
            <IVault::UserBalanceOp as alloy_sol_types::SolType>::RustType,
        >,
    }
    ///Container type for the return parameters of the [`manageUserBalance((uint8,address,uint256,address,address)[])`](manageUserBalanceCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct manageUserBalanceReturn {}
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Array<IVault::UserBalanceOp>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Vec<
                    <IVault::UserBalanceOp as alloy_sol_types::SolType>::RustType,
                >,
            );
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(
                _t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>,
            ) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<manageUserBalanceCall>
            for UnderlyingRustTuple<'_> {
                fn from(value: manageUserBalanceCall) -> Self {
                    (value.ops,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for manageUserBalanceCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { ops: tuple.0 }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = ();
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = ();
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(
                _t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>,
            ) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<manageUserBalanceReturn>
            for UnderlyingRustTuple<'_> {
                fn from(value: manageUserBalanceReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for manageUserBalanceReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl manageUserBalanceReturn {
            fn _tokenize(
                &self,
            ) -> <manageUserBalanceCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for manageUserBalanceCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Array<IVault::UserBalanceOp>,
            );
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = manageUserBalanceReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "manageUserBalance((uint8,address,uint256,address,address)[])";
            const SELECTOR: [u8; 4] = [14u8, 142u8, 62u8, 132u8];
            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }
            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Array<
                        IVault::UserBalanceOp,
                    > as alloy_sol_types::SolType>::tokenize(&self.ops),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                manageUserBalanceReturn::_tokenize(ret)
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence(data)
                    .map(Into::into)
            }
            #[inline]
            fn abi_decode_returns_validate(
                data: &[u8],
            ) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence_validate(data)
                    .map(Into::into)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `setRelayerApproval(address,address,bool)` and selector `0xfa6e671d`.
```solidity
function setRelayerApproval(address sender, address relayer, bool approved) external;
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct setRelayerApprovalCall {
        #[allow(missing_docs)]
        pub sender: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub relayer: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub approved: bool,
    }
    ///Container type for the return parameters of the [`setRelayerApproval(address,address,bool)`](setRelayerApprovalCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct setRelayerApprovalReturn {}
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Bool,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
                alloy_sol_types::private::Address,
                bool,
            );
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(
                _t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>,
            ) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<setRelayerApprovalCall>
            for UnderlyingRustTuple<'_> {
                fn from(value: setRelayerApprovalCall) -> Self {
                    (value.sender, value.relayer, value.approved)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for setRelayerApprovalCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        sender: tuple.0,
                        relayer: tuple.1,
                        approved: tuple.2,
                    }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = ();
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = ();
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(
                _t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>,
            ) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<setRelayerApprovalReturn>
            for UnderlyingRustTuple<'_> {
                fn from(value: setRelayerApprovalReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for setRelayerApprovalReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl setRelayerApprovalReturn {
            fn _tokenize(
                &self,
            ) -> <setRelayerApprovalCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for setRelayerApprovalCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Bool,
            );
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = setRelayerApprovalReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "setRelayerApproval(address,address,bool)";
            const SELECTOR: [u8; 4] = [250u8, 110u8, 103u8, 29u8];
            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }
            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.sender,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.relayer,
                    ),
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::tokenize(
                        &self.approved,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                setRelayerApprovalReturn::_tokenize(ret)
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence(data)
                    .map(Into::into)
            }
            #[inline]
            fn abi_decode_returns_validate(
                data: &[u8],
            ) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence_validate(data)
                    .map(Into::into)
            }
        }
    };
    #[derive()]
    /**Function with signature `swap((bytes32,uint8,address,address,uint256,bytes),(address,bool,address,bool),uint256,uint256)` and selector `0x52bbbe29`.
```solidity
function swap(IVault.SingleSwap memory singleSwap, IVault.FundManagement memory funds, uint256 limit, uint256 deadline) external payable returns (uint256 amountCalculated);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct swapCall {
        #[allow(missing_docs)]
        pub singleSwap: <IVault::SingleSwap as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub funds: <IVault::FundManagement as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub limit: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub deadline: alloy_sol_types::private::primitives::aliases::U256,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`swap((bytes32,uint8,address,address,uint256,bytes),(address,bool,address,bool),uint256,uint256)`](swapCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct swapReturn {
        #[allow(missing_docs)]
        pub amountCalculated: alloy_sol_types::private::primitives::aliases::U256,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                IVault::SingleSwap,
                IVault::FundManagement,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                <IVault::SingleSwap as alloy_sol_types::SolType>::RustType,
                <IVault::FundManagement as alloy_sol_types::SolType>::RustType,
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::primitives::aliases::U256,
            );
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(
                _t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>,
            ) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<swapCall> for UnderlyingRustTuple<'_> {
                fn from(value: swapCall) -> Self {
                    (value.singleSwap, value.funds, value.limit, value.deadline)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for swapCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        singleSwap: tuple.0,
                        funds: tuple.1,
                        limit: tuple.2,
                        deadline: tuple.3,
                    }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (alloy_sol_types::sol_data::Uint<256>,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::primitives::aliases::U256,
            );
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(
                _t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>,
            ) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<swapReturn> for UnderlyingRustTuple<'_> {
                fn from(value: swapReturn) -> Self {
                    (value.amountCalculated,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for swapReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { amountCalculated: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for swapCall {
            type Parameters<'a> = (
                IVault::SingleSwap,
                IVault::FundManagement,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::primitives::aliases::U256;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Uint<256>,);
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "swap((bytes32,uint8,address,address,uint256,bytes),(address,bool,address,bool),uint256,uint256)";
            const SELECTOR: [u8; 4] = [82u8, 187u8, 190u8, 41u8];
            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }
            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <IVault::SingleSwap as alloy_sol_types::SolType>::tokenize(
                        &self.singleSwap,
                    ),
                    <IVault::FundManagement as alloy_sol_types::SolType>::tokenize(
                        &self.funds,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.limit),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.deadline),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                (
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(ret),
                )
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence(data)
                    .map(|r| {
                        let r: swapReturn = r.into();
                        r.amountCalculated
                    })
            }
            #[inline]
            fn abi_decode_returns_validate(
                data: &[u8],
            ) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence_validate(data)
                    .map(|r| {
                        let r: swapReturn = r.into();
                        r.amountCalculated
                    })
            }
        }
    };
    ///Container for all the [`BalancerV2Vault`](self) function calls.
    #[derive(Clone)]
    #[derive()]
    pub enum BalancerV2VaultCalls {
        #[allow(missing_docs)]
        WETH(WETHCall),
        #[allow(missing_docs)]
        batchSwap(batchSwapCall),
        #[allow(missing_docs)]
        flashLoan(flashLoanCall),
        #[allow(missing_docs)]
        getInternalBalance(getInternalBalanceCall),
        #[allow(missing_docs)]
        getPausedState(getPausedStateCall),
        #[allow(missing_docs)]
        getPool(getPoolCall),
        #[allow(missing_docs)]
        getPoolTokens(getPoolTokensCall),
        #[allow(missing_docs)]
        hasApprovedRelayer(hasApprovedRelayerCall),
        #[allow(missing_docs)]
        manageUserBalance(manageUserBalanceCall),
        #[allow(missing_docs)]
        setRelayerApproval(setRelayerApprovalCall),
        #[allow(missing_docs)]
        swap(swapCall),
    }
    impl BalancerV2VaultCalls {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the variants.
        /// No guarantees are made about the order of the selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 4usize]] = &[
            [14u8, 142u8, 62u8, 132u8],
            [15u8, 90u8, 110u8, 250u8],
            [28u8, 13u8, 224u8, 81u8],
            [82u8, 187u8, 190u8, 41u8],
            [92u8, 56u8, 68u8, 158u8],
            [148u8, 91u8, 206u8, 201u8],
            [173u8, 92u8, 70u8, 72u8],
            [246u8, 192u8, 9u8, 39u8],
            [249u8, 77u8, 70u8, 104u8],
            [250u8, 110u8, 103u8, 29u8],
            [254u8, 201u8, 13u8, 114u8],
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(manageUserBalance),
            ::core::stringify!(getInternalBalance),
            ::core::stringify!(getPausedState),
            ::core::stringify!(swap),
            ::core::stringify!(flashLoan),
            ::core::stringify!(batchSwap),
            ::core::stringify!(WETH),
            ::core::stringify!(getPool),
            ::core::stringify!(getPoolTokens),
            ::core::stringify!(setRelayerApproval),
            ::core::stringify!(hasApprovedRelayer),
        ];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <manageUserBalanceCall as alloy_sol_types::SolCall>::SIGNATURE,
            <getInternalBalanceCall as alloy_sol_types::SolCall>::SIGNATURE,
            <getPausedStateCall as alloy_sol_types::SolCall>::SIGNATURE,
            <swapCall as alloy_sol_types::SolCall>::SIGNATURE,
            <flashLoanCall as alloy_sol_types::SolCall>::SIGNATURE,
            <batchSwapCall as alloy_sol_types::SolCall>::SIGNATURE,
            <WETHCall as alloy_sol_types::SolCall>::SIGNATURE,
            <getPoolCall as alloy_sol_types::SolCall>::SIGNATURE,
            <getPoolTokensCall as alloy_sol_types::SolCall>::SIGNATURE,
            <setRelayerApprovalCall as alloy_sol_types::SolCall>::SIGNATURE,
            <hasApprovedRelayerCall as alloy_sol_types::SolCall>::SIGNATURE,
        ];
        /// Returns the signature for the given selector, if known.
        #[inline]
        pub fn signature_by_selector(
            selector: [u8; 4usize],
        ) -> ::core::option::Option<&'static str> {
            match Self::SELECTORS.binary_search(&selector) {
                ::core::result::Result::Ok(idx) => {
                    ::core::option::Option::Some(Self::SIGNATURES[idx])
                }
                ::core::result::Result::Err(_) => ::core::option::Option::None,
            }
        }
        /// Returns the enum variant name for the given selector, if known.
        #[inline]
        pub fn name_by_selector(
            selector: [u8; 4usize],
        ) -> ::core::option::Option<&'static str> {
            let sig = Self::signature_by_selector(selector)?;
            sig.split_once('(').map(|(name, _)| name)
        }
    }
    #[automatically_derived]
    impl alloy_sol_types::SolInterface for BalancerV2VaultCalls {
        const NAME: &'static str = "BalancerV2VaultCalls";
        const MIN_DATA_LENGTH: usize = 0usize;
        const COUNT: usize = 11usize;
        #[inline]
        fn selector(&self) -> [u8; 4] {
            match self {
                Self::WETH(_) => <WETHCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::batchSwap(_) => {
                    <batchSwapCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::flashLoan(_) => {
                    <flashLoanCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::getInternalBalance(_) => {
                    <getInternalBalanceCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::getPausedState(_) => {
                    <getPausedStateCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::getPool(_) => <getPoolCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::getPoolTokens(_) => {
                    <getPoolTokensCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::hasApprovedRelayer(_) => {
                    <hasApprovedRelayerCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::manageUserBalance(_) => {
                    <manageUserBalanceCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::setRelayerApproval(_) => {
                    <setRelayerApprovalCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::swap(_) => <swapCall as alloy_sol_types::SolCall>::SELECTOR,
            }
        }
        #[inline]
        fn selector_at(i: usize) -> ::core::option::Option<[u8; 4]> {
            Self::SELECTORS.get(i).copied()
        }
        #[inline]
        fn valid_selector(selector: [u8; 4]) -> bool {
            Self::SELECTORS.binary_search(&selector).is_ok()
        }
        #[inline]
        #[allow(non_snake_case)]
        fn abi_decode_raw(
            selector: [u8; 4],
            data: &[u8],
        ) -> alloy_sol_types::Result<Self> {
            static DECODE_SHIMS: &[fn(
                &[u8],
            ) -> alloy_sol_types::Result<BalancerV2VaultCalls>] = &[
                {
                    fn manageUserBalance(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV2VaultCalls> {
                        <manageUserBalanceCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(BalancerV2VaultCalls::manageUserBalance)
                    }
                    manageUserBalance
                },
                {
                    fn getInternalBalance(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV2VaultCalls> {
                        <getInternalBalanceCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(BalancerV2VaultCalls::getInternalBalance)
                    }
                    getInternalBalance
                },
                {
                    fn getPausedState(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV2VaultCalls> {
                        <getPausedStateCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(BalancerV2VaultCalls::getPausedState)
                    }
                    getPausedState
                },
                {
                    fn swap(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV2VaultCalls> {
                        <swapCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(BalancerV2VaultCalls::swap)
                    }
                    swap
                },
                {
                    fn flashLoan(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV2VaultCalls> {
                        <flashLoanCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(BalancerV2VaultCalls::flashLoan)
                    }
                    flashLoan
                },
                {
                    fn batchSwap(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV2VaultCalls> {
                        <batchSwapCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(BalancerV2VaultCalls::batchSwap)
                    }
                    batchSwap
                },
                {
                    fn WETH(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV2VaultCalls> {
                        <WETHCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(BalancerV2VaultCalls::WETH)
                    }
                    WETH
                },
                {
                    fn getPool(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV2VaultCalls> {
                        <getPoolCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(BalancerV2VaultCalls::getPool)
                    }
                    getPool
                },
                {
                    fn getPoolTokens(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV2VaultCalls> {
                        <getPoolTokensCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(BalancerV2VaultCalls::getPoolTokens)
                    }
                    getPoolTokens
                },
                {
                    fn setRelayerApproval(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV2VaultCalls> {
                        <setRelayerApprovalCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(BalancerV2VaultCalls::setRelayerApproval)
                    }
                    setRelayerApproval
                },
                {
                    fn hasApprovedRelayer(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV2VaultCalls> {
                        <hasApprovedRelayerCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(BalancerV2VaultCalls::hasApprovedRelayer)
                    }
                    hasApprovedRelayer
                },
            ];
            let Ok(idx) = Self::SELECTORS.binary_search(&selector) else {
                return Err(
                    alloy_sol_types::Error::unknown_selector(
                        <Self as alloy_sol_types::SolInterface>::NAME,
                        selector,
                    ),
                );
            };
            DECODE_SHIMS[idx](data)
        }
        #[inline]
        #[allow(non_snake_case)]
        fn abi_decode_raw_validate(
            selector: [u8; 4],
            data: &[u8],
        ) -> alloy_sol_types::Result<Self> {
            static DECODE_VALIDATE_SHIMS: &[fn(
                &[u8],
            ) -> alloy_sol_types::Result<BalancerV2VaultCalls>] = &[
                {
                    fn manageUserBalance(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV2VaultCalls> {
                        <manageUserBalanceCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(BalancerV2VaultCalls::manageUserBalance)
                    }
                    manageUserBalance
                },
                {
                    fn getInternalBalance(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV2VaultCalls> {
                        <getInternalBalanceCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(BalancerV2VaultCalls::getInternalBalance)
                    }
                    getInternalBalance
                },
                {
                    fn getPausedState(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV2VaultCalls> {
                        <getPausedStateCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(BalancerV2VaultCalls::getPausedState)
                    }
                    getPausedState
                },
                {
                    fn swap(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV2VaultCalls> {
                        <swapCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(BalancerV2VaultCalls::swap)
                    }
                    swap
                },
                {
                    fn flashLoan(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV2VaultCalls> {
                        <flashLoanCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(BalancerV2VaultCalls::flashLoan)
                    }
                    flashLoan
                },
                {
                    fn batchSwap(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV2VaultCalls> {
                        <batchSwapCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(BalancerV2VaultCalls::batchSwap)
                    }
                    batchSwap
                },
                {
                    fn WETH(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV2VaultCalls> {
                        <WETHCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(BalancerV2VaultCalls::WETH)
                    }
                    WETH
                },
                {
                    fn getPool(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV2VaultCalls> {
                        <getPoolCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(BalancerV2VaultCalls::getPool)
                    }
                    getPool
                },
                {
                    fn getPoolTokens(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV2VaultCalls> {
                        <getPoolTokensCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(BalancerV2VaultCalls::getPoolTokens)
                    }
                    getPoolTokens
                },
                {
                    fn setRelayerApproval(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV2VaultCalls> {
                        <setRelayerApprovalCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(BalancerV2VaultCalls::setRelayerApproval)
                    }
                    setRelayerApproval
                },
                {
                    fn hasApprovedRelayer(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV2VaultCalls> {
                        <hasApprovedRelayerCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(BalancerV2VaultCalls::hasApprovedRelayer)
                    }
                    hasApprovedRelayer
                },
            ];
            let Ok(idx) = Self::SELECTORS.binary_search(&selector) else {
                return Err(
                    alloy_sol_types::Error::unknown_selector(
                        <Self as alloy_sol_types::SolInterface>::NAME,
                        selector,
                    ),
                );
            };
            DECODE_VALIDATE_SHIMS[idx](data)
        }
        #[inline]
        fn abi_encoded_size(&self) -> usize {
            match self {
                Self::WETH(inner) => {
                    <WETHCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::batchSwap(inner) => {
                    <batchSwapCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::flashLoan(inner) => {
                    <flashLoanCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::getInternalBalance(inner) => {
                    <getInternalBalanceCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::getPausedState(inner) => {
                    <getPausedStateCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::getPool(inner) => {
                    <getPoolCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::getPoolTokens(inner) => {
                    <getPoolTokensCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::hasApprovedRelayer(inner) => {
                    <hasApprovedRelayerCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::manageUserBalance(inner) => {
                    <manageUserBalanceCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::setRelayerApproval(inner) => {
                    <setRelayerApprovalCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::swap(inner) => {
                    <swapCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
            }
        }
        #[inline]
        fn abi_encode_raw(&self, out: &mut alloy_sol_types::private::Vec<u8>) {
            match self {
                Self::WETH(inner) => {
                    <WETHCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::batchSwap(inner) => {
                    <batchSwapCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::flashLoan(inner) => {
                    <flashLoanCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::getInternalBalance(inner) => {
                    <getInternalBalanceCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::getPausedState(inner) => {
                    <getPausedStateCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::getPool(inner) => {
                    <getPoolCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::getPoolTokens(inner) => {
                    <getPoolTokensCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::hasApprovedRelayer(inner) => {
                    <hasApprovedRelayerCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::manageUserBalance(inner) => {
                    <manageUserBalanceCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::setRelayerApproval(inner) => {
                    <setRelayerApprovalCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::swap(inner) => {
                    <swapCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
            }
        }
    }
    ///Container for all the [`BalancerV2Vault`](self) events.
    #[derive(Clone)]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub enum BalancerV2VaultEvents {
        #[allow(missing_docs)]
        AuthorizerChanged(AuthorizerChanged),
        #[allow(missing_docs)]
        ExternalBalanceTransfer(ExternalBalanceTransfer),
        #[allow(missing_docs)]
        FlashLoan(FlashLoan),
        #[allow(missing_docs)]
        InternalBalanceChanged(InternalBalanceChanged),
        #[allow(missing_docs)]
        PausedStateChanged(PausedStateChanged),
        #[allow(missing_docs)]
        PoolBalanceChanged(PoolBalanceChanged),
        #[allow(missing_docs)]
        PoolBalanceManaged(PoolBalanceManaged),
        #[allow(missing_docs)]
        PoolRegistered(PoolRegistered),
        #[allow(missing_docs)]
        RelayerApprovalChanged(RelayerApprovalChanged),
        #[allow(missing_docs)]
        Swap(Swap),
        #[allow(missing_docs)]
        TokensDeregistered(TokensDeregistered),
        #[allow(missing_docs)]
        TokensRegistered(TokensRegistered),
    }
    impl BalancerV2VaultEvents {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the variants.
        /// No guarantees are made about the order of the selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 32usize]] = &[
            [
                13u8, 125u8, 117u8, 224u8, 26u8, 185u8, 87u8, 128u8, 211u8, 205u8, 28u8,
                142u8, 192u8, 221u8, 108u8, 44u8, 225u8, 158u8, 58u8, 32u8, 66u8, 126u8,
                236u8, 139u8, 245u8, 50u8, 131u8, 182u8, 251u8, 142u8, 149u8, 240u8,
            ],
            [
                24u8, 225u8, 234u8, 65u8, 57u8, 230u8, 132u8, 19u8, 215u8, 208u8, 138u8,
                167u8, 82u8, 231u8, 21u8, 104u8, 227u8, 107u8, 44u8, 91u8, 249u8, 64u8,
                137u8, 51u8, 20u8, 194u8, 197u8, 176u8, 30u8, 170u8, 12u8, 66u8,
            ],
            [
                33u8, 112u8, 199u8, 65u8, 196u8, 21u8, 49u8, 174u8, 194u8, 14u8, 124u8,
                16u8, 124u8, 36u8, 238u8, 207u8, 221u8, 21u8, 230u8, 156u8, 155u8, 176u8,
                168u8, 221u8, 55u8, 177u8, 132u8, 11u8, 158u8, 11u8, 32u8, 123u8,
            ],
            [
                60u8, 19u8, 188u8, 48u8, 184u8, 232u8, 120u8, 197u8, 63u8, 210u8, 163u8,
                107u8, 103u8, 148u8, 9u8, 192u8, 115u8, 175u8, 215u8, 89u8, 80u8, 190u8,
                67u8, 216u8, 133u8, 135u8, 104u8, 233u8, 86u8, 251u8, 194u8, 14u8,
            ],
            [
                70u8, 150u8, 31u8, 219u8, 69u8, 2u8, 182u8, 70u8, 213u8, 9u8, 95u8,
                186u8, 118u8, 0u8, 72u8, 106u8, 138u8, 192u8, 80u8, 65u8, 213u8, 92u8,
                223u8, 15u8, 22u8, 237u8, 103u8, 113u8, 128u8, 181u8, 202u8, 216u8,
            ],
            [
                84u8, 10u8, 26u8, 63u8, 40u8, 52u8, 12u8, 174u8, 195u8, 54u8, 200u8,
                29u8, 141u8, 123u8, 61u8, 241u8, 57u8, 238u8, 92u8, 220u8, 24u8, 57u8,
                164u8, 242u8, 131u8, 215u8, 235u8, 183u8, 234u8, 174u8, 45u8, 92u8,
            ],
            [
                110u8, 220u8, 175u8, 98u8, 65u8, 16u8, 91u8, 76u8, 148u8, 194u8, 239u8,
                219u8, 243u8, 166u8, 177u8, 36u8, 88u8, 235u8, 61u8, 7u8, 190u8, 58u8,
                14u8, 129u8, 210u8, 75u8, 19u8, 196u8, 64u8, 69u8, 254u8, 122u8,
            ],
            [
                125u8, 205u8, 198u8, 208u8, 46u8, 244u8, 12u8, 124u8, 26u8, 112u8, 70u8,
                160u8, 17u8, 176u8, 88u8, 189u8, 127u8, 152u8, 143u8, 161u8, 78u8, 32u8,
                166u8, 99u8, 68u8, 249u8, 212u8, 230u8, 6u8, 87u8, 214u8, 16u8,
            ],
            [
                148u8, 185u8, 121u8, 182u8, 131u8, 26u8, 81u8, 41u8, 62u8, 38u8, 65u8,
                66u8, 111u8, 151u8, 116u8, 127u8, 238u8, 212u8, 111u8, 23u8, 119u8,
                159u8, 237u8, 156u8, 209u8, 141u8, 30u8, 206u8, 252u8, 254u8, 146u8,
                239u8,
            ],
            [
                158u8, 58u8, 94u8, 55u8, 34u8, 69u8, 50u8, 222u8, 166u8, 123u8, 137u8,
                250u8, 206u8, 24u8, 87u8, 3u8, 115u8, 138u8, 34u8, 138u8, 110u8, 138u8,
                35u8, 222u8, 229u8, 70u8, 150u8, 1u8, 128u8, 211u8, 190u8, 100u8,
            ],
            [
                229u8, 206u8, 36u8, 144u8, 135u8, 206u8, 4u8, 240u8, 90u8, 149u8, 113u8,
                146u8, 67u8, 84u8, 0u8, 253u8, 151u8, 134u8, 141u8, 186u8, 14u8, 106u8,
                75u8, 76u8, 4u8, 154u8, 191u8, 138u8, 248u8, 13u8, 174u8, 120u8,
            ],
            [
                245u8, 132u8, 125u8, 63u8, 33u8, 151u8, 177u8, 108u8, 220u8, 210u8, 9u8,
                142u8, 201u8, 93u8, 9u8, 5u8, 205u8, 26u8, 189u8, 175u8, 65u8, 95u8, 7u8,
                187u8, 124u8, 239u8, 43u8, 186u8, 138u8, 197u8, 222u8, 196u8,
            ],
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(FlashLoan),
            ::core::stringify!(InternalBalanceChanged),
            ::core::stringify!(Swap),
            ::core::stringify!(PoolRegistered),
            ::core::stringify!(RelayerApprovalChanged),
            ::core::stringify!(ExternalBalanceTransfer),
            ::core::stringify!(PoolBalanceManaged),
            ::core::stringify!(TokensDeregistered),
            ::core::stringify!(AuthorizerChanged),
            ::core::stringify!(PausedStateChanged),
            ::core::stringify!(PoolBalanceChanged),
            ::core::stringify!(TokensRegistered),
        ];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <FlashLoan as alloy_sol_types::SolEvent>::SIGNATURE,
            <InternalBalanceChanged as alloy_sol_types::SolEvent>::SIGNATURE,
            <Swap as alloy_sol_types::SolEvent>::SIGNATURE,
            <PoolRegistered as alloy_sol_types::SolEvent>::SIGNATURE,
            <RelayerApprovalChanged as alloy_sol_types::SolEvent>::SIGNATURE,
            <ExternalBalanceTransfer as alloy_sol_types::SolEvent>::SIGNATURE,
            <PoolBalanceManaged as alloy_sol_types::SolEvent>::SIGNATURE,
            <TokensDeregistered as alloy_sol_types::SolEvent>::SIGNATURE,
            <AuthorizerChanged as alloy_sol_types::SolEvent>::SIGNATURE,
            <PausedStateChanged as alloy_sol_types::SolEvent>::SIGNATURE,
            <PoolBalanceChanged as alloy_sol_types::SolEvent>::SIGNATURE,
            <TokensRegistered as alloy_sol_types::SolEvent>::SIGNATURE,
        ];
        /// Returns the signature for the given selector, if known.
        #[inline]
        pub fn signature_by_selector(
            selector: [u8; 32usize],
        ) -> ::core::option::Option<&'static str> {
            match Self::SELECTORS.binary_search(&selector) {
                ::core::result::Result::Ok(idx) => {
                    ::core::option::Option::Some(Self::SIGNATURES[idx])
                }
                ::core::result::Result::Err(_) => ::core::option::Option::None,
            }
        }
        /// Returns the enum variant name for the given selector, if known.
        #[inline]
        pub fn name_by_selector(
            selector: [u8; 32usize],
        ) -> ::core::option::Option<&'static str> {
            let sig = Self::signature_by_selector(selector)?;
            sig.split_once('(').map(|(name, _)| name)
        }
    }
    #[automatically_derived]
    impl alloy_sol_types::SolEventInterface for BalancerV2VaultEvents {
        const NAME: &'static str = "BalancerV2VaultEvents";
        const COUNT: usize = 12usize;
        fn decode_raw_log(
            topics: &[alloy_sol_types::Word],
            data: &[u8],
        ) -> alloy_sol_types::Result<Self> {
            match topics.first().copied() {
                Some(
                    <AuthorizerChanged as alloy_sol_types::SolEvent>::SIGNATURE_HASH,
                ) => {
                    <AuthorizerChanged as alloy_sol_types::SolEvent>::decode_raw_log(
                            topics,
                            data,
                        )
                        .map(Self::AuthorizerChanged)
                }
                Some(
                    <ExternalBalanceTransfer as alloy_sol_types::SolEvent>::SIGNATURE_HASH,
                ) => {
                    <ExternalBalanceTransfer as alloy_sol_types::SolEvent>::decode_raw_log(
                            topics,
                            data,
                        )
                        .map(Self::ExternalBalanceTransfer)
                }
                Some(<FlashLoan as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <FlashLoan as alloy_sol_types::SolEvent>::decode_raw_log(
                            topics,
                            data,
                        )
                        .map(Self::FlashLoan)
                }
                Some(
                    <InternalBalanceChanged as alloy_sol_types::SolEvent>::SIGNATURE_HASH,
                ) => {
                    <InternalBalanceChanged as alloy_sol_types::SolEvent>::decode_raw_log(
                            topics,
                            data,
                        )
                        .map(Self::InternalBalanceChanged)
                }
                Some(
                    <PausedStateChanged as alloy_sol_types::SolEvent>::SIGNATURE_HASH,
                ) => {
                    <PausedStateChanged as alloy_sol_types::SolEvent>::decode_raw_log(
                            topics,
                            data,
                        )
                        .map(Self::PausedStateChanged)
                }
                Some(
                    <PoolBalanceChanged as alloy_sol_types::SolEvent>::SIGNATURE_HASH,
                ) => {
                    <PoolBalanceChanged as alloy_sol_types::SolEvent>::decode_raw_log(
                            topics,
                            data,
                        )
                        .map(Self::PoolBalanceChanged)
                }
                Some(
                    <PoolBalanceManaged as alloy_sol_types::SolEvent>::SIGNATURE_HASH,
                ) => {
                    <PoolBalanceManaged as alloy_sol_types::SolEvent>::decode_raw_log(
                            topics,
                            data,
                        )
                        .map(Self::PoolBalanceManaged)
                }
                Some(<PoolRegistered as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <PoolRegistered as alloy_sol_types::SolEvent>::decode_raw_log(
                            topics,
                            data,
                        )
                        .map(Self::PoolRegistered)
                }
                Some(
                    <RelayerApprovalChanged as alloy_sol_types::SolEvent>::SIGNATURE_HASH,
                ) => {
                    <RelayerApprovalChanged as alloy_sol_types::SolEvent>::decode_raw_log(
                            topics,
                            data,
                        )
                        .map(Self::RelayerApprovalChanged)
                }
                Some(<Swap as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <Swap as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::Swap)
                }
                Some(
                    <TokensDeregistered as alloy_sol_types::SolEvent>::SIGNATURE_HASH,
                ) => {
                    <TokensDeregistered as alloy_sol_types::SolEvent>::decode_raw_log(
                            topics,
                            data,
                        )
                        .map(Self::TokensDeregistered)
                }
                Some(<TokensRegistered as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <TokensRegistered as alloy_sol_types::SolEvent>::decode_raw_log(
                            topics,
                            data,
                        )
                        .map(Self::TokensRegistered)
                }
                _ => {
                    alloy_sol_types::private::Err(alloy_sol_types::Error::InvalidLog {
                        name: <Self as alloy_sol_types::SolEventInterface>::NAME,
                        log: alloy_sol_types::private::Box::new(
                            alloy_sol_types::private::LogData::new_unchecked(
                                topics.to_vec(),
                                data.to_vec().into(),
                            ),
                        ),
                    })
                }
            }
        }
    }
    #[automatically_derived]
    impl alloy_sol_types::private::IntoLogData for BalancerV2VaultEvents {
        fn to_log_data(&self) -> alloy_sol_types::private::LogData {
            match self {
                Self::AuthorizerChanged(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::ExternalBalanceTransfer(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::FlashLoan(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::InternalBalanceChanged(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::PausedStateChanged(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::PoolBalanceChanged(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::PoolBalanceManaged(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::PoolRegistered(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::RelayerApprovalChanged(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::Swap(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::TokensDeregistered(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::TokensRegistered(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
            }
        }
        fn into_log_data(self) -> alloy_sol_types::private::LogData {
            match self {
                Self::AuthorizerChanged(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::ExternalBalanceTransfer(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::FlashLoan(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::InternalBalanceChanged(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::PausedStateChanged(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::PoolBalanceChanged(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::PoolBalanceManaged(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::PoolRegistered(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::RelayerApprovalChanged(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::Swap(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::TokensDeregistered(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::TokensRegistered(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
            }
        }
    }
    use alloy_contract as alloy_contract;
    /**Creates a new wrapper around an on-chain [`BalancerV2Vault`](self) contract instance.

See the [wrapper's documentation](`BalancerV2VaultInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> BalancerV2VaultInstance<P, N> {
        BalancerV2VaultInstance::<P, N>::new(address, __provider)
    }
    /**Deploys this contract using the given `provider` and constructor arguments, if any.

Returns a new instance of the contract, if the deployment was successful.

For more fine-grained control over the deployment process, use [`deploy_builder`] instead.*/
    #[inline]
    pub fn deploy<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        __provider: P,
        authorizer: alloy_sol_types::private::Address,
        weth: alloy_sol_types::private::Address,
        pauseWindowDuration: alloy_sol_types::private::primitives::aliases::U256,
        bufferPeriodDuration: alloy_sol_types::private::primitives::aliases::U256,
    ) -> impl ::core::future::Future<
        Output = alloy_contract::Result<BalancerV2VaultInstance<P, N>>,
    > {
        BalancerV2VaultInstance::<
            P,
            N,
        >::deploy(
            __provider,
            authorizer,
            weth,
            pauseWindowDuration,
            bufferPeriodDuration,
        )
    }
    /**Creates a `RawCallBuilder` for deploying this contract using the given `provider`
and constructor arguments, if any.

This is a simple wrapper around creating a `RawCallBuilder` with the data set to
the bytecode concatenated with the constructor's ABI-encoded arguments.*/
    #[inline]
    pub fn deploy_builder<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        __provider: P,
        authorizer: alloy_sol_types::private::Address,
        weth: alloy_sol_types::private::Address,
        pauseWindowDuration: alloy_sol_types::private::primitives::aliases::U256,
        bufferPeriodDuration: alloy_sol_types::private::primitives::aliases::U256,
    ) -> alloy_contract::RawCallBuilder<P, N> {
        BalancerV2VaultInstance::<
            P,
            N,
        >::deploy_builder(
            __provider,
            authorizer,
            weth,
            pauseWindowDuration,
            bufferPeriodDuration,
        )
    }
    /**A [`BalancerV2Vault`](self) instance.

Contains type-safe methods for interacting with an on-chain instance of the
[`BalancerV2Vault`](self) contract located at a given `address`, using a given
provider `P`.

If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
documentation on how to provide it), the `deploy` and `deploy_builder` methods can
be used to deploy a new instance of the contract.

See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct BalancerV2VaultInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for BalancerV2VaultInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("BalancerV2VaultInstance").field(&self.address).finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    > BalancerV2VaultInstance<P, N> {
        /**Creates a new wrapper around an on-chain [`BalancerV2Vault`](self) contract instance.

See the [wrapper's documentation](`BalancerV2VaultInstance`) for more details.*/
        #[inline]
        pub const fn new(
            address: alloy_sol_types::private::Address,
            __provider: P,
        ) -> Self {
            Self {
                address,
                provider: __provider,
                _network: ::core::marker::PhantomData,
            }
        }
        /**Deploys this contract using the given `provider` and constructor arguments, if any.

Returns a new instance of the contract, if the deployment was successful.

For more fine-grained control over the deployment process, use [`deploy_builder`] instead.*/
        #[inline]
        pub async fn deploy(
            __provider: P,
            authorizer: alloy_sol_types::private::Address,
            weth: alloy_sol_types::private::Address,
            pauseWindowDuration: alloy_sol_types::private::primitives::aliases::U256,
            bufferPeriodDuration: alloy_sol_types::private::primitives::aliases::U256,
        ) -> alloy_contract::Result<BalancerV2VaultInstance<P, N>> {
            let call_builder = Self::deploy_builder(
                __provider,
                authorizer,
                weth,
                pauseWindowDuration,
                bufferPeriodDuration,
            );
            let contract_address = call_builder.deploy().await?;
            Ok(Self::new(contract_address, call_builder.provider))
        }
        /**Creates a `RawCallBuilder` for deploying this contract using the given `provider`
and constructor arguments, if any.

This is a simple wrapper around creating a `RawCallBuilder` with the data set to
the bytecode concatenated with the constructor's ABI-encoded arguments.*/
        #[inline]
        pub fn deploy_builder(
            __provider: P,
            authorizer: alloy_sol_types::private::Address,
            weth: alloy_sol_types::private::Address,
            pauseWindowDuration: alloy_sol_types::private::primitives::aliases::U256,
            bufferPeriodDuration: alloy_sol_types::private::primitives::aliases::U256,
        ) -> alloy_contract::RawCallBuilder<P, N> {
            alloy_contract::RawCallBuilder::new_raw_deploy(
                __provider,
                [
                    &BYTECODE[..],
                    &alloy_sol_types::SolConstructor::abi_encode(
                        &constructorCall {
                            authorizer,
                            weth,
                            pauseWindowDuration,
                            bufferPeriodDuration,
                        },
                    )[..],
                ]
                    .concat()
                    .into(),
            )
        }
        /// Returns a reference to the address.
        #[inline]
        pub const fn address(&self) -> &alloy_sol_types::private::Address {
            &self.address
        }
        /// Sets the address.
        #[inline]
        pub fn set_address(&mut self, address: alloy_sol_types::private::Address) {
            self.address = address;
        }
        /// Sets the address and returns `self`.
        pub fn at(mut self, address: alloy_sol_types::private::Address) -> Self {
            self.set_address(address);
            self
        }
        /// Returns a reference to the provider.
        #[inline]
        pub const fn provider(&self) -> &P {
            &self.provider
        }
    }
    impl<P: ::core::clone::Clone, N> BalancerV2VaultInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned provider.
        #[inline]
        pub fn with_cloned_provider(self) -> BalancerV2VaultInstance<P, N> {
            BalancerV2VaultInstance {
                address: self.address,
                provider: ::core::clone::Clone::clone(&self.provider),
                _network: ::core::marker::PhantomData,
            }
        }
    }
    /// Function calls.
    impl<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    > BalancerV2VaultInstance<P, N> {
        /// Creates a new call builder using this contract instance's provider and address.
        ///
        /// Note that the call can be any function call, not just those defined in this
        /// contract. Prefer using the other methods for building type-safe contract calls.
        pub fn call_builder<C: alloy_sol_types::SolCall>(
            &self,
            call: &C,
        ) -> alloy_contract::SolCallBuilder<&P, C, N> {
            alloy_contract::SolCallBuilder::new_sol(&self.provider, &self.address, call)
        }
        ///Creates a new call builder for the [`WETH`] function.
        pub fn WETH(&self) -> alloy_contract::SolCallBuilder<&P, WETHCall, N> {
            self.call_builder(&WETHCall)
        }
        ///Creates a new call builder for the [`batchSwap`] function.
        pub fn batchSwap(
            &self,
            kind: <IVault::SwapKind as alloy_sol_types::SolType>::RustType,
            swaps: alloy_sol_types::private::Vec<
                <IVault::BatchSwapStep as alloy_sol_types::SolType>::RustType,
            >,
            assets: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
            funds: <IVault::FundManagement as alloy_sol_types::SolType>::RustType,
            limits: alloy_sol_types::private::Vec<
                alloy_sol_types::private::primitives::aliases::I256,
            >,
            deadline: alloy_sol_types::private::primitives::aliases::U256,
        ) -> alloy_contract::SolCallBuilder<&P, batchSwapCall, N> {
            self.call_builder(
                &batchSwapCall {
                    kind,
                    swaps,
                    assets,
                    funds,
                    limits,
                    deadline,
                },
            )
        }
        ///Creates a new call builder for the [`flashLoan`] function.
        pub fn flashLoan(
            &self,
            recipient: alloy_sol_types::private::Address,
            tokens: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
            amounts: alloy_sol_types::private::Vec<
                alloy_sol_types::private::primitives::aliases::U256,
            >,
            userData: alloy_sol_types::private::Bytes,
        ) -> alloy_contract::SolCallBuilder<&P, flashLoanCall, N> {
            self.call_builder(
                &flashLoanCall {
                    recipient,
                    tokens,
                    amounts,
                    userData,
                },
            )
        }
        ///Creates a new call builder for the [`getInternalBalance`] function.
        pub fn getInternalBalance(
            &self,
            user: alloy_sol_types::private::Address,
            tokens: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
        ) -> alloy_contract::SolCallBuilder<&P, getInternalBalanceCall, N> {
            self.call_builder(
                &getInternalBalanceCall {
                    user,
                    tokens,
                },
            )
        }
        ///Creates a new call builder for the [`getPausedState`] function.
        pub fn getPausedState(
            &self,
        ) -> alloy_contract::SolCallBuilder<&P, getPausedStateCall, N> {
            self.call_builder(&getPausedStateCall)
        }
        ///Creates a new call builder for the [`getPool`] function.
        pub fn getPool(
            &self,
            poolId: alloy_sol_types::private::FixedBytes<32>,
        ) -> alloy_contract::SolCallBuilder<&P, getPoolCall, N> {
            self.call_builder(&getPoolCall { poolId })
        }
        ///Creates a new call builder for the [`getPoolTokens`] function.
        pub fn getPoolTokens(
            &self,
            poolId: alloy_sol_types::private::FixedBytes<32>,
        ) -> alloy_contract::SolCallBuilder<&P, getPoolTokensCall, N> {
            self.call_builder(&getPoolTokensCall { poolId })
        }
        ///Creates a new call builder for the [`hasApprovedRelayer`] function.
        pub fn hasApprovedRelayer(
            &self,
            user: alloy_sol_types::private::Address,
            relayer: alloy_sol_types::private::Address,
        ) -> alloy_contract::SolCallBuilder<&P, hasApprovedRelayerCall, N> {
            self.call_builder(
                &hasApprovedRelayerCall {
                    user,
                    relayer,
                },
            )
        }
        ///Creates a new call builder for the [`manageUserBalance`] function.
        pub fn manageUserBalance(
            &self,
            ops: alloy_sol_types::private::Vec<
                <IVault::UserBalanceOp as alloy_sol_types::SolType>::RustType,
            >,
        ) -> alloy_contract::SolCallBuilder<&P, manageUserBalanceCall, N> {
            self.call_builder(&manageUserBalanceCall { ops })
        }
        ///Creates a new call builder for the [`setRelayerApproval`] function.
        pub fn setRelayerApproval(
            &self,
            sender: alloy_sol_types::private::Address,
            relayer: alloy_sol_types::private::Address,
            approved: bool,
        ) -> alloy_contract::SolCallBuilder<&P, setRelayerApprovalCall, N> {
            self.call_builder(
                &setRelayerApprovalCall {
                    sender,
                    relayer,
                    approved,
                },
            )
        }
        ///Creates a new call builder for the [`swap`] function.
        pub fn swap(
            &self,
            singleSwap: <IVault::SingleSwap as alloy_sol_types::SolType>::RustType,
            funds: <IVault::FundManagement as alloy_sol_types::SolType>::RustType,
            limit: alloy_sol_types::private::primitives::aliases::U256,
            deadline: alloy_sol_types::private::primitives::aliases::U256,
        ) -> alloy_contract::SolCallBuilder<&P, swapCall, N> {
            self.call_builder(
                &swapCall {
                    singleSwap,
                    funds,
                    limit,
                    deadline,
                },
            )
        }
    }
    /// Event filters.
    impl<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    > BalancerV2VaultInstance<P, N> {
        /// Creates a new event filter using this contract instance's provider and address.
        ///
        /// Note that the type can be any event, not just those defined in this contract.
        /// Prefer using the other methods for building type-safe event filters.
        pub fn event_filter<E: alloy_sol_types::SolEvent>(
            &self,
        ) -> alloy_contract::Event<&P, E, N> {
            alloy_contract::Event::new_sol(&self.provider, &self.address)
        }
        ///Creates a new event filter for the [`AuthorizerChanged`] event.
        pub fn AuthorizerChanged_filter(
            &self,
        ) -> alloy_contract::Event<&P, AuthorizerChanged, N> {
            self.event_filter::<AuthorizerChanged>()
        }
        ///Creates a new event filter for the [`ExternalBalanceTransfer`] event.
        pub fn ExternalBalanceTransfer_filter(
            &self,
        ) -> alloy_contract::Event<&P, ExternalBalanceTransfer, N> {
            self.event_filter::<ExternalBalanceTransfer>()
        }
        ///Creates a new event filter for the [`FlashLoan`] event.
        pub fn FlashLoan_filter(&self) -> alloy_contract::Event<&P, FlashLoan, N> {
            self.event_filter::<FlashLoan>()
        }
        ///Creates a new event filter for the [`InternalBalanceChanged`] event.
        pub fn InternalBalanceChanged_filter(
            &self,
        ) -> alloy_contract::Event<&P, InternalBalanceChanged, N> {
            self.event_filter::<InternalBalanceChanged>()
        }
        ///Creates a new event filter for the [`PausedStateChanged`] event.
        pub fn PausedStateChanged_filter(
            &self,
        ) -> alloy_contract::Event<&P, PausedStateChanged, N> {
            self.event_filter::<PausedStateChanged>()
        }
        ///Creates a new event filter for the [`PoolBalanceChanged`] event.
        pub fn PoolBalanceChanged_filter(
            &self,
        ) -> alloy_contract::Event<&P, PoolBalanceChanged, N> {
            self.event_filter::<PoolBalanceChanged>()
        }
        ///Creates a new event filter for the [`PoolBalanceManaged`] event.
        pub fn PoolBalanceManaged_filter(
            &self,
        ) -> alloy_contract::Event<&P, PoolBalanceManaged, N> {
            self.event_filter::<PoolBalanceManaged>()
        }
        ///Creates a new event filter for the [`PoolRegistered`] event.
        pub fn PoolRegistered_filter(
            &self,
        ) -> alloy_contract::Event<&P, PoolRegistered, N> {
            self.event_filter::<PoolRegistered>()
        }
        ///Creates a new event filter for the [`RelayerApprovalChanged`] event.
        pub fn RelayerApprovalChanged_filter(
            &self,
        ) -> alloy_contract::Event<&P, RelayerApprovalChanged, N> {
            self.event_filter::<RelayerApprovalChanged>()
        }
        ///Creates a new event filter for the [`Swap`] event.
        pub fn Swap_filter(&self) -> alloy_contract::Event<&P, Swap, N> {
            self.event_filter::<Swap>()
        }
        ///Creates a new event filter for the [`TokensDeregistered`] event.
        pub fn TokensDeregistered_filter(
            &self,
        ) -> alloy_contract::Event<&P, TokensDeregistered, N> {
            self.event_filter::<TokensDeregistered>()
        }
        ///Creates a new event filter for the [`TokensRegistered`] event.
        pub fn TokensRegistered_filter(
            &self,
        ) -> alloy_contract::Event<&P, TokensRegistered, N> {
            self.event_filter::<TokensRegistered>()
        }
    }
}
pub type Instance = BalancerV2Vault::BalancerV2VaultInstance<
    ::alloy_provider::DynProvider,
>;
use {
    std::{sync::LazyLock, collections::HashMap},
    anyhow::{Result, Context},
    alloy_primitives::{address, Address},
    alloy_provider::{Provider, DynProvider},
};
pub const fn deployment_info(chain_id: u64) -> Option<(Address, Option<u64>)> {
    match chain_id {
        11155111u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0xBA12222222228d8Ba445958a75a0704d566BF2C8"
                ),
                Some(3418831u64),
            ))
        }
        56u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0xBA12222222228d8Ba445958a75a0704d566BF2C8"
                ),
                Some(22691002u64),
            ))
        }
        43114u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0xBA12222222228d8Ba445958a75a0704d566BF2C8"
                ),
                Some(26386141u64),
            ))
        }
        42161u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0xBA12222222228d8Ba445958a75a0704d566BF2C8"
                ),
                Some(222832u64),
            ))
        }
        10u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0xBA12222222228d8Ba445958a75a0704d566BF2C8"
                ),
                Some(7003431u64),
            ))
        }
        137u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0xBA12222222228d8Ba445958a75a0704d566BF2C8"
                ),
                Some(15832990u64),
            ))
        }
        1u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0xBA12222222228d8Ba445958a75a0704d566BF2C8"
                ),
                Some(12272146u64),
            ))
        }
        100u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0xBA12222222228d8Ba445958a75a0704d566BF2C8"
                ),
                Some(24821598u64),
            ))
        }
        8453u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0xBA12222222228d8Ba445958a75a0704d566BF2C8"
                ),
                Some(1196036u64),
            ))
        }
        57073u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0xBA12222222228d8Ba445958a75a0704d566BF2C8"
                ),
                Some(34313901u64),
            ))
        }
        _ => None,
    }
}
pub const fn deployment_address(chain_id: &u64) -> Option<::alloy_primitives::Address> {
    match deployment_info(*chain_id) {
        Some((address, _)) => Some(address),
        None => None,
    }
}
pub const fn deployment_block(chain_id: &u64) -> Option<u64> {
    match deployment_info(*chain_id) {
        Some((_, block)) => block,
        None => None,
    }
}
impl Instance {
    pub fn deployed(
        provider: &DynProvider,
    ) -> impl Future<Output = Result<Self>> + Send {
        async move {
            let chain_id = provider
                .get_chain_id()
                .await
                .context("could not fetch current chain id")?;
            let (address, _deployed_block) = deployment_info(chain_id)
                .with_context(|| format!("no deployment info for chain {chain_id:?}"))?;
            Ok(Instance::new(address, provider.clone()))
        }
    }
}
