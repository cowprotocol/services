#![allow(unused_imports, unused_attributes, clippy::all, rustdoc::all, non_snake_case)]
//! Auto-generated contract bindings. Do not edit.
///Module containing a contract's types and functions.
/**

```solidity
library ConstantProduct {
    struct TradingParams { uint256 minTradedToken0; address priceOracle; bytes priceOracleData; bytes32 appData; }
}
```*/
#[allow(
    non_camel_case_types,
    non_snake_case,
    clippy::pub_underscore_fields,
    clippy::style,
    clippy::empty_structs_with_brackets
)]
pub mod ConstantProduct {
    use super::*;
    use alloy_sol_types as alloy_sol_types;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**```solidity
struct TradingParams { uint256 minTradedToken0; address priceOracle; bytes priceOracleData; bytes32 appData; }
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct TradingParams {
        #[allow(missing_docs)]
        pub minTradedToken0: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub priceOracle: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub priceOracleData: alloy_sol_types::private::Bytes,
        #[allow(missing_docs)]
        pub appData: alloy_sol_types::private::FixedBytes<32>,
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
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Bytes,
            alloy_sol_types::sol_data::FixedBytes<32>,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
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
        impl ::core::convert::From<TradingParams> for UnderlyingRustTuple<'_> {
            fn from(value: TradingParams) -> Self {
                (
                    value.minTradedToken0,
                    value.priceOracle,
                    value.priceOracleData,
                    value.appData,
                )
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for TradingParams {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    minTradedToken0: tuple.0,
                    priceOracle: tuple.1,
                    priceOracleData: tuple.2,
                    appData: tuple.3,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for TradingParams {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for TradingParams {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
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
        impl alloy_sol_types::SolType for TradingParams {
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
        impl alloy_sol_types::SolStruct for TradingParams {
            const NAME: &'static str = "TradingParams";
            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "TradingParams(uint256 minTradedToken0,address priceOracle,bytes priceOracleData,bytes32 appData)",
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
                            &self.minTradedToken0,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.priceOracle,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::eip712_data_word(
                            &self.priceOracleData,
                        )
                        .0,
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.appData)
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for TradingParams {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.minTradedToken0,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.priceOracle,
                    )
                    + <alloy_sol_types::sol_data::Bytes as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.priceOracleData,
                    )
                    + <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.appData,
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
                    &rust.minTradedToken0,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.priceOracle,
                    out,
                );
                <alloy_sol_types::sol_data::Bytes as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.priceOracleData,
                    out,
                );
                <alloy_sol_types::sol_data::FixedBytes<
                    32,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.appData,
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
    /**Creates a new wrapper around an on-chain [`ConstantProduct`](self) contract instance.

See the [wrapper's documentation](`ConstantProductInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> ConstantProductInstance<P, N> {
        ConstantProductInstance::<P, N>::new(address, __provider)
    }
    /**A [`ConstantProduct`](self) instance.

Contains type-safe methods for interacting with an on-chain instance of the
[`ConstantProduct`](self) contract located at a given `address`, using a given
provider `P`.

If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
documentation on how to provide it), the `deploy` and `deploy_builder` methods can
be used to deploy a new instance of the contract.

See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct ConstantProductInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for ConstantProductInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("ConstantProductInstance").field(&self.address).finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    > ConstantProductInstance<P, N> {
        /**Creates a new wrapper around an on-chain [`ConstantProduct`](self) contract instance.

See the [wrapper's documentation](`ConstantProductInstance`) for more details.*/
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
    impl<P: ::core::clone::Clone, N> ConstantProductInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned provider.
        #[inline]
        pub fn with_cloned_provider(self) -> ConstantProductInstance<P, N> {
            ConstantProductInstance {
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
    > ConstantProductInstance<P, N> {
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
    > ConstantProductInstance<P, N> {
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
/**

Generated by the following Solidity interface...
```solidity
library ConstantProduct {
    struct TradingParams {
        uint256 minTradedToken0;
        address priceOracle;
        bytes priceOracleData;
        bytes32 appData;
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

interface CowAmm {
    error CommitOutsideOfSettlement();
    error OnlyManagerCanCall();
    error OrderDoesNotMatchCommitmentHash();
    error OrderDoesNotMatchDefaultTradeableOrder();
    error OrderDoesNotMatchMessageHash();
    error OrderNotValid(string);
    error PollTryAtBlock(uint256 blockNumber, string message);
    error TradingParamsDoNotMatchHash();

    event TradingDisabled();
    event TradingEnabled(bytes32 indexed hash, ConstantProduct.TradingParams params);

    constructor(address _solutionSettler, address _token0, address _token1);

    function commit(bytes32 orderHash) external;
    function hash(ConstantProduct.TradingParams memory tradingParams) external pure returns (bytes32);
    function isValidSignature(bytes32 _hash, bytes memory signature) external view returns (bytes4);
    function manager() external view returns (address);
    function token0() external view returns (address);
    function token1() external view returns (address);
    function verify(ConstantProduct.TradingParams memory tradingParams, GPv2Order.Data memory order) external view;
}
```

...which was generated by the following JSON ABI:
```json
[
  {
    "type": "constructor",
    "inputs": [
      {
        "name": "_solutionSettler",
        "type": "address",
        "internalType": "contract ISettlement"
      },
      {
        "name": "_token0",
        "type": "address",
        "internalType": "contract IERC20"
      },
      {
        "name": "_token1",
        "type": "address",
        "internalType": "contract IERC20"
      }
    ],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "commit",
    "inputs": [
      {
        "name": "orderHash",
        "type": "bytes32",
        "internalType": "bytes32"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "hash",
    "inputs": [
      {
        "name": "tradingParams",
        "type": "tuple",
        "internalType": "struct ConstantProduct.TradingParams",
        "components": [
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
        ]
      }
    ],
    "outputs": [
      {
        "name": "",
        "type": "bytes32",
        "internalType": "bytes32"
      }
    ],
    "stateMutability": "pure"
  },
  {
    "type": "function",
    "name": "isValidSignature",
    "inputs": [
      {
        "name": "_hash",
        "type": "bytes32",
        "internalType": "bytes32"
      },
      {
        "name": "signature",
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
    "name": "manager",
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
    "name": "token0",
    "inputs": [],
    "outputs": [
      {
        "name": "",
        "type": "address",
        "internalType": "contract IERC20"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "token1",
    "inputs": [],
    "outputs": [
      {
        "name": "",
        "type": "address",
        "internalType": "contract IERC20"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "verify",
    "inputs": [
      {
        "name": "tradingParams",
        "type": "tuple",
        "internalType": "struct ConstantProduct.TradingParams",
        "components": [
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
        ]
      },
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
      }
    ],
    "outputs": [],
    "stateMutability": "view"
  },
  {
    "type": "event",
    "name": "TradingDisabled",
    "inputs": [],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "TradingEnabled",
    "inputs": [
      {
        "name": "hash",
        "type": "bytes32",
        "indexed": true,
        "internalType": "bytes32"
      },
      {
        "name": "params",
        "type": "tuple",
        "indexed": false,
        "internalType": "struct ConstantProduct.TradingParams",
        "components": [
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
        ]
      }
    ],
    "anonymous": false
  },
  {
    "type": "error",
    "name": "CommitOutsideOfSettlement",
    "inputs": []
  },
  {
    "type": "error",
    "name": "OnlyManagerCanCall",
    "inputs": []
  },
  {
    "type": "error",
    "name": "OrderDoesNotMatchCommitmentHash",
    "inputs": []
  },
  {
    "type": "error",
    "name": "OrderDoesNotMatchDefaultTradeableOrder",
    "inputs": []
  },
  {
    "type": "error",
    "name": "OrderDoesNotMatchMessageHash",
    "inputs": []
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
  },
  {
    "type": "error",
    "name": "PollTryAtBlock",
    "inputs": [
      {
        "name": "blockNumber",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "message",
        "type": "string",
        "internalType": "string"
      }
    ]
  },
  {
    "type": "error",
    "name": "TradingParamsDoNotMatchHash",
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
pub mod CowAmm {
    use super::*;
    use alloy_sol_types as alloy_sol_types;
    /// The creation / init bytecode of the contract.
    ///
    /// ```text
    ///0x610120604052348015610010575f80fd5b5060405161267838038061267883398101604081905261002f9161052f565b6001600160a01b03831660808190526040805163f698da2560e01b8152905163f698da259160048082019260209290919082900301815f875af1158015610078573d5f803e3d5ffd5b505050506040513d601f19601f8201168201806040525081019061009c9190610579565b610100526100aa823361015f565b6100b4813361015f565b336001600160a01b031660e0816001600160a01b0316815250505f836001600160a01b0316639b552cc26040518163ffffffff1660e01b81526004016020604051808303815f875af115801561010c573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906101309190610590565b905061013c838261015f565b610146828261015f565b506001600160a01b0391821660a0521660c0525061061c565b6101746001600160a01b038316825f19610178565b5050565b8015806101f05750604051636eb1769f60e11b81523060048201526001600160a01b03838116602483015284169063dd62ed3e90604401602060405180830381865afa1580156101ca573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906101ee9190610579565b155b6102675760405162461bcd60e51b815260206004820152603660248201527f5361666545524332303a20617070726f76652066726f6d206e6f6e2d7a65726f60448201527f20746f206e6f6e2d7a65726f20616c6c6f77616e63650000000000000000000060648201526084015b60405180910390fd5b604080516001600160a01b038416602482015260448082018490528251808303909101815260649091019091526020810180516001600160e01b0390811663095ea7b360e01b179091526102bd9185916102c216565b505050565b6040805180820190915260208082527f5361666545524332303a206c6f772d6c6576656c2063616c6c206661696c6564908201525f9061030e906001600160a01b03851690849061038d565b905080515f148061032e57508080602001905181019061032e91906105b2565b6102bd5760405162461bcd60e51b815260206004820152602a60248201527f5361666545524332303a204552433230206f7065726174696f6e20646964206e6044820152691bdd081cdd58d8d9595960b21b606482015260840161025e565b606061039b84845f856103a3565b949350505050565b6060824710156104045760405162461bcd60e51b815260206004820152602660248201527f416464726573733a20696e73756666696369656e742062616c616e636520666f6044820152651c8818d85b1b60d21b606482015260840161025e565b5f80866001600160a01b0316858760405161041f91906105d1565b5f6040518083038185875af1925050503d805f8114610459576040519150601f19603f3d011682016040523d82523d5f602084013e61045e565b606091505b5090925090506104708783838761047b565b979650505050505050565b606083156104e95782515f036104e2576001600160a01b0385163b6104e25760405162461bcd60e51b815260206004820152601d60248201527f416464726573733a2063616c6c20746f206e6f6e2d636f6e7472616374000000604482015260640161025e565b508161039b565b61039b83838151156104fe5781518083602001fd5b8060405162461bcd60e51b815260040161025e91906105e7565b6001600160a01b038116811461052c575f80fd5b50565b5f805f60608486031215610541575f80fd5b835161054c81610518565b602085015190935061055d81610518565b604085015190925061056e81610518565b809150509250925092565b5f60208284031215610589575f80fd5b5051919050565b5f602082840312156105a0575f80fd5b81516105ab81610518565b9392505050565b5f602082840312156105c2575f80fd5b815180151581146105ab575f80fd5b5f82518060208501845e5f920191825250919050565b602081525f82518060208401528060208501604085015e5f604082850101526040601f19601f83011684010191505092915050565b60805160a05160c05160e05161010051611fbd6106bb5f395f81816102db015261042b01525f8181610236015281816104d90152610bf901525f81816102b40152818161059901528181610d5c01528181610ebf01528181610f8e015261100d01525f81816101380152818161057701528181610d3b01528181610e2801528181610f6b015261103001525f818161032201526112140152611fbd5ff3fe608060405234801561000f575f80fd5b506004361061012f575f3560e01c8063b09aaaca116100ad578063e3e6f5b21161007d578063eec50b9711610063578063eec50b9714610344578063f14fcbc81461034c578063ff2dbc9814610203575f80fd5b8063e3e6f5b2146102fd578063e516715b1461031d575f80fd5b8063b09aaaca14610289578063c5f3d2541461029c578063d21220a7146102af578063d25e0cb6146102d6575f80fd5b80631c7de94111610102578063481c6a75116100e8578063481c6a7514610231578063981a160b14610258578063a029a8d414610276575f80fd5b80631c7de941146102035780633e706e321461020a575f80fd5b80630dfe1681146101335780631303a484146101845780631626ba7e146101b557806317700f01146101f9575b5f80fd5b61015a7f000000000000000000000000000000000000000000000000000000000000000081565b60405173ffffffffffffffffffffffffffffffffffffffff90911681526020015b60405180910390f35b7f6c3c90245457060f6517787b2c4b8cf500ca889d2304af02043bd5b513e3b5935c5b60405190815260200161017b565b6101c86101c33660046116bf565b61035f565b6040517fffffffff00000000000000000000000000000000000000000000000000000000909116815260200161017b565b6102016104d7565b005b6101a75f81565b6101a77f6c3c90245457060f6517787b2c4b8cf500ca889d2304af02043bd5b513e3b59381565b61015a7f000000000000000000000000000000000000000000000000000000000000000081565b61026161012c81565b60405163ffffffff909116815260200161017b565b6102016102843660046119ee565b610573565b6101a7610297366004611a3b565b610bc8565b6102016102aa366004611a75565b610bf7565b61015a7f000000000000000000000000000000000000000000000000000000000000000081565b6101a77f000000000000000000000000000000000000000000000000000000000000000081565b61031061030b366004611a3b565b610cb7565b60405161017b9190611aac565b61015a7f000000000000000000000000000000000000000000000000000000000000000081565b6101a75f5481565b61020161035a366004611b9a565b6111fc565b5f808061036e84860186611bb1565b915091505f5461037d82610bc8565b146103b4576040517ff1a6789000000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0820180517fd5a25ba2e97094ad7d83dc28a6572da797d6b3e7fc6663bd93efb789fc17e48982526101a0822091526040517f190100000000000000000000000000000000000000000000000000000000000081527f00000000000000000000000000000000000000000000000000000000000000006002820152602281019190915260429020868114610494576040517f593fcacd00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b61049f818385611291565b6104a98284610573565b507f1626ba7e00000000000000000000000000000000000000000000000000000000925050505b9392505050565b7f000000000000000000000000000000000000000000000000000000000000000073ffffffffffffffffffffffffffffffffffffffff163314610546576040517ff87d0d1600000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b5f8080556040517fbcb8b8fbdea8aa6dc4ae41213e4da81e605a3d1a56ed851b9355182321c091909190a1565b80517f0000000000000000000000000000000000000000000000000000000000000000907f00000000000000000000000000000000000000000000000000000000000000009073ffffffffffffffffffffffffffffffffffffffff808416911614610677578073ffffffffffffffffffffffffffffffffffffffff16835f015173ffffffffffffffffffffffffffffffffffffffff1614610675576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601260248201527f696e76616c69642073656c6c20746f6b656e000000000000000000000000000060448201526064015b60405180910390fd5b905b6040517f70a082310000000000000000000000000000000000000000000000000000000081523060048201525f9073ffffffffffffffffffffffffffffffffffffffff8416906370a0823190602401602060405180830381865afa1580156106e1573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906107059190611bff565b6040517f70a082310000000000000000000000000000000000000000000000000000000081523060048201529091505f9073ffffffffffffffffffffffffffffffffffffffff8416906370a0823190602401602060405180830381865afa158015610772573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906107969190611bff565b90508273ffffffffffffffffffffffffffffffffffffffff16856020015173ffffffffffffffffffffffffffffffffffffffff1614610831576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601160248201527f696e76616c69642062757920746f6b656e000000000000000000000000000000604482015260640161066c565b604085015173ffffffffffffffffffffffffffffffffffffffff16156108b3576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601d60248201527f7265636569766572206d757374206265207a65726f2061646472657373000000604482015260640161066c565b6108bf61012c42611c43565b8560a0015163ffffffff161115610932576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601e60248201527f76616c696469747920746f6f2066617220696e20746865206675747572650000604482015260640161066c565b85606001518560c00151146109a3576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152600f60248201527f696e76616c696420617070446174610000000000000000000000000000000000604482015260640161066c565b60e085015115610a0f576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601760248201527f66656520616d6f756e74206d757374206265207a65726f000000000000000000604482015260640161066c565b7f5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc985610160015114610a9d576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601d60248201527f627579546f6b656e42616c616e6365206d757374206265206572633230000000604482015260640161066c565b7f5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc985610140015114610b2b576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601e60248201527f73656c6c546f6b656e42616c616e6365206d7573742062652065726332300000604482015260640161066c565b6060850151610b3a9082611c56565b60808601516060870151610b4e9085611c6d565b610b589190611c56565b1015610bc0576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601760248201527f726563656976656420616d6f756e7420746f6f206c6f77000000000000000000604482015260640161066c565b505050505050565b5f81604051602001610bda9190611ccc565b604051602081830303815290604052805190602001209050919050565b7f000000000000000000000000000000000000000000000000000000000000000073ffffffffffffffffffffffffffffffffffffffff163314610c66576040517ff87d0d1600000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b5f610c7361029783611d27565b9050805f81905550807f510e4a4f76907c2d6158b343f7c4f2f597df385b727c26e9ef90e75093ace19a83604051610cab9190611d79565b60405180910390a25050565b60408051610180810182525f80825260208201819052918101829052606081018290526080810182905260a0810182905260c0810182905260e081018290526101008101829052610120810182905261014081018290526101608101919091525f80836020015173ffffffffffffffffffffffffffffffffffffffff1663355efdd97f00000000000000000000000000000000000000000000000000000000000000007f000000000000000000000000000000000000000000000000000000000000000087604001516040518463ffffffff1660e01b8152600401610d9e93929190611e3a565b6040805180830381865afa158015610db8573d5f803e3d5ffd5b505050506040513d601f19601f82011682018060405250810190610ddc9190611e72565b6040517f70a0823100000000000000000000000000000000000000000000000000000000815230600482015291935091505f90819073ffffffffffffffffffffffffffffffffffffffff7f000000000000000000000000000000000000000000000000000000000000000016906370a0823190602401602060405180830381865afa158015610e6d573d5f803e3d5ffd5b505050506040513d601f19601f82011682018060405250810190610e919190611bff565b6040517f70a082310000000000000000000000000000000000000000000000000000000081523060048201527f000000000000000000000000000000000000000000000000000000000000000073ffffffffffffffffffffffffffffffffffffffff16906370a0823190602401602060405180830381865afa158015610f19573d5f803e3d5ffd5b505050506040513d601f19601f82011682018060405250810190610f3d9190611bff565b90925090505f80808080610f518888611c56565b90505f610f5e8a88611c56565b90505f8282101561100b577f000000000000000000000000000000000000000000000000000000000000000096507f00000000000000000000000000000000000000000000000000000000000000009550610fd6610fbd60028b611ec1565b610fd184610fcc8e6002611c56565b611346565b61137e565b945061100185610fe6818d611c56565b610ff09085611c43565b610ffa8c8f611c56565b60016113cb565b9350849050611098565b7f000000000000000000000000000000000000000000000000000000000000000096507f0000000000000000000000000000000000000000000000000000000000000000955061106e61105f60028a611ec1565b610fd185610fcc8f6002611c56565b94506110928561107e818e611c56565b6110889086611c43565b610ffa8b8e611c56565b93508390505b8c518110156110df576110df6040518060400160405280601781526020017f74726164656420616d6f756e7420746f6f20736d616c6c000000000000000000815250611426565b6040518061018001604052808873ffffffffffffffffffffffffffffffffffffffff1681526020018773ffffffffffffffffffffffffffffffffffffffff1681526020015f73ffffffffffffffffffffffffffffffffffffffff16815260200186815260200185815260200161115661012c611466565b63ffffffff1681526020018e6060015181526020015f81526020017ff3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee34677581526020016001151581526020017f5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc981526020017f5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc98152509b505050505050505050505050919050565b3373ffffffffffffffffffffffffffffffffffffffff7f0000000000000000000000000000000000000000000000000000000000000000161461126b576040517fbf84897700000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b807f6c3c90245457060f6517787b2c4b8cf500ca889d2304af02043bd5b513e3b5935d50565b7f6c3c90245457060f6517787b2c4b8cf500ca889d2304af02043bd5b513e3b5935c8381146113405780156112f2576040517fdafbdd1f00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b5f6112fc84610cb7565b90506113088382611487565b61133e576040517fd9ff24c700000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b505b50505050565b5f82156113735781611359600185611c6d565b6113639190611ec1565b61136e906001611c43565b611375565b5f5b90505b92915050565b5f818310156113c5576113c56040518060400160405280601581526020017f7375627472616374696f6e20756e646572666c6f770000000000000000000000815250611426565b50900390565b5f806113d8868686611599565b905060018360028111156113ee576113ee611ed4565b14801561140a57505f848061140557611405611e94565b868809115b1561141d5761141a600182611c43565b90505b95945050505050565b611431436001611c43565b816040517f1fe8506e00000000000000000000000000000000000000000000000000000000815260040161066c929190611f01565b5f81806114738142611f19565b61147d9190611f3b565b6113789190611f63565b5f80825f015173ffffffffffffffffffffffffffffffffffffffff16845f015173ffffffffffffffffffffffffffffffffffffffff161490505f836020015173ffffffffffffffffffffffffffffffffffffffff16856020015173ffffffffffffffffffffffffffffffffffffffff161490505f846060015186606001511490505f856080015187608001511490505f8660a0015163ffffffff168860a0015163ffffffff161490505f8761010001518961010001511490505f88610120015115158a6101200151151514905086801561155e5750855b80156115675750845b80156115705750835b80156115795750825b80156115825750815b801561158b5750805b9a9950505050505050505050565b5f80807fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff858709858702925082811083820303915050805f036115ef578382816115e5576115e5611e94565b04925050506104d0565b808411611658576040517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601560248201527f4d6174683a206d756c446976206f766572666c6f770000000000000000000000604482015260640161066c565b5f8486880960026001871981018816978890046003810283188082028403028082028403028082028403028082028403028082028403029081029092039091025f889003889004909101858311909403939093029303949094049190911702949350505050565b5f805f604084860312156116d1575f80fd5b83359250602084013567ffffffffffffffff808211156116ef575f80fd5b818601915086601f830112611702575f80fd5b813581811115611710575f80fd5b876020828501011115611721575f80fd5b6020830194508093505050509250925092565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52604160045260245ffd5b6040516080810167ffffffffffffffff8111828210171561178457611784611734565b60405290565b604051610180810167ffffffffffffffff8111828210171561178457611784611734565b604051601f82017fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe016810167ffffffffffffffff811182821017156117f5576117f5611734565b604052919050565b73ffffffffffffffffffffffffffffffffffffffff8116811461181e575f80fd5b50565b5f60808284031215611831575f80fd5b611839611761565b90508135815260208083013561184e816117fd565b82820152604083013567ffffffffffffffff8082111561186c575f80fd5b818501915085601f83011261187f575f80fd5b81358181111561189157611891611734565b6118c1847fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f840116016117ae565b915080825286848285010111156118d6575f80fd5b80848401858401375f848284010152508060408501525050506060820135606082015292915050565b803561190a816117fd565b919050565b803563ffffffff8116811461190a575f80fd5b8035801515811461190a575f80fd5b5f6101808284031215611942575f80fd5b61194a61178a565b9050611955826118ff565b8152611963602083016118ff565b6020820152611974604083016118ff565b6040820152606082013560608201526080820135608082015261199960a0830161190f565b60a082015260c082013560c082015260e082013560e08201526101008083013581830152506101206119cc818401611922565b9082015261014082810135908201526101609182013591810191909152919050565b5f806101a08385031215611a00575f80fd5b823567ffffffffffffffff811115611a16575f80fd5b611a2285828601611821565b925050611a328460208501611931565b90509250929050565b5f60208284031215611a4b575f80fd5b813567ffffffffffffffff811115611a61575f80fd5b611a6d84828501611821565b949350505050565b5f60208284031215611a85575f80fd5b813567ffffffffffffffff811115611a9b575f80fd5b8201608081850312156104d0575f80fd5b815173ffffffffffffffffffffffffffffffffffffffff16815261018081016020830151611af2602084018273ffffffffffffffffffffffffffffffffffffffff169052565b506040830151611b1a604084018273ffffffffffffffffffffffffffffffffffffffff169052565b50606083015160608301526080830151608083015260a0830151611b4660a084018263ffffffff169052565b5060c083015160c083015260e083015160e083015261010080840151818401525061012080840151611b7b8285018215159052565b5050610140838101519083015261016092830151929091019190915290565b5f60208284031215611baa575f80fd5b5035919050565b5f806101a08385031215611bc3575f80fd5b611bcd8484611931565b915061018083013567ffffffffffffffff811115611be9575f80fd5b611bf585828601611821565b9150509250929050565b5f60208284031215611c0f575f80fd5b5051919050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52601160045260245ffd5b8082018082111561137857611378611c16565b808202811582820484141761137857611378611c16565b8181038181111561137857611378611c16565b5f81518084528060208401602086015e5f6020828601015260207fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f83011685010191505092915050565b602081528151602082015273ffffffffffffffffffffffffffffffffffffffff60208301511660408201525f604083015160806060840152611d1160a0840182611c80565b9050606084015160808401528091505092915050565b5f6113783683611821565b81835281816020850137505f602082840101525f60207fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f840116840101905092915050565b60208152813560208201525f6020830135611d93816117fd565b73ffffffffffffffffffffffffffffffffffffffff811660408401525060408301357fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe1843603018112611de4575f80fd5b830160208101903567ffffffffffffffff811115611e00575f80fd5b803603821315611e0e575f80fd5b60806060850152611e2360a085018284611d32565b915050606084013560808401528091505092915050565b5f73ffffffffffffffffffffffffffffffffffffffff80861683528085166020840152506060604083015261141d6060830184611c80565b5f8060408385031215611e83575f80fd5b505080516020909101519092909150565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52601260045260245ffd5b5f82611ecf57611ecf611e94565b500490565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52602160045260245ffd5b828152604060208201525f611a6d6040830184611c80565b5f63ffffffff80841680611f2f57611f2f611e94565b92169190910492915050565b63ffffffff818116838216028082169190828114611f5b57611f5b611c16565b505092915050565b63ffffffff818116838216019080821115611f8057611f80611c16565b509291505056fea2646970667358221220e3fb228b525d90b942c7e58fe2e2034a17bd258c082fd47740e764a7be45bac664736f6c63430008190033
    /// ```
    #[rustfmt::skip]
    #[allow(clippy::all)]
    pub static BYTECODE: alloy_sol_types::private::Bytes = alloy_sol_types::private::Bytes::from_static(
        b"a\x01 `@R4\x80\x15a\0\x10W_\x80\xFD[P`@Qa&x8\x03\x80a&x\x839\x81\x01`@\x81\x90Ra\0/\x91a\x05/V[`\x01`\x01`\xA0\x1B\x03\x83\x16`\x80\x81\x90R`@\x80Qc\xF6\x98\xDA%`\xE0\x1B\x81R\x90Qc\xF6\x98\xDA%\x91`\x04\x80\x82\x01\x92` \x92\x90\x91\x90\x82\x90\x03\x01\x81_\x87Z\xF1\x15\x80\x15a\0xW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\0\x9C\x91\x90a\x05yV[a\x01\0Ra\0\xAA\x823a\x01_V[a\0\xB4\x813a\x01_V[3`\x01`\x01`\xA0\x1B\x03\x16`\xE0\x81`\x01`\x01`\xA0\x1B\x03\x16\x81RPP_\x83`\x01`\x01`\xA0\x1B\x03\x16c\x9BU,\xC2`@Q\x81c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01` `@Q\x80\x83\x03\x81_\x87Z\xF1\x15\x80\x15a\x01\x0CW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x010\x91\x90a\x05\x90V[\x90Pa\x01<\x83\x82a\x01_V[a\x01F\x82\x82a\x01_V[P`\x01`\x01`\xA0\x1B\x03\x91\x82\x16`\xA0R\x16`\xC0RPa\x06\x1CV[a\x01t`\x01`\x01`\xA0\x1B\x03\x83\x16\x82_\x19a\x01xV[PPV[\x80\x15\x80a\x01\xF0WP`@Qcn\xB1v\x9F`\xE1\x1B\x81R0`\x04\x82\x01R`\x01`\x01`\xA0\x1B\x03\x83\x81\x16`$\x83\x01R\x84\x16\x90c\xDDb\xED>\x90`D\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x01\xCAW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x01\xEE\x91\x90a\x05yV[\x15[a\x02gW`@QbF\x1B\xCD`\xE5\x1B\x81R` `\x04\x82\x01R`6`$\x82\x01R\x7FSafeERC20: approve from non-zero`D\x82\x01R\x7F to non-zero allowance\0\0\0\0\0\0\0\0\0\0`d\x82\x01R`\x84\x01[`@Q\x80\x91\x03\x90\xFD[`@\x80Q`\x01`\x01`\xA0\x1B\x03\x84\x16`$\x82\x01R`D\x80\x82\x01\x84\x90R\x82Q\x80\x83\x03\x90\x91\x01\x81R`d\x90\x91\x01\x90\x91R` \x81\x01\x80Q`\x01`\x01`\xE0\x1B\x03\x90\x81\x16c\t^\xA7\xB3`\xE0\x1B\x17\x90\x91Ra\x02\xBD\x91\x85\x91a\x02\xC2\x16V[PPPV[`@\x80Q\x80\x82\x01\x90\x91R` \x80\x82R\x7FSafeERC20: low-level call failed\x90\x82\x01R_\x90a\x03\x0E\x90`\x01`\x01`\xA0\x1B\x03\x85\x16\x90\x84\x90a\x03\x8DV[\x90P\x80Q_\x14\x80a\x03.WP\x80\x80` \x01\x90Q\x81\x01\x90a\x03.\x91\x90a\x05\xB2V[a\x02\xBDW`@QbF\x1B\xCD`\xE5\x1B\x81R` `\x04\x82\x01R`*`$\x82\x01R\x7FSafeERC20: ERC20 operation did n`D\x82\x01Ri\x1B\xDD\x08\x1C\xDDX\xD8\xD9YY`\xB2\x1B`d\x82\x01R`\x84\x01a\x02^V[``a\x03\x9B\x84\x84_\x85a\x03\xA3V[\x94\x93PPPPV[``\x82G\x10\x15a\x04\x04W`@QbF\x1B\xCD`\xE5\x1B\x81R` `\x04\x82\x01R`&`$\x82\x01R\x7FAddress: insufficient balance fo`D\x82\x01Re\x1C\x88\x18\xD8[\x1B`\xD2\x1B`d\x82\x01R`\x84\x01a\x02^V[_\x80\x86`\x01`\x01`\xA0\x1B\x03\x16\x85\x87`@Qa\x04\x1F\x91\x90a\x05\xD1V[_`@Q\x80\x83\x03\x81\x85\x87Z\xF1\x92PPP=\x80_\x81\x14a\x04YW`@Q\x91P`\x1F\x19`?=\x01\x16\x82\x01`@R=\x82R=_` \x84\x01>a\x04^V[``\x91P[P\x90\x92P\x90Pa\x04p\x87\x83\x83\x87a\x04{V[\x97\x96PPPPPPPV[``\x83\x15a\x04\xE9W\x82Q_\x03a\x04\xE2W`\x01`\x01`\xA0\x1B\x03\x85\x16;a\x04\xE2W`@QbF\x1B\xCD`\xE5\x1B\x81R` `\x04\x82\x01R`\x1D`$\x82\x01R\x7FAddress: call to non-contract\0\0\0`D\x82\x01R`d\x01a\x02^V[P\x81a\x03\x9BV[a\x03\x9B\x83\x83\x81Q\x15a\x04\xFEW\x81Q\x80\x83` \x01\xFD[\x80`@QbF\x1B\xCD`\xE5\x1B\x81R`\x04\x01a\x02^\x91\x90a\x05\xE7V[`\x01`\x01`\xA0\x1B\x03\x81\x16\x81\x14a\x05,W_\x80\xFD[PV[_\x80_``\x84\x86\x03\x12\x15a\x05AW_\x80\xFD[\x83Qa\x05L\x81a\x05\x18V[` \x85\x01Q\x90\x93Pa\x05]\x81a\x05\x18V[`@\x85\x01Q\x90\x92Pa\x05n\x81a\x05\x18V[\x80\x91PP\x92P\x92P\x92V[_` \x82\x84\x03\x12\x15a\x05\x89W_\x80\xFD[PQ\x91\x90PV[_` \x82\x84\x03\x12\x15a\x05\xA0W_\x80\xFD[\x81Qa\x05\xAB\x81a\x05\x18V[\x93\x92PPPV[_` \x82\x84\x03\x12\x15a\x05\xC2W_\x80\xFD[\x81Q\x80\x15\x15\x81\x14a\x05\xABW_\x80\xFD[_\x82Q\x80` \x85\x01\x84^_\x92\x01\x91\x82RP\x91\x90PV[` \x81R_\x82Q\x80` \x84\x01R\x80` \x85\x01`@\x85\x01^_`@\x82\x85\x01\x01R`@`\x1F\x19`\x1F\x83\x01\x16\x84\x01\x01\x91PP\x92\x91PPV[`\x80Q`\xA0Q`\xC0Q`\xE0Qa\x01\0Qa\x1F\xBDa\x06\xBB_9_\x81\x81a\x02\xDB\x01Ra\x04+\x01R_\x81\x81a\x026\x01R\x81\x81a\x04\xD9\x01Ra\x0B\xF9\x01R_\x81\x81a\x02\xB4\x01R\x81\x81a\x05\x99\x01R\x81\x81a\r\\\x01R\x81\x81a\x0E\xBF\x01R\x81\x81a\x0F\x8E\x01Ra\x10\r\x01R_\x81\x81a\x018\x01R\x81\x81a\x05w\x01R\x81\x81a\r;\x01R\x81\x81a\x0E(\x01R\x81\x81a\x0Fk\x01Ra\x100\x01R_\x81\x81a\x03\"\x01Ra\x12\x14\x01Ra\x1F\xBD_\xF3\xFE`\x80`@R4\x80\x15a\0\x0FW_\x80\xFD[P`\x046\x10a\x01/W_5`\xE0\x1C\x80c\xB0\x9A\xAA\xCA\x11a\0\xADW\x80c\xE3\xE6\xF5\xB2\x11a\0}W\x80c\xEE\xC5\x0B\x97\x11a\0cW\x80c\xEE\xC5\x0B\x97\x14a\x03DW\x80c\xF1O\xCB\xC8\x14a\x03LW\x80c\xFF-\xBC\x98\x14a\x02\x03W_\x80\xFD[\x80c\xE3\xE6\xF5\xB2\x14a\x02\xFDW\x80c\xE5\x16q[\x14a\x03\x1DW_\x80\xFD[\x80c\xB0\x9A\xAA\xCA\x14a\x02\x89W\x80c\xC5\xF3\xD2T\x14a\x02\x9CW\x80c\xD2\x12 \xA7\x14a\x02\xAFW\x80c\xD2^\x0C\xB6\x14a\x02\xD6W_\x80\xFD[\x80c\x1C}\xE9A\x11a\x01\x02W\x80cH\x1Cju\x11a\0\xE8W\x80cH\x1Cju\x14a\x021W\x80c\x98\x1A\x16\x0B\x14a\x02XW\x80c\xA0)\xA8\xD4\x14a\x02vW_\x80\xFD[\x80c\x1C}\xE9A\x14a\x02\x03W\x80c>pn2\x14a\x02\nW_\x80\xFD[\x80c\r\xFE\x16\x81\x14a\x013W\x80c\x13\x03\xA4\x84\x14a\x01\x84W\x80c\x16&\xBA~\x14a\x01\xB5W\x80c\x17p\x0F\x01\x14a\x01\xF9W[_\x80\xFD[a\x01Z\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[`@Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x90\x91\x16\x81R` \x01[`@Q\x80\x91\x03\x90\xF3[\x7Fl<\x90$TW\x06\x0Fe\x17x{,K\x8C\xF5\0\xCA\x88\x9D#\x04\xAF\x02\x04;\xD5\xB5\x13\xE3\xB5\x93\\[`@Q\x90\x81R` \x01a\x01{V[a\x01\xC8a\x01\xC36`\x04a\x16\xBFV[a\x03_V[`@Q\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90\x91\x16\x81R` \x01a\x01{V[a\x02\x01a\x04\xD7V[\0[a\x01\xA7_\x81V[a\x01\xA7\x7Fl<\x90$TW\x06\x0Fe\x17x{,K\x8C\xF5\0\xCA\x88\x9D#\x04\xAF\x02\x04;\xD5\xB5\x13\xE3\xB5\x93\x81V[a\x01Z\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[a\x02aa\x01,\x81V[`@Qc\xFF\xFF\xFF\xFF\x90\x91\x16\x81R` \x01a\x01{V[a\x02\x01a\x02\x846`\x04a\x19\xEEV[a\x05sV[a\x01\xA7a\x02\x976`\x04a\x1A;V[a\x0B\xC8V[a\x02\x01a\x02\xAA6`\x04a\x1AuV[a\x0B\xF7V[a\x01Z\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[a\x01\xA7\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[a\x03\x10a\x03\x0B6`\x04a\x1A;V[a\x0C\xB7V[`@Qa\x01{\x91\x90a\x1A\xACV[a\x01Z\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[a\x01\xA7_T\x81V[a\x02\x01a\x03Z6`\x04a\x1B\x9AV[a\x11\xFCV[_\x80\x80a\x03n\x84\x86\x01\x86a\x1B\xB1V[\x91P\x91P_Ta\x03}\x82a\x0B\xC8V[\x14a\x03\xB4W`@Q\x7F\xF1\xA6x\x90\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x82\x01\x80Q\x7F\xD5\xA2[\xA2\xE9p\x94\xAD}\x83\xDC(\xA6W-\xA7\x97\xD6\xB3\xE7\xFCfc\xBD\x93\xEF\xB7\x89\xFC\x17\xE4\x89\x82Ra\x01\xA0\x82 \x91R`@Q\x7F\x19\x01\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\x02\x82\x01R`\"\x81\x01\x91\x90\x91R`B\x90 \x86\x81\x14a\x04\x94W`@Q\x7FY?\xCA\xCD\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\x04\x9F\x81\x83\x85a\x12\x91V[a\x04\xA9\x82\x84a\x05sV[P\x7F\x16&\xBA~\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x92PPP[\x93\x92PPPV[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x163\x14a\x05FW`@Q\x7F\xF8}\r\x16\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[_\x80\x80U`@Q\x7F\xBC\xB8\xB8\xFB\xDE\xA8\xAAm\xC4\xAEA!>M\xA8\x1E`Z=\x1AV\xED\x85\x1B\x93U\x18#!\xC0\x91\x90\x91\x90\xA1V[\x80Q\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x84\x16\x91\x16\x14a\x06wW\x80s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x83_\x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14a\x06uW`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x12`$\x82\x01R\x7Finvalid sell token\0\0\0\0\0\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01[`@Q\x80\x91\x03\x90\xFD[\x90[`@Q\x7Fp\xA0\x821\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R0`\x04\x82\x01R_\x90s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x84\x16\x90cp\xA0\x821\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x06\xE1W=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x07\x05\x91\x90a\x1B\xFFV[`@Q\x7Fp\xA0\x821\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R0`\x04\x82\x01R\x90\x91P_\x90s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x84\x16\x90cp\xA0\x821\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x07rW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x07\x96\x91\x90a\x1B\xFFV[\x90P\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x85` \x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14a\x081W`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x11`$\x82\x01R\x7Finvalid buy token\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x06lV[`@\x85\x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x15a\x08\xB3W`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1D`$\x82\x01R\x7Freceiver must be zero address\0\0\0`D\x82\x01R`d\x01a\x06lV[a\x08\xBFa\x01,Ba\x1CCV[\x85`\xA0\x01Qc\xFF\xFF\xFF\xFF\x16\x11\x15a\t2W`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1E`$\x82\x01R\x7Fvalidity too far in the future\0\0`D\x82\x01R`d\x01a\x06lV[\x85``\x01Q\x85`\xC0\x01Q\x14a\t\xA3W`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x0F`$\x82\x01R\x7Finvalid appData\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x06lV[`\xE0\x85\x01Q\x15a\n\x0FW`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x17`$\x82\x01R\x7Ffee amount must be zero\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x06lV[\x7FZ(\xE96;\xB9B\xB69'\0b\xAAk\xB2\x95\xF44\xBC\xDF\xC4,\x97&{\xF0\x03\xF2r\x06\r\xC9\x85a\x01`\x01Q\x14a\n\x9DW`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1D`$\x82\x01R\x7FbuyTokenBalance must be erc20\0\0\0`D\x82\x01R`d\x01a\x06lV[\x7FZ(\xE96;\xB9B\xB69'\0b\xAAk\xB2\x95\xF44\xBC\xDF\xC4,\x97&{\xF0\x03\xF2r\x06\r\xC9\x85a\x01@\x01Q\x14a\x0B+W`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1E`$\x82\x01R\x7FsellTokenBalance must be erc20\0\0`D\x82\x01R`d\x01a\x06lV[``\x85\x01Qa\x0B:\x90\x82a\x1CVV[`\x80\x86\x01Q``\x87\x01Qa\x0BN\x90\x85a\x1CmV[a\x0BX\x91\x90a\x1CVV[\x10\x15a\x0B\xC0W`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x17`$\x82\x01R\x7Freceived amount too low\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x06lV[PPPPPPV[_\x81`@Q` \x01a\x0B\xDA\x91\x90a\x1C\xCCV[`@Q` \x81\x83\x03\x03\x81R\x90`@R\x80Q\x90` \x01 \x90P\x91\x90PV[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x163\x14a\x0CfW`@Q\x7F\xF8}\r\x16\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[_a\x0Csa\x02\x97\x83a\x1D'V[\x90P\x80_\x81\x90UP\x80\x7FQ\x0EJOv\x90|-aX\xB3C\xF7\xC4\xF2\xF5\x97\xDF8[r|&\xE9\xEF\x90\xE7P\x93\xAC\xE1\x9A\x83`@Qa\x0C\xAB\x91\x90a\x1DyV[`@Q\x80\x91\x03\x90\xA2PPV[`@\x80Qa\x01\x80\x81\x01\x82R_\x80\x82R` \x82\x01\x81\x90R\x91\x81\x01\x82\x90R``\x81\x01\x82\x90R`\x80\x81\x01\x82\x90R`\xA0\x81\x01\x82\x90R`\xC0\x81\x01\x82\x90R`\xE0\x81\x01\x82\x90Ra\x01\0\x81\x01\x82\x90Ra\x01 \x81\x01\x82\x90Ra\x01@\x81\x01\x82\x90Ra\x01`\x81\x01\x91\x90\x91R_\x80\x83` \x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16c5^\xFD\xD9\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x87`@\x01Q`@Q\x84c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01a\r\x9E\x93\x92\x91\x90a\x1E:V[`@\x80Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\r\xB8W=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\r\xDC\x91\x90a\x1ErV[`@Q\x7Fp\xA0\x821\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R0`\x04\x82\x01R\x91\x93P\x91P_\x90\x81\x90s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x90cp\xA0\x821\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x0EmW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x0E\x91\x91\x90a\x1B\xFFV[`@Q\x7Fp\xA0\x821\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R0`\x04\x82\x01R\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90cp\xA0\x821\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x0F\x19W=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x0F=\x91\x90a\x1B\xFFV[\x90\x92P\x90P_\x80\x80\x80\x80a\x0FQ\x88\x88a\x1CVV[\x90P_a\x0F^\x8A\x88a\x1CVV[\x90P_\x82\x82\x10\x15a\x10\x0BW\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x96P\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x95Pa\x0F\xD6a\x0F\xBD`\x02\x8Ba\x1E\xC1V[a\x0F\xD1\x84a\x0F\xCC\x8E`\x02a\x1CVV[a\x13FV[a\x13~V[\x94Pa\x10\x01\x85a\x0F\xE6\x81\x8Da\x1CVV[a\x0F\xF0\x90\x85a\x1CCV[a\x0F\xFA\x8C\x8Fa\x1CVV[`\x01a\x13\xCBV[\x93P\x84\x90Pa\x10\x98V[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x96P\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x95Pa\x10na\x10_`\x02\x8Aa\x1E\xC1V[a\x0F\xD1\x85a\x0F\xCC\x8F`\x02a\x1CVV[\x94Pa\x10\x92\x85a\x10~\x81\x8Ea\x1CVV[a\x10\x88\x90\x86a\x1CCV[a\x0F\xFA\x8B\x8Ea\x1CVV[\x93P\x83\x90P[\x8CQ\x81\x10\x15a\x10\xDFWa\x10\xDF`@Q\x80`@\x01`@R\x80`\x17\x81R` \x01\x7Ftraded amount too small\0\0\0\0\0\0\0\0\0\x81RPa\x14&V[`@Q\x80a\x01\x80\x01`@R\x80\x88s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x87s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01_s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x86\x81R` \x01\x85\x81R` \x01a\x11Va\x01,a\x14fV[c\xFF\xFF\xFF\xFF\x16\x81R` \x01\x8E``\x01Q\x81R` \x01_\x81R` \x01\x7F\xF3\xB2wr\x8B?\xEEt\x94\x81\xEB>\x0B;H\x98\r\xBB\xABxe\x8F\xC4\x19\x02\\\xB1n\xEE4gu\x81R` \x01`\x01\x15\x15\x81R` \x01\x7FZ(\xE96;\xB9B\xB69'\0b\xAAk\xB2\x95\xF44\xBC\xDF\xC4,\x97&{\xF0\x03\xF2r\x06\r\xC9\x81R` \x01\x7FZ(\xE96;\xB9B\xB69'\0b\xAAk\xB2\x95\xF44\xBC\xDF\xC4,\x97&{\xF0\x03\xF2r\x06\r\xC9\x81RP\x9BPPPPPPPPPPPP\x91\x90PV[3s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x14a\x12kW`@Q\x7F\xBF\x84\x89w\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x80\x7Fl<\x90$TW\x06\x0Fe\x17x{,K\x8C\xF5\0\xCA\x88\x9D#\x04\xAF\x02\x04;\xD5\xB5\x13\xE3\xB5\x93]PV[\x7Fl<\x90$TW\x06\x0Fe\x17x{,K\x8C\xF5\0\xCA\x88\x9D#\x04\xAF\x02\x04;\xD5\xB5\x13\xE3\xB5\x93\\\x83\x81\x14a\x13@W\x80\x15a\x12\xF2W`@Q\x7F\xDA\xFB\xDD\x1F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[_a\x12\xFC\x84a\x0C\xB7V[\x90Pa\x13\x08\x83\x82a\x14\x87V[a\x13>W`@Q\x7F\xD9\xFF$\xC7\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[P[PPPPV[_\x82\x15a\x13sW\x81a\x13Y`\x01\x85a\x1CmV[a\x13c\x91\x90a\x1E\xC1V[a\x13n\x90`\x01a\x1CCV[a\x13uV[_[\x90P[\x92\x91PPV[_\x81\x83\x10\x15a\x13\xC5Wa\x13\xC5`@Q\x80`@\x01`@R\x80`\x15\x81R` \x01\x7Fsubtraction underflow\0\0\0\0\0\0\0\0\0\0\0\x81RPa\x14&V[P\x90\x03\x90V[_\x80a\x13\xD8\x86\x86\x86a\x15\x99V[\x90P`\x01\x83`\x02\x81\x11\x15a\x13\xEEWa\x13\xEEa\x1E\xD4V[\x14\x80\x15a\x14\nWP_\x84\x80a\x14\x05Wa\x14\x05a\x1E\x94V[\x86\x88\t\x11[\x15a\x14\x1DWa\x14\x1A`\x01\x82a\x1CCV[\x90P[\x95\x94PPPPPV[a\x141C`\x01a\x1CCV[\x81`@Q\x7F\x1F\xE8Pn\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01a\x06l\x92\x91\x90a\x1F\x01V[_\x81\x80a\x14s\x81Ba\x1F\x19V[a\x14}\x91\x90a\x1F;V[a\x13x\x91\x90a\x1FcV[_\x80\x82_\x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x84_\x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x90P_\x83` \x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x85` \x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x90P_\x84``\x01Q\x86``\x01Q\x14\x90P_\x85`\x80\x01Q\x87`\x80\x01Q\x14\x90P_\x86`\xA0\x01Qc\xFF\xFF\xFF\xFF\x16\x88`\xA0\x01Qc\xFF\xFF\xFF\xFF\x16\x14\x90P_\x87a\x01\0\x01Q\x89a\x01\0\x01Q\x14\x90P_\x88a\x01 \x01Q\x15\x15\x8Aa\x01 \x01Q\x15\x15\x14\x90P\x86\x80\x15a\x15^WP\x85[\x80\x15a\x15gWP\x84[\x80\x15a\x15pWP\x83[\x80\x15a\x15yWP\x82[\x80\x15a\x15\x82WP\x81[\x80\x15a\x15\x8BWP\x80[\x9A\x99PPPPPPPPPPV[_\x80\x80\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x85\x87\t\x85\x87\x02\x92P\x82\x81\x10\x83\x82\x03\x03\x91PP\x80_\x03a\x15\xEFW\x83\x82\x81a\x15\xE5Wa\x15\xE5a\x1E\x94V[\x04\x92PPPa\x04\xD0V[\x80\x84\x11a\x16XW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x15`$\x82\x01R\x7FMath: mulDiv overflow\0\0\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x06lV[_\x84\x86\x88\t`\x02`\x01\x87\x19\x81\x01\x88\x16\x97\x88\x90\x04`\x03\x81\x02\x83\x18\x80\x82\x02\x84\x03\x02\x80\x82\x02\x84\x03\x02\x80\x82\x02\x84\x03\x02\x80\x82\x02\x84\x03\x02\x80\x82\x02\x84\x03\x02\x90\x81\x02\x90\x92\x03\x90\x91\x02_\x88\x90\x03\x88\x90\x04\x90\x91\x01\x85\x83\x11\x90\x94\x03\x93\x90\x93\x02\x93\x03\x94\x90\x94\x04\x91\x90\x91\x17\x02\x94\x93PPPPV[_\x80_`@\x84\x86\x03\x12\x15a\x16\xD1W_\x80\xFD[\x835\x92P` \x84\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a\x16\xEFW_\x80\xFD[\x81\x86\x01\x91P\x86`\x1F\x83\x01\x12a\x17\x02W_\x80\xFD[\x815\x81\x81\x11\x15a\x17\x10W_\x80\xFD[\x87` \x82\x85\x01\x01\x11\x15a\x17!W_\x80\xFD[` \x83\x01\x94P\x80\x93PPPP\x92P\x92P\x92V[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`A`\x04R`$_\xFD[`@Q`\x80\x81\x01g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x82\x82\x10\x17\x15a\x17\x84Wa\x17\x84a\x174V[`@R\x90V[`@Qa\x01\x80\x81\x01g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x82\x82\x10\x17\x15a\x17\x84Wa\x17\x84a\x174V[`@Q`\x1F\x82\x01\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x16\x81\x01g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x82\x82\x10\x17\x15a\x17\xF5Wa\x17\xF5a\x174V[`@R\x91\x90PV[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x16\x81\x14a\x18\x1EW_\x80\xFD[PV[_`\x80\x82\x84\x03\x12\x15a\x181W_\x80\xFD[a\x189a\x17aV[\x90P\x815\x81R` \x80\x83\x015a\x18N\x81a\x17\xFDV[\x82\x82\x01R`@\x83\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a\x18lW_\x80\xFD[\x81\x85\x01\x91P\x85`\x1F\x83\x01\x12a\x18\x7FW_\x80\xFD[\x815\x81\x81\x11\x15a\x18\x91Wa\x18\x91a\x174V[a\x18\xC1\x84\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`\x1F\x84\x01\x16\x01a\x17\xAEV[\x91P\x80\x82R\x86\x84\x82\x85\x01\x01\x11\x15a\x18\xD6W_\x80\xFD[\x80\x84\x84\x01\x85\x84\x017_\x84\x82\x84\x01\x01RP\x80`@\x85\x01RPPP``\x82\x015``\x82\x01R\x92\x91PPV[\x805a\x19\n\x81a\x17\xFDV[\x91\x90PV[\x805c\xFF\xFF\xFF\xFF\x81\x16\x81\x14a\x19\nW_\x80\xFD[\x805\x80\x15\x15\x81\x14a\x19\nW_\x80\xFD[_a\x01\x80\x82\x84\x03\x12\x15a\x19BW_\x80\xFD[a\x19Ja\x17\x8AV[\x90Pa\x19U\x82a\x18\xFFV[\x81Ra\x19c` \x83\x01a\x18\xFFV[` \x82\x01Ra\x19t`@\x83\x01a\x18\xFFV[`@\x82\x01R``\x82\x015``\x82\x01R`\x80\x82\x015`\x80\x82\x01Ra\x19\x99`\xA0\x83\x01a\x19\x0FV[`\xA0\x82\x01R`\xC0\x82\x015`\xC0\x82\x01R`\xE0\x82\x015`\xE0\x82\x01Ra\x01\0\x80\x83\x015\x81\x83\x01RPa\x01 a\x19\xCC\x81\x84\x01a\x19\"V[\x90\x82\x01Ra\x01@\x82\x81\x015\x90\x82\x01Ra\x01`\x91\x82\x015\x91\x81\x01\x91\x90\x91R\x91\x90PV[_\x80a\x01\xA0\x83\x85\x03\x12\x15a\x1A\0W_\x80\xFD[\x825g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x1A\x16W_\x80\xFD[a\x1A\"\x85\x82\x86\x01a\x18!V[\x92PPa\x1A2\x84` \x85\x01a\x191V[\x90P\x92P\x92\x90PV[_` \x82\x84\x03\x12\x15a\x1AKW_\x80\xFD[\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x1AaW_\x80\xFD[a\x1Am\x84\x82\x85\x01a\x18!V[\x94\x93PPPPV[_` \x82\x84\x03\x12\x15a\x1A\x85W_\x80\xFD[\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x1A\x9BW_\x80\xFD[\x82\x01`\x80\x81\x85\x03\x12\x15a\x04\xD0W_\x80\xFD[\x81Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81Ra\x01\x80\x81\x01` \x83\x01Qa\x1A\xF2` \x84\x01\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90RV[P`@\x83\x01Qa\x1B\x1A`@\x84\x01\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90RV[P``\x83\x01Q``\x83\x01R`\x80\x83\x01Q`\x80\x83\x01R`\xA0\x83\x01Qa\x1BF`\xA0\x84\x01\x82c\xFF\xFF\xFF\xFF\x16\x90RV[P`\xC0\x83\x01Q`\xC0\x83\x01R`\xE0\x83\x01Q`\xE0\x83\x01Ra\x01\0\x80\x84\x01Q\x81\x84\x01RPa\x01 \x80\x84\x01Qa\x1B{\x82\x85\x01\x82\x15\x15\x90RV[PPa\x01@\x83\x81\x01Q\x90\x83\x01Ra\x01`\x92\x83\x01Q\x92\x90\x91\x01\x91\x90\x91R\x90V[_` \x82\x84\x03\x12\x15a\x1B\xAAW_\x80\xFD[P5\x91\x90PV[_\x80a\x01\xA0\x83\x85\x03\x12\x15a\x1B\xC3W_\x80\xFD[a\x1B\xCD\x84\x84a\x191V[\x91Pa\x01\x80\x83\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x1B\xE9W_\x80\xFD[a\x1B\xF5\x85\x82\x86\x01a\x18!V[\x91PP\x92P\x92\x90PV[_` \x82\x84\x03\x12\x15a\x1C\x0FW_\x80\xFD[PQ\x91\x90PV[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x11`\x04R`$_\xFD[\x80\x82\x01\x80\x82\x11\x15a\x13xWa\x13xa\x1C\x16V[\x80\x82\x02\x81\x15\x82\x82\x04\x84\x14\x17a\x13xWa\x13xa\x1C\x16V[\x81\x81\x03\x81\x81\x11\x15a\x13xWa\x13xa\x1C\x16V[_\x81Q\x80\x84R\x80` \x84\x01` \x86\x01^_` \x82\x86\x01\x01R` \x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`\x1F\x83\x01\x16\x85\x01\x01\x91PP\x92\x91PPV[` \x81R\x81Q` \x82\x01Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF` \x83\x01Q\x16`@\x82\x01R_`@\x83\x01Q`\x80``\x84\x01Ra\x1D\x11`\xA0\x84\x01\x82a\x1C\x80V[\x90P``\x84\x01Q`\x80\x84\x01R\x80\x91PP\x92\x91PPV[_a\x13x6\x83a\x18!V[\x81\x83R\x81\x81` \x85\x017P_` \x82\x84\x01\x01R_` \x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`\x1F\x84\x01\x16\x84\x01\x01\x90P\x92\x91PPV[` \x81R\x815` \x82\x01R_` \x83\x015a\x1D\x93\x81a\x17\xFDV[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x16`@\x84\x01RP`@\x83\x015\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE1\x846\x03\x01\x81\x12a\x1D\xE4W_\x80\xFD[\x83\x01` \x81\x01\x905g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x1E\0W_\x80\xFD[\x806\x03\x82\x13\x15a\x1E\x0EW_\x80\xFD[`\x80``\x85\x01Ra\x1E#`\xA0\x85\x01\x82\x84a\x1D2V[\x91PP``\x84\x015`\x80\x84\x01R\x80\x91PP\x92\x91PPV[_s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x86\x16\x83R\x80\x85\x16` \x84\x01RP```@\x83\x01Ra\x14\x1D``\x83\x01\x84a\x1C\x80V[_\x80`@\x83\x85\x03\x12\x15a\x1E\x83W_\x80\xFD[PP\x80Q` \x90\x91\x01Q\x90\x92\x90\x91PV[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x12`\x04R`$_\xFD[_\x82a\x1E\xCFWa\x1E\xCFa\x1E\x94V[P\x04\x90V[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`!`\x04R`$_\xFD[\x82\x81R`@` \x82\x01R_a\x1Am`@\x83\x01\x84a\x1C\x80V[_c\xFF\xFF\xFF\xFF\x80\x84\x16\x80a\x1F/Wa\x1F/a\x1E\x94V[\x92\x16\x91\x90\x91\x04\x92\x91PPV[c\xFF\xFF\xFF\xFF\x81\x81\x16\x83\x82\x16\x02\x80\x82\x16\x91\x90\x82\x81\x14a\x1F[Wa\x1F[a\x1C\x16V[PP\x92\x91PPV[c\xFF\xFF\xFF\xFF\x81\x81\x16\x83\x82\x16\x01\x90\x80\x82\x11\x15a\x1F\x80Wa\x1F\x80a\x1C\x16V[P\x92\x91PPV\xFE\xA2dipfsX\"\x12 \xE3\xFB\"\x8BR]\x90\xB9B\xC7\xE5\x8F\xE2\xE2\x03J\x17\xBD%\x8C\x08/\xD4w@\xE7d\xA7\xBEE\xBA\xC6dsolcC\0\x08\x19\x003",
    );
    /// The runtime bytecode of the contract, as deployed on the network.
    ///
    /// ```text
    ///0x608060405234801561000f575f80fd5b506004361061012f575f3560e01c8063b09aaaca116100ad578063e3e6f5b21161007d578063eec50b9711610063578063eec50b9714610344578063f14fcbc81461034c578063ff2dbc9814610203575f80fd5b8063e3e6f5b2146102fd578063e516715b1461031d575f80fd5b8063b09aaaca14610289578063c5f3d2541461029c578063d21220a7146102af578063d25e0cb6146102d6575f80fd5b80631c7de94111610102578063481c6a75116100e8578063481c6a7514610231578063981a160b14610258578063a029a8d414610276575f80fd5b80631c7de941146102035780633e706e321461020a575f80fd5b80630dfe1681146101335780631303a484146101845780631626ba7e146101b557806317700f01146101f9575b5f80fd5b61015a7f000000000000000000000000000000000000000000000000000000000000000081565b60405173ffffffffffffffffffffffffffffffffffffffff90911681526020015b60405180910390f35b7f6c3c90245457060f6517787b2c4b8cf500ca889d2304af02043bd5b513e3b5935c5b60405190815260200161017b565b6101c86101c33660046116bf565b61035f565b6040517fffffffff00000000000000000000000000000000000000000000000000000000909116815260200161017b565b6102016104d7565b005b6101a75f81565b6101a77f6c3c90245457060f6517787b2c4b8cf500ca889d2304af02043bd5b513e3b59381565b61015a7f000000000000000000000000000000000000000000000000000000000000000081565b61026161012c81565b60405163ffffffff909116815260200161017b565b6102016102843660046119ee565b610573565b6101a7610297366004611a3b565b610bc8565b6102016102aa366004611a75565b610bf7565b61015a7f000000000000000000000000000000000000000000000000000000000000000081565b6101a77f000000000000000000000000000000000000000000000000000000000000000081565b61031061030b366004611a3b565b610cb7565b60405161017b9190611aac565b61015a7f000000000000000000000000000000000000000000000000000000000000000081565b6101a75f5481565b61020161035a366004611b9a565b6111fc565b5f808061036e84860186611bb1565b915091505f5461037d82610bc8565b146103b4576040517ff1a6789000000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0820180517fd5a25ba2e97094ad7d83dc28a6572da797d6b3e7fc6663bd93efb789fc17e48982526101a0822091526040517f190100000000000000000000000000000000000000000000000000000000000081527f00000000000000000000000000000000000000000000000000000000000000006002820152602281019190915260429020868114610494576040517f593fcacd00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b61049f818385611291565b6104a98284610573565b507f1626ba7e00000000000000000000000000000000000000000000000000000000925050505b9392505050565b7f000000000000000000000000000000000000000000000000000000000000000073ffffffffffffffffffffffffffffffffffffffff163314610546576040517ff87d0d1600000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b5f8080556040517fbcb8b8fbdea8aa6dc4ae41213e4da81e605a3d1a56ed851b9355182321c091909190a1565b80517f0000000000000000000000000000000000000000000000000000000000000000907f00000000000000000000000000000000000000000000000000000000000000009073ffffffffffffffffffffffffffffffffffffffff808416911614610677578073ffffffffffffffffffffffffffffffffffffffff16835f015173ffffffffffffffffffffffffffffffffffffffff1614610675576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601260248201527f696e76616c69642073656c6c20746f6b656e000000000000000000000000000060448201526064015b60405180910390fd5b905b6040517f70a082310000000000000000000000000000000000000000000000000000000081523060048201525f9073ffffffffffffffffffffffffffffffffffffffff8416906370a0823190602401602060405180830381865afa1580156106e1573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906107059190611bff565b6040517f70a082310000000000000000000000000000000000000000000000000000000081523060048201529091505f9073ffffffffffffffffffffffffffffffffffffffff8416906370a0823190602401602060405180830381865afa158015610772573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906107969190611bff565b90508273ffffffffffffffffffffffffffffffffffffffff16856020015173ffffffffffffffffffffffffffffffffffffffff1614610831576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601160248201527f696e76616c69642062757920746f6b656e000000000000000000000000000000604482015260640161066c565b604085015173ffffffffffffffffffffffffffffffffffffffff16156108b3576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601d60248201527f7265636569766572206d757374206265207a65726f2061646472657373000000604482015260640161066c565b6108bf61012c42611c43565b8560a0015163ffffffff161115610932576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601e60248201527f76616c696469747920746f6f2066617220696e20746865206675747572650000604482015260640161066c565b85606001518560c00151146109a3576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152600f60248201527f696e76616c696420617070446174610000000000000000000000000000000000604482015260640161066c565b60e085015115610a0f576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601760248201527f66656520616d6f756e74206d757374206265207a65726f000000000000000000604482015260640161066c565b7f5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc985610160015114610a9d576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601d60248201527f627579546f6b656e42616c616e6365206d757374206265206572633230000000604482015260640161066c565b7f5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc985610140015114610b2b576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601e60248201527f73656c6c546f6b656e42616c616e6365206d7573742062652065726332300000604482015260640161066c565b6060850151610b3a9082611c56565b60808601516060870151610b4e9085611c6d565b610b589190611c56565b1015610bc0576040517fc8fc272500000000000000000000000000000000000000000000000000000000815260206004820152601760248201527f726563656976656420616d6f756e7420746f6f206c6f77000000000000000000604482015260640161066c565b505050505050565b5f81604051602001610bda9190611ccc565b604051602081830303815290604052805190602001209050919050565b7f000000000000000000000000000000000000000000000000000000000000000073ffffffffffffffffffffffffffffffffffffffff163314610c66576040517ff87d0d1600000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b5f610c7361029783611d27565b9050805f81905550807f510e4a4f76907c2d6158b343f7c4f2f597df385b727c26e9ef90e75093ace19a83604051610cab9190611d79565b60405180910390a25050565b60408051610180810182525f80825260208201819052918101829052606081018290526080810182905260a0810182905260c0810182905260e081018290526101008101829052610120810182905261014081018290526101608101919091525f80836020015173ffffffffffffffffffffffffffffffffffffffff1663355efdd97f00000000000000000000000000000000000000000000000000000000000000007f000000000000000000000000000000000000000000000000000000000000000087604001516040518463ffffffff1660e01b8152600401610d9e93929190611e3a565b6040805180830381865afa158015610db8573d5f803e3d5ffd5b505050506040513d601f19601f82011682018060405250810190610ddc9190611e72565b6040517f70a0823100000000000000000000000000000000000000000000000000000000815230600482015291935091505f90819073ffffffffffffffffffffffffffffffffffffffff7f000000000000000000000000000000000000000000000000000000000000000016906370a0823190602401602060405180830381865afa158015610e6d573d5f803e3d5ffd5b505050506040513d601f19601f82011682018060405250810190610e919190611bff565b6040517f70a082310000000000000000000000000000000000000000000000000000000081523060048201527f000000000000000000000000000000000000000000000000000000000000000073ffffffffffffffffffffffffffffffffffffffff16906370a0823190602401602060405180830381865afa158015610f19573d5f803e3d5ffd5b505050506040513d601f19601f82011682018060405250810190610f3d9190611bff565b90925090505f80808080610f518888611c56565b90505f610f5e8a88611c56565b90505f8282101561100b577f000000000000000000000000000000000000000000000000000000000000000096507f00000000000000000000000000000000000000000000000000000000000000009550610fd6610fbd60028b611ec1565b610fd184610fcc8e6002611c56565b611346565b61137e565b945061100185610fe6818d611c56565b610ff09085611c43565b610ffa8c8f611c56565b60016113cb565b9350849050611098565b7f000000000000000000000000000000000000000000000000000000000000000096507f0000000000000000000000000000000000000000000000000000000000000000955061106e61105f60028a611ec1565b610fd185610fcc8f6002611c56565b94506110928561107e818e611c56565b6110889086611c43565b610ffa8b8e611c56565b93508390505b8c518110156110df576110df6040518060400160405280601781526020017f74726164656420616d6f756e7420746f6f20736d616c6c000000000000000000815250611426565b6040518061018001604052808873ffffffffffffffffffffffffffffffffffffffff1681526020018773ffffffffffffffffffffffffffffffffffffffff1681526020015f73ffffffffffffffffffffffffffffffffffffffff16815260200186815260200185815260200161115661012c611466565b63ffffffff1681526020018e6060015181526020015f81526020017ff3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee34677581526020016001151581526020017f5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc981526020017f5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc98152509b505050505050505050505050919050565b3373ffffffffffffffffffffffffffffffffffffffff7f0000000000000000000000000000000000000000000000000000000000000000161461126b576040517fbf84897700000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b807f6c3c90245457060f6517787b2c4b8cf500ca889d2304af02043bd5b513e3b5935d50565b7f6c3c90245457060f6517787b2c4b8cf500ca889d2304af02043bd5b513e3b5935c8381146113405780156112f2576040517fdafbdd1f00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b5f6112fc84610cb7565b90506113088382611487565b61133e576040517fd9ff24c700000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b505b50505050565b5f82156113735781611359600185611c6d565b6113639190611ec1565b61136e906001611c43565b611375565b5f5b90505b92915050565b5f818310156113c5576113c56040518060400160405280601581526020017f7375627472616374696f6e20756e646572666c6f770000000000000000000000815250611426565b50900390565b5f806113d8868686611599565b905060018360028111156113ee576113ee611ed4565b14801561140a57505f848061140557611405611e94565b868809115b1561141d5761141a600182611c43565b90505b95945050505050565b611431436001611c43565b816040517f1fe8506e00000000000000000000000000000000000000000000000000000000815260040161066c929190611f01565b5f81806114738142611f19565b61147d9190611f3b565b6113789190611f63565b5f80825f015173ffffffffffffffffffffffffffffffffffffffff16845f015173ffffffffffffffffffffffffffffffffffffffff161490505f836020015173ffffffffffffffffffffffffffffffffffffffff16856020015173ffffffffffffffffffffffffffffffffffffffff161490505f846060015186606001511490505f856080015187608001511490505f8660a0015163ffffffff168860a0015163ffffffff161490505f8761010001518961010001511490505f88610120015115158a6101200151151514905086801561155e5750855b80156115675750845b80156115705750835b80156115795750825b80156115825750815b801561158b5750805b9a9950505050505050505050565b5f80807fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff858709858702925082811083820303915050805f036115ef578382816115e5576115e5611e94565b04925050506104d0565b808411611658576040517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601560248201527f4d6174683a206d756c446976206f766572666c6f770000000000000000000000604482015260640161066c565b5f8486880960026001871981018816978890046003810283188082028403028082028403028082028403028082028403028082028403029081029092039091025f889003889004909101858311909403939093029303949094049190911702949350505050565b5f805f604084860312156116d1575f80fd5b83359250602084013567ffffffffffffffff808211156116ef575f80fd5b818601915086601f830112611702575f80fd5b813581811115611710575f80fd5b876020828501011115611721575f80fd5b6020830194508093505050509250925092565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52604160045260245ffd5b6040516080810167ffffffffffffffff8111828210171561178457611784611734565b60405290565b604051610180810167ffffffffffffffff8111828210171561178457611784611734565b604051601f82017fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe016810167ffffffffffffffff811182821017156117f5576117f5611734565b604052919050565b73ffffffffffffffffffffffffffffffffffffffff8116811461181e575f80fd5b50565b5f60808284031215611831575f80fd5b611839611761565b90508135815260208083013561184e816117fd565b82820152604083013567ffffffffffffffff8082111561186c575f80fd5b818501915085601f83011261187f575f80fd5b81358181111561189157611891611734565b6118c1847fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f840116016117ae565b915080825286848285010111156118d6575f80fd5b80848401858401375f848284010152508060408501525050506060820135606082015292915050565b803561190a816117fd565b919050565b803563ffffffff8116811461190a575f80fd5b8035801515811461190a575f80fd5b5f6101808284031215611942575f80fd5b61194a61178a565b9050611955826118ff565b8152611963602083016118ff565b6020820152611974604083016118ff565b6040820152606082013560608201526080820135608082015261199960a0830161190f565b60a082015260c082013560c082015260e082013560e08201526101008083013581830152506101206119cc818401611922565b9082015261014082810135908201526101609182013591810191909152919050565b5f806101a08385031215611a00575f80fd5b823567ffffffffffffffff811115611a16575f80fd5b611a2285828601611821565b925050611a328460208501611931565b90509250929050565b5f60208284031215611a4b575f80fd5b813567ffffffffffffffff811115611a61575f80fd5b611a6d84828501611821565b949350505050565b5f60208284031215611a85575f80fd5b813567ffffffffffffffff811115611a9b575f80fd5b8201608081850312156104d0575f80fd5b815173ffffffffffffffffffffffffffffffffffffffff16815261018081016020830151611af2602084018273ffffffffffffffffffffffffffffffffffffffff169052565b506040830151611b1a604084018273ffffffffffffffffffffffffffffffffffffffff169052565b50606083015160608301526080830151608083015260a0830151611b4660a084018263ffffffff169052565b5060c083015160c083015260e083015160e083015261010080840151818401525061012080840151611b7b8285018215159052565b5050610140838101519083015261016092830151929091019190915290565b5f60208284031215611baa575f80fd5b5035919050565b5f806101a08385031215611bc3575f80fd5b611bcd8484611931565b915061018083013567ffffffffffffffff811115611be9575f80fd5b611bf585828601611821565b9150509250929050565b5f60208284031215611c0f575f80fd5b5051919050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52601160045260245ffd5b8082018082111561137857611378611c16565b808202811582820484141761137857611378611c16565b8181038181111561137857611378611c16565b5f81518084528060208401602086015e5f6020828601015260207fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f83011685010191505092915050565b602081528151602082015273ffffffffffffffffffffffffffffffffffffffff60208301511660408201525f604083015160806060840152611d1160a0840182611c80565b9050606084015160808401528091505092915050565b5f6113783683611821565b81835281816020850137505f602082840101525f60207fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f840116840101905092915050565b60208152813560208201525f6020830135611d93816117fd565b73ffffffffffffffffffffffffffffffffffffffff811660408401525060408301357fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe1843603018112611de4575f80fd5b830160208101903567ffffffffffffffff811115611e00575f80fd5b803603821315611e0e575f80fd5b60806060850152611e2360a085018284611d32565b915050606084013560808401528091505092915050565b5f73ffffffffffffffffffffffffffffffffffffffff80861683528085166020840152506060604083015261141d6060830184611c80565b5f8060408385031215611e83575f80fd5b505080516020909101519092909150565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52601260045260245ffd5b5f82611ecf57611ecf611e94565b500490565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52602160045260245ffd5b828152604060208201525f611a6d6040830184611c80565b5f63ffffffff80841680611f2f57611f2f611e94565b92169190910492915050565b63ffffffff818116838216028082169190828114611f5b57611f5b611c16565b505092915050565b63ffffffff818116838216019080821115611f8057611f80611c16565b509291505056fea2646970667358221220e3fb228b525d90b942c7e58fe2e2034a17bd258c082fd47740e764a7be45bac664736f6c63430008190033
    /// ```
    #[rustfmt::skip]
    #[allow(clippy::all)]
    pub static DEPLOYED_BYTECODE: alloy_sol_types::private::Bytes = alloy_sol_types::private::Bytes::from_static(
        b"`\x80`@R4\x80\x15a\0\x0FW_\x80\xFD[P`\x046\x10a\x01/W_5`\xE0\x1C\x80c\xB0\x9A\xAA\xCA\x11a\0\xADW\x80c\xE3\xE6\xF5\xB2\x11a\0}W\x80c\xEE\xC5\x0B\x97\x11a\0cW\x80c\xEE\xC5\x0B\x97\x14a\x03DW\x80c\xF1O\xCB\xC8\x14a\x03LW\x80c\xFF-\xBC\x98\x14a\x02\x03W_\x80\xFD[\x80c\xE3\xE6\xF5\xB2\x14a\x02\xFDW\x80c\xE5\x16q[\x14a\x03\x1DW_\x80\xFD[\x80c\xB0\x9A\xAA\xCA\x14a\x02\x89W\x80c\xC5\xF3\xD2T\x14a\x02\x9CW\x80c\xD2\x12 \xA7\x14a\x02\xAFW\x80c\xD2^\x0C\xB6\x14a\x02\xD6W_\x80\xFD[\x80c\x1C}\xE9A\x11a\x01\x02W\x80cH\x1Cju\x11a\0\xE8W\x80cH\x1Cju\x14a\x021W\x80c\x98\x1A\x16\x0B\x14a\x02XW\x80c\xA0)\xA8\xD4\x14a\x02vW_\x80\xFD[\x80c\x1C}\xE9A\x14a\x02\x03W\x80c>pn2\x14a\x02\nW_\x80\xFD[\x80c\r\xFE\x16\x81\x14a\x013W\x80c\x13\x03\xA4\x84\x14a\x01\x84W\x80c\x16&\xBA~\x14a\x01\xB5W\x80c\x17p\x0F\x01\x14a\x01\xF9W[_\x80\xFD[a\x01Z\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[`@Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x90\x91\x16\x81R` \x01[`@Q\x80\x91\x03\x90\xF3[\x7Fl<\x90$TW\x06\x0Fe\x17x{,K\x8C\xF5\0\xCA\x88\x9D#\x04\xAF\x02\x04;\xD5\xB5\x13\xE3\xB5\x93\\[`@Q\x90\x81R` \x01a\x01{V[a\x01\xC8a\x01\xC36`\x04a\x16\xBFV[a\x03_V[`@Q\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90\x91\x16\x81R` \x01a\x01{V[a\x02\x01a\x04\xD7V[\0[a\x01\xA7_\x81V[a\x01\xA7\x7Fl<\x90$TW\x06\x0Fe\x17x{,K\x8C\xF5\0\xCA\x88\x9D#\x04\xAF\x02\x04;\xD5\xB5\x13\xE3\xB5\x93\x81V[a\x01Z\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[a\x02aa\x01,\x81V[`@Qc\xFF\xFF\xFF\xFF\x90\x91\x16\x81R` \x01a\x01{V[a\x02\x01a\x02\x846`\x04a\x19\xEEV[a\x05sV[a\x01\xA7a\x02\x976`\x04a\x1A;V[a\x0B\xC8V[a\x02\x01a\x02\xAA6`\x04a\x1AuV[a\x0B\xF7V[a\x01Z\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[a\x01\xA7\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[a\x03\x10a\x03\x0B6`\x04a\x1A;V[a\x0C\xB7V[`@Qa\x01{\x91\x90a\x1A\xACV[a\x01Z\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[a\x01\xA7_T\x81V[a\x02\x01a\x03Z6`\x04a\x1B\x9AV[a\x11\xFCV[_\x80\x80a\x03n\x84\x86\x01\x86a\x1B\xB1V[\x91P\x91P_Ta\x03}\x82a\x0B\xC8V[\x14a\x03\xB4W`@Q\x7F\xF1\xA6x\x90\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x82\x01\x80Q\x7F\xD5\xA2[\xA2\xE9p\x94\xAD}\x83\xDC(\xA6W-\xA7\x97\xD6\xB3\xE7\xFCfc\xBD\x93\xEF\xB7\x89\xFC\x17\xE4\x89\x82Ra\x01\xA0\x82 \x91R`@Q\x7F\x19\x01\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\x02\x82\x01R`\"\x81\x01\x91\x90\x91R`B\x90 \x86\x81\x14a\x04\x94W`@Q\x7FY?\xCA\xCD\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\x04\x9F\x81\x83\x85a\x12\x91V[a\x04\xA9\x82\x84a\x05sV[P\x7F\x16&\xBA~\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x92PPP[\x93\x92PPPV[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x163\x14a\x05FW`@Q\x7F\xF8}\r\x16\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[_\x80\x80U`@Q\x7F\xBC\xB8\xB8\xFB\xDE\xA8\xAAm\xC4\xAEA!>M\xA8\x1E`Z=\x1AV\xED\x85\x1B\x93U\x18#!\xC0\x91\x90\x91\x90\xA1V[\x80Q\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x84\x16\x91\x16\x14a\x06wW\x80s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x83_\x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14a\x06uW`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x12`$\x82\x01R\x7Finvalid sell token\0\0\0\0\0\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01[`@Q\x80\x91\x03\x90\xFD[\x90[`@Q\x7Fp\xA0\x821\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R0`\x04\x82\x01R_\x90s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x84\x16\x90cp\xA0\x821\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x06\xE1W=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x07\x05\x91\x90a\x1B\xFFV[`@Q\x7Fp\xA0\x821\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R0`\x04\x82\x01R\x90\x91P_\x90s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x84\x16\x90cp\xA0\x821\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x07rW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x07\x96\x91\x90a\x1B\xFFV[\x90P\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x85` \x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14a\x081W`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x11`$\x82\x01R\x7Finvalid buy token\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x06lV[`@\x85\x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x15a\x08\xB3W`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1D`$\x82\x01R\x7Freceiver must be zero address\0\0\0`D\x82\x01R`d\x01a\x06lV[a\x08\xBFa\x01,Ba\x1CCV[\x85`\xA0\x01Qc\xFF\xFF\xFF\xFF\x16\x11\x15a\t2W`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1E`$\x82\x01R\x7Fvalidity too far in the future\0\0`D\x82\x01R`d\x01a\x06lV[\x85``\x01Q\x85`\xC0\x01Q\x14a\t\xA3W`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x0F`$\x82\x01R\x7Finvalid appData\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x06lV[`\xE0\x85\x01Q\x15a\n\x0FW`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x17`$\x82\x01R\x7Ffee amount must be zero\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x06lV[\x7FZ(\xE96;\xB9B\xB69'\0b\xAAk\xB2\x95\xF44\xBC\xDF\xC4,\x97&{\xF0\x03\xF2r\x06\r\xC9\x85a\x01`\x01Q\x14a\n\x9DW`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1D`$\x82\x01R\x7FbuyTokenBalance must be erc20\0\0\0`D\x82\x01R`d\x01a\x06lV[\x7FZ(\xE96;\xB9B\xB69'\0b\xAAk\xB2\x95\xF44\xBC\xDF\xC4,\x97&{\xF0\x03\xF2r\x06\r\xC9\x85a\x01@\x01Q\x14a\x0B+W`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x1E`$\x82\x01R\x7FsellTokenBalance must be erc20\0\0`D\x82\x01R`d\x01a\x06lV[``\x85\x01Qa\x0B:\x90\x82a\x1CVV[`\x80\x86\x01Q``\x87\x01Qa\x0BN\x90\x85a\x1CmV[a\x0BX\x91\x90a\x1CVV[\x10\x15a\x0B\xC0W`@Q\x7F\xC8\xFC'%\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x17`$\x82\x01R\x7Freceived amount too low\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x06lV[PPPPPPV[_\x81`@Q` \x01a\x0B\xDA\x91\x90a\x1C\xCCV[`@Q` \x81\x83\x03\x03\x81R\x90`@R\x80Q\x90` \x01 \x90P\x91\x90PV[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x163\x14a\x0CfW`@Q\x7F\xF8}\r\x16\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[_a\x0Csa\x02\x97\x83a\x1D'V[\x90P\x80_\x81\x90UP\x80\x7FQ\x0EJOv\x90|-aX\xB3C\xF7\xC4\xF2\xF5\x97\xDF8[r|&\xE9\xEF\x90\xE7P\x93\xAC\xE1\x9A\x83`@Qa\x0C\xAB\x91\x90a\x1DyV[`@Q\x80\x91\x03\x90\xA2PPV[`@\x80Qa\x01\x80\x81\x01\x82R_\x80\x82R` \x82\x01\x81\x90R\x91\x81\x01\x82\x90R``\x81\x01\x82\x90R`\x80\x81\x01\x82\x90R`\xA0\x81\x01\x82\x90R`\xC0\x81\x01\x82\x90R`\xE0\x81\x01\x82\x90Ra\x01\0\x81\x01\x82\x90Ra\x01 \x81\x01\x82\x90Ra\x01@\x81\x01\x82\x90Ra\x01`\x81\x01\x91\x90\x91R_\x80\x83` \x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16c5^\xFD\xD9\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x87`@\x01Q`@Q\x84c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01a\r\x9E\x93\x92\x91\x90a\x1E:V[`@\x80Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\r\xB8W=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\r\xDC\x91\x90a\x1ErV[`@Q\x7Fp\xA0\x821\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R0`\x04\x82\x01R\x91\x93P\x91P_\x90\x81\x90s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x90cp\xA0\x821\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x0EmW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x0E\x91\x91\x90a\x1B\xFFV[`@Q\x7Fp\xA0\x821\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R0`\x04\x82\x01R\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90cp\xA0\x821\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x0F\x19W=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x0F=\x91\x90a\x1B\xFFV[\x90\x92P\x90P_\x80\x80\x80\x80a\x0FQ\x88\x88a\x1CVV[\x90P_a\x0F^\x8A\x88a\x1CVV[\x90P_\x82\x82\x10\x15a\x10\x0BW\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x96P\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x95Pa\x0F\xD6a\x0F\xBD`\x02\x8Ba\x1E\xC1V[a\x0F\xD1\x84a\x0F\xCC\x8E`\x02a\x1CVV[a\x13FV[a\x13~V[\x94Pa\x10\x01\x85a\x0F\xE6\x81\x8Da\x1CVV[a\x0F\xF0\x90\x85a\x1CCV[a\x0F\xFA\x8C\x8Fa\x1CVV[`\x01a\x13\xCBV[\x93P\x84\x90Pa\x10\x98V[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x96P\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x95Pa\x10na\x10_`\x02\x8Aa\x1E\xC1V[a\x0F\xD1\x85a\x0F\xCC\x8F`\x02a\x1CVV[\x94Pa\x10\x92\x85a\x10~\x81\x8Ea\x1CVV[a\x10\x88\x90\x86a\x1CCV[a\x0F\xFA\x8B\x8Ea\x1CVV[\x93P\x83\x90P[\x8CQ\x81\x10\x15a\x10\xDFWa\x10\xDF`@Q\x80`@\x01`@R\x80`\x17\x81R` \x01\x7Ftraded amount too small\0\0\0\0\0\0\0\0\0\x81RPa\x14&V[`@Q\x80a\x01\x80\x01`@R\x80\x88s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x87s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01_s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x86\x81R` \x01\x85\x81R` \x01a\x11Va\x01,a\x14fV[c\xFF\xFF\xFF\xFF\x16\x81R` \x01\x8E``\x01Q\x81R` \x01_\x81R` \x01\x7F\xF3\xB2wr\x8B?\xEEt\x94\x81\xEB>\x0B;H\x98\r\xBB\xABxe\x8F\xC4\x19\x02\\\xB1n\xEE4gu\x81R` \x01`\x01\x15\x15\x81R` \x01\x7FZ(\xE96;\xB9B\xB69'\0b\xAAk\xB2\x95\xF44\xBC\xDF\xC4,\x97&{\xF0\x03\xF2r\x06\r\xC9\x81R` \x01\x7FZ(\xE96;\xB9B\xB69'\0b\xAAk\xB2\x95\xF44\xBC\xDF\xC4,\x97&{\xF0\x03\xF2r\x06\r\xC9\x81RP\x9BPPPPPPPPPPPP\x91\x90PV[3s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x14a\x12kW`@Q\x7F\xBF\x84\x89w\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x80\x7Fl<\x90$TW\x06\x0Fe\x17x{,K\x8C\xF5\0\xCA\x88\x9D#\x04\xAF\x02\x04;\xD5\xB5\x13\xE3\xB5\x93]PV[\x7Fl<\x90$TW\x06\x0Fe\x17x{,K\x8C\xF5\0\xCA\x88\x9D#\x04\xAF\x02\x04;\xD5\xB5\x13\xE3\xB5\x93\\\x83\x81\x14a\x13@W\x80\x15a\x12\xF2W`@Q\x7F\xDA\xFB\xDD\x1F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[_a\x12\xFC\x84a\x0C\xB7V[\x90Pa\x13\x08\x83\x82a\x14\x87V[a\x13>W`@Q\x7F\xD9\xFF$\xC7\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[P[PPPPV[_\x82\x15a\x13sW\x81a\x13Y`\x01\x85a\x1CmV[a\x13c\x91\x90a\x1E\xC1V[a\x13n\x90`\x01a\x1CCV[a\x13uV[_[\x90P[\x92\x91PPV[_\x81\x83\x10\x15a\x13\xC5Wa\x13\xC5`@Q\x80`@\x01`@R\x80`\x15\x81R` \x01\x7Fsubtraction underflow\0\0\0\0\0\0\0\0\0\0\0\x81RPa\x14&V[P\x90\x03\x90V[_\x80a\x13\xD8\x86\x86\x86a\x15\x99V[\x90P`\x01\x83`\x02\x81\x11\x15a\x13\xEEWa\x13\xEEa\x1E\xD4V[\x14\x80\x15a\x14\nWP_\x84\x80a\x14\x05Wa\x14\x05a\x1E\x94V[\x86\x88\t\x11[\x15a\x14\x1DWa\x14\x1A`\x01\x82a\x1CCV[\x90P[\x95\x94PPPPPV[a\x141C`\x01a\x1CCV[\x81`@Q\x7F\x1F\xE8Pn\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01a\x06l\x92\x91\x90a\x1F\x01V[_\x81\x80a\x14s\x81Ba\x1F\x19V[a\x14}\x91\x90a\x1F;V[a\x13x\x91\x90a\x1FcV[_\x80\x82_\x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x84_\x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x90P_\x83` \x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x85` \x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x90P_\x84``\x01Q\x86``\x01Q\x14\x90P_\x85`\x80\x01Q\x87`\x80\x01Q\x14\x90P_\x86`\xA0\x01Qc\xFF\xFF\xFF\xFF\x16\x88`\xA0\x01Qc\xFF\xFF\xFF\xFF\x16\x14\x90P_\x87a\x01\0\x01Q\x89a\x01\0\x01Q\x14\x90P_\x88a\x01 \x01Q\x15\x15\x8Aa\x01 \x01Q\x15\x15\x14\x90P\x86\x80\x15a\x15^WP\x85[\x80\x15a\x15gWP\x84[\x80\x15a\x15pWP\x83[\x80\x15a\x15yWP\x82[\x80\x15a\x15\x82WP\x81[\x80\x15a\x15\x8BWP\x80[\x9A\x99PPPPPPPPPPV[_\x80\x80\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x85\x87\t\x85\x87\x02\x92P\x82\x81\x10\x83\x82\x03\x03\x91PP\x80_\x03a\x15\xEFW\x83\x82\x81a\x15\xE5Wa\x15\xE5a\x1E\x94V[\x04\x92PPPa\x04\xD0V[\x80\x84\x11a\x16XW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x15`$\x82\x01R\x7FMath: mulDiv overflow\0\0\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x06lV[_\x84\x86\x88\t`\x02`\x01\x87\x19\x81\x01\x88\x16\x97\x88\x90\x04`\x03\x81\x02\x83\x18\x80\x82\x02\x84\x03\x02\x80\x82\x02\x84\x03\x02\x80\x82\x02\x84\x03\x02\x80\x82\x02\x84\x03\x02\x80\x82\x02\x84\x03\x02\x90\x81\x02\x90\x92\x03\x90\x91\x02_\x88\x90\x03\x88\x90\x04\x90\x91\x01\x85\x83\x11\x90\x94\x03\x93\x90\x93\x02\x93\x03\x94\x90\x94\x04\x91\x90\x91\x17\x02\x94\x93PPPPV[_\x80_`@\x84\x86\x03\x12\x15a\x16\xD1W_\x80\xFD[\x835\x92P` \x84\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a\x16\xEFW_\x80\xFD[\x81\x86\x01\x91P\x86`\x1F\x83\x01\x12a\x17\x02W_\x80\xFD[\x815\x81\x81\x11\x15a\x17\x10W_\x80\xFD[\x87` \x82\x85\x01\x01\x11\x15a\x17!W_\x80\xFD[` \x83\x01\x94P\x80\x93PPPP\x92P\x92P\x92V[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`A`\x04R`$_\xFD[`@Q`\x80\x81\x01g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x82\x82\x10\x17\x15a\x17\x84Wa\x17\x84a\x174V[`@R\x90V[`@Qa\x01\x80\x81\x01g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x82\x82\x10\x17\x15a\x17\x84Wa\x17\x84a\x174V[`@Q`\x1F\x82\x01\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x16\x81\x01g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x82\x82\x10\x17\x15a\x17\xF5Wa\x17\xF5a\x174V[`@R\x91\x90PV[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x16\x81\x14a\x18\x1EW_\x80\xFD[PV[_`\x80\x82\x84\x03\x12\x15a\x181W_\x80\xFD[a\x189a\x17aV[\x90P\x815\x81R` \x80\x83\x015a\x18N\x81a\x17\xFDV[\x82\x82\x01R`@\x83\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a\x18lW_\x80\xFD[\x81\x85\x01\x91P\x85`\x1F\x83\x01\x12a\x18\x7FW_\x80\xFD[\x815\x81\x81\x11\x15a\x18\x91Wa\x18\x91a\x174V[a\x18\xC1\x84\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`\x1F\x84\x01\x16\x01a\x17\xAEV[\x91P\x80\x82R\x86\x84\x82\x85\x01\x01\x11\x15a\x18\xD6W_\x80\xFD[\x80\x84\x84\x01\x85\x84\x017_\x84\x82\x84\x01\x01RP\x80`@\x85\x01RPPP``\x82\x015``\x82\x01R\x92\x91PPV[\x805a\x19\n\x81a\x17\xFDV[\x91\x90PV[\x805c\xFF\xFF\xFF\xFF\x81\x16\x81\x14a\x19\nW_\x80\xFD[\x805\x80\x15\x15\x81\x14a\x19\nW_\x80\xFD[_a\x01\x80\x82\x84\x03\x12\x15a\x19BW_\x80\xFD[a\x19Ja\x17\x8AV[\x90Pa\x19U\x82a\x18\xFFV[\x81Ra\x19c` \x83\x01a\x18\xFFV[` \x82\x01Ra\x19t`@\x83\x01a\x18\xFFV[`@\x82\x01R``\x82\x015``\x82\x01R`\x80\x82\x015`\x80\x82\x01Ra\x19\x99`\xA0\x83\x01a\x19\x0FV[`\xA0\x82\x01R`\xC0\x82\x015`\xC0\x82\x01R`\xE0\x82\x015`\xE0\x82\x01Ra\x01\0\x80\x83\x015\x81\x83\x01RPa\x01 a\x19\xCC\x81\x84\x01a\x19\"V[\x90\x82\x01Ra\x01@\x82\x81\x015\x90\x82\x01Ra\x01`\x91\x82\x015\x91\x81\x01\x91\x90\x91R\x91\x90PV[_\x80a\x01\xA0\x83\x85\x03\x12\x15a\x1A\0W_\x80\xFD[\x825g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x1A\x16W_\x80\xFD[a\x1A\"\x85\x82\x86\x01a\x18!V[\x92PPa\x1A2\x84` \x85\x01a\x191V[\x90P\x92P\x92\x90PV[_` \x82\x84\x03\x12\x15a\x1AKW_\x80\xFD[\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x1AaW_\x80\xFD[a\x1Am\x84\x82\x85\x01a\x18!V[\x94\x93PPPPV[_` \x82\x84\x03\x12\x15a\x1A\x85W_\x80\xFD[\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x1A\x9BW_\x80\xFD[\x82\x01`\x80\x81\x85\x03\x12\x15a\x04\xD0W_\x80\xFD[\x81Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81Ra\x01\x80\x81\x01` \x83\x01Qa\x1A\xF2` \x84\x01\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90RV[P`@\x83\x01Qa\x1B\x1A`@\x84\x01\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90RV[P``\x83\x01Q``\x83\x01R`\x80\x83\x01Q`\x80\x83\x01R`\xA0\x83\x01Qa\x1BF`\xA0\x84\x01\x82c\xFF\xFF\xFF\xFF\x16\x90RV[P`\xC0\x83\x01Q`\xC0\x83\x01R`\xE0\x83\x01Q`\xE0\x83\x01Ra\x01\0\x80\x84\x01Q\x81\x84\x01RPa\x01 \x80\x84\x01Qa\x1B{\x82\x85\x01\x82\x15\x15\x90RV[PPa\x01@\x83\x81\x01Q\x90\x83\x01Ra\x01`\x92\x83\x01Q\x92\x90\x91\x01\x91\x90\x91R\x90V[_` \x82\x84\x03\x12\x15a\x1B\xAAW_\x80\xFD[P5\x91\x90PV[_\x80a\x01\xA0\x83\x85\x03\x12\x15a\x1B\xC3W_\x80\xFD[a\x1B\xCD\x84\x84a\x191V[\x91Pa\x01\x80\x83\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x1B\xE9W_\x80\xFD[a\x1B\xF5\x85\x82\x86\x01a\x18!V[\x91PP\x92P\x92\x90PV[_` \x82\x84\x03\x12\x15a\x1C\x0FW_\x80\xFD[PQ\x91\x90PV[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x11`\x04R`$_\xFD[\x80\x82\x01\x80\x82\x11\x15a\x13xWa\x13xa\x1C\x16V[\x80\x82\x02\x81\x15\x82\x82\x04\x84\x14\x17a\x13xWa\x13xa\x1C\x16V[\x81\x81\x03\x81\x81\x11\x15a\x13xWa\x13xa\x1C\x16V[_\x81Q\x80\x84R\x80` \x84\x01` \x86\x01^_` \x82\x86\x01\x01R` \x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`\x1F\x83\x01\x16\x85\x01\x01\x91PP\x92\x91PPV[` \x81R\x81Q` \x82\x01Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF` \x83\x01Q\x16`@\x82\x01R_`@\x83\x01Q`\x80``\x84\x01Ra\x1D\x11`\xA0\x84\x01\x82a\x1C\x80V[\x90P``\x84\x01Q`\x80\x84\x01R\x80\x91PP\x92\x91PPV[_a\x13x6\x83a\x18!V[\x81\x83R\x81\x81` \x85\x017P_` \x82\x84\x01\x01R_` \x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`\x1F\x84\x01\x16\x84\x01\x01\x90P\x92\x91PPV[` \x81R\x815` \x82\x01R_` \x83\x015a\x1D\x93\x81a\x17\xFDV[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x16`@\x84\x01RP`@\x83\x015\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE1\x846\x03\x01\x81\x12a\x1D\xE4W_\x80\xFD[\x83\x01` \x81\x01\x905g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x1E\0W_\x80\xFD[\x806\x03\x82\x13\x15a\x1E\x0EW_\x80\xFD[`\x80``\x85\x01Ra\x1E#`\xA0\x85\x01\x82\x84a\x1D2V[\x91PP``\x84\x015`\x80\x84\x01R\x80\x91PP\x92\x91PPV[_s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x86\x16\x83R\x80\x85\x16` \x84\x01RP```@\x83\x01Ra\x14\x1D``\x83\x01\x84a\x1C\x80V[_\x80`@\x83\x85\x03\x12\x15a\x1E\x83W_\x80\xFD[PP\x80Q` \x90\x91\x01Q\x90\x92\x90\x91PV[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x12`\x04R`$_\xFD[_\x82a\x1E\xCFWa\x1E\xCFa\x1E\x94V[P\x04\x90V[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`!`\x04R`$_\xFD[\x82\x81R`@` \x82\x01R_a\x1Am`@\x83\x01\x84a\x1C\x80V[_c\xFF\xFF\xFF\xFF\x80\x84\x16\x80a\x1F/Wa\x1F/a\x1E\x94V[\x92\x16\x91\x90\x91\x04\x92\x91PPV[c\xFF\xFF\xFF\xFF\x81\x81\x16\x83\x82\x16\x02\x80\x82\x16\x91\x90\x82\x81\x14a\x1F[Wa\x1F[a\x1C\x16V[PP\x92\x91PPV[c\xFF\xFF\xFF\xFF\x81\x81\x16\x83\x82\x16\x01\x90\x80\x82\x11\x15a\x1F\x80Wa\x1F\x80a\x1C\x16V[P\x92\x91PPV\xFE\xA2dipfsX\"\x12 \xE3\xFB\"\x8BR]\x90\xB9B\xC7\xE5\x8F\xE2\xE2\x03J\x17\xBD%\x8C\x08/\xD4w@\xE7d\xA7\xBEE\xBA\xC6dsolcC\0\x08\x19\x003",
    );
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Custom error with signature `CommitOutsideOfSettlement()` and selector `0xbf848977`.
```solidity
error CommitOutsideOfSettlement();
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct CommitOutsideOfSettlement;
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
        impl ::core::convert::From<CommitOutsideOfSettlement>
        for UnderlyingRustTuple<'_> {
            fn from(value: CommitOutsideOfSettlement) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>>
        for CommitOutsideOfSettlement {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for CommitOutsideOfSettlement {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "CommitOutsideOfSettlement()";
            const SELECTOR: [u8; 4] = [191u8, 132u8, 137u8, 119u8];
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
    /**Custom error with signature `OnlyManagerCanCall()` and selector `0xf87d0d16`.
```solidity
error OnlyManagerCanCall();
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct OnlyManagerCanCall;
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
        impl ::core::convert::From<OnlyManagerCanCall> for UnderlyingRustTuple<'_> {
            fn from(value: OnlyManagerCanCall) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for OnlyManagerCanCall {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for OnlyManagerCanCall {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "OnlyManagerCanCall()";
            const SELECTOR: [u8; 4] = [248u8, 125u8, 13u8, 22u8];
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
    /**Custom error with signature `OrderDoesNotMatchCommitmentHash()` and selector `0xdafbdd1f`.
```solidity
error OrderDoesNotMatchCommitmentHash();
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct OrderDoesNotMatchCommitmentHash;
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
        impl ::core::convert::From<OrderDoesNotMatchCommitmentHash>
        for UnderlyingRustTuple<'_> {
            fn from(value: OrderDoesNotMatchCommitmentHash) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>>
        for OrderDoesNotMatchCommitmentHash {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for OrderDoesNotMatchCommitmentHash {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "OrderDoesNotMatchCommitmentHash()";
            const SELECTOR: [u8; 4] = [218u8, 251u8, 221u8, 31u8];
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
    /**Custom error with signature `OrderDoesNotMatchDefaultTradeableOrder()` and selector `0xd9ff24c7`.
```solidity
error OrderDoesNotMatchDefaultTradeableOrder();
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct OrderDoesNotMatchDefaultTradeableOrder;
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
        impl ::core::convert::From<OrderDoesNotMatchDefaultTradeableOrder>
        for UnderlyingRustTuple<'_> {
            fn from(value: OrderDoesNotMatchDefaultTradeableOrder) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>>
        for OrderDoesNotMatchDefaultTradeableOrder {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for OrderDoesNotMatchDefaultTradeableOrder {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "OrderDoesNotMatchDefaultTradeableOrder()";
            const SELECTOR: [u8; 4] = [217u8, 255u8, 36u8, 199u8];
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
    /**Custom error with signature `OrderDoesNotMatchMessageHash()` and selector `0x593fcacd`.
```solidity
error OrderDoesNotMatchMessageHash();
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct OrderDoesNotMatchMessageHash;
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
        impl ::core::convert::From<OrderDoesNotMatchMessageHash>
        for UnderlyingRustTuple<'_> {
            fn from(value: OrderDoesNotMatchMessageHash) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>>
        for OrderDoesNotMatchMessageHash {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for OrderDoesNotMatchMessageHash {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "OrderDoesNotMatchMessageHash()";
            const SELECTOR: [u8; 4] = [89u8, 63u8, 202u8, 205u8];
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
    /**Custom error with signature `PollTryAtBlock(uint256,string)` and selector `0x1fe8506e`.
```solidity
error PollTryAtBlock(uint256 blockNumber, string message);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct PollTryAtBlock {
        #[allow(missing_docs)]
        pub blockNumber: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub message: alloy_sol_types::private::String,
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
            alloy_sol_types::sol_data::String,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::primitives::aliases::U256,
            alloy_sol_types::private::String,
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
        impl ::core::convert::From<PollTryAtBlock> for UnderlyingRustTuple<'_> {
            fn from(value: PollTryAtBlock) -> Self {
                (value.blockNumber, value.message)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for PollTryAtBlock {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    blockNumber: tuple.0,
                    message: tuple.1,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for PollTryAtBlock {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "PollTryAtBlock(uint256,string)";
            const SELECTOR: [u8; 4] = [31u8, 232u8, 80u8, 110u8];
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
                    > as alloy_sol_types::SolType>::tokenize(&self.blockNumber),
                    <alloy_sol_types::sol_data::String as alloy_sol_types::SolType>::tokenize(
                        &self.message,
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
    /**Custom error with signature `TradingParamsDoNotMatchHash()` and selector `0xf1a67890`.
```solidity
error TradingParamsDoNotMatchHash();
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct TradingParamsDoNotMatchHash;
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
        impl ::core::convert::From<TradingParamsDoNotMatchHash>
        for UnderlyingRustTuple<'_> {
            fn from(value: TradingParamsDoNotMatchHash) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>>
        for TradingParamsDoNotMatchHash {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for TradingParamsDoNotMatchHash {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "TradingParamsDoNotMatchHash()";
            const SELECTOR: [u8; 4] = [241u8, 166u8, 120u8, 144u8];
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
    /**Event with signature `TradingDisabled()` and selector `0xbcb8b8fbdea8aa6dc4ae41213e4da81e605a3d1a56ed851b9355182321c09190`.
```solidity
event TradingDisabled();
```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct TradingDisabled;
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
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);
            const SIGNATURE: &'static str = "TradingDisabled()";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 = alloy_sol_types::private::B256::new([
                188u8, 184u8, 184u8, 251u8, 222u8, 168u8, 170u8, 109u8, 196u8, 174u8,
                65u8, 33u8, 62u8, 77u8, 168u8, 30u8, 96u8, 90u8, 61u8, 26u8, 86u8, 237u8,
                133u8, 27u8, 147u8, 85u8, 24u8, 35u8, 33u8, 192u8, 145u8, 144u8,
            ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {}
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
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `TradingEnabled(bytes32,(uint256,address,bytes,bytes32))` and selector `0x510e4a4f76907c2d6158b343f7c4f2f597df385b727c26e9ef90e75093ace19a`.
```solidity
event TradingEnabled(bytes32 indexed hash, ConstantProduct.TradingParams params);
```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct TradingEnabled {
        #[allow(missing_docs)]
        pub hash: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub params: <ConstantProduct::TradingParams as alloy_sol_types::SolType>::RustType,
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
        impl alloy_sol_types::SolEvent for TradingEnabled {
            type DataTuple<'a> = (ConstantProduct::TradingParams,);
            type DataToken<'a> = <Self::DataTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::FixedBytes<32>,
            );
            const SIGNATURE: &'static str = "TradingEnabled(bytes32,(uint256,address,bytes,bytes32))";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 = alloy_sol_types::private::B256::new([
                81u8, 14u8, 74u8, 79u8, 118u8, 144u8, 124u8, 45u8, 97u8, 88u8, 179u8,
                67u8, 247u8, 196u8, 242u8, 245u8, 151u8, 223u8, 56u8, 91u8, 114u8, 124u8,
                38u8, 233u8, 239u8, 144u8, 231u8, 80u8, 147u8, 172u8, 225u8, 154u8,
            ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    hash: topics.1,
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
                    <ConstantProduct::TradingParams as alloy_sol_types::SolType>::tokenize(
                        &self.params,
                    ),
                )
            }
            #[inline]
            fn topics(&self) -> <Self::TopicList as alloy_sol_types::SolType>::RustType {
                (Self::SIGNATURE_HASH.into(), self.hash.clone())
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
                > as alloy_sol_types::EventTopic>::encode_topic(&self.hash);
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for TradingEnabled {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&TradingEnabled> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &TradingEnabled) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    /**Constructor`.
```solidity
constructor(address _solutionSettler, address _token0, address _token1);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct constructorCall {
        #[allow(missing_docs)]
        pub _solutionSettler: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub _token0: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub _token1: alloy_sol_types::private::Address,
    }
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
            impl ::core::convert::From<constructorCall> for UnderlyingRustTuple<'_> {
                fn from(value: constructorCall) -> Self {
                    (value._solutionSettler, value._token0, value._token1)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for constructorCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        _solutionSettler: tuple.0,
                        _token0: tuple.1,
                        _token1: tuple.2,
                    }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolConstructor for constructorCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
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
                        &self._solutionSettler,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self._token0,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self._token1,
                    ),
                )
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `commit(bytes32)` and selector `0xf14fcbc8`.
```solidity
function commit(bytes32 orderHash) external;
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct commitCall {
        #[allow(missing_docs)]
        pub orderHash: alloy_sol_types::private::FixedBytes<32>,
    }
    ///Container type for the return parameters of the [`commit(bytes32)`](commitCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct commitReturn {}
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
            impl ::core::convert::From<commitCall> for UnderlyingRustTuple<'_> {
                fn from(value: commitCall) -> Self {
                    (value.orderHash,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for commitCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { orderHash: tuple.0 }
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
            impl ::core::convert::From<commitReturn> for UnderlyingRustTuple<'_> {
                fn from(value: commitReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for commitReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl commitReturn {
            fn _tokenize(
                &self,
            ) -> <commitCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for commitCall {
            type Parameters<'a> = (alloy_sol_types::sol_data::FixedBytes<32>,);
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = commitReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "commit(bytes32)";
            const SELECTOR: [u8; 4] = [241u8, 79u8, 203u8, 200u8];
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
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                commitReturn::_tokenize(ret)
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
    /**Function with signature `hash((uint256,address,bytes,bytes32))` and selector `0xb09aaaca`.
```solidity
function hash(ConstantProduct.TradingParams memory tradingParams) external pure returns (bytes32);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct hashCall {
        #[allow(missing_docs)]
        pub tradingParams: <ConstantProduct::TradingParams as alloy_sol_types::SolType>::RustType,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`hash((uint256,address,bytes,bytes32))`](hashCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct hashReturn {
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
            type UnderlyingSolTuple<'a> = (ConstantProduct::TradingParams,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                <ConstantProduct::TradingParams as alloy_sol_types::SolType>::RustType,
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
            impl ::core::convert::From<hashCall> for UnderlyingRustTuple<'_> {
                fn from(value: hashCall) -> Self {
                    (value.tradingParams,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for hashCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { tradingParams: tuple.0 }
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
            impl ::core::convert::From<hashReturn> for UnderlyingRustTuple<'_> {
                fn from(value: hashReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for hashReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for hashCall {
            type Parameters<'a> = (ConstantProduct::TradingParams,);
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::FixedBytes<32>;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::FixedBytes<32>,);
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "hash((uint256,address,bytes,bytes32))";
            const SELECTOR: [u8; 4] = [176u8, 154u8, 170u8, 202u8];
            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }
            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <ConstantProduct::TradingParams as alloy_sol_types::SolType>::tokenize(
                        &self.tradingParams,
                    ),
                )
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
                        let r: hashReturn = r.into();
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
                        let r: hashReturn = r.into();
                        r._0
                    })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `isValidSignature(bytes32,bytes)` and selector `0x1626ba7e`.
```solidity
function isValidSignature(bytes32 _hash, bytes memory signature) external view returns (bytes4);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct isValidSignatureCall {
        #[allow(missing_docs)]
        pub _hash: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub signature: alloy_sol_types::private::Bytes,
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
        use alloy_sol_types as alloy_sol_types;
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
            impl ::core::convert::From<isValidSignatureCall>
            for UnderlyingRustTuple<'_> {
                fn from(value: isValidSignatureCall) -> Self {
                    (value._hash, value.signature)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for isValidSignatureCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        _hash: tuple.0,
                        signature: tuple.1,
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
            impl ::core::convert::From<isValidSignatureReturn>
            for UnderlyingRustTuple<'_> {
                fn from(value: isValidSignatureReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>>
            for isValidSignatureReturn {
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
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::FixedBytes<4>;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::FixedBytes<4>,);
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
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
                    > as alloy_sol_types::SolType>::tokenize(&self._hash),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.signature,
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
                <Self::ReturnTuple<
                    '_,
                > as alloy_sol_types::SolType>::abi_decode_sequence(data)
                    .map(|r| {
                        let r: isValidSignatureReturn = r.into();
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
                        let r: isValidSignatureReturn = r.into();
                        r._0
                    })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `manager()` and selector `0x481c6a75`.
```solidity
function manager() external view returns (address);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct managerCall;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`manager()`](managerCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct managerReturn {
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
            impl ::core::convert::From<managerCall> for UnderlyingRustTuple<'_> {
                fn from(value: managerCall) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for managerCall {
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
            impl ::core::convert::From<managerReturn> for UnderlyingRustTuple<'_> {
                fn from(value: managerReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for managerReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for managerCall {
            type Parameters<'a> = ();
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::Address;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "manager()";
            const SELECTOR: [u8; 4] = [72u8, 28u8, 106u8, 117u8];
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
                        let r: managerReturn = r.into();
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
                        let r: managerReturn = r.into();
                        r._0
                    })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `token0()` and selector `0x0dfe1681`.
```solidity
function token0() external view returns (address);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct token0Call;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`token0()`](token0Call) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct token0Return {
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
            impl ::core::convert::From<token0Call> for UnderlyingRustTuple<'_> {
                fn from(value: token0Call) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for token0Call {
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
            impl ::core::convert::From<token0Return> for UnderlyingRustTuple<'_> {
                fn from(value: token0Return) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for token0Return {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for token0Call {
            type Parameters<'a> = ();
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::Address;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "token0()";
            const SELECTOR: [u8; 4] = [13u8, 254u8, 22u8, 129u8];
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
                        let r: token0Return = r.into();
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
                        let r: token0Return = r.into();
                        r._0
                    })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `token1()` and selector `0xd21220a7`.
```solidity
function token1() external view returns (address);
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct token1Call;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`token1()`](token1Call) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct token1Return {
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
            impl ::core::convert::From<token1Call> for UnderlyingRustTuple<'_> {
                fn from(value: token1Call) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for token1Call {
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
            impl ::core::convert::From<token1Return> for UnderlyingRustTuple<'_> {
                fn from(value: token1Return) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for token1Return {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for token1Call {
            type Parameters<'a> = ();
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::Address;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "token1()";
            const SELECTOR: [u8; 4] = [210u8, 18u8, 32u8, 167u8];
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
                        let r: token1Return = r.into();
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
                        let r: token1Return = r.into();
                        r._0
                    })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `verify((uint256,address,bytes,bytes32),(address,address,address,uint256,uint256,uint32,bytes32,uint256,bytes32,bool,bytes32,bytes32))` and selector `0xa029a8d4`.
```solidity
function verify(ConstantProduct.TradingParams memory tradingParams, GPv2Order.Data memory order) external view;
```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct verifyCall {
        #[allow(missing_docs)]
        pub tradingParams: <ConstantProduct::TradingParams as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub order: <GPv2Order::Data as alloy_sol_types::SolType>::RustType,
    }
    ///Container type for the return parameters of the [`verify((uint256,address,bytes,bytes32),(address,address,address,uint256,uint256,uint32,bytes32,uint256,bytes32,bool,bytes32,bytes32))`](verifyCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct verifyReturn {}
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
                ConstantProduct::TradingParams,
                GPv2Order::Data,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                <ConstantProduct::TradingParams as alloy_sol_types::SolType>::RustType,
                <GPv2Order::Data as alloy_sol_types::SolType>::RustType,
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
            impl ::core::convert::From<verifyCall> for UnderlyingRustTuple<'_> {
                fn from(value: verifyCall) -> Self {
                    (value.tradingParams, value.order)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for verifyCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        tradingParams: tuple.0,
                        order: tuple.1,
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
            impl ::core::convert::From<verifyReturn> for UnderlyingRustTuple<'_> {
                fn from(value: verifyReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for verifyReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl verifyReturn {
            fn _tokenize(
                &self,
            ) -> <verifyCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for verifyCall {
            type Parameters<'a> = (ConstantProduct::TradingParams, GPv2Order::Data);
            type Token<'a> = <Self::Parameters<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            type Return = verifyReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<
                'a,
            > as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "verify((uint256,address,bytes,bytes32),(address,address,address,uint256,uint256,uint32,bytes32,uint256,bytes32,bool,bytes32,bytes32))";
            const SELECTOR: [u8; 4] = [160u8, 41u8, 168u8, 212u8];
            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }
            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <ConstantProduct::TradingParams as alloy_sol_types::SolType>::tokenize(
                        &self.tradingParams,
                    ),
                    <GPv2Order::Data as alloy_sol_types::SolType>::tokenize(&self.order),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                verifyReturn::_tokenize(ret)
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
    ///Container for all the [`CowAmm`](self) function calls.
    #[derive(Clone)]
    #[derive()]
    pub enum CowAmmCalls {
        #[allow(missing_docs)]
        commit(commitCall),
        #[allow(missing_docs)]
        hash(hashCall),
        #[allow(missing_docs)]
        isValidSignature(isValidSignatureCall),
        #[allow(missing_docs)]
        manager(managerCall),
        #[allow(missing_docs)]
        token0(token0Call),
        #[allow(missing_docs)]
        token1(token1Call),
        #[allow(missing_docs)]
        verify(verifyCall),
    }
    impl CowAmmCalls {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the variants.
        /// No guarantees are made about the order of the selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 4usize]] = &[
            [13u8, 254u8, 22u8, 129u8],
            [22u8, 38u8, 186u8, 126u8],
            [72u8, 28u8, 106u8, 117u8],
            [160u8, 41u8, 168u8, 212u8],
            [176u8, 154u8, 170u8, 202u8],
            [210u8, 18u8, 32u8, 167u8],
            [241u8, 79u8, 203u8, 200u8],
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(token0),
            ::core::stringify!(isValidSignature),
            ::core::stringify!(manager),
            ::core::stringify!(verify),
            ::core::stringify!(hash),
            ::core::stringify!(token1),
            ::core::stringify!(commit),
        ];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <token0Call as alloy_sol_types::SolCall>::SIGNATURE,
            <isValidSignatureCall as alloy_sol_types::SolCall>::SIGNATURE,
            <managerCall as alloy_sol_types::SolCall>::SIGNATURE,
            <verifyCall as alloy_sol_types::SolCall>::SIGNATURE,
            <hashCall as alloy_sol_types::SolCall>::SIGNATURE,
            <token1Call as alloy_sol_types::SolCall>::SIGNATURE,
            <commitCall as alloy_sol_types::SolCall>::SIGNATURE,
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
    impl alloy_sol_types::SolInterface for CowAmmCalls {
        const NAME: &'static str = "CowAmmCalls";
        const MIN_DATA_LENGTH: usize = 0usize;
        const COUNT: usize = 7usize;
        #[inline]
        fn selector(&self) -> [u8; 4] {
            match self {
                Self::commit(_) => <commitCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::hash(_) => <hashCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::isValidSignature(_) => {
                    <isValidSignatureCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::manager(_) => <managerCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::token0(_) => <token0Call as alloy_sol_types::SolCall>::SELECTOR,
                Self::token1(_) => <token1Call as alloy_sol_types::SolCall>::SELECTOR,
                Self::verify(_) => <verifyCall as alloy_sol_types::SolCall>::SELECTOR,
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
            static DECODE_SHIMS: &[fn(&[u8]) -> alloy_sol_types::Result<CowAmmCalls>] = &[
                {
                    fn token0(data: &[u8]) -> alloy_sol_types::Result<CowAmmCalls> {
                        <token0Call as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(CowAmmCalls::token0)
                    }
                    token0
                },
                {
                    fn isValidSignature(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmCalls> {
                        <isValidSignatureCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(CowAmmCalls::isValidSignature)
                    }
                    isValidSignature
                },
                {
                    fn manager(data: &[u8]) -> alloy_sol_types::Result<CowAmmCalls> {
                        <managerCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(CowAmmCalls::manager)
                    }
                    manager
                },
                {
                    fn verify(data: &[u8]) -> alloy_sol_types::Result<CowAmmCalls> {
                        <verifyCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(CowAmmCalls::verify)
                    }
                    verify
                },
                {
                    fn hash(data: &[u8]) -> alloy_sol_types::Result<CowAmmCalls> {
                        <hashCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(CowAmmCalls::hash)
                    }
                    hash
                },
                {
                    fn token1(data: &[u8]) -> alloy_sol_types::Result<CowAmmCalls> {
                        <token1Call as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(CowAmmCalls::token1)
                    }
                    token1
                },
                {
                    fn commit(data: &[u8]) -> alloy_sol_types::Result<CowAmmCalls> {
                        <commitCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(CowAmmCalls::commit)
                    }
                    commit
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
            ) -> alloy_sol_types::Result<CowAmmCalls>] = &[
                {
                    fn token0(data: &[u8]) -> alloy_sol_types::Result<CowAmmCalls> {
                        <token0Call as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CowAmmCalls::token0)
                    }
                    token0
                },
                {
                    fn isValidSignature(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmCalls> {
                        <isValidSignatureCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CowAmmCalls::isValidSignature)
                    }
                    isValidSignature
                },
                {
                    fn manager(data: &[u8]) -> alloy_sol_types::Result<CowAmmCalls> {
                        <managerCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CowAmmCalls::manager)
                    }
                    manager
                },
                {
                    fn verify(data: &[u8]) -> alloy_sol_types::Result<CowAmmCalls> {
                        <verifyCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CowAmmCalls::verify)
                    }
                    verify
                },
                {
                    fn hash(data: &[u8]) -> alloy_sol_types::Result<CowAmmCalls> {
                        <hashCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CowAmmCalls::hash)
                    }
                    hash
                },
                {
                    fn token1(data: &[u8]) -> alloy_sol_types::Result<CowAmmCalls> {
                        <token1Call as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CowAmmCalls::token1)
                    }
                    token1
                },
                {
                    fn commit(data: &[u8]) -> alloy_sol_types::Result<CowAmmCalls> {
                        <commitCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CowAmmCalls::commit)
                    }
                    commit
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
                Self::commit(inner) => {
                    <commitCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::hash(inner) => {
                    <hashCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::isValidSignature(inner) => {
                    <isValidSignatureCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::manager(inner) => {
                    <managerCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::token0(inner) => {
                    <token0Call as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::token1(inner) => {
                    <token1Call as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::verify(inner) => {
                    <verifyCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
            }
        }
        #[inline]
        fn abi_encode_raw(&self, out: &mut alloy_sol_types::private::Vec<u8>) {
            match self {
                Self::commit(inner) => {
                    <commitCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::hash(inner) => {
                    <hashCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::isValidSignature(inner) => {
                    <isValidSignatureCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::manager(inner) => {
                    <managerCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::token0(inner) => {
                    <token0Call as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::token1(inner) => {
                    <token1Call as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::verify(inner) => {
                    <verifyCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
            }
        }
    }
    ///Container for all the [`CowAmm`](self) custom errors.
    #[derive(Clone)]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub enum CowAmmErrors {
        #[allow(missing_docs)]
        CommitOutsideOfSettlement(CommitOutsideOfSettlement),
        #[allow(missing_docs)]
        OnlyManagerCanCall(OnlyManagerCanCall),
        #[allow(missing_docs)]
        OrderDoesNotMatchCommitmentHash(OrderDoesNotMatchCommitmentHash),
        #[allow(missing_docs)]
        OrderDoesNotMatchDefaultTradeableOrder(OrderDoesNotMatchDefaultTradeableOrder),
        #[allow(missing_docs)]
        OrderDoesNotMatchMessageHash(OrderDoesNotMatchMessageHash),
        #[allow(missing_docs)]
        OrderNotValid(OrderNotValid),
        #[allow(missing_docs)]
        PollTryAtBlock(PollTryAtBlock),
        #[allow(missing_docs)]
        TradingParamsDoNotMatchHash(TradingParamsDoNotMatchHash),
    }
    impl CowAmmErrors {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the variants.
        /// No guarantees are made about the order of the selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 4usize]] = &[
            [31u8, 232u8, 80u8, 110u8],
            [89u8, 63u8, 202u8, 205u8],
            [191u8, 132u8, 137u8, 119u8],
            [200u8, 252u8, 39u8, 37u8],
            [217u8, 255u8, 36u8, 199u8],
            [218u8, 251u8, 221u8, 31u8],
            [241u8, 166u8, 120u8, 144u8],
            [248u8, 125u8, 13u8, 22u8],
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(PollTryAtBlock),
            ::core::stringify!(OrderDoesNotMatchMessageHash),
            ::core::stringify!(CommitOutsideOfSettlement),
            ::core::stringify!(OrderNotValid),
            ::core::stringify!(OrderDoesNotMatchDefaultTradeableOrder),
            ::core::stringify!(OrderDoesNotMatchCommitmentHash),
            ::core::stringify!(TradingParamsDoNotMatchHash),
            ::core::stringify!(OnlyManagerCanCall),
        ];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <PollTryAtBlock as alloy_sol_types::SolError>::SIGNATURE,
            <OrderDoesNotMatchMessageHash as alloy_sol_types::SolError>::SIGNATURE,
            <CommitOutsideOfSettlement as alloy_sol_types::SolError>::SIGNATURE,
            <OrderNotValid as alloy_sol_types::SolError>::SIGNATURE,
            <OrderDoesNotMatchDefaultTradeableOrder as alloy_sol_types::SolError>::SIGNATURE,
            <OrderDoesNotMatchCommitmentHash as alloy_sol_types::SolError>::SIGNATURE,
            <TradingParamsDoNotMatchHash as alloy_sol_types::SolError>::SIGNATURE,
            <OnlyManagerCanCall as alloy_sol_types::SolError>::SIGNATURE,
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
    impl alloy_sol_types::SolInterface for CowAmmErrors {
        const NAME: &'static str = "CowAmmErrors";
        const MIN_DATA_LENGTH: usize = 0usize;
        const COUNT: usize = 8usize;
        #[inline]
        fn selector(&self) -> [u8; 4] {
            match self {
                Self::CommitOutsideOfSettlement(_) => {
                    <CommitOutsideOfSettlement as alloy_sol_types::SolError>::SELECTOR
                }
                Self::OnlyManagerCanCall(_) => {
                    <OnlyManagerCanCall as alloy_sol_types::SolError>::SELECTOR
                }
                Self::OrderDoesNotMatchCommitmentHash(_) => {
                    <OrderDoesNotMatchCommitmentHash as alloy_sol_types::SolError>::SELECTOR
                }
                Self::OrderDoesNotMatchDefaultTradeableOrder(_) => {
                    <OrderDoesNotMatchDefaultTradeableOrder as alloy_sol_types::SolError>::SELECTOR
                }
                Self::OrderDoesNotMatchMessageHash(_) => {
                    <OrderDoesNotMatchMessageHash as alloy_sol_types::SolError>::SELECTOR
                }
                Self::OrderNotValid(_) => {
                    <OrderNotValid as alloy_sol_types::SolError>::SELECTOR
                }
                Self::PollTryAtBlock(_) => {
                    <PollTryAtBlock as alloy_sol_types::SolError>::SELECTOR
                }
                Self::TradingParamsDoNotMatchHash(_) => {
                    <TradingParamsDoNotMatchHash as alloy_sol_types::SolError>::SELECTOR
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
            static DECODE_SHIMS: &[fn(&[u8]) -> alloy_sol_types::Result<CowAmmErrors>] = &[
                {
                    fn PollTryAtBlock(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmErrors> {
                        <PollTryAtBlock as alloy_sol_types::SolError>::abi_decode_raw(
                                data,
                            )
                            .map(CowAmmErrors::PollTryAtBlock)
                    }
                    PollTryAtBlock
                },
                {
                    fn OrderDoesNotMatchMessageHash(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmErrors> {
                        <OrderDoesNotMatchMessageHash as alloy_sol_types::SolError>::abi_decode_raw(
                                data,
                            )
                            .map(CowAmmErrors::OrderDoesNotMatchMessageHash)
                    }
                    OrderDoesNotMatchMessageHash
                },
                {
                    fn CommitOutsideOfSettlement(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmErrors> {
                        <CommitOutsideOfSettlement as alloy_sol_types::SolError>::abi_decode_raw(
                                data,
                            )
                            .map(CowAmmErrors::CommitOutsideOfSettlement)
                    }
                    CommitOutsideOfSettlement
                },
                {
                    fn OrderNotValid(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmErrors> {
                        <OrderNotValid as alloy_sol_types::SolError>::abi_decode_raw(
                                data,
                            )
                            .map(CowAmmErrors::OrderNotValid)
                    }
                    OrderNotValid
                },
                {
                    fn OrderDoesNotMatchDefaultTradeableOrder(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmErrors> {
                        <OrderDoesNotMatchDefaultTradeableOrder as alloy_sol_types::SolError>::abi_decode_raw(
                                data,
                            )
                            .map(CowAmmErrors::OrderDoesNotMatchDefaultTradeableOrder)
                    }
                    OrderDoesNotMatchDefaultTradeableOrder
                },
                {
                    fn OrderDoesNotMatchCommitmentHash(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmErrors> {
                        <OrderDoesNotMatchCommitmentHash as alloy_sol_types::SolError>::abi_decode_raw(
                                data,
                            )
                            .map(CowAmmErrors::OrderDoesNotMatchCommitmentHash)
                    }
                    OrderDoesNotMatchCommitmentHash
                },
                {
                    fn TradingParamsDoNotMatchHash(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmErrors> {
                        <TradingParamsDoNotMatchHash as alloy_sol_types::SolError>::abi_decode_raw(
                                data,
                            )
                            .map(CowAmmErrors::TradingParamsDoNotMatchHash)
                    }
                    TradingParamsDoNotMatchHash
                },
                {
                    fn OnlyManagerCanCall(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmErrors> {
                        <OnlyManagerCanCall as alloy_sol_types::SolError>::abi_decode_raw(
                                data,
                            )
                            .map(CowAmmErrors::OnlyManagerCanCall)
                    }
                    OnlyManagerCanCall
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
            ) -> alloy_sol_types::Result<CowAmmErrors>] = &[
                {
                    fn PollTryAtBlock(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmErrors> {
                        <PollTryAtBlock as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CowAmmErrors::PollTryAtBlock)
                    }
                    PollTryAtBlock
                },
                {
                    fn OrderDoesNotMatchMessageHash(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmErrors> {
                        <OrderDoesNotMatchMessageHash as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CowAmmErrors::OrderDoesNotMatchMessageHash)
                    }
                    OrderDoesNotMatchMessageHash
                },
                {
                    fn CommitOutsideOfSettlement(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmErrors> {
                        <CommitOutsideOfSettlement as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CowAmmErrors::CommitOutsideOfSettlement)
                    }
                    CommitOutsideOfSettlement
                },
                {
                    fn OrderNotValid(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmErrors> {
                        <OrderNotValid as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CowAmmErrors::OrderNotValid)
                    }
                    OrderNotValid
                },
                {
                    fn OrderDoesNotMatchDefaultTradeableOrder(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmErrors> {
                        <OrderDoesNotMatchDefaultTradeableOrder as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CowAmmErrors::OrderDoesNotMatchDefaultTradeableOrder)
                    }
                    OrderDoesNotMatchDefaultTradeableOrder
                },
                {
                    fn OrderDoesNotMatchCommitmentHash(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmErrors> {
                        <OrderDoesNotMatchCommitmentHash as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CowAmmErrors::OrderDoesNotMatchCommitmentHash)
                    }
                    OrderDoesNotMatchCommitmentHash
                },
                {
                    fn TradingParamsDoNotMatchHash(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmErrors> {
                        <TradingParamsDoNotMatchHash as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CowAmmErrors::TradingParamsDoNotMatchHash)
                    }
                    TradingParamsDoNotMatchHash
                },
                {
                    fn OnlyManagerCanCall(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<CowAmmErrors> {
                        <OnlyManagerCanCall as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(CowAmmErrors::OnlyManagerCanCall)
                    }
                    OnlyManagerCanCall
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
                Self::CommitOutsideOfSettlement(inner) => {
                    <CommitOutsideOfSettlement as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::OnlyManagerCanCall(inner) => {
                    <OnlyManagerCanCall as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::OrderDoesNotMatchCommitmentHash(inner) => {
                    <OrderDoesNotMatchCommitmentHash as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::OrderDoesNotMatchDefaultTradeableOrder(inner) => {
                    <OrderDoesNotMatchDefaultTradeableOrder as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::OrderDoesNotMatchMessageHash(inner) => {
                    <OrderDoesNotMatchMessageHash as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::OrderNotValid(inner) => {
                    <OrderNotValid as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
                Self::PollTryAtBlock(inner) => {
                    <PollTryAtBlock as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::TradingParamsDoNotMatchHash(inner) => {
                    <TradingParamsDoNotMatchHash as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
            }
        }
        #[inline]
        fn abi_encode_raw(&self, out: &mut alloy_sol_types::private::Vec<u8>) {
            match self {
                Self::CommitOutsideOfSettlement(inner) => {
                    <CommitOutsideOfSettlement as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::OnlyManagerCanCall(inner) => {
                    <OnlyManagerCanCall as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::OrderDoesNotMatchCommitmentHash(inner) => {
                    <OrderDoesNotMatchCommitmentHash as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::OrderDoesNotMatchDefaultTradeableOrder(inner) => {
                    <OrderDoesNotMatchDefaultTradeableOrder as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::OrderDoesNotMatchMessageHash(inner) => {
                    <OrderDoesNotMatchMessageHash as alloy_sol_types::SolError>::abi_encode_raw(
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
                Self::PollTryAtBlock(inner) => {
                    <PollTryAtBlock as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::TradingParamsDoNotMatchHash(inner) => {
                    <TradingParamsDoNotMatchHash as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
            }
        }
    }
    ///Container for all the [`CowAmm`](self) events.
    #[derive(Clone)]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub enum CowAmmEvents {
        #[allow(missing_docs)]
        TradingDisabled(TradingDisabled),
        #[allow(missing_docs)]
        TradingEnabled(TradingEnabled),
    }
    impl CowAmmEvents {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the variants.
        /// No guarantees are made about the order of the selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 32usize]] = &[
            [
                81u8, 14u8, 74u8, 79u8, 118u8, 144u8, 124u8, 45u8, 97u8, 88u8, 179u8,
                67u8, 247u8, 196u8, 242u8, 245u8, 151u8, 223u8, 56u8, 91u8, 114u8, 124u8,
                38u8, 233u8, 239u8, 144u8, 231u8, 80u8, 147u8, 172u8, 225u8, 154u8,
            ],
            [
                188u8, 184u8, 184u8, 251u8, 222u8, 168u8, 170u8, 109u8, 196u8, 174u8,
                65u8, 33u8, 62u8, 77u8, 168u8, 30u8, 96u8, 90u8, 61u8, 26u8, 86u8, 237u8,
                133u8, 27u8, 147u8, 85u8, 24u8, 35u8, 33u8, 192u8, 145u8, 144u8,
            ],
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(TradingEnabled),
            ::core::stringify!(TradingDisabled),
        ];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <TradingEnabled as alloy_sol_types::SolEvent>::SIGNATURE,
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
    impl alloy_sol_types::SolEventInterface for CowAmmEvents {
        const NAME: &'static str = "CowAmmEvents";
        const COUNT: usize = 2usize;
        fn decode_raw_log(
            topics: &[alloy_sol_types::Word],
            data: &[u8],
        ) -> alloy_sol_types::Result<Self> {
            match topics.first().copied() {
                Some(<TradingDisabled as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <TradingDisabled as alloy_sol_types::SolEvent>::decode_raw_log(
                            topics,
                            data,
                        )
                        .map(Self::TradingDisabled)
                }
                Some(<TradingEnabled as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <TradingEnabled as alloy_sol_types::SolEvent>::decode_raw_log(
                            topics,
                            data,
                        )
                        .map(Self::TradingEnabled)
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
    impl alloy_sol_types::private::IntoLogData for CowAmmEvents {
        fn to_log_data(&self) -> alloy_sol_types::private::LogData {
            match self {
                Self::TradingDisabled(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::TradingEnabled(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
            }
        }
        fn into_log_data(self) -> alloy_sol_types::private::LogData {
            match self {
                Self::TradingDisabled(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::TradingEnabled(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
            }
        }
    }
    use alloy_contract as alloy_contract;
    /**Creates a new wrapper around an on-chain [`CowAmm`](self) contract instance.

See the [wrapper's documentation](`CowAmmInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> CowAmmInstance<P, N> {
        CowAmmInstance::<P, N>::new(address, __provider)
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
        _solutionSettler: alloy_sol_types::private::Address,
        _token0: alloy_sol_types::private::Address,
        _token1: alloy_sol_types::private::Address,
    ) -> impl ::core::future::Future<
        Output = alloy_contract::Result<CowAmmInstance<P, N>>,
    > {
        CowAmmInstance::<P, N>::deploy(__provider, _solutionSettler, _token0, _token1)
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
        _solutionSettler: alloy_sol_types::private::Address,
        _token0: alloy_sol_types::private::Address,
        _token1: alloy_sol_types::private::Address,
    ) -> alloy_contract::RawCallBuilder<P, N> {
        CowAmmInstance::<
            P,
            N,
        >::deploy_builder(__provider, _solutionSettler, _token0, _token1)
    }
    /**A [`CowAmm`](self) instance.

Contains type-safe methods for interacting with an on-chain instance of the
[`CowAmm`](self) contract located at a given `address`, using a given
provider `P`.

If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
documentation on how to provide it), the `deploy` and `deploy_builder` methods can
be used to deploy a new instance of the contract.

See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct CowAmmInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for CowAmmInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("CowAmmInstance").field(&self.address).finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    > CowAmmInstance<P, N> {
        /**Creates a new wrapper around an on-chain [`CowAmm`](self) contract instance.

See the [wrapper's documentation](`CowAmmInstance`) for more details.*/
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
            _solutionSettler: alloy_sol_types::private::Address,
            _token0: alloy_sol_types::private::Address,
            _token1: alloy_sol_types::private::Address,
        ) -> alloy_contract::Result<CowAmmInstance<P, N>> {
            let call_builder = Self::deploy_builder(
                __provider,
                _solutionSettler,
                _token0,
                _token1,
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
            _solutionSettler: alloy_sol_types::private::Address,
            _token0: alloy_sol_types::private::Address,
            _token1: alloy_sol_types::private::Address,
        ) -> alloy_contract::RawCallBuilder<P, N> {
            alloy_contract::RawCallBuilder::new_raw_deploy(
                __provider,
                [
                    &BYTECODE[..],
                    &alloy_sol_types::SolConstructor::abi_encode(
                        &constructorCall {
                            _solutionSettler,
                            _token0,
                            _token1,
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
    impl<P: ::core::clone::Clone, N> CowAmmInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned provider.
        #[inline]
        pub fn with_cloned_provider(self) -> CowAmmInstance<P, N> {
            CowAmmInstance {
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
    > CowAmmInstance<P, N> {
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
        ///Creates a new call builder for the [`commit`] function.
        pub fn commit(
            &self,
            orderHash: alloy_sol_types::private::FixedBytes<32>,
        ) -> alloy_contract::SolCallBuilder<&P, commitCall, N> {
            self.call_builder(&commitCall { orderHash })
        }
        ///Creates a new call builder for the [`hash`] function.
        pub fn hash(
            &self,
            tradingParams: <ConstantProduct::TradingParams as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, hashCall, N> {
            self.call_builder(&hashCall { tradingParams })
        }
        ///Creates a new call builder for the [`isValidSignature`] function.
        pub fn isValidSignature(
            &self,
            _hash: alloy_sol_types::private::FixedBytes<32>,
            signature: alloy_sol_types::private::Bytes,
        ) -> alloy_contract::SolCallBuilder<&P, isValidSignatureCall, N> {
            self.call_builder(
                &isValidSignatureCall {
                    _hash,
                    signature,
                },
            )
        }
        ///Creates a new call builder for the [`manager`] function.
        pub fn manager(&self) -> alloy_contract::SolCallBuilder<&P, managerCall, N> {
            self.call_builder(&managerCall)
        }
        ///Creates a new call builder for the [`token0`] function.
        pub fn token0(&self) -> alloy_contract::SolCallBuilder<&P, token0Call, N> {
            self.call_builder(&token0Call)
        }
        ///Creates a new call builder for the [`token1`] function.
        pub fn token1(&self) -> alloy_contract::SolCallBuilder<&P, token1Call, N> {
            self.call_builder(&token1Call)
        }
        ///Creates a new call builder for the [`verify`] function.
        pub fn verify(
            &self,
            tradingParams: <ConstantProduct::TradingParams as alloy_sol_types::SolType>::RustType,
            order: <GPv2Order::Data as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, verifyCall, N> {
            self.call_builder(&verifyCall { tradingParams, order })
        }
    }
    /// Event filters.
    impl<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    > CowAmmInstance<P, N> {
        /// Creates a new event filter using this contract instance's provider and address.
        ///
        /// Note that the type can be any event, not just those defined in this contract.
        /// Prefer using the other methods for building type-safe event filters.
        pub fn event_filter<E: alloy_sol_types::SolEvent>(
            &self,
        ) -> alloy_contract::Event<&P, E, N> {
            alloy_contract::Event::new_sol(&self.provider, &self.address)
        }
        ///Creates a new event filter for the [`TradingDisabled`] event.
        pub fn TradingDisabled_filter(
            &self,
        ) -> alloy_contract::Event<&P, TradingDisabled, N> {
            self.event_filter::<TradingDisabled>()
        }
        ///Creates a new event filter for the [`TradingEnabled`] event.
        pub fn TradingEnabled_filter(
            &self,
        ) -> alloy_contract::Event<&P, TradingEnabled, N> {
            self.event_filter::<TradingEnabled>()
        }
    }
}
pub type Instance = CowAmm::CowAmmInstance<::alloy_provider::DynProvider>;
