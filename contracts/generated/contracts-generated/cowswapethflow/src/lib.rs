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
library EthFlowOrder {
    struct Data { address buyToken; address receiver; uint256 sellAmount; uint256 buyAmount; bytes32 appData; uint256 feeAmount; uint32 validTo; bool partiallyFillable; int64 quoteId; }
}
```*/
#[allow(
    non_camel_case_types,
    non_snake_case,
    clippy::pub_underscore_fields,
    clippy::style,
    clippy::empty_structs_with_brackets
)]
pub mod EthFlowOrder {
    use super::*;
    use alloy_sol_types;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**```solidity
    struct Data { address buyToken; address receiver; uint256 sellAmount; uint256 buyAmount; bytes32 appData; uint256 feeAmount; uint32 validTo; bool partiallyFillable; int64 quoteId; }
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct Data {
        #[allow(missing_docs)]
        pub buyToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub receiver: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub sellAmount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub buyAmount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub appData: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub feeAmount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub validTo: u32,
        #[allow(missing_docs)]
        pub partiallyFillable: bool,
        #[allow(missing_docs)]
        pub quoteId: i64,
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
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::FixedBytes<32>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<32>,
            alloy_sol_types::sol_data::Bool,
            alloy_sol_types::sol_data::Int<64>,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::Address,
            alloy_sol_types::private::Address,
            alloy_sol_types::private::primitives::aliases::U256,
            alloy_sol_types::private::primitives::aliases::U256,
            alloy_sol_types::private::FixedBytes<32>,
            alloy_sol_types::private::primitives::aliases::U256,
            u32,
            bool,
            i64,
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
        impl ::core::convert::From<Data> for UnderlyingRustTuple<'_> {
            fn from(value: Data) -> Self {
                (
                    value.buyToken,
                    value.receiver,
                    value.sellAmount,
                    value.buyAmount,
                    value.appData,
                    value.feeAmount,
                    value.validTo,
                    value.partiallyFillable,
                    value.quoteId,
                )
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for Data {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    buyToken: tuple.0,
                    receiver: tuple.1,
                    sellAmount: tuple.2,
                    buyAmount: tuple.3,
                    appData: tuple.4,
                    feeAmount: tuple.5,
                    validTo: tuple.6,
                    partiallyFillable: tuple.7,
                    quoteId: tuple.8,
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
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self.appData),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.feeAmount),
                    <alloy_sol_types::sol_data::Uint<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self.validTo),
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::tokenize(
                        &self.partiallyFillable,
                    ),
                    <alloy_sol_types::sol_data::Int<
                        64,
                    > as alloy_sol_types::SolType>::tokenize(&self.quoteId),
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
        impl alloy_sol_types::SolType for Data {
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
        impl alloy_sol_types::SolStruct for Data {
            const NAME: &'static str = "Data";
            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "Data(address buyToken,address receiver,uint256 sellAmount,uint256 buyAmount,bytes32 appData,uint256 feeAmount,uint32 validTo,bool partiallyFillable,int64 quoteId)",
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
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.appData)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.feeAmount)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        32,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.validTo)
                        .0,
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::eip712_data_word(
                            &self.partiallyFillable,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Int<
                        64,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.quoteId)
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
                        32,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.validTo,
                    )
                    + <alloy_sol_types::sol_data::Bool as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.partiallyFillable,
                    )
                    + <alloy_sol_types::sol_data::Int<
                        64,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.quoteId,
                    )
            }
            #[inline]
            fn encode_topic_preimage(
                rust: &Self::RustType,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                out.reserve(<Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust));
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
                    32,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.validTo,
                    out,
                );
                <alloy_sol_types::sol_data::Bool as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.partiallyFillable,
                    out,
                );
                <alloy_sol_types::sol_data::Int<
                    64,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.quoteId,
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
    /**Creates a new wrapper around an on-chain [`EthFlowOrder`](self) contract instance.

    See the [wrapper's documentation](`EthFlowOrderInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> EthFlowOrderInstance<P, N> {
        EthFlowOrderInstance::<P, N>::new(address, __provider)
    }
    /**A [`EthFlowOrder`](self) instance.

    Contains type-safe methods for interacting with an on-chain instance of the
    [`EthFlowOrder`](self) contract located at a given `address`, using a given
    provider `P`.

    If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
    documentation on how to provide it), the `deploy` and `deploy_builder` methods can
    be used to deploy a new instance of the contract.

    See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct EthFlowOrderInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for EthFlowOrderInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("EthFlowOrderInstance")
                .field(&self.address)
                .finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        EthFlowOrderInstance<P, N>
    {
        /**Creates a new wrapper around an on-chain [`EthFlowOrder`](self) contract instance.

        See the [wrapper's documentation](`EthFlowOrderInstance`) for more details.*/
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
    impl<P: ::core::clone::Clone, N> EthFlowOrderInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned provider.
        #[inline]
        pub fn with_cloned_provider(self) -> EthFlowOrderInstance<P, N> {
            EthFlowOrderInstance {
                address: self.address,
                provider: ::core::clone::Clone::clone(&self.provider),
                _network: ::core::marker::PhantomData,
            }
        }
    }
    /// Function calls.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        EthFlowOrderInstance<P, N>
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
        EthFlowOrderInstance<P, N>
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
    use alloy_sol_types;
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
        use alloy_sol_types;
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
        fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
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
        impl alloy_sol_types::SolType for Data {
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
        impl alloy_sol_types::SolStruct for Data {
            const NAME: &'static str = "Data";
            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "Data(address sellToken,address buyToken,address receiver,uint256 sellAmount,uint256 buyAmount,uint32 validTo,bytes32 appData,uint256 feeAmount,bytes32 kind,bool partiallyFillable,bytes32 sellTokenBalance,bytes32 buyTokenBalance)",
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
                out.reserve(<Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust));
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
            fn encode_topic(rust: &Self::RustType) -> alloy_sol_types::abi::token::WordToken {
                let mut out = alloy_sol_types::private::Vec::new();
                <Self as alloy_sol_types::EventTopic>::encode_topic_preimage(rust, &mut out);
                alloy_sol_types::abi::token::WordToken(alloy_sol_types::private::keccak256(out))
            }
        }
    };
    use alloy_contract;
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
            f.debug_tuple("GPv2OrderInstance")
                .field(&self.address)
                .finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        GPv2OrderInstance<P, N>
    {
        /**Creates a new wrapper around an on-chain [`GPv2Order`](self) contract instance.

        See the [wrapper's documentation](`GPv2OrderInstance`) for more details.*/
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
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        GPv2OrderInstance<P, N>
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
        GPv2OrderInstance<P, N>
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
///Module containing a contract's types and functions.
/**

```solidity
library ICoWSwapOnchainOrders {
    type OnchainSigningScheme is uint8;
    struct OnchainSignature { OnchainSigningScheme scheme; bytes data; }
}
```*/
#[allow(
    non_camel_case_types,
    non_snake_case,
    clippy::pub_underscore_fields,
    clippy::style,
    clippy::empty_structs_with_brackets
)]
pub mod ICoWSwapOnchainOrders {
    use super::*;
    use alloy_sol_types;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct OnchainSigningScheme(u8);
    const _: () = {
        use alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<OnchainSigningScheme> for u8 {
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
        impl OnchainSigningScheme {
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
        impl From<u8> for OnchainSigningScheme {
            fn from(value: u8) -> Self {
                Self::from_underlying(value)
            }
        }
        #[automatically_derived]
        impl From<OnchainSigningScheme> for u8 {
            fn from(value: OnchainSigningScheme) -> Self {
                value.into_underlying()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolType for OnchainSigningScheme {
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
        impl alloy_sol_types::EventTopic for OnchainSigningScheme {
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
    struct OnchainSignature { OnchainSigningScheme scheme; bytes data; }
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct OnchainSignature {
        #[allow(missing_docs)]
        pub scheme: <OnchainSigningScheme as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub data: alloy_sol_types::private::Bytes,
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
        type UnderlyingSolTuple<'a> = (OnchainSigningScheme, alloy_sol_types::sol_data::Bytes);
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            <OnchainSigningScheme as alloy_sol_types::SolType>::RustType,
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
        impl ::core::convert::From<OnchainSignature> for UnderlyingRustTuple<'_> {
            fn from(value: OnchainSignature) -> Self {
                (value.scheme, value.data)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for OnchainSignature {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    scheme: tuple.0,
                    data: tuple.1,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for OnchainSignature {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for OnchainSignature {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <OnchainSigningScheme as alloy_sol_types::SolType>::tokenize(&self.scheme),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.data,
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
        impl alloy_sol_types::SolType for OnchainSignature {
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
        impl alloy_sol_types::SolStruct for OnchainSignature {
            const NAME: &'static str = "OnchainSignature";
            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed("OnchainSignature(uint8 scheme,bytes data)")
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
                    <OnchainSigningScheme as alloy_sol_types::SolType>::eip712_data_word(
                            &self.scheme,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::eip712_data_word(
                            &self.data,
                        )
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for OnchainSignature {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <OnchainSigningScheme as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.scheme,
                    )
                    + <alloy_sol_types::sol_data::Bytes as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.data,
                    )
            }
            #[inline]
            fn encode_topic_preimage(
                rust: &Self::RustType,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                out.reserve(<Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust));
                <OnchainSigningScheme as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.scheme,
                    out,
                );
                <alloy_sol_types::sol_data::Bytes as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.data,
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
    /**Creates a new wrapper around an on-chain [`ICoWSwapOnchainOrders`](self) contract instance.

    See the [wrapper's documentation](`ICoWSwapOnchainOrdersInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> ICoWSwapOnchainOrdersInstance<P, N> {
        ICoWSwapOnchainOrdersInstance::<P, N>::new(address, __provider)
    }
    /**A [`ICoWSwapOnchainOrders`](self) instance.

    Contains type-safe methods for interacting with an on-chain instance of the
    [`ICoWSwapOnchainOrders`](self) contract located at a given `address`, using a given
    provider `P`.

    If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
    documentation on how to provide it), the `deploy` and `deploy_builder` methods can
    be used to deploy a new instance of the contract.

    See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct ICoWSwapOnchainOrdersInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for ICoWSwapOnchainOrdersInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("ICoWSwapOnchainOrdersInstance")
                .field(&self.address)
                .finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        ICoWSwapOnchainOrdersInstance<P, N>
    {
        /**Creates a new wrapper around an on-chain [`ICoWSwapOnchainOrders`](self) contract instance.

        See the [wrapper's documentation](`ICoWSwapOnchainOrdersInstance`) for more details.*/
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
    impl<P: ::core::clone::Clone, N> ICoWSwapOnchainOrdersInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned provider.
        #[inline]
        pub fn with_cloned_provider(self) -> ICoWSwapOnchainOrdersInstance<P, N> {
            ICoWSwapOnchainOrdersInstance {
                address: self.address,
                provider: ::core::clone::Clone::clone(&self.provider),
                _network: ::core::marker::PhantomData,
            }
        }
    }
    /// Function calls.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        ICoWSwapOnchainOrdersInstance<P, N>
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
        ICoWSwapOnchainOrdersInstance<P, N>
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
library EthFlowOrder {
    struct Data {
        address buyToken;
        address receiver;
        uint256 sellAmount;
        uint256 buyAmount;
        bytes32 appData;
        uint256 feeAmount;
        uint32 validTo;
        bool partiallyFillable;
        int64 quoteId;
    }
}

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

library ICoWSwapOnchainOrders {
    type OnchainSigningScheme is uint8;
    struct OnchainSignature {
        OnchainSigningScheme scheme;
        bytes data;
    }
}

interface CoWSwapEthFlow {
    error EthTransferFailed();
    error IncorrectEthAmount();
    error NotAllowedToInvalidateOrder(bytes32 orderHash);
    error NotAllowedZeroSellAmount();
    error OrderIsAlreadyExpired();
    error OrderIsAlreadyOwned(bytes32 orderHash);
    error ReceiverMustBeSet();

    event OrderInvalidation(bytes orderUid);
    event OrderPlacement(address indexed sender, GPv2Order.Data order, ICoWSwapOnchainOrders.OnchainSignature signature, bytes data);
    event OrderRefund(bytes orderUid, address indexed refunder);

    constructor(address _cowSwapSettlement, address _wrappedNativeToken);

    receive() external payable;

    function cowSwapSettlement() external view returns (address);
    function createOrder(EthFlowOrder.Data memory order) external payable returns (bytes32 orderHash);
    function invalidateOrder(EthFlowOrder.Data memory order) external;
    function invalidateOrdersIgnoringNotAllowed(EthFlowOrder.Data[] memory orderArray) external;
    function isValidSignature(bytes32 orderHash, bytes memory) external view returns (bytes4);
    function orders(bytes32) external view returns (address owner, uint32 validTo);
    function unwrap(uint256 amount) external;
    function wrap(uint256 amount) external;
    function wrapAll() external;
    function wrappedNativeToken() external view returns (address);
}
```

...which was generated by the following JSON ABI:
```json
[
  {
    "type": "constructor",
    "inputs": [
      {
        "name": "_cowSwapSettlement",
        "type": "address",
        "internalType": "contract ICoWSwapSettlement"
      },
      {
        "name": "_wrappedNativeToken",
        "type": "address",
        "internalType": "contract IWrappedNativeToken"
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
    "name": "cowSwapSettlement",
    "inputs": [],
    "outputs": [
      {
        "name": "",
        "type": "address",
        "internalType": "contract ICoWSwapSettlement"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "createOrder",
    "inputs": [
      {
        "name": "order",
        "type": "tuple",
        "internalType": "struct EthFlowOrder.Data",
        "components": [
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
            "name": "validTo",
            "type": "uint32",
            "internalType": "uint32"
          },
          {
            "name": "partiallyFillable",
            "type": "bool",
            "internalType": "bool"
          },
          {
            "name": "quoteId",
            "type": "int64",
            "internalType": "int64"
          }
        ]
      }
    ],
    "outputs": [
      {
        "name": "orderHash",
        "type": "bytes32",
        "internalType": "bytes32"
      }
    ],
    "stateMutability": "payable"
  },
  {
    "type": "function",
    "name": "invalidateOrder",
    "inputs": [
      {
        "name": "order",
        "type": "tuple",
        "internalType": "struct EthFlowOrder.Data",
        "components": [
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
            "name": "validTo",
            "type": "uint32",
            "internalType": "uint32"
          },
          {
            "name": "partiallyFillable",
            "type": "bool",
            "internalType": "bool"
          },
          {
            "name": "quoteId",
            "type": "int64",
            "internalType": "int64"
          }
        ]
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "invalidateOrdersIgnoringNotAllowed",
    "inputs": [
      {
        "name": "orderArray",
        "type": "tuple[]",
        "internalType": "struct EthFlowOrder.Data[]",
        "components": [
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
            "name": "validTo",
            "type": "uint32",
            "internalType": "uint32"
          },
          {
            "name": "partiallyFillable",
            "type": "bool",
            "internalType": "bool"
          },
          {
            "name": "quoteId",
            "type": "int64",
            "internalType": "int64"
          }
        ]
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "isValidSignature",
    "inputs": [
      {
        "name": "orderHash",
        "type": "bytes32",
        "internalType": "bytes32"
      },
      {
        "name": "",
        "type": "bytes",
        "internalType": "bytes"
      }
    ],
    "outputs": [
      {
        "name": "",
        "type": "bytes4",
        "internalType": "bytes4"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "orders",
    "inputs": [
      {
        "name": "",
        "type": "bytes32",
        "internalType": "bytes32"
      }
    ],
    "outputs": [
      {
        "name": "owner",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "validTo",
        "type": "uint32",
        "internalType": "uint32"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "unwrap",
    "inputs": [
      {
        "name": "amount",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "wrap",
    "inputs": [
      {
        "name": "amount",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "wrapAll",
    "inputs": [],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "wrappedNativeToken",
    "inputs": [],
    "outputs": [
      {
        "name": "",
        "type": "address",
        "internalType": "contract IWrappedNativeToken"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "event",
    "name": "OrderInvalidation",
    "inputs": [
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
    "name": "OrderPlacement",
    "inputs": [
      {
        "name": "sender",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "order",
        "type": "tuple",
        "indexed": false,
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
        "type": "tuple",
        "indexed": false,
        "internalType": "struct ICoWSwapOnchainOrders.OnchainSignature",
        "components": [
          {
            "name": "scheme",
            "type": "uint8",
            "internalType": "enum ICoWSwapOnchainOrders.OnchainSigningScheme"
          },
          {
            "name": "data",
            "type": "bytes",
            "internalType": "bytes"
          }
        ]
      },
      {
        "name": "data",
        "type": "bytes",
        "indexed": false,
        "internalType": "bytes"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "OrderRefund",
    "inputs": [
      {
        "name": "orderUid",
        "type": "bytes",
        "indexed": false,
        "internalType": "bytes"
      },
      {
        "name": "refunder",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      }
    ],
    "anonymous": false
  },
  {
    "type": "error",
    "name": "EthTransferFailed",
    "inputs": []
  },
  {
    "type": "error",
    "name": "IncorrectEthAmount",
    "inputs": []
  },
  {
    "type": "error",
    "name": "NotAllowedToInvalidateOrder",
    "inputs": [
      {
        "name": "orderHash",
        "type": "bytes32",
        "internalType": "bytes32"
      }
    ]
  },
  {
    "type": "error",
    "name": "NotAllowedZeroSellAmount",
    "inputs": []
  },
  {
    "type": "error",
    "name": "OrderIsAlreadyExpired",
    "inputs": []
  },
  {
    "type": "error",
    "name": "OrderIsAlreadyOwned",
    "inputs": [
      {
        "name": "orderHash",
        "type": "bytes32",
        "internalType": "bytes32"
      }
    ]
  },
  {
    "type": "error",
    "name": "ReceiverMustBeSet",
    "inputs": []
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
pub mod CoWSwapEthFlow {
    use super::*;
    use alloy_sol_types;
    /// The creation / init bytecode of the contract.
    ///
    /// ```text
    ///0x60e06040523480156200001157600080fd5b5060405162001b2a38038062001b2a83398101604081905262000034916200021e565b816200004b816200015260201b6200089b1760201c565b608052506001600160a01b0380831660a081905290821660c081905260408051634daa966160e11b81529051919263095ea7b3929091639b552cc291600480830192602092919082900301816000875af1158015620000ae573d6000803e3d6000fd5b505050506040513d601f19601f82011682018060405250810190620000d491906200025d565b6040516001600160e01b031960e084901b1681526001600160a01b03909116600482015260001960248201526044016020604051808303816000875af115801562000123573d6000803e3d6000fd5b505050506040513d601f19601f8201168201806040525081019062000149919062000284565b505050620002a8565b604080517f8b73c3c69bb8fe3d512ecc4cf759cc79239f7b179b0ffacaa9a75d522b39400f60208201527f6c85c0337eba1661327f94f3bf46c8a7f9311a563f4d5c948362567f5d8ed60c918101919091527ff9446b8e937d86f0bc87cac73923491692b123ca5f8761908494703758206adf606082015246608082018190526001600160a01b03831660a083015260009160c00160405160208183030381529060405280519060200120915050919050565b6001600160a01b03811681146200021b57600080fd5b50565b600080604083850312156200023257600080fd5b82516200023f8162000205565b6020840151909250620002528162000205565b809150509250929050565b6000602082840312156200027057600080fd5b81516200027d8162000205565b9392505050565b6000602082840312156200029757600080fd5b815180151581146200027d57600080fd5b60805160a05160c0516118216200030960003960008181610129015281816105ff015281816107ad0152818161082501528181610c3301526110310152600081816102ce0152610f4b015260008181610bf70152610cd901526118216000f3fe6080604052600436106100b55760003560e01c80637bc41b9611610069578063de0e9a3e1161004e578063de0e9a3e1461027c578063ea598cb01461029c578063ec30bb88146102bc57600080fd5b80637bc41b96146101c85780639c3f1e90146101e857600080fd5b8063322bba211161009a578063322bba21146101705780634c84c1c8146101915780634cb76498146101a857600080fd5b80631626ba7e146100c157806317fcb39b1461011757600080fd5b366100bc57005b600080fd5b3480156100cd57600080fd5b506100e16100dc36600461126e565b6102f0565b6040517fffffffff0000000000000000000000000000000000000000000000000000000090911681526020015b60405180910390f35b34801561012357600080fd5b5061014b7f000000000000000000000000000000000000000000000000000000000000000081565b60405173ffffffffffffffffffffffffffffffffffffffff909116815260200161010e565b61018361017e36600461132b565b6103de565b60405190815260200161010e565b34801561019d57600080fd5b506101a6610720565b005b3480156101b457600080fd5b506101a66101c3366004611344565b61072b565b3480156101d457600080fd5b506101a66101e336600461132b565b610770565b3480156101f457600080fd5b5061024b6102033660046113ba565b60006020819052908152604090205473ffffffffffffffffffffffffffffffffffffffff81169074010000000000000000000000000000000000000000900463ffffffff1682565b6040805173ffffffffffffffffffffffffffffffffffffffff909316835263ffffffff90911660208301520161010e565b34801561028857600080fd5b506101a66102973660046113ba565b61077e565b3480156102a857600080fd5b506101a66102b73660046113ba565b610821565b3480156102c857600080fd5b5061014b7f000000000000000000000000000000000000000000000000000000000000000081565b60008281526020818152604080832081518083019092525473ffffffffffffffffffffffffffffffffffffffff81168083527401000000000000000000000000000000000000000090910463ffffffff1692820192909252901580159061036f5750805173ffffffffffffffffffffffffffffffffffffffff90811614155b8015610385575042816020015163ffffffff1610155b156103b357507f1626ba7e0000000000000000000000000000000000000000000000000000000090506103d8565b507fffffffff0000000000000000000000000000000000000000000000000000000090505b92915050565b60006103f260a08301356040840135611402565b341461042a576040517f8b6ebb4d00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b8160400135600003610468576040517feaec5c9d00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b4261047960e0840160c0850161142e565b63ffffffff1610156104b7576040517f89bb260100000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b60408051808201909152338152600090602081016104db60e0860160c0870161142e565b63ffffffff169052604080518082019091529091506000908082815260200130604051602001610536919060609190911b7fffffffffffffffffffffffffffffffffffffffff00000000000000000000000016815260140190565b604080517fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe081840301815291905290529050600061057c61012086016101008701611462565b6020808501516040516105c393920160c09290921b825260e01b7fffffffff00000000000000000000000000000000000000000000000000000000166008820152600c0190565b604080517fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0818403018152919052835190915061063a906106337f000000000000000000000000000000000000000000000000000000000000000061062d368a90038a018a6114b1565b9061095b565b8484610b2a565b60008181526020819052604090205490945073ffffffffffffffffffffffffffffffffffffffff16156106a1576040517f56a1d2b2000000000000000000000000000000000000000000000000000000008152600481018590526024015b60405180910390fd5b505060008281526020818152604090912082518154929093015163ffffffff1674010000000000000000000000000000000000000000027fffffffffffffffff00000000000000000000000000000000000000000000000090921673ffffffffffffffffffffffffffffffffffffffff90931692909217179055919050565b61072947610821565b565b60005b8181101561076b5761075983838381811061074b5761074b61154b565b905061012002016000610c2c565b806107638161157a565b91505061072e565b505050565b61077b816001610c2c565b50565b6040517f2e1a7d4d000000000000000000000000000000000000000000000000000000008152600481018290527f000000000000000000000000000000000000000000000000000000000000000073ffffffffffffffffffffffffffffffffffffffff1690632e1a7d4d90602401600060405180830381600087803b15801561080657600080fd5b505af115801561081a573d6000803e3d6000fd5b5050505050565b60007f000000000000000000000000000000000000000000000000000000000000000073ffffffffffffffffffffffffffffffffffffffff168260405160006040518083038185875af1925050503d806000811461081a576040519150601f19603f3d011682016040523d82523d6000602084013e61081a565b604080517f8b73c3c69bb8fe3d512ecc4cf759cc79239f7b179b0ffacaa9a75d522b39400f60208201527f6c85c0337eba1661327f94f3bf46c8a7f9311a563f4d5c948362567f5d8ed60c918101919091527ff9446b8e937d86f0bc87cac73923491692b123ca5f8761908494703758206adf6060820152466080820181905273ffffffffffffffffffffffffffffffffffffffff831660a083015260009160c00160405160208183030381529060405280519060200120915050919050565b604080516101808101825260008082526020808301829052928201819052606082018190526080820181905260a0820181905260c0820181905260e082018190526101008201819052610120820181905261014082018190526101608201529083015173ffffffffffffffffffffffffffffffffffffffff16610a0a576040517fefc9ccdf00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6040518061018001604052808373ffffffffffffffffffffffffffffffffffffffff168152602001846000015173ffffffffffffffffffffffffffffffffffffffff168152602001846020015173ffffffffffffffffffffffffffffffffffffffff168152602001846040015181526020018460600151815260200163ffffffff80168152602001846080015181526020018460a0015181526020017ff3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee34677581526020018460e00151151581526020017f5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc981526020017f5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc9815250905092915050565b60008473ffffffffffffffffffffffffffffffffffffffff167fcf5f9de2984132265203b5c335b25727702ca77262ff622e136baa7362bf1da9858585604051610b7693929190611676565b60405180910390a25050507fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe00180517fd5a25ba2e97094ad7d83dc28a6572da797d6b3e7fc6663bd93efb789fc17e48982526101a0822091526040517f190100000000000000000000000000000000000000000000000000000000000081527f00000000000000000000000000000000000000000000000000000000000000006002820152602281019190915260429020919050565b6000610c617f000000000000000000000000000000000000000000000000000000000000000061062d368690038601866114b1565b7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0810180517fd5a25ba2e97094ad7d83dc28a6572da797d6b3e7fc6663bd93efb789fc17e48982526101a082209152604080517f190100000000000000000000000000000000000000000000000000000000000081527f0000000000000000000000000000000000000000000000000000000000000000600282015260228101929092526042909120600081815260208181529083902083518085019094525473ffffffffffffffffffffffffffffffffffffffff8082168086527401000000000000000000000000000000000000000090920463ffffffff1692850183905294955091934290911015911480610d8d5750815173ffffffffffffffffffffffffffffffffffffffff16155b80610db75750808015610db75750815173ffffffffffffffffffffffffffffffffffffffff163314155b15610dff578415610df7576040517ff8cc70ce00000000000000000000000000000000000000000000000000000000815260048101849052602401610698565b505050505050565b60008381526020818152604080832080547fffffffffffffffffffffffff00000000000000000000000000000000000000001673ffffffffffffffffffffffffffffffffffffffff1790558051603880825260608201909252918201818036833750505060a0860151909150610e7a90829086903090611149565b8115610ebc577fb8bad102ac8bbacfef31ff1c906ec6d951c230b4dce750bb0376b812ad35852a81604051610eaf9190611790565b60405180910390a1610f0b565b3373ffffffffffffffffffffffffffffffffffffffff167f195271068a288191e4b265c641a56b9832919f69e9e7d6c2f31ba40278aeb85a82604051610f029190611790565b60405180910390a25b6040517f2479fb6e00000000000000000000000000000000000000000000000000000000815260009073ffffffffffffffffffffffffffffffffffffffff7f00000000000000000000000000000000000000000000000000000000000000001690632479fb6e90610f80908590600401611790565b6020604051808303816000875af1158015610f9f573d6000803e3d6000fd5b505050506040513d601f19601f82011682018060405250810190610fc391906117a3565b90506000808760600151838960e001510281610fe157610fe16117bc565b048860e00151039050808389606001510301915050804710156110a4576040517f2e1a7d4d00000000000000000000000000000000000000000000000000000000815247820360048201819052907f000000000000000000000000000000000000000000000000000000000000000073ffffffffffffffffffffffffffffffffffffffff1690632e1a7d4d90602401600060405180830381600087803b15801561108a57600080fd5b505af115801561109e573d6000803e3d6000fd5b50505050505b845160405160009173ffffffffffffffffffffffffffffffffffffffff169083908381818185875af1925050503d80600081146110fd576040519150601f19603f3d011682016040523d82523d6000602084013e611102565b606091505b505090508061113d576040517f6d963f8800000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b50505050505050505050565b60388451146111b4576040517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601960248201527f475076323a2075696420627566666572206f766572666c6f77000000000000006044820152606401610698565b60388401526034830152602090910152565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052604160045260246000fd5b604051610120810167ffffffffffffffff81118282101715611219576112196111c6565b60405290565b604051601f82017fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe016810167ffffffffffffffff81118282101715611266576112666111c6565b604052919050565b6000806040838503121561128157600080fd5b8235915060208084013567ffffffffffffffff808211156112a157600080fd5b818601915086601f8301126112b557600080fd5b8135818111156112c7576112c76111c6565b6112f7847fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f8401160161121f565b9150808252878482850101111561130d57600080fd5b80848401858401376000848284010152508093505050509250929050565b6000610120828403121561133e57600080fd5b50919050565b6000806020838503121561135757600080fd5b823567ffffffffffffffff8082111561136f57600080fd5b818501915085601f83011261138357600080fd5b81358181111561139257600080fd5b866020610120830285010111156113a857600080fd5b60209290920196919550909350505050565b6000602082840312156113cc57600080fd5b5035919050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052601160045260246000fd5b808201808211156103d8576103d86113d3565b803563ffffffff8116811461142957600080fd5b919050565b60006020828403121561144057600080fd5b61144982611415565b9392505050565b8035600781900b811461142957600080fd5b60006020828403121561147457600080fd5b61144982611450565b803573ffffffffffffffffffffffffffffffffffffffff8116811461142957600080fd5b8035801515811461142957600080fd5b600061012082840312156114c457600080fd5b6114cc6111f5565b6114d58361147d565b81526114e36020840161147d565b602082015260408301356040820152606083013560608201526080830135608082015260a083013560a082015261151c60c08401611415565b60c082015261152d60e084016114a1565b60e0820152610100611540818501611450565b908201529392505050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052603260045260246000fd5b60007fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff82036115ab576115ab6113d3565b5060010190565b6000815180845260005b818110156115d8576020818501810151868301820152016115bc565b5060006020828601015260207fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f83011685010191505092915050565b6000815160028110611651577f4e487b7100000000000000000000000000000000000000000000000000000000600052602160045260246000fd5b8084525060208201516040602085015261166e60408501826115b2565b949350505050565b835173ffffffffffffffffffffffffffffffffffffffff16815260006101c060208601516116bc602085018273ffffffffffffffffffffffffffffffffffffffff169052565b5060408601516116e4604085018273ffffffffffffffffffffffffffffffffffffffff169052565b50606086015160608401526080860151608084015260a086015161171060a085018263ffffffff169052565b5060c086015160c084015260e086015160e0840152610100808701518185015250610120808701516117458286018215159052565b505061014086810151908401526101608087015190840152610180830181905261177181840186611616565b90508281036101a084015261178681856115b2565b9695505050505050565b60208152600061144960208301846115b2565b6000602082840312156117b557600080fd5b5051919050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052601260045260246000fdfea2646970667358221220d3219a243fb3b7683c6c6a0918144885c8551f0fd87b19a0e7355ed3d10e937064736f6c63430008100033
    /// ```
    #[rustfmt::skip]
    #[allow(clippy::all)]
    pub static BYTECODE: alloy_sol_types::private::Bytes = alloy_sol_types::private::Bytes::from_static(
        b"`\xE0`@R4\x80\x15b\0\0\x11W`\0\x80\xFD[P`@Qb\0\x1B*8\x03\x80b\0\x1B*\x839\x81\x01`@\x81\x90Rb\0\x004\x91b\0\x02\x1EV[\x81b\0\0K\x81b\0\x01R` \x1Bb\0\x08\x9B\x17` \x1CV[`\x80RP`\x01`\x01`\xA0\x1B\x03\x80\x83\x16`\xA0\x81\x90R\x90\x82\x16`\xC0\x81\x90R`@\x80QcM\xAA\x96a`\xE1\x1B\x81R\x90Q\x91\x92c\t^\xA7\xB3\x92\x90\x91c\x9BU,\xC2\x91`\x04\x80\x83\x01\x92` \x92\x91\x90\x82\x90\x03\x01\x81`\0\x87Z\xF1\x15\x80\x15b\0\0\xAEW=`\0\x80>=`\0\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90b\0\0\xD4\x91\x90b\0\x02]V[`@Q`\x01`\x01`\xE0\x1B\x03\x19`\xE0\x84\x90\x1B\x16\x81R`\x01`\x01`\xA0\x1B\x03\x90\x91\x16`\x04\x82\x01R`\0\x19`$\x82\x01R`D\x01` `@Q\x80\x83\x03\x81`\0\x87Z\xF1\x15\x80\x15b\0\x01#W=`\0\x80>=`\0\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90b\0\x01I\x91\x90b\0\x02\x84V[PPPb\0\x02\xA8V[`@\x80Q\x7F\x8Bs\xC3\xC6\x9B\xB8\xFE=Q.\xCCL\xF7Y\xCCy#\x9F{\x17\x9B\x0F\xFA\xCA\xA9\xA7]R+9@\x0F` \x82\x01R\x7Fl\x85\xC03~\xBA\x16a2\x7F\x94\xF3\xBFF\xC8\xA7\xF91\x1AV?M\\\x94\x83bV\x7F]\x8E\xD6\x0C\x91\x81\x01\x91\x90\x91R\x7F\xF9Dk\x8E\x93}\x86\xF0\xBC\x87\xCA\xC79#I\x16\x92\xB1#\xCA_\x87a\x90\x84\x94p7X j\xDF``\x82\x01RF`\x80\x82\x01\x81\x90R`\x01`\x01`\xA0\x1B\x03\x83\x16`\xA0\x83\x01R`\0\x91`\xC0\x01`@Q` \x81\x83\x03\x03\x81R\x90`@R\x80Q\x90` \x01 \x91PP\x91\x90PV[`\x01`\x01`\xA0\x1B\x03\x81\x16\x81\x14b\0\x02\x1BW`\0\x80\xFD[PV[`\0\x80`@\x83\x85\x03\x12\x15b\0\x022W`\0\x80\xFD[\x82Qb\0\x02?\x81b\0\x02\x05V[` \x84\x01Q\x90\x92Pb\0\x02R\x81b\0\x02\x05V[\x80\x91PP\x92P\x92\x90PV[`\0` \x82\x84\x03\x12\x15b\0\x02pW`\0\x80\xFD[\x81Qb\0\x02}\x81b\0\x02\x05V[\x93\x92PPPV[`\0` \x82\x84\x03\x12\x15b\0\x02\x97W`\0\x80\xFD[\x81Q\x80\x15\x15\x81\x14b\0\x02}W`\0\x80\xFD[`\x80Q`\xA0Q`\xC0Qa\x18!b\0\x03\t`\09`\0\x81\x81a\x01)\x01R\x81\x81a\x05\xFF\x01R\x81\x81a\x07\xAD\x01R\x81\x81a\x08%\x01R\x81\x81a\x0C3\x01Ra\x101\x01R`\0\x81\x81a\x02\xCE\x01Ra\x0FK\x01R`\0\x81\x81a\x0B\xF7\x01Ra\x0C\xD9\x01Ra\x18!`\0\xF3\xFE`\x80`@R`\x046\x10a\0\xB5W`\x005`\xE0\x1C\x80c{\xC4\x1B\x96\x11a\0iW\x80c\xDE\x0E\x9A>\x11a\0NW\x80c\xDE\x0E\x9A>\x14a\x02|W\x80c\xEAY\x8C\xB0\x14a\x02\x9CW\x80c\xEC0\xBB\x88\x14a\x02\xBCW`\0\x80\xFD[\x80c{\xC4\x1B\x96\x14a\x01\xC8W\x80c\x9C?\x1E\x90\x14a\x01\xE8W`\0\x80\xFD[\x80c2+\xBA!\x11a\0\x9AW\x80c2+\xBA!\x14a\x01pW\x80cL\x84\xC1\xC8\x14a\x01\x91W\x80cL\xB7d\x98\x14a\x01\xA8W`\0\x80\xFD[\x80c\x16&\xBA~\x14a\0\xC1W\x80c\x17\xFC\xB3\x9B\x14a\x01\x17W`\0\x80\xFD[6a\0\xBCW\0[`\0\x80\xFD[4\x80\x15a\0\xCDW`\0\x80\xFD[Pa\0\xE1a\0\xDC6`\x04a\x12nV[a\x02\xF0V[`@Q\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90\x91\x16\x81R` \x01[`@Q\x80\x91\x03\x90\xF3[4\x80\x15a\x01#W`\0\x80\xFD[Pa\x01K\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[`@Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x90\x91\x16\x81R` \x01a\x01\x0EV[a\x01\x83a\x01~6`\x04a\x13+V[a\x03\xDEV[`@Q\x90\x81R` \x01a\x01\x0EV[4\x80\x15a\x01\x9DW`\0\x80\xFD[Pa\x01\xA6a\x07 V[\0[4\x80\x15a\x01\xB4W`\0\x80\xFD[Pa\x01\xA6a\x01\xC36`\x04a\x13DV[a\x07+V[4\x80\x15a\x01\xD4W`\0\x80\xFD[Pa\x01\xA6a\x01\xE36`\x04a\x13+V[a\x07pV[4\x80\x15a\x01\xF4W`\0\x80\xFD[Pa\x02Ka\x02\x036`\x04a\x13\xBAV[`\0` \x81\x90R\x90\x81R`@\x90 Ts\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x16\x90t\x01\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90\x04c\xFF\xFF\xFF\xFF\x16\x82V[`@\x80Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x90\x93\x16\x83Rc\xFF\xFF\xFF\xFF\x90\x91\x16` \x83\x01R\x01a\x01\x0EV[4\x80\x15a\x02\x88W`\0\x80\xFD[Pa\x01\xA6a\x02\x976`\x04a\x13\xBAV[a\x07~V[4\x80\x15a\x02\xA8W`\0\x80\xFD[Pa\x01\xA6a\x02\xB76`\x04a\x13\xBAV[a\x08!V[4\x80\x15a\x02\xC8W`\0\x80\xFD[Pa\x01K\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[`\0\x82\x81R` \x81\x81R`@\x80\x83 \x81Q\x80\x83\x01\x90\x92RTs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x16\x80\x83Rt\x01\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90\x91\x04c\xFF\xFF\xFF\xFF\x16\x92\x82\x01\x92\x90\x92R\x90\x15\x80\x15\x90a\x03oWP\x80Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x90\x81\x16\x14\x15[\x80\x15a\x03\x85WPB\x81` \x01Qc\xFF\xFF\xFF\xFF\x16\x10\x15[\x15a\x03\xB3WP\x7F\x16&\xBA~\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90Pa\x03\xD8V[P\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90P[\x92\x91PPV[`\0a\x03\xF2`\xA0\x83\x015`@\x84\x015a\x14\x02V[4\x14a\x04*W`@Q\x7F\x8Bn\xBBM\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x81`@\x015`\0\x03a\x04hW`@Q\x7F\xEA\xEC\\\x9D\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[Ba\x04y`\xE0\x84\x01`\xC0\x85\x01a\x14.V[c\xFF\xFF\xFF\xFF\x16\x10\x15a\x04\xB7W`@Q\x7F\x89\xBB&\x01\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[`@\x80Q\x80\x82\x01\x90\x91R3\x81R`\0\x90` \x81\x01a\x04\xDB`\xE0\x86\x01`\xC0\x87\x01a\x14.V[c\xFF\xFF\xFF\xFF\x16\x90R`@\x80Q\x80\x82\x01\x90\x91R\x90\x91P`\0\x90\x80\x82\x81R` \x010`@Q` \x01a\x056\x91\x90``\x91\x90\x91\x1B\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\x16\x81R`\x14\x01\x90V[`@\x80Q\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x81\x84\x03\x01\x81R\x91\x90R\x90R\x90P`\0a\x05|a\x01 \x86\x01a\x01\0\x87\x01a\x14bV[` \x80\x85\x01Q`@Qa\x05\xC3\x93\x92\x01`\xC0\x92\x90\x92\x1B\x82R`\xE0\x1B\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16`\x08\x82\x01R`\x0C\x01\x90V[`@\x80Q\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x81\x84\x03\x01\x81R\x91\x90R\x83Q\x90\x91Pa\x06:\x90a\x063\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0a\x06-6\x8A\x90\x03\x8A\x01\x8Aa\x14\xB1V[\x90a\t[V[\x84\x84a\x0B*V[`\0\x81\x81R` \x81\x90R`@\x90 T\x90\x94Ps\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x15a\x06\xA1W`@Q\x7FV\xA1\xD2\xB2\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x81\x01\x85\x90R`$\x01[`@Q\x80\x91\x03\x90\xFD[PP`\0\x82\x81R` \x81\x81R`@\x90\x91 \x82Q\x81T\x92\x90\x93\x01Qc\xFF\xFF\xFF\xFF\x16t\x01\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x02\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90\x92\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x90\x93\x16\x92\x90\x92\x17\x17\x90U\x91\x90PV[a\x07)Ga\x08!V[V[`\0[\x81\x81\x10\x15a\x07kWa\x07Y\x83\x83\x83\x81\x81\x10a\x07KWa\x07Ka\x15KV[\x90Pa\x01 \x02\x01`\0a\x0C,V[\x80a\x07c\x81a\x15zV[\x91PPa\x07.V[PPPV[a\x07{\x81`\x01a\x0C,V[PV[`@Q\x7F.\x1A}M\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x81\x01\x82\x90R\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90c.\x1A}M\x90`$\x01`\0`@Q\x80\x83\x03\x81`\0\x87\x80;\x15\x80\x15a\x08\x06W`\0\x80\xFD[PZ\xF1\x15\x80\x15a\x08\x1AW=`\0\x80>=`\0\xFD[PPPPPV[`\0\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x82`@Q`\0`@Q\x80\x83\x03\x81\x85\x87Z\xF1\x92PPP=\x80`\0\x81\x14a\x08\x1AW`@Q\x91P`\x1F\x19`?=\x01\x16\x82\x01`@R=\x82R=`\0` \x84\x01>a\x08\x1AV[`@\x80Q\x7F\x8Bs\xC3\xC6\x9B\xB8\xFE=Q.\xCCL\xF7Y\xCCy#\x9F{\x17\x9B\x0F\xFA\xCA\xA9\xA7]R+9@\x0F` \x82\x01R\x7Fl\x85\xC03~\xBA\x16a2\x7F\x94\xF3\xBFF\xC8\xA7\xF91\x1AV?M\\\x94\x83bV\x7F]\x8E\xD6\x0C\x91\x81\x01\x91\x90\x91R\x7F\xF9Dk\x8E\x93}\x86\xF0\xBC\x87\xCA\xC79#I\x16\x92\xB1#\xCA_\x87a\x90\x84\x94p7X j\xDF``\x82\x01RF`\x80\x82\x01\x81\x90Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x83\x16`\xA0\x83\x01R`\0\x91`\xC0\x01`@Q` \x81\x83\x03\x03\x81R\x90`@R\x80Q\x90` \x01 \x91PP\x91\x90PV[`@\x80Qa\x01\x80\x81\x01\x82R`\0\x80\x82R` \x80\x83\x01\x82\x90R\x92\x82\x01\x81\x90R``\x82\x01\x81\x90R`\x80\x82\x01\x81\x90R`\xA0\x82\x01\x81\x90R`\xC0\x82\x01\x81\x90R`\xE0\x82\x01\x81\x90Ra\x01\0\x82\x01\x81\x90Ra\x01 \x82\x01\x81\x90Ra\x01@\x82\x01\x81\x90Ra\x01`\x82\x01R\x90\x83\x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16a\n\nW`@Q\x7F\xEF\xC9\xCC\xDF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[`@Q\x80a\x01\x80\x01`@R\x80\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x84`\0\x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x84` \x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x84`@\x01Q\x81R` \x01\x84``\x01Q\x81R` \x01c\xFF\xFF\xFF\xFF\x80\x16\x81R` \x01\x84`\x80\x01Q\x81R` \x01\x84`\xA0\x01Q\x81R` \x01\x7F\xF3\xB2wr\x8B?\xEEt\x94\x81\xEB>\x0B;H\x98\r\xBB\xABxe\x8F\xC4\x19\x02\\\xB1n\xEE4gu\x81R` \x01\x84`\xE0\x01Q\x15\x15\x81R` \x01\x7FZ(\xE96;\xB9B\xB69'\0b\xAAk\xB2\x95\xF44\xBC\xDF\xC4,\x97&{\xF0\x03\xF2r\x06\r\xC9\x81R` \x01\x7FZ(\xE96;\xB9B\xB69'\0b\xAAk\xB2\x95\xF44\xBC\xDF\xC4,\x97&{\xF0\x03\xF2r\x06\r\xC9\x81RP\x90P\x92\x91PPV[`\0\x84s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x7F\xCF_\x9D\xE2\x98A2&R\x03\xB5\xC35\xB2W'p,\xA7rb\xFFb.\x13k\xAAsb\xBF\x1D\xA9\x85\x85\x85`@Qa\x0Bv\x93\x92\x91\x90a\x16vV[`@Q\x80\x91\x03\x90\xA2PPP\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x01\x80Q\x7F\xD5\xA2[\xA2\xE9p\x94\xAD}\x83\xDC(\xA6W-\xA7\x97\xD6\xB3\xE7\xFCfc\xBD\x93\xEF\xB7\x89\xFC\x17\xE4\x89\x82Ra\x01\xA0\x82 \x91R`@Q\x7F\x19\x01\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\x02\x82\x01R`\"\x81\x01\x91\x90\x91R`B\x90 \x91\x90PV[`\0a\x0Ca\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0a\x06-6\x86\x90\x03\x86\x01\x86a\x14\xB1V[\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x81\x01\x80Q\x7F\xD5\xA2[\xA2\xE9p\x94\xAD}\x83\xDC(\xA6W-\xA7\x97\xD6\xB3\xE7\xFCfc\xBD\x93\xEF\xB7\x89\xFC\x17\xE4\x89\x82Ra\x01\xA0\x82 \x91R`@\x80Q\x7F\x19\x01\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\x02\x82\x01R`\"\x81\x01\x92\x90\x92R`B\x90\x91 `\0\x81\x81R` \x81\x81R\x90\x83\x90 \x83Q\x80\x85\x01\x90\x94RTs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x16\x80\x86Rt\x01\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90\x92\x04c\xFF\xFF\xFF\xFF\x16\x92\x85\x01\x83\x90R\x94\x95P\x91\x93B\x90\x91\x10\x15\x91\x14\x80a\r\x8DWP\x81Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x15[\x80a\r\xB7WP\x80\x80\x15a\r\xB7WP\x81Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x163\x14\x15[\x15a\r\xFFW\x84\x15a\r\xF7W`@Q\x7F\xF8\xCCp\xCE\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x81\x01\x84\x90R`$\x01a\x06\x98V[PPPPPPV[`\0\x83\x81R` \x81\x81R`@\x80\x83 \x80T\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x17\x90U\x80Q`8\x80\x82R``\x82\x01\x90\x92R\x91\x82\x01\x81\x806\x837PPP`\xA0\x86\x01Q\x90\x91Pa\x0Ez\x90\x82\x90\x86\x900\x90a\x11IV[\x81\x15a\x0E\xBCW\x7F\xB8\xBA\xD1\x02\xAC\x8B\xBA\xCF\xEF1\xFF\x1C\x90n\xC6\xD9Q\xC20\xB4\xDC\xE7P\xBB\x03v\xB8\x12\xAD5\x85*\x81`@Qa\x0E\xAF\x91\x90a\x17\x90V[`@Q\x80\x91\x03\x90\xA1a\x0F\x0BV[3s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x7F\x19Rq\x06\x8A(\x81\x91\xE4\xB2e\xC6A\xA5k\x982\x91\x9Fi\xE9\xE7\xD6\xC2\xF3\x1B\xA4\x02x\xAE\xB8Z\x82`@Qa\x0F\x02\x91\x90a\x17\x90V[`@Q\x80\x91\x03\x90\xA2[`@Q\x7F$y\xFBn\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\0\x90s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x90c$y\xFBn\x90a\x0F\x80\x90\x85\x90`\x04\x01a\x17\x90V[` `@Q\x80\x83\x03\x81`\0\x87Z\xF1\x15\x80\x15a\x0F\x9FW=`\0\x80>=`\0\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x0F\xC3\x91\x90a\x17\xA3V[\x90P`\0\x80\x87``\x01Q\x83\x89`\xE0\x01Q\x02\x81a\x0F\xE1Wa\x0F\xE1a\x17\xBCV[\x04\x88`\xE0\x01Q\x03\x90P\x80\x83\x89``\x01Q\x03\x01\x91PP\x80G\x10\x15a\x10\xA4W`@Q\x7F.\x1A}M\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RG\x82\x03`\x04\x82\x01\x81\x90R\x90\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90c.\x1A}M\x90`$\x01`\0`@Q\x80\x83\x03\x81`\0\x87\x80;\x15\x80\x15a\x10\x8AW`\0\x80\xFD[PZ\xF1\x15\x80\x15a\x10\x9EW=`\0\x80>=`\0\xFD[PPPPP[\x84Q`@Q`\0\x91s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90\x83\x90\x83\x81\x81\x81\x85\x87Z\xF1\x92PPP=\x80`\0\x81\x14a\x10\xFDW`@Q\x91P`\x1F\x19`?=\x01\x16\x82\x01`@R=\x82R=`\0` \x84\x01>a\x11\x02V[``\x91P[PP\x90P\x80a\x11=W`@Q\x7Fm\x96?\x88\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[PPPPPPPPPPV[`8\x84Q\x14a\x11\xB4W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x19`$\x82\x01R\x7FGPv2: uid buffer overflow\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x06\x98V[`8\x84\x01R`4\x83\x01R` \x90\x91\x01RV[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\0R`A`\x04R`$`\0\xFD[`@Qa\x01 \x81\x01g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x82\x82\x10\x17\x15a\x12\x19Wa\x12\x19a\x11\xC6V[`@R\x90V[`@Q`\x1F\x82\x01\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x16\x81\x01g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x82\x82\x10\x17\x15a\x12fWa\x12fa\x11\xC6V[`@R\x91\x90PV[`\0\x80`@\x83\x85\x03\x12\x15a\x12\x81W`\0\x80\xFD[\x825\x91P` \x80\x84\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a\x12\xA1W`\0\x80\xFD[\x81\x86\x01\x91P\x86`\x1F\x83\x01\x12a\x12\xB5W`\0\x80\xFD[\x815\x81\x81\x11\x15a\x12\xC7Wa\x12\xC7a\x11\xC6V[a\x12\xF7\x84\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`\x1F\x84\x01\x16\x01a\x12\x1FV[\x91P\x80\x82R\x87\x84\x82\x85\x01\x01\x11\x15a\x13\rW`\0\x80\xFD[\x80\x84\x84\x01\x85\x84\x017`\0\x84\x82\x84\x01\x01RP\x80\x93PPPP\x92P\x92\x90PV[`\0a\x01 \x82\x84\x03\x12\x15a\x13>W`\0\x80\xFD[P\x91\x90PV[`\0\x80` \x83\x85\x03\x12\x15a\x13WW`\0\x80\xFD[\x825g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a\x13oW`\0\x80\xFD[\x81\x85\x01\x91P\x85`\x1F\x83\x01\x12a\x13\x83W`\0\x80\xFD[\x815\x81\x81\x11\x15a\x13\x92W`\0\x80\xFD[\x86` a\x01 \x83\x02\x85\x01\x01\x11\x15a\x13\xA8W`\0\x80\xFD[` \x92\x90\x92\x01\x96\x91\x95P\x90\x93PPPPV[`\0` \x82\x84\x03\x12\x15a\x13\xCCW`\0\x80\xFD[P5\x91\x90PV[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\0R`\x11`\x04R`$`\0\xFD[\x80\x82\x01\x80\x82\x11\x15a\x03\xD8Wa\x03\xD8a\x13\xD3V[\x805c\xFF\xFF\xFF\xFF\x81\x16\x81\x14a\x14)W`\0\x80\xFD[\x91\x90PV[`\0` \x82\x84\x03\x12\x15a\x14@W`\0\x80\xFD[a\x14I\x82a\x14\x15V[\x93\x92PPPV[\x805`\x07\x81\x90\x0B\x81\x14a\x14)W`\0\x80\xFD[`\0` \x82\x84\x03\x12\x15a\x14tW`\0\x80\xFD[a\x14I\x82a\x14PV[\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x16\x81\x14a\x14)W`\0\x80\xFD[\x805\x80\x15\x15\x81\x14a\x14)W`\0\x80\xFD[`\0a\x01 \x82\x84\x03\x12\x15a\x14\xC4W`\0\x80\xFD[a\x14\xCCa\x11\xF5V[a\x14\xD5\x83a\x14}V[\x81Ra\x14\xE3` \x84\x01a\x14}V[` \x82\x01R`@\x83\x015`@\x82\x01R``\x83\x015``\x82\x01R`\x80\x83\x015`\x80\x82\x01R`\xA0\x83\x015`\xA0\x82\x01Ra\x15\x1C`\xC0\x84\x01a\x14\x15V[`\xC0\x82\x01Ra\x15-`\xE0\x84\x01a\x14\xA1V[`\xE0\x82\x01Ra\x01\0a\x15@\x81\x85\x01a\x14PV[\x90\x82\x01R\x93\x92PPPV[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\0R`2`\x04R`$`\0\xFD[`\0\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x03a\x15\xABWa\x15\xABa\x13\xD3V[P`\x01\x01\x90V[`\0\x81Q\x80\x84R`\0[\x81\x81\x10\x15a\x15\xD8W` \x81\x85\x01\x81\x01Q\x86\x83\x01\x82\x01R\x01a\x15\xBCV[P`\0` \x82\x86\x01\x01R` \x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`\x1F\x83\x01\x16\x85\x01\x01\x91PP\x92\x91PPV[`\0\x81Q`\x02\x81\x10a\x16QW\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\0R`!`\x04R`$`\0\xFD[\x80\x84RP` \x82\x01Q`@` \x85\x01Ra\x16n`@\x85\x01\x82a\x15\xB2V[\x94\x93PPPPV[\x83Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R`\0a\x01\xC0` \x86\x01Qa\x16\xBC` \x85\x01\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90RV[P`@\x86\x01Qa\x16\xE4`@\x85\x01\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90RV[P``\x86\x01Q``\x84\x01R`\x80\x86\x01Q`\x80\x84\x01R`\xA0\x86\x01Qa\x17\x10`\xA0\x85\x01\x82c\xFF\xFF\xFF\xFF\x16\x90RV[P`\xC0\x86\x01Q`\xC0\x84\x01R`\xE0\x86\x01Q`\xE0\x84\x01Ra\x01\0\x80\x87\x01Q\x81\x85\x01RPa\x01 \x80\x87\x01Qa\x17E\x82\x86\x01\x82\x15\x15\x90RV[PPa\x01@\x86\x81\x01Q\x90\x84\x01Ra\x01`\x80\x87\x01Q\x90\x84\x01Ra\x01\x80\x83\x01\x81\x90Ra\x17q\x81\x84\x01\x86a\x16\x16V[\x90P\x82\x81\x03a\x01\xA0\x84\x01Ra\x17\x86\x81\x85a\x15\xB2V[\x96\x95PPPPPPV[` \x81R`\0a\x14I` \x83\x01\x84a\x15\xB2V[`\0` \x82\x84\x03\x12\x15a\x17\xB5W`\0\x80\xFD[PQ\x91\x90PV[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\0R`\x12`\x04R`$`\0\xFD\xFE\xA2dipfsX\"\x12 \xD3!\x9A$?\xB3\xB7h<lj\t\x18\x14H\x85\xC8U\x1F\x0F\xD8{\x19\xA0\xE75^\xD3\xD1\x0E\x93pdsolcC\0\x08\x10\x003",
    );
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Custom error with signature `EthTransferFailed()` and selector `0x6d963f88`.
    ```solidity
    error EthTransferFailed();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct EthTransferFailed;
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
        impl ::core::convert::From<EthTransferFailed> for UnderlyingRustTuple<'_> {
            fn from(value: EthTransferFailed) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for EthTransferFailed {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for EthTransferFailed {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "EthTransferFailed()";
            const SELECTOR: [u8; 4] = [109u8, 150u8, 63u8, 136u8];
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
                <Self::Parameters<'_> as alloy_sol_types::SolType>::abi_decode_sequence_validate(
                    data,
                )
                .map(Self::new)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Custom error with signature `IncorrectEthAmount()` and selector `0x8b6ebb4d`.
    ```solidity
    error IncorrectEthAmount();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct IncorrectEthAmount;
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
        impl ::core::convert::From<IncorrectEthAmount> for UnderlyingRustTuple<'_> {
            fn from(value: IncorrectEthAmount) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for IncorrectEthAmount {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for IncorrectEthAmount {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "IncorrectEthAmount()";
            const SELECTOR: [u8; 4] = [139u8, 110u8, 187u8, 77u8];
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
                <Self::Parameters<'_> as alloy_sol_types::SolType>::abi_decode_sequence_validate(
                    data,
                )
                .map(Self::new)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Custom error with signature `NotAllowedToInvalidateOrder(bytes32)` and selector `0xf8cc70ce`.
    ```solidity
    error NotAllowedToInvalidateOrder(bytes32 orderHash);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct NotAllowedToInvalidateOrder {
        #[allow(missing_docs)]
        pub orderHash: alloy_sol_types::private::FixedBytes<32>,
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
        type UnderlyingSolTuple<'a> = (alloy_sol_types::sol_data::FixedBytes<32>,);
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (alloy_sol_types::private::FixedBytes<32>,);
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
        impl ::core::convert::From<NotAllowedToInvalidateOrder> for UnderlyingRustTuple<'_> {
            fn from(value: NotAllowedToInvalidateOrder) -> Self {
                (value.orderHash,)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for NotAllowedToInvalidateOrder {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self { orderHash: tuple.0 }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for NotAllowedToInvalidateOrder {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "NotAllowedToInvalidateOrder(bytes32)";
            const SELECTOR: [u8; 4] = [248u8, 204u8, 112u8, 206u8];
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
                    > as alloy_sol_types::SolType>::tokenize(&self.orderHash),
                )
            }
            #[inline]
            fn abi_decode_raw_validate(data: &[u8]) -> alloy_sol_types::Result<Self> {
                <Self::Parameters<'_> as alloy_sol_types::SolType>::abi_decode_sequence_validate(
                    data,
                )
                .map(Self::new)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Custom error with signature `NotAllowedZeroSellAmount()` and selector `0xeaec5c9d`.
    ```solidity
    error NotAllowedZeroSellAmount();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct NotAllowedZeroSellAmount;
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
        impl ::core::convert::From<NotAllowedZeroSellAmount> for UnderlyingRustTuple<'_> {
            fn from(value: NotAllowedZeroSellAmount) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for NotAllowedZeroSellAmount {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for NotAllowedZeroSellAmount {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "NotAllowedZeroSellAmount()";
            const SELECTOR: [u8; 4] = [234u8, 236u8, 92u8, 157u8];
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
                <Self::Parameters<'_> as alloy_sol_types::SolType>::abi_decode_sequence_validate(
                    data,
                )
                .map(Self::new)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Custom error with signature `OrderIsAlreadyExpired()` and selector `0x89bb2601`.
    ```solidity
    error OrderIsAlreadyExpired();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct OrderIsAlreadyExpired;
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
        impl ::core::convert::From<OrderIsAlreadyExpired> for UnderlyingRustTuple<'_> {
            fn from(value: OrderIsAlreadyExpired) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for OrderIsAlreadyExpired {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for OrderIsAlreadyExpired {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "OrderIsAlreadyExpired()";
            const SELECTOR: [u8; 4] = [137u8, 187u8, 38u8, 1u8];
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
                <Self::Parameters<'_> as alloy_sol_types::SolType>::abi_decode_sequence_validate(
                    data,
                )
                .map(Self::new)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Custom error with signature `OrderIsAlreadyOwned(bytes32)` and selector `0x56a1d2b2`.
    ```solidity
    error OrderIsAlreadyOwned(bytes32 orderHash);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct OrderIsAlreadyOwned {
        #[allow(missing_docs)]
        pub orderHash: alloy_sol_types::private::FixedBytes<32>,
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
        type UnderlyingSolTuple<'a> = (alloy_sol_types::sol_data::FixedBytes<32>,);
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (alloy_sol_types::private::FixedBytes<32>,);
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
        impl ::core::convert::From<OrderIsAlreadyOwned> for UnderlyingRustTuple<'_> {
            fn from(value: OrderIsAlreadyOwned) -> Self {
                (value.orderHash,)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for OrderIsAlreadyOwned {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self { orderHash: tuple.0 }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for OrderIsAlreadyOwned {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "OrderIsAlreadyOwned(bytes32)";
            const SELECTOR: [u8; 4] = [86u8, 161u8, 210u8, 178u8];
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
                    > as alloy_sol_types::SolType>::tokenize(&self.orderHash),
                )
            }
            #[inline]
            fn abi_decode_raw_validate(data: &[u8]) -> alloy_sol_types::Result<Self> {
                <Self::Parameters<'_> as alloy_sol_types::SolType>::abi_decode_sequence_validate(
                    data,
                )
                .map(Self::new)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Custom error with signature `ReceiverMustBeSet()` and selector `0xefc9ccdf`.
    ```solidity
    error ReceiverMustBeSet();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct ReceiverMustBeSet;
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
        impl ::core::convert::From<ReceiverMustBeSet> for UnderlyingRustTuple<'_> {
            fn from(value: ReceiverMustBeSet) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for ReceiverMustBeSet {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for ReceiverMustBeSet {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "ReceiverMustBeSet()";
            const SELECTOR: [u8; 4] = [239u8, 201u8, 204u8, 223u8];
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
                <Self::Parameters<'_> as alloy_sol_types::SolType>::abi_decode_sequence_validate(
                    data,
                )
                .map(Self::new)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `OrderInvalidation(bytes)` and selector `0xb8bad102ac8bbacfef31ff1c906ec6d951c230b4dce750bb0376b812ad35852a`.
    ```solidity
    event OrderInvalidation(bytes orderUid);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct OrderInvalidation {
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
        use alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::SolEvent for OrderInvalidation {
            type DataTuple<'a> = (alloy_sol_types::sol_data::Bytes,);
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);
            const SIGNATURE: &'static str = "OrderInvalidation(bytes)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    184u8, 186u8, 209u8, 2u8, 172u8, 139u8, 186u8, 207u8, 239u8, 49u8, 255u8, 28u8,
                    144u8, 110u8, 198u8, 217u8, 81u8, 194u8, 48u8, 180u8, 220u8, 231u8, 80u8,
                    187u8, 3u8, 118u8, 184u8, 18u8, 173u8, 53u8, 133u8, 42u8,
                ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self { orderUid: data.0 }
            }
            #[inline]
            fn check_signature(
                topics: &<Self::TopicList as alloy_sol_types::SolType>::RustType,
            ) -> alloy_sol_types::Result<()> {
                if topics.0 != Self::SIGNATURE_HASH {
                    return Err(alloy_sol_types::Error::invalid_event_signature_hash(
                        Self::SIGNATURE,
                        topics.0,
                        Self::SIGNATURE_HASH,
                    ));
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
                out[0usize] = alloy_sol_types::abi::token::WordToken(Self::SIGNATURE_HASH);
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for OrderInvalidation {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&OrderInvalidation> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &OrderInvalidation) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive()]
    /**Event with signature `OrderPlacement(address,(address,address,address,uint256,uint256,uint32,bytes32,uint256,bytes32,bool,bytes32,bytes32),(uint8,bytes),bytes)` and selector `0xcf5f9de2984132265203b5c335b25727702ca77262ff622e136baa7362bf1da9`.
    ```solidity
    event OrderPlacement(address indexed sender, GPv2Order.Data order, ICoWSwapOnchainOrders.OnchainSignature signature, bytes data);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct OrderPlacement {
        #[allow(missing_docs)]
        pub sender: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub order: <GPv2Order::Data as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub signature:
            <ICoWSwapOnchainOrders::OnchainSignature as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub data: alloy_sol_types::private::Bytes,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::SolEvent for OrderPlacement {
            type DataTuple<'a> = (
                GPv2Order::Data,
                ICoWSwapOnchainOrders::OnchainSignature,
                alloy_sol_types::sol_data::Bytes,
            );
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "OrderPlacement(address,(address,address,address,uint256,uint256,uint32,bytes32,uint256,bytes32,bool,bytes32,bytes32),(uint8,bytes),bytes)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    207u8, 95u8, 157u8, 226u8, 152u8, 65u8, 50u8, 38u8, 82u8, 3u8, 181u8, 195u8,
                    53u8, 178u8, 87u8, 39u8, 112u8, 44u8, 167u8, 114u8, 98u8, 255u8, 98u8, 46u8,
                    19u8, 107u8, 170u8, 115u8, 98u8, 191u8, 29u8, 169u8,
                ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    sender: topics.1,
                    order: data.0,
                    signature: data.1,
                    data: data.2,
                }
            }
            #[inline]
            fn check_signature(
                topics: &<Self::TopicList as alloy_sol_types::SolType>::RustType,
            ) -> alloy_sol_types::Result<()> {
                if topics.0 != Self::SIGNATURE_HASH {
                    return Err(alloy_sol_types::Error::invalid_event_signature_hash(
                        Self::SIGNATURE,
                        topics.0,
                        Self::SIGNATURE_HASH,
                    ));
                }
                Ok(())
            }
            #[inline]
            fn tokenize_body(&self) -> Self::DataToken<'_> {
                (
                    <GPv2Order::Data as alloy_sol_types::SolType>::tokenize(&self.order),
                    <ICoWSwapOnchainOrders::OnchainSignature as alloy_sol_types::SolType>::tokenize(
                        &self.signature,
                    ),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.data,
                    ),
                )
            }
            #[inline]
            fn topics(&self) -> <Self::TopicList as alloy_sol_types::SolType>::RustType {
                (Self::SIGNATURE_HASH.into(), self.sender.clone())
            }
            #[inline]
            fn encode_topics_raw(
                &self,
                out: &mut [alloy_sol_types::abi::token::WordToken],
            ) -> alloy_sol_types::Result<()> {
                if out.len() < <Self::TopicList as alloy_sol_types::TopicList>::COUNT {
                    return Err(alloy_sol_types::Error::Overrun);
                }
                out[0usize] = alloy_sol_types::abi::token::WordToken(Self::SIGNATURE_HASH);
                out[1usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.sender,
                );
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for OrderPlacement {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&OrderPlacement> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &OrderPlacement) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `OrderRefund(bytes,address)` and selector `0x195271068a288191e4b265c641a56b9832919f69e9e7d6c2f31ba40278aeb85a`.
    ```solidity
    event OrderRefund(bytes orderUid, address indexed refunder);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct OrderRefund {
        #[allow(missing_docs)]
        pub orderUid: alloy_sol_types::private::Bytes,
        #[allow(missing_docs)]
        pub refunder: alloy_sol_types::private::Address,
    }
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    const _: () = {
        use alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::SolEvent for OrderRefund {
            type DataTuple<'a> = (alloy_sol_types::sol_data::Bytes,);
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "OrderRefund(bytes,address)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    25u8, 82u8, 113u8, 6u8, 138u8, 40u8, 129u8, 145u8, 228u8, 178u8, 101u8, 198u8,
                    65u8, 165u8, 107u8, 152u8, 50u8, 145u8, 159u8, 105u8, 233u8, 231u8, 214u8,
                    194u8, 243u8, 27u8, 164u8, 2u8, 120u8, 174u8, 184u8, 90u8,
                ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    orderUid: data.0,
                    refunder: topics.1,
                }
            }
            #[inline]
            fn check_signature(
                topics: &<Self::TopicList as alloy_sol_types::SolType>::RustType,
            ) -> alloy_sol_types::Result<()> {
                if topics.0 != Self::SIGNATURE_HASH {
                    return Err(alloy_sol_types::Error::invalid_event_signature_hash(
                        Self::SIGNATURE,
                        topics.0,
                        Self::SIGNATURE_HASH,
                    ));
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
                (Self::SIGNATURE_HASH.into(), self.refunder.clone())
            }
            #[inline]
            fn encode_topics_raw(
                &self,
                out: &mut [alloy_sol_types::abi::token::WordToken],
            ) -> alloy_sol_types::Result<()> {
                if out.len() < <Self::TopicList as alloy_sol_types::TopicList>::COUNT {
                    return Err(alloy_sol_types::Error::Overrun);
                }
                out[0usize] = alloy_sol_types::abi::token::WordToken(Self::SIGNATURE_HASH);
                out[1usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.refunder,
                );
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for OrderRefund {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&OrderRefund> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &OrderRefund) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    /**Constructor`.
    ```solidity
    constructor(address _cowSwapSettlement, address _wrappedNativeToken);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct constructorCall {
        #[allow(missing_docs)]
        pub _cowSwapSettlement: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub _wrappedNativeToken: alloy_sol_types::private::Address,
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
                    (value._cowSwapSettlement, value._wrappedNativeToken)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for constructorCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        _cowSwapSettlement: tuple.0,
                        _wrappedNativeToken: tuple.1,
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
                        &self._cowSwapSettlement,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self._wrappedNativeToken,
                    ),
                )
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `cowSwapSettlement()` and selector `0xec30bb88`.
    ```solidity
    function cowSwapSettlement() external view returns (address);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct cowSwapSettlementCall;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`cowSwapSettlement()`](cowSwapSettlementCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct cowSwapSettlementReturn {
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
            impl ::core::convert::From<cowSwapSettlementCall> for UnderlyingRustTuple<'_> {
                fn from(value: cowSwapSettlementCall) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for cowSwapSettlementCall {
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
            impl ::core::convert::From<cowSwapSettlementReturn> for UnderlyingRustTuple<'_> {
                fn from(value: cowSwapSettlementReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for cowSwapSettlementReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for cowSwapSettlementCall {
            type Parameters<'a> = ();
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::Address;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "cowSwapSettlement()";
            const SELECTOR: [u8; 4] = [236u8, 48u8, 187u8, 136u8];
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
                        let r: cowSwapSettlementReturn = r.into();
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
                    let r: cowSwapSettlementReturn = r.into();
                    r._0
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `createOrder((address,address,uint256,uint256,bytes32,uint256,uint32,bool,int64))` and selector `0x322bba21`.
    ```solidity
    function createOrder(EthFlowOrder.Data memory order) external payable returns (bytes32 orderHash);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct createOrderCall {
        #[allow(missing_docs)]
        pub order: <EthFlowOrder::Data as alloy_sol_types::SolType>::RustType,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`createOrder((address,address,uint256,uint256,bytes32,uint256,uint32,bool,int64))`](createOrderCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct createOrderReturn {
        #[allow(missing_docs)]
        pub orderHash: alloy_sol_types::private::FixedBytes<32>,
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
            type UnderlyingSolTuple<'a> = (EthFlowOrder::Data,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> =
                (<EthFlowOrder::Data as alloy_sol_types::SolType>::RustType,);
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
            impl ::core::convert::From<createOrderCall> for UnderlyingRustTuple<'_> {
                fn from(value: createOrderCall) -> Self {
                    (value.order,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for createOrderCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { order: tuple.0 }
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
            fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<createOrderReturn> for UnderlyingRustTuple<'_> {
                fn from(value: createOrderReturn) -> Self {
                    (value.orderHash,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for createOrderReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { orderHash: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for createOrderCall {
            type Parameters<'a> = (EthFlowOrder::Data,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::FixedBytes<32>;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::FixedBytes<32>,);
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str =
                "createOrder((address,address,uint256,uint256,bytes32,uint256,uint32,bool,int64))";
            const SELECTOR: [u8; 4] = [50u8, 43u8, 186u8, 33u8];
            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }
            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (<EthFlowOrder::Data as alloy_sol_types::SolType>::tokenize(
                    &self.order,
                ),)
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
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence(data).map(
                    |r| {
                        let r: createOrderReturn = r.into();
                        r.orderHash
                    },
                )
            }
            #[inline]
            fn abi_decode_returns_validate(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence_validate(
                    data,
                )
                .map(|r| {
                    let r: createOrderReturn = r.into();
                    r.orderHash
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `invalidateOrder((address,address,uint256,uint256,bytes32,uint256,uint32,bool,int64))` and selector `0x7bc41b96`.
    ```solidity
    function invalidateOrder(EthFlowOrder.Data memory order) external;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct invalidateOrderCall {
        #[allow(missing_docs)]
        pub order: <EthFlowOrder::Data as alloy_sol_types::SolType>::RustType,
    }
    ///Container type for the return parameters of the [`invalidateOrder((address,address,uint256,uint256,bytes32,uint256,uint32,bool,int64))`](invalidateOrderCall) function.
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
        use alloy_sol_types;
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (EthFlowOrder::Data,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> =
                (<EthFlowOrder::Data as alloy_sol_types::SolType>::RustType,);
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
            impl ::core::convert::From<invalidateOrderCall> for UnderlyingRustTuple<'_> {
                fn from(value: invalidateOrderCall) -> Self {
                    (value.order,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for invalidateOrderCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { order: tuple.0 }
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
            fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<invalidateOrderReturn> for UnderlyingRustTuple<'_> {
                fn from(value: invalidateOrderReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for invalidateOrderReturn {
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
            type Parameters<'a> = (EthFlowOrder::Data,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = invalidateOrderReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "invalidateOrder((address,address,uint256,uint256,bytes32,uint256,uint32,bool,int64))";
            const SELECTOR: [u8; 4] = [123u8, 196u8, 27u8, 150u8];
            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }
            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (<EthFlowOrder::Data as alloy_sol_types::SolType>::tokenize(
                    &self.order,
                ),)
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                invalidateOrderReturn::_tokenize(ret)
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
    /**Function with signature `invalidateOrdersIgnoringNotAllowed((address,address,uint256,uint256,bytes32,uint256,uint32,bool,int64)[])` and selector `0x4cb76498`.
    ```solidity
    function invalidateOrdersIgnoringNotAllowed(EthFlowOrder.Data[] memory orderArray) external;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct invalidateOrdersIgnoringNotAllowedCall {
        #[allow(missing_docs)]
        pub orderArray: alloy_sol_types::private::Vec<
            <EthFlowOrder::Data as alloy_sol_types::SolType>::RustType,
        >,
    }
    ///Container type for the return parameters of the [`invalidateOrdersIgnoringNotAllowed((address,address,uint256,uint256,bytes32,uint256,uint32,bool,int64)[])`](invalidateOrdersIgnoringNotAllowedCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct invalidateOrdersIgnoringNotAllowedReturn {}
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
            type UnderlyingSolTuple<'a> = (alloy_sol_types::sol_data::Array<EthFlowOrder::Data>,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Vec<
                    <EthFlowOrder::Data as alloy_sol_types::SolType>::RustType,
                >,
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
            impl ::core::convert::From<invalidateOrdersIgnoringNotAllowedCall> for UnderlyingRustTuple<'_> {
                fn from(value: invalidateOrdersIgnoringNotAllowedCall) -> Self {
                    (value.orderArray,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for invalidateOrdersIgnoringNotAllowedCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        orderArray: tuple.0,
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
            fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<invalidateOrdersIgnoringNotAllowedReturn> for UnderlyingRustTuple<'_> {
                fn from(value: invalidateOrdersIgnoringNotAllowedReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for invalidateOrdersIgnoringNotAllowedReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl invalidateOrdersIgnoringNotAllowedReturn {
            fn _tokenize(
                &self,
            ) -> <invalidateOrdersIgnoringNotAllowedCall as alloy_sol_types::SolCall>::ReturnToken<'_>
            {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for invalidateOrdersIgnoringNotAllowedCall {
            type Parameters<'a> = (alloy_sol_types::sol_data::Array<EthFlowOrder::Data>,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = invalidateOrdersIgnoringNotAllowedReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "invalidateOrdersIgnoringNotAllowed((address,address,uint256,uint256,bytes32,uint256,uint32,bool,int64)[])";
            const SELECTOR: [u8; 4] = [76u8, 183u8, 100u8, 152u8];
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
                        EthFlowOrder::Data,
                    > as alloy_sol_types::SolType>::tokenize(&self.orderArray),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                invalidateOrdersIgnoringNotAllowedReturn::_tokenize(ret)
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
    /**Function with signature `isValidSignature(bytes32,bytes)` and selector `0x1626ba7e`.
    ```solidity
    function isValidSignature(bytes32 orderHash, bytes memory) external view returns (bytes4);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct isValidSignatureCall {
        #[allow(missing_docs)]
        pub orderHash: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub _1: alloy_sol_types::private::Bytes,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`isValidSignature(bytes32,bytes)`](isValidSignatureCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct isValidSignatureReturn {
        #[allow(missing_docs)]
        pub _0: alloy_sol_types::private::FixedBytes<4>,
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
                alloy_sol_types::sol_data::Bytes,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::FixedBytes<32>,
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
            impl ::core::convert::From<isValidSignatureCall> for UnderlyingRustTuple<'_> {
                fn from(value: isValidSignatureCall) -> Self {
                    (value.orderHash, value._1)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for isValidSignatureCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        orderHash: tuple.0,
                        _1: tuple.1,
                    }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (alloy_sol_types::sol_data::FixedBytes<4>,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (alloy_sol_types::private::FixedBytes<4>,);
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
            impl ::core::convert::From<isValidSignatureReturn> for UnderlyingRustTuple<'_> {
                fn from(value: isValidSignatureReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for isValidSignatureReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for isValidSignatureCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Bytes,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::FixedBytes<4>;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::FixedBytes<4>,);
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "isValidSignature(bytes32,bytes)";
            const SELECTOR: [u8; 4] = [22u8, 38u8, 186u8, 126u8];
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
                    > as alloy_sol_types::SolType>::tokenize(&self.orderHash),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self._1,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                (
                    <alloy_sol_types::sol_data::FixedBytes<
                        4,
                    > as alloy_sol_types::SolType>::tokenize(ret),
                )
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence(data).map(
                    |r| {
                        let r: isValidSignatureReturn = r.into();
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
                    let r: isValidSignatureReturn = r.into();
                    r._0
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `orders(bytes32)` and selector `0x9c3f1e90`.
    ```solidity
    function orders(bytes32) external view returns (address owner, uint32 validTo);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct ordersCall(pub alloy_sol_types::private::FixedBytes<32>);
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`orders(bytes32)`](ordersCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct ordersReturn {
        #[allow(missing_docs)]
        pub owner: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub validTo: u32,
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
            type UnderlyingSolTuple<'a> = (alloy_sol_types::sol_data::FixedBytes<32>,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (alloy_sol_types::private::FixedBytes<32>,);
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
            impl ::core::convert::From<ordersCall> for UnderlyingRustTuple<'_> {
                fn from(value: ordersCall) -> Self {
                    (value.0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for ordersCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self(tuple.0)
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<32>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (alloy_sol_types::private::Address, u32);
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
            impl ::core::convert::From<ordersReturn> for UnderlyingRustTuple<'_> {
                fn from(value: ordersReturn) -> Self {
                    (value.owner, value.validTo)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for ordersReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        owner: tuple.0,
                        validTo: tuple.1,
                    }
                }
            }
        }
        impl ordersReturn {
            fn _tokenize(&self) -> <ordersCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.owner,
                    ),
                    <alloy_sol_types::sol_data::Uint<32> as alloy_sol_types::SolType>::tokenize(
                        &self.validTo,
                    ),
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for ordersCall {
            type Parameters<'a> = (alloy_sol_types::sol_data::FixedBytes<32>,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = ordersReturn;
            type ReturnTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<32>,
            );
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "orders(bytes32)";
            const SELECTOR: [u8; 4] = [156u8, 63u8, 30u8, 144u8];
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
                    > as alloy_sol_types::SolType>::tokenize(&self.0),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                ordersReturn::_tokenize(ret)
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
    /**Function with signature `unwrap(uint256)` and selector `0xde0e9a3e`.
    ```solidity
    function unwrap(uint256 amount) external;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct unwrapCall {
        #[allow(missing_docs)]
        pub amount: alloy_sol_types::private::primitives::aliases::U256,
    }
    ///Container type for the return parameters of the [`unwrap(uint256)`](unwrapCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct unwrapReturn {}
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
            impl ::core::convert::From<unwrapCall> for UnderlyingRustTuple<'_> {
                fn from(value: unwrapCall) -> Self {
                    (value.amount,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for unwrapCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { amount: tuple.0 }
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
            fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<unwrapReturn> for UnderlyingRustTuple<'_> {
                fn from(value: unwrapReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for unwrapReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl unwrapReturn {
            fn _tokenize(&self) -> <unwrapCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for unwrapCall {
            type Parameters<'a> = (alloy_sol_types::sol_data::Uint<256>,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = unwrapReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "unwrap(uint256)";
            const SELECTOR: [u8; 4] = [222u8, 14u8, 154u8, 62u8];
            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }
            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.amount,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                unwrapReturn::_tokenize(ret)
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
    /**Function with signature `wrap(uint256)` and selector `0xea598cb0`.
    ```solidity
    function wrap(uint256 amount) external;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct wrapCall {
        #[allow(missing_docs)]
        pub amount: alloy_sol_types::private::primitives::aliases::U256,
    }
    ///Container type for the return parameters of the [`wrap(uint256)`](wrapCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct wrapReturn {}
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
            impl ::core::convert::From<wrapCall> for UnderlyingRustTuple<'_> {
                fn from(value: wrapCall) -> Self {
                    (value.amount,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for wrapCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { amount: tuple.0 }
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
            fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<wrapReturn> for UnderlyingRustTuple<'_> {
                fn from(value: wrapReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for wrapReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl wrapReturn {
            fn _tokenize(&self) -> <wrapCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for wrapCall {
            type Parameters<'a> = (alloy_sol_types::sol_data::Uint<256>,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = wrapReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "wrap(uint256)";
            const SELECTOR: [u8; 4] = [234u8, 89u8, 140u8, 176u8];
            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }
            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.amount,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                wrapReturn::_tokenize(ret)
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
    /**Function with signature `wrapAll()` and selector `0x4c84c1c8`.
    ```solidity
    function wrapAll() external;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct wrapAllCall;
    ///Container type for the return parameters of the [`wrapAll()`](wrapAllCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct wrapAllReturn {}
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
            impl ::core::convert::From<wrapAllCall> for UnderlyingRustTuple<'_> {
                fn from(value: wrapAllCall) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for wrapAllCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self
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
            fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<wrapAllReturn> for UnderlyingRustTuple<'_> {
                fn from(value: wrapAllReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for wrapAllReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl wrapAllReturn {
            fn _tokenize(&self) -> <wrapAllCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for wrapAllCall {
            type Parameters<'a> = ();
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = wrapAllReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "wrapAll()";
            const SELECTOR: [u8; 4] = [76u8, 132u8, 193u8, 200u8];
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
                wrapAllReturn::_tokenize(ret)
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
    /**Function with signature `wrappedNativeToken()` and selector `0x17fcb39b`.
    ```solidity
    function wrappedNativeToken() external view returns (address);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct wrappedNativeTokenCall;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`wrappedNativeToken()`](wrappedNativeTokenCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct wrappedNativeTokenReturn {
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
            impl ::core::convert::From<wrappedNativeTokenCall> for UnderlyingRustTuple<'_> {
                fn from(value: wrappedNativeTokenCall) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for wrappedNativeTokenCall {
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
            impl ::core::convert::From<wrappedNativeTokenReturn> for UnderlyingRustTuple<'_> {
                fn from(value: wrappedNativeTokenReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for wrappedNativeTokenReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for wrappedNativeTokenCall {
            type Parameters<'a> = ();
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::Address;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "wrappedNativeToken()";
            const SELECTOR: [u8; 4] = [23u8, 252u8, 179u8, 155u8];
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
                        let r: wrappedNativeTokenReturn = r.into();
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
                    let r: wrappedNativeTokenReturn = r.into();
                    r._0
                })
            }
        }
    };
    ///Container for all the [`CoWSwapEthFlow`](self) function calls.
    #[derive(Clone)]
    pub enum CoWSwapEthFlowCalls {
        #[allow(missing_docs)]
        cowSwapSettlement(cowSwapSettlementCall),
        #[allow(missing_docs)]
        createOrder(createOrderCall),
        #[allow(missing_docs)]
        invalidateOrder(invalidateOrderCall),
        #[allow(missing_docs)]
        invalidateOrdersIgnoringNotAllowed(invalidateOrdersIgnoringNotAllowedCall),
        #[allow(missing_docs)]
        isValidSignature(isValidSignatureCall),
        #[allow(missing_docs)]
        orders(ordersCall),
        #[allow(missing_docs)]
        unwrap(unwrapCall),
        #[allow(missing_docs)]
        wrap(wrapCall),
        #[allow(missing_docs)]
        wrapAll(wrapAllCall),
        #[allow(missing_docs)]
        wrappedNativeToken(wrappedNativeTokenCall),
    }
    impl CoWSwapEthFlowCalls {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the variants.
        /// No guarantees are made about the order of the selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 4usize]] = &[
            [22u8, 38u8, 186u8, 126u8],
            [23u8, 252u8, 179u8, 155u8],
            [50u8, 43u8, 186u8, 33u8],
            [76u8, 132u8, 193u8, 200u8],
            [76u8, 183u8, 100u8, 152u8],
            [123u8, 196u8, 27u8, 150u8],
            [156u8, 63u8, 30u8, 144u8],
            [222u8, 14u8, 154u8, 62u8],
            [234u8, 89u8, 140u8, 176u8],
            [236u8, 48u8, 187u8, 136u8],
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(isValidSignature),
            ::core::stringify!(wrappedNativeToken),
            ::core::stringify!(createOrder),
            ::core::stringify!(wrapAll),
            ::core::stringify!(invalidateOrdersIgnoringNotAllowed),
            ::core::stringify!(invalidateOrder),
            ::core::stringify!(orders),
            ::core::stringify!(unwrap),
            ::core::stringify!(wrap),
            ::core::stringify!(cowSwapSettlement),
        ];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <isValidSignatureCall as alloy_sol_types::SolCall>::SIGNATURE,
            <wrappedNativeTokenCall as alloy_sol_types::SolCall>::SIGNATURE,
            <createOrderCall as alloy_sol_types::SolCall>::SIGNATURE,
            <wrapAllCall as alloy_sol_types::SolCall>::SIGNATURE,
            <invalidateOrdersIgnoringNotAllowedCall as alloy_sol_types::SolCall>::SIGNATURE,
            <invalidateOrderCall as alloy_sol_types::SolCall>::SIGNATURE,
            <ordersCall as alloy_sol_types::SolCall>::SIGNATURE,
            <unwrapCall as alloy_sol_types::SolCall>::SIGNATURE,
            <wrapCall as alloy_sol_types::SolCall>::SIGNATURE,
            <cowSwapSettlementCall as alloy_sol_types::SolCall>::SIGNATURE,
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
    impl alloy_sol_types::SolInterface for CoWSwapEthFlowCalls {
        const NAME: &'static str = "CoWSwapEthFlowCalls";
        const MIN_DATA_LENGTH: usize = 0usize;
        const COUNT: usize = 10usize;
        #[inline]
        fn selector(&self) -> [u8; 4] {
            match self {
                Self::cowSwapSettlement(_) => {
                    <cowSwapSettlementCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::createOrder(_) => <createOrderCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::invalidateOrder(_) => {
                    <invalidateOrderCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::invalidateOrdersIgnoringNotAllowed(_) => {
                    <invalidateOrdersIgnoringNotAllowedCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::isValidSignature(_) => {
                    <isValidSignatureCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::orders(_) => <ordersCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::unwrap(_) => <unwrapCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::wrap(_) => <wrapCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::wrapAll(_) => <wrapAllCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::wrappedNativeToken(_) => {
                    <wrappedNativeTokenCall as alloy_sol_types::SolCall>::SELECTOR
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
            static DECODE_SHIMS: &[fn(&[u8]) -> alloy_sol_types::Result<CoWSwapEthFlowCalls>] = &[
                {
                    fn isValidSignature(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CoWSwapEthFlowCalls> {
                        <isValidSignatureCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(CoWSwapEthFlowCalls::isValidSignature)
                    }
                    isValidSignature
                },
                {
                    fn wrappedNativeToken(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CoWSwapEthFlowCalls> {
                        <wrappedNativeTokenCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(CoWSwapEthFlowCalls::wrappedNativeToken)
                    }
                    wrappedNativeToken
                },
                {
                    fn createOrder(data: &[u8]) -> alloy_sol_types::Result<CoWSwapEthFlowCalls> {
                        <createOrderCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(CoWSwapEthFlowCalls::createOrder)
                    }
                    createOrder
                },
                {
                    fn wrapAll(data: &[u8]) -> alloy_sol_types::Result<CoWSwapEthFlowCalls> {
                        <wrapAllCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(CoWSwapEthFlowCalls::wrapAll)
                    }
                    wrapAll
                },
                {
                    fn invalidateOrdersIgnoringNotAllowed(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CoWSwapEthFlowCalls> {
                        <invalidateOrdersIgnoringNotAllowedCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(CoWSwapEthFlowCalls::invalidateOrdersIgnoringNotAllowed)
                    }
                    invalidateOrdersIgnoringNotAllowed
                },
                {
                    fn invalidateOrder(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CoWSwapEthFlowCalls> {
                        <invalidateOrderCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(CoWSwapEthFlowCalls::invalidateOrder)
                    }
                    invalidateOrder
                },
                {
                    fn orders(data: &[u8]) -> alloy_sol_types::Result<CoWSwapEthFlowCalls> {
                        <ordersCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(CoWSwapEthFlowCalls::orders)
                    }
                    orders
                },
                {
                    fn unwrap(data: &[u8]) -> alloy_sol_types::Result<CoWSwapEthFlowCalls> {
                        <unwrapCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(CoWSwapEthFlowCalls::unwrap)
                    }
                    unwrap
                },
                {
                    fn wrap(data: &[u8]) -> alloy_sol_types::Result<CoWSwapEthFlowCalls> {
                        <wrapCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(CoWSwapEthFlowCalls::wrap)
                    }
                    wrap
                },
                {
                    fn cowSwapSettlement(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CoWSwapEthFlowCalls> {
                        <cowSwapSettlementCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(CoWSwapEthFlowCalls::cowSwapSettlement)
                    }
                    cowSwapSettlement
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
                -> alloy_sol_types::Result<CoWSwapEthFlowCalls>] = &[
                {
                    fn isValidSignature(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CoWSwapEthFlowCalls> {
                        <isValidSignatureCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(CoWSwapEthFlowCalls::isValidSignature)
                    }
                    isValidSignature
                },
                {
                    fn wrappedNativeToken(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CoWSwapEthFlowCalls> {
                        <wrappedNativeTokenCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CoWSwapEthFlowCalls::wrappedNativeToken)
                    }
                    wrappedNativeToken
                },
                {
                    fn createOrder(data: &[u8]) -> alloy_sol_types::Result<CoWSwapEthFlowCalls> {
                        <createOrderCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(CoWSwapEthFlowCalls::createOrder)
                    }
                    createOrder
                },
                {
                    fn wrapAll(data: &[u8]) -> alloy_sol_types::Result<CoWSwapEthFlowCalls> {
                        <wrapAllCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(CoWSwapEthFlowCalls::wrapAll)
                    }
                    wrapAll
                },
                {
                    fn invalidateOrdersIgnoringNotAllowed(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CoWSwapEthFlowCalls> {
                        <invalidateOrdersIgnoringNotAllowedCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CoWSwapEthFlowCalls::invalidateOrdersIgnoringNotAllowed)
                    }
                    invalidateOrdersIgnoringNotAllowed
                },
                {
                    fn invalidateOrder(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CoWSwapEthFlowCalls> {
                        <invalidateOrderCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(CoWSwapEthFlowCalls::invalidateOrder)
                    }
                    invalidateOrder
                },
                {
                    fn orders(data: &[u8]) -> alloy_sol_types::Result<CoWSwapEthFlowCalls> {
                        <ordersCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(CoWSwapEthFlowCalls::orders)
                    }
                    orders
                },
                {
                    fn unwrap(data: &[u8]) -> alloy_sol_types::Result<CoWSwapEthFlowCalls> {
                        <unwrapCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(CoWSwapEthFlowCalls::unwrap)
                    }
                    unwrap
                },
                {
                    fn wrap(data: &[u8]) -> alloy_sol_types::Result<CoWSwapEthFlowCalls> {
                        <wrapCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(CoWSwapEthFlowCalls::wrap)
                    }
                    wrap
                },
                {
                    fn cowSwapSettlement(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CoWSwapEthFlowCalls> {
                        <cowSwapSettlementCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CoWSwapEthFlowCalls::cowSwapSettlement)
                    }
                    cowSwapSettlement
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
                Self::cowSwapSettlement(inner) => {
                    <cowSwapSettlementCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::createOrder(inner) => {
                    <createOrderCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::invalidateOrder(inner) => {
                    <invalidateOrderCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::invalidateOrdersIgnoringNotAllowed(inner) => {
                    <invalidateOrdersIgnoringNotAllowedCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::isValidSignature(inner) => {
                    <isValidSignatureCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::orders(inner) => {
                    <ordersCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::unwrap(inner) => {
                    <unwrapCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::wrap(inner) => {
                    <wrapCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::wrapAll(inner) => {
                    <wrapAllCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::wrappedNativeToken(inner) => {
                    <wrappedNativeTokenCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
            }
        }
        #[inline]
        fn abi_encode_raw(&self, out: &mut alloy_sol_types::private::Vec<u8>) {
            match self {
                Self::cowSwapSettlement(inner) => {
                    <cowSwapSettlementCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::createOrder(inner) => {
                    <createOrderCall as alloy_sol_types::SolCall>::abi_encode_raw(
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
                Self::invalidateOrdersIgnoringNotAllowed(inner) => {
                    <invalidateOrdersIgnoringNotAllowedCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::isValidSignature(inner) => {
                    <isValidSignatureCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::orders(inner) => {
                    <ordersCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::unwrap(inner) => {
                    <unwrapCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::wrap(inner) => {
                    <wrapCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::wrapAll(inner) => {
                    <wrapAllCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::wrappedNativeToken(inner) => {
                    <wrappedNativeTokenCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
            }
        }
    }
    ///Container for all the [`CoWSwapEthFlow`](self) custom errors.
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    pub enum CoWSwapEthFlowErrors {
        #[allow(missing_docs)]
        EthTransferFailed(EthTransferFailed),
        #[allow(missing_docs)]
        IncorrectEthAmount(IncorrectEthAmount),
        #[allow(missing_docs)]
        NotAllowedToInvalidateOrder(NotAllowedToInvalidateOrder),
        #[allow(missing_docs)]
        NotAllowedZeroSellAmount(NotAllowedZeroSellAmount),
        #[allow(missing_docs)]
        OrderIsAlreadyExpired(OrderIsAlreadyExpired),
        #[allow(missing_docs)]
        OrderIsAlreadyOwned(OrderIsAlreadyOwned),
        #[allow(missing_docs)]
        ReceiverMustBeSet(ReceiverMustBeSet),
    }
    impl CoWSwapEthFlowErrors {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the variants.
        /// No guarantees are made about the order of the selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 4usize]] = &[
            [86u8, 161u8, 210u8, 178u8],
            [109u8, 150u8, 63u8, 136u8],
            [137u8, 187u8, 38u8, 1u8],
            [139u8, 110u8, 187u8, 77u8],
            [234u8, 236u8, 92u8, 157u8],
            [239u8, 201u8, 204u8, 223u8],
            [248u8, 204u8, 112u8, 206u8],
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(OrderIsAlreadyOwned),
            ::core::stringify!(EthTransferFailed),
            ::core::stringify!(OrderIsAlreadyExpired),
            ::core::stringify!(IncorrectEthAmount),
            ::core::stringify!(NotAllowedZeroSellAmount),
            ::core::stringify!(ReceiverMustBeSet),
            ::core::stringify!(NotAllowedToInvalidateOrder),
        ];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <OrderIsAlreadyOwned as alloy_sol_types::SolError>::SIGNATURE,
            <EthTransferFailed as alloy_sol_types::SolError>::SIGNATURE,
            <OrderIsAlreadyExpired as alloy_sol_types::SolError>::SIGNATURE,
            <IncorrectEthAmount as alloy_sol_types::SolError>::SIGNATURE,
            <NotAllowedZeroSellAmount as alloy_sol_types::SolError>::SIGNATURE,
            <ReceiverMustBeSet as alloy_sol_types::SolError>::SIGNATURE,
            <NotAllowedToInvalidateOrder as alloy_sol_types::SolError>::SIGNATURE,
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
    impl alloy_sol_types::SolInterface for CoWSwapEthFlowErrors {
        const NAME: &'static str = "CoWSwapEthFlowErrors";
        const MIN_DATA_LENGTH: usize = 0usize;
        const COUNT: usize = 7usize;
        #[inline]
        fn selector(&self) -> [u8; 4] {
            match self {
                Self::EthTransferFailed(_) => {
                    <EthTransferFailed as alloy_sol_types::SolError>::SELECTOR
                }
                Self::IncorrectEthAmount(_) => {
                    <IncorrectEthAmount as alloy_sol_types::SolError>::SELECTOR
                }
                Self::NotAllowedToInvalidateOrder(_) => {
                    <NotAllowedToInvalidateOrder as alloy_sol_types::SolError>::SELECTOR
                }
                Self::NotAllowedZeroSellAmount(_) => {
                    <NotAllowedZeroSellAmount as alloy_sol_types::SolError>::SELECTOR
                }
                Self::OrderIsAlreadyExpired(_) => {
                    <OrderIsAlreadyExpired as alloy_sol_types::SolError>::SELECTOR
                }
                Self::OrderIsAlreadyOwned(_) => {
                    <OrderIsAlreadyOwned as alloy_sol_types::SolError>::SELECTOR
                }
                Self::ReceiverMustBeSet(_) => {
                    <ReceiverMustBeSet as alloy_sol_types::SolError>::SELECTOR
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
            static DECODE_SHIMS: &[fn(&[u8]) -> alloy_sol_types::Result<CoWSwapEthFlowErrors>] = &[
                {
                    fn OrderIsAlreadyOwned(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CoWSwapEthFlowErrors> {
                        <OrderIsAlreadyOwned as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(CoWSwapEthFlowErrors::OrderIsAlreadyOwned)
                    }
                    OrderIsAlreadyOwned
                },
                {
                    fn EthTransferFailed(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CoWSwapEthFlowErrors> {
                        <EthTransferFailed as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(CoWSwapEthFlowErrors::EthTransferFailed)
                    }
                    EthTransferFailed
                },
                {
                    fn OrderIsAlreadyExpired(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CoWSwapEthFlowErrors> {
                        <OrderIsAlreadyExpired as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(CoWSwapEthFlowErrors::OrderIsAlreadyExpired)
                    }
                    OrderIsAlreadyExpired
                },
                {
                    fn IncorrectEthAmount(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CoWSwapEthFlowErrors> {
                        <IncorrectEthAmount as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(CoWSwapEthFlowErrors::IncorrectEthAmount)
                    }
                    IncorrectEthAmount
                },
                {
                    fn NotAllowedZeroSellAmount(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CoWSwapEthFlowErrors> {
                        <NotAllowedZeroSellAmount as alloy_sol_types::SolError>::abi_decode_raw(
                            data,
                        )
                        .map(CoWSwapEthFlowErrors::NotAllowedZeroSellAmount)
                    }
                    NotAllowedZeroSellAmount
                },
                {
                    fn ReceiverMustBeSet(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CoWSwapEthFlowErrors> {
                        <ReceiverMustBeSet as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(CoWSwapEthFlowErrors::ReceiverMustBeSet)
                    }
                    ReceiverMustBeSet
                },
                {
                    fn NotAllowedToInvalidateOrder(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CoWSwapEthFlowErrors> {
                        <NotAllowedToInvalidateOrder as alloy_sol_types::SolError>::abi_decode_raw(
                            data,
                        )
                        .map(CoWSwapEthFlowErrors::NotAllowedToInvalidateOrder)
                    }
                    NotAllowedToInvalidateOrder
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
                -> alloy_sol_types::Result<CoWSwapEthFlowErrors>] = &[
                {
                    fn OrderIsAlreadyOwned(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CoWSwapEthFlowErrors> {
                        <OrderIsAlreadyOwned as alloy_sol_types::SolError>::abi_decode_raw_validate(
                            data,
                        )
                        .map(CoWSwapEthFlowErrors::OrderIsAlreadyOwned)
                    }
                    OrderIsAlreadyOwned
                },
                {
                    fn EthTransferFailed(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CoWSwapEthFlowErrors> {
                        <EthTransferFailed as alloy_sol_types::SolError>::abi_decode_raw_validate(
                            data,
                        )
                        .map(CoWSwapEthFlowErrors::EthTransferFailed)
                    }
                    EthTransferFailed
                },
                {
                    fn OrderIsAlreadyExpired(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CoWSwapEthFlowErrors> {
                        <OrderIsAlreadyExpired as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CoWSwapEthFlowErrors::OrderIsAlreadyExpired)
                    }
                    OrderIsAlreadyExpired
                },
                {
                    fn IncorrectEthAmount(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CoWSwapEthFlowErrors> {
                        <IncorrectEthAmount as alloy_sol_types::SolError>::abi_decode_raw_validate(
                            data,
                        )
                        .map(CoWSwapEthFlowErrors::IncorrectEthAmount)
                    }
                    IncorrectEthAmount
                },
                {
                    fn NotAllowedZeroSellAmount(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CoWSwapEthFlowErrors> {
                        <NotAllowedZeroSellAmount as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CoWSwapEthFlowErrors::NotAllowedZeroSellAmount)
                    }
                    NotAllowedZeroSellAmount
                },
                {
                    fn ReceiverMustBeSet(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CoWSwapEthFlowErrors> {
                        <ReceiverMustBeSet as alloy_sol_types::SolError>::abi_decode_raw_validate(
                            data,
                        )
                        .map(CoWSwapEthFlowErrors::ReceiverMustBeSet)
                    }
                    ReceiverMustBeSet
                },
                {
                    fn NotAllowedToInvalidateOrder(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CoWSwapEthFlowErrors> {
                        <NotAllowedToInvalidateOrder as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CoWSwapEthFlowErrors::NotAllowedToInvalidateOrder)
                    }
                    NotAllowedToInvalidateOrder
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
                Self::EthTransferFailed(inner) => {
                    <EthTransferFailed as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
                Self::IncorrectEthAmount(inner) => {
                    <IncorrectEthAmount as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
                Self::NotAllowedToInvalidateOrder(inner) => {
                    <NotAllowedToInvalidateOrder as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::NotAllowedZeroSellAmount(inner) => {
                    <NotAllowedZeroSellAmount as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
                Self::OrderIsAlreadyExpired(inner) => {
                    <OrderIsAlreadyExpired as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
                Self::OrderIsAlreadyOwned(inner) => {
                    <OrderIsAlreadyOwned as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
                Self::ReceiverMustBeSet(inner) => {
                    <ReceiverMustBeSet as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
            }
        }
        #[inline]
        fn abi_encode_raw(&self, out: &mut alloy_sol_types::private::Vec<u8>) {
            match self {
                Self::EthTransferFailed(inner) => {
                    <EthTransferFailed as alloy_sol_types::SolError>::abi_encode_raw(inner, out)
                }
                Self::IncorrectEthAmount(inner) => {
                    <IncorrectEthAmount as alloy_sol_types::SolError>::abi_encode_raw(inner, out)
                }
                Self::NotAllowedToInvalidateOrder(inner) => {
                    <NotAllowedToInvalidateOrder as alloy_sol_types::SolError>::abi_encode_raw(
                        inner, out,
                    )
                }
                Self::NotAllowedZeroSellAmount(inner) => {
                    <NotAllowedZeroSellAmount as alloy_sol_types::SolError>::abi_encode_raw(
                        inner, out,
                    )
                }
                Self::OrderIsAlreadyExpired(inner) => {
                    <OrderIsAlreadyExpired as alloy_sol_types::SolError>::abi_encode_raw(inner, out)
                }
                Self::OrderIsAlreadyOwned(inner) => {
                    <OrderIsAlreadyOwned as alloy_sol_types::SolError>::abi_encode_raw(inner, out)
                }
                Self::ReceiverMustBeSet(inner) => {
                    <ReceiverMustBeSet as alloy_sol_types::SolError>::abi_encode_raw(inner, out)
                }
            }
        }
    }
    ///Container for all the [`CoWSwapEthFlow`](self) events.
    #[derive(Clone)]
    pub enum CoWSwapEthFlowEvents {
        #[allow(missing_docs)]
        OrderInvalidation(OrderInvalidation),
        #[allow(missing_docs)]
        OrderPlacement(OrderPlacement),
        #[allow(missing_docs)]
        OrderRefund(OrderRefund),
    }
    impl CoWSwapEthFlowEvents {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the variants.
        /// No guarantees are made about the order of the selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 32usize]] = &[
            [
                25u8, 82u8, 113u8, 6u8, 138u8, 40u8, 129u8, 145u8, 228u8, 178u8, 101u8, 198u8,
                65u8, 165u8, 107u8, 152u8, 50u8, 145u8, 159u8, 105u8, 233u8, 231u8, 214u8, 194u8,
                243u8, 27u8, 164u8, 2u8, 120u8, 174u8, 184u8, 90u8,
            ],
            [
                184u8, 186u8, 209u8, 2u8, 172u8, 139u8, 186u8, 207u8, 239u8, 49u8, 255u8, 28u8,
                144u8, 110u8, 198u8, 217u8, 81u8, 194u8, 48u8, 180u8, 220u8, 231u8, 80u8, 187u8,
                3u8, 118u8, 184u8, 18u8, 173u8, 53u8, 133u8, 42u8,
            ],
            [
                207u8, 95u8, 157u8, 226u8, 152u8, 65u8, 50u8, 38u8, 82u8, 3u8, 181u8, 195u8, 53u8,
                178u8, 87u8, 39u8, 112u8, 44u8, 167u8, 114u8, 98u8, 255u8, 98u8, 46u8, 19u8, 107u8,
                170u8, 115u8, 98u8, 191u8, 29u8, 169u8,
            ],
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(OrderRefund),
            ::core::stringify!(OrderInvalidation),
            ::core::stringify!(OrderPlacement),
        ];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <OrderRefund as alloy_sol_types::SolEvent>::SIGNATURE,
            <OrderInvalidation as alloy_sol_types::SolEvent>::SIGNATURE,
            <OrderPlacement as alloy_sol_types::SolEvent>::SIGNATURE,
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
        pub fn name_by_selector(selector: [u8; 32usize]) -> ::core::option::Option<&'static str> {
            let sig = Self::signature_by_selector(selector)?;
            sig.split_once('(').map(|(name, _)| name)
        }
    }
    #[automatically_derived]
    impl alloy_sol_types::SolEventInterface for CoWSwapEthFlowEvents {
        const NAME: &'static str = "CoWSwapEthFlowEvents";
        const COUNT: usize = 3usize;
        fn decode_raw_log(
            topics: &[alloy_sol_types::Word],
            data: &[u8],
        ) -> alloy_sol_types::Result<Self> {
            match topics.first().copied() {
                Some(<OrderInvalidation as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <OrderInvalidation as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::OrderInvalidation)
                }
                Some(<OrderPlacement as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <OrderPlacement as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::OrderPlacement)
                }
                Some(<OrderRefund as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <OrderRefund as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::OrderRefund)
                }
                _ => alloy_sol_types::private::Err(alloy_sol_types::Error::InvalidLog {
                    name: <Self as alloy_sol_types::SolEventInterface>::NAME,
                    log: alloy_sol_types::private::Box::new(
                        alloy_sol_types::private::LogData::new_unchecked(
                            topics.to_vec(),
                            data.to_vec().into(),
                        ),
                    ),
                }),
            }
        }
    }
    #[automatically_derived]
    impl alloy_sol_types::private::IntoLogData for CoWSwapEthFlowEvents {
        fn to_log_data(&self) -> alloy_sol_types::private::LogData {
            match self {
                Self::OrderInvalidation(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::OrderPlacement(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::OrderRefund(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
            }
        }
        fn into_log_data(self) -> alloy_sol_types::private::LogData {
            match self {
                Self::OrderInvalidation(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::OrderPlacement(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::OrderRefund(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
            }
        }
    }
    use alloy_contract;
    /**Creates a new wrapper around an on-chain [`CoWSwapEthFlow`](self) contract instance.

    See the [wrapper's documentation](`CoWSwapEthFlowInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> CoWSwapEthFlowInstance<P, N> {
        CoWSwapEthFlowInstance::<P, N>::new(address, __provider)
    }
    /**Deploys this contract using the given `provider` and constructor arguments, if any.

    Returns a new instance of the contract, if the deployment was successful.

    For more fine-grained control over the deployment process, use [`deploy_builder`] instead.*/
    #[inline]
    pub fn deploy<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>(
        __provider: P,
        _cowSwapSettlement: alloy_sol_types::private::Address,
        _wrappedNativeToken: alloy_sol_types::private::Address,
    ) -> impl ::core::future::Future<Output = alloy_contract::Result<CoWSwapEthFlowInstance<P, N>>>
    {
        CoWSwapEthFlowInstance::<P, N>::deploy(__provider, _cowSwapSettlement, _wrappedNativeToken)
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
        _cowSwapSettlement: alloy_sol_types::private::Address,
        _wrappedNativeToken: alloy_sol_types::private::Address,
    ) -> alloy_contract::RawCallBuilder<P, N> {
        CoWSwapEthFlowInstance::<P, N>::deploy_builder(
            __provider,
            _cowSwapSettlement,
            _wrappedNativeToken,
        )
    }
    /**A [`CoWSwapEthFlow`](self) instance.

    Contains type-safe methods for interacting with an on-chain instance of the
    [`CoWSwapEthFlow`](self) contract located at a given `address`, using a given
    provider `P`.

    If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
    documentation on how to provide it), the `deploy` and `deploy_builder` methods can
    be used to deploy a new instance of the contract.

    See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct CoWSwapEthFlowInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for CoWSwapEthFlowInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("CoWSwapEthFlowInstance")
                .field(&self.address)
                .finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        CoWSwapEthFlowInstance<P, N>
    {
        /**Creates a new wrapper around an on-chain [`CoWSwapEthFlow`](self) contract instance.

        See the [wrapper's documentation](`CoWSwapEthFlowInstance`) for more details.*/
        #[inline]
        pub const fn new(address: alloy_sol_types::private::Address, __provider: P) -> Self {
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
            _cowSwapSettlement: alloy_sol_types::private::Address,
            _wrappedNativeToken: alloy_sol_types::private::Address,
        ) -> alloy_contract::Result<CoWSwapEthFlowInstance<P, N>> {
            let call_builder =
                Self::deploy_builder(__provider, _cowSwapSettlement, _wrappedNativeToken);
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
            _cowSwapSettlement: alloy_sol_types::private::Address,
            _wrappedNativeToken: alloy_sol_types::private::Address,
        ) -> alloy_contract::RawCallBuilder<P, N> {
            alloy_contract::RawCallBuilder::new_raw_deploy(
                __provider,
                [
                    &BYTECODE[..],
                    &alloy_sol_types::SolConstructor::abi_encode(&constructorCall {
                        _cowSwapSettlement,
                        _wrappedNativeToken,
                    })[..],
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
    impl<P: ::core::clone::Clone, N> CoWSwapEthFlowInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned provider.
        #[inline]
        pub fn with_cloned_provider(self) -> CoWSwapEthFlowInstance<P, N> {
            CoWSwapEthFlowInstance {
                address: self.address,
                provider: ::core::clone::Clone::clone(&self.provider),
                _network: ::core::marker::PhantomData,
            }
        }
    }
    /// Function calls.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        CoWSwapEthFlowInstance<P, N>
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
        ///Creates a new call builder for the [`cowSwapSettlement`] function.
        pub fn cowSwapSettlement(
            &self,
        ) -> alloy_contract::SolCallBuilder<&P, cowSwapSettlementCall, N> {
            self.call_builder(&cowSwapSettlementCall)
        }
        ///Creates a new call builder for the [`createOrder`] function.
        pub fn createOrder(
            &self,
            order: <EthFlowOrder::Data as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, createOrderCall, N> {
            self.call_builder(&createOrderCall { order })
        }
        ///Creates a new call builder for the [`invalidateOrder`] function.
        pub fn invalidateOrder(
            &self,
            order: <EthFlowOrder::Data as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, invalidateOrderCall, N> {
            self.call_builder(&invalidateOrderCall { order })
        }
        ///Creates a new call builder for the [`invalidateOrdersIgnoringNotAllowed`] function.
        pub fn invalidateOrdersIgnoringNotAllowed(
            &self,
            orderArray: alloy_sol_types::private::Vec<
                <EthFlowOrder::Data as alloy_sol_types::SolType>::RustType,
            >,
        ) -> alloy_contract::SolCallBuilder<&P, invalidateOrdersIgnoringNotAllowedCall, N> {
            self.call_builder(&invalidateOrdersIgnoringNotAllowedCall { orderArray })
        }
        ///Creates a new call builder for the [`isValidSignature`] function.
        pub fn isValidSignature(
            &self,
            orderHash: alloy_sol_types::private::FixedBytes<32>,
            _1: alloy_sol_types::private::Bytes,
        ) -> alloy_contract::SolCallBuilder<&P, isValidSignatureCall, N> {
            self.call_builder(&isValidSignatureCall { orderHash, _1 })
        }
        ///Creates a new call builder for the [`orders`] function.
        pub fn orders(
            &self,
            _0: alloy_sol_types::private::FixedBytes<32>,
        ) -> alloy_contract::SolCallBuilder<&P, ordersCall, N> {
            self.call_builder(&ordersCall(_0))
        }
        ///Creates a new call builder for the [`unwrap`] function.
        pub fn unwrap(
            &self,
            amount: alloy_sol_types::private::primitives::aliases::U256,
        ) -> alloy_contract::SolCallBuilder<&P, unwrapCall, N> {
            self.call_builder(&unwrapCall { amount })
        }
        ///Creates a new call builder for the [`wrap`] function.
        pub fn wrap(
            &self,
            amount: alloy_sol_types::private::primitives::aliases::U256,
        ) -> alloy_contract::SolCallBuilder<&P, wrapCall, N> {
            self.call_builder(&wrapCall { amount })
        }
        ///Creates a new call builder for the [`wrapAll`] function.
        pub fn wrapAll(&self) -> alloy_contract::SolCallBuilder<&P, wrapAllCall, N> {
            self.call_builder(&wrapAllCall)
        }
        ///Creates a new call builder for the [`wrappedNativeToken`] function.
        pub fn wrappedNativeToken(
            &self,
        ) -> alloy_contract::SolCallBuilder<&P, wrappedNativeTokenCall, N> {
            self.call_builder(&wrappedNativeTokenCall)
        }
    }
    /// Event filters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        CoWSwapEthFlowInstance<P, N>
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
        ///Creates a new event filter for the [`OrderInvalidation`] event.
        pub fn OrderInvalidation_filter(&self) -> alloy_contract::Event<&P, OrderInvalidation, N> {
            self.event_filter::<OrderInvalidation>()
        }
        ///Creates a new event filter for the [`OrderPlacement`] event.
        pub fn OrderPlacement_filter(&self) -> alloy_contract::Event<&P, OrderPlacement, N> {
            self.event_filter::<OrderPlacement>()
        }
        ///Creates a new event filter for the [`OrderRefund`] event.
        pub fn OrderRefund_filter(&self) -> alloy_contract::Event<&P, OrderRefund, N> {
            self.event_filter::<OrderRefund>()
        }
    }
}
pub type Instance = CoWSwapEthFlow::CoWSwapEthFlowInstance<::alloy_provider::DynProvider>;
use {
    alloy_primitives::{Address, address},
    alloy_provider::{DynProvider, Provider},
    anyhow::{Context, Result},
    std::{collections::HashMap, sync::LazyLock},
};
pub const fn deployment_info(chain_id: u64) -> Option<(Address, Option<u64>)> {
    match chain_id {
        100u64 => Some((
            ::alloy_primitives::address!("0x40a50cf069e992aa4536211b23f286ef88752187"),
            Some(25414331u64),
        )),
        42161u64 => Some((
            ::alloy_primitives::address!("0x6DFE75B5ddce1ADE279D4fa6BD6AeF3cBb6f49dB"),
            Some(204747458u64),
        )),
        8453u64 => Some((
            ::alloy_primitives::address!("0x3C3eA1829891BC9bEC3d06A81d5d169e52a415e3"),
            Some(21490258u64),
        )),
        232u64 => Some((
            ::alloy_primitives::address!("0xFb337f8a725A142f65fb9ff4902d41cc901de222"),
            Some(3007173u64),
        )),
        10u64 => Some((
            ::alloy_primitives::address!("0x04501b9b1d52e67f6862d157e00d13419d2d6e95"),
            Some(134607215u64),
        )),
        1u64 => Some((
            ::alloy_primitives::address!("0x40a50cf069e992aa4536211b23f286ef88752187"),
            Some(16169866u64),
        )),
        43114u64 => Some((
            ::alloy_primitives::address!("0x04501b9b1d52e67f6862d157e00d13419d2d6e95"),
            Some(60496408u64),
        )),
        137u64 => Some((
            ::alloy_primitives::address!("0x04501b9b1d52e67f6862d157e00d13419d2d6e95"),
            Some(71296258u64),
        )),
        59144u64 => Some((
            ::alloy_primitives::address!("0x04501b9b1d52e67f6862d157e00d13419d2d6e95"),
            Some(24522097u64),
        )),
        9745u64 => Some((
            ::alloy_primitives::address!("0x04501b9b1d52e67f6862d157e00d13419d2d6e95"),
            Some(3521855u64),
        )),
        56u64 => Some((
            ::alloy_primitives::address!("0x04501b9b1d52e67f6862d157e00d13419d2d6e95"),
            Some(48411237u64),
        )),
        11155111u64 => Some((
            ::alloy_primitives::address!("0x0b7795E18767259CC253a2dF471db34c72B49516"),
            Some(4718739u64),
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
