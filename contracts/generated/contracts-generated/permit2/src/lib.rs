#![allow(unused_imports, unused_attributes, clippy::all, rustdoc::all, non_snake_case)]
//! Auto-generated contract bindings. Do not edit.
///Module containing a contract's types and functions.
/**

```solidity
library IAllowanceTransfer {
    struct PermitDetails { address token; uint160 amount; uint48 expiration; uint48 nonce; }
    struct PermitSingle { PermitDetails details; address spender; uint256 sigDeadline; }
    struct TokenSpenderPair { address token; address spender; }
}
```*/
#[allow(
    non_camel_case_types,
    non_snake_case,
    clippy::pub_underscore_fields,
    clippy::style,
    clippy::empty_structs_with_brackets
)]
pub mod IAllowanceTransfer {
    use super::*;
    use alloy_sol_types as alloy_sol_types;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**```solidity
struct PermitDetails { address token; uint160 amount; uint48 expiration; uint48 nonce; }
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct PermitDetails {
        #[allow(missing_docs)]
        pub token: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub amount: alloy_sol_types::private::primitives::aliases::U160,
        #[allow(missing_docs)]
        pub expiration: alloy_sol_types::private::primitives::aliases::U48,
        #[allow(missing_docs)]
        pub nonce: alloy_sol_types::private::primitives::aliases::U48,
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
            alloy_sol_types::sol_data::Uint<160>,
            alloy_sol_types::sol_data::Uint<48>,
            alloy_sol_types::sol_data::Uint<48>,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::Address,
            alloy_sol_types::private::primitives::aliases::U160,
            alloy_sol_types::private::primitives::aliases::U48,
            alloy_sol_types::private::primitives::aliases::U48,
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
        impl ::core::convert::From<PermitDetails> for UnderlyingRustTuple<'_> {
            fn from(value: PermitDetails) -> Self {
                (value.token, value.amount, value.expiration, value.nonce)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for PermitDetails {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    token: tuple.0,
                    amount: tuple.1,
                    expiration: tuple.2,
                    nonce: tuple.3,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for PermitDetails {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for PermitDetails {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.token,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        160,
                    > as alloy_sol_types::SolType>::tokenize(&self.amount),
                    <alloy_sol_types::sol_data::Uint<
                        48,
                    > as alloy_sol_types::SolType>::tokenize(&self.expiration),
                    <alloy_sol_types::sol_data::Uint<
                        48,
                    > as alloy_sol_types::SolType>::tokenize(&self.nonce),
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
        impl alloy_sol_types::SolType for PermitDetails {
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
        impl alloy_sol_types::SolStruct for PermitDetails {
            const NAME: &'static str = "PermitDetails";
            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "PermitDetails(address token,uint160 amount,uint48 expiration,uint48 nonce)",
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
                            &self.token,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        160,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.amount)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        48,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.expiration)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        48,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.nonce)
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for PermitDetails {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.token,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        160,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.amount,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        48,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.expiration,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        48,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(&rust.nonce)
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
                    &rust.token,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    160,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.amount,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    48,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.expiration,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    48,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.nonce,
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
struct PermitSingle { PermitDetails details; address spender; uint256 sigDeadline; }
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct PermitSingle {
        #[allow(missing_docs)]
        pub details: <PermitDetails as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub spender: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub sigDeadline: alloy_sol_types::private::primitives::aliases::U256,
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
            PermitDetails,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Uint<256>,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            <PermitDetails as alloy_sol_types::SolType>::RustType,
            alloy_sol_types::private::Address,
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
        impl ::core::convert::From<PermitSingle> for UnderlyingRustTuple<'_> {
            fn from(value: PermitSingle) -> Self {
                (value.details, value.spender, value.sigDeadline)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for PermitSingle {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    details: tuple.0,
                    spender: tuple.1,
                    sigDeadline: tuple.2,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for PermitSingle {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for PermitSingle {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <PermitDetails as alloy_sol_types::SolType>::tokenize(&self.details),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.spender,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.sigDeadline),
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
        impl alloy_sol_types::SolType for PermitSingle {
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
        impl alloy_sol_types::SolStruct for PermitSingle {
            const NAME: &'static str = "PermitSingle";
            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "PermitSingle(PermitDetails details,address spender,uint256 sigDeadline)",
                )
            }
            #[inline]
            fn eip712_components() -> alloy_sol_types::private::Vec<
                alloy_sol_types::private::Cow<'static, str>,
            > {
                let mut components = alloy_sol_types::private::Vec::with_capacity(1);
                components
                    .push(
                        <PermitDetails as alloy_sol_types::SolStruct>::eip712_root_type(),
                    );
                components
                    .extend(
                        <PermitDetails as alloy_sol_types::SolStruct>::eip712_components(),
                    );
                components
            }
            #[inline]
            fn eip712_encode_data(&self) -> alloy_sol_types::private::Vec<u8> {
                [
                    <PermitDetails as alloy_sol_types::SolType>::eip712_data_word(
                            &self.details,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.spender,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.sigDeadline)
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for PermitSingle {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <PermitDetails as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.details,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.spender,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.sigDeadline,
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
                <PermitDetails as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.details,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.spender,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.sigDeadline,
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
struct TokenSpenderPair { address token; address spender; }
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct TokenSpenderPair {
        #[allow(missing_docs)]
        pub token: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub spender: alloy_sol_types::private::Address,
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
        impl ::core::convert::From<TokenSpenderPair> for UnderlyingRustTuple<'_> {
            fn from(value: TokenSpenderPair) -> Self {
                (value.token, value.spender)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for TokenSpenderPair {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    token: tuple.0,
                    spender: tuple.1,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for TokenSpenderPair {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for TokenSpenderPair {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.token,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.spender,
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
        impl alloy_sol_types::SolType for TokenSpenderPair {
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
        impl alloy_sol_types::SolStruct for TokenSpenderPair {
            const NAME: &'static str = "TokenSpenderPair";
            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "TokenSpenderPair(address token,address spender)",
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
                            &self.token,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.spender,
                        )
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for TokenSpenderPair {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.token,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.spender,
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
                    &rust.token,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.spender,
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
    /**Creates a new wrapper around an on-chain [`IAllowanceTransfer`](self) contract instance.

See the [wrapper's documentation](`IAllowanceTransferInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> IAllowanceTransferInstance<P, N> {
        IAllowanceTransferInstance::<P, N>::new(address, __provider)
    }
    /**A [`IAllowanceTransfer`](self) instance.

Contains type-safe methods for interacting with an on-chain instance of the
[`IAllowanceTransfer`](self) contract located at a given `address`, using a given
provider `P`.

If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
documentation on how to provide it), the `deploy` and `deploy_builder` methods can
be used to deploy a new instance of the contract.

See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct IAllowanceTransferInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for IAllowanceTransferInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("IAllowanceTransferInstance").field(&self.address).finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    > IAllowanceTransferInstance<P, N> {
        /**Creates a new wrapper around an on-chain [`IAllowanceTransfer`](self) contract instance.

See the [wrapper's documentation](`IAllowanceTransferInstance`) for more details.*/
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
    impl<P: ::core::clone::Clone, N> IAllowanceTransferInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned provider.
        #[inline]
        pub fn with_cloned_provider(self) -> IAllowanceTransferInstance<P, N> {
            IAllowanceTransferInstance {
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
    > IAllowanceTransferInstance<P, N> {
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
    > IAllowanceTransferInstance<P, N> {
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
///Module containing a contract's types and functions.
/**

```solidity
library ISignatureTransfer {
    struct PermitBatchTransferFrom { TokenPermissions[] permitted; uint256 nonce; uint256 deadline; }
    struct PermitTransferFrom { TokenPermissions permitted; uint256 nonce; uint256 deadline; }
    struct SignatureTransferDetails { address to; uint256 requestedAmount; }
    struct TokenPermissions { address token; uint256 amount; }
}
```*/
#[allow(
    non_camel_case_types,
    non_snake_case,
    clippy::pub_underscore_fields,
    clippy::style,
    clippy::empty_structs_with_brackets
)]
pub mod ISignatureTransfer {
    use super::*;
    use alloy_sol_types as alloy_sol_types;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**```solidity
struct PermitBatchTransferFrom { TokenPermissions[] permitted; uint256 nonce; uint256 deadline; }
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct PermitBatchTransferFrom {
        #[allow(missing_docs)]
        pub permitted: alloy_sol_types::private::Vec<
            <TokenPermissions as alloy_sol_types::SolType>::RustType,
        >,
        #[allow(missing_docs)]
        pub nonce: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub deadline: alloy_sol_types::private::primitives::aliases::U256,
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
            alloy_sol_types::sol_data::Array<TokenPermissions>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<256>,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::Vec<
                <TokenPermissions as alloy_sol_types::SolType>::RustType,
            >,
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
        impl ::core::convert::From<PermitBatchTransferFrom> for UnderlyingRustTuple<'_> {
            fn from(value: PermitBatchTransferFrom) -> Self {
                (value.permitted, value.nonce, value.deadline)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for PermitBatchTransferFrom {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    permitted: tuple.0,
                    nonce: tuple.1,
                    deadline: tuple.2,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for PermitBatchTransferFrom {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for PermitBatchTransferFrom {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Array<
                        TokenPermissions,
                    > as alloy_sol_types::SolType>::tokenize(&self.permitted),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.nonce),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.deadline),
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
        impl alloy_sol_types::SolType for PermitBatchTransferFrom {
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
        impl alloy_sol_types::SolStruct for PermitBatchTransferFrom {
            const NAME: &'static str = "PermitBatchTransferFrom";
            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "PermitBatchTransferFrom(TokenPermissions[] permitted,uint256 nonce,uint256 deadline)",
                )
            }
            #[inline]
            fn eip712_components() -> alloy_sol_types::private::Vec<
                alloy_sol_types::private::Cow<'static, str>,
            > {
                let mut components = alloy_sol_types::private::Vec::with_capacity(1);
                components
                    .push(
                        <TokenPermissions as alloy_sol_types::SolStruct>::eip712_root_type(),
                    );
                components
                    .extend(
                        <TokenPermissions as alloy_sol_types::SolStruct>::eip712_components(),
                    );
                components
            }
            #[inline]
            fn eip712_encode_data(&self) -> alloy_sol_types::private::Vec<u8> {
                [
                    <alloy_sol_types::sol_data::Array<
                        TokenPermissions,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.permitted)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.nonce)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.deadline)
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for PermitBatchTransferFrom {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Array<
                        TokenPermissions,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.permitted,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(&rust.nonce)
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.deadline,
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
                <alloy_sol_types::sol_data::Array<
                    TokenPermissions,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.permitted,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.nonce,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.deadline,
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
struct PermitTransferFrom { TokenPermissions permitted; uint256 nonce; uint256 deadline; }
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct PermitTransferFrom {
        #[allow(missing_docs)]
        pub permitted: <TokenPermissions as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub nonce: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub deadline: alloy_sol_types::private::primitives::aliases::U256,
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
            TokenPermissions,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<256>,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            <TokenPermissions as alloy_sol_types::SolType>::RustType,
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
        impl ::core::convert::From<PermitTransferFrom> for UnderlyingRustTuple<'_> {
            fn from(value: PermitTransferFrom) -> Self {
                (value.permitted, value.nonce, value.deadline)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for PermitTransferFrom {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    permitted: tuple.0,
                    nonce: tuple.1,
                    deadline: tuple.2,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for PermitTransferFrom {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for PermitTransferFrom {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <TokenPermissions as alloy_sol_types::SolType>::tokenize(
                        &self.permitted,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.nonce),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.deadline),
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
        impl alloy_sol_types::SolType for PermitTransferFrom {
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
        impl alloy_sol_types::SolStruct for PermitTransferFrom {
            const NAME: &'static str = "PermitTransferFrom";
            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "PermitTransferFrom(TokenPermissions permitted,uint256 nonce,uint256 deadline)",
                )
            }
            #[inline]
            fn eip712_components() -> alloy_sol_types::private::Vec<
                alloy_sol_types::private::Cow<'static, str>,
            > {
                let mut components = alloy_sol_types::private::Vec::with_capacity(1);
                components
                    .push(
                        <TokenPermissions as alloy_sol_types::SolStruct>::eip712_root_type(),
                    );
                components
                    .extend(
                        <TokenPermissions as alloy_sol_types::SolStruct>::eip712_components(),
                    );
                components
            }
            #[inline]
            fn eip712_encode_data(&self) -> alloy_sol_types::private::Vec<u8> {
                [
                    <TokenPermissions as alloy_sol_types::SolType>::eip712_data_word(
                            &self.permitted,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.nonce)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.deadline)
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for PermitTransferFrom {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <TokenPermissions as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.permitted,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(&rust.nonce)
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.deadline,
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
                <TokenPermissions as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.permitted,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.nonce,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.deadline,
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
struct SignatureTransferDetails { address to; uint256 requestedAmount; }
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct SignatureTransferDetails {
        #[allow(missing_docs)]
        pub to: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub requestedAmount: alloy_sol_types::private::primitives::aliases::U256,
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
            alloy_sol_types::sol_data::Uint<256>,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::Address,
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
        impl ::core::convert::From<SignatureTransferDetails>
        for UnderlyingRustTuple<'_> {
            fn from(value: SignatureTransferDetails) -> Self {
                (value.to, value.requestedAmount)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>>
        for SignatureTransferDetails {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    to: tuple.0,
                    requestedAmount: tuple.1,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for SignatureTransferDetails {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for SignatureTransferDetails {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.to,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.requestedAmount),
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
        impl alloy_sol_types::SolType for SignatureTransferDetails {
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
        impl alloy_sol_types::SolStruct for SignatureTransferDetails {
            const NAME: &'static str = "SignatureTransferDetails";
            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "SignatureTransferDetails(address to,uint256 requestedAmount)",
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
                            &self.to,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(
                            &self.requestedAmount,
                        )
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for SignatureTransferDetails {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.to,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.requestedAmount,
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
                    &rust.to,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.requestedAmount,
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
struct TokenPermissions { address token; uint256 amount; }
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct TokenPermissions {
        #[allow(missing_docs)]
        pub token: alloy_sol_types::private::Address,
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
        #[doc(hidden)]
        #[allow(dead_code)]
        type UnderlyingSolTuple<'a> = (
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Uint<256>,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::Address,
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
        impl ::core::convert::From<TokenPermissions> for UnderlyingRustTuple<'_> {
            fn from(value: TokenPermissions) -> Self {
                (value.token, value.amount)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for TokenPermissions {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    token: tuple.0,
                    amount: tuple.1,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for TokenPermissions {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for TokenPermissions {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.token,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.amount),
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
        impl alloy_sol_types::SolType for TokenPermissions {
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
        impl alloy_sol_types::SolStruct for TokenPermissions {
            const NAME: &'static str = "TokenPermissions";
            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "TokenPermissions(address token,uint256 amount)",
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
                            &self.token,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.amount)
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for TokenPermissions {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.token,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.amount,
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
                    &rust.token,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.amount,
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
    /**Creates a new wrapper around an on-chain [`ISignatureTransfer`](self) contract instance.

See the [wrapper's documentation](`ISignatureTransferInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> ISignatureTransferInstance<P, N> {
        ISignatureTransferInstance::<P, N>::new(address, __provider)
    }
    /**A [`ISignatureTransfer`](self) instance.

Contains type-safe methods for interacting with an on-chain instance of the
[`ISignatureTransfer`](self) contract located at a given `address`, using a given
provider `P`.

If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
documentation on how to provide it), the `deploy` and `deploy_builder` methods can
be used to deploy a new instance of the contract.

See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct ISignatureTransferInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for ISignatureTransferInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("ISignatureTransferInstance").field(&self.address).finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    > ISignatureTransferInstance<P, N> {
        /**Creates a new wrapper around an on-chain [`ISignatureTransfer`](self) contract instance.

See the [wrapper's documentation](`ISignatureTransferInstance`) for more details.*/
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
    impl<P: ::core::clone::Clone, N> ISignatureTransferInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned provider.
        #[inline]
        pub fn with_cloned_provider(self) -> ISignatureTransferInstance<P, N> {
            ISignatureTransferInstance {
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
    > ISignatureTransferInstance<P, N> {
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
    > ISignatureTransferInstance<P, N> {
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
library IAllowanceTransfer {
    struct PermitDetails {
        address token;
        uint160 amount;
        uint48 expiration;
        uint48 nonce;
    }
    struct PermitSingle {
        PermitDetails details;
        address spender;
        uint256 sigDeadline;
    }
    struct TokenSpenderPair {
        address token;
        address spender;
    }
}

library ISignatureTransfer {
    struct PermitBatchTransferFrom {
        TokenPermissions[] permitted;
        uint256 nonce;
        uint256 deadline;
    }
    struct PermitTransferFrom {
        TokenPermissions permitted;
        uint256 nonce;
        uint256 deadline;
    }
    struct SignatureTransferDetails {
        address to;
        uint256 requestedAmount;
    }
    struct TokenPermissions {
        address token;
        uint256 amount;
    }
}

interface Permit2 {
    error AllowanceExpired(uint256 deadline);
    error ExcessiveInvalidation();
    error InsufficientAllowance(uint256 amount);
    error InvalidAmount(uint256 maxAmount);
    error InvalidContractSignature();
    error InvalidNonce();
    error InvalidSignature();
    error InvalidSignatureLength();
    error InvalidSigner();
    error LengthMismatch();
    error SignatureExpired(uint256 signatureDeadline);

    event Approval(address indexed owner, address indexed token, address indexed spender, uint160 amount, uint48 expiration);
    event Lockdown(address indexed owner, address token, address spender);
    event NonceInvalidation(address indexed owner, address indexed token, address indexed spender, uint48 newNonce, uint48 oldNonce);
    event Permit(address indexed owner, address indexed token, address indexed spender, uint160 amount, uint48 expiration, uint48 nonce);
    event UnorderedNonceInvalidation(address indexed owner, uint256 word, uint256 mask);

    function DOMAIN_SEPARATOR() external view returns (bytes32);
    function allowance(address, address, address) external view returns (uint160 amount, uint48 expiration, uint48 nonce);
    function approve(address token, address spender, uint160 amount, uint48 expiration) external;
    function invalidateNonces(address token, address spender, uint48 newNonce) external;
    function invalidateUnorderedNonces(uint256 wordPos, uint256 mask) external;
    function lockdown(IAllowanceTransfer.TokenSpenderPair[] memory approvals) external;
    function nonceBitmap(address, uint256) external view returns (uint256);
    function permit(address owner, IAllowanceTransfer.PermitSingle memory permitSingle, bytes memory signature) external;
    function permitTransferFrom(ISignatureTransfer.PermitTransferFrom memory permit, ISignatureTransfer.SignatureTransferDetails memory transferDetails, address owner, bytes memory signature) external;
    function permitWitnessTransferFrom(ISignatureTransfer.PermitBatchTransferFrom memory permit, ISignatureTransfer.SignatureTransferDetails[] memory transferDetails, address owner, bytes32 witness, string memory witnessTypeString, bytes memory signature) external;
    function transferFrom(address from, address to, uint160 amount, address token) external;
}
```

...which was generated by the following JSON ABI:
```json
[
  {
    "type": "function",
    "name": "DOMAIN_SEPARATOR",
    "inputs": [],
    "outputs": [
      {
        "name": "",
        "type": "bytes32",
        "internalType": "bytes32"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "allowance",
    "inputs": [
      {
        "name": "",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "",
        "type": "address",
        "internalType": "address"
      }
    ],
    "outputs": [
      {
        "name": "amount",
        "type": "uint160",
        "internalType": "uint160"
      },
      {
        "name": "expiration",
        "type": "uint48",
        "internalType": "uint48"
      },
      {
        "name": "nonce",
        "type": "uint48",
        "internalType": "uint48"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "approve",
    "inputs": [
      {
        "name": "token",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "spender",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "amount",
        "type": "uint160",
        "internalType": "uint160"
      },
      {
        "name": "expiration",
        "type": "uint48",
        "internalType": "uint48"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "invalidateNonces",
    "inputs": [
      {
        "name": "token",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "spender",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "newNonce",
        "type": "uint48",
        "internalType": "uint48"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "invalidateUnorderedNonces",
    "inputs": [
      {
        "name": "wordPos",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "mask",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "lockdown",
    "inputs": [
      {
        "name": "approvals",
        "type": "tuple[]",
        "internalType": "struct IAllowanceTransfer.TokenSpenderPair[]",
        "components": [
          {
            "name": "token",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "spender",
            "type": "address",
            "internalType": "address"
          }
        ]
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "nonceBitmap",
    "inputs": [
      {
        "name": "",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "outputs": [
      {
        "name": "",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "permit",
    "inputs": [
      {
        "name": "owner",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "permitSingle",
        "type": "tuple",
        "internalType": "struct IAllowanceTransfer.PermitSingle",
        "components": [
          {
            "name": "details",
            "type": "tuple",
            "internalType": "struct IAllowanceTransfer.PermitDetails",
            "components": [
              {
                "name": "token",
                "type": "address",
                "internalType": "address"
              },
              {
                "name": "amount",
                "type": "uint160",
                "internalType": "uint160"
              },
              {
                "name": "expiration",
                "type": "uint48",
                "internalType": "uint48"
              },
              {
                "name": "nonce",
                "type": "uint48",
                "internalType": "uint48"
              }
            ]
          },
          {
            "name": "spender",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "sigDeadline",
            "type": "uint256",
            "internalType": "uint256"
          }
        ]
      },
      {
        "name": "signature",
        "type": "bytes",
        "internalType": "bytes"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "permitTransferFrom",
    "inputs": [
      {
        "name": "permit",
        "type": "tuple",
        "internalType": "struct ISignatureTransfer.PermitTransferFrom",
        "components": [
          {
            "name": "permitted",
            "type": "tuple",
            "internalType": "struct ISignatureTransfer.TokenPermissions",
            "components": [
              {
                "name": "token",
                "type": "address",
                "internalType": "address"
              },
              {
                "name": "amount",
                "type": "uint256",
                "internalType": "uint256"
              }
            ]
          },
          {
            "name": "nonce",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "deadline",
            "type": "uint256",
            "internalType": "uint256"
          }
        ]
      },
      {
        "name": "transferDetails",
        "type": "tuple",
        "internalType": "struct ISignatureTransfer.SignatureTransferDetails",
        "components": [
          {
            "name": "to",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "requestedAmount",
            "type": "uint256",
            "internalType": "uint256"
          }
        ]
      },
      {
        "name": "owner",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "signature",
        "type": "bytes",
        "internalType": "bytes"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "permitWitnessTransferFrom",
    "inputs": [
      {
        "name": "permit",
        "type": "tuple",
        "internalType": "struct ISignatureTransfer.PermitBatchTransferFrom",
        "components": [
          {
            "name": "permitted",
            "type": "tuple[]",
            "internalType": "struct ISignatureTransfer.TokenPermissions[]",
            "components": [
              {
                "name": "token",
                "type": "address",
                "internalType": "address"
              },
              {
                "name": "amount",
                "type": "uint256",
                "internalType": "uint256"
              }
            ]
          },
          {
            "name": "nonce",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "deadline",
            "type": "uint256",
            "internalType": "uint256"
          }
        ]
      },
      {
        "name": "transferDetails",
        "type": "tuple[]",
        "internalType": "struct ISignatureTransfer.SignatureTransferDetails[]",
        "components": [
          {
            "name": "to",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "requestedAmount",
            "type": "uint256",
            "internalType": "uint256"
          }
        ]
      },
      {
        "name": "owner",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "witness",
        "type": "bytes32",
        "internalType": "bytes32"
      },
      {
        "name": "witnessTypeString",
        "type": "string",
        "internalType": "string"
      },
      {
        "name": "signature",
        "type": "bytes",
        "internalType": "bytes"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "transferFrom",
    "inputs": [
      {
        "name": "from",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "to",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "amount",
        "type": "uint160",
        "internalType": "uint160"
      },
      {
        "name": "token",
        "type": "address",
        "internalType": "address"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "event",
    "name": "Approval",
    "inputs": [
      {
        "name": "owner",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "token",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "spender",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "amount",
        "type": "uint160",
        "indexed": false,
        "internalType": "uint160"
      },
      {
        "name": "expiration",
        "type": "uint48",
        "indexed": false,
        "internalType": "uint48"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "Lockdown",
    "inputs": [
      {
        "name": "owner",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "token",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "spender",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "NonceInvalidation",
    "inputs": [
      {
        "name": "owner",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "token",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "spender",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "newNonce",
        "type": "uint48",
        "indexed": false,
        "internalType": "uint48"
      },
      {
        "name": "oldNonce",
        "type": "uint48",
        "indexed": false,
        "internalType": "uint48"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "Permit",
    "inputs": [
      {
        "name": "owner",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "token",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "spender",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "amount",
        "type": "uint160",
        "indexed": false,
        "internalType": "uint160"
      },
      {
        "name": "expiration",
        "type": "uint48",
        "indexed": false,
        "internalType": "uint48"
      },
      {
        "name": "nonce",
        "type": "uint48",
        "indexed": false,
        "internalType": "uint48"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "UnorderedNonceInvalidation",
    "inputs": [
      {
        "name": "owner",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "word",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "mask",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      }
    ],
    "anonymous": false
  },
  {
    "type": "error",
    "name": "AllowanceExpired",
    "inputs": [
      {
        "name": "deadline",
        "type": "uint256",
        "internalType": "uint256"
      }
    ]
  },
  {
    "type": "error",
    "name": "ExcessiveInvalidation",
    "inputs": []
  },
  {
    "type": "error",
    "name": "InsufficientAllowance",
    "inputs": [
      {
        "name": "amount",
        "type": "uint256",
        "internalType": "uint256"
      }
    ]
  },
  {
    "type": "error",
    "name": "InvalidAmount",
    "inputs": [
      {
        "name": "maxAmount",
        "type": "uint256",
        "internalType": "uint256"
      }
    ]
  },
  {
    "type": "error",
    "name": "InvalidContractSignature",
    "inputs": []
  },
  {
    "type": "error",
    "name": "InvalidNonce",
    "inputs": []
  },
  {
    "type": "error",
    "name": "InvalidSignature",
    "inputs": []
  },
  {
    "type": "error",
    "name": "InvalidSignatureLength",
    "inputs": []
  },
  {
    "type": "error",
    "name": "InvalidSigner",
    "inputs": []
  },
  {
    "type": "error",
    "name": "LengthMismatch",
    "inputs": []
  },
  {
    "type": "error",
    "name": "SignatureExpired",
    "inputs": [
      {
        "name": "signatureDeadline",
        "type": "uint256",
        "internalType": "uint256"
      }
    ]
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
pub mod Permit2 {
    use super::*;
    use alloy_sol_types as alloy_sol_types;
    /// The creation / init bytecode of the contract.
    ///
    /// ```text
    ///0x
    /// ```
    #[rustfmt::skip]
    #[allow(clippy::all)]
    pub static BYTECODE: alloy_sol_types::private::Bytes = alloy_sol_types::private::Bytes::from_static(
        b"",
    );
    /// The runtime bytecode of the contract, as deployed on the network.
    ///
    /// ```text
    ///0x
    /// ```
    #[rustfmt::skip]
    #[allow(clippy::all)]
    pub static DEPLOYED_BYTECODE: alloy_sol_types::private::Bytes = alloy_sol_types::private::Bytes::from_static(
        b"",
    );
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Custom error with signature `AllowanceExpired(uint256)` and selector `0xd81b2f2e`.
```solidity
error AllowanceExpired(uint256 deadline);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct AllowanceExpired {
        #[allow(missing_docs)]
        pub deadline: alloy_sol_types::private::primitives::aliases::U256,
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
        impl ::core::convert::From<AllowanceExpired> for UnderlyingRustTuple<'_> {
            fn from(value: AllowanceExpired) -> Self {
                (value.deadline,)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for AllowanceExpired {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self { deadline: tuple.0 }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for AllowanceExpired {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "AllowanceExpired(uint256)";
            const SELECTOR: [u8; 4] = [216u8, 27u8, 47u8, 46u8];
            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }
            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.deadline),
                )
            }
            #[inline]
            fn abi_decode_raw_validate(data: &[u8]) -> alloy_sol_types::Result<Self> {
                <Self::Parameters<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence_validate(data)
                    .map(Self::new)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Custom error with signature `ExcessiveInvalidation()` and selector `0x24d35a26`.
```solidity
error ExcessiveInvalidation();
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct ExcessiveInvalidation;
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
        impl ::core::convert::From<ExcessiveInvalidation> for UnderlyingRustTuple<'_> {
            fn from(value: ExcessiveInvalidation) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for ExcessiveInvalidation {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for ExcessiveInvalidation {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "ExcessiveInvalidation()";
            const SELECTOR: [u8; 4] = [36u8, 211u8, 90u8, 38u8];
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
            fn abi_decode_raw_validate(data: &[u8]) -> alloy_sol_types::Result<Self> {
                <Self::Parameters<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence_validate(data)
                    .map(Self::new)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Custom error with signature `InsufficientAllowance(uint256)` and selector `0xf96fb071`.
```solidity
error InsufficientAllowance(uint256 amount);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct InsufficientAllowance {
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
        impl ::core::convert::From<InsufficientAllowance> for UnderlyingRustTuple<'_> {
            fn from(value: InsufficientAllowance) -> Self {
                (value.amount,)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for InsufficientAllowance {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self { amount: tuple.0 }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for InsufficientAllowance {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "InsufficientAllowance(uint256)";
            const SELECTOR: [u8; 4] = [249u8, 111u8, 176u8, 113u8];
            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }
            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.amount),
                )
            }
            #[inline]
            fn abi_decode_raw_validate(data: &[u8]) -> alloy_sol_types::Result<Self> {
                <Self::Parameters<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence_validate(data)
                    .map(Self::new)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Custom error with signature `InvalidAmount(uint256)` and selector `0x3728b83d`.
```solidity
error InvalidAmount(uint256 maxAmount);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct InvalidAmount {
        #[allow(missing_docs)]
        pub maxAmount: alloy_sol_types::private::primitives::aliases::U256,
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
        impl ::core::convert::From<InvalidAmount> for UnderlyingRustTuple<'_> {
            fn from(value: InvalidAmount) -> Self {
                (value.maxAmount,)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for InvalidAmount {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self { maxAmount: tuple.0 }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for InvalidAmount {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "InvalidAmount(uint256)";
            const SELECTOR: [u8; 4] = [55u8, 40u8, 184u8, 61u8];
            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }
            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.maxAmount),
                )
            }
            #[inline]
            fn abi_decode_raw_validate(data: &[u8]) -> alloy_sol_types::Result<Self> {
                <Self::Parameters<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence_validate(data)
                    .map(Self::new)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Custom error with signature `InvalidContractSignature()` and selector `0xb0669cbc`.
```solidity
error InvalidContractSignature();
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct InvalidContractSignature;
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
        impl ::core::convert::From<InvalidContractSignature>
        for UnderlyingRustTuple<'_> {
            fn from(value: InvalidContractSignature) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>>
        for InvalidContractSignature {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for InvalidContractSignature {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "InvalidContractSignature()";
            const SELECTOR: [u8; 4] = [176u8, 102u8, 156u8, 188u8];
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
            fn abi_decode_raw_validate(data: &[u8]) -> alloy_sol_types::Result<Self> {
                <Self::Parameters<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence_validate(data)
                    .map(Self::new)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Custom error with signature `InvalidNonce()` and selector `0x756688fe`.
```solidity
error InvalidNonce();
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct InvalidNonce;
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
        impl ::core::convert::From<InvalidNonce> for UnderlyingRustTuple<'_> {
            fn from(value: InvalidNonce) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for InvalidNonce {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for InvalidNonce {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "InvalidNonce()";
            const SELECTOR: [u8; 4] = [117u8, 102u8, 136u8, 254u8];
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
            fn abi_decode_raw_validate(data: &[u8]) -> alloy_sol_types::Result<Self> {
                <Self::Parameters<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence_validate(data)
                    .map(Self::new)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Custom error with signature `InvalidSignature()` and selector `0x8baa579f`.
```solidity
error InvalidSignature();
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct InvalidSignature;
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
        impl ::core::convert::From<InvalidSignature> for UnderlyingRustTuple<'_> {
            fn from(value: InvalidSignature) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for InvalidSignature {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for InvalidSignature {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "InvalidSignature()";
            const SELECTOR: [u8; 4] = [139u8, 170u8, 87u8, 159u8];
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
            fn abi_decode_raw_validate(data: &[u8]) -> alloy_sol_types::Result<Self> {
                <Self::Parameters<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence_validate(data)
                    .map(Self::new)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Custom error with signature `InvalidSignatureLength()` and selector `0x4be6321b`.
```solidity
error InvalidSignatureLength();
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct InvalidSignatureLength;
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
        impl ::core::convert::From<InvalidSignatureLength> for UnderlyingRustTuple<'_> {
            fn from(value: InvalidSignatureLength) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for InvalidSignatureLength {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for InvalidSignatureLength {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "InvalidSignatureLength()";
            const SELECTOR: [u8; 4] = [75u8, 230u8, 50u8, 27u8];
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
            fn abi_decode_raw_validate(data: &[u8]) -> alloy_sol_types::Result<Self> {
                <Self::Parameters<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence_validate(data)
                    .map(Self::new)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Custom error with signature `InvalidSigner()` and selector `0x815e1d64`.
```solidity
error InvalidSigner();
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct InvalidSigner;
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
        impl ::core::convert::From<InvalidSigner> for UnderlyingRustTuple<'_> {
            fn from(value: InvalidSigner) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for InvalidSigner {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for InvalidSigner {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "InvalidSigner()";
            const SELECTOR: [u8; 4] = [129u8, 94u8, 29u8, 100u8];
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
            fn abi_decode_raw_validate(data: &[u8]) -> alloy_sol_types::Result<Self> {
                <Self::Parameters<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence_validate(data)
                    .map(Self::new)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Custom error with signature `LengthMismatch()` and selector `0xff633a38`.
```solidity
error LengthMismatch();
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct LengthMismatch;
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
        impl ::core::convert::From<LengthMismatch> for UnderlyingRustTuple<'_> {
            fn from(value: LengthMismatch) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for LengthMismatch {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for LengthMismatch {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "LengthMismatch()";
            const SELECTOR: [u8; 4] = [255u8, 99u8, 58u8, 56u8];
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
            fn abi_decode_raw_validate(data: &[u8]) -> alloy_sol_types::Result<Self> {
                <Self::Parameters<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence_validate(data)
                    .map(Self::new)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Custom error with signature `SignatureExpired(uint256)` and selector `0xcd21db4f`.
```solidity
error SignatureExpired(uint256 signatureDeadline);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct SignatureExpired {
        #[allow(missing_docs)]
        pub signatureDeadline: alloy_sol_types::private::primitives::aliases::U256,
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
        impl ::core::convert::From<SignatureExpired> for UnderlyingRustTuple<'_> {
            fn from(value: SignatureExpired) -> Self {
                (value.signatureDeadline,)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for SignatureExpired {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self { signatureDeadline: tuple.0 }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for SignatureExpired {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "SignatureExpired(uint256)";
            const SELECTOR: [u8; 4] = [205u8, 33u8, 219u8, 79u8];
            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }
            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.signatureDeadline),
                )
            }
            #[inline]
            fn abi_decode_raw_validate(data: &[u8]) -> alloy_sol_types::Result<Self> {
                <Self::Parameters<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence_validate(data)
                    .map(Self::new)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `Approval(address,address,address,uint160,uint48)` and selector `0xda9fa7c1b00402c17d0161b249b1ab8bbec047c5a52207b9c112deffd817036b`.
```solidity
event Approval(address indexed owner, address indexed token, address indexed spender, uint160 amount, uint48 expiration);
```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct Approval {
        #[allow(missing_docs)]
        pub owner: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub token: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub spender: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub amount: alloy_sol_types::private::primitives::aliases::U160,
        #[allow(missing_docs)]
        pub expiration: alloy_sol_types::private::primitives::aliases::U48,
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
        impl alloy_sol_types::SolEvent for Approval {
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::Uint<160>,
                alloy_sol_types::sol_data::Uint<48>,
            );
            type DataToken<'a> = <Self::DataTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "Approval(address,address,address,uint160,uint48)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 = alloy_sol_types::private::B256::new([
                218u8, 159u8, 167u8, 193u8, 176u8, 4u8, 2u8, 193u8, 125u8, 1u8, 97u8,
                178u8, 73u8, 177u8, 171u8, 139u8, 190u8, 192u8, 71u8, 197u8, 165u8, 34u8,
                7u8, 185u8, 193u8, 18u8, 222u8, 255u8, 216u8, 23u8, 3u8, 107u8,
            ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    owner: topics.1,
                    token: topics.2,
                    spender: topics.3,
                    amount: data.0,
                    expiration: data.1,
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
                        160,
                    > as alloy_sol_types::SolType>::tokenize(&self.amount),
                    <alloy_sol_types::sol_data::Uint<
                        48,
                    > as alloy_sol_types::SolType>::tokenize(&self.expiration),
                )
            }
            #[inline]
            fn topics(&self) -> <Self::TopicList as alloy_sol_types::SolType>::RustType {
                (
                    Self::SIGNATURE_HASH.into(),
                    self.owner.clone(),
                    self.token.clone(),
                    self.spender.clone(),
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
                out[1usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.owner,
                );
                out[2usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.token,
                );
                out[3usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.spender,
                );
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for Approval {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&Approval> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &Approval) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `Lockdown(address,address,address)` and selector `0x89b1add15eff56b3dfe299ad94e01f2b52fbcb80ae1a3baea6ae8c04cb2b98a4`.
```solidity
event Lockdown(address indexed owner, address token, address spender);
```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct Lockdown {
        #[allow(missing_docs)]
        pub owner: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub token: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub spender: alloy_sol_types::private::Address,
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
        impl alloy_sol_types::SolEvent for Lockdown {
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
            );
            type DataToken<'a> = <Self::DataTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "Lockdown(address,address,address)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 = alloy_sol_types::private::B256::new([
                137u8, 177u8, 173u8, 209u8, 94u8, 255u8, 86u8, 179u8, 223u8, 226u8,
                153u8, 173u8, 148u8, 224u8, 31u8, 43u8, 82u8, 251u8, 203u8, 128u8, 174u8,
                26u8, 59u8, 174u8, 166u8, 174u8, 140u8, 4u8, 203u8, 43u8, 152u8, 164u8,
            ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    owner: topics.1,
                    token: data.0,
                    spender: data.1,
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
                        &self.token,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.spender,
                    ),
                )
            }
            #[inline]
            fn topics(&self) -> <Self::TopicList as alloy_sol_types::SolType>::RustType {
                (Self::SIGNATURE_HASH.into(), self.owner.clone())
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
                    &self.owner,
                );
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for Lockdown {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&Lockdown> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &Lockdown) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `NonceInvalidation(address,address,address,uint48,uint48)` and selector `0x55eb90d810e1700b35a8e7e25395ff7f2b2259abd7415ca2284dfb1c246418f3`.
```solidity
event NonceInvalidation(address indexed owner, address indexed token, address indexed spender, uint48 newNonce, uint48 oldNonce);
```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct NonceInvalidation {
        #[allow(missing_docs)]
        pub owner: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub token: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub spender: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub newNonce: alloy_sol_types::private::primitives::aliases::U48,
        #[allow(missing_docs)]
        pub oldNonce: alloy_sol_types::private::primitives::aliases::U48,
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
        impl alloy_sol_types::SolEvent for NonceInvalidation {
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::Uint<48>,
                alloy_sol_types::sol_data::Uint<48>,
            );
            type DataToken<'a> = <Self::DataTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "NonceInvalidation(address,address,address,uint48,uint48)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 = alloy_sol_types::private::B256::new([
                85u8, 235u8, 144u8, 216u8, 16u8, 225u8, 112u8, 11u8, 53u8, 168u8, 231u8,
                226u8, 83u8, 149u8, 255u8, 127u8, 43u8, 34u8, 89u8, 171u8, 215u8, 65u8,
                92u8, 162u8, 40u8, 77u8, 251u8, 28u8, 36u8, 100u8, 24u8, 243u8,
            ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    owner: topics.1,
                    token: topics.2,
                    spender: topics.3,
                    newNonce: data.0,
                    oldNonce: data.1,
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
                        48,
                    > as alloy_sol_types::SolType>::tokenize(&self.newNonce),
                    <alloy_sol_types::sol_data::Uint<
                        48,
                    > as alloy_sol_types::SolType>::tokenize(&self.oldNonce),
                )
            }
            #[inline]
            fn topics(&self) -> <Self::TopicList as alloy_sol_types::SolType>::RustType {
                (
                    Self::SIGNATURE_HASH.into(),
                    self.owner.clone(),
                    self.token.clone(),
                    self.spender.clone(),
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
                out[1usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.owner,
                );
                out[2usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.token,
                );
                out[3usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.spender,
                );
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for NonceInvalidation {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&NonceInvalidation> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &NonceInvalidation) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `Permit(address,address,address,uint160,uint48,uint48)` and selector `0xc6a377bfc4eb120024a8ac08eef205be16b817020812c73223e81d1bdb9708ec`.
```solidity
event Permit(address indexed owner, address indexed token, address indexed spender, uint160 amount, uint48 expiration, uint48 nonce);
```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct Permit {
        #[allow(missing_docs)]
        pub owner: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub token: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub spender: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub amount: alloy_sol_types::private::primitives::aliases::U160,
        #[allow(missing_docs)]
        pub expiration: alloy_sol_types::private::primitives::aliases::U48,
        #[allow(missing_docs)]
        pub nonce: alloy_sol_types::private::primitives::aliases::U48,
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
        impl alloy_sol_types::SolEvent for Permit {
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::Uint<160>,
                alloy_sol_types::sol_data::Uint<48>,
                alloy_sol_types::sol_data::Uint<48>,
            );
            type DataToken<'a> = <Self::DataTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "Permit(address,address,address,uint160,uint48,uint48)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 = alloy_sol_types::private::B256::new([
                198u8, 163u8, 119u8, 191u8, 196u8, 235u8, 18u8, 0u8, 36u8, 168u8, 172u8,
                8u8, 238u8, 242u8, 5u8, 190u8, 22u8, 184u8, 23u8, 2u8, 8u8, 18u8, 199u8,
                50u8, 35u8, 232u8, 29u8, 27u8, 219u8, 151u8, 8u8, 236u8,
            ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    owner: topics.1,
                    token: topics.2,
                    spender: topics.3,
                    amount: data.0,
                    expiration: data.1,
                    nonce: data.2,
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
                        160,
                    > as alloy_sol_types::SolType>::tokenize(&self.amount),
                    <alloy_sol_types::sol_data::Uint<
                        48,
                    > as alloy_sol_types::SolType>::tokenize(&self.expiration),
                    <alloy_sol_types::sol_data::Uint<
                        48,
                    > as alloy_sol_types::SolType>::tokenize(&self.nonce),
                )
            }
            #[inline]
            fn topics(&self) -> <Self::TopicList as alloy_sol_types::SolType>::RustType {
                (
                    Self::SIGNATURE_HASH.into(),
                    self.owner.clone(),
                    self.token.clone(),
                    self.spender.clone(),
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
                out[1usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.owner,
                );
                out[2usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.token,
                );
                out[3usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.spender,
                );
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for Permit {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&Permit> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &Permit) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `UnorderedNonceInvalidation(address,uint256,uint256)` and selector `0x3704902f963766a4e561bbaab6e6cdc1b1dd12f6e9e99648da8843b3f46b918d`.
```solidity
event UnorderedNonceInvalidation(address indexed owner, uint256 word, uint256 mask);
```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct UnorderedNonceInvalidation {
        #[allow(missing_docs)]
        pub owner: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub word: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub mask: alloy_sol_types::private::primitives::aliases::U256,
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
        impl alloy_sol_types::SolEvent for UnorderedNonceInvalidation {
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
            );
            const SIGNATURE: &'static str = "UnorderedNonceInvalidation(address,uint256,uint256)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 = alloy_sol_types::private::B256::new([
                55u8, 4u8, 144u8, 47u8, 150u8, 55u8, 102u8, 164u8, 229u8, 97u8, 187u8,
                170u8, 182u8, 230u8, 205u8, 193u8, 177u8, 221u8, 18u8, 246u8, 233u8,
                233u8, 150u8, 72u8, 218u8, 136u8, 67u8, 179u8, 244u8, 107u8, 145u8, 141u8,
            ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    owner: topics.1,
                    word: data.0,
                    mask: data.1,
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
                    > as alloy_sol_types::SolType>::tokenize(&self.word),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.mask),
                )
            }
            #[inline]
            fn topics(&self) -> <Self::TopicList as alloy_sol_types::SolType>::RustType {
                (Self::SIGNATURE_HASH.into(), self.owner.clone())
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
                    &self.owner,
                );
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for UnorderedNonceInvalidation {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&UnorderedNonceInvalidation> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(
                this: &UnorderedNonceInvalidation,
            ) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `DOMAIN_SEPARATOR()` and selector `0x3644e515`.
```solidity
function DOMAIN_SEPARATOR() external view returns (bytes32);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct DOMAIN_SEPARATORCall;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`DOMAIN_SEPARATOR()`](DOMAIN_SEPARATORCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct DOMAIN_SEPARATORReturn {
        #[allow(missing_docs)]
        pub _0: alloy_sol_types::private::FixedBytes<32>,
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
            impl ::core::convert::From<DOMAIN_SEPARATORCall>
            for UnderlyingRustTuple<'_> {
                fn from(value: DOMAIN_SEPARATORCall) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for DOMAIN_SEPARATORCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self
                }
            }
        }
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
            impl ::core::convert::From<DOMAIN_SEPARATORReturn>
            for UnderlyingRustTuple<'_> {
                fn from(value: DOMAIN_SEPARATORReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for DOMAIN_SEPARATORReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for DOMAIN_SEPARATORCall {
            type Parameters<'a> = ();
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::FixedBytes<32>;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::FixedBytes<32>,);
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "DOMAIN_SEPARATOR()";
            const SELECTOR: [u8; 4] = [54u8, 68u8, 229u8, 21u8];
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
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(ret),
                )
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence(data)
                    .map(|r| {
                        let r: DOMAIN_SEPARATORReturn = r.into();
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
                        let r: DOMAIN_SEPARATORReturn = r.into();
                        r._0
                    })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `allowance(address,address,address)` and selector `0x927da105`.
```solidity
function allowance(address, address, address) external view returns (uint160 amount, uint48 expiration, uint48 nonce);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct allowanceCall {
        #[allow(missing_docs)]
        pub _0: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub _1: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub _2: alloy_sol_types::private::Address,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`allowance(address,address,address)`](allowanceCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct allowanceReturn {
        #[allow(missing_docs)]
        pub amount: alloy_sol_types::private::primitives::aliases::U160,
        #[allow(missing_docs)]
        pub expiration: alloy_sol_types::private::primitives::aliases::U48,
        #[allow(missing_docs)]
        pub nonce: alloy_sol_types::private::primitives::aliases::U48,
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
                alloy_sol_types::sol_data::Address,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
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
            impl ::core::convert::From<allowanceCall> for UnderlyingRustTuple<'_> {
                fn from(value: allowanceCall) -> Self {
                    (value._0, value._1, value._2)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for allowanceCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        _0: tuple.0,
                        _1: tuple.1,
                        _2: tuple.2,
                    }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Uint<160>,
                alloy_sol_types::sol_data::Uint<48>,
                alloy_sol_types::sol_data::Uint<48>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::primitives::aliases::U160,
                alloy_sol_types::private::primitives::aliases::U48,
                alloy_sol_types::private::primitives::aliases::U48,
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
            impl ::core::convert::From<allowanceReturn> for UnderlyingRustTuple<'_> {
                fn from(value: allowanceReturn) -> Self {
                    (value.amount, value.expiration, value.nonce)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for allowanceReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        amount: tuple.0,
                        expiration: tuple.1,
                        nonce: tuple.2,
                    }
                }
            }
        }
        impl allowanceReturn {
            fn _tokenize(
                &self,
            ) -> <allowanceCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                (
                    <alloy_sol_types::sol_data::Uint<
                        160,
                    > as alloy_sol_types::SolType>::tokenize(&self.amount),
                    <alloy_sol_types::sol_data::Uint<
                        48,
                    > as alloy_sol_types::SolType>::tokenize(&self.expiration),
                    <alloy_sol_types::sol_data::Uint<
                        48,
                    > as alloy_sol_types::SolType>::tokenize(&self.nonce),
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for allowanceCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
            );
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = allowanceReturn;
            type ReturnTuple<'a> = (
                alloy_sol_types::sol_data::Uint<160>,
                alloy_sol_types::sol_data::Uint<48>,
                alloy_sol_types::sol_data::Uint<48>,
            );
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "allowance(address,address,address)";
            const SELECTOR: [u8; 4] = [146u8, 125u8, 161u8, 5u8];
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
                        &self._0,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self._1,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self._2,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                allowanceReturn::_tokenize(ret)
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
    /**Function with signature `approve(address,address,uint160,uint48)` and selector `0x87517c45`.
```solidity
function approve(address token, address spender, uint160 amount, uint48 expiration) external;
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct approveCall {
        #[allow(missing_docs)]
        pub token: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub spender: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub amount: alloy_sol_types::private::primitives::aliases::U160,
        #[allow(missing_docs)]
        pub expiration: alloy_sol_types::private::primitives::aliases::U48,
    }
    ///Container type for the return parameters of the [`approve(address,address,uint160,uint48)`](approveCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct approveReturn {}
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
                alloy_sol_types::sol_data::Uint<160>,
                alloy_sol_types::sol_data::Uint<48>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
                alloy_sol_types::private::Address,
                alloy_sol_types::private::primitives::aliases::U160,
                alloy_sol_types::private::primitives::aliases::U48,
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
            impl ::core::convert::From<approveCall> for UnderlyingRustTuple<'_> {
                fn from(value: approveCall) -> Self {
                    (value.token, value.spender, value.amount, value.expiration)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for approveCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        token: tuple.0,
                        spender: tuple.1,
                        amount: tuple.2,
                        expiration: tuple.3,
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
            impl ::core::convert::From<approveReturn> for UnderlyingRustTuple<'_> {
                fn from(value: approveReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for approveReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl approveReturn {
            fn _tokenize(
                &self,
            ) -> <approveCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for approveCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<160>,
                alloy_sol_types::sol_data::Uint<48>,
            );
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = approveReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "approve(address,address,uint160,uint48)";
            const SELECTOR: [u8; 4] = [135u8, 81u8, 124u8, 69u8];
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
                        &self.token,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.spender,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        160,
                    > as alloy_sol_types::SolType>::tokenize(&self.amount),
                    <alloy_sol_types::sol_data::Uint<
                        48,
                    > as alloy_sol_types::SolType>::tokenize(&self.expiration),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                approveReturn::_tokenize(ret)
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
    /**Function with signature `invalidateNonces(address,address,uint48)` and selector `0x65d9723c`.
```solidity
function invalidateNonces(address token, address spender, uint48 newNonce) external;
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct invalidateNoncesCall {
        #[allow(missing_docs)]
        pub token: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub spender: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub newNonce: alloy_sol_types::private::primitives::aliases::U48,
    }
    ///Container type for the return parameters of the [`invalidateNonces(address,address,uint48)`](invalidateNoncesCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct invalidateNoncesReturn {}
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
                alloy_sol_types::sol_data::Uint<48>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
                alloy_sol_types::private::Address,
                alloy_sol_types::private::primitives::aliases::U48,
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
            impl ::core::convert::From<invalidateNoncesCall>
            for UnderlyingRustTuple<'_> {
                fn from(value: invalidateNoncesCall) -> Self {
                    (value.token, value.spender, value.newNonce)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for invalidateNoncesCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        token: tuple.0,
                        spender: tuple.1,
                        newNonce: tuple.2,
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
            impl ::core::convert::From<invalidateNoncesReturn>
            for UnderlyingRustTuple<'_> {
                fn from(value: invalidateNoncesReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for invalidateNoncesReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl invalidateNoncesReturn {
            fn _tokenize(
                &self,
            ) -> <invalidateNoncesCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for invalidateNoncesCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<48>,
            );
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = invalidateNoncesReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "invalidateNonces(address,address,uint48)";
            const SELECTOR: [u8; 4] = [101u8, 217u8, 114u8, 60u8];
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
                        &self.token,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.spender,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        48,
                    > as alloy_sol_types::SolType>::tokenize(&self.newNonce),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                invalidateNoncesReturn::_tokenize(ret)
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
    /**Function with signature `invalidateUnorderedNonces(uint256,uint256)` and selector `0x3ff9dcb1`.
```solidity
function invalidateUnorderedNonces(uint256 wordPos, uint256 mask) external;
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct invalidateUnorderedNoncesCall {
        #[allow(missing_docs)]
        pub wordPos: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub mask: alloy_sol_types::private::primitives::aliases::U256,
    }
    ///Container type for the return parameters of the [`invalidateUnorderedNonces(uint256,uint256)`](invalidateUnorderedNoncesCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct invalidateUnorderedNoncesReturn {}
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
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
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
            impl ::core::convert::From<invalidateUnorderedNoncesCall>
            for UnderlyingRustTuple<'_> {
                fn from(value: invalidateUnorderedNoncesCall) -> Self {
                    (value.wordPos, value.mask)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for invalidateUnorderedNoncesCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        wordPos: tuple.0,
                        mask: tuple.1,
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
            impl ::core::convert::From<invalidateUnorderedNoncesReturn>
            for UnderlyingRustTuple<'_> {
                fn from(value: invalidateUnorderedNoncesReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for invalidateUnorderedNoncesReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl invalidateUnorderedNoncesReturn {
            fn _tokenize(
                &self,
            ) -> <invalidateUnorderedNoncesCall as alloy_sol_types::SolCall>::ReturnToken<
                '_,
            > {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for invalidateUnorderedNoncesCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = invalidateUnorderedNoncesReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "invalidateUnorderedNonces(uint256,uint256)";
            const SELECTOR: [u8; 4] = [63u8, 249u8, 220u8, 177u8];
            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }
            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.wordPos),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.mask),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                invalidateUnorderedNoncesReturn::_tokenize(ret)
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
    /**Function with signature `lockdown((address,address)[])` and selector `0xcc53287f`.
```solidity
function lockdown(IAllowanceTransfer.TokenSpenderPair[] memory approvals) external;
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct lockdownCall {
        #[allow(missing_docs)]
        pub approvals: alloy_sol_types::private::Vec<
            <IAllowanceTransfer::TokenSpenderPair as alloy_sol_types::SolType>::RustType,
        >,
    }
    ///Container type for the return parameters of the [`lockdown((address,address)[])`](lockdownCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct lockdownReturn {}
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
                alloy_sol_types::sol_data::Array<IAllowanceTransfer::TokenSpenderPair>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Vec<
                    <IAllowanceTransfer::TokenSpenderPair as alloy_sol_types::SolType>::RustType,
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
            impl ::core::convert::From<lockdownCall> for UnderlyingRustTuple<'_> {
                fn from(value: lockdownCall) -> Self {
                    (value.approvals,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for lockdownCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { approvals: tuple.0 }
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
            impl ::core::convert::From<lockdownReturn> for UnderlyingRustTuple<'_> {
                fn from(value: lockdownReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for lockdownReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl lockdownReturn {
            fn _tokenize(
                &self,
            ) -> <lockdownCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for lockdownCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Array<IAllowanceTransfer::TokenSpenderPair>,
            );
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = lockdownReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "lockdown((address,address)[])";
            const SELECTOR: [u8; 4] = [204u8, 83u8, 40u8, 127u8];
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
                        IAllowanceTransfer::TokenSpenderPair,
                    > as alloy_sol_types::SolType>::tokenize(&self.approvals),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                lockdownReturn::_tokenize(ret)
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
    /**Function with signature `nonceBitmap(address,uint256)` and selector `0x4fe02b44`.
```solidity
function nonceBitmap(address, uint256) external view returns (uint256);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct nonceBitmapCall {
        #[allow(missing_docs)]
        pub _0: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub _1: alloy_sol_types::private::primitives::aliases::U256,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`nonceBitmap(address,uint256)`](nonceBitmapCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct nonceBitmapReturn {
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
        use alloy_sol_types as alloy_sol_types;
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
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
            impl ::core::convert::From<nonceBitmapCall> for UnderlyingRustTuple<'_> {
                fn from(value: nonceBitmapCall) -> Self {
                    (value._0, value._1)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for nonceBitmapCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0, _1: tuple.1 }
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
            impl ::core::convert::From<nonceBitmapReturn> for UnderlyingRustTuple<'_> {
                fn from(value: nonceBitmapReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for nonceBitmapReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for nonceBitmapCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
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
            const SIGNATURE: &'static str = "nonceBitmap(address,uint256)";
            const SELECTOR: [u8; 4] = [79u8, 224u8, 43u8, 68u8];
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
                        &self._0,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self._1),
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
                        let r: nonceBitmapReturn = r.into();
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
                        let r: nonceBitmapReturn = r.into();
                        r._0
                    })
            }
        }
    };
    #[derive()]
    /**Function with signature `permit(address,((address,uint160,uint48,uint48),address,uint256),bytes)` and selector `0x2b67b570`.
```solidity
function permit(address owner, IAllowanceTransfer.PermitSingle memory permitSingle, bytes memory signature) external;
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct permitCall {
        #[allow(missing_docs)]
        pub owner: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub permitSingle: <IAllowanceTransfer::PermitSingle as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub signature: alloy_sol_types::private::Bytes,
    }
    ///Container type for the return parameters of the [`permit(address,((address,uint160,uint48,uint48),address,uint256),bytes)`](permitCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct permitReturn {}
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
                IAllowanceTransfer::PermitSingle,
                alloy_sol_types::sol_data::Bytes,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
                <IAllowanceTransfer::PermitSingle as alloy_sol_types::SolType>::RustType,
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
            impl ::core::convert::From<permitCall> for UnderlyingRustTuple<'_> {
                fn from(value: permitCall) -> Self {
                    (value.owner, value.permitSingle, value.signature)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for permitCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        owner: tuple.0,
                        permitSingle: tuple.1,
                        signature: tuple.2,
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
            impl ::core::convert::From<permitReturn> for UnderlyingRustTuple<'_> {
                fn from(value: permitReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for permitReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl permitReturn {
            fn _tokenize(
                &self,
            ) -> <permitCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for permitCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                IAllowanceTransfer::PermitSingle,
                alloy_sol_types::sol_data::Bytes,
            );
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = permitReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "permit(address,((address,uint160,uint48,uint48),address,uint256),bytes)";
            const SELECTOR: [u8; 4] = [43u8, 103u8, 181u8, 112u8];
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
                        &self.owner,
                    ),
                    <IAllowanceTransfer::PermitSingle as alloy_sol_types::SolType>::tokenize(
                        &self.permitSingle,
                    ),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.signature,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                permitReturn::_tokenize(ret)
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
    /**Function with signature `permitTransferFrom(((address,uint256),uint256,uint256),(address,uint256),address,bytes)` and selector `0x30f28b7a`.
```solidity
function permitTransferFrom(ISignatureTransfer.PermitTransferFrom memory permit, ISignatureTransfer.SignatureTransferDetails memory transferDetails, address owner, bytes memory signature) external;
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct permitTransferFromCall {
        #[allow(missing_docs)]
        pub permit: <ISignatureTransfer::PermitTransferFrom as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub transferDetails: <ISignatureTransfer::SignatureTransferDetails as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub owner: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub signature: alloy_sol_types::private::Bytes,
    }
    ///Container type for the return parameters of the [`permitTransferFrom(((address,uint256),uint256,uint256),(address,uint256),address,bytes)`](permitTransferFromCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct permitTransferFromReturn {}
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
                ISignatureTransfer::PermitTransferFrom,
                ISignatureTransfer::SignatureTransferDetails,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Bytes,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                <ISignatureTransfer::PermitTransferFrom as alloy_sol_types::SolType>::RustType,
                <ISignatureTransfer::SignatureTransferDetails as alloy_sol_types::SolType>::RustType,
                alloy_sol_types::private::Address,
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
            impl ::core::convert::From<permitTransferFromCall>
            for UnderlyingRustTuple<'_> {
                fn from(value: permitTransferFromCall) -> Self {
                    (value.permit, value.transferDetails, value.owner, value.signature)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for permitTransferFromCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        permit: tuple.0,
                        transferDetails: tuple.1,
                        owner: tuple.2,
                        signature: tuple.3,
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
            impl ::core::convert::From<permitTransferFromReturn>
            for UnderlyingRustTuple<'_> {
                fn from(value: permitTransferFromReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for permitTransferFromReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl permitTransferFromReturn {
            fn _tokenize(
                &self,
            ) -> <permitTransferFromCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for permitTransferFromCall {
            type Parameters<'a> = (
                ISignatureTransfer::PermitTransferFrom,
                ISignatureTransfer::SignatureTransferDetails,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Bytes,
            );
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = permitTransferFromReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "permitTransferFrom(((address,uint256),uint256,uint256),(address,uint256),address,bytes)";
            const SELECTOR: [u8; 4] = [48u8, 242u8, 139u8, 122u8];
            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }
            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <ISignatureTransfer::PermitTransferFrom as alloy_sol_types::SolType>::tokenize(
                        &self.permit,
                    ),
                    <ISignatureTransfer::SignatureTransferDetails as alloy_sol_types::SolType>::tokenize(
                        &self.transferDetails,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.owner,
                    ),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.signature,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                permitTransferFromReturn::_tokenize(ret)
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
    /**Function with signature `permitWitnessTransferFrom(((address,uint256)[],uint256,uint256),(address,uint256)[],address,bytes32,string,bytes)` and selector `0xfe8ec1a7`.
```solidity
function permitWitnessTransferFrom(ISignatureTransfer.PermitBatchTransferFrom memory permit, ISignatureTransfer.SignatureTransferDetails[] memory transferDetails, address owner, bytes32 witness, string memory witnessTypeString, bytes memory signature) external;
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct permitWitnessTransferFromCall {
        #[allow(missing_docs)]
        pub permit: <ISignatureTransfer::PermitBatchTransferFrom as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub transferDetails: alloy_sol_types::private::Vec<
            <ISignatureTransfer::SignatureTransferDetails as alloy_sol_types::SolType>::RustType,
        >,
        #[allow(missing_docs)]
        pub owner: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub witness: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub witnessTypeString: alloy_sol_types::private::String,
        #[allow(missing_docs)]
        pub signature: alloy_sol_types::private::Bytes,
    }
    ///Container type for the return parameters of the [`permitWitnessTransferFrom(((address,uint256)[],uint256,uint256),(address,uint256)[],address,bytes32,string,bytes)`](permitWitnessTransferFromCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct permitWitnessTransferFromReturn {}
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
                ISignatureTransfer::PermitBatchTransferFrom,
                alloy_sol_types::sol_data::Array<
                    ISignatureTransfer::SignatureTransferDetails,
                >,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::String,
                alloy_sol_types::sol_data::Bytes,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                <ISignatureTransfer::PermitBatchTransferFrom as alloy_sol_types::SolType>::RustType,
                alloy_sol_types::private::Vec<
                    <ISignatureTransfer::SignatureTransferDetails as alloy_sol_types::SolType>::RustType,
                >,
                alloy_sol_types::private::Address,
                alloy_sol_types::private::FixedBytes<32>,
                alloy_sol_types::private::String,
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
            impl ::core::convert::From<permitWitnessTransferFromCall>
            for UnderlyingRustTuple<'_> {
                fn from(value: permitWitnessTransferFromCall) -> Self {
                    (
                        value.permit,
                        value.transferDetails,
                        value.owner,
                        value.witness,
                        value.witnessTypeString,
                        value.signature,
                    )
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for permitWitnessTransferFromCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        permit: tuple.0,
                        transferDetails: tuple.1,
                        owner: tuple.2,
                        witness: tuple.3,
                        witnessTypeString: tuple.4,
                        signature: tuple.5,
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
            impl ::core::convert::From<permitWitnessTransferFromReturn>
            for UnderlyingRustTuple<'_> {
                fn from(value: permitWitnessTransferFromReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for permitWitnessTransferFromReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl permitWitnessTransferFromReturn {
            fn _tokenize(
                &self,
            ) -> <permitWitnessTransferFromCall as alloy_sol_types::SolCall>::ReturnToken<
                '_,
            > {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for permitWitnessTransferFromCall {
            type Parameters<'a> = (
                ISignatureTransfer::PermitBatchTransferFrom,
                alloy_sol_types::sol_data::Array<
                    ISignatureTransfer::SignatureTransferDetails,
                >,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::String,
                alloy_sol_types::sol_data::Bytes,
            );
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = permitWitnessTransferFromReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "permitWitnessTransferFrom(((address,uint256)[],uint256,uint256),(address,uint256)[],address,bytes32,string,bytes)";
            const SELECTOR: [u8; 4] = [254u8, 142u8, 193u8, 167u8];
            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }
            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <ISignatureTransfer::PermitBatchTransferFrom as alloy_sol_types::SolType>::tokenize(
                        &self.permit,
                    ),
                    <alloy_sol_types::sol_data::Array<
                        ISignatureTransfer::SignatureTransferDetails,
                    > as alloy_sol_types::SolType>::tokenize(&self.transferDetails),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.owner,
                    ),
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self.witness),
                    <alloy_sol_types::sol_data::String as alloy_sol_types::SolType>::tokenize(
                        &self.witnessTypeString,
                    ),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.signature,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                permitWitnessTransferFromReturn::_tokenize(ret)
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
    /**Function with signature `transferFrom(address,address,uint160,address)` and selector `0x36c78516`.
```solidity
function transferFrom(address from, address to, uint160 amount, address token) external;
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct transferFromCall {
        #[allow(missing_docs)]
        pub from: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub to: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub amount: alloy_sol_types::private::primitives::aliases::U160,
        #[allow(missing_docs)]
        pub token: alloy_sol_types::private::Address,
    }
    ///Container type for the return parameters of the [`transferFrom(address,address,uint160,address)`](transferFromCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct transferFromReturn {}
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
                alloy_sol_types::sol_data::Uint<160>,
                alloy_sol_types::sol_data::Address,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
                alloy_sol_types::private::Address,
                alloy_sol_types::private::primitives::aliases::U160,
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
            impl ::core::convert::From<transferFromCall> for UnderlyingRustTuple<'_> {
                fn from(value: transferFromCall) -> Self {
                    (value.from, value.to, value.amount, value.token)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for transferFromCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        from: tuple.0,
                        to: tuple.1,
                        amount: tuple.2,
                        token: tuple.3,
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
            impl ::core::convert::From<transferFromReturn> for UnderlyingRustTuple<'_> {
                fn from(value: transferFromReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for transferFromReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl transferFromReturn {
            fn _tokenize(
                &self,
            ) -> <transferFromCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for transferFromCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<160>,
                alloy_sol_types::sol_data::Address,
            );
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = transferFromReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "transferFrom(address,address,uint160,address)";
            const SELECTOR: [u8; 4] = [54u8, 199u8, 133u8, 22u8];
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
                        &self.from,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.to,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        160,
                    > as alloy_sol_types::SolType>::tokenize(&self.amount),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.token,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                transferFromReturn::_tokenize(ret)
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
    ///Container for all the [`Permit2`](self) function calls.
    #[derive(Clone)]
    #[derive()]
    pub enum Permit2Calls {
        #[allow(missing_docs)]
        DOMAIN_SEPARATOR(DOMAIN_SEPARATORCall),
        #[allow(missing_docs)]
        allowance(allowanceCall),
        #[allow(missing_docs)]
        approve(approveCall),
        #[allow(missing_docs)]
        invalidateNonces(invalidateNoncesCall),
        #[allow(missing_docs)]
        invalidateUnorderedNonces(invalidateUnorderedNoncesCall),
        #[allow(missing_docs)]
        lockdown(lockdownCall),
        #[allow(missing_docs)]
        nonceBitmap(nonceBitmapCall),
        #[allow(missing_docs)]
        permit(permitCall),
        #[allow(missing_docs)]
        permitTransferFrom(permitTransferFromCall),
        #[allow(missing_docs)]
        permitWitnessTransferFrom(permitWitnessTransferFromCall),
        #[allow(missing_docs)]
        transferFrom(transferFromCall),
    }
    impl Permit2Calls {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the variants.
        /// No guarantees are made about the order of the selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 4usize]] = &[
            [43u8, 103u8, 181u8, 112u8],
            [48u8, 242u8, 139u8, 122u8],
            [54u8, 68u8, 229u8, 21u8],
            [54u8, 199u8, 133u8, 22u8],
            [63u8, 249u8, 220u8, 177u8],
            [79u8, 224u8, 43u8, 68u8],
            [101u8, 217u8, 114u8, 60u8],
            [135u8, 81u8, 124u8, 69u8],
            [146u8, 125u8, 161u8, 5u8],
            [204u8, 83u8, 40u8, 127u8],
            [254u8, 142u8, 193u8, 167u8],
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(permit),
            ::core::stringify!(permitTransferFrom),
            ::core::stringify!(DOMAIN_SEPARATOR),
            ::core::stringify!(transferFrom),
            ::core::stringify!(invalidateUnorderedNonces),
            ::core::stringify!(nonceBitmap),
            ::core::stringify!(invalidateNonces),
            ::core::stringify!(approve),
            ::core::stringify!(allowance),
            ::core::stringify!(lockdown),
            ::core::stringify!(permitWitnessTransferFrom),
        ];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <permitCall as alloy_sol_types::SolCall>::SIGNATURE,
            <permitTransferFromCall as alloy_sol_types::SolCall>::SIGNATURE,
            <DOMAIN_SEPARATORCall as alloy_sol_types::SolCall>::SIGNATURE,
            <transferFromCall as alloy_sol_types::SolCall>::SIGNATURE,
            <invalidateUnorderedNoncesCall as alloy_sol_types::SolCall>::SIGNATURE,
            <nonceBitmapCall as alloy_sol_types::SolCall>::SIGNATURE,
            <invalidateNoncesCall as alloy_sol_types::SolCall>::SIGNATURE,
            <approveCall as alloy_sol_types::SolCall>::SIGNATURE,
            <allowanceCall as alloy_sol_types::SolCall>::SIGNATURE,
            <lockdownCall as alloy_sol_types::SolCall>::SIGNATURE,
            <permitWitnessTransferFromCall as alloy_sol_types::SolCall>::SIGNATURE,
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
    impl alloy_sol_types::SolInterface for Permit2Calls {
        const NAME: &'static str = "Permit2Calls";
        const MIN_DATA_LENGTH: usize = 0usize;
        const COUNT: usize = 11usize;
        #[inline]
        fn selector(&self) -> [u8; 4] {
            match self {
                Self::DOMAIN_SEPARATOR(_) => {
                    <DOMAIN_SEPARATORCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::allowance(_) => {
                    <allowanceCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::approve(_) => <approveCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::invalidateNonces(_) => {
                    <invalidateNoncesCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::invalidateUnorderedNonces(_) => {
                    <invalidateUnorderedNoncesCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::lockdown(_) => <lockdownCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::nonceBitmap(_) => {
                    <nonceBitmapCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::permit(_) => <permitCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::permitTransferFrom(_) => {
                    <permitTransferFromCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::permitWitnessTransferFrom(_) => {
                    <permitWitnessTransferFromCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::transferFrom(_) => {
                    <transferFromCall as alloy_sol_types::SolCall>::SELECTOR
                }
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
            static DECODE_SHIMS: &[fn(&[u8]) -> alloy_sol_types::Result<Permit2Calls>] = &[
                {
                    fn permit(data: &[u8]) -> alloy_sol_types::Result<Permit2Calls> {
                        <permitCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(Permit2Calls::permit)
                    }
                    permit
                },
                {
                    fn permitTransferFrom(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Calls> {
                        <permitTransferFromCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(Permit2Calls::permitTransferFrom)
                    }
                    permitTransferFrom
                },
                {
                    fn DOMAIN_SEPARATOR(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Calls> {
                        <DOMAIN_SEPARATORCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(Permit2Calls::DOMAIN_SEPARATOR)
                    }
                    DOMAIN_SEPARATOR
                },
                {
                    fn transferFrom(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Calls> {
                        <transferFromCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(Permit2Calls::transferFrom)
                    }
                    transferFrom
                },
                {
                    fn invalidateUnorderedNonces(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Calls> {
                        <invalidateUnorderedNoncesCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(Permit2Calls::invalidateUnorderedNonces)
                    }
                    invalidateUnorderedNonces
                },
                {
                    fn nonceBitmap(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Calls> {
                        <nonceBitmapCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(Permit2Calls::nonceBitmap)
                    }
                    nonceBitmap
                },
                {
                    fn invalidateNonces(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Calls> {
                        <invalidateNoncesCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(Permit2Calls::invalidateNonces)
                    }
                    invalidateNonces
                },
                {
                    fn approve(data: &[u8]) -> alloy_sol_types::Result<Permit2Calls> {
                        <approveCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(Permit2Calls::approve)
                    }
                    approve
                },
                {
                    fn allowance(data: &[u8]) -> alloy_sol_types::Result<Permit2Calls> {
                        <allowanceCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(Permit2Calls::allowance)
                    }
                    allowance
                },
                {
                    fn lockdown(data: &[u8]) -> alloy_sol_types::Result<Permit2Calls> {
                        <lockdownCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(Permit2Calls::lockdown)
                    }
                    lockdown
                },
                {
                    fn permitWitnessTransferFrom(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Calls> {
                        <permitWitnessTransferFromCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(Permit2Calls::permitWitnessTransferFrom)
                    }
                    permitWitnessTransferFrom
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
            ) -> alloy_sol_types::Result<Permit2Calls>] = &[
                {
                    fn permit(data: &[u8]) -> alloy_sol_types::Result<Permit2Calls> {
                        <permitCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(Permit2Calls::permit)
                    }
                    permit
                },
                {
                    fn permitTransferFrom(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Calls> {
                        <permitTransferFromCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(Permit2Calls::permitTransferFrom)
                    }
                    permitTransferFrom
                },
                {
                    fn DOMAIN_SEPARATOR(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Calls> {
                        <DOMAIN_SEPARATORCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(Permit2Calls::DOMAIN_SEPARATOR)
                    }
                    DOMAIN_SEPARATOR
                },
                {
                    fn transferFrom(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Calls> {
                        <transferFromCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(Permit2Calls::transferFrom)
                    }
                    transferFrom
                },
                {
                    fn invalidateUnorderedNonces(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Calls> {
                        <invalidateUnorderedNoncesCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(Permit2Calls::invalidateUnorderedNonces)
                    }
                    invalidateUnorderedNonces
                },
                {
                    fn nonceBitmap(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Calls> {
                        <nonceBitmapCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(Permit2Calls::nonceBitmap)
                    }
                    nonceBitmap
                },
                {
                    fn invalidateNonces(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Calls> {
                        <invalidateNoncesCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(Permit2Calls::invalidateNonces)
                    }
                    invalidateNonces
                },
                {
                    fn approve(data: &[u8]) -> alloy_sol_types::Result<Permit2Calls> {
                        <approveCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(Permit2Calls::approve)
                    }
                    approve
                },
                {
                    fn allowance(data: &[u8]) -> alloy_sol_types::Result<Permit2Calls> {
                        <allowanceCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(Permit2Calls::allowance)
                    }
                    allowance
                },
                {
                    fn lockdown(data: &[u8]) -> alloy_sol_types::Result<Permit2Calls> {
                        <lockdownCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(Permit2Calls::lockdown)
                    }
                    lockdown
                },
                {
                    fn permitWitnessTransferFrom(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Calls> {
                        <permitWitnessTransferFromCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(Permit2Calls::permitWitnessTransferFrom)
                    }
                    permitWitnessTransferFrom
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
                Self::DOMAIN_SEPARATOR(inner) => {
                    <DOMAIN_SEPARATORCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::allowance(inner) => {
                    <allowanceCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::approve(inner) => {
                    <approveCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::invalidateNonces(inner) => {
                    <invalidateNoncesCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::invalidateUnorderedNonces(inner) => {
                    <invalidateUnorderedNoncesCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::lockdown(inner) => {
                    <lockdownCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::nonceBitmap(inner) => {
                    <nonceBitmapCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::permit(inner) => {
                    <permitCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::permitTransferFrom(inner) => {
                    <permitTransferFromCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::permitWitnessTransferFrom(inner) => {
                    <permitWitnessTransferFromCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::transferFrom(inner) => {
                    <transferFromCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
            }
        }
        #[inline]
        fn abi_encode_raw(&self, out: &mut alloy_sol_types::private::Vec<u8>) {
            match self {
                Self::DOMAIN_SEPARATOR(inner) => {
                    <DOMAIN_SEPARATORCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::allowance(inner) => {
                    <allowanceCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::approve(inner) => {
                    <approveCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::invalidateNonces(inner) => {
                    <invalidateNoncesCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::invalidateUnorderedNonces(inner) => {
                    <invalidateUnorderedNoncesCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::lockdown(inner) => {
                    <lockdownCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::nonceBitmap(inner) => {
                    <nonceBitmapCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::permit(inner) => {
                    <permitCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::permitTransferFrom(inner) => {
                    <permitTransferFromCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::permitWitnessTransferFrom(inner) => {
                    <permitWitnessTransferFromCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::transferFrom(inner) => {
                    <transferFromCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
            }
        }
    }
    ///Container for all the [`Permit2`](self) custom errors.
    #[derive(Clone)]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub enum Permit2Errors {
        #[allow(missing_docs)]
        AllowanceExpired(AllowanceExpired),
        #[allow(missing_docs)]
        ExcessiveInvalidation(ExcessiveInvalidation),
        #[allow(missing_docs)]
        InsufficientAllowance(InsufficientAllowance),
        #[allow(missing_docs)]
        InvalidAmount(InvalidAmount),
        #[allow(missing_docs)]
        InvalidContractSignature(InvalidContractSignature),
        #[allow(missing_docs)]
        InvalidNonce(InvalidNonce),
        #[allow(missing_docs)]
        InvalidSignature(InvalidSignature),
        #[allow(missing_docs)]
        InvalidSignatureLength(InvalidSignatureLength),
        #[allow(missing_docs)]
        InvalidSigner(InvalidSigner),
        #[allow(missing_docs)]
        LengthMismatch(LengthMismatch),
        #[allow(missing_docs)]
        SignatureExpired(SignatureExpired),
    }
    impl Permit2Errors {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the variants.
        /// No guarantees are made about the order of the selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 4usize]] = &[
            [36u8, 211u8, 90u8, 38u8],
            [55u8, 40u8, 184u8, 61u8],
            [75u8, 230u8, 50u8, 27u8],
            [117u8, 102u8, 136u8, 254u8],
            [129u8, 94u8, 29u8, 100u8],
            [139u8, 170u8, 87u8, 159u8],
            [176u8, 102u8, 156u8, 188u8],
            [205u8, 33u8, 219u8, 79u8],
            [216u8, 27u8, 47u8, 46u8],
            [249u8, 111u8, 176u8, 113u8],
            [255u8, 99u8, 58u8, 56u8],
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(ExcessiveInvalidation),
            ::core::stringify!(InvalidAmount),
            ::core::stringify!(InvalidSignatureLength),
            ::core::stringify!(InvalidNonce),
            ::core::stringify!(InvalidSigner),
            ::core::stringify!(InvalidSignature),
            ::core::stringify!(InvalidContractSignature),
            ::core::stringify!(SignatureExpired),
            ::core::stringify!(AllowanceExpired),
            ::core::stringify!(InsufficientAllowance),
            ::core::stringify!(LengthMismatch),
        ];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <ExcessiveInvalidation as alloy_sol_types::SolError>::SIGNATURE,
            <InvalidAmount as alloy_sol_types::SolError>::SIGNATURE,
            <InvalidSignatureLength as alloy_sol_types::SolError>::SIGNATURE,
            <InvalidNonce as alloy_sol_types::SolError>::SIGNATURE,
            <InvalidSigner as alloy_sol_types::SolError>::SIGNATURE,
            <InvalidSignature as alloy_sol_types::SolError>::SIGNATURE,
            <InvalidContractSignature as alloy_sol_types::SolError>::SIGNATURE,
            <SignatureExpired as alloy_sol_types::SolError>::SIGNATURE,
            <AllowanceExpired as alloy_sol_types::SolError>::SIGNATURE,
            <InsufficientAllowance as alloy_sol_types::SolError>::SIGNATURE,
            <LengthMismatch as alloy_sol_types::SolError>::SIGNATURE,
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
    impl alloy_sol_types::SolInterface for Permit2Errors {
        const NAME: &'static str = "Permit2Errors";
        const MIN_DATA_LENGTH: usize = 0usize;
        const COUNT: usize = 11usize;
        #[inline]
        fn selector(&self) -> [u8; 4] {
            match self {
                Self::AllowanceExpired(_) => {
                    <AllowanceExpired as alloy_sol_types::SolError>::SELECTOR
                }
                Self::ExcessiveInvalidation(_) => {
                    <ExcessiveInvalidation as alloy_sol_types::SolError>::SELECTOR
                }
                Self::InsufficientAllowance(_) => {
                    <InsufficientAllowance as alloy_sol_types::SolError>::SELECTOR
                }
                Self::InvalidAmount(_) => {
                    <InvalidAmount as alloy_sol_types::SolError>::SELECTOR
                }
                Self::InvalidContractSignature(_) => {
                    <InvalidContractSignature as alloy_sol_types::SolError>::SELECTOR
                }
                Self::InvalidNonce(_) => {
                    <InvalidNonce as alloy_sol_types::SolError>::SELECTOR
                }
                Self::InvalidSignature(_) => {
                    <InvalidSignature as alloy_sol_types::SolError>::SELECTOR
                }
                Self::InvalidSignatureLength(_) => {
                    <InvalidSignatureLength as alloy_sol_types::SolError>::SELECTOR
                }
                Self::InvalidSigner(_) => {
                    <InvalidSigner as alloy_sol_types::SolError>::SELECTOR
                }
                Self::LengthMismatch(_) => {
                    <LengthMismatch as alloy_sol_types::SolError>::SELECTOR
                }
                Self::SignatureExpired(_) => {
                    <SignatureExpired as alloy_sol_types::SolError>::SELECTOR
                }
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
            ) -> alloy_sol_types::Result<Permit2Errors>] = &[
                {
                    fn ExcessiveInvalidation(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Errors> {
                        <ExcessiveInvalidation as alloy_sol_types::SolError>::abi_decode_raw(
                                data,
                            )
                            .map(Permit2Errors::ExcessiveInvalidation)
                    }
                    ExcessiveInvalidation
                },
                {
                    fn InvalidAmount(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Errors> {
                        <InvalidAmount as alloy_sol_types::SolError>::abi_decode_raw(
                                data,
                            )
                            .map(Permit2Errors::InvalidAmount)
                    }
                    InvalidAmount
                },
                {
                    fn InvalidSignatureLength(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Errors> {
                        <InvalidSignatureLength as alloy_sol_types::SolError>::abi_decode_raw(
                                data,
                            )
                            .map(Permit2Errors::InvalidSignatureLength)
                    }
                    InvalidSignatureLength
                },
                {
                    fn InvalidNonce(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Errors> {
                        <InvalidNonce as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(Permit2Errors::InvalidNonce)
                    }
                    InvalidNonce
                },
                {
                    fn InvalidSigner(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Errors> {
                        <InvalidSigner as alloy_sol_types::SolError>::abi_decode_raw(
                                data,
                            )
                            .map(Permit2Errors::InvalidSigner)
                    }
                    InvalidSigner
                },
                {
                    fn InvalidSignature(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Errors> {
                        <InvalidSignature as alloy_sol_types::SolError>::abi_decode_raw(
                                data,
                            )
                            .map(Permit2Errors::InvalidSignature)
                    }
                    InvalidSignature
                },
                {
                    fn InvalidContractSignature(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Errors> {
                        <InvalidContractSignature as alloy_sol_types::SolError>::abi_decode_raw(
                                data,
                            )
                            .map(Permit2Errors::InvalidContractSignature)
                    }
                    InvalidContractSignature
                },
                {
                    fn SignatureExpired(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Errors> {
                        <SignatureExpired as alloy_sol_types::SolError>::abi_decode_raw(
                                data,
                            )
                            .map(Permit2Errors::SignatureExpired)
                    }
                    SignatureExpired
                },
                {
                    fn AllowanceExpired(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Errors> {
                        <AllowanceExpired as alloy_sol_types::SolError>::abi_decode_raw(
                                data,
                            )
                            .map(Permit2Errors::AllowanceExpired)
                    }
                    AllowanceExpired
                },
                {
                    fn InsufficientAllowance(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Errors> {
                        <InsufficientAllowance as alloy_sol_types::SolError>::abi_decode_raw(
                                data,
                            )
                            .map(Permit2Errors::InsufficientAllowance)
                    }
                    InsufficientAllowance
                },
                {
                    fn LengthMismatch(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Errors> {
                        <LengthMismatch as alloy_sol_types::SolError>::abi_decode_raw(
                                data,
                            )
                            .map(Permit2Errors::LengthMismatch)
                    }
                    LengthMismatch
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
            ) -> alloy_sol_types::Result<Permit2Errors>] = &[
                {
                    fn ExcessiveInvalidation(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Errors> {
                        <ExcessiveInvalidation as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(Permit2Errors::ExcessiveInvalidation)
                    }
                    ExcessiveInvalidation
                },
                {
                    fn InvalidAmount(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Errors> {
                        <InvalidAmount as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(Permit2Errors::InvalidAmount)
                    }
                    InvalidAmount
                },
                {
                    fn InvalidSignatureLength(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Errors> {
                        <InvalidSignatureLength as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(Permit2Errors::InvalidSignatureLength)
                    }
                    InvalidSignatureLength
                },
                {
                    fn InvalidNonce(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Errors> {
                        <InvalidNonce as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(Permit2Errors::InvalidNonce)
                    }
                    InvalidNonce
                },
                {
                    fn InvalidSigner(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Errors> {
                        <InvalidSigner as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(Permit2Errors::InvalidSigner)
                    }
                    InvalidSigner
                },
                {
                    fn InvalidSignature(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Errors> {
                        <InvalidSignature as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(Permit2Errors::InvalidSignature)
                    }
                    InvalidSignature
                },
                {
                    fn InvalidContractSignature(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Errors> {
                        <InvalidContractSignature as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(Permit2Errors::InvalidContractSignature)
                    }
                    InvalidContractSignature
                },
                {
                    fn SignatureExpired(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Errors> {
                        <SignatureExpired as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(Permit2Errors::SignatureExpired)
                    }
                    SignatureExpired
                },
                {
                    fn AllowanceExpired(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Errors> {
                        <AllowanceExpired as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(Permit2Errors::AllowanceExpired)
                    }
                    AllowanceExpired
                },
                {
                    fn InsufficientAllowance(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Errors> {
                        <InsufficientAllowance as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(Permit2Errors::InsufficientAllowance)
                    }
                    InsufficientAllowance
                },
                {
                    fn LengthMismatch(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<Permit2Errors> {
                        <LengthMismatch as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(Permit2Errors::LengthMismatch)
                    }
                    LengthMismatch
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
                Self::AllowanceExpired(inner) => {
                    <AllowanceExpired as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::ExcessiveInvalidation(inner) => {
                    <ExcessiveInvalidation as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::InsufficientAllowance(inner) => {
                    <InsufficientAllowance as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::InvalidAmount(inner) => {
                    <InvalidAmount as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
                Self::InvalidContractSignature(inner) => {
                    <InvalidContractSignature as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::InvalidNonce(inner) => {
                    <InvalidNonce as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
                Self::InvalidSignature(inner) => {
                    <InvalidSignature as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::InvalidSignatureLength(inner) => {
                    <InvalidSignatureLength as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::InvalidSigner(inner) => {
                    <InvalidSigner as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
                Self::LengthMismatch(inner) => {
                    <LengthMismatch as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::SignatureExpired(inner) => {
                    <SignatureExpired as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
            }
        }
        #[inline]
        fn abi_encode_raw(&self, out: &mut alloy_sol_types::private::Vec<u8>) {
            match self {
                Self::AllowanceExpired(inner) => {
                    <AllowanceExpired as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::ExcessiveInvalidation(inner) => {
                    <ExcessiveInvalidation as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::InsufficientAllowance(inner) => {
                    <InsufficientAllowance as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::InvalidAmount(inner) => {
                    <InvalidAmount as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::InvalidContractSignature(inner) => {
                    <InvalidContractSignature as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::InvalidNonce(inner) => {
                    <InvalidNonce as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::InvalidSignature(inner) => {
                    <InvalidSignature as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::InvalidSignatureLength(inner) => {
                    <InvalidSignatureLength as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::InvalidSigner(inner) => {
                    <InvalidSigner as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::LengthMismatch(inner) => {
                    <LengthMismatch as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::SignatureExpired(inner) => {
                    <SignatureExpired as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
            }
        }
    }
    ///Container for all the [`Permit2`](self) events.
    #[derive(Clone)]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub enum Permit2Events {
        #[allow(missing_docs)]
        Approval(Approval),
        #[allow(missing_docs)]
        Lockdown(Lockdown),
        #[allow(missing_docs)]
        NonceInvalidation(NonceInvalidation),
        #[allow(missing_docs)]
        Permit(Permit),
        #[allow(missing_docs)]
        UnorderedNonceInvalidation(UnorderedNonceInvalidation),
    }
    impl Permit2Events {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the variants.
        /// No guarantees are made about the order of the selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 32usize]] = &[
            [
                55u8, 4u8, 144u8, 47u8, 150u8, 55u8, 102u8, 164u8, 229u8, 97u8, 187u8,
                170u8, 182u8, 230u8, 205u8, 193u8, 177u8, 221u8, 18u8, 246u8, 233u8,
                233u8, 150u8, 72u8, 218u8, 136u8, 67u8, 179u8, 244u8, 107u8, 145u8, 141u8,
            ],
            [
                85u8, 235u8, 144u8, 216u8, 16u8, 225u8, 112u8, 11u8, 53u8, 168u8, 231u8,
                226u8, 83u8, 149u8, 255u8, 127u8, 43u8, 34u8, 89u8, 171u8, 215u8, 65u8,
                92u8, 162u8, 40u8, 77u8, 251u8, 28u8, 36u8, 100u8, 24u8, 243u8,
            ],
            [
                137u8, 177u8, 173u8, 209u8, 94u8, 255u8, 86u8, 179u8, 223u8, 226u8,
                153u8, 173u8, 148u8, 224u8, 31u8, 43u8, 82u8, 251u8, 203u8, 128u8, 174u8,
                26u8, 59u8, 174u8, 166u8, 174u8, 140u8, 4u8, 203u8, 43u8, 152u8, 164u8,
            ],
            [
                198u8, 163u8, 119u8, 191u8, 196u8, 235u8, 18u8, 0u8, 36u8, 168u8, 172u8,
                8u8, 238u8, 242u8, 5u8, 190u8, 22u8, 184u8, 23u8, 2u8, 8u8, 18u8, 199u8,
                50u8, 35u8, 232u8, 29u8, 27u8, 219u8, 151u8, 8u8, 236u8,
            ],
            [
                218u8, 159u8, 167u8, 193u8, 176u8, 4u8, 2u8, 193u8, 125u8, 1u8, 97u8,
                178u8, 73u8, 177u8, 171u8, 139u8, 190u8, 192u8, 71u8, 197u8, 165u8, 34u8,
                7u8, 185u8, 193u8, 18u8, 222u8, 255u8, 216u8, 23u8, 3u8, 107u8,
            ],
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(UnorderedNonceInvalidation),
            ::core::stringify!(NonceInvalidation),
            ::core::stringify!(Lockdown),
            ::core::stringify!(Permit),
            ::core::stringify!(Approval),
        ];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <UnorderedNonceInvalidation as alloy_sol_types::SolEvent>::SIGNATURE,
            <NonceInvalidation as alloy_sol_types::SolEvent>::SIGNATURE,
            <Lockdown as alloy_sol_types::SolEvent>::SIGNATURE,
            <Permit as alloy_sol_types::SolEvent>::SIGNATURE,
            <Approval as alloy_sol_types::SolEvent>::SIGNATURE,
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
    impl alloy_sol_types::SolEventInterface for Permit2Events {
        const NAME: &'static str = "Permit2Events";
        const COUNT: usize = 5usize;
        fn decode_raw_log(
            topics: &[alloy_sol_types::Word],
            data: &[u8],
        ) -> alloy_sol_types::Result<Self> {
            match topics.first().copied() {
                Some(<Approval as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <Approval as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::Approval)
                }
                Some(<Lockdown as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <Lockdown as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::Lockdown)
                }
                Some(
                    <NonceInvalidation as alloy_sol_types::SolEvent>::SIGNATURE_HASH,
                ) => {
                    <NonceInvalidation as alloy_sol_types::SolEvent>::decode_raw_log(
                            topics,
                            data,
                        )
                        .map(Self::NonceInvalidation)
                }
                Some(<Permit as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <Permit as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::Permit)
                }
                Some(
                    <UnorderedNonceInvalidation as alloy_sol_types::SolEvent>::SIGNATURE_HASH,
                ) => {
                    <UnorderedNonceInvalidation as alloy_sol_types::SolEvent>::decode_raw_log(
                            topics,
                            data,
                        )
                        .map(Self::UnorderedNonceInvalidation)
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
    impl alloy_sol_types::private::IntoLogData for Permit2Events {
        fn to_log_data(&self) -> alloy_sol_types::private::LogData {
            match self {
                Self::Approval(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::Lockdown(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::NonceInvalidation(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::Permit(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::UnorderedNonceInvalidation(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
            }
        }
        fn into_log_data(self) -> alloy_sol_types::private::LogData {
            match self {
                Self::Approval(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::Lockdown(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::NonceInvalidation(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::Permit(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::UnorderedNonceInvalidation(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
            }
        }
    }
    use alloy_contract as alloy_contract;
    /**Creates a new wrapper around an on-chain [`Permit2`](self) contract instance.

See the [wrapper's documentation](`Permit2Instance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> Permit2Instance<P, N> {
        Permit2Instance::<P, N>::new(address, __provider)
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
    ) -> impl ::core::future::Future<
        Output = alloy_contract::Result<Permit2Instance<P, N>>,
    > {
        Permit2Instance::<P, N>::deploy(__provider)
    }
    /**Creates a `RawCallBuilder` for deploying this contract using the given `provider`
and constructor arguments, if any.

This is a simple wrapper around creating a `RawCallBuilder` with the data set to
the bytecode concatenated with the constructor's ABI-encoded arguments.*/
    #[inline]
    pub fn deploy_builder<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(__provider: P) -> alloy_contract::RawCallBuilder<P, N> {
        Permit2Instance::<P, N>::deploy_builder(__provider)
    }
    /**A [`Permit2`](self) instance.

Contains type-safe methods for interacting with an on-chain instance of the
[`Permit2`](self) contract located at a given `address`, using a given
provider `P`.

If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
documentation on how to provide it), the `deploy` and `deploy_builder` methods can
be used to deploy a new instance of the contract.

See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct Permit2Instance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for Permit2Instance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("Permit2Instance").field(&self.address).finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    > Permit2Instance<P, N> {
        /**Creates a new wrapper around an on-chain [`Permit2`](self) contract instance.

See the [wrapper's documentation](`Permit2Instance`) for more details.*/
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
        ) -> alloy_contract::Result<Permit2Instance<P, N>> {
            let call_builder = Self::deploy_builder(__provider);
            let contract_address = call_builder.deploy().await?;
            Ok(Self::new(contract_address, call_builder.provider))
        }
        /**Creates a `RawCallBuilder` for deploying this contract using the given `provider`
and constructor arguments, if any.

This is a simple wrapper around creating a `RawCallBuilder` with the data set to
the bytecode concatenated with the constructor's ABI-encoded arguments.*/
        #[inline]
        pub fn deploy_builder(__provider: P) -> alloy_contract::RawCallBuilder<P, N> {
            alloy_contract::RawCallBuilder::new_raw_deploy(
                __provider,
                ::core::clone::Clone::clone(&BYTECODE),
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
    impl<P: ::core::clone::Clone, N> Permit2Instance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned provider.
        #[inline]
        pub fn with_cloned_provider(self) -> Permit2Instance<P, N> {
            Permit2Instance {
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
    > Permit2Instance<P, N> {
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
        ///Creates a new call builder for the [`DOMAIN_SEPARATOR`] function.
        pub fn DOMAIN_SEPARATOR(
            &self,
        ) -> alloy_contract::SolCallBuilder<&P, DOMAIN_SEPARATORCall, N> {
            self.call_builder(&DOMAIN_SEPARATORCall)
        }
        ///Creates a new call builder for the [`allowance`] function.
        pub fn allowance(
            &self,
            _0: alloy_sol_types::private::Address,
            _1: alloy_sol_types::private::Address,
            _2: alloy_sol_types::private::Address,
        ) -> alloy_contract::SolCallBuilder<&P, allowanceCall, N> {
            self.call_builder(&allowanceCall { _0, _1, _2 })
        }
        ///Creates a new call builder for the [`approve`] function.
        pub fn approve(
            &self,
            token: alloy_sol_types::private::Address,
            spender: alloy_sol_types::private::Address,
            amount: alloy_sol_types::private::primitives::aliases::U160,
            expiration: alloy_sol_types::private::primitives::aliases::U48,
        ) -> alloy_contract::SolCallBuilder<&P, approveCall, N> {
            self.call_builder(
                &approveCall {
                    token,
                    spender,
                    amount,
                    expiration,
                },
            )
        }
        ///Creates a new call builder for the [`invalidateNonces`] function.
        pub fn invalidateNonces(
            &self,
            token: alloy_sol_types::private::Address,
            spender: alloy_sol_types::private::Address,
            newNonce: alloy_sol_types::private::primitives::aliases::U48,
        ) -> alloy_contract::SolCallBuilder<&P, invalidateNoncesCall, N> {
            self.call_builder(
                &invalidateNoncesCall {
                    token,
                    spender,
                    newNonce,
                },
            )
        }
        ///Creates a new call builder for the [`invalidateUnorderedNonces`] function.
        pub fn invalidateUnorderedNonces(
            &self,
            wordPos: alloy_sol_types::private::primitives::aliases::U256,
            mask: alloy_sol_types::private::primitives::aliases::U256,
        ) -> alloy_contract::SolCallBuilder<&P, invalidateUnorderedNoncesCall, N> {
            self.call_builder(
                &invalidateUnorderedNoncesCall {
                    wordPos,
                    mask,
                },
            )
        }
        ///Creates a new call builder for the [`lockdown`] function.
        pub fn lockdown(
            &self,
            approvals: alloy_sol_types::private::Vec<
                <IAllowanceTransfer::TokenSpenderPair as alloy_sol_types::SolType>::RustType,
            >,
        ) -> alloy_contract::SolCallBuilder<&P, lockdownCall, N> {
            self.call_builder(&lockdownCall { approvals })
        }
        ///Creates a new call builder for the [`nonceBitmap`] function.
        pub fn nonceBitmap(
            &self,
            _0: alloy_sol_types::private::Address,
            _1: alloy_sol_types::private::primitives::aliases::U256,
        ) -> alloy_contract::SolCallBuilder<&P, nonceBitmapCall, N> {
            self.call_builder(&nonceBitmapCall { _0, _1 })
        }
        ///Creates a new call builder for the [`permit`] function.
        pub fn permit(
            &self,
            owner: alloy_sol_types::private::Address,
            permitSingle: <IAllowanceTransfer::PermitSingle as alloy_sol_types::SolType>::RustType,
            signature: alloy_sol_types::private::Bytes,
        ) -> alloy_contract::SolCallBuilder<&P, permitCall, N> {
            self.call_builder(
                &permitCall {
                    owner,
                    permitSingle,
                    signature,
                },
            )
        }
        ///Creates a new call builder for the [`permitTransferFrom`] function.
        pub fn permitTransferFrom(
            &self,
            permit: <ISignatureTransfer::PermitTransferFrom as alloy_sol_types::SolType>::RustType,
            transferDetails: <ISignatureTransfer::SignatureTransferDetails as alloy_sol_types::SolType>::RustType,
            owner: alloy_sol_types::private::Address,
            signature: alloy_sol_types::private::Bytes,
        ) -> alloy_contract::SolCallBuilder<&P, permitTransferFromCall, N> {
            self.call_builder(
                &permitTransferFromCall {
                    permit,
                    transferDetails,
                    owner,
                    signature,
                },
            )
        }
        ///Creates a new call builder for the [`permitWitnessTransferFrom`] function.
        pub fn permitWitnessTransferFrom(
            &self,
            permit: <ISignatureTransfer::PermitBatchTransferFrom as alloy_sol_types::SolType>::RustType,
            transferDetails: alloy_sol_types::private::Vec<
                <ISignatureTransfer::SignatureTransferDetails as alloy_sol_types::SolType>::RustType,
            >,
            owner: alloy_sol_types::private::Address,
            witness: alloy_sol_types::private::FixedBytes<32>,
            witnessTypeString: alloy_sol_types::private::String,
            signature: alloy_sol_types::private::Bytes,
        ) -> alloy_contract::SolCallBuilder<&P, permitWitnessTransferFromCall, N> {
            self.call_builder(
                &permitWitnessTransferFromCall {
                    permit,
                    transferDetails,
                    owner,
                    witness,
                    witnessTypeString,
                    signature,
                },
            )
        }
        ///Creates a new call builder for the [`transferFrom`] function.
        pub fn transferFrom(
            &self,
            from: alloy_sol_types::private::Address,
            to: alloy_sol_types::private::Address,
            amount: alloy_sol_types::private::primitives::aliases::U160,
            token: alloy_sol_types::private::Address,
        ) -> alloy_contract::SolCallBuilder<&P, transferFromCall, N> {
            self.call_builder(
                &transferFromCall {
                    from,
                    to,
                    amount,
                    token,
                },
            )
        }
    }
    /// Event filters.
    impl<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    > Permit2Instance<P, N> {
        /// Creates a new event filter using this contract instance's provider and address.
        ///
        /// Note that the type can be any event, not just those defined in this contract.
        /// Prefer using the other methods for building type-safe event filters.
        pub fn event_filter<E: alloy_sol_types::SolEvent>(
            &self,
        ) -> alloy_contract::Event<&P, E, N> {
            alloy_contract::Event::new_sol(&self.provider, &self.address)
        }
        ///Creates a new event filter for the [`Approval`] event.
        pub fn Approval_filter(&self) -> alloy_contract::Event<&P, Approval, N> {
            self.event_filter::<Approval>()
        }
        ///Creates a new event filter for the [`Lockdown`] event.
        pub fn Lockdown_filter(&self) -> alloy_contract::Event<&P, Lockdown, N> {
            self.event_filter::<Lockdown>()
        }
        ///Creates a new event filter for the [`NonceInvalidation`] event.
        pub fn NonceInvalidation_filter(
            &self,
        ) -> alloy_contract::Event<&P, NonceInvalidation, N> {
            self.event_filter::<NonceInvalidation>()
        }
        ///Creates a new event filter for the [`Permit`] event.
        pub fn Permit_filter(&self) -> alloy_contract::Event<&P, Permit, N> {
            self.event_filter::<Permit>()
        }
        ///Creates a new event filter for the [`UnorderedNonceInvalidation`] event.
        pub fn UnorderedNonceInvalidation_filter(
            &self,
        ) -> alloy_contract::Event<&P, UnorderedNonceInvalidation, N> {
            self.event_filter::<UnorderedNonceInvalidation>()
        }
    }
}
pub type Instance = Permit2::Permit2Instance<::alloy_provider::DynProvider>;
use {
    std::{sync::LazyLock, collections::HashMap},
    anyhow::{Result, Context},
    alloy_primitives::{address, Address},
    alloy_provider::{Provider, DynProvider},
};
pub const fn deployment_info(chain_id: u64) -> Option<(Address, Option<u64>)> {
    match chain_id {
        137u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0x000000000022D473030F116dDEE9F6B43aC78BA3"
                ),
                Some(35701901u64),
            ))
        }
        57073u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0x000000000022D473030F116dDEE9F6B43aC78BA3"
                ),
                Some(0u64),
            ))
        }
        1u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0x000000000022D473030F116dDEE9F6B43aC78BA3"
                ),
                Some(15986406u64),
            ))
        }
        100u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0x000000000022D473030F116dDEE9F6B43aC78BA3"
                ),
                Some(27338672u64),
            ))
        }
        8453u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0x000000000022D473030F116dDEE9F6B43aC78BA3"
                ),
                Some(1425180u64),
            ))
        }
        56u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0x000000000022D473030F116dDEE9F6B43aC78BA3"
                ),
                Some(25343783u64),
            ))
        }
        10u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0x000000000022D473030F116dDEE9F6B43aC78BA3"
                ),
                Some(38854427u64),
            ))
        }
        43114u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0x000000000022D473030F116dDEE9F6B43aC78BA3"
                ),
                Some(28844415u64),
            ))
        }
        9745u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0x000000000022D473030F116dDEE9F6B43aC78BA3"
                ),
                Some(7808u64),
            ))
        }
        42161u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0x000000000022D473030F116dDEE9F6B43aC78BA3"
                ),
                Some(38692735u64),
            ))
        }
        11155111u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0x000000000022D473030F116dDEE9F6B43aC78BA3"
                ),
                Some(2356287u64),
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
