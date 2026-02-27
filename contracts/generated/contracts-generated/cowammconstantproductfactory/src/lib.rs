#![allow(unused_imports, unused_attributes, clippy::all, rustdoc::all, non_snake_case)]
//! Auto-generated contract bindings. Do not edit.
///Module containing a contract's types and functions.
/**

```solidity
library GPv2Order {
    struct Data { address sellToken; address buyToken; address receiver; uint256 sellAmount; uint256 buyAmount; uint32 validTo; bytes32 appData; uint256 feeAmount; bytes32 kind; bool partiallyFillable; bytes32 sellTokenBalance; bytes32 buyTokenBalance; }
}
```*/
#[allow(
    non_camel_case_types,
    non_snake_case,
    clippy::pub_underscore_fields,
    clippy::style,
    clippy::empty_structs_with_brackets
)]
pub mod GPv2Order {
    use super::*;
    use alloy_sol_types as alloy_sol_types;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**```solidity
struct Data { address sellToken; address buyToken; address receiver; uint256 sellAmount; uint256 buyAmount; uint32 validTo; bytes32 appData; uint256 feeAmount; bytes32 kind; bool partiallyFillable; bytes32 sellTokenBalance; bytes32 buyTokenBalance; }
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct Data {
        #[allow(missing_docs)]
        pub sellToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub buyToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub receiver: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub sellAmount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub buyAmount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub validTo: u32,
        #[allow(missing_docs)]
        pub appData: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub feeAmount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub kind: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub partiallyFillable: bool,
        #[allow(missing_docs)]
        pub sellTokenBalance: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub buyTokenBalance: alloy_sol_types::private::FixedBytes<32>,
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
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<32>,
            alloy_sol_types::sol_data::FixedBytes<32>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::FixedBytes<32>,
            alloy_sol_types::sol_data::Bool,
            alloy_sol_types::sol_data::FixedBytes<32>,
            alloy_sol_types::sol_data::FixedBytes<32>,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::Address,
            alloy_sol_types::private::Address,
            alloy_sol_types::private::Address,
            alloy_sol_types::private::primitives::aliases::U256,
            alloy_sol_types::private::primitives::aliases::U256,
            u32,
            alloy_sol_types::private::FixedBytes<32>,
            alloy_sol_types::private::primitives::aliases::U256,
            alloy_sol_types::private::FixedBytes<32>,
            bool,
            alloy_sol_types::private::FixedBytes<32>,
            alloy_sol_types::private::FixedBytes<32>,
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
        impl ::core::convert::From<Data> for UnderlyingRustTuple<'_> {
            fn from(value: Data) -> Self {
                (
                    value.sellToken,
                    value.buyToken,
                    value.receiver,
                    value.sellAmount,
                    value.buyAmount,
                    value.validTo,
                    value.appData,
                    value.feeAmount,
                    value.kind,
                    value.partiallyFillable,
                    value.sellTokenBalance,
                    value.buyTokenBalance,
                )
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for Data {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    sellToken: tuple.0,
                    buyToken: tuple.1,
                    receiver: tuple.2,
                    sellAmount: tuple.3,
                    buyAmount: tuple.4,
                    validTo: tuple.5,
                    appData: tuple.6,
                    feeAmount: tuple.7,
                    kind: tuple.8,
                    partiallyFillable: tuple.9,
                    sellTokenBalance: tuple.10,
                    buyTokenBalance: tuple.11,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for Data {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for Data {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.sellToken,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.buyToken,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.receiver,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.sellAmount),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.buyAmount),
                    <alloy_sol_types::sol_data::Uint<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self.validTo),
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self.appData),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.feeAmount),
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self.kind),
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::tokenize(
                        &self.partiallyFillable,
                    ),
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self.sellTokenBalance),
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self.buyTokenBalance),
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
        impl alloy_sol_types::SolType for Data {
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
        impl alloy_sol_types::SolStruct for Data {
            const NAME: &'static str = "Data";
            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "Data(address sellToken,address buyToken,address receiver,uint256 sellAmount,uint256 buyAmount,uint32 validTo,bytes32 appData,uint256 feeAmount,bytes32 kind,bool partiallyFillable,bytes32 sellTokenBalance,bytes32 buyTokenBalance)",
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
                            &self.sellToken,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.buyToken,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.receiver,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.sellAmount)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.buyAmount)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        32,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.validTo)
                        .0,
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.appData)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.feeAmount)
                        .0,
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.kind)
                        .0,
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::eip712_data_word(
                            &self.partiallyFillable,
                        )
                        .0,
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::eip712_data_word(
                            &self.sellTokenBalance,
                        )
                        .0,
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::eip712_data_word(
                            &self.buyTokenBalance,
                        )
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for Data {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.sellToken,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.buyToken,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.receiver,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.sellAmount,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.buyAmount,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        32,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.validTo,
                    )
                    + <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.appData,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.feeAmount,
                    )
                    + <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(&rust.kind)
                    + <alloy_sol_types::sol_data::Bool as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.partiallyFillable,
                    )
                    + <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.sellTokenBalance,
                    )
                    + <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.buyTokenBalance,
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
                    &rust.sellToken,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.buyToken,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.receiver,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.sellAmount,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.buyAmount,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    32,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.validTo,
                    out,
                );
                <alloy_sol_types::sol_data::FixedBytes<
                    32,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.appData,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.feeAmount,
                    out,
                );
                <alloy_sol_types::sol_data::FixedBytes<
                    32,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.kind,
                    out,
                );
                <alloy_sol_types::sol_data::Bool as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.partiallyFillable,
                    out,
                );
                <alloy_sol_types::sol_data::FixedBytes<
                    32,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.sellTokenBalance,
                    out,
                );
                <alloy_sol_types::sol_data::FixedBytes<
                    32,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.buyTokenBalance,
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
    /**Creates a new wrapper around an on-chain [`GPv2Order`](self) contract instance.

See the [wrapper's documentation](`GPv2OrderInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> GPv2OrderInstance<P, N> {
        GPv2OrderInstance::<P, N>::new(address, __provider)
    }
    /**A [`GPv2Order`](self) instance.

Contains type-safe methods for interacting with an on-chain instance of the
[`GPv2Order`](self) contract located at a given `address`, using a given
provider `P`.

If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
documentation on how to provide it), the `deploy` and `deploy_builder` methods can
be used to deploy a new instance of the contract.

See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct GPv2OrderInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for GPv2OrderInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("GPv2OrderInstance").field(&self.address).finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    > GPv2OrderInstance<P, N> {
        /**Creates a new wrapper around an on-chain [`GPv2Order`](self) contract instance.

See the [wrapper's documentation](`GPv2OrderInstance`) for more details.*/
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
    impl<P: ::core::clone::Clone, N> GPv2OrderInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned provider.
        #[inline]
        pub fn with_cloned_provider(self) -> GPv2OrderInstance<P, N> {
            GPv2OrderInstance {
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
    > GPv2OrderInstance<P, N> {
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
    > GPv2OrderInstance<P, N> {
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
library IConditionalOrder {
    struct ConditionalOrderParams { address handler; bytes32 salt; bytes staticInput; }
}
```*/
#[allow(
    non_camel_case_types,
    non_snake_case,
    clippy::pub_underscore_fields,
    clippy::style,
    clippy::empty_structs_with_brackets
)]
pub mod IConditionalOrder {
    use super::*;
    use alloy_sol_types as alloy_sol_types;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**```solidity
struct ConditionalOrderParams { address handler; bytes32 salt; bytes staticInput; }
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct ConditionalOrderParams {
        #[allow(missing_docs)]
        pub handler: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub salt: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub staticInput: alloy_sol_types::private::Bytes,
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
            alloy_sol_types::sol_data::FixedBytes<32>,
            alloy_sol_types::sol_data::Bytes,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::Address,
            alloy_sol_types::private::FixedBytes<32>,
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
        impl ::core::convert::From<ConditionalOrderParams> for UnderlyingRustTuple<'_> {
            fn from(value: ConditionalOrderParams) -> Self {
                (value.handler, value.salt, value.staticInput)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for ConditionalOrderParams {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    handler: tuple.0,
                    salt: tuple.1,
                    staticInput: tuple.2,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for ConditionalOrderParams {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for ConditionalOrderParams {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.handler,
                    ),
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self.salt),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.staticInput,
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
        impl alloy_sol_types::SolType for ConditionalOrderParams {
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
        impl alloy_sol_types::SolStruct for ConditionalOrderParams {
            const NAME: &'static str = "ConditionalOrderParams";
            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "ConditionalOrderParams(address handler,bytes32 salt,bytes staticInput)",
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
                            &self.handler,
                        )
                        .0,
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.salt)
                        .0,
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::eip712_data_word(
                            &self.staticInput,
                        )
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for ConditionalOrderParams {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.handler,
                    )
                    + <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(&rust.salt)
                    + <alloy_sol_types::sol_data::Bytes as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.staticInput,
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
                    &rust.handler,
                    out,
                );
                <alloy_sol_types::sol_data::FixedBytes<
                    32,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.salt,
                    out,
                );
                <alloy_sol_types::sol_data::Bytes as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.staticInput,
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
    /**Creates a new wrapper around an on-chain [`IConditionalOrder`](self) contract instance.

See the [wrapper's documentation](`IConditionalOrderInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> IConditionalOrderInstance<P, N> {
        IConditionalOrderInstance::<P, N>::new(address, __provider)
    }
    /**A [`IConditionalOrder`](self) instance.

Contains type-safe methods for interacting with an on-chain instance of the
[`IConditionalOrder`](self) contract located at a given `address`, using a given
provider `P`.

If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
documentation on how to provide it), the `deploy` and `deploy_builder` methods can
be used to deploy a new instance of the contract.

See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct IConditionalOrderInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for IConditionalOrderInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("IConditionalOrderInstance").field(&self.address).finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    > IConditionalOrderInstance<P, N> {
        /**Creates a new wrapper around an on-chain [`IConditionalOrder`](self) contract instance.

See the [wrapper's documentation](`IConditionalOrderInstance`) for more details.*/
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
    impl<P: ::core::clone::Clone, N> IConditionalOrderInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned provider.
        #[inline]
        pub fn with_cloned_provider(self) -> IConditionalOrderInstance<P, N> {
            IConditionalOrderInstance {
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
    > IConditionalOrderInstance<P, N> {
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
    > IConditionalOrderInstance<P, N> {
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
library GPv2Order {
    struct Data {
        address sellToken;
        address buyToken;
        address receiver;
        uint256 sellAmount;
        uint256 buyAmount;
        uint32 validTo;
        bytes32 appData;
        uint256 feeAmount;
        bytes32 kind;
        bool partiallyFillable;
        bytes32 sellTokenBalance;
        bytes32 buyTokenBalance;
    }
}

library IConditionalOrder {
    struct ConditionalOrderParams {
        address handler;
        bytes32 salt;
        bytes staticInput;
    }
}

interface CowAmmConstantProductFactory {
    error OnlyOwnerCanCall(address owner);
    error OrderNotValid(string);

    event ConditionalOrderCreated(address indexed owner, IConditionalOrder.ConditionalOrderParams params);
    event Deployed(address indexed amm, address indexed owner, address token0, address token1);
    event TradingDisabled(address indexed amm);

    constructor(address _settler);

    function ammDeterministicAddress(address ammOwner, address token0, address token1) external view returns (address);
    function create(address token0, uint256 amount0, address token1, uint256 amount1, uint256 minTradedToken0, address priceOracle, bytes memory priceOracleData, bytes32 appData) external returns (address amm);
    function deposit(address amm, uint256 amount0, uint256 amount1) external;
    function disableTrading(address amm) external;
    function getTradeableOrderWithSignature(address amm, IConditionalOrder.ConditionalOrderParams memory params, bytes memory, bytes32[] memory) external view returns (GPv2Order.Data memory order, bytes memory signature);
    function owner(address) external view returns (address);
    function settler() external view returns (address);
    function updateParameters(address amm, uint256 minTradedToken0, address priceOracle, bytes memory priceOracleData, bytes32 appData) external;
    function withdraw(address amm, uint256 amount0, uint256 amount1) external;
}
```

...which was generated by the following JSON ABI:
```json
[
  {
    "type": "constructor",
    "inputs": [
      {
        "name": "_settler",
        "type": "address",
        "internalType": "contract ISettlement"
      }
    ],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "ammDeterministicAddress",
    "inputs": [
      {
        "name": "ammOwner",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "token0",
        "type": "address",
        "internalType": "contract IERC20"
      },
      {
        "name": "token1",
        "type": "address",
        "internalType": "contract IERC20"
      }
    ],
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
    "name": "create",
    "inputs": [
      {
        "name": "token0",
        "type": "address",
        "internalType": "contract IERC20"
      },
      {
        "name": "amount0",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "token1",
        "type": "address",
        "internalType": "contract IERC20"
      },
      {
        "name": "amount1",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "minTradedToken0",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "priceOracle",
        "type": "address",
        "internalType": "contract IPriceOracle"
      },
      {
        "name": "priceOracleData",
        "type": "bytes",
        "internalType": "bytes"
      },
      {
        "name": "appData",
        "type": "bytes32",
        "internalType": "bytes32"
      }
    ],
    "outputs": [
      {
        "name": "amm",
        "type": "address",
        "internalType": "contract ConstantProduct"
      }
    ],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "deposit",
    "inputs": [
      {
        "name": "amm",
        "type": "address",
        "internalType": "contract ConstantProduct"
      },
      {
        "name": "amount0",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "amount1",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "disableTrading",
    "inputs": [
      {
        "name": "amm",
        "type": "address",
        "internalType": "contract ConstantProduct"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "getTradeableOrderWithSignature",
    "inputs": [
      {
        "name": "amm",
        "type": "address",
        "internalType": "contract ConstantProduct"
      },
      {
        "name": "params",
        "type": "tuple",
        "internalType": "struct IConditionalOrder.ConditionalOrderParams",
        "components": [
          {
            "name": "handler",
            "type": "address",
            "internalType": "contract IConditionalOrder"
          },
          {
            "name": "salt",
            "type": "bytes32",
            "internalType": "bytes32"
          },
          {
            "name": "staticInput",
            "type": "bytes",
            "internalType": "bytes"
          }
        ]
      },
      {
        "name": "",
        "type": "bytes",
        "internalType": "bytes"
      },
      {
        "name": "",
        "type": "bytes32[]",
        "internalType": "bytes32[]"
      }
    ],
    "outputs": [
      {
        "name": "order",
        "type": "tuple",
        "internalType": "struct GPv2Order.Data",
        "components": [
          {
            "name": "sellToken",
            "type": "address",
            "internalType": "contract IERC20"
          },
          {
            "name": "buyToken",
            "type": "address",
            "internalType": "contract IERC20"
          },
          {
            "name": "receiver",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "sellAmount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "buyAmount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "validTo",
            "type": "uint32",
            "internalType": "uint32"
          },
          {
            "name": "appData",
            "type": "bytes32",
            "internalType": "bytes32"
          },
          {
            "name": "feeAmount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "kind",
            "type": "bytes32",
            "internalType": "bytes32"
          },
          {
            "name": "partiallyFillable",
            "type": "bool",
            "internalType": "bool"
          },
          {
            "name": "sellTokenBalance",
            "type": "bytes32",
            "internalType": "bytes32"
          },
          {
            "name": "buyTokenBalance",
            "type": "bytes32",
            "internalType": "bytes32"
          }
        ]
      },
      {
        "name": "signature",
        "type": "bytes",
        "internalType": "bytes"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "owner",
    "inputs": [
      {
        "name": "",
        "type": "address",
        "internalType": "contract ConstantProduct"
      }
    ],
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
    "name": "settler",
    "inputs": [],
    "outputs": [
      {
        "name": "",
        "type": "address",
        "internalType": "contract ISettlement"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "updateParameters",
    "inputs": [
      {
        "name": "amm",
        "type": "address",
        "internalType": "contract ConstantProduct"
      },
      {
        "name": "minTradedToken0",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "priceOracle",
        "type": "address",
        "internalType": "contract IPriceOracle"
      },
      {
        "name": "priceOracleData",
        "type": "bytes",
        "internalType": "bytes"
      },
      {
        "name": "appData",
        "type": "bytes32",
        "internalType": "bytes32"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "withdraw",
    "inputs": [
      {
        "name": "amm",
        "type": "address",
        "internalType": "contract ConstantProduct"
      },
      {
        "name": "amount0",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "amount1",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "event",
    "name": "ConditionalOrderCreated",
    "inputs": [
      {
        "name": "owner",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "params",
        "type": "tuple",
        "indexed": false,
        "internalType": "struct IConditionalOrder.ConditionalOrderParams",
        "components": [
          {
            "name": "handler",
            "type": "address",
            "internalType": "contract IConditionalOrder"
          },
          {
            "name": "salt",
            "type": "bytes32",
            "internalType": "bytes32"
          },
          {
            "name": "staticInput",
            "type": "bytes",
            "internalType": "bytes"
          }
        ]
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "Deployed",
    "inputs": [
      {
        "name": "amm",
        "type": "address",
        "indexed": true,
        "internalType": "contract ConstantProduct"
      },
      {
        "name": "owner",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "token0",
        "type": "address",
        "indexed": false,
        "internalType": "contract IERC20"
      },
      {
        "name": "token1",
        "type": "address",
        "indexed": false,
        "internalType": "contract IERC20"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "TradingDisabled",
    "inputs": [
      {
        "name": "amm",
        "type": "address",
        "indexed": true,
        "internalType": "contract ConstantProduct"
      }
    ],
    "anonymous": false
  },
  {
    "type": "error",
    "name": "OnlyOwnerCanCall",
    "inputs": [
      {
        "name": "owner",
        "type": "address",
        "internalType": "address"
      }
    ]
  },
  {
    "type": "error",
    "name": "OrderNotValid",
    "inputs": [
      {
        "name": "",
        "type": "string",
        "internalType": "string"
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
pub mod CowAmmConstantProductFactory {
    use super::*;
    use alloy_sol_types as alloy_sol_types;
    /// The creation / init bytecode of the contract.
    ///
    /// ```text
    ///0x60a0604052348015600e575f80fd5b506040516141b33803806141b3833981016040819052602b91603b565b6001600160a01b03166080526066565b5f60208284031215604a575f80fd5b81516001600160a01b0381168114605f575f80fd5b9392505050565b60805161412761008c5f395f8181610189015281816102a801526108c401526141275ff3fe608060405234801561000f575f80fd5b506004361061009f575f3560e01c806337ebdf5011610072578063666e1b3911610058578063666e1b391461014f578063ab221a7614610184578063b5c5f672146101ab575f80fd5b806337ebdf50146101295780635b5d9ee61461013c575f80fd5b80630efe6a8b146100a357806322b155c6146100b857806326e0a196146100f55780632791056514610116575b5f80fd5b6100b66100b13660046111ea565b6101be565b005b6100cb6100c6366004611261565b6102a3565b60405173ffffffffffffffffffffffffffffffffffffffff90911681526020015b60405180910390f35b6101086101033660046112fb565b61045f565b6040516100ec9291906114fd565b6100b6610124366004611527565b610797565b6100cb610137366004611549565b61082f565b6100b661014a366004611591565b610a01565b6100cb61015d366004611527565b5f6020819052908152604090205473ffffffffffffffffffffffffffffffffffffffff1681565b6100cb7f000000000000000000000000000000000000000000000000000000000000000081565b6100b66101b93660046111ea565b610b17565b61024f3384848673ffffffffffffffffffffffffffffffffffffffff16630dfe16816040518163ffffffff1660e01b8152600401602060405180830381865afa15801561020d573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906102319190611617565b73ffffffffffffffffffffffffffffffffffffffff16929190610c46565b61029e3384838673ffffffffffffffffffffffffffffffffffffffff1663d21220a76040518163ffffffff1660e01b8152600401602060405180830381865afa15801561020d573d5f803e3d5ffd5b505050565b5f33807f00000000000000000000000000000000000000000000000000000000000000008c8b6040516102d5906111b9565b73ffffffffffffffffffffffffffffffffffffffff9384168152918316602083015290911660408201526060018190604051809103905ff590508015801561031f573d5f803e3d5ffd5b506040805173ffffffffffffffffffffffffffffffffffffffff8e811682528c81166020830152929450828416928516917f6707255b2c5ca81220b2f3e408a269cb83baa6aa7e5e37aa1756883a6cdf06f1910160405180910390a373ffffffffffffffffffffffffffffffffffffffff8281165f90815260208190526040902080547fffffffffffffffffffffffff0000000000000000000000000000000000000000169183169190911790556103d8828b8a6101be565b5f60405180608001604052808981526020018873ffffffffffffffffffffffffffffffffffffffff16815260200187878080601f0160208091040260200160405190810160405280939291908181526020018383808284375f9201919091525050509082525060200185905290506104508382610cdb565b50509998505050505050505050565b60408051610180810182525f80825260208201819052918101829052606081018290526080810182905260a0810182905260c0810182905260e081018290526101008101829052610120810182905261014081018290526101608101919091526060306104cf6020890189611527565b73ffffffffffffffffffffffffffffffffffffffff1614610551576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601a60248201527f63616e206f6e6c792068616e646c65206f776e206f726465727300000000000060448201526064015b60405180910390fd5b5f61055f6040890189611632565b81019061056c919061175c565b90508873ffffffffffffffffffffffffffffffffffffffff1663eec50b976040518163ffffffff1660e01b8152600401602060405180830381865afa1580156105b7573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906105db919061185a565b6040517fb09aaaca00000000000000000000000000000000000000000000000000000000815273ffffffffffffffffffffffffffffffffffffffff8b169063b09aaaca9061062d9085906004016118c3565b602060405180830381865afa158015610648573d5f803e3d5ffd5b505050506040513d601f19601f8201168201806040525081019061066c919061185a565b146106d3576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601a60248201527f696e76616c69642074726164696e6720706172616d65746572730000000000006044820152606401610548565b6040517fe3e6f5b200000000000000000000000000000000000000000000000000000000815273ffffffffffffffffffffffffffffffffffffffff8a169063e3e6f5b2906107259084906004016118c3565b61018060405180830381865afa158015610741573d5f803e3d5ffd5b505050506040513d601f19601f8201168201806040525081019061076591906118f7565b9250828160405160200161077a9291906119b3565b604051602081830303815290604052915050965096945050505050565b73ffffffffffffffffffffffffffffffffffffffff8082165f9081526020819052604090205482911633146108225773ffffffffffffffffffffffffffffffffffffffff8181165f90815260208190526040908190205490517f68bafff800000000000000000000000000000000000000000000000000000000815291166004820152602401610548565b61082b82610e05565b5050565b5f807fff000000000000000000000000000000000000000000000000000000000000003073ffffffffffffffffffffffffffffffffffffffff8716604051610879602082016111b9565b8181037fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe09081018352601f90910116604081815273ffffffffffffffffffffffffffffffffffffffff7f000000000000000000000000000000000000000000000000000000000000000081166020840152808b169183019190915288166060820152608001604080517fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe08184030181529082905261093a92916020016119eb565b604051602081830303815290604052805190602001206040516020016109c294939291907fff0000000000000000000000000000000000000000000000000000000000000094909416845260609290921b7fffffffffffffffffffffffffffffffffffffffff0000000000000000000000001660018401526015830152603582015260550190565b604080518083037fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0018152919052805160209091012095945050505050565b73ffffffffffffffffffffffffffffffffffffffff8087165f908152602081905260409020548791163314610a8c5773ffffffffffffffffffffffffffffffffffffffff8181165f90815260208190526040908190205490517f68bafff800000000000000000000000000000000000000000000000000000000815291166004820152602401610548565b5f60405180608001604052808881526020018773ffffffffffffffffffffffffffffffffffffffff16815260200186868080601f0160208091040260200160405190810160405280939291908181526020018383808284375f920191909152505050908252506020018490529050610b0388610e05565b610b0d8882610cdb565b5050505050505050565b73ffffffffffffffffffffffffffffffffffffffff8084165f908152602081905260409020548491163314610ba25773ffffffffffffffffffffffffffffffffffffffff8181165f90815260208190526040908190205490517f68bafff800000000000000000000000000000000000000000000000000000000815291166004820152602401610548565b610bf18433858773ffffffffffffffffffffffffffffffffffffffff16630dfe16816040518163ffffffff1660e01b8152600401602060405180830381865afa15801561020d573d5f803e3d5ffd5b610c408433848773ffffffffffffffffffffffffffffffffffffffff1663d21220a76040518163ffffffff1660e01b8152600401602060405180830381865afa15801561020d573d5f803e3d5ffd5b50505050565b6040805173ffffffffffffffffffffffffffffffffffffffff85811660248301528416604482015260648082018490528251808303909101815260849091019091526020810180517bffffffffffffffffffffffffffffffffffffffffffffffffffffffff167f23b872dd00000000000000000000000000000000000000000000000000000000179052610c40908590610ea3565b6040517fc5f3d25400000000000000000000000000000000000000000000000000000000815273ffffffffffffffffffffffffffffffffffffffff83169063c5f3d25490610d2d9084906004016118c3565b5f604051808303815f87803b158015610d44575f80fd5b505af1158015610d56573d5f803e3d5ffd5b5050604080516060810182523081525f6020808301829052835191955073ffffffffffffffffffffffffffffffffffffffff881694507f2cceac5555b0ca45a3744ced542f54b56ad2eb45e521962372eef212a2cbf36193830191610dbd918891016118c3565b604080517fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0818403018152918152915251610df891906119ff565b60405180910390a2505050565b8073ffffffffffffffffffffffffffffffffffffffff166317700f016040518163ffffffff1660e01b81526004015f604051808303815f87803b158015610e4a575f80fd5b505af1158015610e5c573d5f803e3d5ffd5b505060405173ffffffffffffffffffffffffffffffffffffffff841692507fc75bf4f03c02fab9414a7d7a54048c0486722bc72f33ad924709a0593608ad2791505f90a250565b5f610f04826040518060400160405280602081526020017f5361666545524332303a206c6f772d6c6576656c2063616c6c206661696c65648152508573ffffffffffffffffffffffffffffffffffffffff16610fb09092919063ffffffff16565b905080515f1480610f24575080806020019051810190610f249190611a43565b61029e576040517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152602a60248201527f5361666545524332303a204552433230206f7065726174696f6e20646964206e60448201527f6f742073756363656564000000000000000000000000000000000000000000006064820152608401610548565b6060610fbe84845f85610fc6565b949350505050565b606082471015611058576040517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152602660248201527f416464726573733a20696e73756666696369656e742062616c616e636520666f60448201527f722063616c6c00000000000000000000000000000000000000000000000000006064820152608401610548565b5f808673ffffffffffffffffffffffffffffffffffffffff1685876040516110809190611a5c565b5f6040518083038185875af1925050503d805f81146110ba576040519150601f19603f3d011682016040523d82523d5f602084013e6110bf565b606091505b50915091506110d0878383876110db565b979650505050505050565b606083156111705782515f036111695773ffffffffffffffffffffffffffffffffffffffff85163b611169576040517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601d60248201527f416464726573733a2063616c6c20746f206e6f6e2d636f6e74726163740000006044820152606401610548565b5081610fbe565b610fbe83838151156111855781518083602001fd5b806040517f08c379a00000000000000000000000000000000000000000000000000000000081526004016105489190611a67565b61267880611a7a83390190565b73ffffffffffffffffffffffffffffffffffffffff811681146111e7575f80fd5b50565b5f805f606084860312156111fc575f80fd5b8335611207816111c6565b95602085013595506040909401359392505050565b5f8083601f84011261122c575f80fd5b50813567ffffffffffffffff811115611243575f80fd5b60208301915083602082850101111561125a575f80fd5b9250929050565b5f805f805f805f805f6101008a8c03121561127a575f80fd5b8935611285816111c6565b985060208a0135975060408a013561129c816111c6565b965060608a0135955060808a0135945060a08a01356112ba816111c6565b935060c08a013567ffffffffffffffff8111156112d5575f80fd5b6112e18c828d0161121c565b9a9d999c50979a9699959894979660e00135949350505050565b5f805f805f8060808789031215611310575f80fd5b863561131b816111c6565b9550602087013567ffffffffffffffff80821115611337575f80fd5b908801906060828b03121561134a575f80fd5b9095506040880135908082111561135f575f80fd5b61136b8a838b0161121c565b90965094506060890135915080821115611383575f80fd5b818901915089601f830112611396575f80fd5b8135818111156113a4575f80fd5b8a60208260051b85010111156113b8575f80fd5b6020830194508093505050509295509295509295565b805173ffffffffffffffffffffffffffffffffffffffff168252602081015161140f602084018273ffffffffffffffffffffffffffffffffffffffff169052565b506040810151611437604084018273ffffffffffffffffffffffffffffffffffffffff169052565b50606081015160608301526080810151608083015260a081015161146360a084018263ffffffff169052565b5060c081015160c083015260e081015160e0830152610100808201518184015250610120808201516114988285018215159052565b5050610140818101519083015261016090810151910152565b5f81518084528060208401602086015e5f6020828601015260207fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f83011685010191505092915050565b5f6101a061150b83866113ce565b8061018084015261151e818401856114b1565b95945050505050565b5f60208284031215611537575f80fd5b8135611542816111c6565b9392505050565b5f805f6060848603121561155b575f80fd5b8335611566816111c6565b92506020840135611576816111c6565b91506040840135611586816111c6565b809150509250925092565b5f805f805f8060a087890312156115a6575f80fd5b86356115b1816111c6565b95506020870135945060408701356115c8816111c6565b9350606087013567ffffffffffffffff8111156115e3575f80fd5b6115ef89828a0161121c565b979a9699509497949695608090950135949350505050565b8051611612816111c6565b919050565b5f60208284031215611627575f80fd5b8151611542816111c6565b5f8083357fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe1843603018112611665575f80fd5b83018035915067ffffffffffffffff82111561167f575f80fd5b60200191503681900382131561125a575f80fd5b7f4e487b71000000000000000000000000000000000000000000000000000000005f52604160045260245ffd5b6040516080810167ffffffffffffffff811182821017156116e3576116e3611693565b60405290565b604051610180810167ffffffffffffffff811182821017156116e3576116e3611693565b604051601f82017fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe016810167ffffffffffffffff8111828210171561175457611754611693565b604052919050565b5f602080838503121561176d575f80fd5b823567ffffffffffffffff80821115611784575f80fd5b9084019060808287031215611797575f80fd5b61179f6116c0565b82358152838301356117b0816111c6565b818501526040830135828111156117c5575f80fd5b8301601f810188136117d5575f80fd5b8035838111156117e7576117e7611693565b611817867fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f8401160161170d565b9350808452888682840101111561182c575f80fd5b80868301878601375f8682860101525050816040820152606083013560608201528094505050505092915050565b5f6020828403121561186a575f80fd5b5051919050565b8051825273ffffffffffffffffffffffffffffffffffffffff60208201511660208301525f6040820151608060408501526118af60808501826114b1565b606093840151949093019390935250919050565b602081525f6115426020830184611871565b805163ffffffff81168114611612575f80fd5b80518015158114611612575f80fd5b5f6101808284031215611908575f80fd5b6119106116e9565b61191983611607565b815261192760208401611607565b602082015261193860408401611607565b6040820152606083015160608201526080830151608082015261195d60a084016118d5565b60a082015260c083015160c082015260e083015160e08201526101008084015181830152506101206119908185016118e8565b908201526101408381015190820152610160928301519281019290925250919050565b5f6101a06119c183866113ce565b8061018084015261151e81840185611871565b5f81518060208401855e5f93019283525090919050565b5f610fbe6119f983866119d4565b846119d4565b6020815273ffffffffffffffffffffffffffffffffffffffff8251166020820152602082015160408201525f6040830151606080840152610fbe60808401826114b1565b5f60208284031215611a53575f80fd5b611542826118e8565b5f61154282846119d4565b602081525f61154260208301846114b156fe610120604052348015610010575f80fd5b5060405161267838038061267883398101604081905261002f9161052f565b6001600160a01b03831660808190526040805163f698da2560e01b8152905163f698da259160048082019260209290919082900301815f875af1158015610078573d5f803e3d5ffd5b505050506040513d601f19601f8201168201806040525081019061009c9190610579565b610100526100aa823361015f565b6100b4813361015f565b336001600160a01b031660e0816001600160a01b0316815250505f836001600160a01b0316639b552cc26040518163ffffffff1660e01b81526004016020604051808303815f875af115801561010c573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906101309190610590565b905061013c838261015f565b610146828261015f565b506001600160a01b0391821660a0521660c0525061061c565b6101746001600160a01b038316825f19610178565b5050565b8015806101f05750604051636eb1769f60e11b81523060048201526001600160a01b03838116602483015284169063dd62ed3e90604401602060405180830381865afa1580156101ca573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906101ee9190610579565b155b6102675760405162461bcd60e51b815260206004820152603660248201527f5361666545524332303a20617070726f76652066726f6d206e6f6e2d7a65726f60448201527f20746f206e6f6e2d7a65726f20616c6c6f77616e63650000000000000000000060648201526084015b60405180910390fd5b604080516001600160a01b038416602482015260448082018490528251808303909101815260649091019091526020810180516001600160e01b0390811663095ea7b360e01b179091526102bd9185916102c216565b505050565b6040805180820190915260208082527f5361666545524332303a206c6f772d6c6576656c2063616c6c206661696c6564908201525f9061030e906001600160a01b03851690849061038d565b905080515f148061032e57508080602001905181019061032e91906105b2565b6102bd5760405162461bcd60e51b815260206004820152602a60248201527f5361666545524332303a204552433230206f7065726174696f6e20646964206e6044820152691bdd081cdd58d8d9595960b21b606482015260840161025e565b606061039b84845f856103a3565b949350505050565b6060824710156104045760405162461bcd60e51b815260206004820152602660248201527f416464726573733a20696e73756666696369656e742062616c616e636520666f6044820152651c8818d85b1b60d21b606482015260840161025e565b5f80866001600160a01b0316858760405161041f91906105d1565b5f6040518083038185875af1925050503d805f8114610459576040519150601f19603f3d011682016040523d82523d5f602084013e61045e565b606091505b5090925090506104708783838761047b565b979650505050505050565b606083156104e95782515f036104e2576001600160a01b0385163b6104e25760405162461bcd60e51b815260206004820152601d60248201527f416464726573733a2063616c6c20746f206e6f6e2d636f6e7472616374000000604482015260640161025e565b508161039b565b61039b83838151156104fe5781518083602001fd5b8060405162461bcd60e51b815260040161025e91906105e7565b6001600160a01b038116811461052c575f80fd5b50565b5f805f60608486031215610541575f80fd5b835161054c81610518565b602085015190935061055d81610518565b604085015190925061056e81610518565b809150509250925092565b5f60208284031215610589575f80fd5b5051919050565b5f602082840312156105a0575f80fd5b81516105ab81610518565b9392505050565b5f602082840312156105c2575f80fd5b815180151581146105ab575f80fd5b5f82518060208501845e5f920191825250919050565b602081525f82518060208401528060208501604085015e5f604082850101526040601f19601f83011684010191505092915050565b60805160a05160c05160e05161010051611fbd6106bb5f395f81816102db015261042b01525f8181610236015281816104d90152610bf901525f81816102b40152818161059901528181610d5c01528181610ebf01528181610f8e015261100d01525f81816101380152818161057701528181610d3b01528181610e2801528181610f6b015261103001525f818161032201526112140152611fbd5ff3fe608060405234801561000f575f80fd5b506004361061012f575f3560e01c8063b09aaaca116100ad578063e3e6f5b21161007d578063eec50b9711610063578063eec50b9714610344578063f14fcbc81461034c578063ff2dbc9814610203575f80fd5b8063e3e6f5b2146102fd578063e516715b1461031d575f80fd5b8063b09aaaca14610289578063c5f3d2541461029c578063d21220a7146102af578063d25e0cb6146102d6575f80fd5b80631c7de94111610102578063481c6a75116100e8578063481c6a7514610231578063981a160b14610258578063a029a8d414610276575f80fd5b80631c7de941146102035780633e706e321461020a575f80fd5b80630dfe1681146101335780631303a484146101845780631626ba7e146101b557806317700f01146101f9575b5f80fd5b61015a7f000000000000000000000000000000000000000000000000000000000000000081565b60405173ffffffffffffffffffffffffffffffffffffffff90911681526020015b60405180910390f35b7f6c3c90245457060f6517787b2c4b8cf500ca889d2304af02043bd5b513e3b5935c5b60405190815260200161017b565b6101c86101c33660046116bf565b61035f565b6040517fffffffff00000000000000000000000000000000000000000000000000000000909116815260200161017b565b6102016104d7565b005b6101a75f81565b6101a77f6c3c90245457060f6517787b2c4b8cf500ca889d2304af02043bd5b513e3b59381565b61015a7f000000000000000000000000000000000000000000000000000000000000000081565b61026161012c81565b60405163ffffffff909116815260200161017b565b6102016102843660046119ee565b610573565b6101a7610297366004611a3b565b610bc8565b6102016102aa366004611a75565b610bf7565b61015a7f000000000000000000000000000000000000000000000000000000000000000081565b6101a77f000000000000000000000000000000000000000000000000000000000000000081565b61031061030b366004611a3b565b610cb7565b60405161017b9190611aac565b61015a7f000000000000000000000000000000000000000000000000000000000000000081565b6101a75f5481565b61020161035a366004611b9a565b6111fc565b5f808061036e84860186611bb1565b915091505f5461037d82610bc8565b146103b4576040517ff1a6789000000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0820180517fd5a25ba2e97094ad7d83dc28a6572da797d6b3e7fc6663bd93efb789fc17e48982526101a0822091526040517f190100000000000000000000000000000000000000000000000000000000000081527f00000000000000000000000000000000000000000000000000000000000000006002820152602281019190915260429020868114610494576040517f593fcacd00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b61049f818385611291565b6104a98284610573565b507f1626ba7e00000000000000000000000000000000000000000000000000000000925050505b9392505050565b7f000000000000000000000000000000000000000000000000000000000000000073ffffffffffffffffffffffffffffffffffffffff163314610546576040517ff87d0d1600000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b5f8080556040517fbcb8b8fbdea8aa6dc4ae41213e4da81e605a3d1a56ed851b9355182321c091909190a1565b80517f0000000000000000000000000000000000000000000000000000000000000000907f00000000000000000000000000000000000000000000000000000000000000009073ffffffffffffffffffffffffffffffffffffffff808416911614610677578073ffffffffffffffffffffffffffffffffffffffff16835f015173ffffffffffffffffffffffffffffffffffffffff1614610675576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601260248201527f696e76616c69642073656c6c20746f6b656e000000000000000000000000000060448201526064015b60405180910390fd5b905b6040517f70a082310000000000000000000000000000000000000000000000000000000081523060048201525f9073ffffffffffffffffffffffffffffffffffffffff8416906370a0823190602401602060405180830381865afa1580156106e1573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906107059190611bff565b6040517f70a082310000000000000000000000000000000000000000000000000000000081523060048201529091505f9073ffffffffffffffffffffffffffffffffffffffff8416906370a0823190602401602060405180830381865afa158015610772573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906107969190611bff565b90508273ffffffffffffffffffffffffffffffffffffffff16856020015173ffffffffffffffffffffffffffffffffffffffff1614610831576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601160248201527f696e76616c69642062757920746f6b656e000000000000000000000000000000604482015260640161066c565b604085015173ffffffffffffffffffffffffffffffffffffffff16156108b3576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601d60248201527f7265636569766572206d757374206265207a65726f2061646472657373000000604482015260640161066c565b6108bf61012c42611c43565b8560a0015163ffffffff161115610932576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601e60248201527f76616c696469747920746f6f2066617220696e20746865206675747572650000604482015260640161066c565b85606001518560c00151146109a3576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152600f60248201527f696e76616c696420617070446174610000000000000000000000000000000000604482015260640161066c565b60e085015115610a0f576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601760248201527f66656520616d6f756e74206d757374206265207a65726f000000000000000000604482015260640161066c565b7f5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc985610160015114610a9d576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601d60248201527f627579546f6b656e42616c616e6365206d757374206265206572633230000000604482015260640161066c565b7f5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc985610140015114610b2b576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601e60248201527f73656c6c546f6b656e42616c616e6365206d7573742062652065726332300000604482015260640161066c565b6060850151610b3a9082611c56565b60808601516060870151610b4e9085611c6d565b610b589190611c56565b1015610bc0576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601760248201527f726563656976656420616d6f756e7420746f6f206c6f77000000000000000000604482015260640161066c565b505050505050565b5f81604051602001610bda9190611ccc565b604051602081830303815290604052805190602001209050919050565b7f000000000000000000000000000000000000000000000000000000000000000073ffffffffffffffffffffffffffffffffffffffff163314610c66576040517ff87d0d1600000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b5f610c7361029783611d27565b9050805f81905550807f510e4a4f76907c2d6158b343f7c4f2f597df385b727c26e9ef90e75093ace19a83604051610cab9190611d79565b60405180910390a25050565b60408051610180810182525f80825260208201819052918101829052606081018290526080810182905260a0810182905260c0810182905260e081018290526101008101829052610120810182905261014081018290526101608101919091525f80836020015173ffffffffffffffffffffffffffffffffffffffff1663355efdd97f00000000000000000000000000000000000000000000000000000000000000007f000000000000000000000000000000000000000000000000000000000000000087604001516040518463ffffffff1660e01b8152600401610d9e93929190611e3a565b6040805180830381865afa158015610db8573d5f803e3d5ffd5b505050506040513d601f19601f82011682018060405250810190610ddc9190611e72565b6040517f70a0823100000000000000000000000000000000000000000000000000000000815230600482015291935091505f90819073ffffffffffffffffffffffffffffffffffffffff7f000000000000000000000000000000000000000000000000000000000000000016906370a0823190602401602060405180830381865afa158015610e6d573d5f803e3d5ffd5b505050506040513d601f19601f82011682018060405250810190610e919190611bff565b6040517f70a082310000000000000000000000000000000000000000000000000000000081523060048201527f000000000000000000000000000000000000000000000000000000000000000073ffffffffffffffffffffffffffffffffffffffff16906370a0823190602401602060405180830381865afa158015610f19573d5f803e3d5ffd5b505050506040513d601f19601f82011682018060405250810190610f3d9190611bff565b90925090505f80808080610f518888611c56565b90505f610f5e8a88611c56565b90505f8282101561100b577f000000000000000000000000000000000000000000000000000000000000000096507f00000000000000000000000000000000000000000000000000000000000000009550610fd6610fbd60028b611ec1565b610fd184610fcc8e6002611c56565b611346565b61137e565b945061100185610fe6818d611c56565b610ff09085611c43565b610ffa8c8f611c56565b60016113cb565b9350849050611098565b7f000000000000000000000000000000000000000000000000000000000000000096507f0000000000000000000000000000000000000000000000000000000000000000955061106e61105f60028a611ec1565b610fd185610fcc8f6002611c56565b94506110928561107e818e611c56565b6110889086611c43565b610ffa8b8e611c56565b93508390505b8c518110156110df576110df6040518060400160405280601781526020017f74726164656420616d6f756e7420746f6f20736d616c6c000000000000000000815250611426565b6040518061018001604052808873ffffffffffffffffffffffffffffffffffffffff1681526020018773ffffffffffffffffffffffffffffffffffffffff1681526020015f73ffffffffffffffffffffffffffffffffffffffff16815260200186815260200185815260200161115661012c611466565b63ffffffff1681526020018e6060015181526020015f81526020017ff3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee34677581526020016001151581526020017f5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc981526020017f5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc98152509b505050505050505050505050919050565b3373ffffffffffffffffffffffffffffffffffffffff7f0000000000000000000000000000000000000000000000000000000000000000161461126b576040517fbf84897700000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b807f6c3c90245457060f6517787b2c4b8cf500ca889d2304af02043bd5b513e3b5935d50565b7f6c3c90245457060f6517787b2c4b8cf500ca889d2304af02043bd5b513e3b5935c8381146113405780156112f2576040517fdafbdd1f00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b5f6112fc84610cb7565b90506113088382611487565b61133e576040517fd9ff24c700000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b505b50505050565b5f82156113735781611359600185611c6d565b6113639190611ec1565b61136e906001611c43565b611375565b5f5b90505b92915050565b5f818310156113c5576113c56040518060400160405280601581526020017f7375627472616374696f6e20756e646572666c6f770000000000000000000000815250611426565b50900390565b5f806113d8868686611599565b905060018360028111156113ee576113ee611ed4565b14801561140a57505f848061140557611405611e94565b868809115b1561141d5761141a600182611c43565b90505b95945050505050565b611431436001611c43565b816040517f1fe8506e00000000000000000000000000000000000000000000000000000000815260040161066c929190611f01565b5f81806114738142611f19565b61147d9190611f3b565b6113789190611f63565b5f80825f015173ffffffffffffffffffffffffffffffffffffffff16845f015173ffffffffffffffffffffffffffffffffffffffff161490505f836020015173ffffffffffffffffffffffffffffffffffffffff16856020015173ffffffffffffffffffffffffffffffffffffffff161490505f846060015186606001511490505f856080015187608001511490505f8660a0015163ffffffff168860a0015163ffffffff161490505f8761010001518961010001511490505f88610120015115158a6101200151151514905086801561155e5750855b80156115675750845b80156115705750835b80156115795750825b80156115825750815b801561158b5750805b9a9950505050505050505050565b5f80807fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff858709858702925082811083820303915050805f036115ef578382816115e5576115e5611e94565b04925050506104d0565b808411611658576040517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601560248201527f4d6174683a206d756c446976206f766572666c6f770000000000000000000000604482015260640161066c565b5f8486880960026001871981018816978890046003810283188082028403028082028403028082028403028082028403028082028403029081029092039091025f889003889004909101858311909403939093029303949094049190911702949350505050565b5f805f604084860312156116d1575f80fd5b83359250602084013567ffffffffffffffff808211156116ef575f80fd5b818601915086601f830112611702575f80fd5b813581811115611710575f80fd5b876020828501011115611721575f80fd5b6020830194508093505050509250925092565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52604160045260245ffd5b6040516080810167ffffffffffffffff8111828210171561178457611784611734565b60405290565b604051610180810167ffffffffffffffff8111828210171561178457611784611734565b604051601f82017fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe016810167ffffffffffffffff811182821017156117f5576117f5611734565b604052919050565b73ffffffffffffffffffffffffffffffffffffffff8116811461181e575f80fd5b50565b5f60808284031215611831575f80fd5b611839611761565b90508135815260208083013561184e816117fd565b82820152604083013567ffffffffffffffff8082111561186c575f80fd5b818501915085601f83011261187f575f80fd5b81358181111561189157611891611734565b6118c1847fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f840116016117ae565b915080825286848285010111156118d6575f80fd5b80848401858401375f848284010152508060408501525050506060820135606082015292915050565b803561190a816117fd565b919050565b803563ffffffff8116811461190a575f80fd5b8035801515811461190a575f80fd5b5f6101808284031215611942575f80fd5b61194a61178a565b9050611955826118ff565b8152611963602083016118ff565b6020820152611974604083016118ff565b6040820152606082013560608201526080820135608082015261199960a0830161190f565b60a082015260c082013560c082015260e082013560e08201526101008083013581830152506101206119cc818401611922565b9082015261014082810135908201526101609182013591810191909152919050565b5f806101a08385031215611a00575f80fd5b823567ffffffffffffffff811115611a16575f80fd5b611a2285828601611821565b925050611a328460208501611931565b90509250929050565b5f60208284031215611a4b575f80fd5b813567ffffffffffffffff811115611a61575f80fd5b611a6d84828501611821565b949350505050565b5f60208284031215611a85575f80fd5b813567ffffffffffffffff811115611a9b575f80fd5b8201608081850312156104d0575f80fd5b815173ffffffffffffffffffffffffffffffffffffffff16815261018081016020830151611af2602084018273ffffffffffffffffffffffffffffffffffffffff169052565b506040830151611b1a604084018273ffffffffffffffffffffffffffffffffffffffff169052565b50606083015160608301526080830151608083015260a0830151611b4660a084018263ffffffff169052565b5060c083015160c083015260e083015160e083015261010080840151818401525061012080840151611b7b8285018215159052565b5050610140838101519083015261016092830151929091019190915290565b5f60208284031215611baa575f80fd5b5035919050565b5f806101a08385031215611bc3575f80fd5b611bcd8484611931565b915061018083013567ffffffffffffffff811115611be9575f80fd5b611bf585828601611821565b9150509250929050565b5f60208284031215611c0f575f80fd5b5051919050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52601160045260245ffd5b8082018082111561137857611378611c16565b808202811582820484141761137857611378611c16565b8181038181111561137857611378611c16565b5f81518084528060208401602086015e5f6020828601015260207fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f83011685010191505092915050565b602081528151602082015273ffffffffffffffffffffffffffffffffffffffff60208301511660408201525f604083015160806060840152611d1160a0840182611c80565b9050606084015160808401528091505092915050565b5f6113783683611821565b81835281816020850137505f602082840101525f60207fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f840116840101905092915050565b60208152813560208201525f6020830135611d93816117fd565b73ffffffffffffffffffffffffffffffffffffffff811660408401525060408301357fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe1843603018112611de4575f80fd5b830160208101903567ffffffffffffffff811115611e00575f80fd5b803603821315611e0e575f80fd5b60806060850152611e2360a085018284611d32565b915050606084013560808401528091505092915050565b5f73ffffffffffffffffffffffffffffffffffffffff80861683528085166020840152506060604083015261141d6060830184611c80565b5f8060408385031215611e83575f80fd5b505080516020909101519092909150565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52601260045260245ffd5b5f82611ecf57611ecf611e94565b500490565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52602160045260245ffd5b828152604060208201525f611a6d6040830184611c80565b5f63ffffffff80841680611f2f57611f2f611e94565b92169190910492915050565b63ffffffff818116838216028082169190828114611f5b57611f5b611c16565b505092915050565b63ffffffff818116838216019080821115611f8057611f80611c16565b509291505056fea2646970667358221220e3fb228b525d90b942c7e58fe2e2034a17bd258c082fd47740e764a7be45bac664736f6c63430008190033a26469706673582212201190cf42f989cee23f12597c8c1e9daab6d8c816513349c3ce7fd229cae5b0ff64736f6c63430008190033
    /// ```
    #[rustfmt::skip]
    #[allow(clippy::all)]
    pub static BYTECODE: alloy_sol_types::private::Bytes = alloy_sol_types::private::Bytes::from_static(
        b"`\xA0`@R4\x80\x15`\x0EW_\x80\xFD[P`@QaA\xB38\x03\x80aA\xB3\x839\x81\x01`@\x81\x90R`+\x91`;V[`\x01`\x01`\xA0\x1B\x03\x16`\x80R`fV[_` \x82\x84\x03\x12\x15`JW_\x80\xFD[\x81Q`\x01`\x01`\xA0\x1B\x03\x81\x16\x81\x14`_W_\x80\xFD[\x93\x92PPPV[`\x80QaA'a\0\x8C_9_\x81\x81a\x01\x89\x01R\x81\x81a\x02\xA8\x01Ra\x08\xC4\x01RaA'_\xF3\xFE`\x80`@R4\x80\x15a\0\x0FW_\x80\xFD[P`\x046\x10a\0\x9FW_5`\xE0\x1C\x80c7\xEB\xDFP\x11a\0rW\x80cfn\x1B9\x11a\0XW\x80cfn\x1B9\x14a\x01OW\x80c\xAB\"\x1Av\x14a\x01\x84W\x80c\xB5\xC5\xF6r\x14a\x01\xABW_\x80\xFD[\x80c7\xEB\xDFP\x14a\x01)W\x80c[]\x9E\xE6\x14a\x01<W_\x80\xFD[\x80c\x0E\xFEj\x8B\x14a\0\xA3W\x80c\"\xB1U\xC6\x14a\0\xB8W\x80c&\xE0\xA1\x96\x14a\0\xF5W\x80c'\x91\x05e\x14a\x01\x16W[_\x80\xFD[a\0\xB6a\0\xB16`\x04a\x11\xEAV[a\x01\xBEV[\0[a\0\xCBa\0\xC66`\x04a\x12aV[a\x02\xA3V[`@Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x90\x91\x16\x81R` \x01[`@Q\x80\x91\x03\x90\xF3[a\x01\x08a\x01\x036`\x04a\x12\xFBV[a\x04_V[`@Qa\0\xEC\x92\x91\x90a\x14\xFDV[a\0\xB6a\x01$6`\x04a\x15'V[a\x07\x97V[a\0\xCBa\x0176`\x04a\x15IV[a\x08/V[a\0\xB6a\x01J6`\x04a\x15\x91V[a\n\x01V[a\0\xCBa\x01]6`\x04a\x15'V[_` \x81\x90R\x90\x81R`@\x90 Ts\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81V[a\0\xCB\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[a\0\xB6a\x01\xB96`\x04a\x11\xEAV[a\x0B\x17V[a\x02O3\x84\x84\x86s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16c\r\xFE\x16\x81`@Q\x81c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x02\rW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x021\x91\x90a\x16\x17V[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x92\x91\x90a\x0CFV[a\x02\x9E3\x84\x83\x86s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16c\xD2\x12 \xA7`@Q\x81c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x02\rW=_\x80>=_\xFD[PPPV[_3\x80\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x8C\x8B`@Qa\x02\xD5\x90a\x11\xB9V[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x93\x84\x16\x81R\x91\x83\x16` \x83\x01R\x90\x91\x16`@\x82\x01R``\x01\x81\x90`@Q\x80\x91\x03\x90_\xF5\x90P\x80\x15\x80\x15a\x03\x1FW=_\x80>=_\xFD[P`@\x80Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x8E\x81\x16\x82R\x8C\x81\x16` \x83\x01R\x92\x94P\x82\x84\x16\x92\x85\x16\x91\x7Fg\x07%[,\\\xA8\x12 \xB2\xF3\xE4\x08\xA2i\xCB\x83\xBA\xA6\xAA~^7\xAA\x17V\x88:l\xDF\x06\xF1\x91\x01`@Q\x80\x91\x03\x90\xA3s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x81\x16_\x90\x81R` \x81\x90R`@\x90 \x80T\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x91\x83\x16\x91\x90\x91\x17\x90Ua\x03\xD8\x82\x8B\x8Aa\x01\xBEV[_`@Q\x80`\x80\x01`@R\x80\x89\x81R` \x01\x88s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x87\x87\x80\x80`\x1F\x01` \x80\x91\x04\x02` \x01`@Q\x90\x81\x01`@R\x80\x93\x92\x91\x90\x81\x81R` \x01\x83\x83\x80\x82\x847_\x92\x01\x91\x90\x91RPPP\x90\x82RP` \x01\x85\x90R\x90Pa\x04P\x83\x82a\x0C\xDBV[PP\x99\x98PPPPPPPPPV[`@\x80Qa\x01\x80\x81\x01\x82R_\x80\x82R` \x82\x01\x81\x90R\x91\x81\x01\x82\x90R``\x81\x01\x82\x90R`\x80\x81\x01\x82\x90R`\xA0\x81\x01\x82\x90R`\xC0\x81\x01\x82\x90R`\xE0\x81\x01\x82\x90Ra\x01\0\x81\x01\x82\x90Ra\x01 \x81\x01\x82\x90Ra\x01@\x81\x01\x82\x90Ra\x01`\x81\x01\x91\x90\x91R``0a\x04\xCF` \x89\x01\x89a\x15'V[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14a\x05QW`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1A`$\x82\x01R\x7Fcan only handle own orders\0\0\0\0\0\0`D\x82\x01R`d\x01[`@Q\x80\x91\x03\x90\xFD[_a\x05_`@\x89\x01\x89a\x162V[\x81\x01\x90a\x05l\x91\x90a\x17\\V[\x90P\x88s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16c\xEE\xC5\x0B\x97`@Q\x81c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x05\xB7W=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x05\xDB\x91\x90a\x18ZV[`@Q\x7F\xB0\x9A\xAA\xCA\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x8B\x16\x90c\xB0\x9A\xAA\xCA\x90a\x06-\x90\x85\x90`\x04\x01a\x18\xC3V[` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x06HW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x06l\x91\x90a\x18ZV[\x14a\x06\xD3W`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1A`$\x82\x01R\x7Finvalid trading parameters\0\0\0\0\0\0`D\x82\x01R`d\x01a\x05HV[`@Q\x7F\xE3\xE6\xF5\xB2\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x8A\x16\x90c\xE3\xE6\xF5\xB2\x90a\x07%\x90\x84\x90`\x04\x01a\x18\xC3V[a\x01\x80`@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x07AW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x07e\x91\x90a\x18\xF7V[\x92P\x82\x81`@Q` \x01a\x07z\x92\x91\x90a\x19\xB3V[`@Q` \x81\x83\x03\x03\x81R\x90`@R\x91PP\x96P\x96\x94PPPPPV[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x16_\x90\x81R` \x81\x90R`@\x90 T\x82\x91\x163\x14a\x08\"Ws\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x81\x16_\x90\x81R` \x81\x90R`@\x90\x81\x90 T\x90Q\x7Fh\xBA\xFF\xF8\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R\x91\x16`\x04\x82\x01R`$\x01a\x05HV[a\x08+\x82a\x0E\x05V[PPV[_\x80\x7F\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x000s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x87\x16`@Qa\x08y` \x82\x01a\x11\xB9V[\x81\x81\x03\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x90\x81\x01\x83R`\x1F\x90\x91\x01\x16`@\x81\x81Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81\x16` \x84\x01R\x80\x8B\x16\x91\x83\x01\x91\x90\x91R\x88\x16``\x82\x01R`\x80\x01`@\x80Q\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x81\x84\x03\x01\x81R\x90\x82\x90Ra\t:\x92\x91` \x01a\x19\xEBV[`@Q` \x81\x83\x03\x03\x81R\x90`@R\x80Q\x90` \x01 `@Q` \x01a\t\xC2\x94\x93\x92\x91\x90\x7F\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x94\x90\x94\x16\x84R``\x92\x90\x92\x1B\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\x16`\x01\x84\x01R`\x15\x83\x01R`5\x82\x01R`U\x01\x90V[`@\x80Q\x80\x83\x03\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x01\x81R\x91\x90R\x80Q` \x90\x91\x01 \x95\x94PPPPPV[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x87\x16_\x90\x81R` \x81\x90R`@\x90 T\x87\x91\x163\x14a\n\x8CWs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x81\x16_\x90\x81R` \x81\x90R`@\x90\x81\x90 T\x90Q\x7Fh\xBA\xFF\xF8\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R\x91\x16`\x04\x82\x01R`$\x01a\x05HV[_`@Q\x80`\x80\x01`@R\x80\x88\x81R` \x01\x87s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x86\x86\x80\x80`\x1F\x01` \x80\x91\x04\x02` \x01`@Q\x90\x81\x01`@R\x80\x93\x92\x91\x90\x81\x81R` \x01\x83\x83\x80\x82\x847_\x92\x01\x91\x90\x91RPPP\x90\x82RP` \x01\x84\x90R\x90Pa\x0B\x03\x88a\x0E\x05V[a\x0B\r\x88\x82a\x0C\xDBV[PPPPPPPPV[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x84\x16_\x90\x81R` \x81\x90R`@\x90 T\x84\x91\x163\x14a\x0B\xA2Ws\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x81\x16_\x90\x81R` \x81\x90R`@\x90\x81\x90 T\x90Q\x7Fh\xBA\xFF\xF8\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R\x91\x16`\x04\x82\x01R`$\x01a\x05HV[a\x0B\xF1\x843\x85\x87s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16c\r\xFE\x16\x81`@Q\x81c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x02\rW=_\x80>=_\xFD[a\x0C@\x843\x84\x87s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16c\xD2\x12 \xA7`@Q\x81c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x02\rW=_\x80>=_\xFD[PPPPV[`@\x80Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x85\x81\x16`$\x83\x01R\x84\x16`D\x82\x01R`d\x80\x82\x01\x84\x90R\x82Q\x80\x83\x03\x90\x91\x01\x81R`\x84\x90\x91\x01\x90\x91R` \x81\x01\x80Q{\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x7F#\xB8r\xDD\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x17\x90Ra\x0C@\x90\x85\x90a\x0E\xA3V[`@Q\x7F\xC5\xF3\xD2T\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x83\x16\x90c\xC5\xF3\xD2T\x90a\r-\x90\x84\x90`\x04\x01a\x18\xC3V[_`@Q\x80\x83\x03\x81_\x87\x80;\x15\x80\x15a\rDW_\x80\xFD[PZ\xF1\x15\x80\x15a\rVW=_\x80>=_\xFD[PP`@\x80Q``\x81\x01\x82R0\x81R_` \x80\x83\x01\x82\x90R\x83Q\x91\x95Ps\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x88\x16\x94P\x7F,\xCE\xACUU\xB0\xCAE\xA3tL\xEDT/T\xB5j\xD2\xEBE\xE5!\x96#r\xEE\xF2\x12\xA2\xCB\xF3a\x93\x83\x01\x91a\r\xBD\x91\x88\x91\x01a\x18\xC3V[`@\x80Q\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x81\x84\x03\x01\x81R\x91\x81R\x91RQa\r\xF8\x91\x90a\x19\xFFV[`@Q\x80\x91\x03\x90\xA2PPPV[\x80s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16c\x17p\x0F\x01`@Q\x81c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01_`@Q\x80\x83\x03\x81_\x87\x80;\x15\x80\x15a\x0EJW_\x80\xFD[PZ\xF1\x15\x80\x15a\x0E\\W=_\x80>=_\xFD[PP`@Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x84\x16\x92P\x7F\xC7[\xF4\xF0<\x02\xFA\xB9AJ}zT\x04\x8C\x04\x86r+\xC7/3\xAD\x92G\t\xA0Y6\x08\xAD'\x91P_\x90\xA2PV[_a\x0F\x04\x82`@Q\x80`@\x01`@R\x80` \x81R` \x01\x7FSafeERC20: low-level call failed\x81RP\x85s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16a\x0F\xB0\x90\x92\x91\x90c\xFF\xFF\xFF\xFF\x16V[\x90P\x80Q_\x14\x80a\x0F$WP\x80\x80` \x01\x90Q\x81\x01\x90a\x0F$\x91\x90a\x1ACV[a\x02\x9EW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`*`$\x82\x01R\x7FSafeERC20: ERC20 operation did n`D\x82\x01R\x7Fot succeed\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`d\x82\x01R`\x84\x01a\x05HV[``a\x0F\xBE\x84\x84_\x85a\x0F\xC6V[\x94\x93PPPPV[``\x82G\x10\x15a\x10XW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`&`$\x82\x01R\x7FAddress: insufficient balance fo`D\x82\x01R\x7Fr call\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`d\x82\x01R`\x84\x01a\x05HV[_\x80\x86s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x85\x87`@Qa\x10\x80\x91\x90a\x1A\\V[_`@Q\x80\x83\x03\x81\x85\x87Z\xF1\x92PPP=\x80_\x81\x14a\x10\xBAW`@Q\x91P`\x1F\x19`?=\x01\x16\x82\x01`@R=\x82R=_` \x84\x01>a\x10\xBFV[``\x91P[P\x91P\x91Pa\x10\xD0\x87\x83\x83\x87a\x10\xDBV[\x97\x96PPPPPPPV[``\x83\x15a\x11pW\x82Q_\x03a\x11iWs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x85\x16;a\x11iW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1D`$\x82\x01R\x7FAddress: call to non-contract\0\0\0`D\x82\x01R`d\x01a\x05HV[P\x81a\x0F\xBEV[a\x0F\xBE\x83\x83\x81Q\x15a\x11\x85W\x81Q\x80\x83` \x01\xFD[\x80`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01a\x05H\x91\x90a\x1AgV[a&x\x80a\x1Az\x839\x01\x90V[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x16\x81\x14a\x11\xE7W_\x80\xFD[PV[_\x80_``\x84\x86\x03\x12\x15a\x11\xFCW_\x80\xFD[\x835a\x12\x07\x81a\x11\xC6V[\x95` \x85\x015\x95P`@\x90\x94\x015\x93\x92PPPV[_\x80\x83`\x1F\x84\x01\x12a\x12,W_\x80\xFD[P\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x12CW_\x80\xFD[` \x83\x01\x91P\x83` \x82\x85\x01\x01\x11\x15a\x12ZW_\x80\xFD[\x92P\x92\x90PV[_\x80_\x80_\x80_\x80_a\x01\0\x8A\x8C\x03\x12\x15a\x12zW_\x80\xFD[\x895a\x12\x85\x81a\x11\xC6V[\x98P` \x8A\x015\x97P`@\x8A\x015a\x12\x9C\x81a\x11\xC6V[\x96P``\x8A\x015\x95P`\x80\x8A\x015\x94P`\xA0\x8A\x015a\x12\xBA\x81a\x11\xC6V[\x93P`\xC0\x8A\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x12\xD5W_\x80\xFD[a\x12\xE1\x8C\x82\x8D\x01a\x12\x1CV[\x9A\x9D\x99\x9CP\x97\x9A\x96\x99\x95\x98\x94\x97\x96`\xE0\x015\x94\x93PPPPV[_\x80_\x80_\x80`\x80\x87\x89\x03\x12\x15a\x13\x10W_\x80\xFD[\x865a\x13\x1B\x81a\x11\xC6V[\x95P` \x87\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a\x137W_\x80\xFD[\x90\x88\x01\x90``\x82\x8B\x03\x12\x15a\x13JW_\x80\xFD[\x90\x95P`@\x88\x015\x90\x80\x82\x11\x15a\x13_W_\x80\xFD[a\x13k\x8A\x83\x8B\x01a\x12\x1CV[\x90\x96P\x94P``\x89\x015\x91P\x80\x82\x11\x15a\x13\x83W_\x80\xFD[\x81\x89\x01\x91P\x89`\x1F\x83\x01\x12a\x13\x96W_\x80\xFD[\x815\x81\x81\x11\x15a\x13\xA4W_\x80\xFD[\x8A` \x82`\x05\x1B\x85\x01\x01\x11\x15a\x13\xB8W_\x80\xFD[` \x83\x01\x94P\x80\x93PPPP\x92\x95P\x92\x95P\x92\x95V[\x80Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x82R` \x81\x01Qa\x14\x0F` \x84\x01\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90RV[P`@\x81\x01Qa\x147`@\x84\x01\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90RV[P``\x81\x01Q``\x83\x01R`\x80\x81\x01Q`\x80\x83\x01R`\xA0\x81\x01Qa\x14c`\xA0\x84\x01\x82c\xFF\xFF\xFF\xFF\x16\x90RV[P`\xC0\x81\x01Q`\xC0\x83\x01R`\xE0\x81\x01Q`\xE0\x83\x01Ra\x01\0\x80\x82\x01Q\x81\x84\x01RPa\x01 \x80\x82\x01Qa\x14\x98\x82\x85\x01\x82\x15\x15\x90RV[PPa\x01@\x81\x81\x01Q\x90\x83\x01Ra\x01`\x90\x81\x01Q\x91\x01RV[_\x81Q\x80\x84R\x80` \x84\x01` \x86\x01^_` \x82\x86\x01\x01R` \x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`\x1F\x83\x01\x16\x85\x01\x01\x91PP\x92\x91PPV[_a\x01\xA0a\x15\x0B\x83\x86a\x13\xCEV[\x80a\x01\x80\x84\x01Ra\x15\x1E\x81\x84\x01\x85a\x14\xB1V[\x95\x94PPPPPV[_` \x82\x84\x03\x12\x15a\x157W_\x80\xFD[\x815a\x15B\x81a\x11\xC6V[\x93\x92PPPV[_\x80_``\x84\x86\x03\x12\x15a\x15[W_\x80\xFD[\x835a\x15f\x81a\x11\xC6V[\x92P` \x84\x015a\x15v\x81a\x11\xC6V[\x91P`@\x84\x015a\x15\x86\x81a\x11\xC6V[\x80\x91PP\x92P\x92P\x92V[_\x80_\x80_\x80`\xA0\x87\x89\x03\x12\x15a\x15\xA6W_\x80\xFD[\x865a\x15\xB1\x81a\x11\xC6V[\x95P` \x87\x015\x94P`@\x87\x015a\x15\xC8\x81a\x11\xC6V[\x93P``\x87\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x15\xE3W_\x80\xFD[a\x15\xEF\x89\x82\x8A\x01a\x12\x1CV[\x97\x9A\x96\x99P\x94\x97\x94\x96\x95`\x80\x90\x95\x015\x94\x93PPPPV[\x80Qa\x16\x12\x81a\x11\xC6V[\x91\x90PV[_` \x82\x84\x03\x12\x15a\x16'W_\x80\xFD[\x81Qa\x15B\x81a\x11\xC6V[_\x80\x835\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE1\x846\x03\x01\x81\x12a\x16eW_\x80\xFD[\x83\x01\x805\x91Pg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11\x15a\x16\x7FW_\x80\xFD[` \x01\x91P6\x81\x90\x03\x82\x13\x15a\x12ZW_\x80\xFD[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`A`\x04R`$_\xFD[`@Q`\x80\x81\x01g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x82\x82\x10\x17\x15a\x16\xE3Wa\x16\xE3a\x16\x93V[`@R\x90V[`@Qa\x01\x80\x81\x01g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x82\x82\x10\x17\x15a\x16\xE3Wa\x16\xE3a\x16\x93V[`@Q`\x1F\x82\x01\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x16\x81\x01g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x82\x82\x10\x17\x15a\x17TWa\x17Ta\x16\x93V[`@R\x91\x90PV[_` \x80\x83\x85\x03\x12\x15a\x17mW_\x80\xFD[\x825g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a\x17\x84W_\x80\xFD[\x90\x84\x01\x90`\x80\x82\x87\x03\x12\x15a\x17\x97W_\x80\xFD[a\x17\x9Fa\x16\xC0V[\x825\x81R\x83\x83\x015a\x17\xB0\x81a\x11\xC6V[\x81\x85\x01R`@\x83\x015\x82\x81\x11\x15a\x17\xC5W_\x80\xFD[\x83\x01`\x1F\x81\x01\x88\x13a\x17\xD5W_\x80\xFD[\x805\x83\x81\x11\x15a\x17\xE7Wa\x17\xE7a\x16\x93V[a\x18\x17\x86\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`\x1F\x84\x01\x16\x01a\x17\rV[\x93P\x80\x84R\x88\x86\x82\x84\x01\x01\x11\x15a\x18,W_\x80\xFD[\x80\x86\x83\x01\x87\x86\x017_\x86\x82\x86\x01\x01RPP\x81`@\x82\x01R``\x83\x015``\x82\x01R\x80\x94PPPPP\x92\x91PPV[_` \x82\x84\x03\x12\x15a\x18jW_\x80\xFD[PQ\x91\x90PV[\x80Q\x82Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF` \x82\x01Q\x16` \x83\x01R_`@\x82\x01Q`\x80`@\x85\x01Ra\x18\xAF`\x80\x85\x01\x82a\x14\xB1V[``\x93\x84\x01Q\x94\x90\x93\x01\x93\x90\x93RP\x91\x90PV[` \x81R_a\x15B` \x83\x01\x84a\x18qV[\x80Qc\xFF\xFF\xFF\xFF\x81\x16\x81\x14a\x16\x12W_\x80\xFD[\x80Q\x80\x15\x15\x81\x14a\x16\x12W_\x80\xFD[_a\x01\x80\x82\x84\x03\x12\x15a\x19\x08W_\x80\xFD[a\x19\x10a\x16\xE9V[a\x19\x19\x83a\x16\x07V[\x81Ra\x19'` \x84\x01a\x16\x07V[` \x82\x01Ra\x198`@\x84\x01a\x16\x07V[`@\x82\x01R``\x83\x01Q``\x82\x01R`\x80\x83\x01Q`\x80\x82\x01Ra\x19]`\xA0\x84\x01a\x18\xD5V[`\xA0\x82\x01R`\xC0\x83\x01Q`\xC0\x82\x01R`\xE0\x83\x01Q`\xE0\x82\x01Ra\x01\0\x80\x84\x01Q\x81\x83\x01RPa\x01 a\x19\x90\x81\x85\x01a\x18\xE8V[\x90\x82\x01Ra\x01@\x83\x81\x01Q\x90\x82\x01Ra\x01`\x92\x83\x01Q\x92\x81\x01\x92\x90\x92RP\x91\x90PV[_a\x01\xA0a\x19\xC1\x83\x86a\x13\xCEV[\x80a\x01\x80\x84\x01Ra\x15\x1E\x81\x84\x01\x85a\x18qV[_\x81Q\x80` \x84\x01\x85^_\x93\x01\x92\x83RP\x90\x91\x90PV[_a\x0F\xBEa\x19\xF9\x83\x86a\x19\xD4V[\x84a\x19\xD4V[` \x81Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82Q\x16` \x82\x01R` \x82\x01Q`@\x82\x01R_`@\x83\x01Q``\x80\x84\x01Ra\x0F\xBE`\x80\x84\x01\x82a\x14\xB1V[_` \x82\x84\x03\x12\x15a\x1ASW_\x80\xFD[a\x15B\x82a\x18\xE8V[_a\x15B\x82\x84a\x19\xD4V[` \x81R_a\x15B` \x83\x01\x84a\x14\xB1V\xFEa\x01 `@R4\x80\x15a\0\x10W_\x80\xFD[P`@Qa&x8\x03\x80a&x\x839\x81\x01`@\x81\x90Ra\0/\x91a\x05/V[`\x01`\x01`\xA0\x1B\x03\x83\x16`\x80\x81\x90R`@\x80Qc\xF6\x98\xDA%`\xE0\x1B\x81R\x90Qc\xF6\x98\xDA%\x91`\x04\x80\x82\x01\x92` \x92\x90\x91\x90\x82\x90\x03\x01\x81_\x87Z\xF1\x15\x80\x15a\0xW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\0\x9C\x91\x90a\x05yV[a\x01\0Ra\0\xAA\x823a\x01_V[a\0\xB4\x813a\x01_V[3`\x01`\x01`\xA0\x1B\x03\x16`\xE0\x81`\x01`\x01`\xA0\x1B\x03\x16\x81RPP_\x83`\x01`\x01`\xA0\x1B\x03\x16c\x9BU,\xC2`@Q\x81c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01` `@Q\x80\x83\x03\x81_\x87Z\xF1\x15\x80\x15a\x01\x0CW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x010\x91\x90a\x05\x90V[\x90Pa\x01<\x83\x82a\x01_V[a\x01F\x82\x82a\x01_V[P`\x01`\x01`\xA0\x1B\x03\x91\x82\x16`\xA0R\x16`\xC0RPa\x06\x1CV[a\x01t`\x01`\x01`\xA0\x1B\x03\x83\x16\x82_\x19a\x01xV[PPV[\x80\x15\x80a\x01\xF0WP`@Qcn\xB1v\x9F`\xE1\x1B\x81R0`\x04\x82\x01R`\x01`\x01`\xA0\x1B\x03\x83\x81\x16`$\x83\x01R\x84\x16\x90c\xDDb\xED>\x90`D\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x01\xCAW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x01\xEE\x91\x90a\x05yV[\x15[a\x02gW`@QbF\x1B\xCD`\xE5\x1B\x81R` `\x04\x82\x01R`6`$\x82\x01R\x7FSafeERC20: approve from non-zero`D\x82\x01R\x7F to non-zero allowance\0\0\0\0\0\0\0\0\0\0`d\x82\x01R`\x84\x01[`@Q\x80\x91\x03\x90\xFD[`@\x80Q`\x01`\x01`\xA0\x1B\x03\x84\x16`$\x82\x01R`D\x80\x82\x01\x84\x90R\x82Q\x80\x83\x03\x90\x91\x01\x81R`d\x90\x91\x01\x90\x91R` \x81\x01\x80Q`\x01`\x01`\xE0\x1B\x03\x90\x81\x16c\t^\xA7\xB3`\xE0\x1B\x17\x90\x91Ra\x02\xBD\x91\x85\x91a\x02\xC2\x16V[PPPV[`@\x80Q\x80\x82\x01\x90\x91R` \x80\x82R\x7FSafeERC20: low-level call failed\x90\x82\x01R_\x90a\x03\x0E\x90`\x01`\x01`\xA0\x1B\x03\x85\x16\x90\x84\x90a\x03\x8DV[\x90P\x80Q_\x14\x80a\x03.WP\x80\x80` \x01\x90Q\x81\x01\x90a\x03.\x91\x90a\x05\xB2V[a\x02\xBDW`@QbF\x1B\xCD`\xE5\x1B\x81R` `\x04\x82\x01R`*`$\x82\x01R\x7FSafeERC20: ERC20 operation did n`D\x82\x01Ri\x1B\xDD\x08\x1C\xDDX\xD8\xD9YY`\xB2\x1B`d\x82\x01R`\x84\x01a\x02^V[``a\x03\x9B\x84\x84_\x85a\x03\xA3V[\x94\x93PPPPV[``\x82G\x10\x15a\x04\x04W`@QbF\x1B\xCD`\xE5\x1B\x81R` `\x04\x82\x01R`&`$\x82\x01R\x7FAddress: insufficient balance fo`D\x82\x01Re\x1C\x88\x18\xD8[\x1B`\xD2\x1B`d\x82\x01R`\x84\x01a\x02^V[_\x80\x86`\x01`\x01`\xA0\x1B\x03\x16\x85\x87`@Qa\x04\x1F\x91\x90a\x05\xD1V[_`@Q\x80\x83\x03\x81\x85\x87Z\xF1\x92PPP=\x80_\x81\x14a\x04YW`@Q\x91P`\x1F\x19`?=\x01\x16\x82\x01`@R=\x82R=_` \x84\x01>a\x04^V[``\x91P[P\x90\x92P\x90Pa\x04p\x87\x83\x83\x87a\x04{V[\x97\x96PPPPPPPV[``\x83\x15a\x04\xE9W\x82Q_\x03a\x04\xE2W`\x01`\x01`\xA0\x1B\x03\x85\x16;a\x04\xE2W`@QbF\x1B\xCD`\xE5\x1B\x81R` `\x04\x82\x01R`\x1D`$\x82\x01R\x7FAddress: call to non-contract\0\0\0`D\x82\x01R`d\x01a\x02^V[P\x81a\x03\x9BV[a\x03\x9B\x83\x83\x81Q\x15a\x04\xFEW\x81Q\x80\x83` \x01\xFD[\x80`@QbF\x1B\xCD`\xE5\x1B\x81R`\x04\x01a\x02^\x91\x90a\x05\xE7V[`\x01`\x01`\xA0\x1B\x03\x81\x16\x81\x14a\x05,W_\x80\xFD[PV[_\x80_``\x84\x86\x03\x12\x15a\x05AW_\x80\xFD[\x83Qa\x05L\x81a\x05\x18V[` \x85\x01Q\x90\x93Pa\x05]\x81a\x05\x18V[`@\x85\x01Q\x90\x92Pa\x05n\x81a\x05\x18V[\x80\x91PP\x92P\x92P\x92V[_` \x82\x84\x03\x12\x15a\x05\x89W_\x80\xFD[PQ\x91\x90PV[_` \x82\x84\x03\x12\x15a\x05\xA0W_\x80\xFD[\x81Qa\x05\xAB\x81a\x05\x18V[\x93\x92PPPV[_` \x82\x84\x03\x12\x15a\x05\xC2W_\x80\xFD[\x81Q\x80\x15\x15\x81\x14a\x05\xABW_\x80\xFD[_\x82Q\x80` \x85\x01\x84^_\x92\x01\x91\x82RP\x91\x90PV[` \x81R_\x82Q\x80` \x84\x01R\x80` \x85\x01`@\x85\x01^_`@\x82\x85\x01\x01R`@`\x1F\x19`\x1F\x83\x01\x16\x84\x01\x01\x91PP\x92\x91PPV[`\x80Q`\xA0Q`\xC0Q`\xE0Qa\x01\0Qa\x1F\xBDa\x06\xBB_9_\x81\x81a\x02\xDB\x01Ra\x04+\x01R_\x81\x81a\x026\x01R\x81\x81a\x04\xD9\x01Ra\x0B\xF9\x01R_\x81\x81a\x02\xB4\x01R\x81\x81a\x05\x99\x01R\x81\x81a\r\\\x01R\x81\x81a\x0E\xBF\x01R\x81\x81a\x0F\x8E\x01Ra\x10\r\x01R_\x81\x81a\x018\x01R\x81\x81a\x05w\x01R\x81\x81a\r;\x01R\x81\x81a\x0E(\x01R\x81\x81a\x0Fk\x01Ra\x100\x01R_\x81\x81a\x03\"\x01Ra\x12\x14\x01Ra\x1F\xBD_\xF3\xFE`\x80`@R4\x80\x15a\0\x0FW_\x80\xFD[P`\x046\x10a\x01/W_5`\xE0\x1C\x80c\xB0\x9A\xAA\xCA\x11a\0\xADW\x80c\xE3\xE6\xF5\xB2\x11a\0}W\x80c\xEE\xC5\x0B\x97\x11a\0cW\x80c\xEE\xC5\x0B\x97\x14a\x03DW\x80c\xF1O\xCB\xC8\x14a\x03LW\x80c\xFF-\xBC\x98\x14a\x02\x03W_\x80\xFD[\x80c\xE3\xE6\xF5\xB2\x14a\x02\xFDW\x80c\xE5\x16q[\x14a\x03\x1DW_\x80\xFD[\x80c\xB0\x9A\xAA\xCA\x14a\x02\x89W\x80c\xC5\xF3\xD2T\x14a\x02\x9CW\x80c\xD2\x12 \xA7\x14a\x02\xAFW\x80c\xD2^\x0C\xB6\x14a\x02\xD6W_\x80\xFD[\x80c\x1C}\xE9A\x11a\x01\x02W\x80cH\x1Cju\x11a\0\xE8W\x80cH\x1Cju\x14a\x021W\x80c\x98\x1A\x16\x0B\x14a\x02XW\x80c\xA0)\xA8\xD4\x14a\x02vW_\x80\xFD[\x80c\x1C}\xE9A\x14a\x02\x03W\x80c>pn2\x14a\x02\nW_\x80\xFD[\x80c\r\xFE\x16\x81\x14a\x013W\x80c\x13\x03\xA4\x84\x14a\x01\x84W\x80c\x16&\xBA~\x14a\x01\xB5W\x80c\x17p\x0F\x01\x14a\x01\xF9W[_\x80\xFD[a\x01Z\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[`@Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x90\x91\x16\x81R` \x01[`@Q\x80\x91\x03\x90\xF3[\x7Fl<\x90$TW\x06\x0Fe\x17x{,K\x8C\xF5\0\xCA\x88\x9D#\x04\xAF\x02\x04;\xD5\xB5\x13\xE3\xB5\x93\\[`@Q\x90\x81R` \x01a\x01{V[a\x01\xC8a\x01\xC36`\x04a\x16\xBFV[a\x03_V[`@Q\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90\x91\x16\x81R` \x01a\x01{V[a\x02\x01a\x04\xD7V[\0[a\x01\xA7_\x81V[a\x01\xA7\x7Fl<\x90$TW\x06\x0Fe\x17x{,K\x8C\xF5\0\xCA\x88\x9D#\x04\xAF\x02\x04;\xD5\xB5\x13\xE3\xB5\x93\x81V[a\x01Z\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[a\x02aa\x01,\x81V[`@Qc\xFF\xFF\xFF\xFF\x90\x91\x16\x81R` \x01a\x01{V[a\x02\x01a\x02\x846`\x04a\x19\xEEV[a\x05sV[a\x01\xA7a\x02\x976`\x04a\x1A;V[a\x0B\xC8V[a\x02\x01a\x02\xAA6`\x04a\x1AuV[a\x0B\xF7V[a\x01Z\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[a\x01\xA7\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[a\x03\x10a\x03\x0B6`\x04a\x1A;V[a\x0C\xB7V[`@Qa\x01{\x91\x90a\x1A\xACV[a\x01Z\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[a\x01\xA7_T\x81V[a\x02\x01a\x03Z6`\x04a\x1B\x9AV[a\x11\xFCV[_\x80\x80a\x03n\x84\x86\x01\x86a\x1B\xB1V[\x91P\x91P_Ta\x03}\x82a\x0B\xC8V[\x14a\x03\xB4W`@Q\x7F\xF1\xA6x\x90\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x82\x01\x80Q\x7F\xD5\xA2[\xA2\xE9p\x94\xAD}\x83\xDC(\xA6W-\xA7\x97\xD6\xB3\xE7\xFCfc\xBD\x93\xEF\xB7\x89\xFC\x17\xE4\x89\x82Ra\x01\xA0\x82 \x91R`@Q\x7F\x19\x01\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\x02\x82\x01R`\"\x81\x01\x91\x90\x91R`B\x90 \x86\x81\x14a\x04\x94W`@Q\x7FY?\xCA\xCD\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\x04\x9F\x81\x83\x85a\x12\x91V[a\x04\xA9\x82\x84a\x05sV[P\x7F\x16&\xBA~\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x92PPP[\x93\x92PPPV[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x163\x14a\x05FW`@Q\x7F\xF8}\r\x16\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[_\x80\x80U`@Q\x7F\xBC\xB8\xB8\xFB\xDE\xA8\xAAm\xC4\xAEA!>M\xA8\x1E`Z=\x1AV\xED\x85\x1B\x93U\x18#!\xC0\x91\x90\x91\x90\xA1V[\x80Q\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x84\x16\x91\x16\x14a\x06wW\x80s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x83_\x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14a\x06uW`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x12`$\x82\x01R\x7Finvalid sell token\0\0\0\0\0\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01[`@Q\x80\x91\x03\x90\xFD[\x90[`@Q\x7Fp\xA0\x821\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R0`\x04\x82\x01R_\x90s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x84\x16\x90cp\xA0\x821\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x06\xE1W=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x07\x05\x91\x90a\x1B\xFFV[`@Q\x7Fp\xA0\x821\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R0`\x04\x82\x01R\x90\x91P_\x90s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x84\x16\x90cp\xA0\x821\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x07rW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x07\x96\x91\x90a\x1B\xFFV[\x90P\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x85` \x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14a\x081W`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x11`$\x82\x01R\x7Finvalid buy token\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x06lV[`@\x85\x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x15a\x08\xB3W`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1D`$\x82\x01R\x7Freceiver must be zero address\0\0\0`D\x82\x01R`d\x01a\x06lV[a\x08\xBFa\x01,Ba\x1CCV[\x85`\xA0\x01Qc\xFF\xFF\xFF\xFF\x16\x11\x15a\t2W`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1E`$\x82\x01R\x7Fvalidity too far in the future\0\0`D\x82\x01R`d\x01a\x06lV[\x85``\x01Q\x85`\xC0\x01Q\x14a\t\xA3W`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x0F`$\x82\x01R\x7Finvalid appData\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x06lV[`\xE0\x85\x01Q\x15a\n\x0FW`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x17`$\x82\x01R\x7Ffee amount must be zero\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x06lV[\x7FZ(\xE96;\xB9B\xB69'\0b\xAAk\xB2\x95\xF44\xBC\xDF\xC4,\x97&{\xF0\x03\xF2r\x06\r\xC9\x85a\x01`\x01Q\x14a\n\x9DW`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1D`$\x82\x01R\x7FbuyTokenBalance must be erc20\0\0\0`D\x82\x01R`d\x01a\x06lV[\x7FZ(\xE96;\xB9B\xB69'\0b\xAAk\xB2\x95\xF44\xBC\xDF\xC4,\x97&{\xF0\x03\xF2r\x06\r\xC9\x85a\x01@\x01Q\x14a\x0B+W`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1E`$\x82\x01R\x7FsellTokenBalance must be erc20\0\0`D\x82\x01R`d\x01a\x06lV[``\x85\x01Qa\x0B:\x90\x82a\x1CVV[`\x80\x86\x01Q``\x87\x01Qa\x0BN\x90\x85a\x1CmV[a\x0BX\x91\x90a\x1CVV[\x10\x15a\x0B\xC0W`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x17`$\x82\x01R\x7Freceived amount too low\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x06lV[PPPPPPV[_\x81`@Q` \x01a\x0B\xDA\x91\x90a\x1C\xCCV[`@Q` \x81\x83\x03\x03\x81R\x90`@R\x80Q\x90` \x01 \x90P\x91\x90PV[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x163\x14a\x0CfW`@Q\x7F\xF8}\r\x16\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[_a\x0Csa\x02\x97\x83a\x1D'V[\x90P\x80_\x81\x90UP\x80\x7FQ\x0EJOv\x90|-aX\xB3C\xF7\xC4\xF2\xF5\x97\xDF8[r|&\xE9\xEF\x90\xE7P\x93\xAC\xE1\x9A\x83`@Qa\x0C\xAB\x91\x90a\x1DyV[`@Q\x80\x91\x03\x90\xA2PPV[`@\x80Qa\x01\x80\x81\x01\x82R_\x80\x82R` \x82\x01\x81\x90R\x91\x81\x01\x82\x90R``\x81\x01\x82\x90R`\x80\x81\x01\x82\x90R`\xA0\x81\x01\x82\x90R`\xC0\x81\x01\x82\x90R`\xE0\x81\x01\x82\x90Ra\x01\0\x81\x01\x82\x90Ra\x01 \x81\x01\x82\x90Ra\x01@\x81\x01\x82\x90Ra\x01`\x81\x01\x91\x90\x91R_\x80\x83` \x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16c5^\xFD\xD9\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x87`@\x01Q`@Q\x84c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01a\r\x9E\x93\x92\x91\x90a\x1E:V[`@\x80Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\r\xB8W=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\r\xDC\x91\x90a\x1ErV[`@Q\x7Fp\xA0\x821\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R0`\x04\x82\x01R\x91\x93P\x91P_\x90\x81\x90s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x90cp\xA0\x821\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x0EmW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x0E\x91\x91\x90a\x1B\xFFV[`@Q\x7Fp\xA0\x821\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R0`\x04\x82\x01R\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90cp\xA0\x821\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x0F\x19W=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x0F=\x91\x90a\x1B\xFFV[\x90\x92P\x90P_\x80\x80\x80\x80a\x0FQ\x88\x88a\x1CVV[\x90P_a\x0F^\x8A\x88a\x1CVV[\x90P_\x82\x82\x10\x15a\x10\x0BW\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x96P\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x95Pa\x0F\xD6a\x0F\xBD`\x02\x8Ba\x1E\xC1V[a\x0F\xD1\x84a\x0F\xCC\x8E`\x02a\x1CVV[a\x13FV[a\x13~V[\x94Pa\x10\x01\x85a\x0F\xE6\x81\x8Da\x1CVV[a\x0F\xF0\x90\x85a\x1CCV[a\x0F\xFA\x8C\x8Fa\x1CVV[`\x01a\x13\xCBV[\x93P\x84\x90Pa\x10\x98V[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x96P\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x95Pa\x10na\x10_`\x02\x8Aa\x1E\xC1V[a\x0F\xD1\x85a\x0F\xCC\x8F`\x02a\x1CVV[\x94Pa\x10\x92\x85a\x10~\x81\x8Ea\x1CVV[a\x10\x88\x90\x86a\x1CCV[a\x0F\xFA\x8B\x8Ea\x1CVV[\x93P\x83\x90P[\x8CQ\x81\x10\x15a\x10\xDFWa\x10\xDF`@Q\x80`@\x01`@R\x80`\x17\x81R` \x01\x7Ftraded amount too small\0\0\0\0\0\0\0\0\0\x81RPa\x14&V[`@Q\x80a\x01\x80\x01`@R\x80\x88s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x87s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01_s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x86\x81R` \x01\x85\x81R` \x01a\x11Va\x01,a\x14fV[c\xFF\xFF\xFF\xFF\x16\x81R` \x01\x8E``\x01Q\x81R` \x01_\x81R` \x01\x7F\xF3\xB2wr\x8B?\xEEt\x94\x81\xEB>\x0B;H\x98\r\xBB\xABxe\x8F\xC4\x19\x02\\\xB1n\xEE4gu\x81R` \x01`\x01\x15\x15\x81R` \x01\x7FZ(\xE96;\xB9B\xB69'\0b\xAAk\xB2\x95\xF44\xBC\xDF\xC4,\x97&{\xF0\x03\xF2r\x06\r\xC9\x81R` \x01\x7FZ(\xE96;\xB9B\xB69'\0b\xAAk\xB2\x95\xF44\xBC\xDF\xC4,\x97&{\xF0\x03\xF2r\x06\r\xC9\x81RP\x9BPPPPPPPPPPPP\x91\x90PV[3s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x14a\x12kW`@Q\x7F\xBF\x84\x89w\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x80\x7Fl<\x90$TW\x06\x0Fe\x17x{,K\x8C\xF5\0\xCA\x88\x9D#\x04\xAF\x02\x04;\xD5\xB5\x13\xE3\xB5\x93]PV[\x7Fl<\x90$TW\x06\x0Fe\x17x{,K\x8C\xF5\0\xCA\x88\x9D#\x04\xAF\x02\x04;\xD5\xB5\x13\xE3\xB5\x93\\\x83\x81\x14a\x13@W\x80\x15a\x12\xF2W`@Q\x7F\xDA\xFB\xDD\x1F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[_a\x12\xFC\x84a\x0C\xB7V[\x90Pa\x13\x08\x83\x82a\x14\x87V[a\x13>W`@Q\x7F\xD9\xFF$\xC7\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[P[PPPPV[_\x82\x15a\x13sW\x81a\x13Y`\x01\x85a\x1CmV[a\x13c\x91\x90a\x1E\xC1V[a\x13n\x90`\x01a\x1CCV[a\x13uV[_[\x90P[\x92\x91PPV[_\x81\x83\x10\x15a\x13\xC5Wa\x13\xC5`@Q\x80`@\x01`@R\x80`\x15\x81R` \x01\x7Fsubtraction underflow\0\0\0\0\0\0\0\0\0\0\0\x81RPa\x14&V[P\x90\x03\x90V[_\x80a\x13\xD8\x86\x86\x86a\x15\x99V[\x90P`\x01\x83`\x02\x81\x11\x15a\x13\xEEWa\x13\xEEa\x1E\xD4V[\x14\x80\x15a\x14\nWP_\x84\x80a\x14\x05Wa\x14\x05a\x1E\x94V[\x86\x88\t\x11[\x15a\x14\x1DWa\x14\x1A`\x01\x82a\x1CCV[\x90P[\x95\x94PPPPPV[a\x141C`\x01a\x1CCV[\x81`@Q\x7F\x1F\xE8Pn\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01a\x06l\x92\x91\x90a\x1F\x01V[_\x81\x80a\x14s\x81Ba\x1F\x19V[a\x14}\x91\x90a\x1F;V[a\x13x\x91\x90a\x1FcV[_\x80\x82_\x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x84_\x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x90P_\x83` \x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x85` \x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x90P_\x84``\x01Q\x86``\x01Q\x14\x90P_\x85`\x80\x01Q\x87`\x80\x01Q\x14\x90P_\x86`\xA0\x01Qc\xFF\xFF\xFF\xFF\x16\x88`\xA0\x01Qc\xFF\xFF\xFF\xFF\x16\x14\x90P_\x87a\x01\0\x01Q\x89a\x01\0\x01Q\x14\x90P_\x88a\x01 \x01Q\x15\x15\x8Aa\x01 \x01Q\x15\x15\x14\x90P\x86\x80\x15a\x15^WP\x85[\x80\x15a\x15gWP\x84[\x80\x15a\x15pWP\x83[\x80\x15a\x15yWP\x82[\x80\x15a\x15\x82WP\x81[\x80\x15a\x15\x8BWP\x80[\x9A\x99PPPPPPPPPPV[_\x80\x80\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x85\x87\t\x85\x87\x02\x92P\x82\x81\x10\x83\x82\x03\x03\x91PP\x80_\x03a\x15\xEFW\x83\x82\x81a\x15\xE5Wa\x15\xE5a\x1E\x94V[\x04\x92PPPa\x04\xD0V[\x80\x84\x11a\x16XW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x15`$\x82\x01R\x7FMath: mulDiv overflow\0\0\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x06lV[_\x84\x86\x88\t`\x02`\x01\x87\x19\x81\x01\x88\x16\x97\x88\x90\x04`\x03\x81\x02\x83\x18\x80\x82\x02\x84\x03\x02\x80\x82\x02\x84\x03\x02\x80\x82\x02\x84\x03\x02\x80\x82\x02\x84\x03\x02\x80\x82\x02\x84\x03\x02\x90\x81\x02\x90\x92\x03\x90\x91\x02_\x88\x90\x03\x88\x90\x04\x90\x91\x01\x85\x83\x11\x90\x94\x03\x93\x90\x93\x02\x93\x03\x94\x90\x94\x04\x91\x90\x91\x17\x02\x94\x93PPPPV[_\x80_`@\x84\x86\x03\x12\x15a\x16\xD1W_\x80\xFD[\x835\x92P` \x84\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a\x16\xEFW_\x80\xFD[\x81\x86\x01\x91P\x86`\x1F\x83\x01\x12a\x17\x02W_\x80\xFD[\x815\x81\x81\x11\x15a\x17\x10W_\x80\xFD[\x87` \x82\x85\x01\x01\x11\x15a\x17!W_\x80\xFD[` \x83\x01\x94P\x80\x93PPPP\x92P\x92P\x92V[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`A`\x04R`$_\xFD[`@Q`\x80\x81\x01g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x82\x82\x10\x17\x15a\x17\x84Wa\x17\x84a\x174V[`@R\x90V[`@Qa\x01\x80\x81\x01g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x82\x82\x10\x17\x15a\x17\x84Wa\x17\x84a\x174V[`@Q`\x1F\x82\x01\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x16\x81\x01g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x82\x82\x10\x17\x15a\x17\xF5Wa\x17\xF5a\x174V[`@R\x91\x90PV[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x16\x81\x14a\x18\x1EW_\x80\xFD[PV[_`\x80\x82\x84\x03\x12\x15a\x181W_\x80\xFD[a\x189a\x17aV[\x90P\x815\x81R` \x80\x83\x015a\x18N\x81a\x17\xFDV[\x82\x82\x01R`@\x83\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a\x18lW_\x80\xFD[\x81\x85\x01\x91P\x85`\x1F\x83\x01\x12a\x18\x7FW_\x80\xFD[\x815\x81\x81\x11\x15a\x18\x91Wa\x18\x91a\x174V[a\x18\xC1\x84\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`\x1F\x84\x01\x16\x01a\x17\xAEV[\x91P\x80\x82R\x86\x84\x82\x85\x01\x01\x11\x15a\x18\xD6W_\x80\xFD[\x80\x84\x84\x01\x85\x84\x017_\x84\x82\x84\x01\x01RP\x80`@\x85\x01RPPP``\x82\x015``\x82\x01R\x92\x91PPV[\x805a\x19\n\x81a\x17\xFDV[\x91\x90PV[\x805c\xFF\xFF\xFF\xFF\x81\x16\x81\x14a\x19\nW_\x80\xFD[\x805\x80\x15\x15\x81\x14a\x19\nW_\x80\xFD[_a\x01\x80\x82\x84\x03\x12\x15a\x19BW_\x80\xFD[a\x19Ja\x17\x8AV[\x90Pa\x19U\x82a\x18\xFFV[\x81Ra\x19c` \x83\x01a\x18\xFFV[` \x82\x01Ra\x19t`@\x83\x01a\x18\xFFV[`@\x82\x01R``\x82\x015``\x82\x01R`\x80\x82\x015`\x80\x82\x01Ra\x19\x99`\xA0\x83\x01a\x19\x0FV[`\xA0\x82\x01R`\xC0\x82\x015`\xC0\x82\x01R`\xE0\x82\x015`\xE0\x82\x01Ra\x01\0\x80\x83\x015\x81\x83\x01RPa\x01 a\x19\xCC\x81\x84\x01a\x19\"V[\x90\x82\x01Ra\x01@\x82\x81\x015\x90\x82\x01Ra\x01`\x91\x82\x015\x91\x81\x01\x91\x90\x91R\x91\x90PV[_\x80a\x01\xA0\x83\x85\x03\x12\x15a\x1A\0W_\x80\xFD[\x825g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x1A\x16W_\x80\xFD[a\x1A\"\x85\x82\x86\x01a\x18!V[\x92PPa\x1A2\x84` \x85\x01a\x191V[\x90P\x92P\x92\x90PV[_` \x82\x84\x03\x12\x15a\x1AKW_\x80\xFD[\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x1AaW_\x80\xFD[a\x1Am\x84\x82\x85\x01a\x18!V[\x94\x93PPPPV[_` \x82\x84\x03\x12\x15a\x1A\x85W_\x80\xFD[\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x1A\x9BW_\x80\xFD[\x82\x01`\x80\x81\x85\x03\x12\x15a\x04\xD0W_\x80\xFD[\x81Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81Ra\x01\x80\x81\x01` \x83\x01Qa\x1A\xF2` \x84\x01\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90RV[P`@\x83\x01Qa\x1B\x1A`@\x84\x01\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90RV[P``\x83\x01Q``\x83\x01R`\x80\x83\x01Q`\x80\x83\x01R`\xA0\x83\x01Qa\x1BF`\xA0\x84\x01\x82c\xFF\xFF\xFF\xFF\x16\x90RV[P`\xC0\x83\x01Q`\xC0\x83\x01R`\xE0\x83\x01Q`\xE0\x83\x01Ra\x01\0\x80\x84\x01Q\x81\x84\x01RPa\x01 \x80\x84\x01Qa\x1B{\x82\x85\x01\x82\x15\x15\x90RV[PPa\x01@\x83\x81\x01Q\x90\x83\x01Ra\x01`\x92\x83\x01Q\x92\x90\x91\x01\x91\x90\x91R\x90V[_` \x82\x84\x03\x12\x15a\x1B\xAAW_\x80\xFD[P5\x91\x90PV[_\x80a\x01\xA0\x83\x85\x03\x12\x15a\x1B\xC3W_\x80\xFD[a\x1B\xCD\x84\x84a\x191V[\x91Pa\x01\x80\x83\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x1B\xE9W_\x80\xFD[a\x1B\xF5\x85\x82\x86\x01a\x18!V[\x91PP\x92P\x92\x90PV[_` \x82\x84\x03\x12\x15a\x1C\x0FW_\x80\xFD[PQ\x91\x90PV[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x11`\x04R`$_\xFD[\x80\x82\x01\x80\x82\x11\x15a\x13xWa\x13xa\x1C\x16V[\x80\x82\x02\x81\x15\x82\x82\x04\x84\x14\x17a\x13xWa\x13xa\x1C\x16V[\x81\x81\x03\x81\x81\x11\x15a\x13xWa\x13xa\x1C\x16V[_\x81Q\x80\x84R\x80` \x84\x01` \x86\x01^_` \x82\x86\x01\x01R` \x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`\x1F\x83\x01\x16\x85\x01\x01\x91PP\x92\x91PPV[` \x81R\x81Q` \x82\x01Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF` \x83\x01Q\x16`@\x82\x01R_`@\x83\x01Q`\x80``\x84\x01Ra\x1D\x11`\xA0\x84\x01\x82a\x1C\x80V[\x90P``\x84\x01Q`\x80\x84\x01R\x80\x91PP\x92\x91PPV[_a\x13x6\x83a\x18!V[\x81\x83R\x81\x81` \x85\x017P_` \x82\x84\x01\x01R_` \x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`\x1F\x84\x01\x16\x84\x01\x01\x90P\x92\x91PPV[` \x81R\x815` \x82\x01R_` \x83\x015a\x1D\x93\x81a\x17\xFDV[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x16`@\x84\x01RP`@\x83\x015\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE1\x846\x03\x01\x81\x12a\x1D\xE4W_\x80\xFD[\x83\x01` \x81\x01\x905g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x1E\0W_\x80\xFD[\x806\x03\x82\x13\x15a\x1E\x0EW_\x80\xFD[`\x80``\x85\x01Ra\x1E#`\xA0\x85\x01\x82\x84a\x1D2V[\x91PP``\x84\x015`\x80\x84\x01R\x80\x91PP\x92\x91PPV[_s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x86\x16\x83R\x80\x85\x16` \x84\x01RP```@\x83\x01Ra\x14\x1D``\x83\x01\x84a\x1C\x80V[_\x80`@\x83\x85\x03\x12\x15a\x1E\x83W_\x80\xFD[PP\x80Q` \x90\x91\x01Q\x90\x92\x90\x91PV[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x12`\x04R`$_\xFD[_\x82a\x1E\xCFWa\x1E\xCFa\x1E\x94V[P\x04\x90V[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`!`\x04R`$_\xFD[\x82\x81R`@` \x82\x01R_a\x1Am`@\x83\x01\x84a\x1C\x80V[_c\xFF\xFF\xFF\xFF\x80\x84\x16\x80a\x1F/Wa\x1F/a\x1E\x94V[\x92\x16\x91\x90\x91\x04\x92\x91PPV[c\xFF\xFF\xFF\xFF\x81\x81\x16\x83\x82\x16\x02\x80\x82\x16\x91\x90\x82\x81\x14a\x1F[Wa\x1F[a\x1C\x16V[PP\x92\x91PPV[c\xFF\xFF\xFF\xFF\x81\x81\x16\x83\x82\x16\x01\x90\x80\x82\x11\x15a\x1F\x80Wa\x1F\x80a\x1C\x16V[P\x92\x91PPV\xFE\xA2dipfsX\"\x12 \xE3\xFB\"\x8BR]\x90\xB9B\xC7\xE5\x8F\xE2\xE2\x03J\x17\xBD%\x8C\x08/\xD4w@\xE7d\xA7\xBEE\xBA\xC6dsolcC\0\x08\x19\x003\xA2dipfsX\"\x12 \x11\x90\xCFB\xF9\x89\xCE\xE2?\x12Y|\x8C\x1E\x9D\xAA\xB6\xD8\xC8\x16Q3I\xC3\xCE\x7F\xD2)\xCA\xE5\xB0\xFFdsolcC\0\x08\x19\x003",
    );
    /// The runtime bytecode of the contract, as deployed on the network.
    ///
    /// ```text
    ///0x608060405234801561000f575f80fd5b506004361061009f575f3560e01c806337ebdf5011610072578063666e1b3911610058578063666e1b391461014f578063ab221a7614610184578063b5c5f672146101ab575f80fd5b806337ebdf50146101295780635b5d9ee61461013c575f80fd5b80630efe6a8b146100a357806322b155c6146100b857806326e0a196146100f55780632791056514610116575b5f80fd5b6100b66100b13660046111ea565b6101be565b005b6100cb6100c6366004611261565b6102a3565b60405173ffffffffffffffffffffffffffffffffffffffff90911681526020015b60405180910390f35b6101086101033660046112fb565b61045f565b6040516100ec9291906114fd565b6100b6610124366004611527565b610797565b6100cb610137366004611549565b61082f565b6100b661014a366004611591565b610a01565b6100cb61015d366004611527565b5f6020819052908152604090205473ffffffffffffffffffffffffffffffffffffffff1681565b6100cb7f000000000000000000000000000000000000000000000000000000000000000081565b6100b66101b93660046111ea565b610b17565b61024f3384848673ffffffffffffffffffffffffffffffffffffffff16630dfe16816040518163ffffffff1660e01b8152600401602060405180830381865afa15801561020d573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906102319190611617565b73ffffffffffffffffffffffffffffffffffffffff16929190610c46565b61029e3384838673ffffffffffffffffffffffffffffffffffffffff1663d21220a76040518163ffffffff1660e01b8152600401602060405180830381865afa15801561020d573d5f803e3d5ffd5b505050565b5f33807f00000000000000000000000000000000000000000000000000000000000000008c8b6040516102d5906111b9565b73ffffffffffffffffffffffffffffffffffffffff9384168152918316602083015290911660408201526060018190604051809103905ff590508015801561031f573d5f803e3d5ffd5b506040805173ffffffffffffffffffffffffffffffffffffffff8e811682528c81166020830152929450828416928516917f6707255b2c5ca81220b2f3e408a269cb83baa6aa7e5e37aa1756883a6cdf06f1910160405180910390a373ffffffffffffffffffffffffffffffffffffffff8281165f90815260208190526040902080547fffffffffffffffffffffffff0000000000000000000000000000000000000000169183169190911790556103d8828b8a6101be565b5f60405180608001604052808981526020018873ffffffffffffffffffffffffffffffffffffffff16815260200187878080601f0160208091040260200160405190810160405280939291908181526020018383808284375f9201919091525050509082525060200185905290506104508382610cdb565b50509998505050505050505050565b60408051610180810182525f80825260208201819052918101829052606081018290526080810182905260a0810182905260c0810182905260e081018290526101008101829052610120810182905261014081018290526101608101919091526060306104cf6020890189611527565b73ffffffffffffffffffffffffffffffffffffffff1614610551576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601a60248201527f63616e206f6e6c792068616e646c65206f776e206f726465727300000000000060448201526064015b60405180910390fd5b5f61055f6040890189611632565b81019061056c919061175c565b90508873ffffffffffffffffffffffffffffffffffffffff1663eec50b976040518163ffffffff1660e01b8152600401602060405180830381865afa1580156105b7573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906105db919061185a565b6040517fb09aaaca00000000000000000000000000000000000000000000000000000000815273ffffffffffffffffffffffffffffffffffffffff8b169063b09aaaca9061062d9085906004016118c3565b602060405180830381865afa158015610648573d5f803e3d5ffd5b505050506040513d601f19601f8201168201806040525081019061066c919061185a565b146106d3576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601a60248201527f696e76616c69642074726164696e6720706172616d65746572730000000000006044820152606401610548565b6040517fe3e6f5b200000000000000000000000000000000000000000000000000000000815273ffffffffffffffffffffffffffffffffffffffff8a169063e3e6f5b2906107259084906004016118c3565b61018060405180830381865afa158015610741573d5f803e3d5ffd5b505050506040513d601f19601f8201168201806040525081019061076591906118f7565b9250828160405160200161077a9291906119b3565b604051602081830303815290604052915050965096945050505050565b73ffffffffffffffffffffffffffffffffffffffff8082165f9081526020819052604090205482911633146108225773ffffffffffffffffffffffffffffffffffffffff8181165f90815260208190526040908190205490517f68bafff800000000000000000000000000000000000000000000000000000000815291166004820152602401610548565b61082b82610e05565b5050565b5f807fff000000000000000000000000000000000000000000000000000000000000003073ffffffffffffffffffffffffffffffffffffffff8716604051610879602082016111b9565b8181037fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe09081018352601f90910116604081815273ffffffffffffffffffffffffffffffffffffffff7f000000000000000000000000000000000000000000000000000000000000000081166020840152808b169183019190915288166060820152608001604080517fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe08184030181529082905261093a92916020016119eb565b604051602081830303815290604052805190602001206040516020016109c294939291907fff0000000000000000000000000000000000000000000000000000000000000094909416845260609290921b7fffffffffffffffffffffffffffffffffffffffff0000000000000000000000001660018401526015830152603582015260550190565b604080518083037fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0018152919052805160209091012095945050505050565b73ffffffffffffffffffffffffffffffffffffffff8087165f908152602081905260409020548791163314610a8c5773ffffffffffffffffffffffffffffffffffffffff8181165f90815260208190526040908190205490517f68bafff800000000000000000000000000000000000000000000000000000000815291166004820152602401610548565b5f60405180608001604052808881526020018773ffffffffffffffffffffffffffffffffffffffff16815260200186868080601f0160208091040260200160405190810160405280939291908181526020018383808284375f920191909152505050908252506020018490529050610b0388610e05565b610b0d8882610cdb565b5050505050505050565b73ffffffffffffffffffffffffffffffffffffffff8084165f908152602081905260409020548491163314610ba25773ffffffffffffffffffffffffffffffffffffffff8181165f90815260208190526040908190205490517f68bafff800000000000000000000000000000000000000000000000000000000815291166004820152602401610548565b610bf18433858773ffffffffffffffffffffffffffffffffffffffff16630dfe16816040518163ffffffff1660e01b8152600401602060405180830381865afa15801561020d573d5f803e3d5ffd5b610c408433848773ffffffffffffffffffffffffffffffffffffffff1663d21220a76040518163ffffffff1660e01b8152600401602060405180830381865afa15801561020d573d5f803e3d5ffd5b50505050565b6040805173ffffffffffffffffffffffffffffffffffffffff85811660248301528416604482015260648082018490528251808303909101815260849091019091526020810180517bffffffffffffffffffffffffffffffffffffffffffffffffffffffff167f23b872dd00000000000000000000000000000000000000000000000000000000179052610c40908590610ea3565b6040517fc5f3d25400000000000000000000000000000000000000000000000000000000815273ffffffffffffffffffffffffffffffffffffffff83169063c5f3d25490610d2d9084906004016118c3565b5f604051808303815f87803b158015610d44575f80fd5b505af1158015610d56573d5f803e3d5ffd5b5050604080516060810182523081525f6020808301829052835191955073ffffffffffffffffffffffffffffffffffffffff881694507f2cceac5555b0ca45a3744ced542f54b56ad2eb45e521962372eef212a2cbf36193830191610dbd918891016118c3565b604080517fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0818403018152918152915251610df891906119ff565b60405180910390a2505050565b8073ffffffffffffffffffffffffffffffffffffffff166317700f016040518163ffffffff1660e01b81526004015f604051808303815f87803b158015610e4a575f80fd5b505af1158015610e5c573d5f803e3d5ffd5b505060405173ffffffffffffffffffffffffffffffffffffffff841692507fc75bf4f03c02fab9414a7d7a54048c0486722bc72f33ad924709a0593608ad2791505f90a250565b5f610f04826040518060400160405280602081526020017f5361666545524332303a206c6f772d6c6576656c2063616c6c206661696c65648152508573ffffffffffffffffffffffffffffffffffffffff16610fb09092919063ffffffff16565b905080515f1480610f24575080806020019051810190610f249190611a43565b61029e576040517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152602a60248201527f5361666545524332303a204552433230206f7065726174696f6e20646964206e60448201527f6f742073756363656564000000000000000000000000000000000000000000006064820152608401610548565b6060610fbe84845f85610fc6565b949350505050565b606082471015611058576040517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152602660248201527f416464726573733a20696e73756666696369656e742062616c616e636520666f60448201527f722063616c6c00000000000000000000000000000000000000000000000000006064820152608401610548565b5f808673ffffffffffffffffffffffffffffffffffffffff1685876040516110809190611a5c565b5f6040518083038185875af1925050503d805f81146110ba576040519150601f19603f3d011682016040523d82523d5f602084013e6110bf565b606091505b50915091506110d0878383876110db565b979650505050505050565b606083156111705782515f036111695773ffffffffffffffffffffffffffffffffffffffff85163b611169576040517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601d60248201527f416464726573733a2063616c6c20746f206e6f6e2d636f6e74726163740000006044820152606401610548565b5081610fbe565b610fbe83838151156111855781518083602001fd5b806040517f08c379a00000000000000000000000000000000000000000000000000000000081526004016105489190611a67565b61267880611a7a83390190565b73ffffffffffffffffffffffffffffffffffffffff811681146111e7575f80fd5b50565b5f805f606084860312156111fc575f80fd5b8335611207816111c6565b95602085013595506040909401359392505050565b5f8083601f84011261122c575f80fd5b50813567ffffffffffffffff811115611243575f80fd5b60208301915083602082850101111561125a575f80fd5b9250929050565b5f805f805f805f805f6101008a8c03121561127a575f80fd5b8935611285816111c6565b985060208a0135975060408a013561129c816111c6565b965060608a0135955060808a0135945060a08a01356112ba816111c6565b935060c08a013567ffffffffffffffff8111156112d5575f80fd5b6112e18c828d0161121c565b9a9d999c50979a9699959894979660e00135949350505050565b5f805f805f8060808789031215611310575f80fd5b863561131b816111c6565b9550602087013567ffffffffffffffff80821115611337575f80fd5b908801906060828b03121561134a575f80fd5b9095506040880135908082111561135f575f80fd5b61136b8a838b0161121c565b90965094506060890135915080821115611383575f80fd5b818901915089601f830112611396575f80fd5b8135818111156113a4575f80fd5b8a60208260051b85010111156113b8575f80fd5b6020830194508093505050509295509295509295565b805173ffffffffffffffffffffffffffffffffffffffff168252602081015161140f602084018273ffffffffffffffffffffffffffffffffffffffff169052565b506040810151611437604084018273ffffffffffffffffffffffffffffffffffffffff169052565b50606081015160608301526080810151608083015260a081015161146360a084018263ffffffff169052565b5060c081015160c083015260e081015160e0830152610100808201518184015250610120808201516114988285018215159052565b5050610140818101519083015261016090810151910152565b5f81518084528060208401602086015e5f6020828601015260207fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f83011685010191505092915050565b5f6101a061150b83866113ce565b8061018084015261151e818401856114b1565b95945050505050565b5f60208284031215611537575f80fd5b8135611542816111c6565b9392505050565b5f805f6060848603121561155b575f80fd5b8335611566816111c6565b92506020840135611576816111c6565b91506040840135611586816111c6565b809150509250925092565b5f805f805f8060a087890312156115a6575f80fd5b86356115b1816111c6565b95506020870135945060408701356115c8816111c6565b9350606087013567ffffffffffffffff8111156115e3575f80fd5b6115ef89828a0161121c565b979a9699509497949695608090950135949350505050565b8051611612816111c6565b919050565b5f60208284031215611627575f80fd5b8151611542816111c6565b5f8083357fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe1843603018112611665575f80fd5b83018035915067ffffffffffffffff82111561167f575f80fd5b60200191503681900382131561125a575f80fd5b7f4e487b71000000000000000000000000000000000000000000000000000000005f52604160045260245ffd5b6040516080810167ffffffffffffffff811182821017156116e3576116e3611693565b60405290565b604051610180810167ffffffffffffffff811182821017156116e3576116e3611693565b604051601f82017fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe016810167ffffffffffffffff8111828210171561175457611754611693565b604052919050565b5f602080838503121561176d575f80fd5b823567ffffffffffffffff80821115611784575f80fd5b9084019060808287031215611797575f80fd5b61179f6116c0565b82358152838301356117b0816111c6565b818501526040830135828111156117c5575f80fd5b8301601f810188136117d5575f80fd5b8035838111156117e7576117e7611693565b611817867fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f8401160161170d565b9350808452888682840101111561182c575f80fd5b80868301878601375f8682860101525050816040820152606083013560608201528094505050505092915050565b5f6020828403121561186a575f80fd5b5051919050565b8051825273ffffffffffffffffffffffffffffffffffffffff60208201511660208301525f6040820151608060408501526118af60808501826114b1565b606093840151949093019390935250919050565b602081525f6115426020830184611871565b805163ffffffff81168114611612575f80fd5b80518015158114611612575f80fd5b5f6101808284031215611908575f80fd5b6119106116e9565b61191983611607565b815261192760208401611607565b602082015261193860408401611607565b6040820152606083015160608201526080830151608082015261195d60a084016118d5565b60a082015260c083015160c082015260e083015160e08201526101008084015181830152506101206119908185016118e8565b908201526101408381015190820152610160928301519281019290925250919050565b5f6101a06119c183866113ce565b8061018084015261151e81840185611871565b5f81518060208401855e5f93019283525090919050565b5f610fbe6119f983866119d4565b846119d4565b6020815273ffffffffffffffffffffffffffffffffffffffff8251166020820152602082015160408201525f6040830151606080840152610fbe60808401826114b1565b5f60208284031215611a53575f80fd5b611542826118e8565b5f61154282846119d4565b602081525f61154260208301846114b156fe610120604052348015610010575f80fd5b5060405161267838038061267883398101604081905261002f9161052f565b6001600160a01b03831660808190526040805163f698da2560e01b8152905163f698da259160048082019260209290919082900301815f875af1158015610078573d5f803e3d5ffd5b505050506040513d601f19601f8201168201806040525081019061009c9190610579565b610100526100aa823361015f565b6100b4813361015f565b336001600160a01b031660e0816001600160a01b0316815250505f836001600160a01b0316639b552cc26040518163ffffffff1660e01b81526004016020604051808303815f875af115801561010c573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906101309190610590565b905061013c838261015f565b610146828261015f565b506001600160a01b0391821660a0521660c0525061061c565b6101746001600160a01b038316825f19610178565b5050565b8015806101f05750604051636eb1769f60e11b81523060048201526001600160a01b03838116602483015284169063dd62ed3e90604401602060405180830381865afa1580156101ca573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906101ee9190610579565b155b6102675760405162461bcd60e51b815260206004820152603660248201527f5361666545524332303a20617070726f76652066726f6d206e6f6e2d7a65726f60448201527f20746f206e6f6e2d7a65726f20616c6c6f77616e63650000000000000000000060648201526084015b60405180910390fd5b604080516001600160a01b038416602482015260448082018490528251808303909101815260649091019091526020810180516001600160e01b0390811663095ea7b360e01b179091526102bd9185916102c216565b505050565b6040805180820190915260208082527f5361666545524332303a206c6f772d6c6576656c2063616c6c206661696c6564908201525f9061030e906001600160a01b03851690849061038d565b905080515f148061032e57508080602001905181019061032e91906105b2565b6102bd5760405162461bcd60e51b815260206004820152602a60248201527f5361666545524332303a204552433230206f7065726174696f6e20646964206e6044820152691bdd081cdd58d8d9595960b21b606482015260840161025e565b606061039b84845f856103a3565b949350505050565b6060824710156104045760405162461bcd60e51b815260206004820152602660248201527f416464726573733a20696e73756666696369656e742062616c616e636520666f6044820152651c8818d85b1b60d21b606482015260840161025e565b5f80866001600160a01b0316858760405161041f91906105d1565b5f6040518083038185875af1925050503d805f8114610459576040519150601f19603f3d011682016040523d82523d5f602084013e61045e565b606091505b5090925090506104708783838761047b565b979650505050505050565b606083156104e95782515f036104e2576001600160a01b0385163b6104e25760405162461bcd60e51b815260206004820152601d60248201527f416464726573733a2063616c6c20746f206e6f6e2d636f6e7472616374000000604482015260640161025e565b508161039b565b61039b83838151156104fe5781518083602001fd5b8060405162461bcd60e51b815260040161025e91906105e7565b6001600160a01b038116811461052c575f80fd5b50565b5f805f60608486031215610541575f80fd5b835161054c81610518565b602085015190935061055d81610518565b604085015190925061056e81610518565b809150509250925092565b5f60208284031215610589575f80fd5b5051919050565b5f602082840312156105a0575f80fd5b81516105ab81610518565b9392505050565b5f602082840312156105c2575f80fd5b815180151581146105ab575f80fd5b5f82518060208501845e5f920191825250919050565b602081525f82518060208401528060208501604085015e5f604082850101526040601f19601f83011684010191505092915050565b60805160a05160c05160e05161010051611fbd6106bb5f395f81816102db015261042b01525f8181610236015281816104d90152610bf901525f81816102b40152818161059901528181610d5c01528181610ebf01528181610f8e015261100d01525f81816101380152818161057701528181610d3b01528181610e2801528181610f6b015261103001525f818161032201526112140152611fbd5ff3fe608060405234801561000f575f80fd5b506004361061012f575f3560e01c8063b09aaaca116100ad578063e3e6f5b21161007d578063eec50b9711610063578063eec50b9714610344578063f14fcbc81461034c578063ff2dbc9814610203575f80fd5b8063e3e6f5b2146102fd578063e516715b1461031d575f80fd5b8063b09aaaca14610289578063c5f3d2541461029c578063d21220a7146102af578063d25e0cb6146102d6575f80fd5b80631c7de94111610102578063481c6a75116100e8578063481c6a7514610231578063981a160b14610258578063a029a8d414610276575f80fd5b80631c7de941146102035780633e706e321461020a575f80fd5b80630dfe1681146101335780631303a484146101845780631626ba7e146101b557806317700f01146101f9575b5f80fd5b61015a7f000000000000000000000000000000000000000000000000000000000000000081565b60405173ffffffffffffffffffffffffffffffffffffffff90911681526020015b60405180910390f35b7f6c3c90245457060f6517787b2c4b8cf500ca889d2304af02043bd5b513e3b5935c5b60405190815260200161017b565b6101c86101c33660046116bf565b61035f565b6040517fffffffff00000000000000000000000000000000000000000000000000000000909116815260200161017b565b6102016104d7565b005b6101a75f81565b6101a77f6c3c90245457060f6517787b2c4b8cf500ca889d2304af02043bd5b513e3b59381565b61015a7f000000000000000000000000000000000000000000000000000000000000000081565b61026161012c81565b60405163ffffffff909116815260200161017b565b6102016102843660046119ee565b610573565b6101a7610297366004611a3b565b610bc8565b6102016102aa366004611a75565b610bf7565b61015a7f000000000000000000000000000000000000000000000000000000000000000081565b6101a77f000000000000000000000000000000000000000000000000000000000000000081565b61031061030b366004611a3b565b610cb7565b60405161017b9190611aac565b61015a7f000000000000000000000000000000000000000000000000000000000000000081565b6101a75f5481565b61020161035a366004611b9a565b6111fc565b5f808061036e84860186611bb1565b915091505f5461037d82610bc8565b146103b4576040517ff1a6789000000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0820180517fd5a25ba2e97094ad7d83dc28a6572da797d6b3e7fc6663bd93efb789fc17e48982526101a0822091526040517f190100000000000000000000000000000000000000000000000000000000000081527f00000000000000000000000000000000000000000000000000000000000000006002820152602281019190915260429020868114610494576040517f593fcacd00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b61049f818385611291565b6104a98284610573565b507f1626ba7e00000000000000000000000000000000000000000000000000000000925050505b9392505050565b7f000000000000000000000000000000000000000000000000000000000000000073ffffffffffffffffffffffffffffffffffffffff163314610546576040517ff87d0d1600000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b5f8080556040517fbcb8b8fbdea8aa6dc4ae41213e4da81e605a3d1a56ed851b9355182321c091909190a1565b80517f0000000000000000000000000000000000000000000000000000000000000000907f00000000000000000000000000000000000000000000000000000000000000009073ffffffffffffffffffffffffffffffffffffffff808416911614610677578073ffffffffffffffffffffffffffffffffffffffff16835f015173ffffffffffffffffffffffffffffffffffffffff1614610675576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601260248201527f696e76616c69642073656c6c20746f6b656e000000000000000000000000000060448201526064015b60405180910390fd5b905b6040517f70a082310000000000000000000000000000000000000000000000000000000081523060048201525f9073ffffffffffffffffffffffffffffffffffffffff8416906370a0823190602401602060405180830381865afa1580156106e1573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906107059190611bff565b6040517f70a082310000000000000000000000000000000000000000000000000000000081523060048201529091505f9073ffffffffffffffffffffffffffffffffffffffff8416906370a0823190602401602060405180830381865afa158015610772573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906107969190611bff565b90508273ffffffffffffffffffffffffffffffffffffffff16856020015173ffffffffffffffffffffffffffffffffffffffff1614610831576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601160248201527f696e76616c69642062757920746f6b656e000000000000000000000000000000604482015260640161066c565b604085015173ffffffffffffffffffffffffffffffffffffffff16156108b3576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601d60248201527f7265636569766572206d757374206265207a65726f2061646472657373000000604482015260640161066c565b6108bf61012c42611c43565b8560a0015163ffffffff161115610932576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601e60248201527f76616c696469747920746f6f2066617220696e20746865206675747572650000604482015260640161066c565b85606001518560c00151146109a3576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152600f60248201527f696e76616c696420617070446174610000000000000000000000000000000000604482015260640161066c565b60e085015115610a0f576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601760248201527f66656520616d6f756e74206d757374206265207a65726f000000000000000000604482015260640161066c565b7f5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc985610160015114610a9d576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601d60248201527f627579546f6b656e42616c616e6365206d757374206265206572633230000000604482015260640161066c565b7f5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc985610140015114610b2b576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601e60248201527f73656c6c546f6b656e42616c616e6365206d7573742062652065726332300000604482015260640161066c565b6060850151610b3a9082611c56565b60808601516060870151610b4e9085611c6d565b610b589190611c56565b1015610bc0576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601760248201527f726563656976656420616d6f756e7420746f6f206c6f77000000000000000000604482015260640161066c565b505050505050565b5f81604051602001610bda9190611ccc565b604051602081830303815290604052805190602001209050919050565b7f000000000000000000000000000000000000000000000000000000000000000073ffffffffffffffffffffffffffffffffffffffff163314610c66576040517ff87d0d1600000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b5f610c7361029783611d27565b9050805f81905550807f510e4a4f76907c2d6158b343f7c4f2f597df385b727c26e9ef90e75093ace19a83604051610cab9190611d79565b60405180910390a25050565b60408051610180810182525f80825260208201819052918101829052606081018290526080810182905260a0810182905260c0810182905260e081018290526101008101829052610120810182905261014081018290526101608101919091525f80836020015173ffffffffffffffffffffffffffffffffffffffff1663355efdd97f00000000000000000000000000000000000000000000000000000000000000007f000000000000000000000000000000000000000000000000000000000000000087604001516040518463ffffffff1660e01b8152600401610d9e93929190611e3a565b6040805180830381865afa158015610db8573d5f803e3d5ffd5b505050506040513d601f19601f82011682018060405250810190610ddc9190611e72565b6040517f70a0823100000000000000000000000000000000000000000000000000000000815230600482015291935091505f90819073ffffffffffffffffffffffffffffffffffffffff7f000000000000000000000000000000000000000000000000000000000000000016906370a0823190602401602060405180830381865afa158015610e6d573d5f803e3d5ffd5b505050506040513d601f19601f82011682018060405250810190610e919190611bff565b6040517f70a082310000000000000000000000000000000000000000000000000000000081523060048201527f000000000000000000000000000000000000000000000000000000000000000073ffffffffffffffffffffffffffffffffffffffff16906370a0823190602401602060405180830381865afa158015610f19573d5f803e3d5ffd5b505050506040513d601f19601f82011682018060405250810190610f3d9190611bff565b90925090505f80808080610f518888611c56565b90505f610f5e8a88611c56565b90505f8282101561100b577f000000000000000000000000000000000000000000000000000000000000000096507f00000000000000000000000000000000000000000000000000000000000000009550610fd6610fbd60028b611ec1565b610fd184610fcc8e6002611c56565b611346565b61137e565b945061100185610fe6818d611c56565b610ff09085611c43565b610ffa8c8f611c56565b60016113cb565b9350849050611098565b7f000000000000000000000000000000000000000000000000000000000000000096507f0000000000000000000000000000000000000000000000000000000000000000955061106e61105f60028a611ec1565b610fd185610fcc8f6002611c56565b94506110928561107e818e611c56565b6110889086611c43565b610ffa8b8e611c56565b93508390505b8c518110156110df576110df6040518060400160405280601781526020017f74726164656420616d6f756e7420746f6f20736d616c6c000000000000000000815250611426565b6040518061018001604052808873ffffffffffffffffffffffffffffffffffffffff1681526020018773ffffffffffffffffffffffffffffffffffffffff1681526020015f73ffffffffffffffffffffffffffffffffffffffff16815260200186815260200185815260200161115661012c611466565b63ffffffff1681526020018e6060015181526020015f81526020017ff3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee34677581526020016001151581526020017f5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc981526020017f5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc98152509b505050505050505050505050919050565b3373ffffffffffffffffffffffffffffffffffffffff7f0000000000000000000000000000000000000000000000000000000000000000161461126b576040517fbf84897700000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b807f6c3c90245457060f6517787b2c4b8cf500ca889d2304af02043bd5b513e3b5935d50565b7f6c3c90245457060f6517787b2c4b8cf500ca889d2304af02043bd5b513e3b5935c8381146113405780156112f2576040517fdafbdd1f00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b5f6112fc84610cb7565b90506113088382611487565b61133e576040517fd9ff24c700000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b505b50505050565b5f82156113735781611359600185611c6d565b6113639190611ec1565b61136e906001611c43565b611375565b5f5b90505b92915050565b5f818310156113c5576113c56040518060400160405280601581526020017f7375627472616374696f6e20756e646572666c6f770000000000000000000000815250611426565b50900390565b5f806113d8868686611599565b905060018360028111156113ee576113ee611ed4565b14801561140a57505f848061140557611405611e94565b868809115b1561141d5761141a600182611c43565b90505b95945050505050565b611431436001611c43565b816040517f1fe8506e00000000000000000000000000000000000000000000000000000000815260040161066c929190611f01565b5f81806114738142611f19565b61147d9190611f3b565b6113789190611f63565b5f80825f015173ffffffffffffffffffffffffffffffffffffffff16845f015173ffffffffffffffffffffffffffffffffffffffff161490505f836020015173ffffffffffffffffffffffffffffffffffffffff16856020015173ffffffffffffffffffffffffffffffffffffffff161490505f846060015186606001511490505f856080015187608001511490505f8660a0015163ffffffff168860a0015163ffffffff161490505f8761010001518961010001511490505f88610120015115158a6101200151151514905086801561155e5750855b80156115675750845b80156115705750835b80156115795750825b80156115825750815b801561158b5750805b9a9950505050505050505050565b5f80807fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff858709858702925082811083820303915050805f036115ef578382816115e5576115e5611e94565b04925050506104d0565b808411611658576040517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601560248201527f4d6174683a206d756c446976206f766572666c6f770000000000000000000000604482015260640161066c565b5f8486880960026001871981018816978890046003810283188082028403028082028403028082028403028082028403028082028403029081029092039091025f889003889004909101858311909403939093029303949094049190911702949350505050565b5f805f604084860312156116d1575f80fd5b83359250602084013567ffffffffffffffff808211156116ef575f80fd5b818601915086601f830112611702575f80fd5b813581811115611710575f80fd5b876020828501011115611721575f80fd5b6020830194508093505050509250925092565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52604160045260245ffd5b6040516080810167ffffffffffffffff8111828210171561178457611784611734565b60405290565b604051610180810167ffffffffffffffff8111828210171561178457611784611734565b604051601f82017fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe016810167ffffffffffffffff811182821017156117f5576117f5611734565b604052919050565b73ffffffffffffffffffffffffffffffffffffffff8116811461181e575f80fd5b50565b5f60808284031215611831575f80fd5b611839611761565b90508135815260208083013561184e816117fd565b82820152604083013567ffffffffffffffff8082111561186c575f80fd5b818501915085601f83011261187f575f80fd5b81358181111561189157611891611734565b6118c1847fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f840116016117ae565b915080825286848285010111156118d6575f80fd5b80848401858401375f848284010152508060408501525050506060820135606082015292915050565b803561190a816117fd565b919050565b803563ffffffff8116811461190a575f80fd5b8035801515811461190a575f80fd5b5f6101808284031215611942575f80fd5b61194a61178a565b9050611955826118ff565b8152611963602083016118ff565b6020820152611974604083016118ff565b6040820152606082013560608201526080820135608082015261199960a0830161190f565b60a082015260c082013560c082015260e082013560e08201526101008083013581830152506101206119cc818401611922565b9082015261014082810135908201526101609182013591810191909152919050565b5f806101a08385031215611a00575f80fd5b823567ffffffffffffffff811115611a16575f80fd5b611a2285828601611821565b925050611a328460208501611931565b90509250929050565b5f60208284031215611a4b575f80fd5b813567ffffffffffffffff811115611a61575f80fd5b611a6d84828501611821565b949350505050565b5f60208284031215611a85575f80fd5b813567ffffffffffffffff811115611a9b575f80fd5b8201608081850312156104d0575f80fd5b815173ffffffffffffffffffffffffffffffffffffffff16815261018081016020830151611af2602084018273ffffffffffffffffffffffffffffffffffffffff169052565b506040830151611b1a604084018273ffffffffffffffffffffffffffffffffffffffff169052565b50606083015160608301526080830151608083015260a0830151611b4660a084018263ffffffff169052565b5060c083015160c083015260e083015160e083015261010080840151818401525061012080840151611b7b8285018215159052565b5050610140838101519083015261016092830151929091019190915290565b5f60208284031215611baa575f80fd5b5035919050565b5f806101a08385031215611bc3575f80fd5b611bcd8484611931565b915061018083013567ffffffffffffffff811115611be9575f80fd5b611bf585828601611821565b9150509250929050565b5f60208284031215611c0f575f80fd5b5051919050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52601160045260245ffd5b8082018082111561137857611378611c16565b808202811582820484141761137857611378611c16565b8181038181111561137857611378611c16565b5f81518084528060208401602086015e5f6020828601015260207fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f83011685010191505092915050565b602081528151602082015273ffffffffffffffffffffffffffffffffffffffff60208301511660408201525f604083015160806060840152611d1160a0840182611c80565b9050606084015160808401528091505092915050565b5f6113783683611821565b81835281816020850137505f602082840101525f60207fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f840116840101905092915050565b60208152813560208201525f6020830135611d93816117fd565b73ffffffffffffffffffffffffffffffffffffffff811660408401525060408301357fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe1843603018112611de4575f80fd5b830160208101903567ffffffffffffffff811115611e00575f80fd5b803603821315611e0e575f80fd5b60806060850152611e2360a085018284611d32565b915050606084013560808401528091505092915050565b5f73ffffffffffffffffffffffffffffffffffffffff80861683528085166020840152506060604083015261141d6060830184611c80565b5f8060408385031215611e83575f80fd5b505080516020909101519092909150565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52601260045260245ffd5b5f82611ecf57611ecf611e94565b500490565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52602160045260245ffd5b828152604060208201525f611a6d6040830184611c80565b5f63ffffffff80841680611f2f57611f2f611e94565b92169190910492915050565b63ffffffff818116838216028082169190828114611f5b57611f5b611c16565b505092915050565b63ffffffff818116838216019080821115611f8057611f80611c16565b509291505056fea2646970667358221220e3fb228b525d90b942c7e58fe2e2034a17bd258c082fd47740e764a7be45bac664736f6c63430008190033a26469706673582212201190cf42f989cee23f12597c8c1e9daab6d8c816513349c3ce7fd229cae5b0ff64736f6c63430008190033
    /// ```
    #[rustfmt::skip]
    #[allow(clippy::all)]
    pub static DEPLOYED_BYTECODE: alloy_sol_types::private::Bytes = alloy_sol_types::private::Bytes::from_static(
        b"`\x80`@R4\x80\x15a\0\x0FW_\x80\xFD[P`\x046\x10a\0\x9FW_5`\xE0\x1C\x80c7\xEB\xDFP\x11a\0rW\x80cfn\x1B9\x11a\0XW\x80cfn\x1B9\x14a\x01OW\x80c\xAB\"\x1Av\x14a\x01\x84W\x80c\xB5\xC5\xF6r\x14a\x01\xABW_\x80\xFD[\x80c7\xEB\xDFP\x14a\x01)W\x80c[]\x9E\xE6\x14a\x01<W_\x80\xFD[\x80c\x0E\xFEj\x8B\x14a\0\xA3W\x80c\"\xB1U\xC6\x14a\0\xB8W\x80c&\xE0\xA1\x96\x14a\0\xF5W\x80c'\x91\x05e\x14a\x01\x16W[_\x80\xFD[a\0\xB6a\0\xB16`\x04a\x11\xEAV[a\x01\xBEV[\0[a\0\xCBa\0\xC66`\x04a\x12aV[a\x02\xA3V[`@Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x90\x91\x16\x81R` \x01[`@Q\x80\x91\x03\x90\xF3[a\x01\x08a\x01\x036`\x04a\x12\xFBV[a\x04_V[`@Qa\0\xEC\x92\x91\x90a\x14\xFDV[a\0\xB6a\x01$6`\x04a\x15'V[a\x07\x97V[a\0\xCBa\x0176`\x04a\x15IV[a\x08/V[a\0\xB6a\x01J6`\x04a\x15\x91V[a\n\x01V[a\0\xCBa\x01]6`\x04a\x15'V[_` \x81\x90R\x90\x81R`@\x90 Ts\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81V[a\0\xCB\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[a\0\xB6a\x01\xB96`\x04a\x11\xEAV[a\x0B\x17V[a\x02O3\x84\x84\x86s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16c\r\xFE\x16\x81`@Q\x81c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x02\rW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x021\x91\x90a\x16\x17V[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x92\x91\x90a\x0CFV[a\x02\x9E3\x84\x83\x86s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16c\xD2\x12 \xA7`@Q\x81c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x02\rW=_\x80>=_\xFD[PPPV[_3\x80\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x8C\x8B`@Qa\x02\xD5\x90a\x11\xB9V[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x93\x84\x16\x81R\x91\x83\x16` \x83\x01R\x90\x91\x16`@\x82\x01R``\x01\x81\x90`@Q\x80\x91\x03\x90_\xF5\x90P\x80\x15\x80\x15a\x03\x1FW=_\x80>=_\xFD[P`@\x80Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x8E\x81\x16\x82R\x8C\x81\x16` \x83\x01R\x92\x94P\x82\x84\x16\x92\x85\x16\x91\x7Fg\x07%[,\\\xA8\x12 \xB2\xF3\xE4\x08\xA2i\xCB\x83\xBA\xA6\xAA~^7\xAA\x17V\x88:l\xDF\x06\xF1\x91\x01`@Q\x80\x91\x03\x90\xA3s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x81\x16_\x90\x81R` \x81\x90R`@\x90 \x80T\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x91\x83\x16\x91\x90\x91\x17\x90Ua\x03\xD8\x82\x8B\x8Aa\x01\xBEV[_`@Q\x80`\x80\x01`@R\x80\x89\x81R` \x01\x88s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x87\x87\x80\x80`\x1F\x01` \x80\x91\x04\x02` \x01`@Q\x90\x81\x01`@R\x80\x93\x92\x91\x90\x81\x81R` \x01\x83\x83\x80\x82\x847_\x92\x01\x91\x90\x91RPPP\x90\x82RP` \x01\x85\x90R\x90Pa\x04P\x83\x82a\x0C\xDBV[PP\x99\x98PPPPPPPPPV[`@\x80Qa\x01\x80\x81\x01\x82R_\x80\x82R` \x82\x01\x81\x90R\x91\x81\x01\x82\x90R``\x81\x01\x82\x90R`\x80\x81\x01\x82\x90R`\xA0\x81\x01\x82\x90R`\xC0\x81\x01\x82\x90R`\xE0\x81\x01\x82\x90Ra\x01\0\x81\x01\x82\x90Ra\x01 \x81\x01\x82\x90Ra\x01@\x81\x01\x82\x90Ra\x01`\x81\x01\x91\x90\x91R``0a\x04\xCF` \x89\x01\x89a\x15'V[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14a\x05QW`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1A`$\x82\x01R\x7Fcan only handle own orders\0\0\0\0\0\0`D\x82\x01R`d\x01[`@Q\x80\x91\x03\x90\xFD[_a\x05_`@\x89\x01\x89a\x162V[\x81\x01\x90a\x05l\x91\x90a\x17\\V[\x90P\x88s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16c\xEE\xC5\x0B\x97`@Q\x81c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x05\xB7W=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x05\xDB\x91\x90a\x18ZV[`@Q\x7F\xB0\x9A\xAA\xCA\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x8B\x16\x90c\xB0\x9A\xAA\xCA\x90a\x06-\x90\x85\x90`\x04\x01a\x18\xC3V[` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x06HW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x06l\x91\x90a\x18ZV[\x14a\x06\xD3W`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1A`$\x82\x01R\x7Finvalid trading parameters\0\0\0\0\0\0`D\x82\x01R`d\x01a\x05HV[`@Q\x7F\xE3\xE6\xF5\xB2\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x8A\x16\x90c\xE3\xE6\xF5\xB2\x90a\x07%\x90\x84\x90`\x04\x01a\x18\xC3V[a\x01\x80`@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x07AW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x07e\x91\x90a\x18\xF7V[\x92P\x82\x81`@Q` \x01a\x07z\x92\x91\x90a\x19\xB3V[`@Q` \x81\x83\x03\x03\x81R\x90`@R\x91PP\x96P\x96\x94PPPPPV[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x16_\x90\x81R` \x81\x90R`@\x90 T\x82\x91\x163\x14a\x08\"Ws\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x81\x16_\x90\x81R` \x81\x90R`@\x90\x81\x90 T\x90Q\x7Fh\xBA\xFF\xF8\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R\x91\x16`\x04\x82\x01R`$\x01a\x05HV[a\x08+\x82a\x0E\x05V[PPV[_\x80\x7F\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x000s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x87\x16`@Qa\x08y` \x82\x01a\x11\xB9V[\x81\x81\x03\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x90\x81\x01\x83R`\x1F\x90\x91\x01\x16`@\x81\x81Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81\x16` \x84\x01R\x80\x8B\x16\x91\x83\x01\x91\x90\x91R\x88\x16``\x82\x01R`\x80\x01`@\x80Q\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x81\x84\x03\x01\x81R\x90\x82\x90Ra\t:\x92\x91` \x01a\x19\xEBV[`@Q` \x81\x83\x03\x03\x81R\x90`@R\x80Q\x90` \x01 `@Q` \x01a\t\xC2\x94\x93\x92\x91\x90\x7F\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x94\x90\x94\x16\x84R``\x92\x90\x92\x1B\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\x16`\x01\x84\x01R`\x15\x83\x01R`5\x82\x01R`U\x01\x90V[`@\x80Q\x80\x83\x03\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x01\x81R\x91\x90R\x80Q` \x90\x91\x01 \x95\x94PPPPPV[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x87\x16_\x90\x81R` \x81\x90R`@\x90 T\x87\x91\x163\x14a\n\x8CWs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x81\x16_\x90\x81R` \x81\x90R`@\x90\x81\x90 T\x90Q\x7Fh\xBA\xFF\xF8\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R\x91\x16`\x04\x82\x01R`$\x01a\x05HV[_`@Q\x80`\x80\x01`@R\x80\x88\x81R` \x01\x87s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x86\x86\x80\x80`\x1F\x01` \x80\x91\x04\x02` \x01`@Q\x90\x81\x01`@R\x80\x93\x92\x91\x90\x81\x81R` \x01\x83\x83\x80\x82\x847_\x92\x01\x91\x90\x91RPPP\x90\x82RP` \x01\x84\x90R\x90Pa\x0B\x03\x88a\x0E\x05V[a\x0B\r\x88\x82a\x0C\xDBV[PPPPPPPPV[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x84\x16_\x90\x81R` \x81\x90R`@\x90 T\x84\x91\x163\x14a\x0B\xA2Ws\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x81\x16_\x90\x81R` \x81\x90R`@\x90\x81\x90 T\x90Q\x7Fh\xBA\xFF\xF8\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R\x91\x16`\x04\x82\x01R`$\x01a\x05HV[a\x0B\xF1\x843\x85\x87s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16c\r\xFE\x16\x81`@Q\x81c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x02\rW=_\x80>=_\xFD[a\x0C@\x843\x84\x87s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16c\xD2\x12 \xA7`@Q\x81c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x02\rW=_\x80>=_\xFD[PPPPV[`@\x80Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x85\x81\x16`$\x83\x01R\x84\x16`D\x82\x01R`d\x80\x82\x01\x84\x90R\x82Q\x80\x83\x03\x90\x91\x01\x81R`\x84\x90\x91\x01\x90\x91R` \x81\x01\x80Q{\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x7F#\xB8r\xDD\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x17\x90Ra\x0C@\x90\x85\x90a\x0E\xA3V[`@Q\x7F\xC5\xF3\xD2T\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x83\x16\x90c\xC5\xF3\xD2T\x90a\r-\x90\x84\x90`\x04\x01a\x18\xC3V[_`@Q\x80\x83\x03\x81_\x87\x80;\x15\x80\x15a\rDW_\x80\xFD[PZ\xF1\x15\x80\x15a\rVW=_\x80>=_\xFD[PP`@\x80Q``\x81\x01\x82R0\x81R_` \x80\x83\x01\x82\x90R\x83Q\x91\x95Ps\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x88\x16\x94P\x7F,\xCE\xACUU\xB0\xCAE\xA3tL\xEDT/T\xB5j\xD2\xEBE\xE5!\x96#r\xEE\xF2\x12\xA2\xCB\xF3a\x93\x83\x01\x91a\r\xBD\x91\x88\x91\x01a\x18\xC3V[`@\x80Q\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x81\x84\x03\x01\x81R\x91\x81R\x91RQa\r\xF8\x91\x90a\x19\xFFV[`@Q\x80\x91\x03\x90\xA2PPPV[\x80s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16c\x17p\x0F\x01`@Q\x81c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01_`@Q\x80\x83\x03\x81_\x87\x80;\x15\x80\x15a\x0EJW_\x80\xFD[PZ\xF1\x15\x80\x15a\x0E\\W=_\x80>=_\xFD[PP`@Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x84\x16\x92P\x7F\xC7[\xF4\xF0<\x02\xFA\xB9AJ}zT\x04\x8C\x04\x86r+\xC7/3\xAD\x92G\t\xA0Y6\x08\xAD'\x91P_\x90\xA2PV[_a\x0F\x04\x82`@Q\x80`@\x01`@R\x80` \x81R` \x01\x7FSafeERC20: low-level call failed\x81RP\x85s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16a\x0F\xB0\x90\x92\x91\x90c\xFF\xFF\xFF\xFF\x16V[\x90P\x80Q_\x14\x80a\x0F$WP\x80\x80` \x01\x90Q\x81\x01\x90a\x0F$\x91\x90a\x1ACV[a\x02\x9EW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`*`$\x82\x01R\x7FSafeERC20: ERC20 operation did n`D\x82\x01R\x7Fot succeed\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`d\x82\x01R`\x84\x01a\x05HV[``a\x0F\xBE\x84\x84_\x85a\x0F\xC6V[\x94\x93PPPPV[``\x82G\x10\x15a\x10XW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`&`$\x82\x01R\x7FAddress: insufficient balance fo`D\x82\x01R\x7Fr call\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`d\x82\x01R`\x84\x01a\x05HV[_\x80\x86s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x85\x87`@Qa\x10\x80\x91\x90a\x1A\\V[_`@Q\x80\x83\x03\x81\x85\x87Z\xF1\x92PPP=\x80_\x81\x14a\x10\xBAW`@Q\x91P`\x1F\x19`?=\x01\x16\x82\x01`@R=\x82R=_` \x84\x01>a\x10\xBFV[``\x91P[P\x91P\x91Pa\x10\xD0\x87\x83\x83\x87a\x10\xDBV[\x97\x96PPPPPPPV[``\x83\x15a\x11pW\x82Q_\x03a\x11iWs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x85\x16;a\x11iW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1D`$\x82\x01R\x7FAddress: call to non-contract\0\0\0`D\x82\x01R`d\x01a\x05HV[P\x81a\x0F\xBEV[a\x0F\xBE\x83\x83\x81Q\x15a\x11\x85W\x81Q\x80\x83` \x01\xFD[\x80`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01a\x05H\x91\x90a\x1AgV[a&x\x80a\x1Az\x839\x01\x90V[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x16\x81\x14a\x11\xE7W_\x80\xFD[PV[_\x80_``\x84\x86\x03\x12\x15a\x11\xFCW_\x80\xFD[\x835a\x12\x07\x81a\x11\xC6V[\x95` \x85\x015\x95P`@\x90\x94\x015\x93\x92PPPV[_\x80\x83`\x1F\x84\x01\x12a\x12,W_\x80\xFD[P\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x12CW_\x80\xFD[` \x83\x01\x91P\x83` \x82\x85\x01\x01\x11\x15a\x12ZW_\x80\xFD[\x92P\x92\x90PV[_\x80_\x80_\x80_\x80_a\x01\0\x8A\x8C\x03\x12\x15a\x12zW_\x80\xFD[\x895a\x12\x85\x81a\x11\xC6V[\x98P` \x8A\x015\x97P`@\x8A\x015a\x12\x9C\x81a\x11\xC6V[\x96P``\x8A\x015\x95P`\x80\x8A\x015\x94P`\xA0\x8A\x015a\x12\xBA\x81a\x11\xC6V[\x93P`\xC0\x8A\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x12\xD5W_\x80\xFD[a\x12\xE1\x8C\x82\x8D\x01a\x12\x1CV[\x9A\x9D\x99\x9CP\x97\x9A\x96\x99\x95\x98\x94\x97\x96`\xE0\x015\x94\x93PPPPV[_\x80_\x80_\x80`\x80\x87\x89\x03\x12\x15a\x13\x10W_\x80\xFD[\x865a\x13\x1B\x81a\x11\xC6V[\x95P` \x87\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a\x137W_\x80\xFD[\x90\x88\x01\x90``\x82\x8B\x03\x12\x15a\x13JW_\x80\xFD[\x90\x95P`@\x88\x015\x90\x80\x82\x11\x15a\x13_W_\x80\xFD[a\x13k\x8A\x83\x8B\x01a\x12\x1CV[\x90\x96P\x94P``\x89\x015\x91P\x80\x82\x11\x15a\x13\x83W_\x80\xFD[\x81\x89\x01\x91P\x89`\x1F\x83\x01\x12a\x13\x96W_\x80\xFD[\x815\x81\x81\x11\x15a\x13\xA4W_\x80\xFD[\x8A` \x82`\x05\x1B\x85\x01\x01\x11\x15a\x13\xB8W_\x80\xFD[` \x83\x01\x94P\x80\x93PPPP\x92\x95P\x92\x95P\x92\x95V[\x80Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x82R` \x81\x01Qa\x14\x0F` \x84\x01\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90RV[P`@\x81\x01Qa\x147`@\x84\x01\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90RV[P``\x81\x01Q``\x83\x01R`\x80\x81\x01Q`\x80\x83\x01R`\xA0\x81\x01Qa\x14c`\xA0\x84\x01\x82c\xFF\xFF\xFF\xFF\x16\x90RV[P`\xC0\x81\x01Q`\xC0\x83\x01R`\xE0\x81\x01Q`\xE0\x83\x01Ra\x01\0\x80\x82\x01Q\x81\x84\x01RPa\x01 \x80\x82\x01Qa\x14\x98\x82\x85\x01\x82\x15\x15\x90RV[PPa\x01@\x81\x81\x01Q\x90\x83\x01Ra\x01`\x90\x81\x01Q\x91\x01RV[_\x81Q\x80\x84R\x80` \x84\x01` \x86\x01^_` \x82\x86\x01\x01R` \x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`\x1F\x83\x01\x16\x85\x01\x01\x91PP\x92\x91PPV[_a\x01\xA0a\x15\x0B\x83\x86a\x13\xCEV[\x80a\x01\x80\x84\x01Ra\x15\x1E\x81\x84\x01\x85a\x14\xB1V[\x95\x94PPPPPV[_` \x82\x84\x03\x12\x15a\x157W_\x80\xFD[\x815a\x15B\x81a\x11\xC6V[\x93\x92PPPV[_\x80_``\x84\x86\x03\x12\x15a\x15[W_\x80\xFD[\x835a\x15f\x81a\x11\xC6V[\x92P` \x84\x015a\x15v\x81a\x11\xC6V[\x91P`@\x84\x015a\x15\x86\x81a\x11\xC6V[\x80\x91PP\x92P\x92P\x92V[_\x80_\x80_\x80`\xA0\x87\x89\x03\x12\x15a\x15\xA6W_\x80\xFD[\x865a\x15\xB1\x81a\x11\xC6V[\x95P` \x87\x015\x94P`@\x87\x015a\x15\xC8\x81a\x11\xC6V[\x93P``\x87\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x15\xE3W_\x80\xFD[a\x15\xEF\x89\x82\x8A\x01a\x12\x1CV[\x97\x9A\x96\x99P\x94\x97\x94\x96\x95`\x80\x90\x95\x015\x94\x93PPPPV[\x80Qa\x16\x12\x81a\x11\xC6V[\x91\x90PV[_` \x82\x84\x03\x12\x15a\x16'W_\x80\xFD[\x81Qa\x15B\x81a\x11\xC6V[_\x80\x835\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE1\x846\x03\x01\x81\x12a\x16eW_\x80\xFD[\x83\x01\x805\x91Pg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11\x15a\x16\x7FW_\x80\xFD[` \x01\x91P6\x81\x90\x03\x82\x13\x15a\x12ZW_\x80\xFD[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`A`\x04R`$_\xFD[`@Q`\x80\x81\x01g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x82\x82\x10\x17\x15a\x16\xE3Wa\x16\xE3a\x16\x93V[`@R\x90V[`@Qa\x01\x80\x81\x01g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x82\x82\x10\x17\x15a\x16\xE3Wa\x16\xE3a\x16\x93V[`@Q`\x1F\x82\x01\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x16\x81\x01g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x82\x82\x10\x17\x15a\x17TWa\x17Ta\x16\x93V[`@R\x91\x90PV[_` \x80\x83\x85\x03\x12\x15a\x17mW_\x80\xFD[\x825g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a\x17\x84W_\x80\xFD[\x90\x84\x01\x90`\x80\x82\x87\x03\x12\x15a\x17\x97W_\x80\xFD[a\x17\x9Fa\x16\xC0V[\x825\x81R\x83\x83\x015a\x17\xB0\x81a\x11\xC6V[\x81\x85\x01R`@\x83\x015\x82\x81\x11\x15a\x17\xC5W_\x80\xFD[\x83\x01`\x1F\x81\x01\x88\x13a\x17\xD5W_\x80\xFD[\x805\x83\x81\x11\x15a\x17\xE7Wa\x17\xE7a\x16\x93V[a\x18\x17\x86\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`\x1F\x84\x01\x16\x01a\x17\rV[\x93P\x80\x84R\x88\x86\x82\x84\x01\x01\x11\x15a\x18,W_\x80\xFD[\x80\x86\x83\x01\x87\x86\x017_\x86\x82\x86\x01\x01RPP\x81`@\x82\x01R``\x83\x015``\x82\x01R\x80\x94PPPPP\x92\x91PPV[_` \x82\x84\x03\x12\x15a\x18jW_\x80\xFD[PQ\x91\x90PV[\x80Q\x82Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF` \x82\x01Q\x16` \x83\x01R_`@\x82\x01Q`\x80`@\x85\x01Ra\x18\xAF`\x80\x85\x01\x82a\x14\xB1V[``\x93\x84\x01Q\x94\x90\x93\x01\x93\x90\x93RP\x91\x90PV[` \x81R_a\x15B` \x83\x01\x84a\x18qV[\x80Qc\xFF\xFF\xFF\xFF\x81\x16\x81\x14a\x16\x12W_\x80\xFD[\x80Q\x80\x15\x15\x81\x14a\x16\x12W_\x80\xFD[_a\x01\x80\x82\x84\x03\x12\x15a\x19\x08W_\x80\xFD[a\x19\x10a\x16\xE9V[a\x19\x19\x83a\x16\x07V[\x81Ra\x19'` \x84\x01a\x16\x07V[` \x82\x01Ra\x198`@\x84\x01a\x16\x07V[`@\x82\x01R``\x83\x01Q``\x82\x01R`\x80\x83\x01Q`\x80\x82\x01Ra\x19]`\xA0\x84\x01a\x18\xD5V[`\xA0\x82\x01R`\xC0\x83\x01Q`\xC0\x82\x01R`\xE0\x83\x01Q`\xE0\x82\x01Ra\x01\0\x80\x84\x01Q\x81\x83\x01RPa\x01 a\x19\x90\x81\x85\x01a\x18\xE8V[\x90\x82\x01Ra\x01@\x83\x81\x01Q\x90\x82\x01Ra\x01`\x92\x83\x01Q\x92\x81\x01\x92\x90\x92RP\x91\x90PV[_a\x01\xA0a\x19\xC1\x83\x86a\x13\xCEV[\x80a\x01\x80\x84\x01Ra\x15\x1E\x81\x84\x01\x85a\x18qV[_\x81Q\x80` \x84\x01\x85^_\x93\x01\x92\x83RP\x90\x91\x90PV[_a\x0F\xBEa\x19\xF9\x83\x86a\x19\xD4V[\x84a\x19\xD4V[` \x81Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82Q\x16` \x82\x01R` \x82\x01Q`@\x82\x01R_`@\x83\x01Q``\x80\x84\x01Ra\x0F\xBE`\x80\x84\x01\x82a\x14\xB1V[_` \x82\x84\x03\x12\x15a\x1ASW_\x80\xFD[a\x15B\x82a\x18\xE8V[_a\x15B\x82\x84a\x19\xD4V[` \x81R_a\x15B` \x83\x01\x84a\x14\xB1V\xFEa\x01 `@R4\x80\x15a\0\x10W_\x80\xFD[P`@Qa&x8\x03\x80a&x\x839\x81\x01`@\x81\x90Ra\0/\x91a\x05/V[`\x01`\x01`\xA0\x1B\x03\x83\x16`\x80\x81\x90R`@\x80Qc\xF6\x98\xDA%`\xE0\x1B\x81R\x90Qc\xF6\x98\xDA%\x91`\x04\x80\x82\x01\x92` \x92\x90\x91\x90\x82\x90\x03\x01\x81_\x87Z\xF1\x15\x80\x15a\0xW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\0\x9C\x91\x90a\x05yV[a\x01\0Ra\0\xAA\x823a\x01_V[a\0\xB4\x813a\x01_V[3`\x01`\x01`\xA0\x1B\x03\x16`\xE0\x81`\x01`\x01`\xA0\x1B\x03\x16\x81RPP_\x83`\x01`\x01`\xA0\x1B\x03\x16c\x9BU,\xC2`@Q\x81c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01` `@Q\x80\x83\x03\x81_\x87Z\xF1\x15\x80\x15a\x01\x0CW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x010\x91\x90a\x05\x90V[\x90Pa\x01<\x83\x82a\x01_V[a\x01F\x82\x82a\x01_V[P`\x01`\x01`\xA0\x1B\x03\x91\x82\x16`\xA0R\x16`\xC0RPa\x06\x1CV[a\x01t`\x01`\x01`\xA0\x1B\x03\x83\x16\x82_\x19a\x01xV[PPV[\x80\x15\x80a\x01\xF0WP`@Qcn\xB1v\x9F`\xE1\x1B\x81R0`\x04\x82\x01R`\x01`\x01`\xA0\x1B\x03\x83\x81\x16`$\x83\x01R\x84\x16\x90c\xDDb\xED>\x90`D\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x01\xCAW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x01\xEE\x91\x90a\x05yV[\x15[a\x02gW`@QbF\x1B\xCD`\xE5\x1B\x81R` `\x04\x82\x01R`6`$\x82\x01R\x7FSafeERC20: approve from non-zero`D\x82\x01R\x7F to non-zero allowance\0\0\0\0\0\0\0\0\0\0`d\x82\x01R`\x84\x01[`@Q\x80\x91\x03\x90\xFD[`@\x80Q`\x01`\x01`\xA0\x1B\x03\x84\x16`$\x82\x01R`D\x80\x82\x01\x84\x90R\x82Q\x80\x83\x03\x90\x91\x01\x81R`d\x90\x91\x01\x90\x91R` \x81\x01\x80Q`\x01`\x01`\xE0\x1B\x03\x90\x81\x16c\t^\xA7\xB3`\xE0\x1B\x17\x90\x91Ra\x02\xBD\x91\x85\x91a\x02\xC2\x16V[PPPV[`@\x80Q\x80\x82\x01\x90\x91R` \x80\x82R\x7FSafeERC20: low-level call failed\x90\x82\x01R_\x90a\x03\x0E\x90`\x01`\x01`\xA0\x1B\x03\x85\x16\x90\x84\x90a\x03\x8DV[\x90P\x80Q_\x14\x80a\x03.WP\x80\x80` \x01\x90Q\x81\x01\x90a\x03.\x91\x90a\x05\xB2V[a\x02\xBDW`@QbF\x1B\xCD`\xE5\x1B\x81R` `\x04\x82\x01R`*`$\x82\x01R\x7FSafeERC20: ERC20 operation did n`D\x82\x01Ri\x1B\xDD\x08\x1C\xDDX\xD8\xD9YY`\xB2\x1B`d\x82\x01R`\x84\x01a\x02^V[``a\x03\x9B\x84\x84_\x85a\x03\xA3V[\x94\x93PPPPV[``\x82G\x10\x15a\x04\x04W`@QbF\x1B\xCD`\xE5\x1B\x81R` `\x04\x82\x01R`&`$\x82\x01R\x7FAddress: insufficient balance fo`D\x82\x01Re\x1C\x88\x18\xD8[\x1B`\xD2\x1B`d\x82\x01R`\x84\x01a\x02^V[_\x80\x86`\x01`\x01`\xA0\x1B\x03\x16\x85\x87`@Qa\x04\x1F\x91\x90a\x05\xD1V[_`@Q\x80\x83\x03\x81\x85\x87Z\xF1\x92PPP=\x80_\x81\x14a\x04YW`@Q\x91P`\x1F\x19`?=\x01\x16\x82\x01`@R=\x82R=_` \x84\x01>a\x04^V[``\x91P[P\x90\x92P\x90Pa\x04p\x87\x83\x83\x87a\x04{V[\x97\x96PPPPPPPV[``\x83\x15a\x04\xE9W\x82Q_\x03a\x04\xE2W`\x01`\x01`\xA0\x1B\x03\x85\x16;a\x04\xE2W`@QbF\x1B\xCD`\xE5\x1B\x81R` `\x04\x82\x01R`\x1D`$\x82\x01R\x7FAddress: call to non-contract\0\0\0`D\x82\x01R`d\x01a\x02^V[P\x81a\x03\x9BV[a\x03\x9B\x83\x83\x81Q\x15a\x04\xFEW\x81Q\x80\x83` \x01\xFD[\x80`@QbF\x1B\xCD`\xE5\x1B\x81R`\x04\x01a\x02^\x91\x90a\x05\xE7V[`\x01`\x01`\xA0\x1B\x03\x81\x16\x81\x14a\x05,W_\x80\xFD[PV[_\x80_``\x84\x86\x03\x12\x15a\x05AW_\x80\xFD[\x83Qa\x05L\x81a\x05\x18V[` \x85\x01Q\x90\x93Pa\x05]\x81a\x05\x18V[`@\x85\x01Q\x90\x92Pa\x05n\x81a\x05\x18V[\x80\x91PP\x92P\x92P\x92V[_` \x82\x84\x03\x12\x15a\x05\x89W_\x80\xFD[PQ\x91\x90PV[_` \x82\x84\x03\x12\x15a\x05\xA0W_\x80\xFD[\x81Qa\x05\xAB\x81a\x05\x18V[\x93\x92PPPV[_` \x82\x84\x03\x12\x15a\x05\xC2W_\x80\xFD[\x81Q\x80\x15\x15\x81\x14a\x05\xABW_\x80\xFD[_\x82Q\x80` \x85\x01\x84^_\x92\x01\x91\x82RP\x91\x90PV[` \x81R_\x82Q\x80` \x84\x01R\x80` \x85\x01`@\x85\x01^_`@\x82\x85\x01\x01R`@`\x1F\x19`\x1F\x83\x01\x16\x84\x01\x01\x91PP\x92\x91PPV[`\x80Q`\xA0Q`\xC0Q`\xE0Qa\x01\0Qa\x1F\xBDa\x06\xBB_9_\x81\x81a\x02\xDB\x01Ra\x04+\x01R_\x81\x81a\x026\x01R\x81\x81a\x04\xD9\x01Ra\x0B\xF9\x01R_\x81\x81a\x02\xB4\x01R\x81\x81a\x05\x99\x01R\x81\x81a\r\\\x01R\x81\x81a\x0E\xBF\x01R\x81\x81a\x0F\x8E\x01Ra\x10\r\x01R_\x81\x81a\x018\x01R\x81\x81a\x05w\x01R\x81\x81a\r;\x01R\x81\x81a\x0E(\x01R\x81\x81a\x0Fk\x01Ra\x100\x01R_\x81\x81a\x03\"\x01Ra\x12\x14\x01Ra\x1F\xBD_\xF3\xFE`\x80`@R4\x80\x15a\0\x0FW_\x80\xFD[P`\x046\x10a\x01/W_5`\xE0\x1C\x80c\xB0\x9A\xAA\xCA\x11a\0\xADW\x80c\xE3\xE6\xF5\xB2\x11a\0}W\x80c\xEE\xC5\x0B\x97\x11a\0cW\x80c\xEE\xC5\x0B\x97\x14a\x03DW\x80c\xF1O\xCB\xC8\x14a\x03LW\x80c\xFF-\xBC\x98\x14a\x02\x03W_\x80\xFD[\x80c\xE3\xE6\xF5\xB2\x14a\x02\xFDW\x80c\xE5\x16q[\x14a\x03\x1DW_\x80\xFD[\x80c\xB0\x9A\xAA\xCA\x14a\x02\x89W\x80c\xC5\xF3\xD2T\x14a\x02\x9CW\x80c\xD2\x12 \xA7\x14a\x02\xAFW\x80c\xD2^\x0C\xB6\x14a\x02\xD6W_\x80\xFD[\x80c\x1C}\xE9A\x11a\x01\x02W\x80cH\x1Cju\x11a\0\xE8W\x80cH\x1Cju\x14a\x021W\x80c\x98\x1A\x16\x0B\x14a\x02XW\x80c\xA0)\xA8\xD4\x14a\x02vW_\x80\xFD[\x80c\x1C}\xE9A\x14a\x02\x03W\x80c>pn2\x14a\x02\nW_\x80\xFD[\x80c\r\xFE\x16\x81\x14a\x013W\x80c\x13\x03\xA4\x84\x14a\x01\x84W\x80c\x16&\xBA~\x14a\x01\xB5W\x80c\x17p\x0F\x01\x14a\x01\xF9W[_\x80\xFD[a\x01Z\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[`@Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x90\x91\x16\x81R` \x01[`@Q\x80\x91\x03\x90\xF3[\x7Fl<\x90$TW\x06\x0Fe\x17x{,K\x8C\xF5\0\xCA\x88\x9D#\x04\xAF\x02\x04;\xD5\xB5\x13\xE3\xB5\x93\\[`@Q\x90\x81R` \x01a\x01{V[a\x01\xC8a\x01\xC36`\x04a\x16\xBFV[a\x03_V[`@Q\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90\x91\x16\x81R` \x01a\x01{V[a\x02\x01a\x04\xD7V[\0[a\x01\xA7_\x81V[a\x01\xA7\x7Fl<\x90$TW\x06\x0Fe\x17x{,K\x8C\xF5\0\xCA\x88\x9D#\x04\xAF\x02\x04;\xD5\xB5\x13\xE3\xB5\x93\x81V[a\x01Z\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[a\x02aa\x01,\x81V[`@Qc\xFF\xFF\xFF\xFF\x90\x91\x16\x81R` \x01a\x01{V[a\x02\x01a\x02\x846`\x04a\x19\xEEV[a\x05sV[a\x01\xA7a\x02\x976`\x04a\x1A;V[a\x0B\xC8V[a\x02\x01a\x02\xAA6`\x04a\x1AuV[a\x0B\xF7V[a\x01Z\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[a\x01\xA7\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[a\x03\x10a\x03\x0B6`\x04a\x1A;V[a\x0C\xB7V[`@Qa\x01{\x91\x90a\x1A\xACV[a\x01Z\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[a\x01\xA7_T\x81V[a\x02\x01a\x03Z6`\x04a\x1B\x9AV[a\x11\xFCV[_\x80\x80a\x03n\x84\x86\x01\x86a\x1B\xB1V[\x91P\x91P_Ta\x03}\x82a\x0B\xC8V[\x14a\x03\xB4W`@Q\x7F\xF1\xA6x\x90\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x82\x01\x80Q\x7F\xD5\xA2[\xA2\xE9p\x94\xAD}\x83\xDC(\xA6W-\xA7\x97\xD6\xB3\xE7\xFCfc\xBD\x93\xEF\xB7\x89\xFC\x17\xE4\x89\x82Ra\x01\xA0\x82 \x91R`@Q\x7F\x19\x01\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\x02\x82\x01R`\"\x81\x01\x91\x90\x91R`B\x90 \x86\x81\x14a\x04\x94W`@Q\x7FY?\xCA\xCD\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\x04\x9F\x81\x83\x85a\x12\x91V[a\x04\xA9\x82\x84a\x05sV[P\x7F\x16&\xBA~\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x92PPP[\x93\x92PPPV[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x163\x14a\x05FW`@Q\x7F\xF8}\r\x16\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[_\x80\x80U`@Q\x7F\xBC\xB8\xB8\xFB\xDE\xA8\xAAm\xC4\xAEA!>M\xA8\x1E`Z=\x1AV\xED\x85\x1B\x93U\x18#!\xC0\x91\x90\x91\x90\xA1V[\x80Q\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x84\x16\x91\x16\x14a\x06wW\x80s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x83_\x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14a\x06uW`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x12`$\x82\x01R\x7Finvalid sell token\0\0\0\0\0\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01[`@Q\x80\x91\x03\x90\xFD[\x90[`@Q\x7Fp\xA0\x821\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R0`\x04\x82\x01R_\x90s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x84\x16\x90cp\xA0\x821\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x06\xE1W=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x07\x05\x91\x90a\x1B\xFFV[`@Q\x7Fp\xA0\x821\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R0`\x04\x82\x01R\x90\x91P_\x90s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x84\x16\x90cp\xA0\x821\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x07rW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x07\x96\x91\x90a\x1B\xFFV[\x90P\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x85` \x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14a\x081W`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x11`$\x82\x01R\x7Finvalid buy token\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x06lV[`@\x85\x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x15a\x08\xB3W`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1D`$\x82\x01R\x7Freceiver must be zero address\0\0\0`D\x82\x01R`d\x01a\x06lV[a\x08\xBFa\x01,Ba\x1CCV[\x85`\xA0\x01Qc\xFF\xFF\xFF\xFF\x16\x11\x15a\t2W`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1E`$\x82\x01R\x7Fvalidity too far in the future\0\0`D\x82\x01R`d\x01a\x06lV[\x85``\x01Q\x85`\xC0\x01Q\x14a\t\xA3W`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x0F`$\x82\x01R\x7Finvalid appData\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x06lV[`\xE0\x85\x01Q\x15a\n\x0FW`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x17`$\x82\x01R\x7Ffee amount must be zero\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x06lV[\x7FZ(\xE96;\xB9B\xB69'\0b\xAAk\xB2\x95\xF44\xBC\xDF\xC4,\x97&{\xF0\x03\xF2r\x06\r\xC9\x85a\x01`\x01Q\x14a\n\x9DW`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1D`$\x82\x01R\x7FbuyTokenBalance must be erc20\0\0\0`D\x82\x01R`d\x01a\x06lV[\x7FZ(\xE96;\xB9B\xB69'\0b\xAAk\xB2\x95\xF44\xBC\xDF\xC4,\x97&{\xF0\x03\xF2r\x06\r\xC9\x85a\x01@\x01Q\x14a\x0B+W`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1E`$\x82\x01R\x7FsellTokenBalance must be erc20\0\0`D\x82\x01R`d\x01a\x06lV[``\x85\x01Qa\x0B:\x90\x82a\x1CVV[`\x80\x86\x01Q``\x87\x01Qa\x0BN\x90\x85a\x1CmV[a\x0BX\x91\x90a\x1CVV[\x10\x15a\x0B\xC0W`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x17`$\x82\x01R\x7Freceived amount too low\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x06lV[PPPPPPV[_\x81`@Q` \x01a\x0B\xDA\x91\x90a\x1C\xCCV[`@Q` \x81\x83\x03\x03\x81R\x90`@R\x80Q\x90` \x01 \x90P\x91\x90PV[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x163\x14a\x0CfW`@Q\x7F\xF8}\r\x16\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[_a\x0Csa\x02\x97\x83a\x1D'V[\x90P\x80_\x81\x90UP\x80\x7FQ\x0EJOv\x90|-aX\xB3C\xF7\xC4\xF2\xF5\x97\xDF8[r|&\xE9\xEF\x90\xE7P\x93\xAC\xE1\x9A\x83`@Qa\x0C\xAB\x91\x90a\x1DyV[`@Q\x80\x91\x03\x90\xA2PPV[`@\x80Qa\x01\x80\x81\x01\x82R_\x80\x82R` \x82\x01\x81\x90R\x91\x81\x01\x82\x90R``\x81\x01\x82\x90R`\x80\x81\x01\x82\x90R`\xA0\x81\x01\x82\x90R`\xC0\x81\x01\x82\x90R`\xE0\x81\x01\x82\x90Ra\x01\0\x81\x01\x82\x90Ra\x01 \x81\x01\x82\x90Ra\x01@\x81\x01\x82\x90Ra\x01`\x81\x01\x91\x90\x91R_\x80\x83` \x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16c5^\xFD\xD9\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x87`@\x01Q`@Q\x84c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01a\r\x9E\x93\x92\x91\x90a\x1E:V[`@\x80Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\r\xB8W=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\r\xDC\x91\x90a\x1ErV[`@Q\x7Fp\xA0\x821\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R0`\x04\x82\x01R\x91\x93P\x91P_\x90\x81\x90s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x90cp\xA0\x821\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x0EmW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x0E\x91\x91\x90a\x1B\xFFV[`@Q\x7Fp\xA0\x821\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R0`\x04\x82\x01R\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90cp\xA0\x821\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x0F\x19W=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x0F=\x91\x90a\x1B\xFFV[\x90\x92P\x90P_\x80\x80\x80\x80a\x0FQ\x88\x88a\x1CVV[\x90P_a\x0F^\x8A\x88a\x1CVV[\x90P_\x82\x82\x10\x15a\x10\x0BW\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x96P\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x95Pa\x0F\xD6a\x0F\xBD`\x02\x8Ba\x1E\xC1V[a\x0F\xD1\x84a\x0F\xCC\x8E`\x02a\x1CVV[a\x13FV[a\x13~V[\x94Pa\x10\x01\x85a\x0F\xE6\x81\x8Da\x1CVV[a\x0F\xF0\x90\x85a\x1CCV[a\x0F\xFA\x8C\x8Fa\x1CVV[`\x01a\x13\xCBV[\x93P\x84\x90Pa\x10\x98V[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x96P\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x95Pa\x10na\x10_`\x02\x8Aa\x1E\xC1V[a\x0F\xD1\x85a\x0F\xCC\x8F`\x02a\x1CVV[\x94Pa\x10\x92\x85a\x10~\x81\x8Ea\x1CVV[a\x10\x88\x90\x86a\x1CCV[a\x0F\xFA\x8B\x8Ea\x1CVV[\x93P\x83\x90P[\x8CQ\x81\x10\x15a\x10\xDFWa\x10\xDF`@Q\x80`@\x01`@R\x80`\x17\x81R` \x01\x7Ftraded amount too small\0\0\0\0\0\0\0\0\0\x81RPa\x14&V[`@Q\x80a\x01\x80\x01`@R\x80\x88s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x87s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01_s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x86\x81R` \x01\x85\x81R` \x01a\x11Va\x01,a\x14fV[c\xFF\xFF\xFF\xFF\x16\x81R` \x01\x8E``\x01Q\x81R` \x01_\x81R` \x01\x7F\xF3\xB2wr\x8B?\xEEt\x94\x81\xEB>\x0B;H\x98\r\xBB\xABxe\x8F\xC4\x19\x02\\\xB1n\xEE4gu\x81R` \x01`\x01\x15\x15\x81R` \x01\x7FZ(\xE96;\xB9B\xB69'\0b\xAAk\xB2\x95\xF44\xBC\xDF\xC4,\x97&{\xF0\x03\xF2r\x06\r\xC9\x81R` \x01\x7FZ(\xE96;\xB9B\xB69'\0b\xAAk\xB2\x95\xF44\xBC\xDF\xC4,\x97&{\xF0\x03\xF2r\x06\r\xC9\x81RP\x9BPPPPPPPPPPPP\x91\x90PV[3s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x14a\x12kW`@Q\x7F\xBF\x84\x89w\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x80\x7Fl<\x90$TW\x06\x0Fe\x17x{,K\x8C\xF5\0\xCA\x88\x9D#\x04\xAF\x02\x04;\xD5\xB5\x13\xE3\xB5\x93]PV[\x7Fl<\x90$TW\x06\x0Fe\x17x{,K\x8C\xF5\0\xCA\x88\x9D#\x04\xAF\x02\x04;\xD5\xB5\x13\xE3\xB5\x93\\\x83\x81\x14a\x13@W\x80\x15a\x12\xF2W`@Q\x7F\xDA\xFB\xDD\x1F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[_a\x12\xFC\x84a\x0C\xB7V[\x90Pa\x13\x08\x83\x82a\x14\x87V[a\x13>W`@Q\x7F\xD9\xFF$\xC7\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[P[PPPPV[_\x82\x15a\x13sW\x81a\x13Y`\x01\x85a\x1CmV[a\x13c\x91\x90a\x1E\xC1V[a\x13n\x90`\x01a\x1CCV[a\x13uV[_[\x90P[\x92\x91PPV[_\x81\x83\x10\x15a\x13\xC5Wa\x13\xC5`@Q\x80`@\x01`@R\x80`\x15\x81R` \x01\x7Fsubtraction underflow\0\0\0\0\0\0\0\0\0\0\0\x81RPa\x14&V[P\x90\x03\x90V[_\x80a\x13\xD8\x86\x86\x86a\x15\x99V[\x90P`\x01\x83`\x02\x81\x11\x15a\x13\xEEWa\x13\xEEa\x1E\xD4V[\x14\x80\x15a\x14\nWP_\x84\x80a\x14\x05Wa\x14\x05a\x1E\x94V[\x86\x88\t\x11[\x15a\x14\x1DWa\x14\x1A`\x01\x82a\x1CCV[\x90P[\x95\x94PPPPPV[a\x141C`\x01a\x1CCV[\x81`@Q\x7F\x1F\xE8Pn\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01a\x06l\x92\x91\x90a\x1F\x01V[_\x81\x80a\x14s\x81Ba\x1F\x19V[a\x14}\x91\x90a\x1F;V[a\x13x\x91\x90a\x1FcV[_\x80\x82_\x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x84_\x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x90P_\x83` \x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x85` \x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x90P_\x84``\x01Q\x86``\x01Q\x14\x90P_\x85`\x80\x01Q\x87`\x80\x01Q\x14\x90P_\x86`\xA0\x01Qc\xFF\xFF\xFF\xFF\x16\x88`\xA0\x01Qc\xFF\xFF\xFF\xFF\x16\x14\x90P_\x87a\x01\0\x01Q\x89a\x01\0\x01Q\x14\x90P_\x88a\x01 \x01Q\x15\x15\x8Aa\x01 \x01Q\x15\x15\x14\x90P\x86\x80\x15a\x15^WP\x85[\x80\x15a\x15gWP\x84[\x80\x15a\x15pWP\x83[\x80\x15a\x15yWP\x82[\x80\x15a\x15\x82WP\x81[\x80\x15a\x15\x8BWP\x80[\x9A\x99PPPPPPPPPPV[_\x80\x80\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x85\x87\t\x85\x87\x02\x92P\x82\x81\x10\x83\x82\x03\x03\x91PP\x80_\x03a\x15\xEFW\x83\x82\x81a\x15\xE5Wa\x15\xE5a\x1E\x94V[\x04\x92PPPa\x04\xD0V[\x80\x84\x11a\x16XW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x15`$\x82\x01R\x7FMath: mulDiv overflow\0\0\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x06lV[_\x84\x86\x88\t`\x02`\x01\x87\x19\x81\x01\x88\x16\x97\x88\x90\x04`\x03\x81\x02\x83\x18\x80\x82\x02\x84\x03\x02\x80\x82\x02\x84\x03\x02\x80\x82\x02\x84\x03\x02\x80\x82\x02\x84\x03\x02\x80\x82\x02\x84\x03\x02\x90\x81\x02\x90\x92\x03\x90\x91\x02_\x88\x90\x03\x88\x90\x04\x90\x91\x01\x85\x83\x11\x90\x94\x03\x93\x90\x93\x02\x93\x03\x94\x90\x94\x04\x91\x90\x91\x17\x02\x94\x93PPPPV[_\x80_`@\x84\x86\x03\x12\x15a\x16\xD1W_\x80\xFD[\x835\x92P` \x84\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a\x16\xEFW_\x80\xFD[\x81\x86\x01\x91P\x86`\x1F\x83\x01\x12a\x17\x02W_\x80\xFD[\x815\x81\x81\x11\x15a\x17\x10W_\x80\xFD[\x87` \x82\x85\x01\x01\x11\x15a\x17!W_\x80\xFD[` \x83\x01\x94P\x80\x93PPPP\x92P\x92P\x92V[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`A`\x04R`$_\xFD[`@Q`\x80\x81\x01g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x82\x82\x10\x17\x15a\x17\x84Wa\x17\x84a\x174V[`@R\x90V[`@Qa\x01\x80\x81\x01g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x82\x82\x10\x17\x15a\x17\x84Wa\x17\x84a\x174V[`@Q`\x1F\x82\x01\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x16\x81\x01g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x82\x82\x10\x17\x15a\x17\xF5Wa\x17\xF5a\x174V[`@R\x91\x90PV[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x16\x81\x14a\x18\x1EW_\x80\xFD[PV[_`\x80\x82\x84\x03\x12\x15a\x181W_\x80\xFD[a\x189a\x17aV[\x90P\x815\x81R` \x80\x83\x015a\x18N\x81a\x17\xFDV[\x82\x82\x01R`@\x83\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a\x18lW_\x80\xFD[\x81\x85\x01\x91P\x85`\x1F\x83\x01\x12a\x18\x7FW_\x80\xFD[\x815\x81\x81\x11\x15a\x18\x91Wa\x18\x91a\x174V[a\x18\xC1\x84\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`\x1F\x84\x01\x16\x01a\x17\xAEV[\x91P\x80\x82R\x86\x84\x82\x85\x01\x01\x11\x15a\x18\xD6W_\x80\xFD[\x80\x84\x84\x01\x85\x84\x017_\x84\x82\x84\x01\x01RP\x80`@\x85\x01RPPP``\x82\x015``\x82\x01R\x92\x91PPV[\x805a\x19\n\x81a\x17\xFDV[\x91\x90PV[\x805c\xFF\xFF\xFF\xFF\x81\x16\x81\x14a\x19\nW_\x80\xFD[\x805\x80\x15\x15\x81\x14a\x19\nW_\x80\xFD[_a\x01\x80\x82\x84\x03\x12\x15a\x19BW_\x80\xFD[a\x19Ja\x17\x8AV[\x90Pa\x19U\x82a\x18\xFFV[\x81Ra\x19c` \x83\x01a\x18\xFFV[` \x82\x01Ra\x19t`@\x83\x01a\x18\xFFV[`@\x82\x01R``\x82\x015``\x82\x01R`\x80\x82\x015`\x80\x82\x01Ra\x19\x99`\xA0\x83\x01a\x19\x0FV[`\xA0\x82\x01R`\xC0\x82\x015`\xC0\x82\x01R`\xE0\x82\x015`\xE0\x82\x01Ra\x01\0\x80\x83\x015\x81\x83\x01RPa\x01 a\x19\xCC\x81\x84\x01a\x19\"V[\x90\x82\x01Ra\x01@\x82\x81\x015\x90\x82\x01Ra\x01`\x91\x82\x015\x91\x81\x01\x91\x90\x91R\x91\x90PV[_\x80a\x01\xA0\x83\x85\x03\x12\x15a\x1A\0W_\x80\xFD[\x825g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x1A\x16W_\x80\xFD[a\x1A\"\x85\x82\x86\x01a\x18!V[\x92PPa\x1A2\x84` \x85\x01a\x191V[\x90P\x92P\x92\x90PV[_` \x82\x84\x03\x12\x15a\x1AKW_\x80\xFD[\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x1AaW_\x80\xFD[a\x1Am\x84\x82\x85\x01a\x18!V[\x94\x93PPPPV[_` \x82\x84\x03\x12\x15a\x1A\x85W_\x80\xFD[\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x1A\x9BW_\x80\xFD[\x82\x01`\x80\x81\x85\x03\x12\x15a\x04\xD0W_\x80\xFD[\x81Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81Ra\x01\x80\x81\x01` \x83\x01Qa\x1A\xF2` \x84\x01\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90RV[P`@\x83\x01Qa\x1B\x1A`@\x84\x01\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90RV[P``\x83\x01Q``\x83\x01R`\x80\x83\x01Q`\x80\x83\x01R`\xA0\x83\x01Qa\x1BF`\xA0\x84\x01\x82c\xFF\xFF\xFF\xFF\x16\x90RV[P`\xC0\x83\x01Q`\xC0\x83\x01R`\xE0\x83\x01Q`\xE0\x83\x01Ra\x01\0\x80\x84\x01Q\x81\x84\x01RPa\x01 \x80\x84\x01Qa\x1B{\x82\x85\x01\x82\x15\x15\x90RV[PPa\x01@\x83\x81\x01Q\x90\x83\x01Ra\x01`\x92\x83\x01Q\x92\x90\x91\x01\x91\x90\x91R\x90V[_` \x82\x84\x03\x12\x15a\x1B\xAAW_\x80\xFD[P5\x91\x90PV[_\x80a\x01\xA0\x83\x85\x03\x12\x15a\x1B\xC3W_\x80\xFD[a\x1B\xCD\x84\x84a\x191V[\x91Pa\x01\x80\x83\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x1B\xE9W_\x80\xFD[a\x1B\xF5\x85\x82\x86\x01a\x18!V[\x91PP\x92P\x92\x90PV[_` \x82\x84\x03\x12\x15a\x1C\x0FW_\x80\xFD[PQ\x91\x90PV[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x11`\x04R`$_\xFD[\x80\x82\x01\x80\x82\x11\x15a\x13xWa\x13xa\x1C\x16V[\x80\x82\x02\x81\x15\x82\x82\x04\x84\x14\x17a\x13xWa\x13xa\x1C\x16V[\x81\x81\x03\x81\x81\x11\x15a\x13xWa\x13xa\x1C\x16V[_\x81Q\x80\x84R\x80` \x84\x01` \x86\x01^_` \x82\x86\x01\x01R` \x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`\x1F\x83\x01\x16\x85\x01\x01\x91PP\x92\x91PPV[` \x81R\x81Q` \x82\x01Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF` \x83\x01Q\x16`@\x82\x01R_`@\x83\x01Q`\x80``\x84\x01Ra\x1D\x11`\xA0\x84\x01\x82a\x1C\x80V[\x90P``\x84\x01Q`\x80\x84\x01R\x80\x91PP\x92\x91PPV[_a\x13x6\x83a\x18!V[\x81\x83R\x81\x81` \x85\x017P_` \x82\x84\x01\x01R_` \x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`\x1F\x84\x01\x16\x84\x01\x01\x90P\x92\x91PPV[` \x81R\x815` \x82\x01R_` \x83\x015a\x1D\x93\x81a\x17\xFDV[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x16`@\x84\x01RP`@\x83\x015\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE1\x846\x03\x01\x81\x12a\x1D\xE4W_\x80\xFD[\x83\x01` \x81\x01\x905g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x1E\0W_\x80\xFD[\x806\x03\x82\x13\x15a\x1E\x0EW_\x80\xFD[`\x80``\x85\x01Ra\x1E#`\xA0\x85\x01\x82\x84a\x1D2V[\x91PP``\x84\x015`\x80\x84\x01R\x80\x91PP\x92\x91PPV[_s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x86\x16\x83R\x80\x85\x16` \x84\x01RP```@\x83\x01Ra\x14\x1D``\x83\x01\x84a\x1C\x80V[_\x80`@\x83\x85\x03\x12\x15a\x1E\x83W_\x80\xFD[PP\x80Q` \x90\x91\x01Q\x90\x92\x90\x91PV[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x12`\x04R`$_\xFD[_\x82a\x1E\xCFWa\x1E\xCFa\x1E\x94V[P\x04\x90V[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`!`\x04R`$_\xFD[\x82\x81R`@` \x82\x01R_a\x1Am`@\x83\x01\x84a\x1C\x80V[_c\xFF\xFF\xFF\xFF\x80\x84\x16\x80a\x1F/Wa\x1F/a\x1E\x94V[\x92\x16\x91\x90\x91\x04\x92\x91PPV[c\xFF\xFF\xFF\xFF\x81\x81\x16\x83\x82\x16\x02\x80\x82\x16\x91\x90\x82\x81\x14a\x1F[Wa\x1F[a\x1C\x16V[PP\x92\x91PPV[c\xFF\xFF\xFF\xFF\x81\x81\x16\x83\x82\x16\x01\x90\x80\x82\x11\x15a\x1F\x80Wa\x1F\x80a\x1C\x16V[P\x92\x91PPV\xFE\xA2dipfsX\"\x12 \xE3\xFB\"\x8BR]\x90\xB9B\xC7\xE5\x8F\xE2\xE2\x03J\x17\xBD%\x8C\x08/\xD4w@\xE7d\xA7\xBEE\xBA\xC6dsolcC\0\x08\x19\x003\xA2dipfsX\"\x12 \x11\x90\xCFB\xF9\x89\xCE\xE2?\x12Y|\x8C\x1E\x9D\xAA\xB6\xD8\xC8\x16Q3I\xC3\xCE\x7F\xD2)\xCA\xE5\xB0\xFFdsolcC\0\x08\x19\x003",
    );
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Custom error with signature `OnlyOwnerCanCall(address)` and selector `0x68bafff8`.
```solidity
error OnlyOwnerCanCall(address owner);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct OnlyOwnerCanCall {
        #[allow(missing_docs)]
        pub owner: alloy_sol_types::private::Address,
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
        impl ::core::convert::From<OnlyOwnerCanCall> for UnderlyingRustTuple<'_> {
            fn from(value: OnlyOwnerCanCall) -> Self {
                (value.owner,)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for OnlyOwnerCanCall {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self { owner: tuple.0 }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for OnlyOwnerCanCall {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "OnlyOwnerCanCall(address)";
            const SELECTOR: [u8; 4] = [104u8, 186u8, 255u8, 248u8];
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
    /**Custom error with signature `OrderNotValid(string)` and selector `0xc8fc2725`.
```solidity
error OrderNotValid(string);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct OrderNotValid(pub alloy_sol_types::private::String);
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
        type UnderlyingSolTuple<'a> = (alloy_sol_types::sol_data::String,);
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (alloy_sol_types::private::String,);
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
        impl ::core::convert::From<OrderNotValid> for UnderlyingRustTuple<'_> {
            fn from(value: OrderNotValid) -> Self {
                (value.0,)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for OrderNotValid {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self(tuple.0)
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for OrderNotValid {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "OrderNotValid(string)";
            const SELECTOR: [u8; 4] = [200u8, 252u8, 39u8, 37u8];
            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }
            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <alloy_sol_types::sol_data::String as alloy_sol_types::SolType>::tokenize(
                        &self.0,
                    ),
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
    /**Event with signature `ConditionalOrderCreated(address,(address,bytes32,bytes))` and selector `0x2cceac5555b0ca45a3744ced542f54b56ad2eb45e521962372eef212a2cbf361`.
```solidity
event ConditionalOrderCreated(address indexed owner, IConditionalOrder.ConditionalOrderParams params);
```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct ConditionalOrderCreated {
        #[allow(missing_docs)]
        pub owner: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub params: <IConditionalOrder::ConditionalOrderParams as alloy_sol_types::SolType>::RustType,
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
        impl alloy_sol_types::SolEvent for ConditionalOrderCreated {
            type DataTuple<'a> = (IConditionalOrder::ConditionalOrderParams,);
            type DataToken<'a> = <Self::DataTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "ConditionalOrderCreated(address,(address,bytes32,bytes))";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 = alloy_sol_types::private::B256::new([
                44u8, 206u8, 172u8, 85u8, 85u8, 176u8, 202u8, 69u8, 163u8, 116u8, 76u8,
                237u8, 84u8, 47u8, 84u8, 181u8, 106u8, 210u8, 235u8, 69u8, 229u8, 33u8,
                150u8, 35u8, 114u8, 238u8, 242u8, 18u8, 162u8, 203u8, 243u8, 97u8,
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
                    params: data.0,
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
                    <IConditionalOrder::ConditionalOrderParams as alloy_sol_types::SolType>::tokenize(
                        &self.params,
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
        impl alloy_sol_types::private::IntoLogData for ConditionalOrderCreated {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&ConditionalOrderCreated> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(
                this: &ConditionalOrderCreated,
            ) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `Deployed(address,address,address,address)` and selector `0x6707255b2c5ca81220b2f3e408a269cb83baa6aa7e5e37aa1756883a6cdf06f1`.
```solidity
event Deployed(address indexed amm, address indexed owner, address token0, address token1);
```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct Deployed {
        #[allow(missing_docs)]
        pub amm: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub owner: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub token0: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub token1: alloy_sol_types::private::Address,
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
        impl alloy_sol_types::SolEvent for Deployed {
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
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "Deployed(address,address,address,address)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 = alloy_sol_types::private::B256::new([
                103u8, 7u8, 37u8, 91u8, 44u8, 92u8, 168u8, 18u8, 32u8, 178u8, 243u8,
                228u8, 8u8, 162u8, 105u8, 203u8, 131u8, 186u8, 166u8, 170u8, 126u8, 94u8,
                55u8, 170u8, 23u8, 86u8, 136u8, 58u8, 108u8, 223u8, 6u8, 241u8,
            ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    amm: topics.1,
                    owner: topics.2,
                    token0: data.0,
                    token1: data.1,
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
                        &self.token0,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.token1,
                    ),
                )
            }
            #[inline]
            fn topics(&self) -> <Self::TopicList as alloy_sol_types::SolType>::RustType {
                (Self::SIGNATURE_HASH.into(), self.amm.clone(), self.owner.clone())
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
                    &self.amm,
                );
                out[2usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.owner,
                );
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for Deployed {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&Deployed> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &Deployed) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `TradingDisabled(address)` and selector `0xc75bf4f03c02fab9414a7d7a54048c0486722bc72f33ad924709a0593608ad27`.
```solidity
event TradingDisabled(address indexed amm);
```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct TradingDisabled {
        #[allow(missing_docs)]
        pub amm: alloy_sol_types::private::Address,
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
        impl alloy_sol_types::SolEvent for TradingDisabled {
            type DataTuple<'a> = ();
            type DataToken<'a> = <Self::DataTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "TradingDisabled(address)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 = alloy_sol_types::private::B256::new([
                199u8, 91u8, 244u8, 240u8, 60u8, 2u8, 250u8, 185u8, 65u8, 74u8, 125u8,
                122u8, 84u8, 4u8, 140u8, 4u8, 134u8, 114u8, 43u8, 199u8, 47u8, 51u8,
                173u8, 146u8, 71u8, 9u8, 160u8, 89u8, 54u8, 8u8, 173u8, 39u8,
            ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self { amm: topics.1 }
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
                (Self::SIGNATURE_HASH.into(), self.amm.clone())
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
                    &self.amm,
                );
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for TradingDisabled {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&TradingDisabled> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &TradingDisabled) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    /**Constructor`.
```solidity
constructor(address _settler);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct constructorCall {
        #[allow(missing_docs)]
        pub _settler: alloy_sol_types::private::Address,
    }
    const _: () = {
        use alloy_sol_types as alloy_sol_types;
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
            impl ::core::convert::From<constructorCall> for UnderlyingRustTuple<'_> {
                fn from(value: constructorCall) -> Self {
                    (value._settler,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for constructorCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _settler: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolConstructor for constructorCall {
            type Parameters<'a> = (alloy_sol_types::sol_data::Address,);
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
                        &self._settler,
                    ),
                )
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `ammDeterministicAddress(address,address,address)` and selector `0x37ebdf50`.
```solidity
function ammDeterministicAddress(address ammOwner, address token0, address token1) external view returns (address);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct ammDeterministicAddressCall {
        #[allow(missing_docs)]
        pub ammOwner: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub token0: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub token1: alloy_sol_types::private::Address,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`ammDeterministicAddress(address,address,address)`](ammDeterministicAddressCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct ammDeterministicAddressReturn {
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
            impl ::core::convert::From<ammDeterministicAddressCall>
            for UnderlyingRustTuple<'_> {
                fn from(value: ammDeterministicAddressCall) -> Self {
                    (value.ammOwner, value.token0, value.token1)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for ammDeterministicAddressCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        ammOwner: tuple.0,
                        token0: tuple.1,
                        token1: tuple.2,
                    }
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
            impl ::core::convert::From<ammDeterministicAddressReturn>
            for UnderlyingRustTuple<'_> {
                fn from(value: ammDeterministicAddressReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for ammDeterministicAddressReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for ammDeterministicAddressCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
            );
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::Address;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "ammDeterministicAddress(address,address,address)";
            const SELECTOR: [u8; 4] = [55u8, 235u8, 223u8, 80u8];
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
                        &self.ammOwner,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.token0,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.token1,
                    ),
                )
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
                        let r: ammDeterministicAddressReturn = r.into();
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
                        let r: ammDeterministicAddressReturn = r.into();
                        r._0
                    })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `create(address,uint256,address,uint256,uint256,address,bytes,bytes32)` and selector `0x22b155c6`.
```solidity
function create(address token0, uint256 amount0, address token1, uint256 amount1, uint256 minTradedToken0, address priceOracle, bytes memory priceOracleData, bytes32 appData) external returns (address amm);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct createCall {
        #[allow(missing_docs)]
        pub token0: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub amount0: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub token1: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub amount1: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub minTradedToken0: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub priceOracle: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub priceOracleData: alloy_sol_types::private::Bytes,
        #[allow(missing_docs)]
        pub appData: alloy_sol_types::private::FixedBytes<32>,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`create(address,uint256,address,uint256,uint256,address,bytes,bytes32)`](createCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct createReturn {
        #[allow(missing_docs)]
        pub amm: alloy_sol_types::private::Address,
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
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Bytes,
                alloy_sol_types::sol_data::FixedBytes<32>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::Address,
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::Address,
                alloy_sol_types::private::Bytes,
                alloy_sol_types::private::FixedBytes<32>,
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
            impl ::core::convert::From<createCall> for UnderlyingRustTuple<'_> {
                fn from(value: createCall) -> Self {
                    (
                        value.token0,
                        value.amount0,
                        value.token1,
                        value.amount1,
                        value.minTradedToken0,
                        value.priceOracle,
                        value.priceOracleData,
                        value.appData,
                    )
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for createCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        token0: tuple.0,
                        amount0: tuple.1,
                        token1: tuple.2,
                        amount1: tuple.3,
                        minTradedToken0: tuple.4,
                        priceOracle: tuple.5,
                        priceOracleData: tuple.6,
                        appData: tuple.7,
                    }
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
            impl ::core::convert::From<createReturn> for UnderlyingRustTuple<'_> {
                fn from(value: createReturn) -> Self {
                    (value.amm,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for createReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { amm: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for createCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Bytes,
                alloy_sol_types::sol_data::FixedBytes<32>,
            );
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::Address;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "create(address,uint256,address,uint256,uint256,address,bytes,bytes32)";
            const SELECTOR: [u8; 4] = [34u8, 177u8, 85u8, 198u8];
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
                        &self.token0,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.amount0),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.token1,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.amount1),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.minTradedToken0),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.priceOracle,
                    ),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.priceOracleData,
                    ),
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self.appData),
                )
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
                        let r: createReturn = r.into();
                        r.amm
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
                        let r: createReturn = r.into();
                        r.amm
                    })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `deposit(address,uint256,uint256)` and selector `0x0efe6a8b`.
```solidity
function deposit(address amm, uint256 amount0, uint256 amount1) external;
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct depositCall {
        #[allow(missing_docs)]
        pub amm: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub amount0: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub amount1: alloy_sol_types::private::primitives::aliases::U256,
    }
    ///Container type for the return parameters of the [`deposit(address,uint256,uint256)`](depositCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct depositReturn {}
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
                alloy_sol_types::sol_data::Uint<256>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
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
            impl ::core::convert::From<depositCall> for UnderlyingRustTuple<'_> {
                fn from(value: depositCall) -> Self {
                    (value.amm, value.amount0, value.amount1)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for depositCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        amm: tuple.0,
                        amount0: tuple.1,
                        amount1: tuple.2,
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
            impl ::core::convert::From<depositReturn> for UnderlyingRustTuple<'_> {
                fn from(value: depositReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for depositReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl depositReturn {
            fn _tokenize(
                &self,
            ) -> <depositCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for depositCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = depositReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "deposit(address,uint256,uint256)";
            const SELECTOR: [u8; 4] = [14u8, 254u8, 106u8, 139u8];
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
                        &self.amm,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.amount0),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.amount1),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                depositReturn::_tokenize(ret)
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
    /**Function with signature `disableTrading(address)` and selector `0x27910565`.
```solidity
function disableTrading(address amm) external;
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct disableTradingCall {
        #[allow(missing_docs)]
        pub amm: alloy_sol_types::private::Address,
    }
    ///Container type for the return parameters of the [`disableTrading(address)`](disableTradingCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct disableTradingReturn {}
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
            impl ::core::convert::From<disableTradingCall> for UnderlyingRustTuple<'_> {
                fn from(value: disableTradingCall) -> Self {
                    (value.amm,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for disableTradingCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { amm: tuple.0 }
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
            impl ::core::convert::From<disableTradingReturn>
            for UnderlyingRustTuple<'_> {
                fn from(value: disableTradingReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for disableTradingReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl disableTradingReturn {
            fn _tokenize(
                &self,
            ) -> <disableTradingCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for disableTradingCall {
            type Parameters<'a> = (alloy_sol_types::sol_data::Address,);
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = disableTradingReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "disableTrading(address)";
            const SELECTOR: [u8; 4] = [39u8, 145u8, 5u8, 101u8];
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
                        &self.amm,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                disableTradingReturn::_tokenize(ret)
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
    /**Function with signature `getTradeableOrderWithSignature(address,(address,bytes32,bytes),bytes,bytes32[])` and selector `0x26e0a196`.
```solidity
function getTradeableOrderWithSignature(address amm, IConditionalOrder.ConditionalOrderParams memory params, bytes memory, bytes32[] memory) external view returns (GPv2Order.Data memory order, bytes memory signature);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct getTradeableOrderWithSignatureCall {
        #[allow(missing_docs)]
        pub amm: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub params: <IConditionalOrder::ConditionalOrderParams as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub _2: alloy_sol_types::private::Bytes,
        #[allow(missing_docs)]
        pub _3: alloy_sol_types::private::Vec<alloy_sol_types::private::FixedBytes<32>>,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`getTradeableOrderWithSignature(address,(address,bytes32,bytes),bytes,bytes32[])`](getTradeableOrderWithSignatureCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct getTradeableOrderWithSignatureReturn {
        #[allow(missing_docs)]
        pub order: <GPv2Order::Data as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub signature: alloy_sol_types::private::Bytes,
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
                IConditionalOrder::ConditionalOrderParams,
                alloy_sol_types::sol_data::Bytes,
                alloy_sol_types::sol_data::Array<
                    alloy_sol_types::sol_data::FixedBytes<32>,
                >,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
                <IConditionalOrder::ConditionalOrderParams as alloy_sol_types::SolType>::RustType,
                alloy_sol_types::private::Bytes,
                alloy_sol_types::private::Vec<alloy_sol_types::private::FixedBytes<32>>,
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
            impl ::core::convert::From<getTradeableOrderWithSignatureCall>
            for UnderlyingRustTuple<'_> {
                fn from(value: getTradeableOrderWithSignatureCall) -> Self {
                    (value.amm, value.params, value._2, value._3)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for getTradeableOrderWithSignatureCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        amm: tuple.0,
                        params: tuple.1,
                        _2: tuple.2,
                        _3: tuple.3,
                    }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                GPv2Order::Data,
                alloy_sol_types::sol_data::Bytes,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                <GPv2Order::Data as alloy_sol_types::SolType>::RustType,
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
            impl ::core::convert::From<getTradeableOrderWithSignatureReturn>
            for UnderlyingRustTuple<'_> {
                fn from(value: getTradeableOrderWithSignatureReturn) -> Self {
                    (value.order, value.signature)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for getTradeableOrderWithSignatureReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        order: tuple.0,
                        signature: tuple.1,
                    }
                }
            }
        }
        impl getTradeableOrderWithSignatureReturn {
            fn _tokenize(
                &self,
            ) -> <getTradeableOrderWithSignatureCall as alloy_sol_types::SolCall>::ReturnToken<
                '_,
            > {
                (
                    <GPv2Order::Data as alloy_sol_types::SolType>::tokenize(&self.order),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.signature,
                    ),
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for getTradeableOrderWithSignatureCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                IConditionalOrder::ConditionalOrderParams,
                alloy_sol_types::sol_data::Bytes,
                alloy_sol_types::sol_data::Array<
                    alloy_sol_types::sol_data::FixedBytes<32>,
                >,
            );
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = getTradeableOrderWithSignatureReturn;
            type ReturnTuple<'a> = (GPv2Order::Data, alloy_sol_types::sol_data::Bytes);
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "getTradeableOrderWithSignature(address,(address,bytes32,bytes),bytes,bytes32[])";
            const SELECTOR: [u8; 4] = [38u8, 224u8, 161u8, 150u8];
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
                        &self.amm,
                    ),
                    <IConditionalOrder::ConditionalOrderParams as alloy_sol_types::SolType>::tokenize(
                        &self.params,
                    ),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self._2,
                    ),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::FixedBytes<32>,
                    > as alloy_sol_types::SolType>::tokenize(&self._3),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                getTradeableOrderWithSignatureReturn::_tokenize(ret)
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
    /**Function with signature `owner(address)` and selector `0x666e1b39`.
```solidity
function owner(address) external view returns (address);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct ownerCall(pub alloy_sol_types::private::Address);
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`owner(address)`](ownerCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct ownerReturn {
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
            impl ::core::convert::From<ownerCall> for UnderlyingRustTuple<'_> {
                fn from(value: ownerCall) -> Self {
                    (value.0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for ownerCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self(tuple.0)
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
            impl ::core::convert::From<ownerReturn> for UnderlyingRustTuple<'_> {
                fn from(value: ownerReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for ownerReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for ownerCall {
            type Parameters<'a> = (alloy_sol_types::sol_data::Address,);
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::Address;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "owner(address)";
            const SELECTOR: [u8; 4] = [102u8, 110u8, 27u8, 57u8];
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
                        &self.0,
                    ),
                )
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
                        let r: ownerReturn = r.into();
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
                        let r: ownerReturn = r.into();
                        r._0
                    })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `settler()` and selector `0xab221a76`.
```solidity
function settler() external view returns (address);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct settlerCall;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`settler()`](settlerCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct settlerReturn {
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
            impl ::core::convert::From<settlerCall> for UnderlyingRustTuple<'_> {
                fn from(value: settlerCall) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for settlerCall {
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
            impl ::core::convert::From<settlerReturn> for UnderlyingRustTuple<'_> {
                fn from(value: settlerReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for settlerReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for settlerCall {
            type Parameters<'a> = ();
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::Address;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "settler()";
            const SELECTOR: [u8; 4] = [171u8, 34u8, 26u8, 118u8];
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
                        let r: settlerReturn = r.into();
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
                        let r: settlerReturn = r.into();
                        r._0
                    })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `updateParameters(address,uint256,address,bytes,bytes32)` and selector `0x5b5d9ee6`.
```solidity
function updateParameters(address amm, uint256 minTradedToken0, address priceOracle, bytes memory priceOracleData, bytes32 appData) external;
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct updateParametersCall {
        #[allow(missing_docs)]
        pub amm: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub minTradedToken0: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub priceOracle: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub priceOracleData: alloy_sol_types::private::Bytes,
        #[allow(missing_docs)]
        pub appData: alloy_sol_types::private::FixedBytes<32>,
    }
    ///Container type for the return parameters of the [`updateParameters(address,uint256,address,bytes,bytes32)`](updateParametersCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct updateParametersReturn {}
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
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Bytes,
                alloy_sol_types::sol_data::FixedBytes<32>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::Address,
                alloy_sol_types::private::Bytes,
                alloy_sol_types::private::FixedBytes<32>,
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
            impl ::core::convert::From<updateParametersCall>
            for UnderlyingRustTuple<'_> {
                fn from(value: updateParametersCall) -> Self {
                    (
                        value.amm,
                        value.minTradedToken0,
                        value.priceOracle,
                        value.priceOracleData,
                        value.appData,
                    )
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for updateParametersCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        amm: tuple.0,
                        minTradedToken0: tuple.1,
                        priceOracle: tuple.2,
                        priceOracleData: tuple.3,
                        appData: tuple.4,
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
            impl ::core::convert::From<updateParametersReturn>
            for UnderlyingRustTuple<'_> {
                fn from(value: updateParametersReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for updateParametersReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl updateParametersReturn {
            fn _tokenize(
                &self,
            ) -> <updateParametersCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for updateParametersCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Bytes,
                alloy_sol_types::sol_data::FixedBytes<32>,
            );
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = updateParametersReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "updateParameters(address,uint256,address,bytes,bytes32)";
            const SELECTOR: [u8; 4] = [91u8, 93u8, 158u8, 230u8];
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
                        &self.amm,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.minTradedToken0),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.priceOracle,
                    ),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.priceOracleData,
                    ),
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self.appData),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                updateParametersReturn::_tokenize(ret)
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
    /**Function with signature `withdraw(address,uint256,uint256)` and selector `0xb5c5f672`.
```solidity
function withdraw(address amm, uint256 amount0, uint256 amount1) external;
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct withdrawCall {
        #[allow(missing_docs)]
        pub amm: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub amount0: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub amount1: alloy_sol_types::private::primitives::aliases::U256,
    }
    ///Container type for the return parameters of the [`withdraw(address,uint256,uint256)`](withdrawCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct withdrawReturn {}
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
                alloy_sol_types::sol_data::Uint<256>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
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
            impl ::core::convert::From<withdrawCall> for UnderlyingRustTuple<'_> {
                fn from(value: withdrawCall) -> Self {
                    (value.amm, value.amount0, value.amount1)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for withdrawCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        amm: tuple.0,
                        amount0: tuple.1,
                        amount1: tuple.2,
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
            impl ::core::convert::From<withdrawReturn> for UnderlyingRustTuple<'_> {
                fn from(value: withdrawReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for withdrawReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl withdrawReturn {
            fn _tokenize(
                &self,
            ) -> <withdrawCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for withdrawCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = withdrawReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "withdraw(address,uint256,uint256)";
            const SELECTOR: [u8; 4] = [181u8, 197u8, 246u8, 114u8];
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
                        &self.amm,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.amount0),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.amount1),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                withdrawReturn::_tokenize(ret)
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
    ///Container for all the [`CowAmmConstantProductFactory`](self) function calls.
    #[derive(Clone)]
    #[derive()]
    pub enum CowAmmConstantProductFactoryCalls {
        #[allow(missing_docs)]
        ammDeterministicAddress(ammDeterministicAddressCall),
        #[allow(missing_docs)]
        create(createCall),
        #[allow(missing_docs)]
        deposit(depositCall),
        #[allow(missing_docs)]
        disableTrading(disableTradingCall),
        #[allow(missing_docs)]
        getTradeableOrderWithSignature(getTradeableOrderWithSignatureCall),
        #[allow(missing_docs)]
        owner(ownerCall),
        #[allow(missing_docs)]
        settler(settlerCall),
        #[allow(missing_docs)]
        updateParameters(updateParametersCall),
        #[allow(missing_docs)]
        withdraw(withdrawCall),
    }
    impl CowAmmConstantProductFactoryCalls {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the variants.
        /// No guarantees are made about the order of the selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 4usize]] = &[
            [14u8, 254u8, 106u8, 139u8],
            [34u8, 177u8, 85u8, 198u8],
            [38u8, 224u8, 161u8, 150u8],
            [39u8, 145u8, 5u8, 101u8],
            [55u8, 235u8, 223u8, 80u8],
            [91u8, 93u8, 158u8, 230u8],
            [102u8, 110u8, 27u8, 57u8],
            [171u8, 34u8, 26u8, 118u8],
            [181u8, 197u8, 246u8, 114u8],
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(deposit),
            ::core::stringify!(create),
            ::core::stringify!(getTradeableOrderWithSignature),
            ::core::stringify!(disableTrading),
            ::core::stringify!(ammDeterministicAddress),
            ::core::stringify!(updateParameters),
            ::core::stringify!(owner),
            ::core::stringify!(settler),
            ::core::stringify!(withdraw),
        ];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <depositCall as alloy_sol_types::SolCall>::SIGNATURE,
            <createCall as alloy_sol_types::SolCall>::SIGNATURE,
            <getTradeableOrderWithSignatureCall as alloy_sol_types::SolCall>::SIGNATURE,
            <disableTradingCall as alloy_sol_types::SolCall>::SIGNATURE,
            <ammDeterministicAddressCall as alloy_sol_types::SolCall>::SIGNATURE,
            <updateParametersCall as alloy_sol_types::SolCall>::SIGNATURE,
            <ownerCall as alloy_sol_types::SolCall>::SIGNATURE,
            <settlerCall as alloy_sol_types::SolCall>::SIGNATURE,
            <withdrawCall as alloy_sol_types::SolCall>::SIGNATURE,
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
    impl alloy_sol_types::SolInterface for CowAmmConstantProductFactoryCalls {
        const NAME: &'static str = "CowAmmConstantProductFactoryCalls";
        const MIN_DATA_LENGTH: usize = 0usize;
        const COUNT: usize = 9usize;
        #[inline]
        fn selector(&self) -> [u8; 4] {
            match self {
                Self::ammDeterministicAddress(_) => {
                    <ammDeterministicAddressCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::create(_) => <createCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::deposit(_) => <depositCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::disableTrading(_) => {
                    <disableTradingCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::getTradeableOrderWithSignature(_) => {
                    <getTradeableOrderWithSignatureCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::owner(_) => <ownerCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::settler(_) => <settlerCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::updateParameters(_) => {
                    <updateParametersCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::withdraw(_) => <withdrawCall as alloy_sol_types::SolCall>::SELECTOR,
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
            ) -> alloy_sol_types::Result<CowAmmConstantProductFactoryCalls>] = &[
                {
                    fn deposit(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmConstantProductFactoryCalls> {
                        <depositCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(CowAmmConstantProductFactoryCalls::deposit)
                    }
                    deposit
                },
                {
                    fn create(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmConstantProductFactoryCalls> {
                        <createCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(CowAmmConstantProductFactoryCalls::create)
                    }
                    create
                },
                {
                    fn getTradeableOrderWithSignature(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmConstantProductFactoryCalls> {
                        <getTradeableOrderWithSignatureCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(
                                CowAmmConstantProductFactoryCalls::getTradeableOrderWithSignature,
                            )
                    }
                    getTradeableOrderWithSignature
                },
                {
                    fn disableTrading(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmConstantProductFactoryCalls> {
                        <disableTradingCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(CowAmmConstantProductFactoryCalls::disableTrading)
                    }
                    disableTrading
                },
                {
                    fn ammDeterministicAddress(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmConstantProductFactoryCalls> {
                        <ammDeterministicAddressCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(
                                CowAmmConstantProductFactoryCalls::ammDeterministicAddress,
                            )
                    }
                    ammDeterministicAddress
                },
                {
                    fn updateParameters(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmConstantProductFactoryCalls> {
                        <updateParametersCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(CowAmmConstantProductFactoryCalls::updateParameters)
                    }
                    updateParameters
                },
                {
                    fn owner(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmConstantProductFactoryCalls> {
                        <ownerCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(CowAmmConstantProductFactoryCalls::owner)
                    }
                    owner
                },
                {
                    fn settler(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmConstantProductFactoryCalls> {
                        <settlerCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(CowAmmConstantProductFactoryCalls::settler)
                    }
                    settler
                },
                {
                    fn withdraw(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmConstantProductFactoryCalls> {
                        <withdrawCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(CowAmmConstantProductFactoryCalls::withdraw)
                    }
                    withdraw
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
            ) -> alloy_sol_types::Result<CowAmmConstantProductFactoryCalls>] = &[
                {
                    fn deposit(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmConstantProductFactoryCalls> {
                        <depositCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CowAmmConstantProductFactoryCalls::deposit)
                    }
                    deposit
                },
                {
                    fn create(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmConstantProductFactoryCalls> {
                        <createCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CowAmmConstantProductFactoryCalls::create)
                    }
                    create
                },
                {
                    fn getTradeableOrderWithSignature(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmConstantProductFactoryCalls> {
                        <getTradeableOrderWithSignatureCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(
                                CowAmmConstantProductFactoryCalls::getTradeableOrderWithSignature,
                            )
                    }
                    getTradeableOrderWithSignature
                },
                {
                    fn disableTrading(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmConstantProductFactoryCalls> {
                        <disableTradingCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CowAmmConstantProductFactoryCalls::disableTrading)
                    }
                    disableTrading
                },
                {
                    fn ammDeterministicAddress(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmConstantProductFactoryCalls> {
                        <ammDeterministicAddressCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(
                                CowAmmConstantProductFactoryCalls::ammDeterministicAddress,
                            )
                    }
                    ammDeterministicAddress
                },
                {
                    fn updateParameters(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmConstantProductFactoryCalls> {
                        <updateParametersCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CowAmmConstantProductFactoryCalls::updateParameters)
                    }
                    updateParameters
                },
                {
                    fn owner(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmConstantProductFactoryCalls> {
                        <ownerCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CowAmmConstantProductFactoryCalls::owner)
                    }
                    owner
                },
                {
                    fn settler(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmConstantProductFactoryCalls> {
                        <settlerCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CowAmmConstantProductFactoryCalls::settler)
                    }
                    settler
                },
                {
                    fn withdraw(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmConstantProductFactoryCalls> {
                        <withdrawCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CowAmmConstantProductFactoryCalls::withdraw)
                    }
                    withdraw
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
                Self::ammDeterministicAddress(inner) => {
                    <ammDeterministicAddressCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::create(inner) => {
                    <createCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::deposit(inner) => {
                    <depositCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::disableTrading(inner) => {
                    <disableTradingCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::getTradeableOrderWithSignature(inner) => {
                    <getTradeableOrderWithSignatureCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::owner(inner) => {
                    <ownerCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::settler(inner) => {
                    <settlerCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::updateParameters(inner) => {
                    <updateParametersCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::withdraw(inner) => {
                    <withdrawCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
            }
        }
        #[inline]
        fn abi_encode_raw(&self, out: &mut alloy_sol_types::private::Vec<u8>) {
            match self {
                Self::ammDeterministicAddress(inner) => {
                    <ammDeterministicAddressCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::create(inner) => {
                    <createCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::deposit(inner) => {
                    <depositCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::disableTrading(inner) => {
                    <disableTradingCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::getTradeableOrderWithSignature(inner) => {
                    <getTradeableOrderWithSignatureCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::owner(inner) => {
                    <ownerCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::settler(inner) => {
                    <settlerCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::updateParameters(inner) => {
                    <updateParametersCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::withdraw(inner) => {
                    <withdrawCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
            }
        }
    }
    ///Container for all the [`CowAmmConstantProductFactory`](self) custom errors.
    #[derive(Clone)]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub enum CowAmmConstantProductFactoryErrors {
        #[allow(missing_docs)]
        OnlyOwnerCanCall(OnlyOwnerCanCall),
        #[allow(missing_docs)]
        OrderNotValid(OrderNotValid),
    }
    impl CowAmmConstantProductFactoryErrors {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the variants.
        /// No guarantees are made about the order of the selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 4usize]] = &[
            [104u8, 186u8, 255u8, 248u8],
            [200u8, 252u8, 39u8, 37u8],
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(OnlyOwnerCanCall),
            ::core::stringify!(OrderNotValid),
        ];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <OnlyOwnerCanCall as alloy_sol_types::SolError>::SIGNATURE,
            <OrderNotValid as alloy_sol_types::SolError>::SIGNATURE,
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
    impl alloy_sol_types::SolInterface for CowAmmConstantProductFactoryErrors {
        const NAME: &'static str = "CowAmmConstantProductFactoryErrors";
        const MIN_DATA_LENGTH: usize = 32usize;
        const COUNT: usize = 2usize;
        #[inline]
        fn selector(&self) -> [u8; 4] {
            match self {
                Self::OnlyOwnerCanCall(_) => {
                    <OnlyOwnerCanCall as alloy_sol_types::SolError>::SELECTOR
                }
                Self::OrderNotValid(_) => {
                    <OrderNotValid as alloy_sol_types::SolError>::SELECTOR
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
            ) -> alloy_sol_types::Result<CowAmmConstantProductFactoryErrors>] = &[
                {
                    fn OnlyOwnerCanCall(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmConstantProductFactoryErrors> {
                        <OnlyOwnerCanCall as alloy_sol_types::SolError>::abi_decode_raw(
                                data,
                            )
                            .map(CowAmmConstantProductFactoryErrors::OnlyOwnerCanCall)
                    }
                    OnlyOwnerCanCall
                },
                {
                    fn OrderNotValid(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmConstantProductFactoryErrors> {
                        <OrderNotValid as alloy_sol_types::SolError>::abi_decode_raw(
                                data,
                            )
                            .map(CowAmmConstantProductFactoryErrors::OrderNotValid)
                    }
                    OrderNotValid
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
            ) -> alloy_sol_types::Result<CowAmmConstantProductFactoryErrors>] = &[
                {
                    fn OnlyOwnerCanCall(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmConstantProductFactoryErrors> {
                        <OnlyOwnerCanCall as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CowAmmConstantProductFactoryErrors::OnlyOwnerCanCall)
                    }
                    OnlyOwnerCanCall
                },
                {
                    fn OrderNotValid(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmConstantProductFactoryErrors> {
                        <OrderNotValid as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CowAmmConstantProductFactoryErrors::OrderNotValid)
                    }
                    OrderNotValid
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
                Self::OnlyOwnerCanCall(inner) => {
                    <OnlyOwnerCanCall as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::OrderNotValid(inner) => {
                    <OrderNotValid as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
            }
        }
        #[inline]
        fn abi_encode_raw(&self, out: &mut alloy_sol_types::private::Vec<u8>) {
            match self {
                Self::OnlyOwnerCanCall(inner) => {
                    <OnlyOwnerCanCall as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::OrderNotValid(inner) => {
                    <OrderNotValid as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
            }
        }
    }
    ///Container for all the [`CowAmmConstantProductFactory`](self) events.
    #[derive(Clone)]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub enum CowAmmConstantProductFactoryEvents {
        #[allow(missing_docs)]
        ConditionalOrderCreated(ConditionalOrderCreated),
        #[allow(missing_docs)]
        Deployed(Deployed),
        #[allow(missing_docs)]
        TradingDisabled(TradingDisabled),
    }
    impl CowAmmConstantProductFactoryEvents {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the variants.
        /// No guarantees are made about the order of the selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 32usize]] = &[
            [
                44u8, 206u8, 172u8, 85u8, 85u8, 176u8, 202u8, 69u8, 163u8, 116u8, 76u8,
                237u8, 84u8, 47u8, 84u8, 181u8, 106u8, 210u8, 235u8, 69u8, 229u8, 33u8,
                150u8, 35u8, 114u8, 238u8, 242u8, 18u8, 162u8, 203u8, 243u8, 97u8,
            ],
            [
                103u8, 7u8, 37u8, 91u8, 44u8, 92u8, 168u8, 18u8, 32u8, 178u8, 243u8,
                228u8, 8u8, 162u8, 105u8, 203u8, 131u8, 186u8, 166u8, 170u8, 126u8, 94u8,
                55u8, 170u8, 23u8, 86u8, 136u8, 58u8, 108u8, 223u8, 6u8, 241u8,
            ],
            [
                199u8, 91u8, 244u8, 240u8, 60u8, 2u8, 250u8, 185u8, 65u8, 74u8, 125u8,
                122u8, 84u8, 4u8, 140u8, 4u8, 134u8, 114u8, 43u8, 199u8, 47u8, 51u8,
                173u8, 146u8, 71u8, 9u8, 160u8, 89u8, 54u8, 8u8, 173u8, 39u8,
            ],
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(ConditionalOrderCreated),
            ::core::stringify!(Deployed),
            ::core::stringify!(TradingDisabled),
        ];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <ConditionalOrderCreated as alloy_sol_types::SolEvent>::SIGNATURE,
            <Deployed as alloy_sol_types::SolEvent>::SIGNATURE,
            <TradingDisabled as alloy_sol_types::SolEvent>::SIGNATURE,
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
    impl alloy_sol_types::SolEventInterface for CowAmmConstantProductFactoryEvents {
        const NAME: &'static str = "CowAmmConstantProductFactoryEvents";
        const COUNT: usize = 3usize;
        fn decode_raw_log(
            topics: &[alloy_sol_types::Word],
            data: &[u8],
        ) -> alloy_sol_types::Result<Self> {
            match topics.first().copied() {
                Some(
                    <ConditionalOrderCreated as alloy_sol_types::SolEvent>::SIGNATURE_HASH,
                ) => {
                    <ConditionalOrderCreated as alloy_sol_types::SolEvent>::decode_raw_log(
                            topics,
                            data,
                        )
                        .map(Self::ConditionalOrderCreated)
                }
                Some(<Deployed as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <Deployed as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::Deployed)
                }
                Some(<TradingDisabled as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <TradingDisabled as alloy_sol_types::SolEvent>::decode_raw_log(
                            topics,
                            data,
                        )
                        .map(Self::TradingDisabled)
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
    impl alloy_sol_types::private::IntoLogData for CowAmmConstantProductFactoryEvents {
        fn to_log_data(&self) -> alloy_sol_types::private::LogData {
            match self {
                Self::ConditionalOrderCreated(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::Deployed(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::TradingDisabled(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
            }
        }
        fn into_log_data(self) -> alloy_sol_types::private::LogData {
            match self {
                Self::ConditionalOrderCreated(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::Deployed(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::TradingDisabled(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
            }
        }
    }
    use alloy_contract as alloy_contract;
    /**Creates a new wrapper around an on-chain [`CowAmmConstantProductFactory`](self) contract instance.

See the [wrapper's documentation](`CowAmmConstantProductFactoryInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> CowAmmConstantProductFactoryInstance<P, N> {
        CowAmmConstantProductFactoryInstance::<P, N>::new(address, __provider)
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
        _settler: alloy_sol_types::private::Address,
    ) -> impl ::core::future::Future<
        Output = alloy_contract::Result<CowAmmConstantProductFactoryInstance<P, N>>,
    > {
        CowAmmConstantProductFactoryInstance::<P, N>::deploy(__provider, _settler)
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
        _settler: alloy_sol_types::private::Address,
    ) -> alloy_contract::RawCallBuilder<P, N> {
        CowAmmConstantProductFactoryInstance::<
            P,
            N,
        >::deploy_builder(__provider, _settler)
    }
    /**A [`CowAmmConstantProductFactory`](self) instance.

Contains type-safe methods for interacting with an on-chain instance of the
[`CowAmmConstantProductFactory`](self) contract located at a given `address`, using a given
provider `P`.

If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
documentation on how to provide it), the `deploy` and `deploy_builder` methods can
be used to deploy a new instance of the contract.

See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct CowAmmConstantProductFactoryInstance<
        P,
        N = alloy_contract::private::Ethereum,
    > {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for CowAmmConstantProductFactoryInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("CowAmmConstantProductFactoryInstance")
                .field(&self.address)
                .finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    > CowAmmConstantProductFactoryInstance<P, N> {
        /**Creates a new wrapper around an on-chain [`CowAmmConstantProductFactory`](self) contract instance.

See the [wrapper's documentation](`CowAmmConstantProductFactoryInstance`) for more details.*/
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
            _settler: alloy_sol_types::private::Address,
        ) -> alloy_contract::Result<CowAmmConstantProductFactoryInstance<P, N>> {
            let call_builder = Self::deploy_builder(__provider, _settler);
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
            _settler: alloy_sol_types::private::Address,
        ) -> alloy_contract::RawCallBuilder<P, N> {
            alloy_contract::RawCallBuilder::new_raw_deploy(
                __provider,
                [
                    &BYTECODE[..],
                    &alloy_sol_types::SolConstructor::abi_encode(
                        &constructorCall { _settler },
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
    impl<P: ::core::clone::Clone, N> CowAmmConstantProductFactoryInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned provider.
        #[inline]
        pub fn with_cloned_provider(self) -> CowAmmConstantProductFactoryInstance<P, N> {
            CowAmmConstantProductFactoryInstance {
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
    > CowAmmConstantProductFactoryInstance<P, N> {
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
        ///Creates a new call builder for the [`ammDeterministicAddress`] function.
        pub fn ammDeterministicAddress(
            &self,
            ammOwner: alloy_sol_types::private::Address,
            token0: alloy_sol_types::private::Address,
            token1: alloy_sol_types::private::Address,
        ) -> alloy_contract::SolCallBuilder<&P, ammDeterministicAddressCall, N> {
            self.call_builder(
                &ammDeterministicAddressCall {
                    ammOwner,
                    token0,
                    token1,
                },
            )
        }
        ///Creates a new call builder for the [`create`] function.
        pub fn create(
            &self,
            token0: alloy_sol_types::private::Address,
            amount0: alloy_sol_types::private::primitives::aliases::U256,
            token1: alloy_sol_types::private::Address,
            amount1: alloy_sol_types::private::primitives::aliases::U256,
            minTradedToken0: alloy_sol_types::private::primitives::aliases::U256,
            priceOracle: alloy_sol_types::private::Address,
            priceOracleData: alloy_sol_types::private::Bytes,
            appData: alloy_sol_types::private::FixedBytes<32>,
        ) -> alloy_contract::SolCallBuilder<&P, createCall, N> {
            self.call_builder(
                &createCall {
                    token0,
                    amount0,
                    token1,
                    amount1,
                    minTradedToken0,
                    priceOracle,
                    priceOracleData,
                    appData,
                },
            )
        }
        ///Creates a new call builder for the [`deposit`] function.
        pub fn deposit(
            &self,
            amm: alloy_sol_types::private::Address,
            amount0: alloy_sol_types::private::primitives::aliases::U256,
            amount1: alloy_sol_types::private::primitives::aliases::U256,
        ) -> alloy_contract::SolCallBuilder<&P, depositCall, N> {
            self.call_builder(
                &depositCall {
                    amm,
                    amount0,
                    amount1,
                },
            )
        }
        ///Creates a new call builder for the [`disableTrading`] function.
        pub fn disableTrading(
            &self,
            amm: alloy_sol_types::private::Address,
        ) -> alloy_contract::SolCallBuilder<&P, disableTradingCall, N> {
            self.call_builder(&disableTradingCall { amm })
        }
        ///Creates a new call builder for the [`getTradeableOrderWithSignature`] function.
        pub fn getTradeableOrderWithSignature(
            &self,
            amm: alloy_sol_types::private::Address,
            params: <IConditionalOrder::ConditionalOrderParams as alloy_sol_types::SolType>::RustType,
            _2: alloy_sol_types::private::Bytes,
            _3: alloy_sol_types::private::Vec<alloy_sol_types::private::FixedBytes<32>>,
        ) -> alloy_contract::SolCallBuilder<&P, getTradeableOrderWithSignatureCall, N> {
            self.call_builder(
                &getTradeableOrderWithSignatureCall {
                    amm,
                    params,
                    _2,
                    _3,
                },
            )
        }
        ///Creates a new call builder for the [`owner`] function.
        pub fn owner(
            &self,
            _0: alloy_sol_types::private::Address,
        ) -> alloy_contract::SolCallBuilder<&P, ownerCall, N> {
            self.call_builder(&ownerCall(_0))
        }
        ///Creates a new call builder for the [`settler`] function.
        pub fn settler(&self) -> alloy_contract::SolCallBuilder<&P, settlerCall, N> {
            self.call_builder(&settlerCall)
        }
        ///Creates a new call builder for the [`updateParameters`] function.
        pub fn updateParameters(
            &self,
            amm: alloy_sol_types::private::Address,
            minTradedToken0: alloy_sol_types::private::primitives::aliases::U256,
            priceOracle: alloy_sol_types::private::Address,
            priceOracleData: alloy_sol_types::private::Bytes,
            appData: alloy_sol_types::private::FixedBytes<32>,
        ) -> alloy_contract::SolCallBuilder<&P, updateParametersCall, N> {
            self.call_builder(
                &updateParametersCall {
                    amm,
                    minTradedToken0,
                    priceOracle,
                    priceOracleData,
                    appData,
                },
            )
        }
        ///Creates a new call builder for the [`withdraw`] function.
        pub fn withdraw(
            &self,
            amm: alloy_sol_types::private::Address,
            amount0: alloy_sol_types::private::primitives::aliases::U256,
            amount1: alloy_sol_types::private::primitives::aliases::U256,
        ) -> alloy_contract::SolCallBuilder<&P, withdrawCall, N> {
            self.call_builder(
                &withdrawCall {
                    amm,
                    amount0,
                    amount1,
                },
            )
        }
    }
    /// Event filters.
    impl<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    > CowAmmConstantProductFactoryInstance<P, N> {
        /// Creates a new event filter using this contract instance's provider and address.
        ///
        /// Note that the type can be any event, not just those defined in this contract.
        /// Prefer using the other methods for building type-safe event filters.
        pub fn event_filter<E: alloy_sol_types::SolEvent>(
            &self,
        ) -> alloy_contract::Event<&P, E, N> {
            alloy_contract::Event::new_sol(&self.provider, &self.address)
        }
        ///Creates a new event filter for the [`ConditionalOrderCreated`] event.
        pub fn ConditionalOrderCreated_filter(
            &self,
        ) -> alloy_contract::Event<&P, ConditionalOrderCreated, N> {
            self.event_filter::<ConditionalOrderCreated>()
        }
        ///Creates a new event filter for the [`Deployed`] event.
        pub fn Deployed_filter(&self) -> alloy_contract::Event<&P, Deployed, N> {
            self.event_filter::<Deployed>()
        }
        ///Creates a new event filter for the [`TradingDisabled`] event.
        pub fn TradingDisabled_filter(
            &self,
        ) -> alloy_contract::Event<&P, TradingDisabled, N> {
            self.event_filter::<TradingDisabled>()
        }
    }
}
pub type Instance = CowAmmConstantProductFactory::CowAmmConstantProductFactoryInstance<
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
                    "0xb808e8183e3a72d196457d127c7fd4befa0d7fd3"
                ),
                Some(5874562u64),
            ))
        }
        100u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0xdb1cba3a87f2db53b6e1e6af48e28ed877592ec0"
                ),
                Some(33874317u64),
            ))
        }
        1u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0x40664207e3375FB4b733d4743CE9b159331fd034"
                ),
                Some(19861952u64),
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
