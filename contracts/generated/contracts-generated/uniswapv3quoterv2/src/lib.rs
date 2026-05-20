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
library IQuoterV2 {
    struct QuoteExactInputSingleParams { address tokenIn; address tokenOut; uint256 amountIn; uint24 fee; uint160 sqrtPriceLimitX96; }
    struct QuoteExactOutputSingleParams { address tokenIn; address tokenOut; uint256 amount; uint24 fee; uint160 sqrtPriceLimitX96; }
}
```*/
#[allow(
    non_camel_case_types,
    non_snake_case,
    clippy::pub_underscore_fields,
    clippy::style,
    clippy::empty_structs_with_brackets
)]
pub mod IQuoterV2 {
    use {super::*, alloy_sol_types};
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**```solidity
    struct QuoteExactInputSingleParams { address tokenIn; address tokenOut; uint256 amountIn; uint24 fee; uint160 sqrtPriceLimitX96; }
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct QuoteExactInputSingleParams {
        #[allow(missing_docs)]
        pub tokenIn: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub tokenOut: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub amountIn: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub fee: alloy_sol_types::private::primitives::aliases::U24,
        #[allow(missing_docs)]
        pub sqrtPriceLimitX96: alloy_sol_types::private::primitives::aliases::U160,
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
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<24>,
            alloy_sol_types::sol_data::Uint<160>,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::Address,
            alloy_sol_types::private::Address,
            alloy_sol_types::private::primitives::aliases::U256,
            alloy_sol_types::private::primitives::aliases::U24,
            alloy_sol_types::private::primitives::aliases::U160,
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
        impl ::core::convert::From<QuoteExactInputSingleParams> for UnderlyingRustTuple<'_> {
            fn from(value: QuoteExactInputSingleParams) -> Self {
                (
                    value.tokenIn,
                    value.tokenOut,
                    value.amountIn,
                    value.fee,
                    value.sqrtPriceLimitX96,
                )
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for QuoteExactInputSingleParams {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    tokenIn: tuple.0,
                    tokenOut: tuple.1,
                    amountIn: tuple.2,
                    fee: tuple.3,
                    sqrtPriceLimitX96: tuple.4,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for QuoteExactInputSingleParams {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for QuoteExactInputSingleParams {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.tokenIn,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.tokenOut,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.amountIn,
                    ),
                    <alloy_sol_types::sol_data::Uint<24> as alloy_sol_types::SolType>::tokenize(
                        &self.fee,
                    ),
                    <alloy_sol_types::sol_data::Uint<160> as alloy_sol_types::SolType>::tokenize(
                        &self.sqrtPriceLimitX96,
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
        impl alloy_sol_types::SolType for QuoteExactInputSingleParams {
            type RustType = Self;
            type Token<'a> = <UnderlyingSolTuple<'a> as alloy_sol_types::SolType>::Token<'a>;

            const ENCODED_SIZE: Option<usize> =
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::ENCODED_SIZE;
            const PACKED_ENCODED_SIZE: Option<usize> =
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::PACKED_ENCODED_SIZE;
            const SOL_NAME: &'static str = <Self as alloy_sol_types::SolStruct>::NAME;

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
        impl alloy_sol_types::SolStruct for QuoteExactInputSingleParams {
            const NAME: &'static str = "QuoteExactInputSingleParams";

            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "QuoteExactInputSingleParams(address tokenIn,address tokenOut,uint256 \
                     amountIn,uint24 fee,uint160 sqrtPriceLimitX96)",
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
                            &self.tokenIn,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.tokenOut,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.amountIn)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        24,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.fee)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        160,
                    > as alloy_sol_types::SolType>::eip712_data_word(
                            &self.sqrtPriceLimitX96,
                        )
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for QuoteExactInputSingleParams {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.tokenIn,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.tokenOut,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.amountIn,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        24,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(&rust.fee)
                    + <alloy_sol_types::sol_data::Uint<
                        160,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.sqrtPriceLimitX96,
                    )
            }

            #[inline]
            fn encode_topic_preimage(
                rust: &Self::RustType,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                out.reserve(<Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust));
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.tokenIn,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.tokenOut,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.amountIn,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    24,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(&rust.fee, out);
                <alloy_sol_types::sol_data::Uint<
                    160,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.sqrtPriceLimitX96,
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
    struct QuoteExactOutputSingleParams { address tokenIn; address tokenOut; uint256 amount; uint24 fee; uint160 sqrtPriceLimitX96; }
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct QuoteExactOutputSingleParams {
        #[allow(missing_docs)]
        pub tokenIn: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub tokenOut: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub amount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub fee: alloy_sol_types::private::primitives::aliases::U24,
        #[allow(missing_docs)]
        pub sqrtPriceLimitX96: alloy_sol_types::private::primitives::aliases::U160,
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
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<24>,
            alloy_sol_types::sol_data::Uint<160>,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::Address,
            alloy_sol_types::private::Address,
            alloy_sol_types::private::primitives::aliases::U256,
            alloy_sol_types::private::primitives::aliases::U24,
            alloy_sol_types::private::primitives::aliases::U160,
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
        impl ::core::convert::From<QuoteExactOutputSingleParams> for UnderlyingRustTuple<'_> {
            fn from(value: QuoteExactOutputSingleParams) -> Self {
                (
                    value.tokenIn,
                    value.tokenOut,
                    value.amount,
                    value.fee,
                    value.sqrtPriceLimitX96,
                )
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for QuoteExactOutputSingleParams {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    tokenIn: tuple.0,
                    tokenOut: tuple.1,
                    amount: tuple.2,
                    fee: tuple.3,
                    sqrtPriceLimitX96: tuple.4,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for QuoteExactOutputSingleParams {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for QuoteExactOutputSingleParams {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.tokenIn,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.tokenOut,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.amount,
                    ),
                    <alloy_sol_types::sol_data::Uint<24> as alloy_sol_types::SolType>::tokenize(
                        &self.fee,
                    ),
                    <alloy_sol_types::sol_data::Uint<160> as alloy_sol_types::SolType>::tokenize(
                        &self.sqrtPriceLimitX96,
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
        impl alloy_sol_types::SolType for QuoteExactOutputSingleParams {
            type RustType = Self;
            type Token<'a> = <UnderlyingSolTuple<'a> as alloy_sol_types::SolType>::Token<'a>;

            const ENCODED_SIZE: Option<usize> =
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::ENCODED_SIZE;
            const PACKED_ENCODED_SIZE: Option<usize> =
                <UnderlyingSolTuple<'_> as alloy_sol_types::SolType>::PACKED_ENCODED_SIZE;
            const SOL_NAME: &'static str = <Self as alloy_sol_types::SolStruct>::NAME;

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
        impl alloy_sol_types::SolStruct for QuoteExactOutputSingleParams {
            const NAME: &'static str = "QuoteExactOutputSingleParams";

            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "QuoteExactOutputSingleParams(address tokenIn,address tokenOut,uint256 \
                     amount,uint24 fee,uint160 sqrtPriceLimitX96)",
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
                            &self.tokenIn,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.tokenOut,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.amount)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        24,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.fee)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        160,
                    > as alloy_sol_types::SolType>::eip712_data_word(
                            &self.sqrtPriceLimitX96,
                        )
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for QuoteExactOutputSingleParams {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.tokenIn,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.tokenOut,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.amount,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        24,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(&rust.fee)
                    + <alloy_sol_types::sol_data::Uint<
                        160,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.sqrtPriceLimitX96,
                    )
            }

            #[inline]
            fn encode_topic_preimage(
                rust: &Self::RustType,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                out.reserve(<Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust));
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.tokenIn,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.tokenOut,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.amount,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    24,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(&rust.fee, out);
                <alloy_sol_types::sol_data::Uint<
                    160,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.sqrtPriceLimitX96,
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
    /**Creates a new wrapper around an on-chain [`IQuoterV2`](self) contract instance.

    See the [wrapper's documentation](`IQuoterV2Instance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> IQuoterV2Instance<P, N> {
        IQuoterV2Instance::<P, N>::new(address, __provider)
    }
    /**A [`IQuoterV2`](self) instance.

    Contains type-safe methods for interacting with an on-chain instance of the
    [`IQuoterV2`](self) contract located at a given `address`, using a given
    provider `P`.

    If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
    documentation on how to provide it), the `deploy` and `deploy_builder` methods can
    be used to deploy a new instance of the contract.

    See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct IQuoterV2Instance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for IQuoterV2Instance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("IQuoterV2Instance")
                .field(&self.address)
                .finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        IQuoterV2Instance<P, N>
    {
        /**Creates a new wrapper around an on-chain [`IQuoterV2`](self) contract instance.

        See the [wrapper's documentation](`IQuoterV2Instance`) for more details.*/
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
    impl<P: ::core::clone::Clone, N> IQuoterV2Instance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned
        /// provider.
        #[inline]
        pub fn with_cloned_provider(self) -> IQuoterV2Instance<P, N> {
            IQuoterV2Instance {
                address: self.address,
                provider: ::core::clone::Clone::clone(&self.provider),
                _network: ::core::marker::PhantomData,
            }
        }
    }
    /// Function calls.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        IQuoterV2Instance<P, N>
    {
        /// Creates a new call builder using this contract instance's provider
        /// and address.
        ///
        /// Note that the call can be any function call, not just those defined
        /// in this contract. Prefer using the other methods for
        /// building type-safe contract calls.
        pub fn call_builder<C: alloy_sol_types::SolCall>(
            &self,
            call: &C,
        ) -> alloy_contract::SolCallBuilder<&P, C, N> {
            alloy_contract::SolCallBuilder::new_sol(&self.provider, &self.address, call)
        }
    }
    /// Event filters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        IQuoterV2Instance<P, N>
    {
        /// Creates a new event filter using this contract instance's provider
        /// and address.
        ///
        /// Note that the type can be any event, not just those defined in this
        /// contract. Prefer using the other methods for building
        /// type-safe event filters.
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
library IQuoterV2 {
    struct QuoteExactInputSingleParams {
        address tokenIn;
        address tokenOut;
        uint256 amountIn;
        uint24 fee;
        uint160 sqrtPriceLimitX96;
    }
    struct QuoteExactOutputSingleParams {
        address tokenIn;
        address tokenOut;
        uint256 amount;
        uint24 fee;
        uint160 sqrtPriceLimitX96;
    }
}

interface UniswapV3QuoterV2 {
    constructor(address _factory, address _WETH9);

    function WETH9() external view returns (address);
    function factory() external view returns (address);
    function quoteExactInput(bytes memory path, uint256 amountIn) external returns (uint256 amountOut, uint160[] memory sqrtPriceX96AfterList, uint32[] memory initializedTicksCrossedList, uint256 gasEstimate);
    function quoteExactInputSingle(IQuoterV2.QuoteExactInputSingleParams memory params) external returns (uint256 amountOut, uint160 sqrtPriceX96After, uint32 initializedTicksCrossed, uint256 gasEstimate);
    function quoteExactOutput(bytes memory path, uint256 amountOut) external returns (uint256 amountIn, uint160[] memory sqrtPriceX96AfterList, uint32[] memory initializedTicksCrossedList, uint256 gasEstimate);
    function quoteExactOutputSingle(IQuoterV2.QuoteExactOutputSingleParams memory params) external returns (uint256 amountIn, uint160 sqrtPriceX96After, uint32 initializedTicksCrossed, uint256 gasEstimate);
}
```

...which was generated by the following JSON ABI:
```json
[
  {
    "type": "constructor",
    "inputs": [
      {
        "name": "_factory",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "_WETH9",
        "type": "address",
        "internalType": "address"
      }
    ],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "WETH9",
    "inputs": [],
    "outputs": [
      {
        "name": "",
        "type": "address",
        "internalType": "address"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "factory",
    "inputs": [],
    "outputs": [
      {
        "name": "",
        "type": "address",
        "internalType": "address"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "quoteExactInput",
    "inputs": [
      {
        "name": "path",
        "type": "bytes",
        "internalType": "bytes"
      },
      {
        "name": "amountIn",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "outputs": [
      {
        "name": "amountOut",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "sqrtPriceX96AfterList",
        "type": "uint160[]",
        "internalType": "uint160[]"
      },
      {
        "name": "initializedTicksCrossedList",
        "type": "uint32[]",
        "internalType": "uint32[]"
      },
      {
        "name": "gasEstimate",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "quoteExactInputSingle",
    "inputs": [
      {
        "name": "params",
        "type": "tuple",
        "internalType": "struct IQuoterV2.QuoteExactInputSingleParams",
        "components": [
          {
            "name": "tokenIn",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "tokenOut",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "amountIn",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "fee",
            "type": "uint24",
            "internalType": "uint24"
          },
          {
            "name": "sqrtPriceLimitX96",
            "type": "uint160",
            "internalType": "uint160"
          }
        ]
      }
    ],
    "outputs": [
      {
        "name": "amountOut",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "sqrtPriceX96After",
        "type": "uint160",
        "internalType": "uint160"
      },
      {
        "name": "initializedTicksCrossed",
        "type": "uint32",
        "internalType": "uint32"
      },
      {
        "name": "gasEstimate",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "quoteExactOutput",
    "inputs": [
      {
        "name": "path",
        "type": "bytes",
        "internalType": "bytes"
      },
      {
        "name": "amountOut",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "outputs": [
      {
        "name": "amountIn",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "sqrtPriceX96AfterList",
        "type": "uint160[]",
        "internalType": "uint160[]"
      },
      {
        "name": "initializedTicksCrossedList",
        "type": "uint32[]",
        "internalType": "uint32[]"
      },
      {
        "name": "gasEstimate",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "quoteExactOutputSingle",
    "inputs": [
      {
        "name": "params",
        "type": "tuple",
        "internalType": "struct IQuoterV2.QuoteExactOutputSingleParams",
        "components": [
          {
            "name": "tokenIn",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "tokenOut",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "amount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "fee",
            "type": "uint24",
            "internalType": "uint24"
          },
          {
            "name": "sqrtPriceLimitX96",
            "type": "uint160",
            "internalType": "uint160"
          }
        ]
      }
    ],
    "outputs": [
      {
        "name": "amountIn",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "sqrtPriceX96After",
        "type": "uint160",
        "internalType": "uint160"
      },
      {
        "name": "initializedTicksCrossed",
        "type": "uint32",
        "internalType": "uint32"
      },
      {
        "name": "gasEstimate",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "stateMutability": "nonpayable"
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
pub mod UniswapV3QuoterV2 {
    use {super::*, alloy_sol_types};
    /**Constructor`.
    ```solidity
    constructor(address _factory, address _WETH9);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct constructorCall {
        #[allow(missing_docs)]
        pub _factory: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub _WETH9: alloy_sol_types::private::Address,
    }
    const _: () = {
        use alloy_sol_types;
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
                    (value._factory, value._WETH9)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for constructorCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        _factory: tuple.0,
                        _WETH9: tuple.1,
                    }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolConstructor for constructorCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
            );
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
                        &self._factory,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self._WETH9,
                    ),
                )
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `WETH9()` and selector `0x4aa4a4fc`.
    ```solidity
    function WETH9() external view returns (address);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct WETH9Call;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`WETH9()`](WETH9Call)
    /// function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct WETH9Return {
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
            impl ::core::convert::From<WETH9Call> for UnderlyingRustTuple<'_> {
                fn from(value: WETH9Call) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for WETH9Call {
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
            impl ::core::convert::From<WETH9Return> for UnderlyingRustTuple<'_> {
                fn from(value: WETH9Return) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for WETH9Return {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for WETH9Call {
            type Parameters<'a> = ();
            type Return = alloy_sol_types::private::Address;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [74u8, 164u8, 164u8, 252u8];
            const SIGNATURE: &'static str = "WETH9()";

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
                        let r: WETH9Return = r.into();
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
                    let r: WETH9Return = r.into();
                    r._0
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `factory()` and selector `0xc45a0155`.
    ```solidity
    function factory() external view returns (address);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct factoryCall;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`factory()`](factoryCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct factoryReturn {
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
            impl ::core::convert::From<factoryCall> for UnderlyingRustTuple<'_> {
                fn from(value: factoryCall) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for factoryCall {
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
            impl ::core::convert::From<factoryReturn> for UnderlyingRustTuple<'_> {
                fn from(value: factoryReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for factoryReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for factoryCall {
            type Parameters<'a> = ();
            type Return = alloy_sol_types::private::Address;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [196u8, 90u8, 1u8, 85u8];
            const SIGNATURE: &'static str = "factory()";

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
                        let r: factoryReturn = r.into();
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
                    let r: factoryReturn = r.into();
                    r._0
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `quoteExactInput(bytes,uint256)` and selector `0xcdca1753`.
    ```solidity
    function quoteExactInput(bytes memory path, uint256 amountIn) external returns (uint256 amountOut, uint160[] memory sqrtPriceX96AfterList, uint32[] memory initializedTicksCrossedList, uint256 gasEstimate);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct quoteExactInputCall {
        #[allow(missing_docs)]
        pub path: alloy_sol_types::private::Bytes,
        #[allow(missing_docs)]
        pub amountIn: alloy_sol_types::private::primitives::aliases::U256,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`quoteExactInput(bytes,uint256)`](quoteExactInputCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct quoteExactInputReturn {
        #[allow(missing_docs)]
        pub amountOut: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub sqrtPriceX96AfterList:
            alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U160>,
        #[allow(missing_docs)]
        pub initializedTicksCrossedList: alloy_sol_types::private::Vec<u32>,
        #[allow(missing_docs)]
        pub gasEstimate: alloy_sol_types::private::primitives::aliases::U256,
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
                alloy_sol_types::sol_data::Bytes,
                alloy_sol_types::sol_data::Uint<256>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Bytes,
                alloy_sol_types::private::primitives::aliases::U256,
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
            impl ::core::convert::From<quoteExactInputCall> for UnderlyingRustTuple<'_> {
                fn from(value: quoteExactInputCall) -> Self {
                    (value.path, value.amountIn)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for quoteExactInputCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        path: tuple.0,
                        amountIn: tuple.1,
                    }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<160>>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<32>>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U160>,
                alloy_sol_types::private::Vec<u32>,
                alloy_sol_types::private::primitives::aliases::U256,
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
            impl ::core::convert::From<quoteExactInputReturn> for UnderlyingRustTuple<'_> {
                fn from(value: quoteExactInputReturn) -> Self {
                    (
                        value.amountOut,
                        value.sqrtPriceX96AfterList,
                        value.initializedTicksCrossedList,
                        value.gasEstimate,
                    )
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for quoteExactInputReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        amountOut: tuple.0,
                        sqrtPriceX96AfterList: tuple.1,
                        initializedTicksCrossedList: tuple.2,
                        gasEstimate: tuple.3,
                    }
                }
            }
        }
        impl quoteExactInputReturn {
            fn _tokenize(
                &self,
            ) -> <quoteExactInputCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                (
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.amountOut),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<160>,
                    > as alloy_sol_types::SolType>::tokenize(
                        &self.sqrtPriceX96AfterList,
                    ),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<32>,
                    > as alloy_sol_types::SolType>::tokenize(
                        &self.initializedTicksCrossedList,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.gasEstimate),
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for quoteExactInputCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Bytes,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type Return = quoteExactInputReturn;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<160>>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<32>>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [205u8, 202u8, 23u8, 83u8];
            const SIGNATURE: &'static str = "quoteExactInput(bytes,uint256)";

            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }

            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.path,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.amountIn,
                    ),
                )
            }

            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                quoteExactInputReturn::_tokenize(ret)
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
    /**Function with signature `quoteExactInputSingle((address,address,uint256,uint24,uint160))` and selector `0xc6a5026a`.
    ```solidity
    function quoteExactInputSingle(IQuoterV2.QuoteExactInputSingleParams memory params) external returns (uint256 amountOut, uint160 sqrtPriceX96After, uint32 initializedTicksCrossed, uint256 gasEstimate);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct quoteExactInputSingleCall {
        #[allow(missing_docs)]
        pub params: <IQuoterV2::QuoteExactInputSingleParams as alloy_sol_types::SolType>::RustType,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`quoteExactInputSingle((address,address,uint256,uint24,
    /// uint160))`](quoteExactInputSingleCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct quoteExactInputSingleReturn {
        #[allow(missing_docs)]
        pub amountOut: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub sqrtPriceX96After: alloy_sol_types::private::primitives::aliases::U160,
        #[allow(missing_docs)]
        pub initializedTicksCrossed: u32,
        #[allow(missing_docs)]
        pub gasEstimate: alloy_sol_types::private::primitives::aliases::U256,
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
            type UnderlyingSolTuple<'a> = (IQuoterV2::QuoteExactInputSingleParams,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> =
                (<IQuoterV2::QuoteExactInputSingleParams as alloy_sol_types::SolType>::RustType,);
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
            impl ::core::convert::From<quoteExactInputSingleCall> for UnderlyingRustTuple<'_> {
                fn from(value: quoteExactInputSingleCall) -> Self {
                    (value.params,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for quoteExactInputSingleCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { params: tuple.0 }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<160>,
                alloy_sol_types::sol_data::Uint<32>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::primitives::aliases::U160,
                u32,
                alloy_sol_types::private::primitives::aliases::U256,
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
            impl ::core::convert::From<quoteExactInputSingleReturn> for UnderlyingRustTuple<'_> {
                fn from(value: quoteExactInputSingleReturn) -> Self {
                    (
                        value.amountOut,
                        value.sqrtPriceX96After,
                        value.initializedTicksCrossed,
                        value.gasEstimate,
                    )
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for quoteExactInputSingleReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        amountOut: tuple.0,
                        sqrtPriceX96After: tuple.1,
                        initializedTicksCrossed: tuple.2,
                        gasEstimate: tuple.3,
                    }
                }
            }
        }
        impl quoteExactInputSingleReturn {
            fn _tokenize(
                &self,
            ) -> <quoteExactInputSingleCall as alloy_sol_types::SolCall>::ReturnToken<'_>
            {
                (
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.amountOut,
                    ),
                    <alloy_sol_types::sol_data::Uint<160> as alloy_sol_types::SolType>::tokenize(
                        &self.sqrtPriceX96After,
                    ),
                    <alloy_sol_types::sol_data::Uint<32> as alloy_sol_types::SolType>::tokenize(
                        &self.initializedTicksCrossed,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.gasEstimate,
                    ),
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for quoteExactInputSingleCall {
            type Parameters<'a> = (IQuoterV2::QuoteExactInputSingleParams,);
            type Return = quoteExactInputSingleReturn;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<160>,
                alloy_sol_types::sol_data::Uint<32>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [198u8, 165u8, 2u8, 106u8];
            const SIGNATURE: &'static str =
                "quoteExactInputSingle((address,address,uint256,uint24,uint160))";

            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }

            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <IQuoterV2::QuoteExactInputSingleParams as alloy_sol_types::SolType>::tokenize(
                        &self.params,
                    ),
                )
            }

            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                quoteExactInputSingleReturn::_tokenize(ret)
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
    /**Function with signature `quoteExactOutput(bytes,uint256)` and selector `0x2f80bb1d`.
    ```solidity
    function quoteExactOutput(bytes memory path, uint256 amountOut) external returns (uint256 amountIn, uint160[] memory sqrtPriceX96AfterList, uint32[] memory initializedTicksCrossedList, uint256 gasEstimate);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct quoteExactOutputCall {
        #[allow(missing_docs)]
        pub path: alloy_sol_types::private::Bytes,
        #[allow(missing_docs)]
        pub amountOut: alloy_sol_types::private::primitives::aliases::U256,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`quoteExactOutput(bytes,uint256)`](quoteExactOutputCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct quoteExactOutputReturn {
        #[allow(missing_docs)]
        pub amountIn: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub sqrtPriceX96AfterList:
            alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U160>,
        #[allow(missing_docs)]
        pub initializedTicksCrossedList: alloy_sol_types::private::Vec<u32>,
        #[allow(missing_docs)]
        pub gasEstimate: alloy_sol_types::private::primitives::aliases::U256,
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
                alloy_sol_types::sol_data::Bytes,
                alloy_sol_types::sol_data::Uint<256>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Bytes,
                alloy_sol_types::private::primitives::aliases::U256,
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
            impl ::core::convert::From<quoteExactOutputCall> for UnderlyingRustTuple<'_> {
                fn from(value: quoteExactOutputCall) -> Self {
                    (value.path, value.amountOut)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for quoteExactOutputCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        path: tuple.0,
                        amountOut: tuple.1,
                    }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<160>>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<32>>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U160>,
                alloy_sol_types::private::Vec<u32>,
                alloy_sol_types::private::primitives::aliases::U256,
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
            impl ::core::convert::From<quoteExactOutputReturn> for UnderlyingRustTuple<'_> {
                fn from(value: quoteExactOutputReturn) -> Self {
                    (
                        value.amountIn,
                        value.sqrtPriceX96AfterList,
                        value.initializedTicksCrossedList,
                        value.gasEstimate,
                    )
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for quoteExactOutputReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        amountIn: tuple.0,
                        sqrtPriceX96AfterList: tuple.1,
                        initializedTicksCrossedList: tuple.2,
                        gasEstimate: tuple.3,
                    }
                }
            }
        }
        impl quoteExactOutputReturn {
            fn _tokenize(
                &self,
            ) -> <quoteExactOutputCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                (
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.amountIn),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<160>,
                    > as alloy_sol_types::SolType>::tokenize(
                        &self.sqrtPriceX96AfterList,
                    ),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<32>,
                    > as alloy_sol_types::SolType>::tokenize(
                        &self.initializedTicksCrossedList,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.gasEstimate),
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for quoteExactOutputCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Bytes,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type Return = quoteExactOutputReturn;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<160>>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<32>>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [47u8, 128u8, 187u8, 29u8];
            const SIGNATURE: &'static str = "quoteExactOutput(bytes,uint256)";

            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }

            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.path,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.amountOut,
                    ),
                )
            }

            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                quoteExactOutputReturn::_tokenize(ret)
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
    /**Function with signature `quoteExactOutputSingle((address,address,uint256,uint24,uint160))` and selector `0xbd21704a`.
    ```solidity
    function quoteExactOutputSingle(IQuoterV2.QuoteExactOutputSingleParams memory params) external returns (uint256 amountIn, uint160 sqrtPriceX96After, uint32 initializedTicksCrossed, uint256 gasEstimate);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct quoteExactOutputSingleCall {
        #[allow(missing_docs)]
        pub params: <IQuoterV2::QuoteExactOutputSingleParams as alloy_sol_types::SolType>::RustType,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`quoteExactOutputSingle((address,address,uint256,uint24,
    /// uint160))`](quoteExactOutputSingleCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct quoteExactOutputSingleReturn {
        #[allow(missing_docs)]
        pub amountIn: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub sqrtPriceX96After: alloy_sol_types::private::primitives::aliases::U160,
        #[allow(missing_docs)]
        pub initializedTicksCrossed: u32,
        #[allow(missing_docs)]
        pub gasEstimate: alloy_sol_types::private::primitives::aliases::U256,
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
            type UnderlyingSolTuple<'a> = (IQuoterV2::QuoteExactOutputSingleParams,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> =
                (<IQuoterV2::QuoteExactOutputSingleParams as alloy_sol_types::SolType>::RustType,);
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
            impl ::core::convert::From<quoteExactOutputSingleCall> for UnderlyingRustTuple<'_> {
                fn from(value: quoteExactOutputSingleCall) -> Self {
                    (value.params,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for quoteExactOutputSingleCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { params: tuple.0 }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<160>,
                alloy_sol_types::sol_data::Uint<32>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::primitives::aliases::U160,
                u32,
                alloy_sol_types::private::primitives::aliases::U256,
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
            impl ::core::convert::From<quoteExactOutputSingleReturn> for UnderlyingRustTuple<'_> {
                fn from(value: quoteExactOutputSingleReturn) -> Self {
                    (
                        value.amountIn,
                        value.sqrtPriceX96After,
                        value.initializedTicksCrossed,
                        value.gasEstimate,
                    )
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for quoteExactOutputSingleReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        amountIn: tuple.0,
                        sqrtPriceX96After: tuple.1,
                        initializedTicksCrossed: tuple.2,
                        gasEstimate: tuple.3,
                    }
                }
            }
        }
        impl quoteExactOutputSingleReturn {
            fn _tokenize(
                &self,
            ) -> <quoteExactOutputSingleCall as alloy_sol_types::SolCall>::ReturnToken<'_>
            {
                (
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.amountIn,
                    ),
                    <alloy_sol_types::sol_data::Uint<160> as alloy_sol_types::SolType>::tokenize(
                        &self.sqrtPriceX96After,
                    ),
                    <alloy_sol_types::sol_data::Uint<32> as alloy_sol_types::SolType>::tokenize(
                        &self.initializedTicksCrossed,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.gasEstimate,
                    ),
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for quoteExactOutputSingleCall {
            type Parameters<'a> = (IQuoterV2::QuoteExactOutputSingleParams,);
            type Return = quoteExactOutputSingleReturn;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<160>,
                alloy_sol_types::sol_data::Uint<32>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [189u8, 33u8, 112u8, 74u8];
            const SIGNATURE: &'static str =
                "quoteExactOutputSingle((address,address,uint256,uint24,uint160))";

            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }

            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <IQuoterV2::QuoteExactOutputSingleParams as alloy_sol_types::SolType>::tokenize(
                        &self.params,
                    ),
                )
            }

            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                quoteExactOutputSingleReturn::_tokenize(ret)
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
    ///Container for all the [`UniswapV3QuoterV2`](self) function calls.
    #[derive(Clone)]
    pub enum UniswapV3QuoterV2Calls {
        #[allow(missing_docs)]
        WETH9(WETH9Call),
        #[allow(missing_docs)]
        factory(factoryCall),
        #[allow(missing_docs)]
        quoteExactInput(quoteExactInputCall),
        #[allow(missing_docs)]
        quoteExactInputSingle(quoteExactInputSingleCall),
        #[allow(missing_docs)]
        quoteExactOutput(quoteExactOutputCall),
        #[allow(missing_docs)]
        quoteExactOutputSingle(quoteExactOutputSingleCall),
    }
    impl UniswapV3QuoterV2Calls {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the
        /// variants. No guarantees are made about the order of the
        /// selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 4usize]] = &[
            [47u8, 128u8, 187u8, 29u8],
            [74u8, 164u8, 164u8, 252u8],
            [189u8, 33u8, 112u8, 74u8],
            [196u8, 90u8, 1u8, 85u8],
            [198u8, 165u8, 2u8, 106u8],
            [205u8, 202u8, 23u8, 83u8],
        ];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <quoteExactOutputCall as alloy_sol_types::SolCall>::SIGNATURE,
            <WETH9Call as alloy_sol_types::SolCall>::SIGNATURE,
            <quoteExactOutputSingleCall as alloy_sol_types::SolCall>::SIGNATURE,
            <factoryCall as alloy_sol_types::SolCall>::SIGNATURE,
            <quoteExactInputSingleCall as alloy_sol_types::SolCall>::SIGNATURE,
            <quoteExactInputCall as alloy_sol_types::SolCall>::SIGNATURE,
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(quoteExactOutput),
            ::core::stringify!(WETH9),
            ::core::stringify!(quoteExactOutputSingle),
            ::core::stringify!(factory),
            ::core::stringify!(quoteExactInputSingle),
            ::core::stringify!(quoteExactInput),
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
    impl alloy_sol_types::SolInterface for UniswapV3QuoterV2Calls {
        const COUNT: usize = 6usize;
        const MIN_DATA_LENGTH: usize = 0usize;
        const NAME: &'static str = "UniswapV3QuoterV2Calls";

        #[inline]
        fn selector(&self) -> [u8; 4] {
            match self {
                Self::WETH9(_) => <WETH9Call as alloy_sol_types::SolCall>::SELECTOR,
                Self::factory(_) => <factoryCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::quoteExactInput(_) => {
                    <quoteExactInputCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::quoteExactInputSingle(_) => {
                    <quoteExactInputSingleCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::quoteExactOutput(_) => {
                    <quoteExactOutputCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::quoteExactOutputSingle(_) => {
                    <quoteExactOutputSingleCall as alloy_sol_types::SolCall>::SELECTOR
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
        fn abi_decode_raw(selector: [u8; 4], data: &[u8]) -> alloy_sol_types::Result<Self> {
            static DECODE_SHIMS: &[fn(&[u8]) -> alloy_sol_types::Result<UniswapV3QuoterV2Calls>] =
                &[
                    {
                        fn quoteExactOutput(
                            data: &[u8],
                        ) -> alloy_sol_types::Result<UniswapV3QuoterV2Calls>
                        {
                            <quoteExactOutputCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                                .map(UniswapV3QuoterV2Calls::quoteExactOutput)
                        }
                        quoteExactOutput
                    },
                    {
                        fn WETH9(data: &[u8]) -> alloy_sol_types::Result<UniswapV3QuoterV2Calls> {
                            <WETH9Call as alloy_sol_types::SolCall>::abi_decode_raw(data)
                                .map(UniswapV3QuoterV2Calls::WETH9)
                        }
                        WETH9
                    },
                    {
                        fn quoteExactOutputSingle(
                            data: &[u8],
                        ) -> alloy_sol_types::Result<UniswapV3QuoterV2Calls>
                        {
                            <quoteExactOutputSingleCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(UniswapV3QuoterV2Calls::quoteExactOutputSingle)
                        }
                        quoteExactOutputSingle
                    },
                    {
                        fn factory(data: &[u8]) -> alloy_sol_types::Result<UniswapV3QuoterV2Calls> {
                            <factoryCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                                .map(UniswapV3QuoterV2Calls::factory)
                        }
                        factory
                    },
                    {
                        fn quoteExactInputSingle(
                            data: &[u8],
                        ) -> alloy_sol_types::Result<UniswapV3QuoterV2Calls>
                        {
                            <quoteExactInputSingleCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(UniswapV3QuoterV2Calls::quoteExactInputSingle)
                        }
                        quoteExactInputSingle
                    },
                    {
                        fn quoteExactInput(
                            data: &[u8],
                        ) -> alloy_sol_types::Result<UniswapV3QuoterV2Calls>
                        {
                            <quoteExactInputCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                                .map(UniswapV3QuoterV2Calls::quoteExactInput)
                        }
                        quoteExactInput
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
            ) -> alloy_sol_types::Result<
                UniswapV3QuoterV2Calls,
            >] = &[
                {
                    fn quoteExactOutput(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<UniswapV3QuoterV2Calls> {
                        <quoteExactOutputCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(UniswapV3QuoterV2Calls::quoteExactOutput)
                    }
                    quoteExactOutput
                },
                {
                    fn WETH9(data: &[u8]) -> alloy_sol_types::Result<UniswapV3QuoterV2Calls> {
                        <WETH9Call as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(UniswapV3QuoterV2Calls::WETH9)
                    }
                    WETH9
                },
                {
                    fn quoteExactOutputSingle(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<UniswapV3QuoterV2Calls> {
                        <quoteExactOutputSingleCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(UniswapV3QuoterV2Calls::quoteExactOutputSingle)
                    }
                    quoteExactOutputSingle
                },
                {
                    fn factory(data: &[u8]) -> alloy_sol_types::Result<UniswapV3QuoterV2Calls> {
                        <factoryCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(UniswapV3QuoterV2Calls::factory)
                    }
                    factory
                },
                {
                    fn quoteExactInputSingle(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<UniswapV3QuoterV2Calls> {
                        <quoteExactInputSingleCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(UniswapV3QuoterV2Calls::quoteExactInputSingle)
                    }
                    quoteExactInputSingle
                },
                {
                    fn quoteExactInput(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<UniswapV3QuoterV2Calls> {
                        <quoteExactInputCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(UniswapV3QuoterV2Calls::quoteExactInput)
                    }
                    quoteExactInput
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
                Self::WETH9(inner) => {
                    <WETH9Call as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::factory(inner) => {
                    <factoryCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::quoteExactInput(inner) => {
                    <quoteExactInputCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::quoteExactInputSingle(inner) => {
                    <quoteExactInputSingleCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::quoteExactOutput(inner) => {
                    <quoteExactOutputCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::quoteExactOutputSingle(inner) => {
                    <quoteExactOutputSingleCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
            }
        }

        #[inline]
        fn abi_encode_raw(&self, out: &mut alloy_sol_types::private::Vec<u8>) {
            match self {
                Self::WETH9(inner) => {
                    <WETH9Call as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::factory(inner) => {
                    <factoryCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::quoteExactInput(inner) => {
                    <quoteExactInputCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::quoteExactInputSingle(inner) => {
                    <quoteExactInputSingleCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner, out,
                    )
                }
                Self::quoteExactOutput(inner) => {
                    <quoteExactOutputCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::quoteExactOutputSingle(inner) => {
                    <quoteExactOutputSingleCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner, out,
                    )
                }
            }
        }
    }
    use alloy_contract;
    /**Creates a new wrapper around an on-chain [`UniswapV3QuoterV2`](self) contract instance.

    See the [wrapper's documentation](`UniswapV3QuoterV2Instance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> UniswapV3QuoterV2Instance<P, N> {
        UniswapV3QuoterV2Instance::<P, N>::new(address, __provider)
    }
    /**A [`UniswapV3QuoterV2`](self) instance.

    Contains type-safe methods for interacting with an on-chain instance of the
    [`UniswapV3QuoterV2`](self) contract located at a given `address`, using a given
    provider `P`.

    If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
    documentation on how to provide it), the `deploy` and `deploy_builder` methods can
    be used to deploy a new instance of the contract.

    See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct UniswapV3QuoterV2Instance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for UniswapV3QuoterV2Instance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("UniswapV3QuoterV2Instance")
                .field(&self.address)
                .finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        UniswapV3QuoterV2Instance<P, N>
    {
        /**Creates a new wrapper around an on-chain [`UniswapV3QuoterV2`](self) contract instance.

        See the [wrapper's documentation](`UniswapV3QuoterV2Instance`) for more details.*/
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
    impl<P: ::core::clone::Clone, N> UniswapV3QuoterV2Instance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned
        /// provider.
        #[inline]
        pub fn with_cloned_provider(self) -> UniswapV3QuoterV2Instance<P, N> {
            UniswapV3QuoterV2Instance {
                address: self.address,
                provider: ::core::clone::Clone::clone(&self.provider),
                _network: ::core::marker::PhantomData,
            }
        }
    }
    /// Function calls.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        UniswapV3QuoterV2Instance<P, N>
    {
        /// Creates a new call builder using this contract instance's provider
        /// and address.
        ///
        /// Note that the call can be any function call, not just those defined
        /// in this contract. Prefer using the other methods for
        /// building type-safe contract calls.
        pub fn call_builder<C: alloy_sol_types::SolCall>(
            &self,
            call: &C,
        ) -> alloy_contract::SolCallBuilder<&P, C, N> {
            alloy_contract::SolCallBuilder::new_sol(&self.provider, &self.address, call)
        }

        ///Creates a new call builder for the [`WETH9`] function.
        pub fn WETH9(&self) -> alloy_contract::SolCallBuilder<&P, WETH9Call, N> {
            self.call_builder(&WETH9Call)
        }

        ///Creates a new call builder for the [`factory`] function.
        pub fn factory(&self) -> alloy_contract::SolCallBuilder<&P, factoryCall, N> {
            self.call_builder(&factoryCall)
        }

        ///Creates a new call builder for the [`quoteExactInput`] function.
        pub fn quoteExactInput(
            &self,
            path: alloy_sol_types::private::Bytes,
            amountIn: alloy_sol_types::private::primitives::aliases::U256,
        ) -> alloy_contract::SolCallBuilder<&P, quoteExactInputCall, N> {
            self.call_builder(&quoteExactInputCall { path, amountIn })
        }

        ///Creates a new call builder for the [`quoteExactInputSingle`]
        /// function.
        pub fn quoteExactInputSingle(
            &self,
            params: <IQuoterV2::QuoteExactInputSingleParams as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, quoteExactInputSingleCall, N> {
            self.call_builder(&quoteExactInputSingleCall { params })
        }

        ///Creates a new call builder for the [`quoteExactOutput`] function.
        pub fn quoteExactOutput(
            &self,
            path: alloy_sol_types::private::Bytes,
            amountOut: alloy_sol_types::private::primitives::aliases::U256,
        ) -> alloy_contract::SolCallBuilder<&P, quoteExactOutputCall, N> {
            self.call_builder(&quoteExactOutputCall { path, amountOut })
        }

        ///Creates a new call builder for the [`quoteExactOutputSingle`]
        /// function.
        pub fn quoteExactOutputSingle(
            &self,
            params: <IQuoterV2::QuoteExactOutputSingleParams as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, quoteExactOutputSingleCall, N> {
            self.call_builder(&quoteExactOutputSingleCall { params })
        }
    }
    /// Event filters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        UniswapV3QuoterV2Instance<P, N>
    {
        /// Creates a new event filter using this contract instance's provider
        /// and address.
        ///
        /// Note that the type can be any event, not just those defined in this
        /// contract. Prefer using the other methods for building
        /// type-safe event filters.
        pub fn event_filter<E: alloy_sol_types::SolEvent>(
            &self,
        ) -> alloy_contract::Event<&P, E, N> {
            alloy_contract::Event::new_sol(&self.provider, &self.address)
        }
    }
}
pub type Instance = UniswapV3QuoterV2::UniswapV3QuoterV2Instance<::alloy_provider::DynProvider>;
use {
    alloy_primitives::{Address, address},
    alloy_provider::{DynProvider, Provider},
    anyhow::{Context, Result},
    std::{collections::HashMap, sync::LazyLock},
};
pub const fn deployment_info(chain_id: u64) -> Option<(Address, Option<u64>)> {
    match chain_id {
        1u64 => Some((
            ::alloy_primitives::address!("0x61fFE014bA17989E743c5F6cB21bF9697530B21e"),
            None,
        )),
        10u64 => Some((
            ::alloy_primitives::address!("0x61fFE014bA17989E743c5F6cB21bF9697530B21e"),
            None,
        )),
        56u64 => Some((
            ::alloy_primitives::address!("0x78D78E420Da98ad378D7799bE8f4AF69033EB077"),
            None,
        )),
        100u64 => Some((
            ::alloy_primitives::address!("0x7E9cB3499A6cee3baBe5c8a3D328EA7FD36578f4"),
            None,
        )),
        137u64 => Some((
            ::alloy_primitives::address!("0x61fFE014bA17989E743c5F6cB21bF9697530B21e"),
            None,
        )),
        8453u64 => Some((
            ::alloy_primitives::address!("0x3d4e44Eb1374240CE5F1B871ab261CD16335B76a"),
            None,
        )),
        9745u64 => Some((
            ::alloy_primitives::address!("0xaa52bB8110fE38D0d2d2AF0B85C3A3eE622CA455"),
            None,
        )),
        42161u64 => Some((
            ::alloy_primitives::address!("0x61fFE014bA17989E743c5F6cB21bF9697530B21e"),
            None,
        )),
        43114u64 => Some((
            ::alloy_primitives::address!("0xbe0F5544EC67e9B3b2D979aaA43f18Fd87E6257F"),
            None,
        )),
        57073u64 => Some((
            ::alloy_primitives::address!("0x96b572D2d880cf2Fa2563651BD23ADE6f5516652"),
            None,
        )),
        59144u64 => Some((
            ::alloy_primitives::address!("0x42bE4D6527829FeFA1493e1fb9F3676d2425C3C1"),
            None,
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
