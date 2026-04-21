#![allow(unused_imports, unused_attributes, clippy::all, rustdoc::all, non_snake_case)]
//! Auto-generated contract bindings. Do not edit.
///Module containing a contract's types and functions.
/**

```solidity
library GPv2Interaction {
    struct Data { address target; uint256 value; bytes callData; }
}
```*/
#[allow(
    non_camel_case_types,
    non_snake_case,
    clippy::pub_underscore_fields,
    clippy::style,
    clippy::empty_structs_with_brackets
)]
pub mod GPv2Interaction {
    use super::*;
    use alloy_sol_types as alloy_sol_types;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**```solidity
struct Data { address target; uint256 value; bytes callData; }
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct Data {
        #[allow(missing_docs)]
        pub target: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub value: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub callData: alloy_sol_types::private::Bytes,
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
            alloy_sol_types::sol_data::Bytes,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
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
        impl ::core::convert::From<Data> for UnderlyingRustTuple<'_> {
            fn from(value: Data) -> Self {
                (value.target, value.value, value.callData)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for Data {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    target: tuple.0,
                    value: tuple.1,
                    callData: tuple.2,
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
                        &self.target,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.value),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.callData,
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
                    "Data(address target,uint256 value,bytes callData)",
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
                            &self.target,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.value)
                        .0,
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::eip712_data_word(
                            &self.callData,
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
                        &rust.target,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(&rust.value)
                    + <alloy_sol_types::sol_data::Bytes as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.callData,
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
                    &rust.target,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.value,
                    out,
                );
                <alloy_sol_types::sol_data::Bytes as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.callData,
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
    /**Creates a new wrapper around an on-chain [`GPv2Interaction`](self) contract instance.

See the [wrapper's documentation](`GPv2InteractionInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> GPv2InteractionInstance<P, N> {
        GPv2InteractionInstance::<P, N>::new(address, __provider)
    }
    /**A [`GPv2Interaction`](self) instance.

Contains type-safe methods for interacting with an on-chain instance of the
[`GPv2Interaction`](self) contract located at a given `address`, using a given
provider `P`.

If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
documentation on how to provide it), the `deploy` and `deploy_builder` methods can
be used to deploy a new instance of the contract.

See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct GPv2InteractionInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for GPv2InteractionInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("GPv2InteractionInstance").field(&self.address).finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    > GPv2InteractionInstance<P, N> {
        /**Creates a new wrapper around an on-chain [`GPv2Interaction`](self) contract instance.

See the [wrapper's documentation](`GPv2InteractionInstance`) for more details.*/
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
    impl<P: ::core::clone::Clone, N> GPv2InteractionInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned provider.
        #[inline]
        pub fn with_cloned_provider(self) -> GPv2InteractionInstance<P, N> {
            GPv2InteractionInstance {
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
    > GPv2InteractionInstance<P, N> {
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
    > GPv2InteractionInstance<P, N> {
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
library GPv2Trade {
    struct Data { uint256 sellTokenIndex; uint256 buyTokenIndex; address receiver; uint256 sellAmount; uint256 buyAmount; uint32 validTo; bytes32 appData; uint256 feeAmount; uint256 flags; uint256 executedAmount; bytes signature; }
}
```*/
#[allow(
    non_camel_case_types,
    non_snake_case,
    clippy::pub_underscore_fields,
    clippy::style,
    clippy::empty_structs_with_brackets
)]
pub mod GPv2Trade {
    use super::*;
    use alloy_sol_types as alloy_sol_types;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**```solidity
struct Data { uint256 sellTokenIndex; uint256 buyTokenIndex; address receiver; uint256 sellAmount; uint256 buyAmount; uint32 validTo; bytes32 appData; uint256 feeAmount; uint256 flags; uint256 executedAmount; bytes signature; }
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct Data {
        #[allow(missing_docs)]
        pub sellTokenIndex: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub buyTokenIndex: alloy_sol_types::private::primitives::aliases::U256,
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
        pub flags: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub executedAmount: alloy_sol_types::private::primitives::aliases::U256,
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
        #[doc(hidden)]
        #[allow(dead_code)]
        type UnderlyingSolTuple<'a> = (
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<32>,
            alloy_sol_types::sol_data::FixedBytes<32>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Bytes,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::primitives::aliases::U256,
            alloy_sol_types::private::primitives::aliases::U256,
            alloy_sol_types::private::Address,
            alloy_sol_types::private::primitives::aliases::U256,
            alloy_sol_types::private::primitives::aliases::U256,
            u32,
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
        impl ::core::convert::From<Data> for UnderlyingRustTuple<'_> {
            fn from(value: Data) -> Self {
                (
                    value.sellTokenIndex,
                    value.buyTokenIndex,
                    value.receiver,
                    value.sellAmount,
                    value.buyAmount,
                    value.validTo,
                    value.appData,
                    value.feeAmount,
                    value.flags,
                    value.executedAmount,
                    value.signature,
                )
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for Data {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    sellTokenIndex: tuple.0,
                    buyTokenIndex: tuple.1,
                    receiver: tuple.2,
                    sellAmount: tuple.3,
                    buyAmount: tuple.4,
                    validTo: tuple.5,
                    appData: tuple.6,
                    feeAmount: tuple.7,
                    flags: tuple.8,
                    executedAmount: tuple.9,
                    signature: tuple.10,
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
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.sellTokenIndex),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.buyTokenIndex),
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
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.flags),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.executedAmount),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.signature,
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
                    "Data(uint256 sellTokenIndex,uint256 buyTokenIndex,address receiver,uint256 sellAmount,uint256 buyAmount,uint32 validTo,bytes32 appData,uint256 feeAmount,uint256 flags,uint256 executedAmount,bytes signature)",
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
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(
                            &self.sellTokenIndex,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.buyTokenIndex)
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
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.flags)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(
                            &self.executedAmount,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::eip712_data_word(
                            &self.signature,
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
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.sellTokenIndex,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.buyTokenIndex,
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
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(&rust.flags)
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.executedAmount,
                    )
                    + <alloy_sol_types::sol_data::Bytes as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.signature,
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
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.sellTokenIndex,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.buyTokenIndex,
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
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.flags,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.executedAmount,
                    out,
                );
                <alloy_sol_types::sol_data::Bytes as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.signature,
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
    /**Creates a new wrapper around an on-chain [`GPv2Trade`](self) contract instance.

See the [wrapper's documentation](`GPv2TradeInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> GPv2TradeInstance<P, N> {
        GPv2TradeInstance::<P, N>::new(address, __provider)
    }
    /**A [`GPv2Trade`](self) instance.

Contains type-safe methods for interacting with an on-chain instance of the
[`GPv2Trade`](self) contract located at a given `address`, using a given
provider `P`.

If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
documentation on how to provide it), the `deploy` and `deploy_builder` methods can
be used to deploy a new instance of the contract.

See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct GPv2TradeInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for GPv2TradeInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("GPv2TradeInstance").field(&self.address).finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    > GPv2TradeInstance<P, N> {
        /**Creates a new wrapper around an on-chain [`GPv2Trade`](self) contract instance.

See the [wrapper's documentation](`GPv2TradeInstance`) for more details.*/
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
    impl<P: ::core::clone::Clone, N> GPv2TradeInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned provider.
        #[inline]
        pub fn with_cloned_provider(self) -> GPv2TradeInstance<P, N> {
            GPv2TradeInstance {
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
    > GPv2TradeInstance<P, N> {
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
    > GPv2TradeInstance<P, N> {
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
library IVault {
    struct BatchSwapStep { bytes32 poolId; uint256 assetInIndex; uint256 assetOutIndex; uint256 amount; bytes userData; }
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
library GPv2Interaction {
    struct Data {
        address target;
        uint256 value;
        bytes callData;
    }
}

library GPv2Trade {
    struct Data {
        uint256 sellTokenIndex;
        uint256 buyTokenIndex;
        address receiver;
        uint256 sellAmount;
        uint256 buyAmount;
        uint32 validTo;
        bytes32 appData;
        uint256 feeAmount;
        uint256 flags;
        uint256 executedAmount;
        bytes signature;
    }
}

library IVault {
    struct BatchSwapStep {
        bytes32 poolId;
        uint256 assetInIndex;
        uint256 assetOutIndex;
        uint256 amount;
        bytes userData;
    }
}

interface GPv2Settlement {
    event Interaction(address indexed target, uint256 value, bytes4 selector);
    event OrderInvalidated(address indexed owner, bytes orderUid);
    event PreSignature(address indexed owner, bytes orderUid, bool signed);
    event Settlement(address indexed solver);
    event Trade(address indexed owner, address sellToken, address buyToken, uint256 sellAmount, uint256 buyAmount, uint256 feeAmount, bytes orderUid);

    constructor(address authenticator_, address vault_);

    receive() external payable;

    function authenticator() external view returns (address);
    function domainSeparator() external view returns (bytes32);
    function filledAmount(bytes memory) external view returns (uint256);
    function invalidateOrder(bytes memory orderUid) external;
    function setPreSignature(bytes memory orderUid, bool signed) external;
    function settle(address[] memory tokens, uint256[] memory clearingPrices, GPv2Trade.Data[] memory trades, GPv2Interaction.Data[][3] memory interactions) external;
    function simulateDelegatecall(address targetContract, bytes memory calldataPayload) external returns (bytes memory response);
    function swap(IVault.BatchSwapStep[] memory swaps, address[] memory tokens, GPv2Trade.Data memory trade) external;
    function vault() external view returns (address);
    function vaultRelayer() external view returns (address);
}
```

...which was generated by the following JSON ABI:
```json
[
  {
    "type": "constructor",
    "inputs": [
      {
        "name": "authenticator_",
        "type": "address",
        "internalType": "contract GPv2Authentication"
      },
      {
        "name": "vault_",
        "type": "address",
        "internalType": "contract IVault"
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
    "name": "authenticator",
    "inputs": [],
    "outputs": [
      {
        "name": "",
        "type": "address",
        "internalType": "contract GPv2Authentication"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "domainSeparator",
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
    "name": "filledAmount",
    "inputs": [
      {
        "name": "",
        "type": "bytes",
        "internalType": "bytes"
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
    "name": "invalidateOrder",
    "inputs": [
      {
        "name": "orderUid",
        "type": "bytes",
        "internalType": "bytes"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "setPreSignature",
    "inputs": [
      {
        "name": "orderUid",
        "type": "bytes",
        "internalType": "bytes"
      },
      {
        "name": "signed",
        "type": "bool",
        "internalType": "bool"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "settle",
    "inputs": [
      {
        "name": "tokens",
        "type": "address[]",
        "internalType": "contract IERC20[]"
      },
      {
        "name": "clearingPrices",
        "type": "uint256[]",
        "internalType": "uint256[]"
      },
      {
        "name": "trades",
        "type": "tuple[]",
        "internalType": "struct GPv2Trade.Data[]",
        "components": [
          {
            "name": "sellTokenIndex",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "buyTokenIndex",
            "type": "uint256",
            "internalType": "uint256"
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
            "name": "flags",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "executedAmount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "signature",
            "type": "bytes",
            "internalType": "bytes"
          }
        ]
      },
      {
        "name": "interactions",
        "type": "tuple[][3]",
        "internalType": "struct GPv2Interaction.Data[][3]",
        "components": [
          {
            "name": "target",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "value",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "callData",
            "type": "bytes",
            "internalType": "bytes"
          }
        ]
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "simulateDelegatecall",
    "inputs": [
      {
        "name": "targetContract",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "calldataPayload",
        "type": "bytes",
        "internalType": "bytes"
      }
    ],
    "outputs": [
      {
        "name": "response",
        "type": "bytes",
        "internalType": "bytes"
      }
    ],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "swap",
    "inputs": [
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
        "name": "tokens",
        "type": "address[]",
        "internalType": "contract IERC20[]"
      },
      {
        "name": "trade",
        "type": "tuple",
        "internalType": "struct GPv2Trade.Data",
        "components": [
          {
            "name": "sellTokenIndex",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "buyTokenIndex",
            "type": "uint256",
            "internalType": "uint256"
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
            "name": "flags",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "executedAmount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "signature",
            "type": "bytes",
            "internalType": "bytes"
          }
        ]
      }
    ],
    "outputs": [],
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
  },
  {
    "type": "function",
    "name": "vaultRelayer",
    "inputs": [],
    "outputs": [
      {
        "name": "",
        "type": "address",
        "internalType": "contract GPv2VaultRelayer"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "event",
    "name": "Interaction",
    "inputs": [
      {
        "name": "target",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "value",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "selector",
        "type": "bytes4",
        "indexed": false,
        "internalType": "bytes4"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "OrderInvalidated",
    "inputs": [
      {
        "name": "owner",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "orderUid",
        "type": "bytes",
        "indexed": false,
        "internalType": "bytes"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "PreSignature",
    "inputs": [
      {
        "name": "owner",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "orderUid",
        "type": "bytes",
        "indexed": false,
        "internalType": "bytes"
      },
      {
        "name": "signed",
        "type": "bool",
        "indexed": false,
        "internalType": "bool"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "Settlement",
    "inputs": [
      {
        "name": "solver",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "Trade",
    "inputs": [
      {
        "name": "owner",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "sellToken",
        "type": "address",
        "indexed": false,
        "internalType": "contract IERC20"
      },
      {
        "name": "buyToken",
        "type": "address",
        "indexed": false,
        "internalType": "contract IERC20"
      },
      {
        "name": "sellAmount",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "buyAmount",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "feeAmount",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "orderUid",
        "type": "bytes",
        "indexed": false,
        "internalType": "bytes"
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
pub mod GPv2Settlement {
    use super::*;
    use alloy_sol_types as alloy_sol_types;
    /// The creation / init bytecode of the contract.
    ///
    /// ```text
    ///0x6101006040523480156200001257600080fd5b50604051620053eb380380620053eb83398101604081905262000035916200015b565b604080517f8b73c3c69bb8fe3d512ecc4cf759cc79239f7b179b0ffacaa9a75d522b39400f6020808301919091527f6c85c0337eba1661327f94f3bf46c8a7f9311a563f4d5c948362567f5d8ed60c828401527ff9446b8e937d86f0bc87cac73923491692b123ca5f8761908494703758206adf606080840191909152466080808501919091523060a08086019190915285518086038201815260c09586019687905280519401939093209052600180556001600160601b031986821b811690925284901b16905281906200010a906200014d565b62000116919062000199565b604051809103906000f08015801562000133573d6000803e3d6000fd5b5060601b6001600160601b03191660e05250620001c69050565b61129e806200414d83390190565b600080604083850312156200016e578182fd5b82516200017b81620001ad565b60208401519092506200018e81620001ad565b809150509250929050565b6001600160a01b0391909116815260200190565b6001600160a01b0381168114620001c357600080fd5b50565b60805160a05160601c60c05160601c60e05160601c613f2562000228600039806104c55280610d61528061109052806115f0525080610556528061158b52508061039252806106bc528061099d52508061131e52806123df5250613f256000f3fe6080604052600436106100ec5760003560e01c80639b552cc21161008a578063ed9f35ce11610059578063ed9f35ce14610274578063f698da2514610294578063f84436bd146102a9578063fbfa77cf146102c9576100f3565b80639b552cc2146101ff578063a2a7d51b14610214578063d08d33d114610234578063ec6cb13f14610254576100f3565b80632479fb6e116100c65780632479fb6e1461016557806343218e19146101925780635624b25b146101bf578063845a101f146101df576100f3565b806313d79a0b146100f857806315337bc01461011a5780632335c76b1461013a576100f3565b366100f357005b600080fd5b34801561010457600080fd5b5061011861011336600461322e565b6102de565b005b34801561012657600080fd5b50610118610135366004613441565b6105c1565b34801561014657600080fd5b5061014f6106ba565b60405161015c91906136ee565b60405180910390f35b34801561017157600080fd5b506101856101803660046134ca565b6106de565b60405161015c91906137f0565b34801561019e57600080fd5b506101b26101ad3660046131a0565b6106fb565b60405161015c919061380d565b3480156101cb57600080fd5b506101b26101da3660046134fd565b610873565b3480156101eb57600080fd5b506101186101fa36600461338e565b6108e9565b34801561020b57600080fd5b5061014f61108e565b34801561022057600080fd5b5061011861022f3660046131ee565b6110b2565b34801561024057600080fd5b5061018561024f3660046134ca565b6110fb565b34801561026057600080fd5b5061011861026f366004613475565b611118565b34801561028057600080fd5b5061011861028f3660046131ee565b6112d7565b3480156102a057600080fd5b5061018561131c565b3480156102b557600080fd5b506101b26102c43660046131a0565b611340565b3480156102d557600080fd5b5061014f611589565b6002600154141561035057604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601f60248201527f5265656e7472616e637947756172643a207265656e7472616e742063616c6c00604482015290519081900360640190fd5b60026001556040517f02cc250d00000000000000000000000000000000000000000000000000000000815273ffffffffffffffffffffffffffffffffffffffff7f000000000000000000000000000000000000000000000000000000000000000016906302cc250d906103c79033906004016136ee565b60206040518083038186803b1580156103df57600080fd5b505afa1580156103f3573d6000803e3d6000fd5b505050506040513d601f19601f820116820180604052508101906104179190613425565b610456576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040161044d90613c78565b60405180910390fd5b6104728160005b60200281019061046d9190613d16565b6115ad565b6000806104838989898989896116ea565b6040517f7d10d11f000000000000000000000000000000000000000000000000000000008152919350915073ffffffffffffffffffffffffffffffffffffffff7f00000000000000000000000000000000000000000000000000000000000000001690637d10d11f906104fa90859060040161370f565b600060405180830381600087803b15801561051457600080fd5b505af1158015610528573d6000803e3d6000fd5b5050505061053c8360016003811061045d57fe5b61057c73ffffffffffffffffffffffffffffffffffffffff7f00000000000000000000000000000000000000000000000000000000000000001682611851565b61058783600261045d565b60405133907f40338ce1a7c49204f0099533b1e9a7ee0a3d261f84974ab7af36105b8c4e9db490600090a250506001805550505050505050565b60006105cd8383611b2f565b5091505073ffffffffffffffffffffffffffffffffffffffff81163314610620576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040161044d90613a1b565b7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff600284846040516106539291906136c2565b9081526020016040518091039020819055508073ffffffffffffffffffffffffffffffffffffffff167f875b6cb035bbd4ac6500fabc6d1e4ca5bdc58a3e2b424ccb5c24cdbebeb009a984846040516106ad9291906137f9565b60405180910390a2505050565b7f000000000000000000000000000000000000000000000000000000000000000081565b805160208183018101805160028252928201919093012091525481565b606060008373ffffffffffffffffffffffffffffffffffffffff16836040518082805190602001908083835b6020831061076457805182527fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe09092019160209182019101610727565b6001836020036101000a038019825116818451168082178552505050505050905001915050600060405180830381855af49150503d80600081146107c4576040519150601f19603f3d011682016040523d82523d6000602084013e6107c9565b606091505b50809350819250505061086c82826040516020018083805190602001908083835b6020831061082757805182527fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe090920191602091820191016107ea565b6001836020036101000a03801982511681845116808217855250505050505090500182151560f81b815260010192505050604051602081830303815290604052611bbd565b5092915050565b606060008260200267ffffffffffffffff8111801561089157600080fd5b506040519080825280601f01601f1916602001820160405280156108bc576020820181803683370190505b50905060005b838110156108df57848101546020808302840101526001016108c2565b5090505b92915050565b6002600154141561095b57604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601f60248201527f5265656e7472616e637947756172643a207265656e7472616e742063616c6c00604482015290519081900360640190fd5b60026001556040517f02cc250d00000000000000000000000000000000000000000000000000000000815273ffffffffffffffffffffffffffffffffffffffff7f000000000000000000000000000000000000000000000000000000000000000016906302cc250d906109d29033906004016136ee565b60206040518083038186803b1580156109ea57600080fd5b505afa1580156109fe573d6000803e3d6000fd5b505050506040513d601f19601f82011682018060405250810190610a229190613425565b610a58576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040161044d90613c78565b6000610a62611bc5565b8051909150610a7382868686611bf2565b60007ff3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee34677582610100015114610aa8576001610aab565b60005b9050610ab5612f90565b60408085015173ffffffffffffffffffffffffffffffffffffffff90811683526101408501517f4ac99ace14ee0a5ef932dc609df0943ab7ac16b7583634612f8dc35a4289a6ce9081146020850152606080880151909216928401929092526101608501519091149082015260008667ffffffffffffffff81118015610b3a57600080fd5b50604051908082528060200260200182016040528015610b64578160200160208202803683370190505b50610100850151909150610120870135907ff3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee3467751415610c30578460800151811015610bda576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040161044d90613c41565b610be78560600151611c90565b82886000013581518110610bf757fe5b602002602001018181525050610c0c81611c90565b60000382886020013581518110610c1f57fe5b602002602001018181525050610cc0565b8460600151811115610c6e576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040161044d90613b9c565b610c7781611c90565b82886000013581518110610c8757fe5b602002602001018181525050610ca08560800151611c90565b60000382886020013581518110610cb357fe5b6020026020010181815250505b610cc8612f90565b8660400151816000019073ffffffffffffffffffffffffffffffffffffffff16908173ffffffffffffffffffffffffffffffffffffffff16815250508560000151816020019073ffffffffffffffffffffffffffffffffffffffff16908173ffffffffffffffffffffffffffffffffffffffff16815250508560e0015181604001818152505085610140015181606001818152505060007f000000000000000000000000000000000000000000000000000000000000000073ffffffffffffffffffffffffffffffffffffffff16634817a286878f8f8f8f8b8b8f60a001518b6040518a63ffffffff1660e01b8152600401610dcc99989796959493929190613877565b600060405180830381600087803b158015610de657600080fd5b505af1158015610dfa573d6000803e3d6000fd5b505050506040513d6000823e601f3d9081017fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0168201604052610e4091908101906132ed565b90506000886020015190506000610e6d838c6000013581518110610e6057fe5b6020026020010151611d25565b90506000610e94848d6020013581518110610e8457fe5b6020026020010151600003611d25565b9050600283604051610ea691906136d2565b908152602001604051809103902054600014610eee576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040161044d90613bd3565b7ff3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee3467758a61010001511415610f825789606001518214610f58576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040161044d90613ac0565b8960600151600284604051610f6d91906136d2565b90815260405190819003602001902055610fe5565b89608001518114610fbf576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040161044d90613af7565b8960800151600284604051610fd491906136d2565b908152604051908190036020019020555b8a6040015173ffffffffffffffffffffffffffffffffffffffff167fa07a543ab8a018198e99ca0184c93fe9050a79400a0a723441f84de1d972cc178b600001518c6020015185858f60e001518960405161104596959493929190613820565b60405180910390a260405133907f40338ce1a7c49204f0099533b1e9a7ee0a3d261f84974ab7af36105b8c4e9db490600090a25050600180555050505050505050505050505050565b7f000000000000000000000000000000000000000000000000000000000000000081565b3033146110eb576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040161044d90613b65565b6110f760008383611d96565b5050565b805160208183018101805160008252928201919093012091525481565b60006111248484611b2f565b5091505073ffffffffffffffffffffffffffffffffffffffff811633146111ac57604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601a60248201527f475076323a2063616e6e6f74207072657369676e206f72646572000000000000604482015290519081900360640190fd5b8115611206577ff59c009283ff87aa78203fc4d9c2df025ee851130fb69cc3e068941f6b5e2d6f60001c60008585604051808383808284378083019250505092505050908152602001604051809103902081905550611232565b600080858560405180838380828437919091019485525050604051928390036020019092209290925550505b8073ffffffffffffffffffffffffffffffffffffffff167f01bf7c8b0ca55deecbea89d7e58295b7ffbf685fd0d96801034ba8c6ffe1c68d858585604051808060200183151581526020018281038252858582818152602001925080828437600083820152604051601f9091017fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe016909201829003965090945050505050a250505050565b303314611310576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040161044d90613b65565b6110f760028383611d96565b7f000000000000000000000000000000000000000000000000000000000000000081565b606060006343218e1960e01b8484604051602401808373ffffffffffffffffffffffffffffffffffffffff16815260200180602001828103825283818151815260200191508051906020019080838360005b838110156113aa578181015183820152602001611392565b50505050905090810190601f1680156113d75780820380516001836020036101000a031916815260200191505b50604080517fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe08184030181529181526020820180517bffffffffffffffffffffffffffffffffffffffffffffffffffffffff167fffffffff00000000000000000000000000000000000000000000000000000000909816979097178752518151919750309688965090945084935091508083835b602083106114a857805182527fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0909201916020918201910161146b565b6001836020036101000a0380198251168184511680821785525050505050509050019150506000604051808303816000865af19150503d806000811461150a576040519150601f19603f3d011682016040523d82523d6000602084013e61150f565b606091505b5090508092505060008260018451038151811061152857fe5b602001015160f81c60f81b7effffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff1916600160f81b14905061156b836001855103611e46565b80156115785750506108e3565b61158183611bbd565b505092915050565b7f000000000000000000000000000000000000000000000000000000000000000081565b60005b818110156116e557368383838181106115c557fe5b90506020028101906115d79190613dde565b905073ffffffffffffffffffffffffffffffffffffffff7f00000000000000000000000000000000000000000000000000000000000000001661161d6020830183613184565b73ffffffffffffffffffffffffffffffffffffffff16141561166b576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040161044d90613caf565b61167481611e4a565b6116816020820182613184565b73ffffffffffffffffffffffffffffffffffffffff167fed99827efb37016f2275f98c4bcf71c7551c75d59e9b450f79fa32e60be672c282602001356116c684611ea1565b6040516116d4929190613ce6565b60405180910390a2506001016115b0565b505050565b60608060006116f7611bc5565b90508367ffffffffffffffff8111801561171057600080fd5b5060405190808252806020026020018201604052801561174a57816020015b611737612f90565b81526020019060019003908161172f5790505b5092508367ffffffffffffffff8111801561176457600080fd5b5060405190808252806020026020018201604052801561179e57816020015b61178b612f90565b8152602001906001900390816117835790505b50915060005b8481101561184457368686838181106117b957fe5b90506020028101906117cb9190613e11565b90506117d9838c8c84611bf2565b61183b838a8a84358181106117ea57fe5b905060200201358b8b856020013581811061180157fe5b9050602002013584610120013589878151811061181a57fe5b602002602001015189888151811061182e57fe5b6020026020010151611ecb565b506001016117a4565b5050965096945050505050565b6000815167ffffffffffffffff8111801561186b57600080fd5b506040519080825280602002602001820160405280156118a557816020015b611892612fb7565b81526020019060019003908161188a5790505b5090506000805b8351811015611a935760008482815181106118c357fe5b6020026020010151905073eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee73ffffffffffffffffffffffffffffffffffffffff16816020015173ffffffffffffffffffffffffffffffffffffffff1614156119c7577f4ac99ace14ee0a5ef932dc609df0943ab7ac16b7583634612f8dc35a4289a6ce81606001511415611977576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040161044d90613b2e565b8051604080830151905173ffffffffffffffffffffffffffffffffffffffff9092169181156108fc0291906000818181858888f193505050501580156119c1573d6000803e3d6000fd5b50611a8a565b7f5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc981606001511415611a2657805160408201516020830151611a219273ffffffffffffffffffffffffffffffffffffffff90911691612216565b611a8a565b6000848480600101955081518110611a3a57fe5b602090810291909101810151600081528382015173ffffffffffffffffffffffffffffffffffffffff90811692820192909252604080850151908201523060608201528351909116608090910152505b506001016118ac565b508015611b2957611aa48282611e46565b6040517f0e8e3e8400000000000000000000000000000000000000000000000000000000815273ffffffffffffffffffffffffffffffffffffffff851690630e8e3e8490611af690859060040161375d565b600060405180830381600087803b158015611b1057600080fd5b505af1158015611b24573d6000803e3d6000fd5b505050505b50505050565b6000808060388414611ba257604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601160248201527f475076323a20696e76616c696420756964000000000000000000000000000000604482015290519081900360640190fd5b5050823593602084013560601c936034013560e01c92509050565b805160208201fd5b611bcd612fe7565b6040805160388082526060820190925290602082018180368337505050602082015290565b83516000611c02838686856122ee565b9050600080611c1f8484611c1a610140890189613d7b565b6123d6565b91509150611c4282828660a001518b60200151612485909392919063ffffffff16565b73ffffffffffffffffffffffffffffffffffffffff81166040890152611c688482612507565b73ffffffffffffffffffffffffffffffffffffffff1660609098019790975250505050505050565b60007f7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff821115611d2157604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601960248201527f53616665436173743a20696e74323536206f766572666c6f7700000000000000604482015290519081900360640190fd5b5090565b600080821215611d2157604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601660248201527f53616665436173743a206e6f7420706f73697469766500000000000000000000604482015290519081900360640190fd5b60005b81811015611b2957366000848484818110611db057fe5b9050602002810190611dc29190613d7b565b915091506000611dd28383611b2f565b92505050428163ffffffff1610611e15576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040161044d90613c0a565b6000878484604051611e289291906136c2565b90815260405190819003602001902055505060019091019050611d99565b9052565b73ffffffffffffffffffffffffffffffffffffffff8135166020820135366000611e776040860186613d7b565b9150915060405181838237600080838387895af1611e99573d6000803e3d6000fd5b505050505050565b60003681611eb26040850185613d7b565b909250905060048110611ec457813592505b5050919050565b8551602087015160a08201514263ffffffff9091161015611f18576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040161044d90613a52565b6080820151611f279087612539565b6060830151611f369089612539565b1015611f6e576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040161044d90613a89565b6000806000807ff3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee346775866101000151141561206f5785610120015115611fdb57889350611fd48660600151611fce868960e0015161253990919063ffffffff16565b906125c9565b9150611fea565b856060015193508560e0015191505b611ffe8a611ff8868e612539565b9061264a565b925061202a8460028760405161201491906136d2565b90815260405190819003602001902054906126e8565b9050856060015181111561206a576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040161044d90613bd3565b612116565b856101200151156120a35788925061209c8660800151611fce858960e0015161253990919063ffffffff16565b91506120b2565b856080015192508560e0015191505b6120c08b611fce858d612539565b93506120d68360028760405161201491906136d2565b90508560800151811115612116576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040161044d90613bd3565b61212084836126e8565b93508060028660405161213391906136d2565b9081526020016040518091039020819055508b6040015173ffffffffffffffffffffffffffffffffffffffff167fa07a543ab8a018198e99ca0184c93fe9050a79400a0a723441f84de1d972cc17876000015188602001518787878b6040516121a196959493929190613820565b60405180910390a250506040808b015173ffffffffffffffffffffffffffffffffffffffff9081168852855181166020808a0191909152888301949094526101408601516060988901529a8701518b16865282850151909a169185019190915297830197909752610160015191015250505050565b6040517fa9059cbb0000000000000000000000000000000000000000000000000000000080825273ffffffffffffffffffffffffffffffffffffffff84166004830152602482018390529060008060448382895af1612279573d6000803e3d6000fd5b506122838461275c565b611b2957604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601560248201527f475076323a206661696c6564207472616e736665720000000000000000000000604482015290519081900360640190fd5b6000838386358181106122fd57fe5b6020908102929092013573ffffffffffffffffffffffffffffffffffffffff168452508490849087013581811061233057fe5b73ffffffffffffffffffffffffffffffffffffffff602091820293909301358316908501525060408087013590911690830152606080860135908301526080808601359083015263ffffffff60a080870135919091169083015260c0808601359083015260e080860135908301526123ac610100860135612826565b61016087019190915261014086019190915290151561012085015261010090930152509392505050565b600080612403867f000000000000000000000000000000000000000000000000000000000000000061297b565b9150600085600381111561241357fe5b141561242b57612424828585612a05565b905061247c565b600185600381111561243957fe5b141561244a57612424828585612a1a565b600285600381111561245857fe5b141561246957612424828585612a82565b6124798285858960a00151612c20565b90505b94509492505050565b60388451146124f557604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601960248201527f475076323a2075696420627566666572206f766572666c6f7700000000000000604482015290519081900360640190fd5b60388401526034830152602090910152565b604082015160009073ffffffffffffffffffffffffffffffffffffffff166125305750806108e3565b50506040015190565b600082612548575060006108e3565b8282028284828161255557fe5b04146125c257604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601660248201527f536166654d6174683a206d756c206f766572666c6f7700000000000000000000604482015290519081900360640190fd5b9392505050565b600080821161263957604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601760248201527f536166654d6174683a206469766973696f6e2062792030000000000000000000604482015290519081900360640190fd5b81838161264257fe5b049392505050565b60008082116126ba57604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601f60248201527f536166654d6174683a206365696c696e67206469766973696f6e206279203000604482015290519081900360640190fd5b8183816126c357fe5b06156126d05760016126d3565b60005b60ff168284816126df57fe5b04019392505050565b6000828201838110156125c257604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601b60248201527f536166654d6174683a206164646974696f6e206f766572666c6f770000000000604482015290519081900360640190fd5b600061279a565b7f08c379a0000000000000000000000000000000000000000000000000000000006000526020600452806024528160445260646000fd5b3d80156127d95760208114612813576127d47f475076323a206d616c666f726d6564207472616e7366657220726573756c7400601f612763565b612820565b823b61280a5761280a7f475076323a206e6f74206120636f6e74726163740000000000000000000000006014612763565b60019150612820565b3d6000803e600051151591505b50919050565b6000808080806001861661285c577ff3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee3467759450612880565b7f6ed88e868af0a1983e3886d5f3e95a2fafbd6c3450bc229e27342283dc429ccc94505b6002861615159350600886166128b8577f5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc9925061290c565b600486166128e8577fabee3b73373acd583a130924aad6dc38cfdc44ba0555ba94ce2ff63980ea0632925061290c565b7f4ac99ace14ee0a5ef932dc609df0943ab7ac16b7583634612f8dc35a4289a6ce92505b6010861661293c577f5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc99150612960565b7f4ac99ace14ee0a5ef932dc609df0943ab7ac16b7583634612f8dc35a4289a6ce91505b600586901c600381111561297057fe5b905091939590929450565b7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe090910180517fd5a25ba2e97094ad7d83dc28a6572da797d6b3e7fc6663bd93efb789fc17e48982526101a0822091526040517f19010000000000000000000000000000000000000000000000000000000000008152600281019290925260228201526042902090565b6000612a12848484612de5565b949350505050565b6000808460405160200180807f19457468657265756d205369676e6564204d6573736167653a0a333200000000815250601c01828152602001915050604051602081830303815290604052805190602001209050612a79818585612de5565b95945050505050565b813560601c366000612a978460148188613e68565b604080517f1626ba7e00000000000000000000000000000000000000000000000000000000808252600482018b81526024830193845260448301859052949650929450919273ffffffffffffffffffffffffffffffffffffffff871692631626ba7e928b928892889290606401848480828437600083820152604051601f9091017fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe016909201965060209550909350505081840390508186803b158015612b5d57600080fd5b505afa158015612b71573d6000803e3d6000fd5b505050506040513d6020811015612b8757600080fd5b50517fffffffff000000000000000000000000000000000000000000000000000000001614612c1757604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601f60248201527f475076323a20696e76616c69642065697031323731207369676e617475726500604482015290519081900360640190fd5b50509392505050565b600060148314612c9157604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601c60248201527f475076323a206d616c666f726d6564207072657369676e617475726500000000604482015290519081900360640190fd5b506040805160388082526060828101909352853590921c9160009190602082018180368337019050509050612cc881878486612485565b7ff59c009283ff87aa78203fc4d9c2df025ee851130fb69cc3e068941f6b5e2d6f60001c6000826040518082805190602001908083835b60208310612d3c57805182527fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe09092019160209182019101612cff565b6001836020036101000a03801982511681845116808217855250505050505090500191505090815260200160405180910390205414612ddc57604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601960248201527f475076323a206f72646572206e6f74207072657369676e656400000000000000604482015290519081900360640190fd5b50949350505050565b600060418214612e5657604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601f60248201527f475076323a206d616c666f726d6564206563647361207369676e617475726500604482015290519081900360640190fd5b604080516000815260208181018084528790528286013560f81c82840181905286356060840181905282880135608085018190529451909493919260019260a0808201937fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe081019281900390910190855afa158015612ed9573d6000803e3d6000fd5b50506040517fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0015194505073ffffffffffffffffffffffffffffffffffffffff8416612f8657604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601d60248201527f475076323a20696e76616c6964206563647361207369676e6174757265000000604482015290519081900360640190fd5b5050509392505050565b60408051608081018252600080825260208201819052918101829052606081019190915290565b6040805160a081019091528060008152600060208201819052604082018190526060820181905260809091015290565b6040518060800160405280612ffa613014565b815260606020820181905260006040830181905291015290565b6040805161018081018252600080825260208201819052918101829052606081018290526080810182905260a0810182905260c0810182905260e0810182905261010081018290526101208101829052610140810182905261016081019190915290565b60008083601f840112613089578182fd5b50813567ffffffffffffffff8111156130a0578182fd5b60208301915083602080830285010111156130ba57600080fd5b9250929050565b60008083601f8401126130d2578182fd5b50813567ffffffffffffffff8111156130e9578182fd5b6020830191508360208285010111156130ba57600080fd5b600082601f830112613111578081fd5b813567ffffffffffffffff81111561312557fe5b61315660207fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f84011601613e44565b81815284602083860101111561316a578283fd5b816020850160208301379081016020019190915292915050565b600060208284031215613195578081fd5b81356125c281613ebc565b600080604083850312156131b2578081fd5b82356131bd81613ebc565b9150602083013567ffffffffffffffff8111156131d8578182fd5b6131e485828601613101565b9150509250929050565b60008060208385031215613200578182fd5b823567ffffffffffffffff811115613216578283fd5b61322285828601613078565b90969095509350505050565b60008060008060008060006080888a031215613248578283fd5b873567ffffffffffffffff8082111561325f578485fd5b61326b8b838c01613078565b909950975060208a0135915080821115613283578485fd5b61328f8b838c01613078565b909750955060408a01359150808211156132a7578485fd5b6132b38b838c01613078565b909550935060608a01359150808211156132cb578283fd5b508801606081018a10156132dd578182fd5b8091505092959891949750929550565b600060208083850312156132ff578182fd5b825167ffffffffffffffff80821115613316578384fd5b818501915085601f830112613329578384fd5b81518181111561333557fe5b8381029150613345848301613e44565b8181528481019084860184860187018a101561335f578788fd5b8795505b83861015613381578051835260019590950194918601918601613363565b5098975050505050505050565b6000806000806000606086880312156133a5578081fd5b853567ffffffffffffffff808211156133bc578283fd5b6133c889838a01613078565b909750955060208801359150808211156133e0578283fd5b6133ec89838a01613078565b90955093506040880135915080821115613404578283fd5b5086016101608189031215613417578182fd5b809150509295509295909350565b600060208284031215613436578081fd5b81516125c281613ee1565b60008060208385031215613453578182fd5b823567ffffffffffffffff811115613469578283fd5b613222858286016130c1565b600080600060408486031215613489578081fd5b833567ffffffffffffffff81111561349f578182fd5b6134ab868287016130c1565b90945092505060208401356134bf81613ee1565b809150509250925092565b6000602082840312156134db578081fd5b813567ffffffffffffffff8111156134f1578182fd5b612a1284828501613101565b6000806040838503121561350f578182fd5b50508035926020909101359150565b60008284526020808501945082825b8581101561356857813561354081613ebc565b73ffffffffffffffffffffffffffffffffffffffff168752958201959082019060010161352d565b509495945050505050565b6000815180845260208085019450808401835b8381101561356857815187529582019590820190600101613586565b600082845282826020860137806020848601015260207fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f85011685010190509392505050565b60008151808452613602816020860160208601613e90565b601f017fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0169290920160200192915050565b73ffffffffffffffffffffffffffffffffffffffff8082511683528060208301511660208401525060408101516040830152606081015160608301525050565b73ffffffffffffffffffffffffffffffffffffffff808251168352602082015115156020840152806040830151166040840152506060810151151560608301525050565b63ffffffff169052565b6000828483379101908152919050565b600082516136e4818460208701613e90565b9190910192915050565b73ffffffffffffffffffffffffffffffffffffffff91909116815260200190565b6020808252825182820181905260009190848201906040850190845b818110156137515761373e838551613634565b928401926080929092019160010161372b565b50909695505050505050565b602080825282518282018190526000919060409081850190868401855b828110156137e357815180516004811061379057fe5b85528087015173ffffffffffffffffffffffffffffffffffffffff908116888701528682015187870152606080830151821690870152608091820151169085015260a0909301929085019060010161377a565b5091979650505050505050565b90815260200190565b600060208252612a126020830184866135a2565b6000602082526125c260208301846135ea565b600073ffffffffffffffffffffffffffffffffffffffff808916835280881660208401525085604083015284606083015283608083015260c060a083015261386b60c08301846135ea565b98975050505050505050565b60006101a0820160028c1061388857fe5b8b835260206101a081850152818b83526101c0850190506101c0828d0286010192508c845b8d8110156139b6577ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe408786030183527fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff618f36030182351261390c578586fd5b8e823501803586528481013585870152604081013560408701526060810135606087015260808101357fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe1823603018112613964578788fd5b8101803567ffffffffffffffff81111561397c578889fd5b80360383131561398a578889fd5b60a060808901526139a160a08901828985016135a2565b975050509284019250908301906001016138ad565b5050505082810360408401526139cd81898b61351e565b90506139dc6060840188613674565b82810360e08401526139ee8187613573565b9150506139ff6101008301856136b8565b613a0d610120830184613634565b9a9950505050505050505050565b6020808252601f908201527f475076323a2063616c6c657220646f6573206e6f74206f776e206f7264657200604082015260600190565b60208082526013908201527f475076323a206f72646572206578706972656400000000000000000000000000604082015260600190565b6020808252601f908201527f475076323a206c696d6974207072696365206e6f742072657370656374656400604082015260600190565b6020808252601f908201527f475076323a2073656c6c20616d6f756e74206e6f742072657370656374656400604082015260600190565b6020808252601e908201527f475076323a2062757920616d6f756e74206e6f74207265737065637465640000604082015260600190565b6020808252601e908201527f475076323a20756e737570706f7274656420696e7465726e616c204554480000604082015260600190565b60208082526018908201527f475076323a206e6f7420616e20696e746572616374696f6e0000000000000000604082015260600190565b60208082526014908201527f475076323a206c696d697420746f6f2068696768000000000000000000000000604082015260600190565b60208082526012908201527f475076323a206f726465722066696c6c65640000000000000000000000000000604082015260600190565b60208082526017908201527f475076323a206f72646572207374696c6c2076616c6964000000000000000000604082015260600190565b60208082526013908201527f475076323a206c696d697420746f6f206c6f7700000000000000000000000000604082015260600190565b60208082526012908201527f475076323a206e6f74206120736f6c7665720000000000000000000000000000604082015260600190565b6020808252601b908201527f475076323a20666f7262696464656e20696e746572616374696f6e0000000000604082015260600190565b9182527fffffffff0000000000000000000000000000000000000000000000000000000016602082015260400190565b60008083357fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe1843603018112613d4a578283fd5b83018035915067ffffffffffffffff821115613d64578283fd5b60209081019250810236038213156130ba57600080fd5b60008083357fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe1843603018112613daf578283fd5b83018035915067ffffffffffffffff821115613dc9578283fd5b6020019150368190038213156130ba57600080fd5b600082357fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffa18336030181126136e4578182fd5b600082357ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffea18336030181126136e4578182fd5b60405181810167ffffffffffffffff81118282101715613e6057fe5b604052919050565b60008085851115613e77578182fd5b83861115613e83578182fd5b5050820193919092039150565b60005b83811015613eab578181015183820152602001613e93565b83811115611b295750506000910152565b73ffffffffffffffffffffffffffffffffffffffff81168114613ede57600080fd5b50565b8015158114613ede57600080fdfea2646970667358221220de5e493c48a3b42da03a5db89085177b8d8ccec6e9bf6e8e48b3809343624c8f64736f6c6343000706003360c060405234801561001057600080fd5b5060405161129e38038061129e83398101604081905261002f9161004b565b33606090811b6080521b6001600160601b03191660a052610079565b60006020828403121561005c578081fd5b81516001600160a01b0381168114610072578182fd5b9392505050565b60805160601c60a05160601c6111ee6100b060003980610130528061020152806102bd5250806093528061024c52506111ee6000f3fe608060405234801561001057600080fd5b50600436106100365760003560e01c80634817a2861461003b5780637d10d11f14610064575b600080fd5b61004e610049366004610cd9565b610079565b60405161005b9190610eb3565b60405180910390f35b610077610072366004610c69565b610234565b005b60603373ffffffffffffffffffffffffffffffffffffffff7f000000000000000000000000000000000000000000000000000000000000000016146100f3576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004016100ea906110e5565b60405180910390fd5b6040517f945bcec900000000000000000000000000000000000000000000000000000000815273ffffffffffffffffffffffffffffffffffffffff7f0000000000000000000000000000000000000000000000000000000000000000169063945bcec990610171908c908c908c908c908c908c908c90600401610f59565b600060405180830381600087803b15801561018b57600080fd5b505af115801561019f573d6000803e3d6000fd5b505050506040513d6000823e601f3d9081017fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe01682016040526101e59190810190610bd9565b905061022873ffffffffffffffffffffffffffffffffffffffff7f00000000000000000000000000000000000000000000000000000000000000001683336102e9565b98975050505050505050565b3373ffffffffffffffffffffffffffffffffffffffff7f000000000000000000000000000000000000000000000000000000000000000016146102a3576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004016100ea906110e5565b6102e573ffffffffffffffffffffffffffffffffffffffff7f000000000000000000000000000000000000000000000000000000000000000016838333610551565b5050565b73eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee61030e6040840160208501610bb6565b73ffffffffffffffffffffffffffffffffffffffff16141561035c576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004016100ea9061111c565b7f5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc9826060013514156103d0576103cb6103986020840184610bb6565b82604085018035906103ad9060208801610bb6565b73ffffffffffffffffffffffffffffffffffffffff16929190610816565b61054c565b604080516001808252818301909252600091816020015b6103ef6109cb565b8152602001906001900390816103e757905050905060008160008151811061041357fe5b602002602001015190507fabee3b73373acd583a130924aad6dc38cfdc44ba0555ba94ce2ff63980ea063284606001351461044f576002610452565b60035b8190600381111561045f57fe5b9081600381111561046c57fe5b90525061047f6040850160208601610bb6565b73ffffffffffffffffffffffffffffffffffffffff16602080830191909152604080860135908301526104b490850185610bb6565b73ffffffffffffffffffffffffffffffffffffffff908116606083015283811660808301526040517f0e8e3e8400000000000000000000000000000000000000000000000000000000815290861690630e8e3e8490610517908590600401610ec6565b600060405180830381600087803b15801561053157600080fd5b505af1158015610545573d6000803e3d6000fd5b5050505050505b505050565b60008267ffffffffffffffff8111801561056a57600080fd5b506040519080825280602002602001820160405280156105a457816020015b6105916109cb565b8152602001906001900390816105895790505b5090506000805b8481101561077857368686838181106105c057fe5b60800291909101915073eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee90506105f06040830160208401610bb6565b73ffffffffffffffffffffffffffffffffffffffff16141561063e576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004016100ea9061111c565b7f5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc9816060013514156106945761068f61067a6020830183610bb6565b86604084018035906103ad9060208701610bb6565b61076f565b60008484806001019550815181106106a857fe5b602002602001015190507fabee3b73373acd583a130924aad6dc38cfdc44ba0555ba94ce2ff63980ea06328260600135146106e45760016106e7565b60035b819060038111156106f457fe5b9081600381111561070157fe5b9052506107146040830160208401610bb6565b73ffffffffffffffffffffffffffffffffffffffff166020808301919091526040808401359083015261074990830183610bb6565b73ffffffffffffffffffffffffffffffffffffffff908116606083015286166080909101525b506001016105ab565b50801561080e5761078982826108fd565b6040517f0e8e3e8400000000000000000000000000000000000000000000000000000000815273ffffffffffffffffffffffffffffffffffffffff871690630e8e3e84906107db908590600401610ec6565b600060405180830381600087803b1580156107f557600080fd5b505af1158015610809573d6000803e3d6000fd5b505050505b505050505050565b6040517f23b872dd0000000000000000000000000000000000000000000000000000000080825273ffffffffffffffffffffffffffffffffffffffff8581166004840152841660248301526044820183905290600080606483828a5af1610881573d6000803e3d6000fd5b5061088b85610901565b6108f657604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601960248201527f475076323a206661696c6564207472616e7366657246726f6d00000000000000604482015290519081900360640190fd5b5050505050565b9052565b600061093f565b7f08c379a0000000000000000000000000000000000000000000000000000000006000526020600452806024528160445260646000fd5b3d801561097e57602081146109b8576109797f475076323a206d616c666f726d6564207472616e7366657220726573756c7400601f610908565b6109c5565b823b6109af576109af7f475076323a206e6f74206120636f6e74726163740000000000000000000000006014610908565b600191506109c5565b3d6000803e600051151591505b50919050565b6040805160a081019091528060008152600060208201819052604082018190526060820181905260809091015290565b600082601f830112610a0b578081fd5b81356020610a20610a1b83611175565b611151565b8281528181019085830183850287018401881015610a3c578586fd5b855b85811015610a63578135610a5181611193565b84529284019290840190600101610a3e565b5090979650505050505050565b600082601f830112610a80578081fd5b81356020610a90610a1b83611175565b8281528181019085830183850287018401881015610aac578586fd5b855b85811015610a6357813584529284019290840190600101610aae565b60008083601f840112610adb578182fd5b50813567ffffffffffffffff811115610af2578182fd5b6020830191508360208083028501011115610b0c57600080fd5b9250929050565b80358015158114610b2357600080fd5b919050565b6000608082840312156109c5578081fd5b600060808284031215610b4a578081fd5b6040516080810181811067ffffffffffffffff82111715610b6757fe5b6040529050808235610b7881611193565b8152610b8660208401610b13565b60208201526040830135610b9981611193565b6040820152610baa60608401610b13565b60608201525092915050565b600060208284031215610bc7578081fd5b8135610bd281611193565b9392505050565b60006020808385031215610beb578182fd5b825167ffffffffffffffff811115610c01578283fd5b8301601f81018513610c11578283fd5b8051610c1f610a1b82611175565b8181528381019083850185840285018601891015610c3b578687fd5b8694505b83851015610c5d578051835260019490940193918501918501610c3f565b50979650505050505050565b60008060208385031215610c7b578081fd5b823567ffffffffffffffff80821115610c92578283fd5b818501915085601f830112610ca5578283fd5b813581811115610cb3578384fd5b866020608083028501011115610cc7578384fd5b60209290920196919550909350505050565b6000806000806000806000806101a0898b031215610cf5578384fd5b883560028110610d03578485fd5b9750602089013567ffffffffffffffff80821115610d1f578586fd5b610d2b8c838d01610aca565b909950975060408b0135915080821115610d43578586fd5b610d4f8c838d016109fb565b9650610d5e8c60608d01610b39565b955060e08b0135915080821115610d73578485fd5b50610d808b828c01610a70565b9350506101008901359150610d998a6101208b01610b28565b90509295985092959890939650565b6000815180845260208085019450808401835b83811015610ded57815173ffffffffffffffffffffffffffffffffffffffff1687529582019590820190600101610dbb565b509495945050505050565b6000815180845260208085019450808401835b83811015610ded57815187529582019590820190600101610e0b565b600082845282826020860137806020848601015260207fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f85011685010190509392505050565b73ffffffffffffffffffffffffffffffffffffffff808251168352602082015115156020840152806040830151166040840152506060810151151560608301525050565b600060208252610bd26020830184610df8565b602080825282518282018190526000919060409081850190868401855b82811015610f4c578151805160048110610ef957fe5b85528087015173ffffffffffffffffffffffffffffffffffffffff908116888701528682015187870152606080830151821690870152608091820151169085015260a09093019290850190600101610ee3565b5091979650505050505050565b600061012080830160028b10610f6b57fe5b8a8452602080850192909252889052610140808401918981028501909101908a845b8b811015611098577ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffec087850301855281357fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff618e3603018112610fed578687fd5b8d01803585528381013584860152604080820135908601526060808201359086015260a0608080830135368490037fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe101811261104757898afd5b8301803567ffffffffffffffff81111561105f578a8bfd5b80360385131561106d578a8bfd5b83838a0152611081848a01828a8501610e27565b998801999850505093850193505050600101610f8d565b50505083810360408501526110ad8189610da8565b9150506110bd6060840187610e6f565b82810360e08401526110cf8186610df8565b9150508261010083015298975050505050505050565b60208082526011908201527f475076323a206e6f742063726561746f72000000000000000000000000000000604082015260600190565b6020808252818101527f475076323a2063616e6e6f74207472616e73666572206e617469766520455448604082015260600190565b60405181810167ffffffffffffffff8111828210171561116d57fe5b604052919050565b600067ffffffffffffffff82111561118957fe5b5060209081020190565b73ffffffffffffffffffffffffffffffffffffffff811681146111b557600080fd5b5056fea2646970667358221220364a6941bea69620b7dc3a957d0ab4cbf3bfc459c7ad3924d220620aca9202fc64736f6c63430007060033
    /// ```
    #[rustfmt::skip]
    #[allow(clippy::all)]
    pub static BYTECODE: alloy_sol_types::private::Bytes = alloy_sol_types::private::Bytes::from_static(
        b"a\x01\0`@R4\x80\x15b\0\0\x12W`\0\x80\xFD[P`@Qb\0S\xEB8\x03\x80b\0S\xEB\x839\x81\x01`@\x81\x90Rb\0\x005\x91b\0\x01[V[`@\x80Q\x7F\x8Bs\xC3\xC6\x9B\xB8\xFE=Q.\xCCL\xF7Y\xCCy#\x9F{\x17\x9B\x0F\xFA\xCA\xA9\xA7]R+9@\x0F` \x80\x83\x01\x91\x90\x91R\x7Fl\x85\xC03~\xBA\x16a2\x7F\x94\xF3\xBFF\xC8\xA7\xF91\x1AV?M\\\x94\x83bV\x7F]\x8E\xD6\x0C\x82\x84\x01R\x7F\xF9Dk\x8E\x93}\x86\xF0\xBC\x87\xCA\xC79#I\x16\x92\xB1#\xCA_\x87a\x90\x84\x94p7X j\xDF``\x80\x84\x01\x91\x90\x91RF`\x80\x80\x85\x01\x91\x90\x91R0`\xA0\x80\x86\x01\x91\x90\x91R\x85Q\x80\x86\x03\x82\x01\x81R`\xC0\x95\x86\x01\x96\x87\x90R\x80Q\x94\x01\x93\x90\x93 \x90R`\x01\x80U`\x01`\x01``\x1B\x03\x19\x86\x82\x1B\x81\x16\x90\x92R\x84\x90\x1B\x16\x90R\x81\x90b\0\x01\n\x90b\0\x01MV[b\0\x01\x16\x91\x90b\0\x01\x99V[`@Q\x80\x91\x03\x90`\0\xF0\x80\x15\x80\x15b\0\x013W=`\0\x80>=`\0\xFD[P``\x1B`\x01`\x01``\x1B\x03\x19\x16`\xE0RPb\0\x01\xC6\x90PV[a\x12\x9E\x80b\0AM\x839\x01\x90V[`\0\x80`@\x83\x85\x03\x12\x15b\0\x01nW\x81\x82\xFD[\x82Qb\0\x01{\x81b\0\x01\xADV[` \x84\x01Q\x90\x92Pb\0\x01\x8E\x81b\0\x01\xADV[\x80\x91PP\x92P\x92\x90PV[`\x01`\x01`\xA0\x1B\x03\x91\x90\x91\x16\x81R` \x01\x90V[`\x01`\x01`\xA0\x1B\x03\x81\x16\x81\x14b\0\x01\xC3W`\0\x80\xFD[PV[`\x80Q`\xA0Q``\x1C`\xC0Q``\x1C`\xE0Q``\x1Ca?%b\0\x02(`\09\x80a\x04\xC5R\x80a\raR\x80a\x10\x90R\x80a\x15\xF0RP\x80a\x05VR\x80a\x15\x8BRP\x80a\x03\x92R\x80a\x06\xBCR\x80a\t\x9DRP\x80a\x13\x1ER\x80a#\xDFRPa?%`\0\xF3\xFE`\x80`@R`\x046\x10a\0\xECW`\x005`\xE0\x1C\x80c\x9BU,\xC2\x11a\0\x8AW\x80c\xED\x9F5\xCE\x11a\0YW\x80c\xED\x9F5\xCE\x14a\x02tW\x80c\xF6\x98\xDA%\x14a\x02\x94W\x80c\xF8D6\xBD\x14a\x02\xA9W\x80c\xFB\xFAw\xCF\x14a\x02\xC9Wa\0\xF3V[\x80c\x9BU,\xC2\x14a\x01\xFFW\x80c\xA2\xA7\xD5\x1B\x14a\x02\x14W\x80c\xD0\x8D3\xD1\x14a\x024W\x80c\xECl\xB1?\x14a\x02TWa\0\xF3V[\x80c$y\xFBn\x11a\0\xC6W\x80c$y\xFBn\x14a\x01eW\x80cC!\x8E\x19\x14a\x01\x92W\x80cV$\xB2[\x14a\x01\xBFW\x80c\x84Z\x10\x1F\x14a\x01\xDFWa\0\xF3V[\x80c\x13\xD7\x9A\x0B\x14a\0\xF8W\x80c\x153{\xC0\x14a\x01\x1AW\x80c#5\xC7k\x14a\x01:Wa\0\xF3V[6a\0\xF3W\0[`\0\x80\xFD[4\x80\x15a\x01\x04W`\0\x80\xFD[Pa\x01\x18a\x01\x136`\x04a2.V[a\x02\xDEV[\0[4\x80\x15a\x01&W`\0\x80\xFD[Pa\x01\x18a\x0156`\x04a4AV[a\x05\xC1V[4\x80\x15a\x01FW`\0\x80\xFD[Pa\x01Oa\x06\xBAV[`@Qa\x01\\\x91\x90a6\xEEV[`@Q\x80\x91\x03\x90\xF3[4\x80\x15a\x01qW`\0\x80\xFD[Pa\x01\x85a\x01\x806`\x04a4\xCAV[a\x06\xDEV[`@Qa\x01\\\x91\x90a7\xF0V[4\x80\x15a\x01\x9EW`\0\x80\xFD[Pa\x01\xB2a\x01\xAD6`\x04a1\xA0V[a\x06\xFBV[`@Qa\x01\\\x91\x90a8\rV[4\x80\x15a\x01\xCBW`\0\x80\xFD[Pa\x01\xB2a\x01\xDA6`\x04a4\xFDV[a\x08sV[4\x80\x15a\x01\xEBW`\0\x80\xFD[Pa\x01\x18a\x01\xFA6`\x04a3\x8EV[a\x08\xE9V[4\x80\x15a\x02\x0BW`\0\x80\xFD[Pa\x01Oa\x10\x8EV[4\x80\x15a\x02 W`\0\x80\xFD[Pa\x01\x18a\x02/6`\x04a1\xEEV[a\x10\xB2V[4\x80\x15a\x02@W`\0\x80\xFD[Pa\x01\x85a\x02O6`\x04a4\xCAV[a\x10\xFBV[4\x80\x15a\x02`W`\0\x80\xFD[Pa\x01\x18a\x02o6`\x04a4uV[a\x11\x18V[4\x80\x15a\x02\x80W`\0\x80\xFD[Pa\x01\x18a\x02\x8F6`\x04a1\xEEV[a\x12\xD7V[4\x80\x15a\x02\xA0W`\0\x80\xFD[Pa\x01\x85a\x13\x1CV[4\x80\x15a\x02\xB5W`\0\x80\xFD[Pa\x01\xB2a\x02\xC46`\x04a1\xA0V[a\x13@V[4\x80\x15a\x02\xD5W`\0\x80\xFD[Pa\x01Oa\x15\x89V[`\x02`\x01T\x14\x15a\x03PW`@\x80Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1F`$\x82\x01R\x7FReentrancyGuard: reentrant call\0`D\x82\x01R\x90Q\x90\x81\x90\x03`d\x01\x90\xFD[`\x02`\x01U`@Q\x7F\x02\xCC%\r\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x90c\x02\xCC%\r\x90a\x03\xC7\x903\x90`\x04\x01a6\xEEV[` `@Q\x80\x83\x03\x81\x86\x80;\x15\x80\x15a\x03\xDFW`\0\x80\xFD[PZ\xFA\x15\x80\x15a\x03\xF3W=`\0\x80>=`\0\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x04\x17\x91\x90a4%V[a\x04VW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01a\x04M\x90a<xV[`@Q\x80\x91\x03\x90\xFD[a\x04r\x81`\0[` \x02\x81\x01\x90a\x04m\x91\x90a=\x16V[a\x15\xADV[`\0\x80a\x04\x83\x89\x89\x89\x89\x89\x89a\x16\xEAV[`@Q\x7F}\x10\xD1\x1F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R\x91\x93P\x91Ps\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x90c}\x10\xD1\x1F\x90a\x04\xFA\x90\x85\x90`\x04\x01a7\x0FV[`\0`@Q\x80\x83\x03\x81`\0\x87\x80;\x15\x80\x15a\x05\x14W`\0\x80\xFD[PZ\xF1\x15\x80\x15a\x05(W=`\0\x80>=`\0\xFD[PPPPa\x05<\x83`\x01`\x03\x81\x10a\x04]W\xFE[a\x05|s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x82a\x18QV[a\x05\x87\x83`\x02a\x04]V[`@Q3\x90\x7F@3\x8C\xE1\xA7\xC4\x92\x04\xF0\t\x953\xB1\xE9\xA7\xEE\n=&\x1F\x84\x97J\xB7\xAF6\x10[\x8CN\x9D\xB4\x90`\0\x90\xA2PP`\x01\x80UPPPPPPPV[`\0a\x05\xCD\x83\x83a\x1B/V[P\x91PPs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x163\x14a\x06 W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01a\x04M\x90a:\x1BV[\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF`\x02\x84\x84`@Qa\x06S\x92\x91\x90a6\xC2V[\x90\x81R` \x01`@Q\x80\x91\x03\x90 \x81\x90UP\x80s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x7F\x87[l\xB05\xBB\xD4\xACe\0\xFA\xBCm\x1EL\xA5\xBD\xC5\x8A>+BL\xCB\\$\xCD\xBE\xBE\xB0\t\xA9\x84\x84`@Qa\x06\xAD\x92\x91\x90a7\xF9V[`@Q\x80\x91\x03\x90\xA2PPPV[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[\x80Q` \x81\x83\x01\x81\x01\x80Q`\x02\x82R\x92\x82\x01\x91\x90\x93\x01 \x91RT\x81V[```\0\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x83`@Q\x80\x82\x80Q\x90` \x01\x90\x80\x83\x83[` \x83\x10a\x07dW\x80Q\x82R\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x90\x92\x01\x91` \x91\x82\x01\x91\x01a\x07'V[`\x01\x83` \x03a\x01\0\n\x03\x80\x19\x82Q\x16\x81\x84Q\x16\x80\x82\x17\x85RPPPPPP\x90P\x01\x91PP`\0`@Q\x80\x83\x03\x81\x85Z\xF4\x91PP=\x80`\0\x81\x14a\x07\xC4W`@Q\x91P`\x1F\x19`?=\x01\x16\x82\x01`@R=\x82R=`\0` \x84\x01>a\x07\xC9V[``\x91P[P\x80\x93P\x81\x92PPPa\x08l\x82\x82`@Q` \x01\x80\x83\x80Q\x90` \x01\x90\x80\x83\x83[` \x83\x10a\x08'W\x80Q\x82R\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x90\x92\x01\x91` \x91\x82\x01\x91\x01a\x07\xEAV[`\x01\x83` \x03a\x01\0\n\x03\x80\x19\x82Q\x16\x81\x84Q\x16\x80\x82\x17\x85RPPPPPP\x90P\x01\x82\x15\x15`\xF8\x1B\x81R`\x01\x01\x92PPP`@Q` \x81\x83\x03\x03\x81R\x90`@Ra\x1B\xBDV[P\x92\x91PPV[```\0\x82` \x02g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x80\x15a\x08\x91W`\0\x80\xFD[P`@Q\x90\x80\x82R\x80`\x1F\x01`\x1F\x19\x16` \x01\x82\x01`@R\x80\x15a\x08\xBCW` \x82\x01\x81\x806\x837\x01\x90P[P\x90P`\0[\x83\x81\x10\x15a\x08\xDFW\x84\x81\x01T` \x80\x83\x02\x84\x01\x01R`\x01\x01a\x08\xC2V[P\x90P[\x92\x91PPV[`\x02`\x01T\x14\x15a\t[W`@\x80Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1F`$\x82\x01R\x7FReentrancyGuard: reentrant call\0`D\x82\x01R\x90Q\x90\x81\x90\x03`d\x01\x90\xFD[`\x02`\x01U`@Q\x7F\x02\xCC%\r\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x90c\x02\xCC%\r\x90a\t\xD2\x903\x90`\x04\x01a6\xEEV[` `@Q\x80\x83\x03\x81\x86\x80;\x15\x80\x15a\t\xEAW`\0\x80\xFD[PZ\xFA\x15\x80\x15a\t\xFEW=`\0\x80>=`\0\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\n\"\x91\x90a4%V[a\nXW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01a\x04M\x90a<xV[`\0a\nba\x1B\xC5V[\x80Q\x90\x91Pa\ns\x82\x86\x86\x86a\x1B\xF2V[`\0\x7F\xF3\xB2wr\x8B?\xEEt\x94\x81\xEB>\x0B;H\x98\r\xBB\xABxe\x8F\xC4\x19\x02\\\xB1n\xEE4gu\x82a\x01\0\x01Q\x14a\n\xA8W`\x01a\n\xABV[`\0[\x90Pa\n\xB5a/\x90V[`@\x80\x85\x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x90\x81\x16\x83Ra\x01@\x85\x01Q\x7FJ\xC9\x9A\xCE\x14\xEE\n^\xF92\xDC`\x9D\xF0\x94:\xB7\xAC\x16\xB7X64a/\x8D\xC3ZB\x89\xA6\xCE\x90\x81\x14` \x85\x01R``\x80\x88\x01Q\x90\x92\x16\x92\x84\x01\x92\x90\x92Ra\x01`\x85\x01Q\x90\x91\x14\x90\x82\x01R`\0\x86g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x80\x15a\x0B:W`\0\x80\xFD[P`@Q\x90\x80\x82R\x80` \x02` \x01\x82\x01`@R\x80\x15a\x0BdW\x81` \x01` \x82\x02\x806\x837\x01\x90P[Pa\x01\0\x85\x01Q\x90\x91Pa\x01 \x87\x015\x90\x7F\xF3\xB2wr\x8B?\xEEt\x94\x81\xEB>\x0B;H\x98\r\xBB\xABxe\x8F\xC4\x19\x02\\\xB1n\xEE4gu\x14\x15a\x0C0W\x84`\x80\x01Q\x81\x10\x15a\x0B\xDAW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01a\x04M\x90a<AV[a\x0B\xE7\x85``\x01Qa\x1C\x90V[\x82\x88`\0\x015\x81Q\x81\x10a\x0B\xF7W\xFE[` \x02` \x01\x01\x81\x81RPPa\x0C\x0C\x81a\x1C\x90V[`\0\x03\x82\x88` \x015\x81Q\x81\x10a\x0C\x1FW\xFE[` \x02` \x01\x01\x81\x81RPPa\x0C\xC0V[\x84``\x01Q\x81\x11\x15a\x0CnW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01a\x04M\x90a;\x9CV[a\x0Cw\x81a\x1C\x90V[\x82\x88`\0\x015\x81Q\x81\x10a\x0C\x87W\xFE[` \x02` \x01\x01\x81\x81RPPa\x0C\xA0\x85`\x80\x01Qa\x1C\x90V[`\0\x03\x82\x88` \x015\x81Q\x81\x10a\x0C\xB3W\xFE[` \x02` \x01\x01\x81\x81RPP[a\x0C\xC8a/\x90V[\x86`@\x01Q\x81`\0\x01\x90s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81RPP\x85`\0\x01Q\x81` \x01\x90s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81RPP\x85`\xE0\x01Q\x81`@\x01\x81\x81RPP\x85a\x01@\x01Q\x81``\x01\x81\x81RPP`\0\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16cH\x17\xA2\x86\x87\x8F\x8F\x8F\x8F\x8B\x8B\x8F`\xA0\x01Q\x8B`@Q\x8Ac\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01a\r\xCC\x99\x98\x97\x96\x95\x94\x93\x92\x91\x90a8wV[`\0`@Q\x80\x83\x03\x81`\0\x87\x80;\x15\x80\x15a\r\xE6W`\0\x80\xFD[PZ\xF1\x15\x80\x15a\r\xFAW=`\0\x80>=`\0\xFD[PPPP`@Q=`\0\x82>`\x1F=\x90\x81\x01\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x16\x82\x01`@Ra\x0E@\x91\x90\x81\x01\x90a2\xEDV[\x90P`\0\x88` \x01Q\x90P`\0a\x0Em\x83\x8C`\0\x015\x81Q\x81\x10a\x0E`W\xFE[` \x02` \x01\x01Qa\x1D%V[\x90P`\0a\x0E\x94\x84\x8D` \x015\x81Q\x81\x10a\x0E\x84W\xFE[` \x02` \x01\x01Q`\0\x03a\x1D%V[\x90P`\x02\x83`@Qa\x0E\xA6\x91\x90a6\xD2V[\x90\x81R` \x01`@Q\x80\x91\x03\x90 T`\0\x14a\x0E\xEEW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01a\x04M\x90a;\xD3V[\x7F\xF3\xB2wr\x8B?\xEEt\x94\x81\xEB>\x0B;H\x98\r\xBB\xABxe\x8F\xC4\x19\x02\\\xB1n\xEE4gu\x8Aa\x01\0\x01Q\x14\x15a\x0F\x82W\x89``\x01Q\x82\x14a\x0FXW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01a\x04M\x90a:\xC0V[\x89``\x01Q`\x02\x84`@Qa\x0Fm\x91\x90a6\xD2V[\x90\x81R`@Q\x90\x81\x90\x03` \x01\x90 Ua\x0F\xE5V[\x89`\x80\x01Q\x81\x14a\x0F\xBFW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01a\x04M\x90a:\xF7V[\x89`\x80\x01Q`\x02\x84`@Qa\x0F\xD4\x91\x90a6\xD2V[\x90\x81R`@Q\x90\x81\x90\x03` \x01\x90 U[\x8A`@\x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x7F\xA0zT:\xB8\xA0\x18\x19\x8E\x99\xCA\x01\x84\xC9?\xE9\x05\ny@\n\nr4A\xF8M\xE1\xD9r\xCC\x17\x8B`\0\x01Q\x8C` \x01Q\x85\x85\x8F`\xE0\x01Q\x89`@Qa\x10E\x96\x95\x94\x93\x92\x91\x90a8 V[`@Q\x80\x91\x03\x90\xA2`@Q3\x90\x7F@3\x8C\xE1\xA7\xC4\x92\x04\xF0\t\x953\xB1\xE9\xA7\xEE\n=&\x1F\x84\x97J\xB7\xAF6\x10[\x8CN\x9D\xB4\x90`\0\x90\xA2PP`\x01\x80UPPPPPPPPPPPPPPV[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[03\x14a\x10\xEBW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01a\x04M\x90a;eV[a\x10\xF7`\0\x83\x83a\x1D\x96V[PPV[\x80Q` \x81\x83\x01\x81\x01\x80Q`\0\x82R\x92\x82\x01\x91\x90\x93\x01 \x91RT\x81V[`\0a\x11$\x84\x84a\x1B/V[P\x91PPs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x163\x14a\x11\xACW`@\x80Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1A`$\x82\x01R\x7FGPv2: cannot presign order\0\0\0\0\0\0`D\x82\x01R\x90Q\x90\x81\x90\x03`d\x01\x90\xFD[\x81\x15a\x12\x06W\x7F\xF5\x9C\0\x92\x83\xFF\x87\xAAx ?\xC4\xD9\xC2\xDF\x02^\xE8Q\x13\x0F\xB6\x9C\xC3\xE0h\x94\x1Fk^-o`\0\x1C`\0\x85\x85`@Q\x80\x83\x83\x80\x82\x847\x80\x83\x01\x92PPP\x92PPP\x90\x81R` \x01`@Q\x80\x91\x03\x90 \x81\x90UPa\x122V[`\0\x80\x85\x85`@Q\x80\x83\x83\x80\x82\x847\x91\x90\x91\x01\x94\x85RPP`@Q\x92\x83\x90\x03` \x01\x90\x92 \x92\x90\x92UPP[\x80s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x7F\x01\xBF|\x8B\x0C\xA5]\xEE\xCB\xEA\x89\xD7\xE5\x82\x95\xB7\xFF\xBFh_\xD0\xD9h\x01\x03K\xA8\xC6\xFF\xE1\xC6\x8D\x85\x85\x85`@Q\x80\x80` \x01\x83\x15\x15\x81R` \x01\x82\x81\x03\x82R\x85\x85\x82\x81\x81R` \x01\x92P\x80\x82\x847`\0\x83\x82\x01R`@Q`\x1F\x90\x91\x01\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x16\x90\x92\x01\x82\x90\x03\x96P\x90\x94PPPPP\xA2PPPPV[03\x14a\x13\x10W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01a\x04M\x90a;eV[a\x10\xF7`\x02\x83\x83a\x1D\x96V[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[```\0cC!\x8E\x19`\xE0\x1B\x84\x84`@Q`$\x01\x80\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x80` \x01\x82\x81\x03\x82R\x83\x81\x81Q\x81R` \x01\x91P\x80Q\x90` \x01\x90\x80\x83\x83`\0[\x83\x81\x10\x15a\x13\xAAW\x81\x81\x01Q\x83\x82\x01R` \x01a\x13\x92V[PPPP\x90P\x90\x81\x01\x90`\x1F\x16\x80\x15a\x13\xD7W\x80\x82\x03\x80Q`\x01\x83` \x03a\x01\0\n\x03\x19\x16\x81R` \x01\x91P[P`@\x80Q\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x81\x84\x03\x01\x81R\x91\x81R` \x82\x01\x80Q{\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90\x98\x16\x97\x90\x97\x17\x87RQ\x81Q\x91\x97P0\x96\x88\x96P\x90\x94P\x84\x93P\x91P\x80\x83\x83[` \x83\x10a\x14\xA8W\x80Q\x82R\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x90\x92\x01\x91` \x91\x82\x01\x91\x01a\x14kV[`\x01\x83` \x03a\x01\0\n\x03\x80\x19\x82Q\x16\x81\x84Q\x16\x80\x82\x17\x85RPPPPPP\x90P\x01\x91PP`\0`@Q\x80\x83\x03\x81`\0\x86Z\xF1\x91PP=\x80`\0\x81\x14a\x15\nW`@Q\x91P`\x1F\x19`?=\x01\x16\x82\x01`@R=\x82R=`\0` \x84\x01>a\x15\x0FV[``\x91P[P\x90P\x80\x92PP`\0\x82`\x01\x84Q\x03\x81Q\x81\x10a\x15(W\xFE[` \x01\x01Q`\xF8\x1C`\xF8\x1B~\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x19\x16`\x01`\xF8\x1B\x14\x90Pa\x15k\x83`\x01\x85Q\x03a\x1EFV[\x80\x15a\x15xWPPa\x08\xE3V[a\x15\x81\x83a\x1B\xBDV[PP\x92\x91PPV[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[`\0[\x81\x81\x10\x15a\x16\xE5W6\x83\x83\x83\x81\x81\x10a\x15\xC5W\xFE[\x90P` \x02\x81\x01\x90a\x15\xD7\x91\x90a=\xDEV[\x90Ps\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16a\x16\x1D` \x83\x01\x83a1\x84V[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15a\x16kW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01a\x04M\x90a<\xAFV[a\x16t\x81a\x1EJV[a\x16\x81` \x82\x01\x82a1\x84V[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x7F\xED\x99\x82~\xFB7\x01o\"u\xF9\x8CK\xCFq\xC7U\x1Cu\xD5\x9E\x9BE\x0Fy\xFA2\xE6\x0B\xE6r\xC2\x82` \x015a\x16\xC6\x84a\x1E\xA1V[`@Qa\x16\xD4\x92\x91\x90a<\xE6V[`@Q\x80\x91\x03\x90\xA2P`\x01\x01a\x15\xB0V[PPPV[``\x80`\0a\x16\xF7a\x1B\xC5V[\x90P\x83g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x80\x15a\x17\x10W`\0\x80\xFD[P`@Q\x90\x80\x82R\x80` \x02` \x01\x82\x01`@R\x80\x15a\x17JW\x81` \x01[a\x177a/\x90V[\x81R` \x01\x90`\x01\x90\x03\x90\x81a\x17/W\x90P[P\x92P\x83g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x80\x15a\x17dW`\0\x80\xFD[P`@Q\x90\x80\x82R\x80` \x02` \x01\x82\x01`@R\x80\x15a\x17\x9EW\x81` \x01[a\x17\x8Ba/\x90V[\x81R` \x01\x90`\x01\x90\x03\x90\x81a\x17\x83W\x90P[P\x91P`\0[\x84\x81\x10\x15a\x18DW6\x86\x86\x83\x81\x81\x10a\x17\xB9W\xFE[\x90P` \x02\x81\x01\x90a\x17\xCB\x91\x90a>\x11V[\x90Pa\x17\xD9\x83\x8C\x8C\x84a\x1B\xF2V[a\x18;\x83\x8A\x8A\x845\x81\x81\x10a\x17\xEAW\xFE[\x90P` \x02\x015\x8B\x8B\x85` \x015\x81\x81\x10a\x18\x01W\xFE[\x90P` \x02\x015\x84a\x01 \x015\x89\x87\x81Q\x81\x10a\x18\x1AW\xFE[` \x02` \x01\x01Q\x89\x88\x81Q\x81\x10a\x18.W\xFE[` \x02` \x01\x01Qa\x1E\xCBV[P`\x01\x01a\x17\xA4V[PP\x96P\x96\x94PPPPPV[`\0\x81Qg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x80\x15a\x18kW`\0\x80\xFD[P`@Q\x90\x80\x82R\x80` \x02` \x01\x82\x01`@R\x80\x15a\x18\xA5W\x81` \x01[a\x18\x92a/\xB7V[\x81R` \x01\x90`\x01\x90\x03\x90\x81a\x18\x8AW\x90P[P\x90P`\0\x80[\x83Q\x81\x10\x15a\x1A\x93W`\0\x84\x82\x81Q\x81\x10a\x18\xC3W\xFE[` \x02` \x01\x01Q\x90Ps\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEEs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81` \x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15a\x19\xC7W\x7FJ\xC9\x9A\xCE\x14\xEE\n^\xF92\xDC`\x9D\xF0\x94:\xB7\xAC\x16\xB7X64a/\x8D\xC3ZB\x89\xA6\xCE\x81``\x01Q\x14\x15a\x19wW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01a\x04M\x90a;.V[\x80Q`@\x80\x83\x01Q\x90Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x90\x92\x16\x91\x81\x15a\x08\xFC\x02\x91\x90`\0\x81\x81\x81\x85\x88\x88\xF1\x93PPPP\x15\x80\x15a\x19\xC1W=`\0\x80>=`\0\xFD[Pa\x1A\x8AV[\x7FZ(\xE96;\xB9B\xB69'\0b\xAAk\xB2\x95\xF44\xBC\xDF\xC4,\x97&{\xF0\x03\xF2r\x06\r\xC9\x81``\x01Q\x14\x15a\x1A&W\x80Q`@\x82\x01Q` \x83\x01Qa\x1A!\x92s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x90\x91\x16\x91a\"\x16V[a\x1A\x8AV[`\0\x84\x84\x80`\x01\x01\x95P\x81Q\x81\x10a\x1A:W\xFE[` \x90\x81\x02\x91\x90\x91\x01\x81\x01Q`\0\x81R\x83\x82\x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x90\x81\x16\x92\x82\x01\x92\x90\x92R`@\x80\x85\x01Q\x90\x82\x01R0``\x82\x01R\x83Q\x90\x91\x16`\x80\x90\x91\x01RP[P`\x01\x01a\x18\xACV[P\x80\x15a\x1B)Wa\x1A\xA4\x82\x82a\x1EFV[`@Q\x7F\x0E\x8E>\x84\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x85\x16\x90c\x0E\x8E>\x84\x90a\x1A\xF6\x90\x85\x90`\x04\x01a7]V[`\0`@Q\x80\x83\x03\x81`\0\x87\x80;\x15\x80\x15a\x1B\x10W`\0\x80\xFD[PZ\xF1\x15\x80\x15a\x1B$W=`\0\x80>=`\0\xFD[PPPP[PPPPV[`\0\x80\x80`8\x84\x14a\x1B\xA2W`@\x80Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x11`$\x82\x01R\x7FGPv2: invalid uid\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`D\x82\x01R\x90Q\x90\x81\x90\x03`d\x01\x90\xFD[PP\x825\x93` \x84\x015``\x1C\x93`4\x015`\xE0\x1C\x92P\x90PV[\x80Q` \x82\x01\xFD[a\x1B\xCDa/\xE7V[`@\x80Q`8\x80\x82R``\x82\x01\x90\x92R\x90` \x82\x01\x81\x806\x837PPP` \x82\x01R\x90V[\x83Q`\0a\x1C\x02\x83\x86\x86\x85a\"\xEEV[\x90P`\0\x80a\x1C\x1F\x84\x84a\x1C\x1Aa\x01@\x89\x01\x89a={V[a#\xD6V[\x91P\x91Pa\x1CB\x82\x82\x86`\xA0\x01Q\x8B` \x01Qa$\x85\x90\x93\x92\x91\x90c\xFF\xFF\xFF\xFF\x16V[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x16`@\x89\x01Ra\x1Ch\x84\x82a%\x07V[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16``\x90\x98\x01\x97\x90\x97RPPPPPPPV[`\0\x7F\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11\x15a\x1D!W`@\x80Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x19`$\x82\x01R\x7FSafeCast: int256 overflow\0\0\0\0\0\0\0`D\x82\x01R\x90Q\x90\x81\x90\x03`d\x01\x90\xFD[P\x90V[`\0\x80\x82\x12\x15a\x1D!W`@\x80Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x16`$\x82\x01R\x7FSafeCast: not positive\0\0\0\0\0\0\0\0\0\0`D\x82\x01R\x90Q\x90\x81\x90\x03`d\x01\x90\xFD[`\0[\x81\x81\x10\x15a\x1B)W6`\0\x84\x84\x84\x81\x81\x10a\x1D\xB0W\xFE[\x90P` \x02\x81\x01\x90a\x1D\xC2\x91\x90a={V[\x91P\x91P`\0a\x1D\xD2\x83\x83a\x1B/V[\x92PPPB\x81c\xFF\xFF\xFF\xFF\x16\x10a\x1E\x15W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01a\x04M\x90a<\nV[`\0\x87\x84\x84`@Qa\x1E(\x92\x91\x90a6\xC2V[\x90\x81R`@Q\x90\x81\x90\x03` \x01\x90 UPP`\x01\x90\x91\x01\x90Pa\x1D\x99V[\x90RV[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x815\x16` \x82\x0156`\0a\x1Ew`@\x86\x01\x86a={V[\x91P\x91P`@Q\x81\x83\x827`\0\x80\x83\x83\x87\x89Z\xF1a\x1E\x99W=`\0\x80>=`\0\xFD[PPPPPPV[`\x006\x81a\x1E\xB2`@\x85\x01\x85a={V[\x90\x92P\x90P`\x04\x81\x10a\x1E\xC4W\x815\x92P[PP\x91\x90PV[\x85Q` \x87\x01Q`\xA0\x82\x01QBc\xFF\xFF\xFF\xFF\x90\x91\x16\x10\x15a\x1F\x18W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01a\x04M\x90a:RV[`\x80\x82\x01Qa\x1F'\x90\x87a%9V[``\x83\x01Qa\x1F6\x90\x89a%9V[\x10\x15a\x1FnW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01a\x04M\x90a:\x89V[`\0\x80`\0\x80\x7F\xF3\xB2wr\x8B?\xEEt\x94\x81\xEB>\x0B;H\x98\r\xBB\xABxe\x8F\xC4\x19\x02\\\xB1n\xEE4gu\x86a\x01\0\x01Q\x14\x15a oW\x85a\x01 \x01Q\x15a\x1F\xDBW\x88\x93Pa\x1F\xD4\x86``\x01Qa\x1F\xCE\x86\x89`\xE0\x01Qa%9\x90\x91\x90c\xFF\xFF\xFF\xFF\x16V[\x90a%\xC9V[\x91Pa\x1F\xEAV[\x85``\x01Q\x93P\x85`\xE0\x01Q\x91P[a\x1F\xFE\x8Aa\x1F\xF8\x86\x8Ea%9V[\x90a&JV[\x92Pa *\x84`\x02\x87`@Qa \x14\x91\x90a6\xD2V[\x90\x81R`@Q\x90\x81\x90\x03` \x01\x90 T\x90a&\xE8V[\x90P\x85``\x01Q\x81\x11\x15a jW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01a\x04M\x90a;\xD3V[a!\x16V[\x85a\x01 \x01Q\x15a \xA3W\x88\x92Pa \x9C\x86`\x80\x01Qa\x1F\xCE\x85\x89`\xE0\x01Qa%9\x90\x91\x90c\xFF\xFF\xFF\xFF\x16V[\x91Pa \xB2V[\x85`\x80\x01Q\x92P\x85`\xE0\x01Q\x91P[a \xC0\x8Ba\x1F\xCE\x85\x8Da%9V[\x93Pa \xD6\x83`\x02\x87`@Qa \x14\x91\x90a6\xD2V[\x90P\x85`\x80\x01Q\x81\x11\x15a!\x16W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01a\x04M\x90a;\xD3V[a! \x84\x83a&\xE8V[\x93P\x80`\x02\x86`@Qa!3\x91\x90a6\xD2V[\x90\x81R` \x01`@Q\x80\x91\x03\x90 \x81\x90UP\x8B`@\x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x7F\xA0zT:\xB8\xA0\x18\x19\x8E\x99\xCA\x01\x84\xC9?\xE9\x05\ny@\n\nr4A\xF8M\xE1\xD9r\xCC\x17\x87`\0\x01Q\x88` \x01Q\x87\x87\x87\x8B`@Qa!\xA1\x96\x95\x94\x93\x92\x91\x90a8 V[`@Q\x80\x91\x03\x90\xA2PP`@\x80\x8B\x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x90\x81\x16\x88R\x85Q\x81\x16` \x80\x8A\x01\x91\x90\x91R\x88\x83\x01\x94\x90\x94Ra\x01@\x86\x01Q``\x98\x89\x01R\x9A\x87\x01Q\x8B\x16\x86R\x82\x85\x01Q\x90\x9A\x16\x91\x85\x01\x91\x90\x91R\x97\x83\x01\x97\x90\x97Ra\x01`\x01Q\x91\x01RPPPPV[`@Q\x7F\xA9\x05\x9C\xBB\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x80\x82Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x84\x16`\x04\x83\x01R`$\x82\x01\x83\x90R\x90`\0\x80`D\x83\x82\x89Z\xF1a\"yW=`\0\x80>=`\0\xFD[Pa\"\x83\x84a'\\V[a\x1B)W`@\x80Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x15`$\x82\x01R\x7FGPv2: failed transfer\0\0\0\0\0\0\0\0\0\0\0`D\x82\x01R\x90Q\x90\x81\x90\x03`d\x01\x90\xFD[`\0\x83\x83\x865\x81\x81\x10a\"\xFDW\xFE[` \x90\x81\x02\x92\x90\x92\x015s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x84RP\x84\x90\x84\x90\x87\x015\x81\x81\x10a#0W\xFE[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF` \x91\x82\x02\x93\x90\x93\x015\x83\x16\x90\x85\x01RP`@\x80\x87\x015\x90\x91\x16\x90\x83\x01R``\x80\x86\x015\x90\x83\x01R`\x80\x80\x86\x015\x90\x83\x01Rc\xFF\xFF\xFF\xFF`\xA0\x80\x87\x015\x91\x90\x91\x16\x90\x83\x01R`\xC0\x80\x86\x015\x90\x83\x01R`\xE0\x80\x86\x015\x90\x83\x01Ra#\xACa\x01\0\x86\x015a(&V[a\x01`\x87\x01\x91\x90\x91Ra\x01@\x86\x01\x91\x90\x91R\x90\x15\x15a\x01 \x85\x01Ra\x01\0\x90\x93\x01RP\x93\x92PPPV[`\0\x80a$\x03\x86\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0a){V[\x91P`\0\x85`\x03\x81\x11\x15a$\x13W\xFE[\x14\x15a$+Wa$$\x82\x85\x85a*\x05V[\x90Pa$|V[`\x01\x85`\x03\x81\x11\x15a$9W\xFE[\x14\x15a$JWa$$\x82\x85\x85a*\x1AV[`\x02\x85`\x03\x81\x11\x15a$XW\xFE[\x14\x15a$iWa$$\x82\x85\x85a*\x82V[a$y\x82\x85\x85\x89`\xA0\x01Qa, V[\x90P[\x94P\x94\x92PPPV[`8\x84Q\x14a$\xF5W`@\x80Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x19`$\x82\x01R\x7FGPv2: uid buffer overflow\0\0\0\0\0\0\0`D\x82\x01R\x90Q\x90\x81\x90\x03`d\x01\x90\xFD[`8\x84\x01R`4\x83\x01R` \x90\x91\x01RV[`@\x82\x01Q`\0\x90s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16a%0WP\x80a\x08\xE3V[PP`@\x01Q\x90V[`\0\x82a%HWP`\0a\x08\xE3V[\x82\x82\x02\x82\x84\x82\x81a%UW\xFE[\x04\x14a%\xC2W`@\x80Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x16`$\x82\x01R\x7FSafeMath: mul overflow\0\0\0\0\0\0\0\0\0\0`D\x82\x01R\x90Q\x90\x81\x90\x03`d\x01\x90\xFD[\x93\x92PPPV[`\0\x80\x82\x11a&9W`@\x80Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x17`$\x82\x01R\x7FSafeMath: division by 0\0\0\0\0\0\0\0\0\0`D\x82\x01R\x90Q\x90\x81\x90\x03`d\x01\x90\xFD[\x81\x83\x81a&BW\xFE[\x04\x93\x92PPPV[`\0\x80\x82\x11a&\xBAW`@\x80Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1F`$\x82\x01R\x7FSafeMath: ceiling division by 0\0`D\x82\x01R\x90Q\x90\x81\x90\x03`d\x01\x90\xFD[\x81\x83\x81a&\xC3W\xFE[\x06\x15a&\xD0W`\x01a&\xD3V[`\0[`\xFF\x16\x82\x84\x81a&\xDFW\xFE[\x04\x01\x93\x92PPPV[`\0\x82\x82\x01\x83\x81\x10\x15a%\xC2W`@\x80Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1B`$\x82\x01R\x7FSafeMath: addition overflow\0\0\0\0\0`D\x82\x01R\x90Q\x90\x81\x90\x03`d\x01\x90\xFD[`\0a'\x9AV[\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\0R` `\x04R\x80`$R\x81`DR`d`\0\xFD[=\x80\x15a'\xD9W` \x81\x14a(\x13Wa'\xD4\x7FGPv2: malformed transfer result\0`\x1Fa'cV[a( V[\x82;a(\nWa(\n\x7FGPv2: not a contract\0\0\0\0\0\0\0\0\0\0\0\0`\x14a'cV[`\x01\x91Pa( V[=`\0\x80>`\0Q\x15\x15\x91P[P\x91\x90PV[`\0\x80\x80\x80\x80`\x01\x86\x16a(\\W\x7F\xF3\xB2wr\x8B?\xEEt\x94\x81\xEB>\x0B;H\x98\r\xBB\xABxe\x8F\xC4\x19\x02\\\xB1n\xEE4gu\x94Pa(\x80V[\x7Fn\xD8\x8E\x86\x8A\xF0\xA1\x98>8\x86\xD5\xF3\xE9Z/\xAF\xBDl4P\xBC\"\x9E'4\"\x83\xDCB\x9C\xCC\x94P[`\x02\x86\x16\x15\x15\x93P`\x08\x86\x16a(\xB8W\x7FZ(\xE96;\xB9B\xB69'\0b\xAAk\xB2\x95\xF44\xBC\xDF\xC4,\x97&{\xF0\x03\xF2r\x06\r\xC9\x92Pa)\x0CV[`\x04\x86\x16a(\xE8W\x7F\xAB\xEE;s7:\xCDX:\x13\t$\xAA\xD6\xDC8\xCF\xDCD\xBA\x05U\xBA\x94\xCE/\xF69\x80\xEA\x062\x92Pa)\x0CV[\x7FJ\xC9\x9A\xCE\x14\xEE\n^\xF92\xDC`\x9D\xF0\x94:\xB7\xAC\x16\xB7X64a/\x8D\xC3ZB\x89\xA6\xCE\x92P[`\x10\x86\x16a)<W\x7FZ(\xE96;\xB9B\xB69'\0b\xAAk\xB2\x95\xF44\xBC\xDF\xC4,\x97&{\xF0\x03\xF2r\x06\r\xC9\x91Pa)`V[\x7FJ\xC9\x9A\xCE\x14\xEE\n^\xF92\xDC`\x9D\xF0\x94:\xB7\xAC\x16\xB7X64a/\x8D\xC3ZB\x89\xA6\xCE\x91P[`\x05\x86\x90\x1C`\x03\x81\x11\x15a)pW\xFE[\x90P\x91\x93\x95\x90\x92\x94PV[\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x90\x91\x01\x80Q\x7F\xD5\xA2[\xA2\xE9p\x94\xAD}\x83\xDC(\xA6W-\xA7\x97\xD6\xB3\xE7\xFCfc\xBD\x93\xEF\xB7\x89\xFC\x17\xE4\x89\x82Ra\x01\xA0\x82 \x91R`@Q\x7F\x19\x01\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x02\x81\x01\x92\x90\x92R`\"\x82\x01R`B\x90 \x90V[`\0a*\x12\x84\x84\x84a-\xE5V[\x94\x93PPPPV[`\0\x80\x84`@Q` \x01\x80\x80\x7F\x19Ethereum Signed Message:\n32\0\0\0\0\x81RP`\x1C\x01\x82\x81R` \x01\x91PP`@Q` \x81\x83\x03\x03\x81R\x90`@R\x80Q\x90` \x01 \x90Pa*y\x81\x85\x85a-\xE5V[\x95\x94PPPPPV[\x815``\x1C6`\0a*\x97\x84`\x14\x81\x88a>hV[`@\x80Q\x7F\x16&\xBA~\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x80\x82R`\x04\x82\x01\x8B\x81R`$\x83\x01\x93\x84R`D\x83\x01\x85\x90R\x94\x96P\x92\x94P\x91\x92s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x87\x16\x92c\x16&\xBA~\x92\x8B\x92\x88\x92\x88\x92\x90`d\x01\x84\x84\x80\x82\x847`\0\x83\x82\x01R`@Q`\x1F\x90\x91\x01\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x16\x90\x92\x01\x96P` \x95P\x90\x93PPP\x81\x84\x03\x90P\x81\x86\x80;\x15\x80\x15a+]W`\0\x80\xFD[PZ\xFA\x15\x80\x15a+qW=`\0\x80>=`\0\xFD[PPPP`@Q=` \x81\x10\x15a+\x87W`\0\x80\xFD[PQ\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x14a,\x17W`@\x80Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1F`$\x82\x01R\x7FGPv2: invalid eip1271 signature\0`D\x82\x01R\x90Q\x90\x81\x90\x03`d\x01\x90\xFD[PP\x93\x92PPPV[`\0`\x14\x83\x14a,\x91W`@\x80Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1C`$\x82\x01R\x7FGPv2: malformed presignature\0\0\0\0`D\x82\x01R\x90Q\x90\x81\x90\x03`d\x01\x90\xFD[P`@\x80Q`8\x80\x82R``\x82\x81\x01\x90\x93R\x855\x90\x92\x1C\x91`\0\x91\x90` \x82\x01\x81\x806\x837\x01\x90PP\x90Pa,\xC8\x81\x87\x84\x86a$\x85V[\x7F\xF5\x9C\0\x92\x83\xFF\x87\xAAx ?\xC4\xD9\xC2\xDF\x02^\xE8Q\x13\x0F\xB6\x9C\xC3\xE0h\x94\x1Fk^-o`\0\x1C`\0\x82`@Q\x80\x82\x80Q\x90` \x01\x90\x80\x83\x83[` \x83\x10a-<W\x80Q\x82R\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x90\x92\x01\x91` \x91\x82\x01\x91\x01a,\xFFV[`\x01\x83` \x03a\x01\0\n\x03\x80\x19\x82Q\x16\x81\x84Q\x16\x80\x82\x17\x85RPPPPPP\x90P\x01\x91PP\x90\x81R` \x01`@Q\x80\x91\x03\x90 T\x14a-\xDCW`@\x80Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x19`$\x82\x01R\x7FGPv2: order not presigned\0\0\0\0\0\0\0`D\x82\x01R\x90Q\x90\x81\x90\x03`d\x01\x90\xFD[P\x94\x93PPPPV[`\0`A\x82\x14a.VW`@\x80Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1F`$\x82\x01R\x7FGPv2: malformed ecdsa signature\0`D\x82\x01R\x90Q\x90\x81\x90\x03`d\x01\x90\xFD[`@\x80Q`\0\x81R` \x81\x81\x01\x80\x84R\x87\x90R\x82\x86\x015`\xF8\x1C\x82\x84\x01\x81\x90R\x865``\x84\x01\x81\x90R\x82\x88\x015`\x80\x85\x01\x81\x90R\x94Q\x90\x94\x93\x91\x92`\x01\x92`\xA0\x80\x82\x01\x93\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x81\x01\x92\x81\x90\x03\x90\x91\x01\x90\x85Z\xFA\x15\x80\x15a.\xD9W=`\0\x80>=`\0\xFD[PP`@Q\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x01Q\x94PPs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x84\x16a/\x86W`@\x80Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1D`$\x82\x01R\x7FGPv2: invalid ecdsa signature\0\0\0`D\x82\x01R\x90Q\x90\x81\x90\x03`d\x01\x90\xFD[PPP\x93\x92PPPV[`@\x80Q`\x80\x81\x01\x82R`\0\x80\x82R` \x82\x01\x81\x90R\x91\x81\x01\x82\x90R``\x81\x01\x91\x90\x91R\x90V[`@\x80Q`\xA0\x81\x01\x90\x91R\x80`\0\x81R`\0` \x82\x01\x81\x90R`@\x82\x01\x81\x90R``\x82\x01\x81\x90R`\x80\x90\x91\x01R\x90V[`@Q\x80`\x80\x01`@R\x80a/\xFAa0\x14V[\x81R``` \x82\x01\x81\x90R`\0`@\x83\x01\x81\x90R\x91\x01R\x90V[`@\x80Qa\x01\x80\x81\x01\x82R`\0\x80\x82R` \x82\x01\x81\x90R\x91\x81\x01\x82\x90R``\x81\x01\x82\x90R`\x80\x81\x01\x82\x90R`\xA0\x81\x01\x82\x90R`\xC0\x81\x01\x82\x90R`\xE0\x81\x01\x82\x90Ra\x01\0\x81\x01\x82\x90Ra\x01 \x81\x01\x82\x90Ra\x01@\x81\x01\x82\x90Ra\x01`\x81\x01\x91\x90\x91R\x90V[`\0\x80\x83`\x1F\x84\x01\x12a0\x89W\x81\x82\xFD[P\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a0\xA0W\x81\x82\xFD[` \x83\x01\x91P\x83` \x80\x83\x02\x85\x01\x01\x11\x15a0\xBAW`\0\x80\xFD[\x92P\x92\x90PV[`\0\x80\x83`\x1F\x84\x01\x12a0\xD2W\x81\x82\xFD[P\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a0\xE9W\x81\x82\xFD[` \x83\x01\x91P\x83` \x82\x85\x01\x01\x11\x15a0\xBAW`\0\x80\xFD[`\0\x82`\x1F\x83\x01\x12a1\x11W\x80\x81\xFD[\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a1%W\xFE[a1V` \x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`\x1F\x84\x01\x16\x01a>DV[\x81\x81R\x84` \x83\x86\x01\x01\x11\x15a1jW\x82\x83\xFD[\x81` \x85\x01` \x83\x017\x90\x81\x01` \x01\x91\x90\x91R\x92\x91PPV[`\0` \x82\x84\x03\x12\x15a1\x95W\x80\x81\xFD[\x815a%\xC2\x81a>\xBCV[`\0\x80`@\x83\x85\x03\x12\x15a1\xB2W\x80\x81\xFD[\x825a1\xBD\x81a>\xBCV[\x91P` \x83\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a1\xD8W\x81\x82\xFD[a1\xE4\x85\x82\x86\x01a1\x01V[\x91PP\x92P\x92\x90PV[`\0\x80` \x83\x85\x03\x12\x15a2\0W\x81\x82\xFD[\x825g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a2\x16W\x82\x83\xFD[a2\"\x85\x82\x86\x01a0xV[\x90\x96\x90\x95P\x93PPPPV[`\0\x80`\0\x80`\0\x80`\0`\x80\x88\x8A\x03\x12\x15a2HW\x82\x83\xFD[\x875g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a2_W\x84\x85\xFD[a2k\x8B\x83\x8C\x01a0xV[\x90\x99P\x97P` \x8A\x015\x91P\x80\x82\x11\x15a2\x83W\x84\x85\xFD[a2\x8F\x8B\x83\x8C\x01a0xV[\x90\x97P\x95P`@\x8A\x015\x91P\x80\x82\x11\x15a2\xA7W\x84\x85\xFD[a2\xB3\x8B\x83\x8C\x01a0xV[\x90\x95P\x93P``\x8A\x015\x91P\x80\x82\x11\x15a2\xCBW\x82\x83\xFD[P\x88\x01``\x81\x01\x8A\x10\x15a2\xDDW\x81\x82\xFD[\x80\x91PP\x92\x95\x98\x91\x94\x97P\x92\x95PV[`\0` \x80\x83\x85\x03\x12\x15a2\xFFW\x81\x82\xFD[\x82Qg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a3\x16W\x83\x84\xFD[\x81\x85\x01\x91P\x85`\x1F\x83\x01\x12a3)W\x83\x84\xFD[\x81Q\x81\x81\x11\x15a35W\xFE[\x83\x81\x02\x91Pa3E\x84\x83\x01a>DV[\x81\x81R\x84\x81\x01\x90\x84\x86\x01\x84\x86\x01\x87\x01\x8A\x10\x15a3_W\x87\x88\xFD[\x87\x95P[\x83\x86\x10\x15a3\x81W\x80Q\x83R`\x01\x95\x90\x95\x01\x94\x91\x86\x01\x91\x86\x01a3cV[P\x98\x97PPPPPPPPV[`\0\x80`\0\x80`\0``\x86\x88\x03\x12\x15a3\xA5W\x80\x81\xFD[\x855g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a3\xBCW\x82\x83\xFD[a3\xC8\x89\x83\x8A\x01a0xV[\x90\x97P\x95P` \x88\x015\x91P\x80\x82\x11\x15a3\xE0W\x82\x83\xFD[a3\xEC\x89\x83\x8A\x01a0xV[\x90\x95P\x93P`@\x88\x015\x91P\x80\x82\x11\x15a4\x04W\x82\x83\xFD[P\x86\x01a\x01`\x81\x89\x03\x12\x15a4\x17W\x81\x82\xFD[\x80\x91PP\x92\x95P\x92\x95\x90\x93PV[`\0` \x82\x84\x03\x12\x15a46W\x80\x81\xFD[\x81Qa%\xC2\x81a>\xE1V[`\0\x80` \x83\x85\x03\x12\x15a4SW\x81\x82\xFD[\x825g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a4iW\x82\x83\xFD[a2\"\x85\x82\x86\x01a0\xC1V[`\0\x80`\0`@\x84\x86\x03\x12\x15a4\x89W\x80\x81\xFD[\x835g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a4\x9FW\x81\x82\xFD[a4\xAB\x86\x82\x87\x01a0\xC1V[\x90\x94P\x92PP` \x84\x015a4\xBF\x81a>\xE1V[\x80\x91PP\x92P\x92P\x92V[`\0` \x82\x84\x03\x12\x15a4\xDBW\x80\x81\xFD[\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a4\xF1W\x81\x82\xFD[a*\x12\x84\x82\x85\x01a1\x01V[`\0\x80`@\x83\x85\x03\x12\x15a5\x0FW\x81\x82\xFD[PP\x805\x92` \x90\x91\x015\x91PV[`\0\x82\x84R` \x80\x85\x01\x94P\x82\x82[\x85\x81\x10\x15a5hW\x815a5@\x81a>\xBCV[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x87R\x95\x82\x01\x95\x90\x82\x01\x90`\x01\x01a5-V[P\x94\x95\x94PPPPPV[`\0\x81Q\x80\x84R` \x80\x85\x01\x94P\x80\x84\x01\x83[\x83\x81\x10\x15a5hW\x81Q\x87R\x95\x82\x01\x95\x90\x82\x01\x90`\x01\x01a5\x86V[`\0\x82\x84R\x82\x82` \x86\x017\x80` \x84\x86\x01\x01R` \x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`\x1F\x85\x01\x16\x85\x01\x01\x90P\x93\x92PPPV[`\0\x81Q\x80\x84Ra6\x02\x81` \x86\x01` \x86\x01a>\x90V[`\x1F\x01\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x16\x92\x90\x92\x01` \x01\x92\x91PPV[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82Q\x16\x83R\x80` \x83\x01Q\x16` \x84\x01RP`@\x81\x01Q`@\x83\x01R``\x81\x01Q``\x83\x01RPPV[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82Q\x16\x83R` \x82\x01Q\x15\x15` \x84\x01R\x80`@\x83\x01Q\x16`@\x84\x01RP``\x81\x01Q\x15\x15``\x83\x01RPPV[c\xFF\xFF\xFF\xFF\x16\x90RV[`\0\x82\x84\x837\x91\x01\x90\x81R\x91\x90PV[`\0\x82Qa6\xE4\x81\x84` \x87\x01a>\x90V[\x91\x90\x91\x01\x92\x91PPV[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x91\x90\x91\x16\x81R` \x01\x90V[` \x80\x82R\x82Q\x82\x82\x01\x81\x90R`\0\x91\x90\x84\x82\x01\x90`@\x85\x01\x90\x84[\x81\x81\x10\x15a7QWa7>\x83\x85Qa64V[\x92\x84\x01\x92`\x80\x92\x90\x92\x01\x91`\x01\x01a7+V[P\x90\x96\x95PPPPPPV[` \x80\x82R\x82Q\x82\x82\x01\x81\x90R`\0\x91\x90`@\x90\x81\x85\x01\x90\x86\x84\x01\x85[\x82\x81\x10\x15a7\xE3W\x81Q\x80Q`\x04\x81\x10a7\x90W\xFE[\x85R\x80\x87\x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x90\x81\x16\x88\x87\x01R\x86\x82\x01Q\x87\x87\x01R``\x80\x83\x01Q\x82\x16\x90\x87\x01R`\x80\x91\x82\x01Q\x16\x90\x85\x01R`\xA0\x90\x93\x01\x92\x90\x85\x01\x90`\x01\x01a7zV[P\x91\x97\x96PPPPPPPV[\x90\x81R` \x01\x90V[`\0` \x82Ra*\x12` \x83\x01\x84\x86a5\xA2V[`\0` \x82Ra%\xC2` \x83\x01\x84a5\xEAV[`\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x89\x16\x83R\x80\x88\x16` \x84\x01RP\x85`@\x83\x01R\x84``\x83\x01R\x83`\x80\x83\x01R`\xC0`\xA0\x83\x01Ra8k`\xC0\x83\x01\x84a5\xEAV[\x98\x97PPPPPPPPV[`\0a\x01\xA0\x82\x01`\x02\x8C\x10a8\x88W\xFE[\x8B\x83R` a\x01\xA0\x81\x85\x01R\x81\x8B\x83Ra\x01\xC0\x85\x01\x90Pa\x01\xC0\x82\x8D\x02\x86\x01\x01\x92P\x8C\x84[\x8D\x81\x10\x15a9\xB6W\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFE@\x87\x86\x03\x01\x83R\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFFa\x8F6\x03\x01\x825\x12a9\x0CW\x85\x86\xFD[\x8E\x825\x01\x805\x86R\x84\x81\x015\x85\x87\x01R`@\x81\x015`@\x87\x01R``\x81\x015``\x87\x01R`\x80\x81\x015\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE1\x826\x03\x01\x81\x12a9dW\x87\x88\xFD[\x81\x01\x805g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a9|W\x88\x89\xFD[\x806\x03\x83\x13\x15a9\x8AW\x88\x89\xFD[`\xA0`\x80\x89\x01Ra9\xA1`\xA0\x89\x01\x82\x89\x85\x01a5\xA2V[\x97PPP\x92\x84\x01\x92P\x90\x83\x01\x90`\x01\x01a8\xADV[PPPP\x82\x81\x03`@\x84\x01Ra9\xCD\x81\x89\x8Ba5\x1EV[\x90Pa9\xDC``\x84\x01\x88a6tV[\x82\x81\x03`\xE0\x84\x01Ra9\xEE\x81\x87a5sV[\x91PPa9\xFFa\x01\0\x83\x01\x85a6\xB8V[a:\ra\x01 \x83\x01\x84a64V[\x9A\x99PPPPPPPPPPV[` \x80\x82R`\x1F\x90\x82\x01R\x7FGPv2: caller does not own order\0`@\x82\x01R``\x01\x90V[` \x80\x82R`\x13\x90\x82\x01R\x7FGPv2: order expired\0\0\0\0\0\0\0\0\0\0\0\0\0`@\x82\x01R``\x01\x90V[` \x80\x82R`\x1F\x90\x82\x01R\x7FGPv2: limit price not respected\0`@\x82\x01R``\x01\x90V[` \x80\x82R`\x1F\x90\x82\x01R\x7FGPv2: sell amount not respected\0`@\x82\x01R``\x01\x90V[` \x80\x82R`\x1E\x90\x82\x01R\x7FGPv2: buy amount not respected\0\0`@\x82\x01R``\x01\x90V[` \x80\x82R`\x1E\x90\x82\x01R\x7FGPv2: unsupported internal ETH\0\0`@\x82\x01R``\x01\x90V[` \x80\x82R`\x18\x90\x82\x01R\x7FGPv2: not an interaction\0\0\0\0\0\0\0\0`@\x82\x01R``\x01\x90V[` \x80\x82R`\x14\x90\x82\x01R\x7FGPv2: limit too high\0\0\0\0\0\0\0\0\0\0\0\0`@\x82\x01R``\x01\x90V[` \x80\x82R`\x12\x90\x82\x01R\x7FGPv2: order filled\0\0\0\0\0\0\0\0\0\0\0\0\0\0`@\x82\x01R``\x01\x90V[` \x80\x82R`\x17\x90\x82\x01R\x7FGPv2: order still valid\0\0\0\0\0\0\0\0\0`@\x82\x01R``\x01\x90V[` \x80\x82R`\x13\x90\x82\x01R\x7FGPv2: limit too low\0\0\0\0\0\0\0\0\0\0\0\0\0`@\x82\x01R``\x01\x90V[` \x80\x82R`\x12\x90\x82\x01R\x7FGPv2: not a solver\0\0\0\0\0\0\0\0\0\0\0\0\0\0`@\x82\x01R``\x01\x90V[` \x80\x82R`\x1B\x90\x82\x01R\x7FGPv2: forbidden interaction\0\0\0\0\0`@\x82\x01R``\x01\x90V[\x91\x82R\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16` \x82\x01R`@\x01\x90V[`\0\x80\x835\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE1\x846\x03\x01\x81\x12a=JW\x82\x83\xFD[\x83\x01\x805\x91Pg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11\x15a=dW\x82\x83\xFD[` \x90\x81\x01\x92P\x81\x026\x03\x82\x13\x15a0\xBAW`\0\x80\xFD[`\0\x80\x835\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE1\x846\x03\x01\x81\x12a=\xAFW\x82\x83\xFD[\x83\x01\x805\x91Pg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11\x15a=\xC9W\x82\x83\xFD[` \x01\x91P6\x81\x90\x03\x82\x13\x15a0\xBAW`\0\x80\xFD[`\0\x825\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xA1\x836\x03\x01\x81\x12a6\xE4W\x81\x82\xFD[`\0\x825\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFE\xA1\x836\x03\x01\x81\x12a6\xE4W\x81\x82\xFD[`@Q\x81\x81\x01g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x82\x82\x10\x17\x15a>`W\xFE[`@R\x91\x90PV[`\0\x80\x85\x85\x11\x15a>wW\x81\x82\xFD[\x83\x86\x11\x15a>\x83W\x81\x82\xFD[PP\x82\x01\x93\x91\x90\x92\x03\x91PV[`\0[\x83\x81\x10\x15a>\xABW\x81\x81\x01Q\x83\x82\x01R` \x01a>\x93V[\x83\x81\x11\x15a\x1B)WPP`\0\x91\x01RV[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x16\x81\x14a>\xDEW`\0\x80\xFD[PV[\x80\x15\x15\x81\x14a>\xDEW`\0\x80\xFD\xFE\xA2dipfsX\"\x12 \xDE^I<H\xA3\xB4-\xA0:]\xB8\x90\x85\x17{\x8D\x8C\xCE\xC6\xE9\xBFn\x8EH\xB3\x80\x93CbL\x8FdsolcC\0\x07\x06\x003`\xC0`@R4\x80\x15a\0\x10W`\0\x80\xFD[P`@Qa\x12\x9E8\x03\x80a\x12\x9E\x839\x81\x01`@\x81\x90Ra\0/\x91a\0KV[3``\x90\x81\x1B`\x80R\x1B`\x01`\x01``\x1B\x03\x19\x16`\xA0Ra\0yV[`\0` \x82\x84\x03\x12\x15a\0\\W\x80\x81\xFD[\x81Q`\x01`\x01`\xA0\x1B\x03\x81\x16\x81\x14a\0rW\x81\x82\xFD[\x93\x92PPPV[`\x80Q``\x1C`\xA0Q``\x1Ca\x11\xEEa\0\xB0`\09\x80a\x010R\x80a\x02\x01R\x80a\x02\xBDRP\x80`\x93R\x80a\x02LRPa\x11\xEE`\0\xF3\xFE`\x80`@R4\x80\x15a\0\x10W`\0\x80\xFD[P`\x046\x10a\x006W`\x005`\xE0\x1C\x80cH\x17\xA2\x86\x14a\0;W\x80c}\x10\xD1\x1F\x14a\0dW[`\0\x80\xFD[a\0Na\0I6`\x04a\x0C\xD9V[a\0yV[`@Qa\0[\x91\x90a\x0E\xB3V[`@Q\x80\x91\x03\x90\xF3[a\0wa\0r6`\x04a\x0CiV[a\x024V[\0[``3s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x14a\0\xF3W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01a\0\xEA\x90a\x10\xE5V[`@Q\x80\x91\x03\x90\xFD[`@Q\x7F\x94[\xCE\xC9\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x90c\x94[\xCE\xC9\x90a\x01q\x90\x8C\x90\x8C\x90\x8C\x90\x8C\x90\x8C\x90\x8C\x90\x8C\x90`\x04\x01a\x0FYV[`\0`@Q\x80\x83\x03\x81`\0\x87\x80;\x15\x80\x15a\x01\x8BW`\0\x80\xFD[PZ\xF1\x15\x80\x15a\x01\x9FW=`\0\x80>=`\0\xFD[PPPP`@Q=`\0\x82>`\x1F=\x90\x81\x01\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x16\x82\x01`@Ra\x01\xE5\x91\x90\x81\x01\x90a\x0B\xD9V[\x90Pa\x02(s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x833a\x02\xE9V[\x98\x97PPPPPPPPV[3s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x14a\x02\xA3W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01a\0\xEA\x90a\x10\xE5V[a\x02\xE5s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x83\x833a\x05QV[PPV[s\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEEa\x03\x0E`@\x84\x01` \x85\x01a\x0B\xB6V[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15a\x03\\W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01a\0\xEA\x90a\x11\x1CV[\x7FZ(\xE96;\xB9B\xB69'\0b\xAAk\xB2\x95\xF44\xBC\xDF\xC4,\x97&{\xF0\x03\xF2r\x06\r\xC9\x82``\x015\x14\x15a\x03\xD0Wa\x03\xCBa\x03\x98` \x84\x01\x84a\x0B\xB6V[\x82`@\x85\x01\x805\x90a\x03\xAD\x90` \x88\x01a\x0B\xB6V[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x92\x91\x90a\x08\x16V[a\x05LV[`@\x80Q`\x01\x80\x82R\x81\x83\x01\x90\x92R`\0\x91\x81` \x01[a\x03\xEFa\t\xCBV[\x81R` \x01\x90`\x01\x90\x03\x90\x81a\x03\xE7W\x90PP\x90P`\0\x81`\0\x81Q\x81\x10a\x04\x13W\xFE[` \x02` \x01\x01Q\x90P\x7F\xAB\xEE;s7:\xCDX:\x13\t$\xAA\xD6\xDC8\xCF\xDCD\xBA\x05U\xBA\x94\xCE/\xF69\x80\xEA\x062\x84``\x015\x14a\x04OW`\x02a\x04RV[`\x03[\x81\x90`\x03\x81\x11\x15a\x04_W\xFE[\x90\x81`\x03\x81\x11\x15a\x04lW\xFE[\x90RPa\x04\x7F`@\x85\x01` \x86\x01a\x0B\xB6V[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16` \x80\x83\x01\x91\x90\x91R`@\x80\x86\x015\x90\x83\x01Ra\x04\xB4\x90\x85\x01\x85a\x0B\xB6V[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x90\x81\x16``\x83\x01R\x83\x81\x16`\x80\x83\x01R`@Q\x7F\x0E\x8E>\x84\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R\x90\x86\x16\x90c\x0E\x8E>\x84\x90a\x05\x17\x90\x85\x90`\x04\x01a\x0E\xC6V[`\0`@Q\x80\x83\x03\x81`\0\x87\x80;\x15\x80\x15a\x051W`\0\x80\xFD[PZ\xF1\x15\x80\x15a\x05EW=`\0\x80>=`\0\xFD[PPPPPP[PPPV[`\0\x82g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x80\x15a\x05jW`\0\x80\xFD[P`@Q\x90\x80\x82R\x80` \x02` \x01\x82\x01`@R\x80\x15a\x05\xA4W\x81` \x01[a\x05\x91a\t\xCBV[\x81R` \x01\x90`\x01\x90\x03\x90\x81a\x05\x89W\x90P[P\x90P`\0\x80[\x84\x81\x10\x15a\x07xW6\x86\x86\x83\x81\x81\x10a\x05\xC0W\xFE[`\x80\x02\x91\x90\x91\x01\x91Ps\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\xEE\x90Pa\x05\xF0`@\x83\x01` \x84\x01a\x0B\xB6V[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15a\x06>W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01a\0\xEA\x90a\x11\x1CV[\x7FZ(\xE96;\xB9B\xB69'\0b\xAAk\xB2\x95\xF44\xBC\xDF\xC4,\x97&{\xF0\x03\xF2r\x06\r\xC9\x81``\x015\x14\x15a\x06\x94Wa\x06\x8Fa\x06z` \x83\x01\x83a\x0B\xB6V[\x86`@\x84\x01\x805\x90a\x03\xAD\x90` \x87\x01a\x0B\xB6V[a\x07oV[`\0\x84\x84\x80`\x01\x01\x95P\x81Q\x81\x10a\x06\xA8W\xFE[` \x02` \x01\x01Q\x90P\x7F\xAB\xEE;s7:\xCDX:\x13\t$\xAA\xD6\xDC8\xCF\xDCD\xBA\x05U\xBA\x94\xCE/\xF69\x80\xEA\x062\x82``\x015\x14a\x06\xE4W`\x01a\x06\xE7V[`\x03[\x81\x90`\x03\x81\x11\x15a\x06\xF4W\xFE[\x90\x81`\x03\x81\x11\x15a\x07\x01W\xFE[\x90RPa\x07\x14`@\x83\x01` \x84\x01a\x0B\xB6V[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16` \x80\x83\x01\x91\x90\x91R`@\x80\x84\x015\x90\x83\x01Ra\x07I\x90\x83\x01\x83a\x0B\xB6V[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x90\x81\x16``\x83\x01R\x86\x16`\x80\x90\x91\x01R[P`\x01\x01a\x05\xABV[P\x80\x15a\x08\x0EWa\x07\x89\x82\x82a\x08\xFDV[`@Q\x7F\x0E\x8E>\x84\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x87\x16\x90c\x0E\x8E>\x84\x90a\x07\xDB\x90\x85\x90`\x04\x01a\x0E\xC6V[`\0`@Q\x80\x83\x03\x81`\0\x87\x80;\x15\x80\x15a\x07\xF5W`\0\x80\xFD[PZ\xF1\x15\x80\x15a\x08\tW=`\0\x80>=`\0\xFD[PPPP[PPPPPPV[`@Q\x7F#\xB8r\xDD\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x80\x82Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x85\x81\x16`\x04\x84\x01R\x84\x16`$\x83\x01R`D\x82\x01\x83\x90R\x90`\0\x80`d\x83\x82\x8AZ\xF1a\x08\x81W=`\0\x80>=`\0\xFD[Pa\x08\x8B\x85a\t\x01V[a\x08\xF6W`@\x80Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x19`$\x82\x01R\x7FGPv2: failed transferFrom\0\0\0\0\0\0\0`D\x82\x01R\x90Q\x90\x81\x90\x03`d\x01\x90\xFD[PPPPPV[\x90RV[`\0a\t?V[\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\0R` `\x04R\x80`$R\x81`DR`d`\0\xFD[=\x80\x15a\t~W` \x81\x14a\t\xB8Wa\ty\x7FGPv2: malformed transfer result\0`\x1Fa\t\x08V[a\t\xC5V[\x82;a\t\xAFWa\t\xAF\x7FGPv2: not a contract\0\0\0\0\0\0\0\0\0\0\0\0`\x14a\t\x08V[`\x01\x91Pa\t\xC5V[=`\0\x80>`\0Q\x15\x15\x91P[P\x91\x90PV[`@\x80Q`\xA0\x81\x01\x90\x91R\x80`\0\x81R`\0` \x82\x01\x81\x90R`@\x82\x01\x81\x90R``\x82\x01\x81\x90R`\x80\x90\x91\x01R\x90V[`\0\x82`\x1F\x83\x01\x12a\n\x0BW\x80\x81\xFD[\x815` a\n a\n\x1B\x83a\x11uV[a\x11QV[\x82\x81R\x81\x81\x01\x90\x85\x83\x01\x83\x85\x02\x87\x01\x84\x01\x88\x10\x15a\n<W\x85\x86\xFD[\x85[\x85\x81\x10\x15a\ncW\x815a\nQ\x81a\x11\x93V[\x84R\x92\x84\x01\x92\x90\x84\x01\x90`\x01\x01a\n>V[P\x90\x97\x96PPPPPPPV[`\0\x82`\x1F\x83\x01\x12a\n\x80W\x80\x81\xFD[\x815` a\n\x90a\n\x1B\x83a\x11uV[\x82\x81R\x81\x81\x01\x90\x85\x83\x01\x83\x85\x02\x87\x01\x84\x01\x88\x10\x15a\n\xACW\x85\x86\xFD[\x85[\x85\x81\x10\x15a\ncW\x815\x84R\x92\x84\x01\x92\x90\x84\x01\x90`\x01\x01a\n\xAEV[`\0\x80\x83`\x1F\x84\x01\x12a\n\xDBW\x81\x82\xFD[P\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\n\xF2W\x81\x82\xFD[` \x83\x01\x91P\x83` \x80\x83\x02\x85\x01\x01\x11\x15a\x0B\x0CW`\0\x80\xFD[\x92P\x92\x90PV[\x805\x80\x15\x15\x81\x14a\x0B#W`\0\x80\xFD[\x91\x90PV[`\0`\x80\x82\x84\x03\x12\x15a\t\xC5W\x80\x81\xFD[`\0`\x80\x82\x84\x03\x12\x15a\x0BJW\x80\x81\xFD[`@Q`\x80\x81\x01\x81\x81\x10g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11\x17\x15a\x0BgW\xFE[`@R\x90P\x80\x825a\x0Bx\x81a\x11\x93V[\x81Ra\x0B\x86` \x84\x01a\x0B\x13V[` \x82\x01R`@\x83\x015a\x0B\x99\x81a\x11\x93V[`@\x82\x01Ra\x0B\xAA``\x84\x01a\x0B\x13V[``\x82\x01RP\x92\x91PPV[`\0` \x82\x84\x03\x12\x15a\x0B\xC7W\x80\x81\xFD[\x815a\x0B\xD2\x81a\x11\x93V[\x93\x92PPPV[`\0` \x80\x83\x85\x03\x12\x15a\x0B\xEBW\x81\x82\xFD[\x82Qg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x0C\x01W\x82\x83\xFD[\x83\x01`\x1F\x81\x01\x85\x13a\x0C\x11W\x82\x83\xFD[\x80Qa\x0C\x1Fa\n\x1B\x82a\x11uV[\x81\x81R\x83\x81\x01\x90\x83\x85\x01\x85\x84\x02\x85\x01\x86\x01\x89\x10\x15a\x0C;W\x86\x87\xFD[\x86\x94P[\x83\x85\x10\x15a\x0C]W\x80Q\x83R`\x01\x94\x90\x94\x01\x93\x91\x85\x01\x91\x85\x01a\x0C?V[P\x97\x96PPPPPPPV[`\0\x80` \x83\x85\x03\x12\x15a\x0C{W\x80\x81\xFD[\x825g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a\x0C\x92W\x82\x83\xFD[\x81\x85\x01\x91P\x85`\x1F\x83\x01\x12a\x0C\xA5W\x82\x83\xFD[\x815\x81\x81\x11\x15a\x0C\xB3W\x83\x84\xFD[\x86` `\x80\x83\x02\x85\x01\x01\x11\x15a\x0C\xC7W\x83\x84\xFD[` \x92\x90\x92\x01\x96\x91\x95P\x90\x93PPPPV[`\0\x80`\0\x80`\0\x80`\0\x80a\x01\xA0\x89\x8B\x03\x12\x15a\x0C\xF5W\x83\x84\xFD[\x885`\x02\x81\x10a\r\x03W\x84\x85\xFD[\x97P` \x89\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a\r\x1FW\x85\x86\xFD[a\r+\x8C\x83\x8D\x01a\n\xCAV[\x90\x99P\x97P`@\x8B\x015\x91P\x80\x82\x11\x15a\rCW\x85\x86\xFD[a\rO\x8C\x83\x8D\x01a\t\xFBV[\x96Pa\r^\x8C``\x8D\x01a\x0B9V[\x95P`\xE0\x8B\x015\x91P\x80\x82\x11\x15a\rsW\x84\x85\xFD[Pa\r\x80\x8B\x82\x8C\x01a\npV[\x93PPa\x01\0\x89\x015\x91Pa\r\x99\x8Aa\x01 \x8B\x01a\x0B(V[\x90P\x92\x95\x98P\x92\x95\x98\x90\x93\x96PV[`\0\x81Q\x80\x84R` \x80\x85\x01\x94P\x80\x84\x01\x83[\x83\x81\x10\x15a\r\xEDW\x81Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x87R\x95\x82\x01\x95\x90\x82\x01\x90`\x01\x01a\r\xBBV[P\x94\x95\x94PPPPPV[`\0\x81Q\x80\x84R` \x80\x85\x01\x94P\x80\x84\x01\x83[\x83\x81\x10\x15a\r\xEDW\x81Q\x87R\x95\x82\x01\x95\x90\x82\x01\x90`\x01\x01a\x0E\x0BV[`\0\x82\x84R\x82\x82` \x86\x017\x80` \x84\x86\x01\x01R` \x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`\x1F\x85\x01\x16\x85\x01\x01\x90P\x93\x92PPPV[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82Q\x16\x83R` \x82\x01Q\x15\x15` \x84\x01R\x80`@\x83\x01Q\x16`@\x84\x01RP``\x81\x01Q\x15\x15``\x83\x01RPPV[`\0` \x82Ra\x0B\xD2` \x83\x01\x84a\r\xF8V[` \x80\x82R\x82Q\x82\x82\x01\x81\x90R`\0\x91\x90`@\x90\x81\x85\x01\x90\x86\x84\x01\x85[\x82\x81\x10\x15a\x0FLW\x81Q\x80Q`\x04\x81\x10a\x0E\xF9W\xFE[\x85R\x80\x87\x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x90\x81\x16\x88\x87\x01R\x86\x82\x01Q\x87\x87\x01R``\x80\x83\x01Q\x82\x16\x90\x87\x01R`\x80\x91\x82\x01Q\x16\x90\x85\x01R`\xA0\x90\x93\x01\x92\x90\x85\x01\x90`\x01\x01a\x0E\xE3V[P\x91\x97\x96PPPPPPPV[`\0a\x01 \x80\x83\x01`\x02\x8B\x10a\x0FkW\xFE[\x8A\x84R` \x80\x85\x01\x92\x90\x92R\x88\x90Ra\x01@\x80\x84\x01\x91\x89\x81\x02\x85\x01\x90\x91\x01\x90\x8A\x84[\x8B\x81\x10\x15a\x10\x98W\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFE\xC0\x87\x85\x03\x01\x85R\x815\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFFa\x8E6\x03\x01\x81\x12a\x0F\xEDW\x86\x87\xFD[\x8D\x01\x805\x85R\x83\x81\x015\x84\x86\x01R`@\x80\x82\x015\x90\x86\x01R``\x80\x82\x015\x90\x86\x01R`\xA0`\x80\x80\x83\x0156\x84\x90\x03\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE1\x01\x81\x12a\x10GW\x89\x8A\xFD[\x83\x01\x805g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x10_W\x8A\x8B\xFD[\x806\x03\x85\x13\x15a\x10mW\x8A\x8B\xFD[\x83\x83\x8A\x01Ra\x10\x81\x84\x8A\x01\x82\x8A\x85\x01a\x0E'V[\x99\x88\x01\x99\x98PPP\x93\x85\x01\x93PPP`\x01\x01a\x0F\x8DV[PPP\x83\x81\x03`@\x85\x01Ra\x10\xAD\x81\x89a\r\xA8V[\x91PPa\x10\xBD``\x84\x01\x87a\x0EoV[\x82\x81\x03`\xE0\x84\x01Ra\x10\xCF\x81\x86a\r\xF8V[\x91PP\x82a\x01\0\x83\x01R\x98\x97PPPPPPPPV[` \x80\x82R`\x11\x90\x82\x01R\x7FGPv2: not creator\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`@\x82\x01R``\x01\x90V[` \x80\x82R\x81\x81\x01R\x7FGPv2: cannot transfer native ETH`@\x82\x01R``\x01\x90V[`@Q\x81\x81\x01g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x82\x82\x10\x17\x15a\x11mW\xFE[`@R\x91\x90PV[`\0g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11\x15a\x11\x89W\xFE[P` \x90\x81\x02\x01\x90V[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x16\x81\x14a\x11\xB5W`\0\x80\xFD[PV\xFE\xA2dipfsX\"\x12 6JiA\xBE\xA6\x96 \xB7\xDC:\x95}\n\xB4\xCB\xF3\xBF\xC4Y\xC7\xAD9$\xD2 b\n\xCA\x92\x02\xFCdsolcC\0\x07\x06\x003",
    );
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `Interaction(address,uint256,bytes4)` and selector `0xed99827efb37016f2275f98c4bcf71c7551c75d59e9b450f79fa32e60be672c2`.
```solidity
event Interaction(address indexed target, uint256 value, bytes4 selector);
```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct Interaction {
        #[allow(missing_docs)]
        pub target: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub value: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub selector: alloy_sol_types::private::FixedBytes<4>,
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
        impl alloy_sol_types::SolEvent for Interaction {
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::FixedBytes<4>,
            );
            type DataToken<'a> = <Self::DataTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "Interaction(address,uint256,bytes4)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 = alloy_sol_types::private::B256::new([
                237u8, 153u8, 130u8, 126u8, 251u8, 55u8, 1u8, 111u8, 34u8, 117u8, 249u8,
                140u8, 75u8, 207u8, 113u8, 199u8, 85u8, 28u8, 117u8, 213u8, 158u8, 155u8,
                69u8, 15u8, 121u8, 250u8, 50u8, 230u8, 11u8, 230u8, 114u8, 194u8,
            ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    target: topics.1,
                    value: data.0,
                    selector: data.1,
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
                    > as alloy_sol_types::SolType>::tokenize(&self.value),
                    <alloy_sol_types::sol_data::FixedBytes<
                        4,
                    > as alloy_sol_types::SolType>::tokenize(&self.selector),
                )
            }
            #[inline]
            fn topics(&self) -> <Self::TopicList as alloy_sol_types::SolType>::RustType {
                (Self::SIGNATURE_HASH.into(), self.target.clone())
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
                    &self.target,
                );
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for Interaction {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&Interaction> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &Interaction) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `OrderInvalidated(address,bytes)` and selector `0x875b6cb035bbd4ac6500fabc6d1e4ca5bdc58a3e2b424ccb5c24cdbebeb009a9`.
```solidity
event OrderInvalidated(address indexed owner, bytes orderUid);
```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct OrderInvalidated {
        #[allow(missing_docs)]
        pub owner: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub orderUid: alloy_sol_types::private::Bytes,
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
        impl alloy_sol_types::SolEvent for OrderInvalidated {
            type DataTuple<'a> = (alloy_sol_types::sol_data::Bytes,);
            type DataToken<'a> = <Self::DataTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "OrderInvalidated(address,bytes)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 = alloy_sol_types::private::B256::new([
                135u8, 91u8, 108u8, 176u8, 53u8, 187u8, 212u8, 172u8, 101u8, 0u8, 250u8,
                188u8, 109u8, 30u8, 76u8, 165u8, 189u8, 197u8, 138u8, 62u8, 43u8, 66u8,
                76u8, 203u8, 92u8, 36u8, 205u8, 190u8, 190u8, 176u8, 9u8, 169u8,
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
                    orderUid: data.0,
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
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.orderUid,
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
        impl alloy_sol_types::private::IntoLogData for OrderInvalidated {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&OrderInvalidated> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &OrderInvalidated) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `PreSignature(address,bytes,bool)` and selector `0x01bf7c8b0ca55deecbea89d7e58295b7ffbf685fd0d96801034ba8c6ffe1c68d`.
```solidity
event PreSignature(address indexed owner, bytes orderUid, bool signed);
```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct PreSignature {
        #[allow(missing_docs)]
        pub owner: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub orderUid: alloy_sol_types::private::Bytes,
        #[allow(missing_docs)]
        pub signed: bool,
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
        impl alloy_sol_types::SolEvent for PreSignature {
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::Bytes,
                alloy_sol_types::sol_data::Bool,
            );
            type DataToken<'a> = <Self::DataTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "PreSignature(address,bytes,bool)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 = alloy_sol_types::private::B256::new([
                1u8, 191u8, 124u8, 139u8, 12u8, 165u8, 93u8, 238u8, 203u8, 234u8, 137u8,
                215u8, 229u8, 130u8, 149u8, 183u8, 255u8, 191u8, 104u8, 95u8, 208u8,
                217u8, 104u8, 1u8, 3u8, 75u8, 168u8, 198u8, 255u8, 225u8, 198u8, 141u8,
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
                    orderUid: data.0,
                    signed: data.1,
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
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.orderUid,
                    ),
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::tokenize(
                        &self.signed,
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
        impl alloy_sol_types::private::IntoLogData for PreSignature {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&PreSignature> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &PreSignature) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `Settlement(address)` and selector `0x40338ce1a7c49204f0099533b1e9a7ee0a3d261f84974ab7af36105b8c4e9db4`.
```solidity
event Settlement(address indexed solver);
```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct Settlement {
        #[allow(missing_docs)]
        pub solver: alloy_sol_types::private::Address,
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
        impl alloy_sol_types::SolEvent for Settlement {
            type DataTuple<'a> = ();
            type DataToken<'a> = <Self::DataTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "Settlement(address)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 = alloy_sol_types::private::B256::new([
                64u8, 51u8, 140u8, 225u8, 167u8, 196u8, 146u8, 4u8, 240u8, 9u8, 149u8,
                51u8, 177u8, 233u8, 167u8, 238u8, 10u8, 61u8, 38u8, 31u8, 132u8, 151u8,
                74u8, 183u8, 175u8, 54u8, 16u8, 91u8, 140u8, 78u8, 157u8, 180u8,
            ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self { solver: topics.1 }
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
                (Self::SIGNATURE_HASH.into(), self.solver.clone())
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
                    &self.solver,
                );
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for Settlement {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&Settlement> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &Settlement) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `Trade(address,address,address,uint256,uint256,uint256,bytes)` and selector `0xa07a543ab8a018198e99ca0184c93fe9050a79400a0a723441f84de1d972cc17`.
```solidity
event Trade(address indexed owner, address sellToken, address buyToken, uint256 sellAmount, uint256 buyAmount, uint256 feeAmount, bytes orderUid);
```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct Trade {
        #[allow(missing_docs)]
        pub owner: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub sellToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub buyToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub sellAmount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub buyAmount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub feeAmount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub orderUid: alloy_sol_types::private::Bytes,
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
        impl alloy_sol_types::SolEvent for Trade {
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Bytes,
            );
            type DataToken<'a> = <Self::DataTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "Trade(address,address,address,uint256,uint256,uint256,bytes)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 = alloy_sol_types::private::B256::new([
                160u8, 122u8, 84u8, 58u8, 184u8, 160u8, 24u8, 25u8, 142u8, 153u8, 202u8,
                1u8, 132u8, 201u8, 63u8, 233u8, 5u8, 10u8, 121u8, 64u8, 10u8, 10u8,
                114u8, 52u8, 65u8, 248u8, 77u8, 225u8, 217u8, 114u8, 204u8, 23u8,
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
                    sellToken: data.0,
                    buyToken: data.1,
                    sellAmount: data.2,
                    buyAmount: data.3,
                    feeAmount: data.4,
                    orderUid: data.5,
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
                        &self.sellToken,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.buyToken,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.sellAmount),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.buyAmount),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.feeAmount),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.orderUid,
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
        impl alloy_sol_types::private::IntoLogData for Trade {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&Trade> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &Trade) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    /**Constructor`.
```solidity
constructor(address authenticator_, address vault_);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct constructorCall {
        #[allow(missing_docs)]
        pub authenticator_: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub vault_: alloy_sol_types::private::Address,
    }
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
            impl ::core::convert::From<constructorCall> for UnderlyingRustTuple<'_> {
                fn from(value: constructorCall) -> Self {
                    (value.authenticator_, value.vault_)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for constructorCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        authenticator_: tuple.0,
                        vault_: tuple.1,
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
                        &self.authenticator_,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.vault_,
                    ),
                )
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `authenticator()` and selector `0x2335c76b`.
```solidity
function authenticator() external view returns (address);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct authenticatorCall;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`authenticator()`](authenticatorCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct authenticatorReturn {
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
            impl ::core::convert::From<authenticatorCall> for UnderlyingRustTuple<'_> {
                fn from(value: authenticatorCall) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for authenticatorCall {
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
            impl ::core::convert::From<authenticatorReturn> for UnderlyingRustTuple<'_> {
                fn from(value: authenticatorReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for authenticatorReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for authenticatorCall {
            type Parameters<'a> = ();
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::Address;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "authenticator()";
            const SELECTOR: [u8; 4] = [35u8, 53u8, 199u8, 107u8];
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
                        let r: authenticatorReturn = r.into();
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
                        let r: authenticatorReturn = r.into();
                        r._0
                    })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `domainSeparator()` and selector `0xf698da25`.
```solidity
function domainSeparator() external view returns (bytes32);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct domainSeparatorCall;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`domainSeparator()`](domainSeparatorCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct domainSeparatorReturn {
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
            impl ::core::convert::From<domainSeparatorCall> for UnderlyingRustTuple<'_> {
                fn from(value: domainSeparatorCall) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for domainSeparatorCall {
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
            impl ::core::convert::From<domainSeparatorReturn>
            for UnderlyingRustTuple<'_> {
                fn from(value: domainSeparatorReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for domainSeparatorReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for domainSeparatorCall {
            type Parameters<'a> = ();
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::FixedBytes<32>;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::FixedBytes<32>,);
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "domainSeparator()";
            const SELECTOR: [u8; 4] = [246u8, 152u8, 218u8, 37u8];
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
                        let r: domainSeparatorReturn = r.into();
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
                        let r: domainSeparatorReturn = r.into();
                        r._0
                    })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `filledAmount(bytes)` and selector `0x2479fb6e`.
```solidity
function filledAmount(bytes memory) external view returns (uint256);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct filledAmountCall(pub alloy_sol_types::private::Bytes);
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`filledAmount(bytes)`](filledAmountCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct filledAmountReturn {
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
            type UnderlyingSolTuple<'a> = (alloy_sol_types::sol_data::Bytes,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (alloy_sol_types::private::Bytes,);
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
            impl ::core::convert::From<filledAmountCall> for UnderlyingRustTuple<'_> {
                fn from(value: filledAmountCall) -> Self {
                    (value.0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for filledAmountCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self(tuple.0)
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
            impl ::core::convert::From<filledAmountReturn> for UnderlyingRustTuple<'_> {
                fn from(value: filledAmountReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for filledAmountReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for filledAmountCall {
            type Parameters<'a> = (alloy_sol_types::sol_data::Bytes,);
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::primitives::aliases::U256;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Uint<256>,);
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "filledAmount(bytes)";
            const SELECTOR: [u8; 4] = [36u8, 121u8, 251u8, 110u8];
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
                        &self.0,
                    ),
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
                        let r: filledAmountReturn = r.into();
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
                        let r: filledAmountReturn = r.into();
                        r._0
                    })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `invalidateOrder(bytes)` and selector `0x15337bc0`.
```solidity
function invalidateOrder(bytes memory orderUid) external;
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct invalidateOrderCall {
        #[allow(missing_docs)]
        pub orderUid: alloy_sol_types::private::Bytes,
    }
    ///Container type for the return parameters of the [`invalidateOrder(bytes)`](invalidateOrderCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct invalidateOrderReturn {}
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
            type UnderlyingSolTuple<'a> = (alloy_sol_types::sol_data::Bytes,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (alloy_sol_types::private::Bytes,);
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
            impl ::core::convert::From<invalidateOrderCall> for UnderlyingRustTuple<'_> {
                fn from(value: invalidateOrderCall) -> Self {
                    (value.orderUid,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for invalidateOrderCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { orderUid: tuple.0 }
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
            impl ::core::convert::From<invalidateOrderReturn>
            for UnderlyingRustTuple<'_> {
                fn from(value: invalidateOrderReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for invalidateOrderReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl invalidateOrderReturn {
            fn _tokenize(
                &self,
            ) -> <invalidateOrderCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for invalidateOrderCall {
            type Parameters<'a> = (alloy_sol_types::sol_data::Bytes,);
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = invalidateOrderReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "invalidateOrder(bytes)";
            const SELECTOR: [u8; 4] = [21u8, 51u8, 123u8, 192u8];
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
                        &self.orderUid,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                invalidateOrderReturn::_tokenize(ret)
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
    /**Function with signature `setPreSignature(bytes,bool)` and selector `0xec6cb13f`.
```solidity
function setPreSignature(bytes memory orderUid, bool signed) external;
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct setPreSignatureCall {
        #[allow(missing_docs)]
        pub orderUid: alloy_sol_types::private::Bytes,
        #[allow(missing_docs)]
        pub signed: bool,
    }
    ///Container type for the return parameters of the [`setPreSignature(bytes,bool)`](setPreSignatureCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct setPreSignatureReturn {}
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
                alloy_sol_types::sol_data::Bytes,
                alloy_sol_types::sol_data::Bool,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (alloy_sol_types::private::Bytes, bool);
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
            impl ::core::convert::From<setPreSignatureCall> for UnderlyingRustTuple<'_> {
                fn from(value: setPreSignatureCall) -> Self {
                    (value.orderUid, value.signed)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for setPreSignatureCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        orderUid: tuple.0,
                        signed: tuple.1,
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
            impl ::core::convert::From<setPreSignatureReturn>
            for UnderlyingRustTuple<'_> {
                fn from(value: setPreSignatureReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for setPreSignatureReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl setPreSignatureReturn {
            fn _tokenize(
                &self,
            ) -> <setPreSignatureCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for setPreSignatureCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Bytes,
                alloy_sol_types::sol_data::Bool,
            );
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = setPreSignatureReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "setPreSignature(bytes,bool)";
            const SELECTOR: [u8; 4] = [236u8, 108u8, 177u8, 63u8];
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
                        &self.orderUid,
                    ),
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::tokenize(
                        &self.signed,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                setPreSignatureReturn::_tokenize(ret)
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
    /**Function with signature `settle(address[],uint256[],(uint256,uint256,address,uint256,uint256,uint32,bytes32,uint256,uint256,uint256,bytes)[],(address,uint256,bytes)[][3])` and selector `0x13d79a0b`.
```solidity
function settle(address[] memory tokens, uint256[] memory clearingPrices, GPv2Trade.Data[] memory trades, GPv2Interaction.Data[][3] memory interactions) external;
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct settleCall {
        #[allow(missing_docs)]
        pub tokens: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
        #[allow(missing_docs)]
        pub clearingPrices: alloy_sol_types::private::Vec<
            alloy_sol_types::private::primitives::aliases::U256,
        >,
        #[allow(missing_docs)]
        pub trades: alloy_sol_types::private::Vec<
            <GPv2Trade::Data as alloy_sol_types::SolType>::RustType,
        >,
        #[allow(missing_docs)]
        pub interactions: [alloy_sol_types::private::Vec<
            <GPv2Interaction::Data as alloy_sol_types::SolType>::RustType,
        >; 3usize],
    }
    ///Container type for the return parameters of the [`settle(address[],uint256[],(uint256,uint256,address,uint256,uint256,uint32,bytes32,uint256,uint256,uint256,bytes)[],(address,uint256,bytes)[][3])`](settleCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct settleReturn {}
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
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
                alloy_sol_types::sol_data::Array<GPv2Trade::Data>,
                alloy_sol_types::sol_data::FixedArray<
                    alloy_sol_types::sol_data::Array<GPv2Interaction::Data>,
                    3usize,
                >,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
                alloy_sol_types::private::Vec<
                    alloy_sol_types::private::primitives::aliases::U256,
                >,
                alloy_sol_types::private::Vec<
                    <GPv2Trade::Data as alloy_sol_types::SolType>::RustType,
                >,
                [alloy_sol_types::private::Vec<
                    <GPv2Interaction::Data as alloy_sol_types::SolType>::RustType,
                >; 3usize],
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
            impl ::core::convert::From<settleCall> for UnderlyingRustTuple<'_> {
                fn from(value: settleCall) -> Self {
                    (
                        value.tokens,
                        value.clearingPrices,
                        value.trades,
                        value.interactions,
                    )
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for settleCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        tokens: tuple.0,
                        clearingPrices: tuple.1,
                        trades: tuple.2,
                        interactions: tuple.3,
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
            impl ::core::convert::From<settleReturn> for UnderlyingRustTuple<'_> {
                fn from(value: settleReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for settleReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl settleReturn {
            fn _tokenize(
                &self,
            ) -> <settleCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for settleCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
                alloy_sol_types::sol_data::Array<GPv2Trade::Data>,
                alloy_sol_types::sol_data::FixedArray<
                    alloy_sol_types::sol_data::Array<GPv2Interaction::Data>,
                    3usize,
                >,
            );
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = settleReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "settle(address[],uint256[],(uint256,uint256,address,uint256,uint256,uint32,bytes32,uint256,uint256,uint256,bytes)[],(address,uint256,bytes)[][3])";
            const SELECTOR: [u8; 4] = [19u8, 215u8, 154u8, 11u8];
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
                        alloy_sol_types::sol_data::Address,
                    > as alloy_sol_types::SolType>::tokenize(&self.tokens),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::SolType>::tokenize(&self.clearingPrices),
                    <alloy_sol_types::sol_data::Array<
                        GPv2Trade::Data,
                    > as alloy_sol_types::SolType>::tokenize(&self.trades),
                    <alloy_sol_types::sol_data::FixedArray<
                        alloy_sol_types::sol_data::Array<GPv2Interaction::Data>,
                        3usize,
                    > as alloy_sol_types::SolType>::tokenize(&self.interactions),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                settleReturn::_tokenize(ret)
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
    /**Function with signature `simulateDelegatecall(address,bytes)` and selector `0xf84436bd`.
```solidity
function simulateDelegatecall(address targetContract, bytes memory calldataPayload) external returns (bytes memory response);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct simulateDelegatecallCall {
        #[allow(missing_docs)]
        pub targetContract: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub calldataPayload: alloy_sol_types::private::Bytes,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`simulateDelegatecall(address,bytes)`](simulateDelegatecallCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct simulateDelegatecallReturn {
        #[allow(missing_docs)]
        pub response: alloy_sol_types::private::Bytes,
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
                alloy_sol_types::sol_data::Bytes,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
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
            impl ::core::convert::From<simulateDelegatecallCall>
            for UnderlyingRustTuple<'_> {
                fn from(value: simulateDelegatecallCall) -> Self {
                    (value.targetContract, value.calldataPayload)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for simulateDelegatecallCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        targetContract: tuple.0,
                        calldataPayload: tuple.1,
                    }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (alloy_sol_types::sol_data::Bytes,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (alloy_sol_types::private::Bytes,);
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
            impl ::core::convert::From<simulateDelegatecallReturn>
            for UnderlyingRustTuple<'_> {
                fn from(value: simulateDelegatecallReturn) -> Self {
                    (value.response,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for simulateDelegatecallReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { response: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for simulateDelegatecallCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Bytes,
            );
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::Bytes;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Bytes,);
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "simulateDelegatecall(address,bytes)";
            const SELECTOR: [u8; 4] = [248u8, 68u8, 54u8, 189u8];
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
                        &self.targetContract,
                    ),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.calldataPayload,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                (
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
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
                        let r: simulateDelegatecallReturn = r.into();
                        r.response
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
                        let r: simulateDelegatecallReturn = r.into();
                        r.response
                    })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `swap((bytes32,uint256,uint256,uint256,bytes)[],address[],(uint256,uint256,address,uint256,uint256,uint32,bytes32,uint256,uint256,uint256,bytes))` and selector `0x845a101f`.
```solidity
function swap(IVault.BatchSwapStep[] memory swaps, address[] memory tokens, GPv2Trade.Data memory trade) external;
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct swapCall {
        #[allow(missing_docs)]
        pub swaps: alloy_sol_types::private::Vec<
            <IVault::BatchSwapStep as alloy_sol_types::SolType>::RustType,
        >,
        #[allow(missing_docs)]
        pub tokens: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
        #[allow(missing_docs)]
        pub trade: <GPv2Trade::Data as alloy_sol_types::SolType>::RustType,
    }
    ///Container type for the return parameters of the [`swap((bytes32,uint256,uint256,uint256,bytes)[],address[],(uint256,uint256,address,uint256,uint256,uint32,bytes32,uint256,uint256,uint256,bytes))`](swapCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct swapReturn {}
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
                alloy_sol_types::sol_data::Array<IVault::BatchSwapStep>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                GPv2Trade::Data,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Vec<
                    <IVault::BatchSwapStep as alloy_sol_types::SolType>::RustType,
                >,
                alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
                <GPv2Trade::Data as alloy_sol_types::SolType>::RustType,
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
                    (value.swaps, value.tokens, value.trade)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for swapCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        swaps: tuple.0,
                        tokens: tuple.1,
                        trade: tuple.2,
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
            impl ::core::convert::From<swapReturn> for UnderlyingRustTuple<'_> {
                fn from(value: swapReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for swapReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl swapReturn {
            fn _tokenize(
                &self,
            ) -> <swapCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for swapCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Array<IVault::BatchSwapStep>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                GPv2Trade::Data,
            );
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = swapReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "swap((bytes32,uint256,uint256,uint256,bytes)[],address[],(uint256,uint256,address,uint256,uint256,uint32,bytes32,uint256,uint256,uint256,bytes))";
            const SELECTOR: [u8; 4] = [132u8, 90u8, 16u8, 31u8];
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
                        IVault::BatchSwapStep,
                    > as alloy_sol_types::SolType>::tokenize(&self.swaps),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Address,
                    > as alloy_sol_types::SolType>::tokenize(&self.tokens),
                    <GPv2Trade::Data as alloy_sol_types::SolType>::tokenize(&self.trade),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                swapReturn::_tokenize(ret)
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
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::Address;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
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
                        let r: vaultReturn = r.into();
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
                        let r: vaultReturn = r.into();
                        r._0
                    })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `vaultRelayer()` and selector `0x9b552cc2`.
```solidity
function vaultRelayer() external view returns (address);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct vaultRelayerCall;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`vaultRelayer()`](vaultRelayerCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct vaultRelayerReturn {
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
            impl ::core::convert::From<vaultRelayerCall> for UnderlyingRustTuple<'_> {
                fn from(value: vaultRelayerCall) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for vaultRelayerCall {
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
            impl ::core::convert::From<vaultRelayerReturn> for UnderlyingRustTuple<'_> {
                fn from(value: vaultRelayerReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for vaultRelayerReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for vaultRelayerCall {
            type Parameters<'a> = ();
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::Address;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "vaultRelayer()";
            const SELECTOR: [u8; 4] = [155u8, 85u8, 44u8, 194u8];
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
                        let r: vaultRelayerReturn = r.into();
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
                        let r: vaultRelayerReturn = r.into();
                        r._0
                    })
            }
        }
    };
    ///Container for all the [`GPv2Settlement`](self) function calls.
    #[derive(Clone)]
    #[derive()]
    pub enum GPv2SettlementCalls {
        #[allow(missing_docs)]
        authenticator(authenticatorCall),
        #[allow(missing_docs)]
        domainSeparator(domainSeparatorCall),
        #[allow(missing_docs)]
        filledAmount(filledAmountCall),
        #[allow(missing_docs)]
        invalidateOrder(invalidateOrderCall),
        #[allow(missing_docs)]
        setPreSignature(setPreSignatureCall),
        #[allow(missing_docs)]
        settle(settleCall),
        #[allow(missing_docs)]
        simulateDelegatecall(simulateDelegatecallCall),
        #[allow(missing_docs)]
        swap(swapCall),
        #[allow(missing_docs)]
        vault(vaultCall),
        #[allow(missing_docs)]
        vaultRelayer(vaultRelayerCall),
    }
    impl GPv2SettlementCalls {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the variants.
        /// No guarantees are made about the order of the selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 4usize]] = &[
            [19u8, 215u8, 154u8, 11u8],
            [21u8, 51u8, 123u8, 192u8],
            [35u8, 53u8, 199u8, 107u8],
            [36u8, 121u8, 251u8, 110u8],
            [132u8, 90u8, 16u8, 31u8],
            [155u8, 85u8, 44u8, 194u8],
            [236u8, 108u8, 177u8, 63u8],
            [246u8, 152u8, 218u8, 37u8],
            [248u8, 68u8, 54u8, 189u8],
            [251u8, 250u8, 119u8, 207u8],
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(settle),
            ::core::stringify!(invalidateOrder),
            ::core::stringify!(authenticator),
            ::core::stringify!(filledAmount),
            ::core::stringify!(swap),
            ::core::stringify!(vaultRelayer),
            ::core::stringify!(setPreSignature),
            ::core::stringify!(domainSeparator),
            ::core::stringify!(simulateDelegatecall),
            ::core::stringify!(vault),
        ];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <settleCall as alloy_sol_types::SolCall>::SIGNATURE,
            <invalidateOrderCall as alloy_sol_types::SolCall>::SIGNATURE,
            <authenticatorCall as alloy_sol_types::SolCall>::SIGNATURE,
            <filledAmountCall as alloy_sol_types::SolCall>::SIGNATURE,
            <swapCall as alloy_sol_types::SolCall>::SIGNATURE,
            <vaultRelayerCall as alloy_sol_types::SolCall>::SIGNATURE,
            <setPreSignatureCall as alloy_sol_types::SolCall>::SIGNATURE,
            <domainSeparatorCall as alloy_sol_types::SolCall>::SIGNATURE,
            <simulateDelegatecallCall as alloy_sol_types::SolCall>::SIGNATURE,
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
        pub fn name_by_selector(
            selector: [u8; 4usize],
        ) -> ::core::option::Option<&'static str> {
            let sig = Self::signature_by_selector(selector)?;
            sig.split_once('(').map(|(name, _)| name)
        }
    }
    #[automatically_derived]
    impl alloy_sol_types::SolInterface for GPv2SettlementCalls {
        const NAME: &'static str = "GPv2SettlementCalls";
        const MIN_DATA_LENGTH: usize = 0usize;
        const COUNT: usize = 10usize;
        #[inline]
        fn selector(&self) -> [u8; 4] {
            match self {
                Self::authenticator(_) => {
                    <authenticatorCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::domainSeparator(_) => {
                    <domainSeparatorCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::filledAmount(_) => {
                    <filledAmountCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::invalidateOrder(_) => {
                    <invalidateOrderCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::setPreSignature(_) => {
                    <setPreSignatureCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::settle(_) => <settleCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::simulateDelegatecall(_) => {
                    <simulateDelegatecallCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::swap(_) => <swapCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::vault(_) => <vaultCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::vaultRelayer(_) => {
                    <vaultRelayerCall as alloy_sol_types::SolCall>::SELECTOR
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
            ) -> alloy_sol_types::Result<GPv2SettlementCalls>] = &[
                {
                    fn settle(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GPv2SettlementCalls> {
                        <settleCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GPv2SettlementCalls::settle)
                    }
                    settle
                },
                {
                    fn invalidateOrder(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GPv2SettlementCalls> {
                        <invalidateOrderCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(GPv2SettlementCalls::invalidateOrder)
                    }
                    invalidateOrder
                },
                {
                    fn authenticator(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GPv2SettlementCalls> {
                        <authenticatorCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(GPv2SettlementCalls::authenticator)
                    }
                    authenticator
                },
                {
                    fn filledAmount(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GPv2SettlementCalls> {
                        <filledAmountCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(GPv2SettlementCalls::filledAmount)
                    }
                    filledAmount
                },
                {
                    fn swap(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GPv2SettlementCalls> {
                        <swapCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GPv2SettlementCalls::swap)
                    }
                    swap
                },
                {
                    fn vaultRelayer(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GPv2SettlementCalls> {
                        <vaultRelayerCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(GPv2SettlementCalls::vaultRelayer)
                    }
                    vaultRelayer
                },
                {
                    fn setPreSignature(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GPv2SettlementCalls> {
                        <setPreSignatureCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(GPv2SettlementCalls::setPreSignature)
                    }
                    setPreSignature
                },
                {
                    fn domainSeparator(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GPv2SettlementCalls> {
                        <domainSeparatorCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(GPv2SettlementCalls::domainSeparator)
                    }
                    domainSeparator
                },
                {
                    fn simulateDelegatecall(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GPv2SettlementCalls> {
                        <simulateDelegatecallCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(GPv2SettlementCalls::simulateDelegatecall)
                    }
                    simulateDelegatecall
                },
                {
                    fn vault(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GPv2SettlementCalls> {
                        <vaultCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GPv2SettlementCalls::vault)
                    }
                    vault
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
            ) -> alloy_sol_types::Result<GPv2SettlementCalls>] = &[
                {
                    fn settle(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GPv2SettlementCalls> {
                        <settleCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(GPv2SettlementCalls::settle)
                    }
                    settle
                },
                {
                    fn invalidateOrder(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GPv2SettlementCalls> {
                        <invalidateOrderCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(GPv2SettlementCalls::invalidateOrder)
                    }
                    invalidateOrder
                },
                {
                    fn authenticator(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GPv2SettlementCalls> {
                        <authenticatorCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(GPv2SettlementCalls::authenticator)
                    }
                    authenticator
                },
                {
                    fn filledAmount(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GPv2SettlementCalls> {
                        <filledAmountCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(GPv2SettlementCalls::filledAmount)
                    }
                    filledAmount
                },
                {
                    fn swap(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GPv2SettlementCalls> {
                        <swapCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(GPv2SettlementCalls::swap)
                    }
                    swap
                },
                {
                    fn vaultRelayer(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GPv2SettlementCalls> {
                        <vaultRelayerCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(GPv2SettlementCalls::vaultRelayer)
                    }
                    vaultRelayer
                },
                {
                    fn setPreSignature(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GPv2SettlementCalls> {
                        <setPreSignatureCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(GPv2SettlementCalls::setPreSignature)
                    }
                    setPreSignature
                },
                {
                    fn domainSeparator(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GPv2SettlementCalls> {
                        <domainSeparatorCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(GPv2SettlementCalls::domainSeparator)
                    }
                    domainSeparator
                },
                {
                    fn simulateDelegatecall(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GPv2SettlementCalls> {
                        <simulateDelegatecallCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(GPv2SettlementCalls::simulateDelegatecall)
                    }
                    simulateDelegatecall
                },
                {
                    fn vault(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GPv2SettlementCalls> {
                        <vaultCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(GPv2SettlementCalls::vault)
                    }
                    vault
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
                Self::authenticator(inner) => {
                    <authenticatorCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::domainSeparator(inner) => {
                    <domainSeparatorCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::filledAmount(inner) => {
                    <filledAmountCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::invalidateOrder(inner) => {
                    <invalidateOrderCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::setPreSignature(inner) => {
                    <setPreSignatureCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::settle(inner) => {
                    <settleCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::simulateDelegatecall(inner) => {
                    <simulateDelegatecallCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::swap(inner) => {
                    <swapCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::vault(inner) => {
                    <vaultCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::vaultRelayer(inner) => {
                    <vaultRelayerCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
            }
        }
        #[inline]
        fn abi_encode_raw(&self, out: &mut alloy_sol_types::private::Vec<u8>) {
            match self {
                Self::authenticator(inner) => {
                    <authenticatorCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::domainSeparator(inner) => {
                    <domainSeparatorCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::filledAmount(inner) => {
                    <filledAmountCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::invalidateOrder(inner) => {
                    <invalidateOrderCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::setPreSignature(inner) => {
                    <setPreSignatureCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::settle(inner) => {
                    <settleCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::simulateDelegatecall(inner) => {
                    <simulateDelegatecallCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::swap(inner) => {
                    <swapCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::vault(inner) => {
                    <vaultCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::vaultRelayer(inner) => {
                    <vaultRelayerCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
            }
        }
    }
    ///Container for all the [`GPv2Settlement`](self) events.
    #[derive(Clone)]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub enum GPv2SettlementEvents {
        #[allow(missing_docs)]
        Interaction(Interaction),
        #[allow(missing_docs)]
        OrderInvalidated(OrderInvalidated),
        #[allow(missing_docs)]
        PreSignature(PreSignature),
        #[allow(missing_docs)]
        Settlement(Settlement),
        #[allow(missing_docs)]
        Trade(Trade),
    }
    impl GPv2SettlementEvents {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the variants.
        /// No guarantees are made about the order of the selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 32usize]] = &[
            [
                1u8, 191u8, 124u8, 139u8, 12u8, 165u8, 93u8, 238u8, 203u8, 234u8, 137u8,
                215u8, 229u8, 130u8, 149u8, 183u8, 255u8, 191u8, 104u8, 95u8, 208u8,
                217u8, 104u8, 1u8, 3u8, 75u8, 168u8, 198u8, 255u8, 225u8, 198u8, 141u8,
            ],
            [
                64u8, 51u8, 140u8, 225u8, 167u8, 196u8, 146u8, 4u8, 240u8, 9u8, 149u8,
                51u8, 177u8, 233u8, 167u8, 238u8, 10u8, 61u8, 38u8, 31u8, 132u8, 151u8,
                74u8, 183u8, 175u8, 54u8, 16u8, 91u8, 140u8, 78u8, 157u8, 180u8,
            ],
            [
                135u8, 91u8, 108u8, 176u8, 53u8, 187u8, 212u8, 172u8, 101u8, 0u8, 250u8,
                188u8, 109u8, 30u8, 76u8, 165u8, 189u8, 197u8, 138u8, 62u8, 43u8, 66u8,
                76u8, 203u8, 92u8, 36u8, 205u8, 190u8, 190u8, 176u8, 9u8, 169u8,
            ],
            [
                160u8, 122u8, 84u8, 58u8, 184u8, 160u8, 24u8, 25u8, 142u8, 153u8, 202u8,
                1u8, 132u8, 201u8, 63u8, 233u8, 5u8, 10u8, 121u8, 64u8, 10u8, 10u8,
                114u8, 52u8, 65u8, 248u8, 77u8, 225u8, 217u8, 114u8, 204u8, 23u8,
            ],
            [
                237u8, 153u8, 130u8, 126u8, 251u8, 55u8, 1u8, 111u8, 34u8, 117u8, 249u8,
                140u8, 75u8, 207u8, 113u8, 199u8, 85u8, 28u8, 117u8, 213u8, 158u8, 155u8,
                69u8, 15u8, 121u8, 250u8, 50u8, 230u8, 11u8, 230u8, 114u8, 194u8,
            ],
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(PreSignature),
            ::core::stringify!(Settlement),
            ::core::stringify!(OrderInvalidated),
            ::core::stringify!(Trade),
            ::core::stringify!(Interaction),
        ];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <PreSignature as alloy_sol_types::SolEvent>::SIGNATURE,
            <Settlement as alloy_sol_types::SolEvent>::SIGNATURE,
            <OrderInvalidated as alloy_sol_types::SolEvent>::SIGNATURE,
            <Trade as alloy_sol_types::SolEvent>::SIGNATURE,
            <Interaction as alloy_sol_types::SolEvent>::SIGNATURE,
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
    impl alloy_sol_types::SolEventInterface for GPv2SettlementEvents {
        const NAME: &'static str = "GPv2SettlementEvents";
        const COUNT: usize = 5usize;
        fn decode_raw_log(
            topics: &[alloy_sol_types::Word],
            data: &[u8],
        ) -> alloy_sol_types::Result<Self> {
            match topics.first().copied() {
                Some(<Interaction as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <Interaction as alloy_sol_types::SolEvent>::decode_raw_log(
                            topics,
                            data,
                        )
                        .map(Self::Interaction)
                }
                Some(<OrderInvalidated as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <OrderInvalidated as alloy_sol_types::SolEvent>::decode_raw_log(
                            topics,
                            data,
                        )
                        .map(Self::OrderInvalidated)
                }
                Some(<PreSignature as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <PreSignature as alloy_sol_types::SolEvent>::decode_raw_log(
                            topics,
                            data,
                        )
                        .map(Self::PreSignature)
                }
                Some(<Settlement as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <Settlement as alloy_sol_types::SolEvent>::decode_raw_log(
                            topics,
                            data,
                        )
                        .map(Self::Settlement)
                }
                Some(<Trade as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <Trade as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::Trade)
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
    impl alloy_sol_types::private::IntoLogData for GPv2SettlementEvents {
        fn to_log_data(&self) -> alloy_sol_types::private::LogData {
            match self {
                Self::Interaction(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::OrderInvalidated(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::PreSignature(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::Settlement(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::Trade(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
            }
        }
        fn into_log_data(self) -> alloy_sol_types::private::LogData {
            match self {
                Self::Interaction(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::OrderInvalidated(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::PreSignature(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::Settlement(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::Trade(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
            }
        }
    }
    use alloy_contract as alloy_contract;
    /**Creates a new wrapper around an on-chain [`GPv2Settlement`](self) contract instance.

See the [wrapper's documentation](`GPv2SettlementInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> GPv2SettlementInstance<P, N> {
        GPv2SettlementInstance::<P, N>::new(address, __provider)
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
        authenticator_: alloy_sol_types::private::Address,
        vault_: alloy_sol_types::private::Address,
    ) -> impl ::core::future::Future<
        Output = alloy_contract::Result<GPv2SettlementInstance<P, N>>,
    > {
        GPv2SettlementInstance::<P, N>::deploy(__provider, authenticator_, vault_)
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
        authenticator_: alloy_sol_types::private::Address,
        vault_: alloy_sol_types::private::Address,
    ) -> alloy_contract::RawCallBuilder<P, N> {
        GPv2SettlementInstance::<
            P,
            N,
        >::deploy_builder(__provider, authenticator_, vault_)
    }
    /**A [`GPv2Settlement`](self) instance.

Contains type-safe methods for interacting with an on-chain instance of the
[`GPv2Settlement`](self) contract located at a given `address`, using a given
provider `P`.

If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
documentation on how to provide it), the `deploy` and `deploy_builder` methods can
be used to deploy a new instance of the contract.

See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct GPv2SettlementInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for GPv2SettlementInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("GPv2SettlementInstance").field(&self.address).finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    > GPv2SettlementInstance<P, N> {
        /**Creates a new wrapper around an on-chain [`GPv2Settlement`](self) contract instance.

See the [wrapper's documentation](`GPv2SettlementInstance`) for more details.*/
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
            authenticator_: alloy_sol_types::private::Address,
            vault_: alloy_sol_types::private::Address,
        ) -> alloy_contract::Result<GPv2SettlementInstance<P, N>> {
            let call_builder = Self::deploy_builder(__provider, authenticator_, vault_);
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
            authenticator_: alloy_sol_types::private::Address,
            vault_: alloy_sol_types::private::Address,
        ) -> alloy_contract::RawCallBuilder<P, N> {
            alloy_contract::RawCallBuilder::new_raw_deploy(
                __provider,
                [
                    &BYTECODE[..],
                    &alloy_sol_types::SolConstructor::abi_encode(
                        &constructorCall {
                            authenticator_,
                            vault_,
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
    impl<P: ::core::clone::Clone, N> GPv2SettlementInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned provider.
        #[inline]
        pub fn with_cloned_provider(self) -> GPv2SettlementInstance<P, N> {
            GPv2SettlementInstance {
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
    > GPv2SettlementInstance<P, N> {
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
        ///Creates a new call builder for the [`authenticator`] function.
        pub fn authenticator(
            &self,
        ) -> alloy_contract::SolCallBuilder<&P, authenticatorCall, N> {
            self.call_builder(&authenticatorCall)
        }
        ///Creates a new call builder for the [`domainSeparator`] function.
        pub fn domainSeparator(
            &self,
        ) -> alloy_contract::SolCallBuilder<&P, domainSeparatorCall, N> {
            self.call_builder(&domainSeparatorCall)
        }
        ///Creates a new call builder for the [`filledAmount`] function.
        pub fn filledAmount(
            &self,
            _0: alloy_sol_types::private::Bytes,
        ) -> alloy_contract::SolCallBuilder<&P, filledAmountCall, N> {
            self.call_builder(&filledAmountCall(_0))
        }
        ///Creates a new call builder for the [`invalidateOrder`] function.
        pub fn invalidateOrder(
            &self,
            orderUid: alloy_sol_types::private::Bytes,
        ) -> alloy_contract::SolCallBuilder<&P, invalidateOrderCall, N> {
            self.call_builder(&invalidateOrderCall { orderUid })
        }
        ///Creates a new call builder for the [`setPreSignature`] function.
        pub fn setPreSignature(
            &self,
            orderUid: alloy_sol_types::private::Bytes,
            signed: bool,
        ) -> alloy_contract::SolCallBuilder<&P, setPreSignatureCall, N> {
            self.call_builder(
                &setPreSignatureCall {
                    orderUid,
                    signed,
                },
            )
        }
        ///Creates a new call builder for the [`settle`] function.
        pub fn settle(
            &self,
            tokens: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
            clearingPrices: alloy_sol_types::private::Vec<
                alloy_sol_types::private::primitives::aliases::U256,
            >,
            trades: alloy_sol_types::private::Vec<
                <GPv2Trade::Data as alloy_sol_types::SolType>::RustType,
            >,
            interactions: [alloy_sol_types::private::Vec<
                <GPv2Interaction::Data as alloy_sol_types::SolType>::RustType,
            >; 3usize],
        ) -> alloy_contract::SolCallBuilder<&P, settleCall, N> {
            self.call_builder(
                &settleCall {
                    tokens,
                    clearingPrices,
                    trades,
                    interactions,
                },
            )
        }
        ///Creates a new call builder for the [`simulateDelegatecall`] function.
        pub fn simulateDelegatecall(
            &self,
            targetContract: alloy_sol_types::private::Address,
            calldataPayload: alloy_sol_types::private::Bytes,
        ) -> alloy_contract::SolCallBuilder<&P, simulateDelegatecallCall, N> {
            self.call_builder(
                &simulateDelegatecallCall {
                    targetContract,
                    calldataPayload,
                },
            )
        }
        ///Creates a new call builder for the [`swap`] function.
        pub fn swap(
            &self,
            swaps: alloy_sol_types::private::Vec<
                <IVault::BatchSwapStep as alloy_sol_types::SolType>::RustType,
            >,
            tokens: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
            trade: <GPv2Trade::Data as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, swapCall, N> {
            self.call_builder(&swapCall { swaps, tokens, trade })
        }
        ///Creates a new call builder for the [`vault`] function.
        pub fn vault(&self) -> alloy_contract::SolCallBuilder<&P, vaultCall, N> {
            self.call_builder(&vaultCall)
        }
        ///Creates a new call builder for the [`vaultRelayer`] function.
        pub fn vaultRelayer(
            &self,
        ) -> alloy_contract::SolCallBuilder<&P, vaultRelayerCall, N> {
            self.call_builder(&vaultRelayerCall)
        }
    }
    /// Event filters.
    impl<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    > GPv2SettlementInstance<P, N> {
        /// Creates a new event filter using this contract instance's provider and address.
        ///
        /// Note that the type can be any event, not just those defined in this contract.
        /// Prefer using the other methods for building type-safe event filters.
        pub fn event_filter<E: alloy_sol_types::SolEvent>(
            &self,
        ) -> alloy_contract::Event<&P, E, N> {
            alloy_contract::Event::new_sol(&self.provider, &self.address)
        }
        ///Creates a new event filter for the [`Interaction`] event.
        pub fn Interaction_filter(&self) -> alloy_contract::Event<&P, Interaction, N> {
            self.event_filter::<Interaction>()
        }
        ///Creates a new event filter for the [`OrderInvalidated`] event.
        pub fn OrderInvalidated_filter(
            &self,
        ) -> alloy_contract::Event<&P, OrderInvalidated, N> {
            self.event_filter::<OrderInvalidated>()
        }
        ///Creates a new event filter for the [`PreSignature`] event.
        pub fn PreSignature_filter(&self) -> alloy_contract::Event<&P, PreSignature, N> {
            self.event_filter::<PreSignature>()
        }
        ///Creates a new event filter for the [`Settlement`] event.
        pub fn Settlement_filter(&self) -> alloy_contract::Event<&P, Settlement, N> {
            self.event_filter::<Settlement>()
        }
        ///Creates a new event filter for the [`Trade`] event.
        pub fn Trade_filter(&self) -> alloy_contract::Event<&P, Trade, N> {
            self.event_filter::<Trade>()
        }
    }
}
pub type Instance = GPv2Settlement::GPv2SettlementInstance<
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
        1u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0x9008D19f58AAbD9eD0D60971565AA8510560ab41"
                ),
                Some(12593265u64),
            ))
        }
        10u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0x9008D19f58AAbD9eD0D60971565AA8510560ab41"
                ),
                Some(134254624u64),
            ))
        }
        56u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0x9008D19f58AAbD9eD0D60971565AA8510560ab41"
                ),
                Some(48173641u64),
            ))
        }
        100u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0x9008D19f58AAbD9eD0D60971565AA8510560ab41"
                ),
                Some(16465100u64),
            ))
        }
        137u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0x9008D19f58AAbD9eD0D60971565AA8510560ab41"
                ),
                Some(45859743u64),
            ))
        }
        8453u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0x9008D19f58AAbD9eD0D60971565AA8510560ab41"
                ),
                Some(21407238u64),
            ))
        }
        9745u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0x9008D19f58AAbD9eD0D60971565AA8510560ab41"
                ),
                Some(3439711u64),
            ))
        }
        42161u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0x9008D19f58AAbD9eD0D60971565AA8510560ab41"
                ),
                Some(204704802u64),
            ))
        }
        43114u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0x9008D19f58AAbD9eD0D60971565AA8510560ab41"
                ),
                Some(59891356u64),
            ))
        }
        57073u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0x9008D19f58AAbD9eD0D60971565AA8510560ab41"
                ),
                Some(34436849u64),
            ))
        }
        59144u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0x9008D19f58AAbD9eD0D60971565AA8510560ab41"
                ),
                Some(24333100u64),
            ))
        }
        11155111u64 => {
            Some((
                ::alloy_primitives::address!(
                    "0x9008D19f58AAbD9eD0D60971565AA8510560ab41"
                ),
                Some(4717488u64),
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
