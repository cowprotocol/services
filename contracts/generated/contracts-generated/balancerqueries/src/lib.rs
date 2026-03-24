#![allow(
    unused_imports,
    unused_attributes,
    clippy::all,
    rustdoc::all,
    non_snake_case
)]
//! Auto-generated contract bindings. Do not edit.
///Module containing a contract's types and functions.
/**

```solidity
library IVault {
    type SwapKind is uint8;
    struct BatchSwapStep { bytes32 poolId; uint256 assetInIndex; uint256 assetOutIndex; uint256 amount; bytes userData; }
    struct ExitPoolRequest { address[] assets; uint256[] minAmountsOut; bytes userData; bool toInternalBalance; }
    struct FundManagement { address sender; bool fromInternalBalance; address recipient; bool toInternalBalance; }
    struct JoinPoolRequest { address[] assets; uint256[] maxAmountsIn; bytes userData; bool fromInternalBalance; }
    struct SingleSwap { bytes32 poolId; SwapKind kind; address assetIn; address assetOut; uint256 amount; bytes userData; }
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
    use alloy_sol_types;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct SwapKind(u8);
    const _: () = {
        use alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<SwapKind> for u8 {
            #[inline]
            fn stv_to_tokens(
                &self,
            ) -> <alloy_sol_types::sol_data::Uint<8> as alloy_sol_types::SolType>::Token<'_>
            {
                alloy_sol_types::private::SolTypeValue::<
                    alloy_sol_types::sol_data::Uint<8>,
                >::stv_to_tokens(self)
            }
            #[inline]
            fn stv_eip712_data_word(&self) -> alloy_sol_types::Word {
                <alloy_sol_types::sol_data::Uint<8> as alloy_sol_types::SolType>::tokenize(self).0
            }
            #[inline]
            fn stv_abi_encode_packed_to(&self, out: &mut alloy_sol_types::private::Vec<u8>) {
                <alloy_sol_types::sol_data::Uint<
                    8,
                > as alloy_sol_types::SolType>::abi_encode_packed_to(self, out)
            }
            #[inline]
            fn stv_abi_packed_encoded_size(&self) -> usize {
                <alloy_sol_types::sol_data::Uint<8> as alloy_sol_types::SolType>::abi_encoded_size(
                    self,
                )
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
            type Token<'a> =
                <alloy_sol_types::sol_data::Uint<8> as alloy_sol_types::SolType>::Token<'a>;
            const SOL_NAME: &'static str = Self::NAME;
            const ENCODED_SIZE: Option<usize> =
                <alloy_sol_types::sol_data::Uint<8> as alloy_sol_types::SolType>::ENCODED_SIZE;
            const PACKED_ENCODED_SIZE: Option<usize> = <alloy_sol_types::sol_data::Uint<
                8,
            > as alloy_sol_types::SolType>::PACKED_ENCODED_SIZE;
            #[inline]
            fn valid_token(token: &Self::Token<'_>) -> bool {
                Self::type_check(token).is_ok()
            }
            #[inline]
            fn type_check(token: &Self::Token<'_>) -> alloy_sol_types::Result<()> {
                <alloy_sol_types::sol_data::Uint<8> as alloy_sol_types::SolType>::type_check(token)
            }
            #[inline]
            fn detokenize(token: Self::Token<'_>) -> Self::RustType {
                <alloy_sol_types::sol_data::Uint<8> as alloy_sol_types::SolType>::detokenize(token)
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
            fn encode_topic(rust: &Self::RustType) -> alloy_sol_types::abi::token::WordToken {
                <alloy_sol_types::sol_data::Uint<8> as alloy_sol_types::EventTopic>::encode_topic(
                    rust,
                )
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
        use alloy_sol_types;
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
        fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
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
                let tuple =
                    <UnderlyingRustTuple<'_> as ::core::convert::From<Self>>::from(self.clone());
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::abi_encoded_size(&tuple)
            }
            #[inline]
            fn stv_eip712_data_word(&self) -> alloy_sol_types::Word {
                <Self as alloy_sol_types::SolStruct>::eip712_hash_struct(self)
            }
            #[inline]
            fn stv_abi_encode_packed_to(&self, out: &mut alloy_sol_types::private::Vec<u8>) {
                let tuple =
                    <UnderlyingRustTuple<'_> as ::core::convert::From<Self>>::from(self.clone());
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::abi_encode_packed_to(
                    &tuple, out,
                )
            }
            #[inline]
            fn stv_abi_packed_encoded_size(&self) -> usize {
                if let Some(size) = <Self as alloy_sol_types::SolType>::PACKED_ENCODED_SIZE {
                    return size;
                }
                let tuple =
                    <UnderlyingRustTuple<'_> as ::core::convert::From<Self>>::from(self.clone());
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::abi_packed_encoded_size(
                    &tuple,
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolType for BatchSwapStep {
            type RustType = Self;
            type Token<'a> = <UnderlyingSolTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SOL_NAME: &'static str = <Self as alloy_sol_types::SolStruct>::NAME;
            const ENCODED_SIZE: Option<usize> =
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::ENCODED_SIZE;
            const PACKED_ENCODED_SIZE: Option<usize> =
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::PACKED_ENCODED_SIZE;
            #[inline]
            fn valid_token(token: &Self::Token<'_>) -> bool {
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::valid_token(token)
            }
            #[inline]
            fn detokenize(token: Self::Token<'_>) -> Self::RustType {
                let tuple = <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::detokenize(token);
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
            fn eip712_components()
            -> alloy_sol_types::private::Vec<alloy_sol_types::private::Cow<'static, str>>
            {
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
                out.reserve(<Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust));
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
            fn encode_topic(rust: &Self::RustType) -> alloy_sol_types::abi::token::WordToken {
                let mut out = alloy_sol_types::private::Vec::new();
                <Self as alloy_sol_types::EventTopic>::encode_topic_preimage(rust, &mut out);
                alloy_sol_types::abi::token::WordToken(alloy_sol_types::private::keccak256(out))
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**```solidity
    struct ExitPoolRequest { address[] assets; uint256[] minAmountsOut; bytes userData; bool toInternalBalance; }
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct ExitPoolRequest {
        #[allow(missing_docs)]
        pub assets: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
        #[allow(missing_docs)]
        pub minAmountsOut:
            alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U256>,
        #[allow(missing_docs)]
        pub userData: alloy_sol_types::private::Bytes,
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
        use alloy_sol_types;
        #[doc(hidden)]
        #[allow(dead_code)]
        type UnderlyingSolTuple<'a> = (
            alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
            alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
            alloy_sol_types::sol_data::Bytes,
            alloy_sol_types::sol_data::Bool,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
            alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U256>,
            alloy_sol_types::private::Bytes,
            bool,
        );
        #[cfg(test)]
        #[allow(dead_code, unreachable_patterns)]
        fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
            match _t {
                alloy_sol_types::private::AssertTypeEq::<
                    <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                >(_) => {}
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<ExitPoolRequest> for UnderlyingRustTuple<'_> {
            fn from(value: ExitPoolRequest) -> Self {
                (
                    value.assets,
                    value.minAmountsOut,
                    value.userData,
                    value.toInternalBalance,
                )
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for ExitPoolRequest {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    assets: tuple.0,
                    minAmountsOut: tuple.1,
                    userData: tuple.2,
                    toInternalBalance: tuple.3,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for ExitPoolRequest {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for ExitPoolRequest {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Address,
                    > as alloy_sol_types::SolType>::tokenize(&self.assets),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::SolType>::tokenize(&self.minAmountsOut),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.userData,
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
                let tuple =
                    <UnderlyingRustTuple<'_> as ::core::convert::From<Self>>::from(self.clone());
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::abi_encoded_size(&tuple)
            }
            #[inline]
            fn stv_eip712_data_word(&self) -> alloy_sol_types::Word {
                <Self as alloy_sol_types::SolStruct>::eip712_hash_struct(self)
            }
            #[inline]
            fn stv_abi_encode_packed_to(&self, out: &mut alloy_sol_types::private::Vec<u8>) {
                let tuple =
                    <UnderlyingRustTuple<'_> as ::core::convert::From<Self>>::from(self.clone());
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::abi_encode_packed_to(
                    &tuple, out,
                )
            }
            #[inline]
            fn stv_abi_packed_encoded_size(&self) -> usize {
                if let Some(size) = <Self as alloy_sol_types::SolType>::PACKED_ENCODED_SIZE {
                    return size;
                }
                let tuple =
                    <UnderlyingRustTuple<'_> as ::core::convert::From<Self>>::from(self.clone());
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::abi_packed_encoded_size(
                    &tuple,
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolType for ExitPoolRequest {
            type RustType = Self;
            type Token<'a> = <UnderlyingSolTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SOL_NAME: &'static str = <Self as alloy_sol_types::SolStruct>::NAME;
            const ENCODED_SIZE: Option<usize> =
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::ENCODED_SIZE;
            const PACKED_ENCODED_SIZE: Option<usize> =
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::PACKED_ENCODED_SIZE;
            #[inline]
            fn valid_token(token: &Self::Token<'_>) -> bool {
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::valid_token(token)
            }
            #[inline]
            fn detokenize(token: Self::Token<'_>) -> Self::RustType {
                let tuple = <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::detokenize(token);
                <Self as ::core::convert::From<UnderlyingRustTuple<'_>>>::from(tuple)
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolStruct for ExitPoolRequest {
            const NAME: &'static str = "ExitPoolRequest";
            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "ExitPoolRequest(address[] assets,uint256[] minAmountsOut,bytes userData,bool toInternalBalance)",
                )
            }
            #[inline]
            fn eip712_components()
            -> alloy_sol_types::private::Vec<alloy_sol_types::private::Cow<'static, str>>
            {
                alloy_sol_types::private::Vec::new()
            }
            #[inline]
            fn eip712_encode_type() -> alloy_sol_types::private::Cow<'static, str> {
                <Self as alloy_sol_types::SolStruct>::eip712_root_type()
            }
            #[inline]
            fn eip712_encode_data(&self) -> alloy_sol_types::private::Vec<u8> {
                [
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Address,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.assets)
                        .0,
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.minAmountsOut)
                        .0,
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::eip712_data_word(
                            &self.userData,
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
        impl alloy_sol_types::EventTopic for ExitPoolRequest {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Address,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.assets,
                    )
                    + <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.minAmountsOut,
                    )
                    + <alloy_sol_types::sol_data::Bytes as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.userData,
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
                out.reserve(<Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust));
                <alloy_sol_types::sol_data::Array<
                    alloy_sol_types::sol_data::Address,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.assets,
                    out,
                );
                <alloy_sol_types::sol_data::Array<
                    alloy_sol_types::sol_data::Uint<256>,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.minAmountsOut,
                    out,
                );
                <alloy_sol_types::sol_data::Bytes as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.userData,
                    out,
                );
                <alloy_sol_types::sol_data::Bool as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.toInternalBalance,
                    out,
                );
            }
            #[inline]
            fn encode_topic(rust: &Self::RustType) -> alloy_sol_types::abi::token::WordToken {
                let mut out = alloy_sol_types::private::Vec::new();
                <Self as alloy_sol_types::EventTopic>::encode_topic_preimage(rust, &mut out);
                alloy_sol_types::abi::token::WordToken(alloy_sol_types::private::keccak256(out))
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
        use alloy_sol_types;
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
        fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
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
                let tuple =
                    <UnderlyingRustTuple<'_> as ::core::convert::From<Self>>::from(self.clone());
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::abi_encoded_size(&tuple)
            }
            #[inline]
            fn stv_eip712_data_word(&self) -> alloy_sol_types::Word {
                <Self as alloy_sol_types::SolStruct>::eip712_hash_struct(self)
            }
            #[inline]
            fn stv_abi_encode_packed_to(&self, out: &mut alloy_sol_types::private::Vec<u8>) {
                let tuple =
                    <UnderlyingRustTuple<'_> as ::core::convert::From<Self>>::from(self.clone());
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::abi_encode_packed_to(
                    &tuple, out,
                )
            }
            #[inline]
            fn stv_abi_packed_encoded_size(&self) -> usize {
                if let Some(size) = <Self as alloy_sol_types::SolType>::PACKED_ENCODED_SIZE {
                    return size;
                }
                let tuple =
                    <UnderlyingRustTuple<'_> as ::core::convert::From<Self>>::from(self.clone());
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::abi_packed_encoded_size(
                    &tuple,
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolType for FundManagement {
            type RustType = Self;
            type Token<'a> = <UnderlyingSolTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SOL_NAME: &'static str = <Self as alloy_sol_types::SolStruct>::NAME;
            const ENCODED_SIZE: Option<usize> =
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::ENCODED_SIZE;
            const PACKED_ENCODED_SIZE: Option<usize> =
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::PACKED_ENCODED_SIZE;
            #[inline]
            fn valid_token(token: &Self::Token<'_>) -> bool {
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::valid_token(token)
            }
            #[inline]
            fn detokenize(token: Self::Token<'_>) -> Self::RustType {
                let tuple = <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::detokenize(token);
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
            fn eip712_components()
            -> alloy_sol_types::private::Vec<alloy_sol_types::private::Cow<'static, str>>
            {
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
                out.reserve(<Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust));
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
            fn encode_topic(rust: &Self::RustType) -> alloy_sol_types::abi::token::WordToken {
                let mut out = alloy_sol_types::private::Vec::new();
                <Self as alloy_sol_types::EventTopic>::encode_topic_preimage(rust, &mut out);
                alloy_sol_types::abi::token::WordToken(alloy_sol_types::private::keccak256(out))
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**```solidity
    struct JoinPoolRequest { address[] assets; uint256[] maxAmountsIn; bytes userData; bool fromInternalBalance; }
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct JoinPoolRequest {
        #[allow(missing_docs)]
        pub assets: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
        #[allow(missing_docs)]
        pub maxAmountsIn:
            alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U256>,
        #[allow(missing_docs)]
        pub userData: alloy_sol_types::private::Bytes,
        #[allow(missing_docs)]
        pub fromInternalBalance: bool,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types;
        #[doc(hidden)]
        #[allow(dead_code)]
        type UnderlyingSolTuple<'a> = (
            alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
            alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
            alloy_sol_types::sol_data::Bytes,
            alloy_sol_types::sol_data::Bool,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
            alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U256>,
            alloy_sol_types::private::Bytes,
            bool,
        );
        #[cfg(test)]
        #[allow(dead_code, unreachable_patterns)]
        fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
            match _t {
                alloy_sol_types::private::AssertTypeEq::<
                    <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                >(_) => {}
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<JoinPoolRequest> for UnderlyingRustTuple<'_> {
            fn from(value: JoinPoolRequest) -> Self {
                (
                    value.assets,
                    value.maxAmountsIn,
                    value.userData,
                    value.fromInternalBalance,
                )
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for JoinPoolRequest {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    assets: tuple.0,
                    maxAmountsIn: tuple.1,
                    userData: tuple.2,
                    fromInternalBalance: tuple.3,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for JoinPoolRequest {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for JoinPoolRequest {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Address,
                    > as alloy_sol_types::SolType>::tokenize(&self.assets),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::SolType>::tokenize(&self.maxAmountsIn),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.userData,
                    ),
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::tokenize(
                        &self.fromInternalBalance,
                    ),
                )
            }
            #[inline]
            fn stv_abi_encoded_size(&self) -> usize {
                if let Some(size) = <Self as alloy_sol_types::SolType>::ENCODED_SIZE {
                    return size;
                }
                let tuple =
                    <UnderlyingRustTuple<'_> as ::core::convert::From<Self>>::from(self.clone());
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::abi_encoded_size(&tuple)
            }
            #[inline]
            fn stv_eip712_data_word(&self) -> alloy_sol_types::Word {
                <Self as alloy_sol_types::SolStruct>::eip712_hash_struct(self)
            }
            #[inline]
            fn stv_abi_encode_packed_to(&self, out: &mut alloy_sol_types::private::Vec<u8>) {
                let tuple =
                    <UnderlyingRustTuple<'_> as ::core::convert::From<Self>>::from(self.clone());
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::abi_encode_packed_to(
                    &tuple, out,
                )
            }
            #[inline]
            fn stv_abi_packed_encoded_size(&self) -> usize {
                if let Some(size) = <Self as alloy_sol_types::SolType>::PACKED_ENCODED_SIZE {
                    return size;
                }
                let tuple =
                    <UnderlyingRustTuple<'_> as ::core::convert::From<Self>>::from(self.clone());
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::abi_packed_encoded_size(
                    &tuple,
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolType for JoinPoolRequest {
            type RustType = Self;
            type Token<'a> = <UnderlyingSolTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SOL_NAME: &'static str = <Self as alloy_sol_types::SolStruct>::NAME;
            const ENCODED_SIZE: Option<usize> =
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::ENCODED_SIZE;
            const PACKED_ENCODED_SIZE: Option<usize> =
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::PACKED_ENCODED_SIZE;
            #[inline]
            fn valid_token(token: &Self::Token<'_>) -> bool {
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::valid_token(token)
            }
            #[inline]
            fn detokenize(token: Self::Token<'_>) -> Self::RustType {
                let tuple = <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::detokenize(token);
                <Self as ::core::convert::From<UnderlyingRustTuple<'_>>>::from(tuple)
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolStruct for JoinPoolRequest {
            const NAME: &'static str = "JoinPoolRequest";
            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "JoinPoolRequest(address[] assets,uint256[] maxAmountsIn,bytes userData,bool fromInternalBalance)",
                )
            }
            #[inline]
            fn eip712_components()
            -> alloy_sol_types::private::Vec<alloy_sol_types::private::Cow<'static, str>>
            {
                alloy_sol_types::private::Vec::new()
            }
            #[inline]
            fn eip712_encode_type() -> alloy_sol_types::private::Cow<'static, str> {
                <Self as alloy_sol_types::SolStruct>::eip712_root_type()
            }
            #[inline]
            fn eip712_encode_data(&self) -> alloy_sol_types::private::Vec<u8> {
                [
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Address,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.assets)
                        .0,
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.maxAmountsIn)
                        .0,
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::eip712_data_word(
                            &self.userData,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::eip712_data_word(
                            &self.fromInternalBalance,
                        )
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for JoinPoolRequest {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Address,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.assets,
                    )
                    + <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.maxAmountsIn,
                    )
                    + <alloy_sol_types::sol_data::Bytes as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.userData,
                    )
                    + <alloy_sol_types::sol_data::Bool as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.fromInternalBalance,
                    )
            }
            #[inline]
            fn encode_topic_preimage(
                rust: &Self::RustType,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                out.reserve(<Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust));
                <alloy_sol_types::sol_data::Array<
                    alloy_sol_types::sol_data::Address,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.assets,
                    out,
                );
                <alloy_sol_types::sol_data::Array<
                    alloy_sol_types::sol_data::Uint<256>,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.maxAmountsIn,
                    out,
                );
                <alloy_sol_types::sol_data::Bytes as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.userData,
                    out,
                );
                <alloy_sol_types::sol_data::Bool as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.fromInternalBalance,
                    out,
                );
            }
            #[inline]
            fn encode_topic(rust: &Self::RustType) -> alloy_sol_types::abi::token::WordToken {
                let mut out = alloy_sol_types::private::Vec::new();
                <Self as alloy_sol_types::EventTopic>::encode_topic_preimage(rust, &mut out);
                alloy_sol_types::abi::token::WordToken(alloy_sol_types::private::keccak256(out))
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
        use alloy_sol_types;
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
        fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
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
                let tuple =
                    <UnderlyingRustTuple<'_> as ::core::convert::From<Self>>::from(self.clone());
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::abi_encoded_size(&tuple)
            }
            #[inline]
            fn stv_eip712_data_word(&self) -> alloy_sol_types::Word {
                <Self as alloy_sol_types::SolStruct>::eip712_hash_struct(self)
            }
            #[inline]
            fn stv_abi_encode_packed_to(&self, out: &mut alloy_sol_types::private::Vec<u8>) {
                let tuple =
                    <UnderlyingRustTuple<'_> as ::core::convert::From<Self>>::from(self.clone());
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::abi_encode_packed_to(
                    &tuple, out,
                )
            }
            #[inline]
            fn stv_abi_packed_encoded_size(&self) -> usize {
                if let Some(size) = <Self as alloy_sol_types::SolType>::PACKED_ENCODED_SIZE {
                    return size;
                }
                let tuple =
                    <UnderlyingRustTuple<'_> as ::core::convert::From<Self>>::from(self.clone());
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::abi_packed_encoded_size(
                    &tuple,
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolType for SingleSwap {
            type RustType = Self;
            type Token<'a> = <UnderlyingSolTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SOL_NAME: &'static str = <Self as alloy_sol_types::SolStruct>::NAME;
            const ENCODED_SIZE: Option<usize> =
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::ENCODED_SIZE;
            const PACKED_ENCODED_SIZE: Option<usize> =
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::PACKED_ENCODED_SIZE;
            #[inline]
            fn valid_token(token: &Self::Token<'_>) -> bool {
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::valid_token(token)
            }
            #[inline]
            fn detokenize(token: Self::Token<'_>) -> Self::RustType {
                let tuple = <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::detokenize(token);
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
            fn eip712_components()
            -> alloy_sol_types::private::Vec<alloy_sol_types::private::Cow<'static, str>>
            {
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
                out.reserve(<Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust));
                <alloy_sol_types::sol_data::FixedBytes<
                    32,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.poolId,
                    out,
                );
                <SwapKind as alloy_sol_types::EventTopic>::encode_topic_preimage(&rust.kind, out);
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
            fn encode_topic(rust: &Self::RustType) -> alloy_sol_types::abi::token::WordToken {
                let mut out = alloy_sol_types::private::Vec::new();
                <Self as alloy_sol_types::EventTopic>::encode_topic_preimage(rust, &mut out);
                alloy_sol_types::abi::token::WordToken(alloy_sol_types::private::keccak256(out))
            }
        }
    };
    use alloy_contract;
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
            f.debug_tuple("IVaultInstance")
                .field(&self.address)
                .finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        IVaultInstance<P, N>
    {
        /**Creates a new wrapper around an on-chain [`IVault`](self) contract instance.

        See the [wrapper's documentation](`IVaultInstance`) for more details.*/
        #[inline]
        pub const fn new(address: alloy_sol_types::private::Address, __provider: P) -> Self {
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
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        IVaultInstance<P, N>
    {
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
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        IVaultInstance<P, N>
    {
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
    type SwapKind is uint8;
    struct BatchSwapStep {
        bytes32 poolId;
        uint256 assetInIndex;
        uint256 assetOutIndex;
        uint256 amount;
        bytes userData;
    }
    struct ExitPoolRequest {
        address[] assets;
        uint256[] minAmountsOut;
        bytes userData;
        bool toInternalBalance;
    }
    struct FundManagement {
        address sender;
        bool fromInternalBalance;
        address payable recipient;
        bool toInternalBalance;
    }
    struct JoinPoolRequest {
        address[] assets;
        uint256[] maxAmountsIn;
        bytes userData;
        bool fromInternalBalance;
    }
    struct SingleSwap {
        bytes32 poolId;
        SwapKind kind;
        address assetIn;
        address assetOut;
        uint256 amount;
        bytes userData;
    }
}

interface BalancerQueries {
    constructor(address _vault);

    function queryBatchSwap(IVault.SwapKind kind, IVault.BatchSwapStep[] memory swaps, address[] memory assets, IVault.FundManagement memory funds) external returns (int256[] memory assetDeltas);
    function queryExit(bytes32 poolId, address sender, address recipient, IVault.ExitPoolRequest memory request) external returns (uint256 bptIn, uint256[] memory amountsOut);
    function queryJoin(bytes32 poolId, address sender, address recipient, IVault.JoinPoolRequest memory request) external returns (uint256 bptOut, uint256[] memory amountsIn);
    function querySwap(IVault.SingleSwap memory singleSwap, IVault.FundManagement memory funds) external returns (uint256);
    function vault() external view returns (address);
}
```

...which was generated by the following JSON ABI:
```json
[
  {
    "type": "constructor",
    "inputs": [
      {
        "name": "_vault",
        "type": "address",
        "internalType": "contract IVault"
      }
    ],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "queryBatchSwap",
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
      }
    ],
    "outputs": [
      {
        "name": "assetDeltas",
        "type": "int256[]",
        "internalType": "int256[]"
      }
    ],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "queryExit",
    "inputs": [
      {
        "name": "poolId",
        "type": "bytes32",
        "internalType": "bytes32"
      },
      {
        "name": "sender",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "recipient",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "request",
        "type": "tuple",
        "internalType": "struct IVault.ExitPoolRequest",
        "components": [
          {
            "name": "assets",
            "type": "address[]",
            "internalType": "contract IAsset[]"
          },
          {
            "name": "minAmountsOut",
            "type": "uint256[]",
            "internalType": "uint256[]"
          },
          {
            "name": "userData",
            "type": "bytes",
            "internalType": "bytes"
          },
          {
            "name": "toInternalBalance",
            "type": "bool",
            "internalType": "bool"
          }
        ]
      }
    ],
    "outputs": [
      {
        "name": "bptIn",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "amountsOut",
        "type": "uint256[]",
        "internalType": "uint256[]"
      }
    ],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "queryJoin",
    "inputs": [
      {
        "name": "poolId",
        "type": "bytes32",
        "internalType": "bytes32"
      },
      {
        "name": "sender",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "recipient",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "request",
        "type": "tuple",
        "internalType": "struct IVault.JoinPoolRequest",
        "components": [
          {
            "name": "assets",
            "type": "address[]",
            "internalType": "contract IAsset[]"
          },
          {
            "name": "maxAmountsIn",
            "type": "uint256[]",
            "internalType": "uint256[]"
          },
          {
            "name": "userData",
            "type": "bytes",
            "internalType": "bytes"
          },
          {
            "name": "fromInternalBalance",
            "type": "bool",
            "internalType": "bool"
          }
        ]
      }
    ],
    "outputs": [
      {
        "name": "bptOut",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "amountsIn",
        "type": "uint256[]",
        "internalType": "uint256[]"
      }
    ],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "querySwap",
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
      }
    ],
    "outputs": [
      {
        "name": "",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "vault",
    "inputs": [],
    "outputs": [
      {
        "name": "",
        "type": "address",
        "internalType": "contract IVault"
      }
    ],
    "stateMutability": "view"
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
pub mod BalancerQueries {
    use super::*;
    use alloy_sol_types;
    /**Constructor`.
    ```solidity
    constructor(address _vault);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct constructorCall {
        #[allow(missing_docs)]
        pub _vault: alloy_sol_types::private::Address,
    }
    const _: () = {
        use alloy_sol_types;
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (alloy_sol_types::sol_data::Address,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (alloy_sol_types::private::Address,);
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
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
                    (value._vault,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for constructorCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _vault: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolConstructor for constructorCall {
            type Parameters<'a> = (alloy_sol_types::sol_data::Address,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
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
                        &self._vault,
                    ),
                )
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `queryBatchSwap(uint8,(bytes32,uint256,uint256,uint256,bytes)[],address[],(address,bool,address,bool))` and selector `0xf84d066e`.
    ```solidity
    function queryBatchSwap(IVault.SwapKind kind, IVault.BatchSwapStep[] memory swaps, address[] memory assets, IVault.FundManagement memory funds) external returns (int256[] memory assetDeltas);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct queryBatchSwapCall {
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
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`queryBatchSwap(uint8,(bytes32,uint256,uint256,uint256,bytes)[],address[],(address,bool,address,bool))`](queryBatchSwapCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct queryBatchSwapReturn {
        #[allow(missing_docs)]
        pub assetDeltas:
            alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::I256>,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types;
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                IVault::SwapKind,
                alloy_sol_types::sol_data::Array<IVault::BatchSwapStep>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                IVault::FundManagement,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                <IVault::SwapKind as alloy_sol_types::SolType>::RustType,
                alloy_sol_types::private::Vec<
                    <IVault::BatchSwapStep as alloy_sol_types::SolType>::RustType,
                >,
                alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
                <IVault::FundManagement as alloy_sol_types::SolType>::RustType,
            );
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<queryBatchSwapCall> for UnderlyingRustTuple<'_> {
                fn from(value: queryBatchSwapCall) -> Self {
                    (value.kind, value.swaps, value.assets, value.funds)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for queryBatchSwapCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        kind: tuple.0,
                        swaps: tuple.1,
                        assets: tuple.2,
                        funds: tuple.3,
                    }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> =
                (alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Int<256>>,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::I256>,
            );
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<queryBatchSwapReturn> for UnderlyingRustTuple<'_> {
                fn from(value: queryBatchSwapReturn) -> Self {
                    (value.assetDeltas,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for queryBatchSwapReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        assetDeltas: tuple.0,
                    }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for queryBatchSwapCall {
            type Parameters<'a> = (
                IVault::SwapKind,
                alloy_sol_types::sol_data::Array<IVault::BatchSwapStep>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                IVault::FundManagement,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return =
                alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::I256>;
            type ReturnTuple<'a> =
                (alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Int<256>>,);
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "queryBatchSwap(uint8,(bytes32,uint256,uint256,uint256,bytes)[],address[],(address,bool,address,bool))";
            const SELECTOR: [u8; 4] = [248u8, 77u8, 6u8, 110u8];
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
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                (<alloy_sol_types::sol_data::Array<
                    alloy_sol_types::sol_data::Int<256>,
                > as alloy_sol_types::SolType>::tokenize(ret),)
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence(data).map(
                    |r| {
                        let r: queryBatchSwapReturn = r.into();
                        r.assetDeltas
                    },
                )
            }
            #[inline]
            fn abi_decode_returns_validate(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence_validate(
                    data,
                )
                .map(|r| {
                    let r: queryBatchSwapReturn = r.into();
                    r.assetDeltas
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `queryExit(bytes32,address,address,(address[],uint256[],bytes,bool))` and selector `0xc7b2c52c`.
    ```solidity
    function queryExit(bytes32 poolId, address sender, address recipient, IVault.ExitPoolRequest memory request) external returns (uint256 bptIn, uint256[] memory amountsOut);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct queryExitCall {
        #[allow(missing_docs)]
        pub poolId: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub sender: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub recipient: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub request: <IVault::ExitPoolRequest as alloy_sol_types::SolType>::RustType,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`queryExit(bytes32,address,address,(address[],uint256[],bytes,bool))`](queryExitCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct queryExitReturn {
        #[allow(missing_docs)]
        pub bptIn: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub amountsOut:
            alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U256>,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types;
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                IVault::ExitPoolRequest,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::FixedBytes<32>,
                alloy_sol_types::private::Address,
                alloy_sol_types::private::Address,
                <IVault::ExitPoolRequest as alloy_sol_types::SolType>::RustType,
            );
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<queryExitCall> for UnderlyingRustTuple<'_> {
                fn from(value: queryExitCall) -> Self {
                    (value.poolId, value.sender, value.recipient, value.request)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for queryExitCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        poolId: tuple.0,
                        sender: tuple.1,
                        recipient: tuple.2,
                        request: tuple.3,
                    }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U256>,
            );
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<queryExitReturn> for UnderlyingRustTuple<'_> {
                fn from(value: queryExitReturn) -> Self {
                    (value.bptIn, value.amountsOut)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for queryExitReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        bptIn: tuple.0,
                        amountsOut: tuple.1,
                    }
                }
            }
        }
        impl queryExitReturn {
            fn _tokenize(&self) -> <queryExitCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                (
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.bptIn),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::SolType>::tokenize(&self.amountsOut),
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for queryExitCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                IVault::ExitPoolRequest,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = queryExitReturn;
            type ReturnTuple<'a> = (
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
            );
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str =
                "queryExit(bytes32,address,address,(address[],uint256[],bytes,bool))";
            const SELECTOR: [u8; 4] = [199u8, 178u8, 197u8, 44u8];
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
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.sender,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.recipient,
                    ),
                    <IVault::ExitPoolRequest as alloy_sol_types::SolType>::tokenize(
                        &self.request,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                queryExitReturn::_tokenize(ret)
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence(data)
                    .map(Into::into)
            }
            #[inline]
            fn abi_decode_returns_validate(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence_validate(
                    data,
                )
                .map(Into::into)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `queryJoin(bytes32,address,address,(address[],uint256[],bytes,bool))` and selector `0x9ebbf05d`.
    ```solidity
    function queryJoin(bytes32 poolId, address sender, address recipient, IVault.JoinPoolRequest memory request) external returns (uint256 bptOut, uint256[] memory amountsIn);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct queryJoinCall {
        #[allow(missing_docs)]
        pub poolId: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub sender: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub recipient: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub request: <IVault::JoinPoolRequest as alloy_sol_types::SolType>::RustType,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`queryJoin(bytes32,address,address,(address[],uint256[],bytes,bool))`](queryJoinCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct queryJoinReturn {
        #[allow(missing_docs)]
        pub bptOut: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub amountsIn:
            alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U256>,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types;
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                IVault::JoinPoolRequest,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::FixedBytes<32>,
                alloy_sol_types::private::Address,
                alloy_sol_types::private::Address,
                <IVault::JoinPoolRequest as alloy_sol_types::SolType>::RustType,
            );
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<queryJoinCall> for UnderlyingRustTuple<'_> {
                fn from(value: queryJoinCall) -> Self {
                    (value.poolId, value.sender, value.recipient, value.request)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for queryJoinCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        poolId: tuple.0,
                        sender: tuple.1,
                        recipient: tuple.2,
                        request: tuple.3,
                    }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U256>,
            );
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<queryJoinReturn> for UnderlyingRustTuple<'_> {
                fn from(value: queryJoinReturn) -> Self {
                    (value.bptOut, value.amountsIn)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for queryJoinReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        bptOut: tuple.0,
                        amountsIn: tuple.1,
                    }
                }
            }
        }
        impl queryJoinReturn {
            fn _tokenize(&self) -> <queryJoinCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                (
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.bptOut),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::SolType>::tokenize(&self.amountsIn),
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for queryJoinCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                IVault::JoinPoolRequest,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = queryJoinReturn;
            type ReturnTuple<'a> = (
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
            );
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str =
                "queryJoin(bytes32,address,address,(address[],uint256[],bytes,bool))";
            const SELECTOR: [u8; 4] = [158u8, 187u8, 240u8, 93u8];
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
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.sender,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.recipient,
                    ),
                    <IVault::JoinPoolRequest as alloy_sol_types::SolType>::tokenize(
                        &self.request,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                queryJoinReturn::_tokenize(ret)
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence(data)
                    .map(Into::into)
            }
            #[inline]
            fn abi_decode_returns_validate(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence_validate(
                    data,
                )
                .map(Into::into)
            }
        }
    };
    #[derive()]
    /**Function with signature `querySwap((bytes32,uint8,address,address,uint256,bytes),(address,bool,address,bool))` and selector `0xe969f6b3`.
    ```solidity
    function querySwap(IVault.SingleSwap memory singleSwap, IVault.FundManagement memory funds) external returns (uint256);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct querySwapCall {
        #[allow(missing_docs)]
        pub singleSwap: <IVault::SingleSwap as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub funds: <IVault::FundManagement as alloy_sol_types::SolType>::RustType,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`querySwap((bytes32,uint8,address,address,uint256,bytes),(address,bool,address,bool))`](querySwapCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct querySwapReturn {
        #[allow(missing_docs)]
        pub _0: alloy_sol_types::private::primitives::aliases::U256,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types;
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (IVault::SingleSwap, IVault::FundManagement);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                <IVault::SingleSwap as alloy_sol_types::SolType>::RustType,
                <IVault::FundManagement as alloy_sol_types::SolType>::RustType,
            );
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<querySwapCall> for UnderlyingRustTuple<'_> {
                fn from(value: querySwapCall) -> Self {
                    (value.singleSwap, value.funds)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for querySwapCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        singleSwap: tuple.0,
                        funds: tuple.1,
                    }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (alloy_sol_types::sol_data::Uint<256>,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (alloy_sol_types::private::primitives::aliases::U256,);
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<querySwapReturn> for UnderlyingRustTuple<'_> {
                fn from(value: querySwapReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for querySwapReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for querySwapCall {
            type Parameters<'a> = (IVault::SingleSwap, IVault::FundManagement);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::primitives::aliases::U256;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Uint<256>,);
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "querySwap((bytes32,uint8,address,address,uint256,bytes),(address,bool,address,bool))";
            const SELECTOR: [u8; 4] = [233u8, 105u8, 246u8, 179u8];
            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }
            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <IVault::SingleSwap as alloy_sol_types::SolType>::tokenize(&self.singleSwap),
                    <IVault::FundManagement as alloy_sol_types::SolType>::tokenize(&self.funds),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                (
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        ret,
                    ),
                )
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence(data).map(
                    |r| {
                        let r: querySwapReturn = r.into();
                        r._0
                    },
                )
            }
            #[inline]
            fn abi_decode_returns_validate(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence_validate(
                    data,
                )
                .map(|r| {
                    let r: querySwapReturn = r.into();
                    r._0
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `vault()` and selector `0xfbfa77cf`.
    ```solidity
    function vault() external view returns (address);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct vaultCall;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`vault()`](vaultCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct vaultReturn {
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
        use alloy_sol_types;
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = ();
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = ();
            #[cfg(test)]
            #[allow(dead_code, unreachable_patterns)]
            fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<vaultCall> for UnderlyingRustTuple<'_> {
                fn from(value: vaultCall) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for vaultCall {
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
            fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<vaultReturn> for UnderlyingRustTuple<'_> {
                fn from(value: vaultReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for vaultReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for vaultCall {
            type Parameters<'a> = ();
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::Address;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "vault()";
            const SELECTOR: [u8; 4] = [251u8, 250u8, 119u8, 207u8];
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
                (<alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(ret),)
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence(data).map(
                    |r| {
                        let r: vaultReturn = r.into();
                        r._0
                    },
                )
            }
            #[inline]
            fn abi_decode_returns_validate(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence_validate(
                    data,
                )
                .map(|r| {
                    let r: vaultReturn = r.into();
                    r._0
                })
            }
        }
    };
    ///Container for all the [`BalancerQueries`](self) function calls.
    #[derive(Clone)]
    pub enum BalancerQueriesCalls {
        #[allow(missing_docs)]
        queryBatchSwap(queryBatchSwapCall),
        #[allow(missing_docs)]
        queryExit(queryExitCall),
        #[allow(missing_docs)]
        queryJoin(queryJoinCall),
        #[allow(missing_docs)]
        querySwap(querySwapCall),
        #[allow(missing_docs)]
        vault(vaultCall),
    }
    impl BalancerQueriesCalls {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the variants.
        /// No guarantees are made about the order of the selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 4usize]] = &[
            [158u8, 187u8, 240u8, 93u8],
            [199u8, 178u8, 197u8, 44u8],
            [233u8, 105u8, 246u8, 179u8],
            [248u8, 77u8, 6u8, 110u8],
            [251u8, 250u8, 119u8, 207u8],
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(queryJoin),
            ::core::stringify!(queryExit),
            ::core::stringify!(querySwap),
            ::core::stringify!(queryBatchSwap),
            ::core::stringify!(vault),
        ];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <queryJoinCall as alloy_sol_types::SolCall>::SIGNATURE,
            <queryExitCall as alloy_sol_types::SolCall>::SIGNATURE,
            <querySwapCall as alloy_sol_types::SolCall>::SIGNATURE,
            <queryBatchSwapCall as alloy_sol_types::SolCall>::SIGNATURE,
            <vaultCall as alloy_sol_types::SolCall>::SIGNATURE,
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
        pub fn name_by_selector(selector: [u8; 4usize]) -> ::core::option::Option<&'static str> {
            let sig = Self::signature_by_selector(selector)?;
            sig.split_once('(').map(|(name, _)| name)
        }
    }
    #[automatically_derived]
    impl alloy_sol_types::SolInterface for BalancerQueriesCalls {
        const NAME: &'static str = "BalancerQueriesCalls";
        const MIN_DATA_LENGTH: usize = 0usize;
        const COUNT: usize = 5usize;
        #[inline]
        fn selector(&self) -> [u8; 4] {
            match self {
                Self::queryBatchSwap(_) => {
                    <queryBatchSwapCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::queryExit(_) => <queryExitCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::queryJoin(_) => <queryJoinCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::querySwap(_) => <querySwapCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::vault(_) => <vaultCall as alloy_sol_types::SolCall>::SELECTOR,
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
        fn abi_decode_raw(selector: [u8; 4], data: &[u8]) -> alloy_sol_types::Result<Self> {
            static DECODE_SHIMS: &[fn(&[u8]) -> alloy_sol_types::Result<BalancerQueriesCalls>] = &[
                {
                    fn queryJoin(data: &[u8]) -> alloy_sol_types::Result<BalancerQueriesCalls> {
                        <queryJoinCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(BalancerQueriesCalls::queryJoin)
                    }
                    queryJoin
                },
                {
                    fn queryExit(data: &[u8]) -> alloy_sol_types::Result<BalancerQueriesCalls> {
                        <queryExitCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(BalancerQueriesCalls::queryExit)
                    }
                    queryExit
                },
                {
                    fn querySwap(data: &[u8]) -> alloy_sol_types::Result<BalancerQueriesCalls> {
                        <querySwapCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(BalancerQueriesCalls::querySwap)
                    }
                    querySwap
                },
                {
                    fn queryBatchSwap(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerQueriesCalls> {
                        <queryBatchSwapCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(BalancerQueriesCalls::queryBatchSwap)
                    }
                    queryBatchSwap
                },
                {
                    fn vault(data: &[u8]) -> alloy_sol_types::Result<BalancerQueriesCalls> {
                        <vaultCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(BalancerQueriesCalls::vault)
                    }
                    vault
                },
            ];
            let Ok(idx) = Self::SELECTORS.binary_search(&selector) else {
                return Err(alloy_sol_types::Error::unknown_selector(
                    <Self as alloy_sol_types::SolInterface>::NAME,
                    selector,
                ));
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
            )
                -> alloy_sol_types::Result<BalancerQueriesCalls>] = &[
                {
                    fn queryJoin(data: &[u8]) -> alloy_sol_types::Result<BalancerQueriesCalls> {
                        <queryJoinCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(BalancerQueriesCalls::queryJoin)
                    }
                    queryJoin
                },
                {
                    fn queryExit(data: &[u8]) -> alloy_sol_types::Result<BalancerQueriesCalls> {
                        <queryExitCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(BalancerQueriesCalls::queryExit)
                    }
                    queryExit
                },
                {
                    fn querySwap(data: &[u8]) -> alloy_sol_types::Result<BalancerQueriesCalls> {
                        <querySwapCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(BalancerQueriesCalls::querySwap)
                    }
                    querySwap
                },
                {
                    fn queryBatchSwap(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerQueriesCalls> {
                        <queryBatchSwapCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(BalancerQueriesCalls::queryBatchSwap)
                    }
                    queryBatchSwap
                },
                {
                    fn vault(data: &[u8]) -> alloy_sol_types::Result<BalancerQueriesCalls> {
                        <vaultCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(BalancerQueriesCalls::vault)
                    }
                    vault
                },
            ];
            let Ok(idx) = Self::SELECTORS.binary_search(&selector) else {
                return Err(alloy_sol_types::Error::unknown_selector(
                    <Self as alloy_sol_types::SolInterface>::NAME,
                    selector,
                ));
            };
            DECODE_VALIDATE_SHIMS[idx](data)
        }
        #[inline]
        fn abi_encoded_size(&self) -> usize {
            match self {
                Self::queryBatchSwap(inner) => {
                    <queryBatchSwapCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::queryExit(inner) => {
                    <queryExitCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::queryJoin(inner) => {
                    <queryJoinCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::querySwap(inner) => {
                    <querySwapCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::vault(inner) => {
                    <vaultCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
            }
        }
        #[inline]
        fn abi_encode_raw(&self, out: &mut alloy_sol_types::private::Vec<u8>) {
            match self {
                Self::queryBatchSwap(inner) => {
                    <queryBatchSwapCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::queryExit(inner) => {
                    <queryExitCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::queryJoin(inner) => {
                    <queryJoinCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::querySwap(inner) => {
                    <querySwapCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::vault(inner) => {
                    <vaultCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
            }
        }
    }
    use alloy_contract;
    /**Creates a new wrapper around an on-chain [`BalancerQueries`](self) contract instance.

    See the [wrapper's documentation](`BalancerQueriesInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> BalancerQueriesInstance<P, N> {
        BalancerQueriesInstance::<P, N>::new(address, __provider)
    }
    /**A [`BalancerQueries`](self) instance.

    Contains type-safe methods for interacting with an on-chain instance of the
    [`BalancerQueries`](self) contract located at a given `address`, using a given
    provider `P`.

    If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
    documentation on how to provide it), the `deploy` and `deploy_builder` methods can
    be used to deploy a new instance of the contract.

    See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct BalancerQueriesInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for BalancerQueriesInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("BalancerQueriesInstance")
                .field(&self.address)
                .finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        BalancerQueriesInstance<P, N>
    {
        /**Creates a new wrapper around an on-chain [`BalancerQueries`](self) contract instance.

        See the [wrapper's documentation](`BalancerQueriesInstance`) for more details.*/
        #[inline]
        pub const fn new(address: alloy_sol_types::private::Address, __provider: P) -> Self {
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
    impl<P: ::core::clone::Clone, N> BalancerQueriesInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned provider.
        #[inline]
        pub fn with_cloned_provider(self) -> BalancerQueriesInstance<P, N> {
            BalancerQueriesInstance {
                address: self.address,
                provider: ::core::clone::Clone::clone(&self.provider),
                _network: ::core::marker::PhantomData,
            }
        }
    }
    /// Function calls.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        BalancerQueriesInstance<P, N>
    {
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
        ///Creates a new call builder for the [`queryBatchSwap`] function.
        pub fn queryBatchSwap(
            &self,
            kind: <IVault::SwapKind as alloy_sol_types::SolType>::RustType,
            swaps: alloy_sol_types::private::Vec<
                <IVault::BatchSwapStep as alloy_sol_types::SolType>::RustType,
            >,
            assets: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
            funds: <IVault::FundManagement as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, queryBatchSwapCall, N> {
            self.call_builder(&queryBatchSwapCall {
                kind,
                swaps,
                assets,
                funds,
            })
        }
        ///Creates a new call builder for the [`queryExit`] function.
        pub fn queryExit(
            &self,
            poolId: alloy_sol_types::private::FixedBytes<32>,
            sender: alloy_sol_types::private::Address,
            recipient: alloy_sol_types::private::Address,
            request: <IVault::ExitPoolRequest as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, queryExitCall, N> {
            self.call_builder(&queryExitCall {
                poolId,
                sender,
                recipient,
                request,
            })
        }
        ///Creates a new call builder for the [`queryJoin`] function.
        pub fn queryJoin(
            &self,
            poolId: alloy_sol_types::private::FixedBytes<32>,
            sender: alloy_sol_types::private::Address,
            recipient: alloy_sol_types::private::Address,
            request: <IVault::JoinPoolRequest as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, queryJoinCall, N> {
            self.call_builder(&queryJoinCall {
                poolId,
                sender,
                recipient,
                request,
            })
        }
        ///Creates a new call builder for the [`querySwap`] function.
        pub fn querySwap(
            &self,
            singleSwap: <IVault::SingleSwap as alloy_sol_types::SolType>::RustType,
            funds: <IVault::FundManagement as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, querySwapCall, N> {
            self.call_builder(&querySwapCall { singleSwap, funds })
        }
        ///Creates a new call builder for the [`vault`] function.
        pub fn vault(&self) -> alloy_contract::SolCallBuilder<&P, vaultCall, N> {
            self.call_builder(&vaultCall)
        }
    }
    /// Event filters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        BalancerQueriesInstance<P, N>
    {
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
pub type Instance = BalancerQueries::BalancerQueriesInstance<::alloy_provider::DynProvider>;
use {
    alloy_primitives::{Address, address},
    alloy_provider::{DynProvider, Provider},
    anyhow::{Context, Result},
    std::{collections::HashMap, sync::LazyLock},
};
pub const fn deployment_info(chain_id: u64) -> Option<(Address, Option<u64>)> {
    match chain_id {
        8453u64 => Some((
            ::alloy_primitives::address!("0x300Ab2038EAc391f26D9F895dc61F8F66a548833"),
            Some(1205869u64),
        )),
        1u64 => Some((
            ::alloy_primitives::address!("0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5"),
            Some(15188261u64),
        )),
        43114u64 => Some((
            ::alloy_primitives::address!("0xC128468b7Ce63eA702C1f104D55A2566b13D3ABD"),
            Some(26387068u64),
        )),
        100u64 => Some((
            ::alloy_primitives::address!("0x0F3e0c4218b7b0108a3643cFe9D3ec0d4F57c54e"),
            Some(24821845u64),
        )),
        42161u64 => Some((
            ::alloy_primitives::address!("0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5"),
            Some(18238624u64),
        )),
        10u64 => Some((
            ::alloy_primitives::address!("0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5"),
            Some(15288107u64),
        )),
        137u64 => Some((
            ::alloy_primitives::address!("0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5"),
            Some(30988035u64),
        )),
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
    pub fn deployed(provider: &DynProvider) -> impl Future<Output = Result<Self>> + Send {
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
