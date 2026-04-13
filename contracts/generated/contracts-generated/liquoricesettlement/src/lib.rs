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
library GPv2Interaction {
    struct Data { address target; uint256 value; bytes callData; }
    struct Hooks { Data[] beforeSettle; Data[] afterSettle; }
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
    use {super::*, alloy_sol_types};
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
        use alloy_sol_types;
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
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.value,
                    ),
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
        impl alloy_sol_types::SolStruct for Data {
            const NAME: &'static str = "Data";

            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "Data(address target,uint256 value,bytes callData)",
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
                out.reserve(<Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust));
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
            fn encode_topic(rust: &Self::RustType) -> alloy_sol_types::abi::token::WordToken {
                let mut out = alloy_sol_types::private::Vec::new();
                <Self as alloy_sol_types::EventTopic>::encode_topic_preimage(rust, &mut out);
                alloy_sol_types::abi::token::WordToken(alloy_sol_types::private::keccak256(out))
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**```solidity
    struct Hooks { Data[] beforeSettle; Data[] afterSettle; }
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct Hooks {
        #[allow(missing_docs)]
        pub beforeSettle:
            alloy_sol_types::private::Vec<<Data as alloy_sol_types::SolType>::RustType>,
        #[allow(missing_docs)]
        pub afterSettle:
            alloy_sol_types::private::Vec<<Data as alloy_sol_types::SolType>::RustType>,
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
            alloy_sol_types::sol_data::Array<Data>,
            alloy_sol_types::sol_data::Array<Data>,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::Vec<<Data as alloy_sol_types::SolType>::RustType>,
            alloy_sol_types::private::Vec<<Data as alloy_sol_types::SolType>::RustType>,
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
        impl ::core::convert::From<Hooks> for UnderlyingRustTuple<'_> {
            fn from(value: Hooks) -> Self {
                (value.beforeSettle, value.afterSettle)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for Hooks {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    beforeSettle: tuple.0,
                    afterSettle: tuple.1,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for Hooks {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for Hooks {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Array<Data> as alloy_sol_types::SolType>::tokenize(
                        &self.beforeSettle,
                    ),
                    <alloy_sol_types::sol_data::Array<Data> as alloy_sol_types::SolType>::tokenize(
                        &self.afterSettle,
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
        impl alloy_sol_types::SolType for Hooks {
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
        impl alloy_sol_types::SolStruct for Hooks {
            const NAME: &'static str = "Hooks";

            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "Hooks(Data[] beforeSettle,Data[] afterSettle)",
                )
            }

            #[inline]
            fn eip712_components()
            -> alloy_sol_types::private::Vec<alloy_sol_types::private::Cow<'static, str>>
            {
                let mut components = alloy_sol_types::private::Vec::with_capacity(2);
                components.push(<Data as alloy_sol_types::SolStruct>::eip712_root_type());
                components.extend(<Data as alloy_sol_types::SolStruct>::eip712_components());
                components.push(<Data as alloy_sol_types::SolStruct>::eip712_root_type());
                components.extend(<Data as alloy_sol_types::SolStruct>::eip712_components());
                components
            }

            #[inline]
            fn eip712_encode_data(&self) -> alloy_sol_types::private::Vec<u8> {
                [
                    <alloy_sol_types::sol_data::Array<
                        Data,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.beforeSettle)
                        .0,
                    <alloy_sol_types::sol_data::Array<
                        Data,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.afterSettle)
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for Hooks {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Array<
                        Data,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.beforeSettle,
                    )
                    + <alloy_sol_types::sol_data::Array<
                        Data,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.afterSettle,
                    )
            }

            #[inline]
            fn encode_topic_preimage(
                rust: &Self::RustType,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                out.reserve(<Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust));
                <alloy_sol_types::sol_data::Array<
                    Data,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.beforeSettle,
                    out,
                );
                <alloy_sol_types::sol_data::Array<
                    Data,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.afterSettle,
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
            f.debug_tuple("GPv2InteractionInstance")
                .field(&self.address)
                .finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        GPv2InteractionInstance<P, N>
    {
        /**Creates a new wrapper around an on-chain [`GPv2Interaction`](self) contract instance.

        See the [wrapper's documentation](`GPv2InteractionInstance`) for more details.*/
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
    impl<P: ::core::clone::Clone, N> GPv2InteractionInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned
        /// provider.
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
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        GPv2InteractionInstance<P, N>
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
        GPv2InteractionInstance<P, N>
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
///Module containing a contract's types and functions.
/**

```solidity
library ILiquoriceSettlement {
    struct BaseTokenData { address addr; uint256 amount; uint256 toRecipient; uint256 toRepay; uint256 toSupply; }
    struct Order { address market; uint256 chainId; string rfqId; uint256 nonce; address trader; address effectiveTrader; uint256 quoteExpiry; address recipient; uint256 minFillAmount; BaseTokenData baseTokenData; QuoteTokenData quoteTokenData; }
    struct QuoteTokenData { address addr; uint256 amount; uint256 toTrader; uint256 toWithdraw; uint256 toBorrow; }
    struct Single { string rfqId; uint256 nonce; address trader; address effectiveTrader; address baseToken; address quoteToken; uint256 baseTokenAmount; uint256 quoteTokenAmount; uint256 minFillAmount; uint256 quoteExpiry; address recipient; }
}
```*/
#[allow(
    non_camel_case_types,
    non_snake_case,
    clippy::pub_underscore_fields,
    clippy::style,
    clippy::empty_structs_with_brackets
)]
pub mod ILiquoriceSettlement {
    use {super::*, alloy_sol_types};
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**```solidity
    struct BaseTokenData { address addr; uint256 amount; uint256 toRecipient; uint256 toRepay; uint256 toSupply; }
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct BaseTokenData {
        #[allow(missing_docs)]
        pub addr: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub amount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub toRecipient: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub toRepay: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub toSupply: alloy_sol_types::private::primitives::aliases::U256,
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
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<256>,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::Address,
            alloy_sol_types::private::primitives::aliases::U256,
            alloy_sol_types::private::primitives::aliases::U256,
            alloy_sol_types::private::primitives::aliases::U256,
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
        impl ::core::convert::From<BaseTokenData> for UnderlyingRustTuple<'_> {
            fn from(value: BaseTokenData) -> Self {
                (
                    value.addr,
                    value.amount,
                    value.toRecipient,
                    value.toRepay,
                    value.toSupply,
                )
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for BaseTokenData {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    addr: tuple.0,
                    amount: tuple.1,
                    toRecipient: tuple.2,
                    toRepay: tuple.3,
                    toSupply: tuple.4,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for BaseTokenData {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for BaseTokenData {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.addr,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.amount,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.toRecipient,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.toRepay,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.toSupply,
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
        impl alloy_sol_types::SolType for BaseTokenData {
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
        impl alloy_sol_types::SolStruct for BaseTokenData {
            const NAME: &'static str = "BaseTokenData";

            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "BaseTokenData(address addr,uint256 amount,uint256 toRecipient,uint256 \
                     toRepay,uint256 toSupply)",
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
                            &self.addr,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.amount)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.toRecipient)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.toRepay)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.toSupply)
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for BaseTokenData {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.addr,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.amount,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.toRecipient,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.toRepay,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.toSupply,
                    )
            }

            #[inline]
            fn encode_topic_preimage(
                rust: &Self::RustType,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                out.reserve(<Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust));
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.addr,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.amount,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.toRecipient,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.toRepay,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.toSupply,
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
    struct Order { address market; uint256 chainId; string rfqId; uint256 nonce; address trader; address effectiveTrader; uint256 quoteExpiry; address recipient; uint256 minFillAmount; BaseTokenData baseTokenData; QuoteTokenData quoteTokenData; }
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct Order {
        #[allow(missing_docs)]
        pub market: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub chainId: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub rfqId: alloy_sol_types::private::String,
        #[allow(missing_docs)]
        pub nonce: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub trader: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub effectiveTrader: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub quoteExpiry: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub recipient: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub minFillAmount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub baseTokenData: <BaseTokenData as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub quoteTokenData: <QuoteTokenData as alloy_sol_types::SolType>::RustType,
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
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::String,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Uint<256>,
            BaseTokenData,
            QuoteTokenData,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::Address,
            alloy_sol_types::private::primitives::aliases::U256,
            alloy_sol_types::private::String,
            alloy_sol_types::private::primitives::aliases::U256,
            alloy_sol_types::private::Address,
            alloy_sol_types::private::Address,
            alloy_sol_types::private::primitives::aliases::U256,
            alloy_sol_types::private::Address,
            alloy_sol_types::private::primitives::aliases::U256,
            <BaseTokenData as alloy_sol_types::SolType>::RustType,
            <QuoteTokenData as alloy_sol_types::SolType>::RustType,
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
        impl ::core::convert::From<Order> for UnderlyingRustTuple<'_> {
            fn from(value: Order) -> Self {
                (
                    value.market,
                    value.chainId,
                    value.rfqId,
                    value.nonce,
                    value.trader,
                    value.effectiveTrader,
                    value.quoteExpiry,
                    value.recipient,
                    value.minFillAmount,
                    value.baseTokenData,
                    value.quoteTokenData,
                )
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for Order {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    market: tuple.0,
                    chainId: tuple.1,
                    rfqId: tuple.2,
                    nonce: tuple.3,
                    trader: tuple.4,
                    effectiveTrader: tuple.5,
                    quoteExpiry: tuple.6,
                    recipient: tuple.7,
                    minFillAmount: tuple.8,
                    baseTokenData: tuple.9,
                    quoteTokenData: tuple.10,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for Order {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for Order {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.market,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.chainId,
                    ),
                    <alloy_sol_types::sol_data::String as alloy_sol_types::SolType>::tokenize(
                        &self.rfqId,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.nonce,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.trader,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.effectiveTrader,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.quoteExpiry,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.recipient,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.minFillAmount,
                    ),
                    <BaseTokenData as alloy_sol_types::SolType>::tokenize(&self.baseTokenData),
                    <QuoteTokenData as alloy_sol_types::SolType>::tokenize(&self.quoteTokenData),
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
        impl alloy_sol_types::SolType for Order {
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
        impl alloy_sol_types::SolStruct for Order {
            const NAME: &'static str = "Order";

            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "Order(address market,uint256 chainId,string rfqId,uint256 nonce,address \
                     trader,address effectiveTrader,uint256 quoteExpiry,address recipient,uint256 \
                     minFillAmount,BaseTokenData baseTokenData,QuoteTokenData quoteTokenData)",
                )
            }

            #[inline]
            fn eip712_components()
            -> alloy_sol_types::private::Vec<alloy_sol_types::private::Cow<'static, str>>
            {
                let mut components = alloy_sol_types::private::Vec::with_capacity(2);
                components.push(<BaseTokenData as alloy_sol_types::SolStruct>::eip712_root_type());
                components
                    .extend(<BaseTokenData as alloy_sol_types::SolStruct>::eip712_components());
                components.push(<QuoteTokenData as alloy_sol_types::SolStruct>::eip712_root_type());
                components
                    .extend(<QuoteTokenData as alloy_sol_types::SolStruct>::eip712_components());
                components
            }

            #[inline]
            fn eip712_encode_data(&self) -> alloy_sol_types::private::Vec<u8> {
                [
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.market,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.chainId)
                        .0,
                    <alloy_sol_types::sol_data::String as alloy_sol_types::SolType>::eip712_data_word(
                            &self.rfqId,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.nonce)
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.trader,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.effectiveTrader,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.quoteExpiry)
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.recipient,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.minFillAmount)
                        .0,
                    <BaseTokenData as alloy_sol_types::SolType>::eip712_data_word(
                            &self.baseTokenData,
                        )
                        .0,
                    <QuoteTokenData as alloy_sol_types::SolType>::eip712_data_word(
                            &self.quoteTokenData,
                        )
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for Order {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.market,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.chainId,
                    )
                    + <alloy_sol_types::sol_data::String as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.rfqId,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(&rust.nonce)
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.trader,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.effectiveTrader,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.quoteExpiry,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.recipient,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.minFillAmount,
                    )
                    + <BaseTokenData as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.baseTokenData,
                    )
                    + <QuoteTokenData as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.quoteTokenData,
                    )
            }

            #[inline]
            fn encode_topic_preimage(
                rust: &Self::RustType,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                out.reserve(<Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust));
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.market,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.chainId,
                    out,
                );
                <alloy_sol_types::sol_data::String as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.rfqId,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.nonce,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.trader,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.effectiveTrader,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.quoteExpiry,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.recipient,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.minFillAmount,
                    out,
                );
                <BaseTokenData as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.baseTokenData,
                    out,
                );
                <QuoteTokenData as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.quoteTokenData,
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
    struct QuoteTokenData { address addr; uint256 amount; uint256 toTrader; uint256 toWithdraw; uint256 toBorrow; }
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct QuoteTokenData {
        #[allow(missing_docs)]
        pub addr: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub amount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub toTrader: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub toWithdraw: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub toBorrow: alloy_sol_types::private::primitives::aliases::U256,
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
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<256>,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::Address,
            alloy_sol_types::private::primitives::aliases::U256,
            alloy_sol_types::private::primitives::aliases::U256,
            alloy_sol_types::private::primitives::aliases::U256,
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
        impl ::core::convert::From<QuoteTokenData> for UnderlyingRustTuple<'_> {
            fn from(value: QuoteTokenData) -> Self {
                (
                    value.addr,
                    value.amount,
                    value.toTrader,
                    value.toWithdraw,
                    value.toBorrow,
                )
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for QuoteTokenData {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    addr: tuple.0,
                    amount: tuple.1,
                    toTrader: tuple.2,
                    toWithdraw: tuple.3,
                    toBorrow: tuple.4,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for QuoteTokenData {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for QuoteTokenData {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.addr,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.amount,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.toTrader,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.toWithdraw,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.toBorrow,
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
        impl alloy_sol_types::SolType for QuoteTokenData {
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
        impl alloy_sol_types::SolStruct for QuoteTokenData {
            const NAME: &'static str = "QuoteTokenData";

            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "QuoteTokenData(address addr,uint256 amount,uint256 toTrader,uint256 \
                     toWithdraw,uint256 toBorrow)",
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
                            &self.addr,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.amount)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.toTrader)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.toWithdraw)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.toBorrow)
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for QuoteTokenData {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.addr,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.amount,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.toTrader,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.toWithdraw,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.toBorrow,
                    )
            }

            #[inline]
            fn encode_topic_preimage(
                rust: &Self::RustType,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                out.reserve(<Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust));
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.addr,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.amount,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.toTrader,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.toWithdraw,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.toBorrow,
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
    struct Single { string rfqId; uint256 nonce; address trader; address effectiveTrader; address baseToken; address quoteToken; uint256 baseTokenAmount; uint256 quoteTokenAmount; uint256 minFillAmount; uint256 quoteExpiry; address recipient; }
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct Single {
        #[allow(missing_docs)]
        pub rfqId: alloy_sol_types::private::String,
        #[allow(missing_docs)]
        pub nonce: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub trader: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub effectiveTrader: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub baseToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub quoteToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub baseTokenAmount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub quoteTokenAmount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub minFillAmount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub quoteExpiry: alloy_sol_types::private::primitives::aliases::U256,
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
        use alloy_sol_types;
        #[doc(hidden)]
        #[allow(dead_code)]
        type UnderlyingSolTuple<'a> = (
            alloy_sol_types::sol_data::String,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Address,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::String,
            alloy_sol_types::private::primitives::aliases::U256,
            alloy_sol_types::private::Address,
            alloy_sol_types::private::Address,
            alloy_sol_types::private::Address,
            alloy_sol_types::private::Address,
            alloy_sol_types::private::primitives::aliases::U256,
            alloy_sol_types::private::primitives::aliases::U256,
            alloy_sol_types::private::primitives::aliases::U256,
            alloy_sol_types::private::primitives::aliases::U256,
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
        impl ::core::convert::From<Single> for UnderlyingRustTuple<'_> {
            fn from(value: Single) -> Self {
                (
                    value.rfqId,
                    value.nonce,
                    value.trader,
                    value.effectiveTrader,
                    value.baseToken,
                    value.quoteToken,
                    value.baseTokenAmount,
                    value.quoteTokenAmount,
                    value.minFillAmount,
                    value.quoteExpiry,
                    value.recipient,
                )
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for Single {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    rfqId: tuple.0,
                    nonce: tuple.1,
                    trader: tuple.2,
                    effectiveTrader: tuple.3,
                    baseToken: tuple.4,
                    quoteToken: tuple.5,
                    baseTokenAmount: tuple.6,
                    quoteTokenAmount: tuple.7,
                    minFillAmount: tuple.8,
                    quoteExpiry: tuple.9,
                    recipient: tuple.10,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for Single {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for Single {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::String as alloy_sol_types::SolType>::tokenize(
                        &self.rfqId,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.nonce,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.trader,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.effectiveTrader,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.baseToken,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.quoteToken,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.baseTokenAmount,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.quoteTokenAmount,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.minFillAmount,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.quoteExpiry,
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
        impl alloy_sol_types::SolType for Single {
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
        impl alloy_sol_types::SolStruct for Single {
            const NAME: &'static str = "Single";

            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "Single(string rfqId,uint256 nonce,address trader,address \
                     effectiveTrader,address baseToken,address quoteToken,uint256 \
                     baseTokenAmount,uint256 quoteTokenAmount,uint256 minFillAmount,uint256 \
                     quoteExpiry,address recipient)",
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
                    <alloy_sol_types::sol_data::String as alloy_sol_types::SolType>::eip712_data_word(
                            &self.rfqId,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.nonce)
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.trader,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.effectiveTrader,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.baseToken,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.quoteToken,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(
                            &self.baseTokenAmount,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(
                            &self.quoteTokenAmount,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.minFillAmount)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.quoteExpiry)
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
        impl alloy_sol_types::EventTopic for Single {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::String as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.rfqId,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(&rust.nonce)
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.trader,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.effectiveTrader,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.baseToken,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.quoteToken,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.baseTokenAmount,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.quoteTokenAmount,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.minFillAmount,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.quoteExpiry,
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
                out.reserve(<Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust));
                <alloy_sol_types::sol_data::String as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.rfqId,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.nonce,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.trader,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.effectiveTrader,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.baseToken,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.quoteToken,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.baseTokenAmount,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.quoteTokenAmount,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.minFillAmount,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.quoteExpiry,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.recipient,
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
    /**Creates a new wrapper around an on-chain [`ILiquoriceSettlement`](self) contract instance.

    See the [wrapper's documentation](`ILiquoriceSettlementInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> ILiquoriceSettlementInstance<P, N> {
        ILiquoriceSettlementInstance::<P, N>::new(address, __provider)
    }
    /**A [`ILiquoriceSettlement`](self) instance.

    Contains type-safe methods for interacting with an on-chain instance of the
    [`ILiquoriceSettlement`](self) contract located at a given `address`, using a given
    provider `P`.

    If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
    documentation on how to provide it), the `deploy` and `deploy_builder` methods can
    be used to deploy a new instance of the contract.

    See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct ILiquoriceSettlementInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for ILiquoriceSettlementInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("ILiquoriceSettlementInstance")
                .field(&self.address)
                .finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        ILiquoriceSettlementInstance<P, N>
    {
        /**Creates a new wrapper around an on-chain [`ILiquoriceSettlement`](self) contract instance.

        See the [wrapper's documentation](`ILiquoriceSettlementInstance`) for more details.*/
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
    impl<P: ::core::clone::Clone, N> ILiquoriceSettlementInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned
        /// provider.
        #[inline]
        pub fn with_cloned_provider(self) -> ILiquoriceSettlementInstance<P, N> {
            ILiquoriceSettlementInstance {
                address: self.address,
                provider: ::core::clone::Clone::clone(&self.provider),
                _network: ::core::marker::PhantomData,
            }
        }
    }
    /// Function calls.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        ILiquoriceSettlementInstance<P, N>
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
        ILiquoriceSettlementInstance<P, N>
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
///Module containing a contract's types and functions.
/**

```solidity
library Signature {
    type TransferCommand is uint8;
    type Type is uint8;
    struct TakerPermitInfo { bytes signature; uint48 nonce; uint48 deadline; }
    struct TypedSignature { Type signatureType; TransferCommand transferCommand; bytes signatureBytes; }
}
```*/
#[allow(
    non_camel_case_types,
    non_snake_case,
    clippy::pub_underscore_fields,
    clippy::style,
    clippy::empty_structs_with_brackets
)]
pub mod Signature {
    use {super::*, alloy_sol_types};
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct TransferCommand(u8);
    const _: () = {
        use alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<TransferCommand> for u8 {
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
        impl TransferCommand {
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
        impl From<u8> for TransferCommand {
            fn from(value: u8) -> Self {
                Self::from_underlying(value)
            }
        }
        #[automatically_derived]
        impl From<TransferCommand> for u8 {
            fn from(value: TransferCommand) -> Self {
                value.into_underlying()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolType for TransferCommand {
            type RustType = u8;
            type Token<'a> =
                <alloy_sol_types::sol_data::Uint<8> as alloy_sol_types::SolType>::Token<'a>;

            const ENCODED_SIZE: Option<usize> =
                <alloy_sol_types::sol_data::Uint<8> as alloy_sol_types::SolType>::ENCODED_SIZE;
            const PACKED_ENCODED_SIZE: Option<usize> = <alloy_sol_types::sol_data::Uint<
                8,
            > as alloy_sol_types::SolType>::PACKED_ENCODED_SIZE;
            const SOL_NAME: &'static str = Self::NAME;

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
        impl alloy_sol_types::EventTopic for TransferCommand {
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
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct Type(u8);
    const _: () = {
        use alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Type> for u8 {
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
        impl Type {
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
        impl From<u8> for Type {
            fn from(value: u8) -> Self {
                Self::from_underlying(value)
            }
        }
        #[automatically_derived]
        impl From<Type> for u8 {
            fn from(value: Type) -> Self {
                value.into_underlying()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolType for Type {
            type RustType = u8;
            type Token<'a> =
                <alloy_sol_types::sol_data::Uint<8> as alloy_sol_types::SolType>::Token<'a>;

            const ENCODED_SIZE: Option<usize> =
                <alloy_sol_types::sol_data::Uint<8> as alloy_sol_types::SolType>::ENCODED_SIZE;
            const PACKED_ENCODED_SIZE: Option<usize> = <alloy_sol_types::sol_data::Uint<
                8,
            > as alloy_sol_types::SolType>::PACKED_ENCODED_SIZE;
            const SOL_NAME: &'static str = Self::NAME;

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
        impl alloy_sol_types::EventTopic for Type {
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
    struct TakerPermitInfo { bytes signature; uint48 nonce; uint48 deadline; }
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct TakerPermitInfo {
        #[allow(missing_docs)]
        pub signature: alloy_sol_types::private::Bytes,
        #[allow(missing_docs)]
        pub nonce: alloy_sol_types::private::primitives::aliases::U48,
        #[allow(missing_docs)]
        pub deadline: alloy_sol_types::private::primitives::aliases::U48,
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
            alloy_sol_types::sol_data::Bytes,
            alloy_sol_types::sol_data::Uint<48>,
            alloy_sol_types::sol_data::Uint<48>,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::Bytes,
            alloy_sol_types::private::primitives::aliases::U48,
            alloy_sol_types::private::primitives::aliases::U48,
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
        impl ::core::convert::From<TakerPermitInfo> for UnderlyingRustTuple<'_> {
            fn from(value: TakerPermitInfo) -> Self {
                (value.signature, value.nonce, value.deadline)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for TakerPermitInfo {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    signature: tuple.0,
                    nonce: tuple.1,
                    deadline: tuple.2,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for TakerPermitInfo {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for TakerPermitInfo {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.signature,
                    ),
                    <alloy_sol_types::sol_data::Uint<48> as alloy_sol_types::SolType>::tokenize(
                        &self.nonce,
                    ),
                    <alloy_sol_types::sol_data::Uint<48> as alloy_sol_types::SolType>::tokenize(
                        &self.deadline,
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
        impl alloy_sol_types::SolType for TakerPermitInfo {
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
        impl alloy_sol_types::SolStruct for TakerPermitInfo {
            const NAME: &'static str = "TakerPermitInfo";

            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "TakerPermitInfo(bytes signature,uint48 nonce,uint48 deadline)",
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
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::eip712_data_word(
                            &self.signature,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        48,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.nonce)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        48,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.deadline)
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for TakerPermitInfo {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Bytes as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.signature,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        48,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(&rust.nonce)
                    + <alloy_sol_types::sol_data::Uint<
                        48,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.deadline,
                    )
            }

            #[inline]
            fn encode_topic_preimage(
                rust: &Self::RustType,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                out.reserve(<Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust));
                <alloy_sol_types::sol_data::Bytes as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.signature,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    48,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.nonce,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    48,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.deadline,
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
    struct TypedSignature { Type signatureType; TransferCommand transferCommand; bytes signatureBytes; }
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct TypedSignature {
        #[allow(missing_docs)]
        pub signatureType: <Type as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub transferCommand: <TransferCommand as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub signatureBytes: alloy_sol_types::private::Bytes,
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
        type UnderlyingSolTuple<'a> = (Type, TransferCommand, alloy_sol_types::sol_data::Bytes);
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            <Type as alloy_sol_types::SolType>::RustType,
            <TransferCommand as alloy_sol_types::SolType>::RustType,
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
        impl ::core::convert::From<TypedSignature> for UnderlyingRustTuple<'_> {
            fn from(value: TypedSignature) -> Self {
                (
                    value.signatureType,
                    value.transferCommand,
                    value.signatureBytes,
                )
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for TypedSignature {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    signatureType: tuple.0,
                    transferCommand: tuple.1,
                    signatureBytes: tuple.2,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for TypedSignature {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for TypedSignature {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <Type as alloy_sol_types::SolType>::tokenize(&self.signatureType),
                    <TransferCommand as alloy_sol_types::SolType>::tokenize(&self.transferCommand),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.signatureBytes,
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
        impl alloy_sol_types::SolType for TypedSignature {
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
        impl alloy_sol_types::SolStruct for TypedSignature {
            const NAME: &'static str = "TypedSignature";

            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "TypedSignature(uint8 signatureType,uint8 transferCommand,bytes \
                     signatureBytes)",
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
                    <Type as alloy_sol_types::SolType>::eip712_data_word(
                            &self.signatureType,
                        )
                        .0,
                    <TransferCommand as alloy_sol_types::SolType>::eip712_data_word(
                            &self.transferCommand,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::eip712_data_word(
                            &self.signatureBytes,
                        )
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for TypedSignature {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <Type as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.signatureType,
                    )
                    + <TransferCommand as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.transferCommand,
                    )
                    + <alloy_sol_types::sol_data::Bytes as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.signatureBytes,
                    )
            }

            #[inline]
            fn encode_topic_preimage(
                rust: &Self::RustType,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                out.reserve(<Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust));
                <Type as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.signatureType,
                    out,
                );
                <TransferCommand as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.transferCommand,
                    out,
                );
                <alloy_sol_types::sol_data::Bytes as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.signatureBytes,
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
    /**Creates a new wrapper around an on-chain [`Signature`](self) contract instance.

    See the [wrapper's documentation](`SignatureInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> SignatureInstance<P, N> {
        SignatureInstance::<P, N>::new(address, __provider)
    }
    /**A [`Signature`](self) instance.

    Contains type-safe methods for interacting with an on-chain instance of the
    [`Signature`](self) contract located at a given `address`, using a given
    provider `P`.

    If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
    documentation on how to provide it), the `deploy` and `deploy_builder` methods can
    be used to deploy a new instance of the contract.

    See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct SignatureInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for SignatureInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("SignatureInstance")
                .field(&self.address)
                .finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        SignatureInstance<P, N>
    {
        /**Creates a new wrapper around an on-chain [`Signature`](self) contract instance.

        See the [wrapper's documentation](`SignatureInstance`) for more details.*/
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
    impl<P: ::core::clone::Clone, N> SignatureInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned
        /// provider.
        #[inline]
        pub fn with_cloned_provider(self) -> SignatureInstance<P, N> {
            SignatureInstance {
                address: self.address,
                provider: ::core::clone::Clone::clone(&self.provider),
                _network: ::core::marker::PhantomData,
            }
        }
    }
    /// Function calls.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        SignatureInstance<P, N>
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
        SignatureInstance<P, N>
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
library GPv2Interaction {
    struct Data {
        address target;
        uint256 value;
        bytes callData;
    }
    struct Hooks {
        Data[] beforeSettle;
        Data[] afterSettle;
    }
}

library ILiquoriceSettlement {
    struct BaseTokenData {
        address addr;
        uint256 amount;
        uint256 toRecipient;
        uint256 toRepay;
        uint256 toSupply;
    }
    struct Order {
        address market;
        uint256 chainId;
        string rfqId;
        uint256 nonce;
        address trader;
        address effectiveTrader;
        uint256 quoteExpiry;
        address recipient;
        uint256 minFillAmount;
        BaseTokenData baseTokenData;
        QuoteTokenData quoteTokenData;
    }
    struct QuoteTokenData {
        address addr;
        uint256 amount;
        uint256 toTrader;
        uint256 toWithdraw;
        uint256 toBorrow;
    }
    struct Single {
        string rfqId;
        uint256 nonce;
        address trader;
        address effectiveTrader;
        address baseToken;
        address quoteToken;
        uint256 baseTokenAmount;
        uint256 quoteTokenAmount;
        uint256 minFillAmount;
        uint256 quoteExpiry;
        address recipient;
    }
}

library Signature {
    type TransferCommand is uint8;
    type Type is uint8;
    struct TakerPermitInfo {
        bytes signature;
        uint48 nonce;
        uint48 deadline;
    }
    struct TypedSignature {
        Type signatureType;
        TransferCommand transferCommand;
        bytes signatureBytes;
    }
}

interface LiquoriceSettlement {
    error ECDSAInvalidSignature();
    error ECDSAInvalidSignatureLength(uint256 length);
    error ECDSAInvalidSignatureS(bytes32 s);
    error InvalidAmount();
    error InvalidAsset();
    error InvalidBaseTokenAmounts();
    error InvalidDestination();
    error InvalidEIP1271Signature();
    error InvalidEIP712Signature();
    error InvalidETHSignSignature();
    error InvalidFillAmount();
    error InvalidHooksTarget();
    error InvalidInteractionsBaseTokenAmounts();
    error InvalidInteractionsQuoteTokenAmounts();
    error InvalidLendingPoolInteraction();
    error InvalidQuoteTokenAmounts();
    error InvalidSignatureType();
    error InvalidSigner();
    error InvalidSource();
    error NonceInvalid();
    error NotMaker();
    error NotSolver();
    error OrderExpired();
    error PartialFillNotSupported();
    error ReceiverNotManager();
    error ReentrancyGuardReentrantCall();
    error SafeERC20FailedOperation(address token);
    error SignatureIsExpired();
    error SignatureIsNotEmpty();
    error UpdatedMakerAmountsTooLow();
    error ZeroMakerAmount();

    event Interaction(address indexed target, uint256 value, bytes4 selector);
    event TradeOrder(string indexed rfqId, address trader, address effectiveTrader, address baseToken, address quoteToken, uint256 baseTokenAmount, uint256 quoteTokenAmount, address recipient);

    constructor(address authenticator_, address repository_, address permit2_);

    receive() external payable;

    function AUTHENTICATOR() external view returns (address);
    function BALANCE_MANAGER() external view returns (address);
    function DOMAIN_SEPARATOR() external view returns (bytes32);
    function REPOSITORY() external view returns (address);
    function cancelLimitOrder(uint256 nonce) external;
    function hashBaseTokenData(ILiquoriceSettlement.BaseTokenData memory _baseTokenData) external pure returns (bytes32);
    function hashOrder(ILiquoriceSettlement.Order memory _order) external view returns (bytes32);
    function hashQuoteTokenData(ILiquoriceSettlement.QuoteTokenData memory _quoteTokenData) external pure returns (bytes32);
    function hashSingleOrder(ILiquoriceSettlement.Single memory _order) external view returns (bytes32);
    function isValidSignature(bytes32 _hash, bytes memory _signature) external view returns (bytes4);
    function settle(address _signer, uint256 _filledTakerAmount, ILiquoriceSettlement.Order memory _order, GPv2Interaction.Data[] memory _interactions, GPv2Interaction.Hooks memory _hooks, Signature.TypedSignature memory _makerSignature, Signature.TypedSignature memory _takerSignature) external;
    function settleSingle(address _signer, ILiquoriceSettlement.Single memory _order, Signature.TypedSignature memory _makerSignature, uint256 _filledTakerAmount, Signature.TypedSignature memory _takerSignature) external payable;
    function settleSingleWithPermitsSignatures(address _signer, ILiquoriceSettlement.Single memory _order, Signature.TypedSignature memory _makerSignature, uint256 _filledTakerAmount, Signature.TypedSignature memory _takerSignature, Signature.TakerPermitInfo memory _takerPermitInfo) external payable;
    function settleWithPermitsSignatures(address _signer, uint256 _filledTakerAmount, ILiquoriceSettlement.Order memory _order, GPv2Interaction.Data[] memory _interactions, GPv2Interaction.Hooks memory _hooks, Signature.TypedSignature memory _makerSignature, Signature.TypedSignature memory _takerSignature, Signature.TakerPermitInfo memory _takerPermitInfo) external payable;
    function validateHooks(address _repository, GPv2Interaction.Hooks memory _hooks) external view;
    function validateInteractions(address _repository, address _signer, bool _isPartialFill, ILiquoriceSettlement.Order memory _order, GPv2Interaction.Data[] memory _interactions) external view;
    function validateOrderAmounts(ILiquoriceSettlement.Order memory _order) external pure;
    function validateSignature(address _validationAddress, bytes32 _hash, Signature.TypedSignature memory _signature) external view;
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
        "internalType": "contract IAllowListAuthentication"
      },
      {
        "name": "repository_",
        "type": "address",
        "internalType": "contract IRepository"
      },
      {
        "name": "permit2_",
        "type": "address",
        "internalType": "address"
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
    "name": "AUTHENTICATOR",
    "inputs": [],
    "outputs": [
      {
        "name": "",
        "type": "address",
        "internalType": "contract IAllowListAuthentication"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "BALANCE_MANAGER",
    "inputs": [],
    "outputs": [
      {
        "name": "",
        "type": "address",
        "internalType": "contract IBalanceManager"
      }
    ],
    "stateMutability": "view"
  },
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
    "name": "REPOSITORY",
    "inputs": [],
    "outputs": [
      {
        "name": "",
        "type": "address",
        "internalType": "contract IRepository"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "cancelLimitOrder",
    "inputs": [
      {
        "name": "nonce",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "hashBaseTokenData",
    "inputs": [
      {
        "name": "_baseTokenData",
        "type": "tuple",
        "internalType": "struct ILiquoriceSettlement.BaseTokenData",
        "components": [
          {
            "name": "addr",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "amount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "toRecipient",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "toRepay",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "toSupply",
            "type": "uint256",
            "internalType": "uint256"
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
    "name": "hashOrder",
    "inputs": [
      {
        "name": "_order",
        "type": "tuple",
        "internalType": "struct ILiquoriceSettlement.Order",
        "components": [
          {
            "name": "market",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "chainId",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "rfqId",
            "type": "string",
            "internalType": "string"
          },
          {
            "name": "nonce",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "trader",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "effectiveTrader",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "quoteExpiry",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "recipient",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "minFillAmount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "baseTokenData",
            "type": "tuple",
            "internalType": "struct ILiquoriceSettlement.BaseTokenData",
            "components": [
              {
                "name": "addr",
                "type": "address",
                "internalType": "address"
              },
              {
                "name": "amount",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toRecipient",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toRepay",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toSupply",
                "type": "uint256",
                "internalType": "uint256"
              }
            ]
          },
          {
            "name": "quoteTokenData",
            "type": "tuple",
            "internalType": "struct ILiquoriceSettlement.QuoteTokenData",
            "components": [
              {
                "name": "addr",
                "type": "address",
                "internalType": "address"
              },
              {
                "name": "amount",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toTrader",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toWithdraw",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toBorrow",
                "type": "uint256",
                "internalType": "uint256"
              }
            ]
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
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "hashQuoteTokenData",
    "inputs": [
      {
        "name": "_quoteTokenData",
        "type": "tuple",
        "internalType": "struct ILiquoriceSettlement.QuoteTokenData",
        "components": [
          {
            "name": "addr",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "amount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "toTrader",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "toWithdraw",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "toBorrow",
            "type": "uint256",
            "internalType": "uint256"
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
    "name": "hashSingleOrder",
    "inputs": [
      {
        "name": "_order",
        "type": "tuple",
        "internalType": "struct ILiquoriceSettlement.Single",
        "components": [
          {
            "name": "rfqId",
            "type": "string",
            "internalType": "string"
          },
          {
            "name": "nonce",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "trader",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "effectiveTrader",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "baseToken",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "quoteToken",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "baseTokenAmount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "quoteTokenAmount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "minFillAmount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "quoteExpiry",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "recipient",
            "type": "address",
            "internalType": "address"
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
    "stateMutability": "view"
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
        "name": "_signature",
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
    "name": "settle",
    "inputs": [
      {
        "name": "_signer",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "_filledTakerAmount",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "_order",
        "type": "tuple",
        "internalType": "struct ILiquoriceSettlement.Order",
        "components": [
          {
            "name": "market",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "chainId",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "rfqId",
            "type": "string",
            "internalType": "string"
          },
          {
            "name": "nonce",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "trader",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "effectiveTrader",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "quoteExpiry",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "recipient",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "minFillAmount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "baseTokenData",
            "type": "tuple",
            "internalType": "struct ILiquoriceSettlement.BaseTokenData",
            "components": [
              {
                "name": "addr",
                "type": "address",
                "internalType": "address"
              },
              {
                "name": "amount",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toRecipient",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toRepay",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toSupply",
                "type": "uint256",
                "internalType": "uint256"
              }
            ]
          },
          {
            "name": "quoteTokenData",
            "type": "tuple",
            "internalType": "struct ILiquoriceSettlement.QuoteTokenData",
            "components": [
              {
                "name": "addr",
                "type": "address",
                "internalType": "address"
              },
              {
                "name": "amount",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toTrader",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toWithdraw",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toBorrow",
                "type": "uint256",
                "internalType": "uint256"
              }
            ]
          }
        ]
      },
      {
        "name": "_interactions",
        "type": "tuple[]",
        "internalType": "struct GPv2Interaction.Data[]",
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
      },
      {
        "name": "_hooks",
        "type": "tuple",
        "internalType": "struct GPv2Interaction.Hooks",
        "components": [
          {
            "name": "beforeSettle",
            "type": "tuple[]",
            "internalType": "struct GPv2Interaction.Data[]",
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
          },
          {
            "name": "afterSettle",
            "type": "tuple[]",
            "internalType": "struct GPv2Interaction.Data[]",
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
        ]
      },
      {
        "name": "_makerSignature",
        "type": "tuple",
        "internalType": "struct Signature.TypedSignature",
        "components": [
          {
            "name": "signatureType",
            "type": "uint8",
            "internalType": "enum Signature.Type"
          },
          {
            "name": "transferCommand",
            "type": "uint8",
            "internalType": "enum Signature.TransferCommand"
          },
          {
            "name": "signatureBytes",
            "type": "bytes",
            "internalType": "bytes"
          }
        ]
      },
      {
        "name": "_takerSignature",
        "type": "tuple",
        "internalType": "struct Signature.TypedSignature",
        "components": [
          {
            "name": "signatureType",
            "type": "uint8",
            "internalType": "enum Signature.Type"
          },
          {
            "name": "transferCommand",
            "type": "uint8",
            "internalType": "enum Signature.TransferCommand"
          },
          {
            "name": "signatureBytes",
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
    "name": "settleSingle",
    "inputs": [
      {
        "name": "_signer",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "_order",
        "type": "tuple",
        "internalType": "struct ILiquoriceSettlement.Single",
        "components": [
          {
            "name": "rfqId",
            "type": "string",
            "internalType": "string"
          },
          {
            "name": "nonce",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "trader",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "effectiveTrader",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "baseToken",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "quoteToken",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "baseTokenAmount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "quoteTokenAmount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "minFillAmount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "quoteExpiry",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "recipient",
            "type": "address",
            "internalType": "address"
          }
        ]
      },
      {
        "name": "_makerSignature",
        "type": "tuple",
        "internalType": "struct Signature.TypedSignature",
        "components": [
          {
            "name": "signatureType",
            "type": "uint8",
            "internalType": "enum Signature.Type"
          },
          {
            "name": "transferCommand",
            "type": "uint8",
            "internalType": "enum Signature.TransferCommand"
          },
          {
            "name": "signatureBytes",
            "type": "bytes",
            "internalType": "bytes"
          }
        ]
      },
      {
        "name": "_filledTakerAmount",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "_takerSignature",
        "type": "tuple",
        "internalType": "struct Signature.TypedSignature",
        "components": [
          {
            "name": "signatureType",
            "type": "uint8",
            "internalType": "enum Signature.Type"
          },
          {
            "name": "transferCommand",
            "type": "uint8",
            "internalType": "enum Signature.TransferCommand"
          },
          {
            "name": "signatureBytes",
            "type": "bytes",
            "internalType": "bytes"
          }
        ]
      }
    ],
    "outputs": [],
    "stateMutability": "payable"
  },
  {
    "type": "function",
    "name": "settleSingleWithPermitsSignatures",
    "inputs": [
      {
        "name": "_signer",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "_order",
        "type": "tuple",
        "internalType": "struct ILiquoriceSettlement.Single",
        "components": [
          {
            "name": "rfqId",
            "type": "string",
            "internalType": "string"
          },
          {
            "name": "nonce",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "trader",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "effectiveTrader",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "baseToken",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "quoteToken",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "baseTokenAmount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "quoteTokenAmount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "minFillAmount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "quoteExpiry",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "recipient",
            "type": "address",
            "internalType": "address"
          }
        ]
      },
      {
        "name": "_makerSignature",
        "type": "tuple",
        "internalType": "struct Signature.TypedSignature",
        "components": [
          {
            "name": "signatureType",
            "type": "uint8",
            "internalType": "enum Signature.Type"
          },
          {
            "name": "transferCommand",
            "type": "uint8",
            "internalType": "enum Signature.TransferCommand"
          },
          {
            "name": "signatureBytes",
            "type": "bytes",
            "internalType": "bytes"
          }
        ]
      },
      {
        "name": "_filledTakerAmount",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "_takerSignature",
        "type": "tuple",
        "internalType": "struct Signature.TypedSignature",
        "components": [
          {
            "name": "signatureType",
            "type": "uint8",
            "internalType": "enum Signature.Type"
          },
          {
            "name": "transferCommand",
            "type": "uint8",
            "internalType": "enum Signature.TransferCommand"
          },
          {
            "name": "signatureBytes",
            "type": "bytes",
            "internalType": "bytes"
          }
        ]
      },
      {
        "name": "_takerPermitInfo",
        "type": "tuple",
        "internalType": "struct Signature.TakerPermitInfo",
        "components": [
          {
            "name": "signature",
            "type": "bytes",
            "internalType": "bytes"
          },
          {
            "name": "nonce",
            "type": "uint48",
            "internalType": "uint48"
          },
          {
            "name": "deadline",
            "type": "uint48",
            "internalType": "uint48"
          }
        ]
      }
    ],
    "outputs": [],
    "stateMutability": "payable"
  },
  {
    "type": "function",
    "name": "settleWithPermitsSignatures",
    "inputs": [
      {
        "name": "_signer",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "_filledTakerAmount",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "_order",
        "type": "tuple",
        "internalType": "struct ILiquoriceSettlement.Order",
        "components": [
          {
            "name": "market",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "chainId",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "rfqId",
            "type": "string",
            "internalType": "string"
          },
          {
            "name": "nonce",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "trader",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "effectiveTrader",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "quoteExpiry",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "recipient",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "minFillAmount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "baseTokenData",
            "type": "tuple",
            "internalType": "struct ILiquoriceSettlement.BaseTokenData",
            "components": [
              {
                "name": "addr",
                "type": "address",
                "internalType": "address"
              },
              {
                "name": "amount",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toRecipient",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toRepay",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toSupply",
                "type": "uint256",
                "internalType": "uint256"
              }
            ]
          },
          {
            "name": "quoteTokenData",
            "type": "tuple",
            "internalType": "struct ILiquoriceSettlement.QuoteTokenData",
            "components": [
              {
                "name": "addr",
                "type": "address",
                "internalType": "address"
              },
              {
                "name": "amount",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toTrader",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toWithdraw",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toBorrow",
                "type": "uint256",
                "internalType": "uint256"
              }
            ]
          }
        ]
      },
      {
        "name": "_interactions",
        "type": "tuple[]",
        "internalType": "struct GPv2Interaction.Data[]",
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
      },
      {
        "name": "_hooks",
        "type": "tuple",
        "internalType": "struct GPv2Interaction.Hooks",
        "components": [
          {
            "name": "beforeSettle",
            "type": "tuple[]",
            "internalType": "struct GPv2Interaction.Data[]",
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
          },
          {
            "name": "afterSettle",
            "type": "tuple[]",
            "internalType": "struct GPv2Interaction.Data[]",
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
        ]
      },
      {
        "name": "_makerSignature",
        "type": "tuple",
        "internalType": "struct Signature.TypedSignature",
        "components": [
          {
            "name": "signatureType",
            "type": "uint8",
            "internalType": "enum Signature.Type"
          },
          {
            "name": "transferCommand",
            "type": "uint8",
            "internalType": "enum Signature.TransferCommand"
          },
          {
            "name": "signatureBytes",
            "type": "bytes",
            "internalType": "bytes"
          }
        ]
      },
      {
        "name": "_takerSignature",
        "type": "tuple",
        "internalType": "struct Signature.TypedSignature",
        "components": [
          {
            "name": "signatureType",
            "type": "uint8",
            "internalType": "enum Signature.Type"
          },
          {
            "name": "transferCommand",
            "type": "uint8",
            "internalType": "enum Signature.TransferCommand"
          },
          {
            "name": "signatureBytes",
            "type": "bytes",
            "internalType": "bytes"
          }
        ]
      },
      {
        "name": "_takerPermitInfo",
        "type": "tuple",
        "internalType": "struct Signature.TakerPermitInfo",
        "components": [
          {
            "name": "signature",
            "type": "bytes",
            "internalType": "bytes"
          },
          {
            "name": "nonce",
            "type": "uint48",
            "internalType": "uint48"
          },
          {
            "name": "deadline",
            "type": "uint48",
            "internalType": "uint48"
          }
        ]
      }
    ],
    "outputs": [],
    "stateMutability": "payable"
  },
  {
    "type": "function",
    "name": "validateHooks",
    "inputs": [
      {
        "name": "_repository",
        "type": "address",
        "internalType": "contract IRepository"
      },
      {
        "name": "_hooks",
        "type": "tuple",
        "internalType": "struct GPv2Interaction.Hooks",
        "components": [
          {
            "name": "beforeSettle",
            "type": "tuple[]",
            "internalType": "struct GPv2Interaction.Data[]",
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
          },
          {
            "name": "afterSettle",
            "type": "tuple[]",
            "internalType": "struct GPv2Interaction.Data[]",
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
        ]
      }
    ],
    "outputs": [],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "validateInteractions",
    "inputs": [
      {
        "name": "_repository",
        "type": "address",
        "internalType": "contract IRepository"
      },
      {
        "name": "_signer",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "_isPartialFill",
        "type": "bool",
        "internalType": "bool"
      },
      {
        "name": "_order",
        "type": "tuple",
        "internalType": "struct ILiquoriceSettlement.Order",
        "components": [
          {
            "name": "market",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "chainId",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "rfqId",
            "type": "string",
            "internalType": "string"
          },
          {
            "name": "nonce",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "trader",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "effectiveTrader",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "quoteExpiry",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "recipient",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "minFillAmount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "baseTokenData",
            "type": "tuple",
            "internalType": "struct ILiquoriceSettlement.BaseTokenData",
            "components": [
              {
                "name": "addr",
                "type": "address",
                "internalType": "address"
              },
              {
                "name": "amount",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toRecipient",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toRepay",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toSupply",
                "type": "uint256",
                "internalType": "uint256"
              }
            ]
          },
          {
            "name": "quoteTokenData",
            "type": "tuple",
            "internalType": "struct ILiquoriceSettlement.QuoteTokenData",
            "components": [
              {
                "name": "addr",
                "type": "address",
                "internalType": "address"
              },
              {
                "name": "amount",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toTrader",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toWithdraw",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toBorrow",
                "type": "uint256",
                "internalType": "uint256"
              }
            ]
          }
        ]
      },
      {
        "name": "_interactions",
        "type": "tuple[]",
        "internalType": "struct GPv2Interaction.Data[]",
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
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "validateOrderAmounts",
    "inputs": [
      {
        "name": "_order",
        "type": "tuple",
        "internalType": "struct ILiquoriceSettlement.Order",
        "components": [
          {
            "name": "market",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "chainId",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "rfqId",
            "type": "string",
            "internalType": "string"
          },
          {
            "name": "nonce",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "trader",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "effectiveTrader",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "quoteExpiry",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "recipient",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "minFillAmount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "baseTokenData",
            "type": "tuple",
            "internalType": "struct ILiquoriceSettlement.BaseTokenData",
            "components": [
              {
                "name": "addr",
                "type": "address",
                "internalType": "address"
              },
              {
                "name": "amount",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toRecipient",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toRepay",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toSupply",
                "type": "uint256",
                "internalType": "uint256"
              }
            ]
          },
          {
            "name": "quoteTokenData",
            "type": "tuple",
            "internalType": "struct ILiquoriceSettlement.QuoteTokenData",
            "components": [
              {
                "name": "addr",
                "type": "address",
                "internalType": "address"
              },
              {
                "name": "amount",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toTrader",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toWithdraw",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "toBorrow",
                "type": "uint256",
                "internalType": "uint256"
              }
            ]
          }
        ]
      }
    ],
    "outputs": [],
    "stateMutability": "pure"
  },
  {
    "type": "function",
    "name": "validateSignature",
    "inputs": [
      {
        "name": "_validationAddress",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "_hash",
        "type": "bytes32",
        "internalType": "bytes32"
      },
      {
        "name": "_signature",
        "type": "tuple",
        "internalType": "struct Signature.TypedSignature",
        "components": [
          {
            "name": "signatureType",
            "type": "uint8",
            "internalType": "enum Signature.Type"
          },
          {
            "name": "transferCommand",
            "type": "uint8",
            "internalType": "enum Signature.TransferCommand"
          },
          {
            "name": "signatureBytes",
            "type": "bytes",
            "internalType": "bytes"
          }
        ]
      }
    ],
    "outputs": [],
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
    "name": "TradeOrder",
    "inputs": [
      {
        "name": "rfqId",
        "type": "string",
        "indexed": true,
        "internalType": "string"
      },
      {
        "name": "trader",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "effectiveTrader",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "baseToken",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "quoteToken",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "baseTokenAmount",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "quoteTokenAmount",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "recipient",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      }
    ],
    "anonymous": false
  },
  {
    "type": "error",
    "name": "ECDSAInvalidSignature",
    "inputs": []
  },
  {
    "type": "error",
    "name": "ECDSAInvalidSignatureLength",
    "inputs": [
      {
        "name": "length",
        "type": "uint256",
        "internalType": "uint256"
      }
    ]
  },
  {
    "type": "error",
    "name": "ECDSAInvalidSignatureS",
    "inputs": [
      {
        "name": "s",
        "type": "bytes32",
        "internalType": "bytes32"
      }
    ]
  },
  {
    "type": "error",
    "name": "InvalidAmount",
    "inputs": []
  },
  {
    "type": "error",
    "name": "InvalidAsset",
    "inputs": []
  },
  {
    "type": "error",
    "name": "InvalidBaseTokenAmounts",
    "inputs": []
  },
  {
    "type": "error",
    "name": "InvalidDestination",
    "inputs": []
  },
  {
    "type": "error",
    "name": "InvalidEIP1271Signature",
    "inputs": []
  },
  {
    "type": "error",
    "name": "InvalidEIP712Signature",
    "inputs": []
  },
  {
    "type": "error",
    "name": "InvalidETHSignSignature",
    "inputs": []
  },
  {
    "type": "error",
    "name": "InvalidFillAmount",
    "inputs": []
  },
  {
    "type": "error",
    "name": "InvalidHooksTarget",
    "inputs": []
  },
  {
    "type": "error",
    "name": "InvalidInteractionsBaseTokenAmounts",
    "inputs": []
  },
  {
    "type": "error",
    "name": "InvalidInteractionsQuoteTokenAmounts",
    "inputs": []
  },
  {
    "type": "error",
    "name": "InvalidLendingPoolInteraction",
    "inputs": []
  },
  {
    "type": "error",
    "name": "InvalidQuoteTokenAmounts",
    "inputs": []
  },
  {
    "type": "error",
    "name": "InvalidSignatureType",
    "inputs": []
  },
  {
    "type": "error",
    "name": "InvalidSigner",
    "inputs": []
  },
  {
    "type": "error",
    "name": "InvalidSource",
    "inputs": []
  },
  {
    "type": "error",
    "name": "NonceInvalid",
    "inputs": []
  },
  {
    "type": "error",
    "name": "NotMaker",
    "inputs": []
  },
  {
    "type": "error",
    "name": "NotSolver",
    "inputs": []
  },
  {
    "type": "error",
    "name": "OrderExpired",
    "inputs": []
  },
  {
    "type": "error",
    "name": "PartialFillNotSupported",
    "inputs": []
  },
  {
    "type": "error",
    "name": "ReceiverNotManager",
    "inputs": []
  },
  {
    "type": "error",
    "name": "ReentrancyGuardReentrantCall",
    "inputs": []
  },
  {
    "type": "error",
    "name": "SafeERC20FailedOperation",
    "inputs": [
      {
        "name": "token",
        "type": "address",
        "internalType": "address"
      }
    ]
  },
  {
    "type": "error",
    "name": "SignatureIsExpired",
    "inputs": []
  },
  {
    "type": "error",
    "name": "SignatureIsNotEmpty",
    "inputs": []
  },
  {
    "type": "error",
    "name": "UpdatedMakerAmountsTooLow",
    "inputs": []
  },
  {
    "type": "error",
    "name": "ZeroMakerAmount",
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
pub mod LiquoriceSettlement {
    use {super::*, alloy_sol_types};
    /// The creation / init bytecode of the contract.
    ///
    /// ```text
    ///0x61012060405234801562000011575f80fd5b5060405162004ef638038062004ef6833981016040819052620000349162000175565b4660a0818152604080517f8b73c3c69bb8fe3d512ecc4cf759cc79239f7b179b0ffacaa9a75d522b39400f60208201527f64afec7be651c92f86754beb2bd5eeaf2fa95e83faf4aee989877dde08e4498c918101919091527fc89efdaa54c0f20c7adf612882df0950f5a951637e0307cdcb4c672f298b8bc660608201526080810192909252309082015260c00160408051601f19818403018152908290528051602090910120608052600180556001600160a01b03841660c05230908290620000fe906200014f565b6001600160a01b03928316815291166020820152604001604051809103905ff0801580156200012f573d5f803e3d5ffd5b506001600160a01b0390811660e052919091166101005250620001c69050565b610963806200459383390190565b6001600160a01b038116811462000172575f80fd5b50565b5f805f6060848603121562000188575f80fd5b835162000195816200015d565b6020850151909350620001a8816200015d565b6040850151909250620001bb816200015d565b809150509250925092565b60805160a05160c05160e0516101005161432d620002665f395f81816102570152818161078401526116dc01525f8181610197015281816107ba0152818161188c01528181611e2c01528181611f1e0152818161202c0152818161230b01528181612427015261303c01525f818161033801528181610412015281816106a70152818161152001526115ff01525f6104da01525f6105a4015261432d5ff3fe608060405260043610610126575f3560e01c8063a5cdc8fc116100a1578063c618618111610071578063db58772811610057578063db58772814610379578063e242924e1461038c578063fa5cd56c146103ab575f80fd5b8063c618618114610327578063cba673a71461035a575f80fd5b8063a5cdc8fc146102ab578063a7ab49bc146102ca578063ae80c584146102e9578063b11f126214610308575f80fd5b806351d46815116100f65780636f35d2d2116100dc5780636f35d2d214610246578063875530ff146102795780639935c86814610298575f80fd5b806351d46815146102125780635aa0e95d14610227575f80fd5b80631626ba7e1461013157806329bcdc95146101865780633644e515146101d15780634c9e03d3146101f3575f80fd5b3661012d57005b5f80fd5b34801561013c575f80fd5b5061015061014b366004613595565b6103ca565b6040517fffffffff0000000000000000000000000000000000000000000000000000000090911681526020015b60405180910390f35b348015610191575f80fd5b506101b97f000000000000000000000000000000000000000000000000000000000000000081565b6040516001600160a01b03909116815260200161017d565b3480156101dc575f80fd5b506101e56104d7565b60405190815260200161017d565b3480156101fe575f80fd5b506101e561020d366004613620565b6105c6565b6102256102203660046136d7565b610667565b005b348015610232575f80fd5b506102256102413660046137e1565b6109e8565b348015610251575f80fd5b506101b97f000000000000000000000000000000000000000000000000000000000000000081565b348015610284575f80fd5b506101e5610293366004613620565b610c30565b6102256102a636600461383f565b610c5f565b3480156102b6575f80fd5b506102256102c53660046138dd565b610c7f565b3480156102d5575f80fd5b506102256102e4366004613901565b610c8c565b3480156102f4575f80fd5b5061022561030336600461399d565b611067565b348015610313575f80fd5b506101e56103223660046139f2565b6112eb565b348015610332575f80fd5b506101b97f000000000000000000000000000000000000000000000000000000000000000081565b348015610365575f80fd5b50610225610374366004613a24565b6114ea565b610225610387366004613b0b565b611878565b348015610397575f80fd5b506101e56103a6366004613bc9565b61194c565b3480156103b6575f80fd5b506102256103c5366004613bc9565b611a8e565b5f806103d7858585611b50565b6040517fe75600c30000000000000000000000000000000000000000000000000000000081526001600160a01b0380831660048301529192507f00000000000000000000000000000000000000000000000000000000000000009091169063e75600c390602401602060405180830381865afa158015610459573d5f803e3d5ffd5b505050506040513d601f19601f8201168201806040525081019061047d9190613bfb565b156104ab577f1626ba7e356f5979dd355a3d2bfb43e80420a480c3b854edce286a82d74968699150506104d0565b507fffffffff0000000000000000000000000000000000000000000000000000000090505b9392505050565b5f7f000000000000000000000000000000000000000000000000000000000000000046146105a157604080517f8b73c3c69bb8fe3d512ecc4cf759cc79239f7b179b0ffacaa9a75d522b39400f60208201527f64afec7be651c92f86754beb2bd5eeaf2fa95e83faf4aee989877dde08e4498c918101919091527fc89efdaa54c0f20c7adf612882df0950f5a951637e0307cdcb4c672f298b8bc660608201524660808201523060a082015260c00160405160208183030381529060405280519060200120905090565b507f000000000000000000000000000000000000000000000000000000000000000090565b5f7f68b8e94dc077458241d6c8d89f0a7665c7cda2cfe70c9eb4437efee1663c66fe6105f56020840184613c16565b836020013584604001358560600135866080013560405160200161064a969594939291909586526001600160a01b0394909416602086015260408501929092526060840152608083015260a082015260c00190565b604051602081830303815290604052805190602001209050919050565b61066f611bdb565b6040517fe75600c30000000000000000000000000000000000000000000000000000000081526001600160a01b038a811660048301527f0000000000000000000000000000000000000000000000000000000000000000169063e75600c390602401602060405180830381865afa1580156106ec573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906107109190613bfb565b610746576040517fb331e42100000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b610200870135881580159061076057506101608801358911155b1561077f5761077c896102008a01356101608b01356001611c1e565b90505b6107b07f00000000000000000000000000000000000000000000000000000000000000008b838b8b8b8b8b8b611c69565b6001600160a01b037f00000000000000000000000000000000000000000000000000000000000000001663bc1178e66107ef60c08b0160a08c01613c16565b6108016101408c016101208d01613c16565b6040517fffffffff0000000000000000000000000000000000000000000000000000000060e085901b1681526108449291906101608e0135908890600401613c8d565b5f604051808303815f87803b15801561085b575f80fd5b505af115801561086d573d5f803e3d5ffd5b505050506108b4888b838c5f148061088957506101608c01358d115b610893578c61089a565b6101608c01355b898c8c60026108af60408e0160208f01613d61565b611e01565b6108c16040890189613d7f565b6040516108cf929190613de0565b6040519081900390207f0fce007c38c6c8ed9e545b3a148095762738618f8c21b673222613e4d45734b661090960a08b0160808c01613c16565b61091960c08c0160a08d01613c16565b61092b6101408d016101208e01613c16565b61093d6101e08e016101c08f01613c16565b8e158061094e57506101608e01358f115b610958578e61095f565b6101408e01355b6102008f013588146109715787610978565b6101e08f01355b8f60e001602081019061098b9190613c16565b604080516001600160a01b0398891681529688166020880152948716948601949094529185166060850152608084015260a083015290911660c082015260e00160405180910390a2506109dd60018055565b505050505050505050565b5f5b6109f48280613def565b9050811015610b065736610a088380613def565b83818110610a1857610a18613e53565b9050602002810190610a2a9190613e80565b90506001600160a01b03841663a8c4bc95610a486020840184613c16565b6040517fffffffff0000000000000000000000000000000000000000000000000000000060e084901b1681526001600160a01b039091166004820152602401602060405180830381865afa158015610aa2573d5f803e3d5ffd5b505050506040513d601f19601f82011682018060405250810190610ac69190613bfb565b15610afd576040517fc99e887200000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b506001016109ea565b505f5b610b166020830183613def565b9050811015610c2b5736610b2d6020840184613def565b83818110610b3d57610b3d613e53565b9050602002810190610b4f9190613e80565b90506001600160a01b03841663a8c4bc95610b6d6020840184613c16565b6040517fffffffff0000000000000000000000000000000000000000000000000000000060e084901b1681526001600160a01b039091166004820152602401602060405180830381865afa158015610bc7573d5f803e3d5ffd5b505050506040513d601f19601f82011682018060405250810190610beb9190613bfb565b15610c22576040517fc99e887200000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b50600101610b09565b505050565b5f7fae676bf6913ac2689b7331293c989fe7723124faf8b5d275f06fbcebc77950096105f56020840184613c16565b610c698482612212565b610c78858585856001806122c9565b5050505050565b610c89338261261e565b50565b610cbf6040518060c001604052805f81526020015f81526020015f81526020015f81526020015f81526020015f81525090565b5f5b828110156110535736848483818110610cdc57610cdc613e53565b9050602002810190610cee9190613e80565b9050365f610cff6040840184613d7f565b90925090506001600160a01b038b1663a8c4bc95610d206020860186613c16565b6040517fffffffff0000000000000000000000000000000000000000000000000000000060e084901b1681526001600160a01b039091166004820152602401602060405180830381865afa158015610d7a573d5f803e3d5ffd5b505050506040513d601f19601f82011682018060405250810190610d9e9190613bfb565b15611045575f8915610ddc576040517f7d617bb300000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b60048210610de8575081355b7fc03a9de9000000000000000000000000000000000000000000000000000000007fffffffff00000000000000000000000000000000000000000000000000000000821601610e5657610e3d83838d8c6126c4565b86602001818151610e4e9190613ebc565b905250611043565b7f243a4b7f000000000000000000000000000000000000000000000000000000007fffffffff00000000000000000000000000000000000000000000000000000000821601610ebc57610eab83838d8c6127da565b86606001818151610e4e9190613ebc565b7f7dc4f458000000000000000000000000000000000000000000000000000000007fffffffff00000000000000000000000000000000000000000000000000000000821601610f4757610f1183838d8c61294e565b60a088015260808701819052606087018051610f2e908390613ebc565b90525060a0860151602087018051610e4e908390613ebc565b7f68931b6b000000000000000000000000000000000000000000000000000000007fffffffff00000000000000000000000000000000000000000000000000000000821601610fab57610f9c83838d8c612bb6565b86518790610e4e908390613ebc565b7f0c9be7e4000000000000000000000000000000000000000000000000000000007fffffffff000000000000000000000000000000000000000000000000000000008216016110115761100083838d8c612cc0565b86604001818151610e4e9190613ebc565b6040517f0561d8b300000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b505b505050806001019050610cc1565b5061105e8482612e29565b50505050505050565b60036110766020830183613f21565b600381111561108757611087613ef4565b036110ec576001600160a01b0383166110ac836110a76040850185613d7f565b611b50565b6001600160a01b031614610c2b576040517fb81d58e700000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b60016110fb6020830183613f21565b600381111561110c5761110c613ef4565b036111a0577f19457468657265756d205369676e6564204d6573736167653a0a3332000000005f908152601c839052603c90206001600160a01b03841661115a826110a76040860186613d7f565b6001600160a01b03161461119a576040517f644ae6c300000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b50505050565b60026111af6020830183613f21565b60038111156111c0576111c0613ef4565b036112b9577f1626ba7e000000000000000000000000000000000000000000000000000000006001600160a01b038416631626ba7e846112036040860186613d7f565b6040518463ffffffff1660e01b815260040161122193929190613f3f565b602060405180830381865afa15801561123c573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906112609190613f58565b7fffffffff000000000000000000000000000000000000000000000000000000001614610c2b576040517f5d52cbe300000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6040517f60cd402d00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b5f6112f46104d7565b7fd28e809b708f5ee38be8347d6d869d8232493c094ab2dde98369e4102369a99d61131f8480613d7f565b604051602001611330929190613f97565b60405160208183030381529060405280519060200120846020013585604001602081019061135e9190613c16565b60408051602081019590955284019290925260608301526001600160a01b0316608082015260a001604080517fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe08184030181529190526113c46080850160608601613c16565b6113d460a0860160808701613c16565b6113e460c0870160a08801613c16565b60c087013560e08801356101008901356101208a013561140c6101608c016101408d01613c16565b604080516001600160a01b03998a166020820152978916908801529487166060870152608086019390935260a085019190915260c084015260e083015290911661010082015261012001604080517fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0818403018152908290526114929291602001613fd7565b6040516020818303038152906040528051906020012060405160200161064a9291907f190100000000000000000000000000000000000000000000000000000000000081526002810192909252602282015260420190565b6114f2611bdb565b6040517f02cc250d0000000000000000000000000000000000000000000000000000000081523360048201527f00000000000000000000000000000000000000000000000000000000000000006001600160a01b0316906302cc250d90602401602060405180830381865afa15801561156d573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906115919190613bfb565b6115c7576040517fc139eabd00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6040517fe75600c30000000000000000000000000000000000000000000000000000000081526001600160a01b0389811660048301527f0000000000000000000000000000000000000000000000000000000000000000169063e75600c390602401602060405180830381865afa158015611644573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906116689190613bfb565b61169e576040517fb331e42100000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b61020086013587158015906116b857506101608701358811155b156116d7576116d4886102008901356101608a01356001611c1e565b90505b6117087f00000000000000000000000000000000000000000000000000000000000000008a838a8a8a8a8a8a611c69565b611745878a838b158061171f57506101608b01358c115b611729578b611730565b6101608b01355b888b8b60016108af60408d0160208e01613d61565b6117526040880188613d7f565b604051611760929190613de0565b6040519081900390207f0fce007c38c6c8ed9e545b3a148095762738618f8c21b673222613e4d45734b661179a60a08a0160808b01613c16565b6117aa60c08b0160a08c01613c16565b6117bc6101408c016101208d01613c16565b6117ce6101e08d016101c08e01613c16565b8d15806117df57506101608d01358e115b6117e9578d6117f0565b6101408d01355b6102008e013588146118025787611809565b6101e08e01355b8e60e001602081019061181c9190613c16565b604080516001600160a01b0398891681529688166020880152948716948601949094529185166060850152608084015260a083015290911660c082015260e00160405180910390a25061186e60018055565b5050505050505050565b6118828583612212565b6001600160a01b037f00000000000000000000000000000000000000000000000000000000000000001663bc1178e66118c16080880160608901613c16565b6118d160a0890160808a01613c16565b8860c00135856040518563ffffffff1660e01b81526004016118f69493929190613c8d565b5f604051808303815f87803b15801561190d575f80fd5b505af115801561191f573d5f803e3d5ffd5b5050505061194486868686600289602001602081019061193f9190613d61565b6122c9565b505050505050565b5f6119556104d7565b7fc994d2ca0375d6d473785e0ce0b1d203f069121bac1314f72c5c0fe601eb39106119836040850185613d7f565b604051602001611994929190613f97565b604080517fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0818403018152919052805160209091012060608501356119df60a0870160808801613c16565b6119ef60c0880160a08901613c16565b60c0880135611a056101008a0160e08b01613c16565b60408051602081019890985287019590955260608601939093526001600160a01b039182166080860152811660a085015260c08401919091521660e0820152610100808501359082015261012001604051602081830303815290604052611a6f8461012001610c30565b611a7c856101c0016105c6565b60405160200161149293929190613feb565b6101a0810135611aa8610180830135610160840135613ebc565b611ab29190613ebc565b61014082013514611aef576040517fc04377d300000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b610240810135611b09610220830135610200840135613ebc565b611b139190613ebc565b6101e082013514610c89576040517f877630be00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b5f80611b918585858080601f0160208091040260200160405190810160405280939291908181526020018383808284375f92019190915250612ed692505050565b90506001600160a01b038116611bd3576040517f815e1d6400000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b949350505050565b600260015403611c17576040517f3ee5aeb500000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6002600155565b5f611c4b611c2b83612f00565b8015611c4657505f8480611c4157611c41614008565b868809115b151590565b611c56868686612f2c565b611c609190613ebc565b95945050505050565b5f611c738761194c565b9050611c80898285611067565b611c9060c0880160a08901613c16565b6001600160a01b0316336001600160a01b031614611cc757611cc2611cbb60c0890160a08a01613c16565b8284611067565b611d0e565b611cd46040830183613d7f565b90505f03611d0e576040517f0e364efc00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b611d2b611d2160c0890160a08a01613c16565b886060013561261e565b8660c00135421115611d69576040517f133df02900000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b5f88118015611d7c575086610100013588105b15611db3576040517f9469744400000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b611dbc87611a8e565b611dc68a856109e8565b611de78a8a5f8b118015611ddf57506102008a01358b14155b8a8a8a610c8c565b611df589886060013561261e565b50505050505050505050565b611e13611e0e8680613def565b613001565b5f611e286101808b01356101a08c0135613ebc565b90507f00000000000000000000000000000000000000000000000000000000000000006001600160a01b031663b519d3696040518060a001604052808d60a0016020810190611e779190613c16565b6001600160a01b03168152602001306001600160a01b031681526020018d610120015f016020810190611eaa9190613c16565b6001600160a01b03168152602001848152602001866002811115611ed057611ed0613ef4565b8152506040518263ffffffff1660e01b8152600401611eef9190614035565b5f604051808303815f87803b158015611f06575f80fd5b505af1158015611f18573d5f803e3d5ffd5b505050507f00000000000000000000000000000000000000000000000000000000000000006001600160a01b031663b519d3696040518060a001604052808d60a0016020810190611f699190613c16565b6001600160a01b031681526020018d60e0016020810190611f8a9190613c16565b6001600160a01b031681526020018d610120015f016020810190611fae9190613c16565b6001600160a01b031681526020018a8152602001866002811115611fd457611fd4613ef4565b8152506040518263ffffffff1660e01b8152600401611ff39190614035565b5f604051808303815f87803b15801561200a575f80fd5b505af115801561201c573d5f803e3d5ffd5b5050505061202a8585613001565b7f00000000000000000000000000000000000000000000000000000000000000006001600160a01b031663b519d3696040518060a001604052808c6001600160a01b031681526020018d60800160208101906120869190613c16565b6001600160a01b031681526020018d6101c0015f0160208101906120aa9190613c16565b6001600160a01b031681526020018b81526020018560028111156120d0576120d0613ef4565b8152506040518263ffffffff1660e01b81526004016120ef9190614035565b5f604051808303815f87803b158015612106575f80fd5b505af1158015612118573d5f803e3d5ffd5b5061212e9250611e0e9150506020880188613def565b5f6121416101408c016101208d01613c16565b6040517f70a082310000000000000000000000000000000000000000000000000000000081523060048201526001600160a01b0391909116906370a0823190602401602060405180830381865afa15801561219e573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906121c291906140b4565b90508015612205576122056121de6101008d0160e08e01613c16565b828d610120015f0160208101906121f59190613c16565b6001600160a01b03169190613139565b5050505050505050505050565b6122226080830160608401613c16565b6001600160a01b0316336001600160a01b0316146122615761225c61224d6080840160608501613c16565b612256846112eb565b83611067565b6122a8565b61226e6040820182613d7f565b90505f036122a8576040517f0e364efc00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6122c56122bb6080840160608501613c16565b836020013561261e565b5050565b60e085013583158015906122e057508560c0013584105b156122fd576122fa848760e001358860c001356001611c1e565b90505b612309878288886131b9565b7f00000000000000000000000000000000000000000000000000000000000000006001600160a01b031663b519d3696040518060a001604052808960600160208101906123569190613c16565b6001600160a01b031681526020016123766101608b016101408c01613c16565b6001600160a01b0316815260200161239460a08b0160808c01613c16565b6001600160a01b031681526020018715806123b257508960c0013588115b6123bc57876123c2565b8960c001355b81526020018660028111156123d9576123d9613ef4565b8152506040518263ffffffff1660e01b81526004016123f89190614035565b5f604051808303815f87803b15801561240f575f80fd5b505af1158015612421573d5f803e3d5ffd5b505050507f00000000000000000000000000000000000000000000000000000000000000006001600160a01b031663b519d3696040518060a001604052808a6001600160a01b031681526020018960400160208101906124819190613c16565b6001600160a01b0316815260200161249f60c08b0160a08c01613c16565b6001600160a01b031681526020018481526020018560028111156124c5576124c5613ef4565b8152506040518263ffffffff1660e01b81526004016124e49190614035565b5f604051808303815f87803b1580156124fb575f80fd5b505af115801561250d573d5f803e3d5ffd5b5061251e9250889150819050613d7f565b60405161252c929190613de0565b60405180910390207f0fce007c38c6c8ed9e545b3a148095762738618f8c21b673222613e4d45734b68760400160208101906125689190613c16565b61257860808a0160608b01613c16565b61258860a08b0160808c01613c16565b61259860c08c0160a08d01613c16565b8915806125a857508b60c001358a115b6125b257896125b8565b8b60c001355b878d6101400160208101906125cd9190613c16565b604080516001600160a01b0398891681529688166020880152948716948601949094529185166060850152608084015260a083015290911660c082015260e00160405180910390a250505050505050565b6001600160a01b0382165f9081526020818152604080832084845290915290205460ff1615612679576040517fbc0da7d600000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6001600160a01b039091165f908152602081815260408083209383529290522080547fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff00166001179055565b5f8080806126d5876004818b6140cb565b8101906126e291906140f2565b50919450925090506126fc61014086016101208701613c16565b6001600160a01b0316836001600160a01b031614612746576040517fc891add200000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b816001600160a01b0316866001600160a01b031614612791576040517f815e1d6400000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6101a085013581146127cf576040517f2c5211c600000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b979650505050505050565b5f808080806127ec886004818c6140cb565b8101906127f99190614142565b929650909450925090506128156101e087016101c08801613c16565b6001600160a01b0316846001600160a01b03161461285f576040517fc891add200000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b826001600160a01b0316876001600160a01b0316146128aa576040517f815e1d6400000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6128ba60a0870160808801613c16565b6001600160a01b0316826001600160a01b031614612904576040517fac6b05f500000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6102408601358114612942576040517f2c5211c600000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b98975050505050505050565b5f805f805f805f805f8c8c600490809261296a939291906140cb565b8101906129779190614190565b959c50939a50919850965094509250905061299a6101e08b016101c08c01613c16565b6001600160a01b0316876001600160a01b0316146129e4576040517fc891add200000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b856001600160a01b03168b6001600160a01b031614612a2f576040517f815e1d6400000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6001600160a01b0385163014612a71576040517f8154374b00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b612a8160a08b0160808c01613c16565b6001600160a01b0316846001600160a01b031614612acb576040517fac6b05f500000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6102408a01358314612b09576040517f2c5211c600000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b612b1b6101408b016101208c01613c16565b6001600160a01b0316826001600160a01b031614612b65576040517fc891add200000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6101a08a01358114612ba3576040517f2c5211c600000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b919c919b50909950505050505050505050565b5f808080612bc7876004818b6140cb565b810190612bd4919061420f565b91945092509050612bed61014086016101208701613c16565b6001600160a01b0316836001600160a01b031614612c37576040517fc891add200000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b816001600160a01b0316866001600160a01b031614612c82576040517f815e1d6400000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b61018085013581146127cf576040517f2c5211c600000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b5f80808080612cd2886004818c6140cb565b810190612cdf919061424d565b5092965090945092509050612cfc6101e087016101c08801613c16565b6001600160a01b0316846001600160a01b031614612d46576040517fc891add200000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b826001600160a01b0316876001600160a01b031614612d91576040517f815e1d6400000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b612da160a0870160808801613c16565b6001600160a01b0316826001600160a01b031614612deb576040517fac6b05f500000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6102208601358114612942576040517f2c5211c600000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b8051610180830135141580612e47575060208101516101a083013514155b15612e7e576040517f4a55da2000000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6040810151610220830135141580612e9f5750606081015161024083013514155b156122c5576040517f77a5920300000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b5f805f80612ee4868661325c565b925092509250612ef482826132a5565b50909150505b92915050565b5f6002826003811115612f1557612f15613ef4565b612f1f91906142b1565b60ff166001149050919050565b5f838302817fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff85870982811083820303915050805f03612f7f57838281612f7557612f75614008565b04925050506104d0565b808411612f9657612f9660038515026011186133ad565b5f848688095f868103871696879004966002600389028118808a02820302808a02820302808a02820302808a02820302808a02820302808a02909103029181900381900460010186841190950394909402919094039290920491909117919091029150509392505050565b5f5b81811015610c2b573683838381811061301e5761301e613e53565b90506020028101906130309190613e80565b90506001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000166130696020830183613c16565b6001600160a01b0316036130a9576040517f79a1bff000000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6130b2816133be565b6130bf6020820182613c16565b6001600160a01b03167fed99827efb37016f2275f98c4bcf71c7551c75d59e9b450f79fa32e60be672c282602001356130f784613401565b604080519283527fffffffff0000000000000000000000000000000000000000000000000000000090911660208301520160405180910390a250600101613003565b604080516001600160a01b038416602482015260448082018490528251808303909101815260649091019091526020810180517bffffffffffffffffffffffffffffffffffffffffffffffffffffffff167fa9059cbb00000000000000000000000000000000000000000000000000000000179052610c2b90849061342a565b5f831180156131cc575081610100013583105b15613203576040517f9469744400000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b61321084612256846112eb565b61321e84836020013561261e565b428261012001351161119a576040517fc56873ba00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b5f805f8351604103613293576020840151604085015160608601515f1a613285888285856134af565b95509550955050505061329e565b505081515f91506002905b9250925092565b5f8260038111156132b8576132b8613ef4565b036132c1575050565b60018260038111156132d5576132d5613ef4565b0361330c576040517ff645eedf00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b600282600381111561332057613320613ef4565b0361335f576040517ffce698f7000000000000000000000000000000000000000000000000000000008152600481018290526024015b60405180910390fd5b600382600381111561337357613373613ef4565b036122c5576040517fd78bce0c00000000000000000000000000000000000000000000000000000000815260048101829052602401613356565b634e487b715f52806020526024601cfd5b5f6133cc6020830183613c16565b90506020820135365f6133e26040860186613d7f565b91509150604051818382375f80838387895af1611944573d5f803e3d5ffd5b5f36816134116040850185613d7f565b90925090506004811061342357813592505b5050919050565b5f8060205f8451602086015f885af180613449576040513d5f823e3d81fd5b50505f513d9150811561346057806001141561346d565b6001600160a01b0384163b155b1561119a576040517f5274afe70000000000000000000000000000000000000000000000000000000081526001600160a01b0385166004820152602401613356565b5f80807f7fffffffffffffffffffffffffffffff5d576e7357a4501ddfe92f46681b20a08411156134e857505f9150600390508261358b565b604080515f808252602082018084528a905260ff891692820192909252606081018790526080810186905260019060a0016020604051602081039080840390855afa158015613539573d5f803e3d5ffd5b50506040517fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe001519150506001600160a01b03811661358257505f92506001915082905061358b565b92505f91508190505b9450945094915050565b5f805f604084860312156135a7575f80fd5b83359250602084013567ffffffffffffffff808211156135c5575f80fd5b818601915086601f8301126135d8575f80fd5b8135818111156135e6575f80fd5b8760208285010111156135f7575f80fd5b6020830194508093505050509250925092565b5f60a0828403121561361a575f80fd5b50919050565b5f60a08284031215613630575f80fd5b6104d0838361360a565b6001600160a01b0381168114610c89575f80fd5b80356136598161363a565b919050565b5f610260828403121561361a575f80fd5b5f8083601f84011261367f575f80fd5b50813567ffffffffffffffff811115613696575f80fd5b6020830191508360208260051b85010111156136b0575f80fd5b9250929050565b5f6040828403121561361a575f80fd5b5f6060828403121561361a575f80fd5b5f805f805f805f805f6101008a8c0312156136f0575f80fd5b6136f98a61364e565b985060208a0135975060408a013567ffffffffffffffff8082111561371c575f80fd5b6137288d838e0161365e565b985060608c013591508082111561373d575f80fd5b6137498d838e0161366f565b909850965060808c0135915080821115613761575f80fd5b61376d8d838e016136b7565b955060a08c0135915080821115613782575f80fd5b61378e8d838e016136c7565b945060c08c01359150808211156137a3575f80fd5b6137af8d838e016136c7565b935060e08c01359150808211156137c4575f80fd5b506137d18c828d016136c7565b9150509295985092959850929598565b5f80604083850312156137f2575f80fd5b82356137fd8161363a565b9150602083013567ffffffffffffffff811115613818575f80fd5b613824858286016136b7565b9150509250929050565b5f610160828403121561361a575f80fd5b5f805f805f60a08688031215613853575f80fd5b853561385e8161363a565b9450602086013567ffffffffffffffff8082111561387a575f80fd5b61388689838a0161382e565b9550604088013591508082111561389b575f80fd5b6138a789838a016136c7565b94506060880135935060808801359150808211156138c3575f80fd5b506138d0888289016136c7565b9150509295509295909350565b5f602082840312156138ed575f80fd5b5035919050565b8015158114610c89575f80fd5b5f805f805f8060a08789031215613916575f80fd5b86356139218161363a565b955060208701356139318161363a565b94506040870135613941816138f4565b9350606087013567ffffffffffffffff8082111561395d575f80fd5b6139698a838b0161365e565b9450608089013591508082111561397e575f80fd5b5061398b89828a0161366f565b979a9699509497509295939492505050565b5f805f606084860312156139af575f80fd5b83356139ba8161363a565b925060208401359150604084013567ffffffffffffffff8111156139dc575f80fd5b6139e8868287016136c7565b9150509250925092565b5f60208284031215613a02575f80fd5b813567ffffffffffffffff811115613a18575f80fd5b611bd38482850161382e565b5f805f805f805f8060e0898b031215613a3b575f80fd5b613a448961364e565b975060208901359650604089013567ffffffffffffffff80821115613a67575f80fd5b613a738c838d0161365e565b975060608b0135915080821115613a88575f80fd5b613a948c838d0161366f565b909750955060808b0135915080821115613aac575f80fd5b613ab88c838d016136b7565b945060a08b0135915080821115613acd575f80fd5b613ad98c838d016136c7565b935060c08b0135915080821115613aee575f80fd5b50613afb8b828c016136c7565b9150509295985092959890939650565b5f805f805f8060c08789031215613b20575f80fd5b613b298761364e565b9550602087013567ffffffffffffffff80821115613b45575f80fd5b613b518a838b0161382e565b96506040890135915080821115613b66575f80fd5b613b728a838b016136c7565b9550606089013594506080890135915080821115613b8e575f80fd5b613b9a8a838b016136c7565b935060a0890135915080821115613baf575f80fd5b50613bbc89828a016136c7565b9150509295509295509295565b5f60208284031215613bd9575f80fd5b813567ffffffffffffffff811115613bef575f80fd5b611bd38482850161365e565b5f60208284031215613c0b575f80fd5b81516104d0816138f4565b5f60208284031215613c26575f80fd5b81356104d08161363a565b81835281816020850137505f602082840101525f60207fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f840116840101905092915050565b803565ffffffffffff81168114613659575f80fd5b5f6001600160a01b0380871683528086166020840152508360408301526080606083015282357fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe1843603018112613ce2575f80fd5b830160208101903567ffffffffffffffff811115613cfe575f80fd5b803603821315613d0c575f80fd5b60606080850152613d2160e085018284613c31565b915050613d3060208501613c78565b65ffffffffffff80821660a086015280613d4c60408801613c78565b1660c086015250508091505095945050505050565b5f60208284031215613d71575f80fd5b8135600381106104d0575f80fd5b5f8083357fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe1843603018112613db2575f80fd5b83018035915067ffffffffffffffff821115613dcc575f80fd5b6020019150368190038213156136b0575f80fd5b818382375f9101908152919050565b5f8083357fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe1843603018112613e22575f80fd5b83018035915067ffffffffffffffff821115613e3c575f80fd5b6020019150600581901b36038213156136b0575f80fd5b7f4e487b71000000000000000000000000000000000000000000000000000000005f52603260045260245ffd5b5f82357fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffa1833603018112613eb2575f80fd5b9190910192915050565b80820180821115612efa577f4e487b71000000000000000000000000000000000000000000000000000000005f52601160045260245ffd5b7f4e487b71000000000000000000000000000000000000000000000000000000005f52602160045260245ffd5b5f60208284031215613f31575f80fd5b8135600481106104d0575f80fd5b838152604060208201525f611c60604083018486613c31565b5f60208284031215613f68575f80fd5b81517fffffffff00000000000000000000000000000000000000000000000000000000811681146104d0575f80fd5b602081525f611bd3602083018486613c31565b5f81515f5b81811015613fc95760208185018101518683015201613faf565b505f93019283525090919050565b5f611bd3613fe58386613faa565b84613faa565b5f613ff68286613faa565b93845250506020820152604001919050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52601260045260245ffd5b5f60a0820190506001600160a01b0380845116835280602085015116602084015280604085015116604084015250606083015160608301526080830151600381106140a7577f4e487b71000000000000000000000000000000000000000000000000000000005f52602160045260245ffd5b8060808401525092915050565b5f602082840312156140c4575f80fd5b5051919050565b5f80858511156140d9575f80fd5b838611156140e5575f80fd5b5050820193919092039150565b5f805f8060808587031215614105575f80fd5b84356141108161363a565b935060208501356141208161363a565b9250604085013591506060850135614137816138f4565b939692955090935050565b5f805f8060808587031215614155575f80fd5b84356141608161363a565b935060208501356141708161363a565b925060408501356141808161363a565b9396929550929360600135925050565b5f805f805f805f60e0888a0312156141a6575f80fd5b87356141b18161363a565b965060208801356141c18161363a565b955060408801356141d18161363a565b945060608801356141e18161363a565b93506080880135925060a08801356141f88161363a565b8092505060c0880135905092959891949750929550565b5f805f60608486031215614221575f80fd5b833561422c8161363a565b9250602084013561423c8161363a565b929592945050506040919091013590565b5f805f805f60a08688031215614261575f80fd5b853561426c8161363a565b9450602086013561427c8161363a565b9350604086013561428c8161363a565b92506060860135915060808601356142a3816138f4565b809150509295509295909350565b5f60ff8316806142e8577f4e487b71000000000000000000000000000000000000000000000000000000005f52601260045260245ffd5b8060ff8416069150509291505056fea26469706673582212209be58acada353061a2a202cc7011f3c91393e0f2e305e9202445b042fc4a4ce664736f6c6343000817003360c060405234801561000f575f80fd5b5060405161096338038061096383398101604081905261002e91610060565b6001600160a01b039182166080521660a052610091565b80516001600160a01b038116811461005b575f80fd5b919050565b5f8060408385031215610071575f80fd5b61007a83610045565b915061008860208401610045565b90509250929050565b60805160a05161089e6100c55f395f81816048015281816101f2015261038101525f818160d30152610327015261089e5ff3fe608060405234801561000f575f80fd5b506004361061003f575f3560e01c80636afdd85014610043578063b519d36914610093578063bc1178e6146100a8575b5f80fd5b61006a7f000000000000000000000000000000000000000000000000000000000000000081565b60405173ffffffffffffffffffffffffffffffffffffffff909116815260200160405180910390f35b6100a66100a136600461060a565b6100bb565b005b6100a66100b6366004610648565b61030f565b3373ffffffffffffffffffffffffffffffffffffffff7f000000000000000000000000000000000000000000000000000000000000000016811461012b576040517f7c214f0400000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6060820135156101af57600161014760a08401608085016106dd565b6002811115610158576101586106b0565b036101b3576101af61016d6020840184610702565b61017d6040850160208601610702565b606085018035906101919060408801610702565b73ffffffffffffffffffffffffffffffffffffffff169291906104cc565b5050565b60026101c560a08401608085016106dd565b60028111156101d6576101d66106b0565b036102dd5773ffffffffffffffffffffffffffffffffffffffff7f0000000000000000000000000000000000000000000000000000000000000000166336c785166102246020850185610702565b6102346040860160208701610702565b606086018035906102489060408901610702565b60405160e086901b7fffffffff0000000000000000000000000000000000000000000000000000000016815273ffffffffffffffffffffffffffffffffffffffff94851660048201529284166024840152908316604483015290911660648201526084015f604051808303815f87803b1580156102c3575f80fd5b505af11580156102d5573d5f803e3d5ffd5b505050505050565b6040517fc79aaa4400000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b3373ffffffffffffffffffffffffffffffffffffffff7f000000000000000000000000000000000000000000000000000000000000000016811461037f576040517f7c214f0400000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b7f000000000000000000000000000000000000000000000000000000000000000073ffffffffffffffffffffffffffffffffffffffff16632b67b57086604051806060016040528060405180608001604052808a73ffffffffffffffffffffffffffffffffffffffff1681526020018973ffffffffffffffffffffffffffffffffffffffff16815260200188604001602081019061041d919061071b565b65ffffffffffff16815260200188602001602081019061043d919061071b565b65ffffffffffff1690528152306020820152604090810190610465906060890190890161071b565b65ffffffffffff1690526104798680610740565b6040518563ffffffff1660e01b815260040161049894939291906107a8565b5f604051808303815f87803b1580156104af575f80fd5b505af11580156104c1573d5f803e3d5ffd5b505050505050505050565b6040805173ffffffffffffffffffffffffffffffffffffffff85811660248301528416604482015260648082018490528251808303909101815260849091019091526020810180517bffffffffffffffffffffffffffffffffffffffffffffffffffffffff167f23b872dd00000000000000000000000000000000000000000000000000000000179052610561908590610567565b50505050565b5f8060205f8451602086015f885af180610586576040513d5f823e3d81fd5b50505f513d9150811561059d5780600114156105b7565b73ffffffffffffffffffffffffffffffffffffffff84163b155b15610561576040517f5274afe700000000000000000000000000000000000000000000000000000000815273ffffffffffffffffffffffffffffffffffffffff8516600482015260240160405180910390fd5b5f60a0828403121561061a575f80fd5b50919050565b803573ffffffffffffffffffffffffffffffffffffffff81168114610643575f80fd5b919050565b5f805f806080858703121561065b575f80fd5b61066485610620565b935061067260208601610620565b925060408501359150606085013567ffffffffffffffff811115610694575f80fd5b8501606081880312156106a5575f80fd5b939692955090935050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52602160045260245ffd5b5f602082840312156106ed575f80fd5b8135600381106106fb575f80fd5b9392505050565b5f60208284031215610712575f80fd5b6106fb82610620565b5f6020828403121561072b575f80fd5b813565ffffffffffff811681146106fb575f80fd5b5f8083357fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe1843603018112610773575f80fd5b83018035915067ffffffffffffffff82111561078d575f80fd5b6020019150368190038213156107a1575f80fd5b9250929050565b5f61010073ffffffffffffffffffffffffffffffffffffffff80881684528651818151166020860152816020820151166040860152604081015165ffffffffffff80821660608801528060608401511660808801525050508060208801511660a085015250604086015160c08401528060e08401528381840152506101208385828501375f838501820152601f9093017fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe01690910190910194935050505056fea2646970667358221220f3565f6589500276fcbb6fb33d3ee3b534d9566c5b2732fe10a6039b753c3a0764736f6c63430008170033
    /// ```
    #[rustfmt::skip]
    #[allow(clippy::all)]
    pub static BYTECODE: alloy_sol_types::private::Bytes = alloy_sol_types::private::Bytes::from_static(
        b"a\x01 `@R4\x80\x15b\0\0\x11W_\x80\xFD[P`@Qb\0N\xF68\x03\x80b\0N\xF6\x839\x81\x01`@\x81\x90Rb\0\x004\x91b\0\x01uV[F`\xA0\x81\x81R`@\x80Q\x7F\x8Bs\xC3\xC6\x9B\xB8\xFE=Q.\xCCL\xF7Y\xCCy#\x9F{\x17\x9B\x0F\xFA\xCA\xA9\xA7]R+9@\x0F` \x82\x01R\x7Fd\xAF\xEC{\xE6Q\xC9/\x86uK\xEB+\xD5\xEE\xAF/\xA9^\x83\xFA\xF4\xAE\xE9\x89\x87}\xDE\x08\xE4I\x8C\x91\x81\x01\x91\x90\x91R\x7F\xC8\x9E\xFD\xAAT\xC0\xF2\x0Cz\xDFa(\x82\xDF\tP\xF5\xA9Qc~\x03\x07\xCD\xCBLg/)\x8B\x8B\xC6``\x82\x01R`\x80\x81\x01\x92\x90\x92R0\x90\x82\x01R`\xC0\x01`@\x80Q`\x1F\x19\x81\x84\x03\x01\x81R\x90\x82\x90R\x80Q` \x90\x91\x01 `\x80R`\x01\x80U`\x01`\x01`\xA0\x1B\x03\x84\x16`\xC0R0\x90\x82\x90b\0\0\xFE\x90b\0\x01OV[`\x01`\x01`\xA0\x1B\x03\x92\x83\x16\x81R\x91\x16` \x82\x01R`@\x01`@Q\x80\x91\x03\x90_\xF0\x80\x15\x80\x15b\0\x01/W=_\x80>=_\xFD[P`\x01`\x01`\xA0\x1B\x03\x90\x81\x16`\xE0R\x91\x90\x91\x16a\x01\0RPb\0\x01\xC6\x90PV[a\tc\x80b\0E\x93\x839\x01\x90V[`\x01`\x01`\xA0\x1B\x03\x81\x16\x81\x14b\0\x01rW_\x80\xFD[PV[_\x80_``\x84\x86\x03\x12\x15b\0\x01\x88W_\x80\xFD[\x83Qb\0\x01\x95\x81b\0\x01]V[` \x85\x01Q\x90\x93Pb\0\x01\xA8\x81b\0\x01]V[`@\x85\x01Q\x90\x92Pb\0\x01\xBB\x81b\0\x01]V[\x80\x91PP\x92P\x92P\x92V[`\x80Q`\xA0Q`\xC0Q`\xE0Qa\x01\0QaC-b\0\x02f_9_\x81\x81a\x02W\x01R\x81\x81a\x07\x84\x01Ra\x16\xDC\x01R_\x81\x81a\x01\x97\x01R\x81\x81a\x07\xBA\x01R\x81\x81a\x18\x8C\x01R\x81\x81a\x1E,\x01R\x81\x81a\x1F\x1E\x01R\x81\x81a ,\x01R\x81\x81a#\x0B\x01R\x81\x81a$'\x01Ra0<\x01R_\x81\x81a\x038\x01R\x81\x81a\x04\x12\x01R\x81\x81a\x06\xA7\x01R\x81\x81a\x15 \x01Ra\x15\xFF\x01R_a\x04\xDA\x01R_a\x05\xA4\x01RaC-_\xF3\xFE`\x80`@R`\x046\x10a\x01&W_5`\xE0\x1C\x80c\xA5\xCD\xC8\xFC\x11a\0\xA1W\x80c\xC6\x18a\x81\x11a\0qW\x80c\xDBXw(\x11a\0WW\x80c\xDBXw(\x14a\x03yW\x80c\xE2B\x92N\x14a\x03\x8CW\x80c\xFA\\\xD5l\x14a\x03\xABW_\x80\xFD[\x80c\xC6\x18a\x81\x14a\x03'W\x80c\xCB\xA6s\xA7\x14a\x03ZW_\x80\xFD[\x80c\xA5\xCD\xC8\xFC\x14a\x02\xABW\x80c\xA7\xABI\xBC\x14a\x02\xCAW\x80c\xAE\x80\xC5\x84\x14a\x02\xE9W\x80c\xB1\x1F\x12b\x14a\x03\x08W_\x80\xFD[\x80cQ\xD4h\x15\x11a\0\xF6W\x80co5\xD2\xD2\x11a\0\xDCW\x80co5\xD2\xD2\x14a\x02FW\x80c\x87U0\xFF\x14a\x02yW\x80c\x995\xC8h\x14a\x02\x98W_\x80\xFD[\x80cQ\xD4h\x15\x14a\x02\x12W\x80cZ\xA0\xE9]\x14a\x02'W_\x80\xFD[\x80c\x16&\xBA~\x14a\x011W\x80c)\xBC\xDC\x95\x14a\x01\x86W\x80c6D\xE5\x15\x14a\x01\xD1W\x80cL\x9E\x03\xD3\x14a\x01\xF3W_\x80\xFD[6a\x01-W\0[_\x80\xFD[4\x80\x15a\x01<W_\x80\xFD[Pa\x01Pa\x01K6`\x04a5\x95V[a\x03\xCAV[`@Q\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90\x91\x16\x81R` \x01[`@Q\x80\x91\x03\x90\xF3[4\x80\x15a\x01\x91W_\x80\xFD[Pa\x01\xB9\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[`@Q`\x01`\x01`\xA0\x1B\x03\x90\x91\x16\x81R` \x01a\x01}V[4\x80\x15a\x01\xDCW_\x80\xFD[Pa\x01\xE5a\x04\xD7V[`@Q\x90\x81R` \x01a\x01}V[4\x80\x15a\x01\xFEW_\x80\xFD[Pa\x01\xE5a\x02\r6`\x04a6 V[a\x05\xC6V[a\x02%a\x02 6`\x04a6\xD7V[a\x06gV[\0[4\x80\x15a\x022W_\x80\xFD[Pa\x02%a\x02A6`\x04a7\xE1V[a\t\xE8V[4\x80\x15a\x02QW_\x80\xFD[Pa\x01\xB9\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[4\x80\x15a\x02\x84W_\x80\xFD[Pa\x01\xE5a\x02\x936`\x04a6 V[a\x0C0V[a\x02%a\x02\xA66`\x04a8?V[a\x0C_V[4\x80\x15a\x02\xB6W_\x80\xFD[Pa\x02%a\x02\xC56`\x04a8\xDDV[a\x0C\x7FV[4\x80\x15a\x02\xD5W_\x80\xFD[Pa\x02%a\x02\xE46`\x04a9\x01V[a\x0C\x8CV[4\x80\x15a\x02\xF4W_\x80\xFD[Pa\x02%a\x03\x036`\x04a9\x9DV[a\x10gV[4\x80\x15a\x03\x13W_\x80\xFD[Pa\x01\xE5a\x03\"6`\x04a9\xF2V[a\x12\xEBV[4\x80\x15a\x032W_\x80\xFD[Pa\x01\xB9\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[4\x80\x15a\x03eW_\x80\xFD[Pa\x02%a\x03t6`\x04a:$V[a\x14\xEAV[a\x02%a\x03\x876`\x04a;\x0BV[a\x18xV[4\x80\x15a\x03\x97W_\x80\xFD[Pa\x01\xE5a\x03\xA66`\x04a;\xC9V[a\x19LV[4\x80\x15a\x03\xB6W_\x80\xFD[Pa\x02%a\x03\xC56`\x04a;\xC9V[a\x1A\x8EV[_\x80a\x03\xD7\x85\x85\x85a\x1BPV[`@Q\x7F\xE7V\0\xC3\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x80\x83\x16`\x04\x83\x01R\x91\x92P\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90\x91\x16\x90c\xE7V\0\xC3\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x04YW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x04}\x91\x90a;\xFBV[\x15a\x04\xABW\x7F\x16&\xBA~5oYy\xDD5Z=+\xFBC\xE8\x04 \xA4\x80\xC3\xB8T\xED\xCE(j\x82\xD7Ihi\x91PPa\x04\xD0V[P\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90P[\x93\x92PPPV[_\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0F\x14a\x05\xA1W`@\x80Q\x7F\x8Bs\xC3\xC6\x9B\xB8\xFE=Q.\xCCL\xF7Y\xCCy#\x9F{\x17\x9B\x0F\xFA\xCA\xA9\xA7]R+9@\x0F` \x82\x01R\x7Fd\xAF\xEC{\xE6Q\xC9/\x86uK\xEB+\xD5\xEE\xAF/\xA9^\x83\xFA\xF4\xAE\xE9\x89\x87}\xDE\x08\xE4I\x8C\x91\x81\x01\x91\x90\x91R\x7F\xC8\x9E\xFD\xAAT\xC0\xF2\x0Cz\xDFa(\x82\xDF\tP\xF5\xA9Qc~\x03\x07\xCD\xCBLg/)\x8B\x8B\xC6``\x82\x01RF`\x80\x82\x01R0`\xA0\x82\x01R`\xC0\x01`@Q` \x81\x83\x03\x03\x81R\x90`@R\x80Q\x90` \x01 \x90P\x90V[P\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90V[_\x7Fh\xB8\xE9M\xC0wE\x82A\xD6\xC8\xD8\x9F\nve\xC7\xCD\xA2\xCF\xE7\x0C\x9E\xB4C~\xFE\xE1f<f\xFEa\x05\xF5` \x84\x01\x84a<\x16V[\x83` \x015\x84`@\x015\x85``\x015\x86`\x80\x015`@Q` \x01a\x06J\x96\x95\x94\x93\x92\x91\x90\x95\x86R`\x01`\x01`\xA0\x1B\x03\x94\x90\x94\x16` \x86\x01R`@\x85\x01\x92\x90\x92R``\x84\x01R`\x80\x83\x01R`\xA0\x82\x01R`\xC0\x01\x90V[`@Q` \x81\x83\x03\x03\x81R\x90`@R\x80Q\x90` \x01 \x90P\x91\x90PV[a\x06oa\x1B\xDBV[`@Q\x7F\xE7V\0\xC3\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x8A\x81\x16`\x04\x83\x01R\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x90c\xE7V\0\xC3\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x06\xECW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x07\x10\x91\x90a;\xFBV[a\x07FW`@Q\x7F\xB31\xE4!\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\x02\0\x87\x015\x88\x15\x80\x15\x90a\x07`WPa\x01`\x88\x015\x89\x11\x15[\x15a\x07\x7FWa\x07|\x89a\x02\0\x8A\x015a\x01`\x8B\x015`\x01a\x1C\x1EV[\x90P[a\x07\xB0\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x8B\x83\x8B\x8B\x8B\x8B\x8B\x8Ba\x1CiV[`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16c\xBC\x11x\xE6a\x07\xEF`\xC0\x8B\x01`\xA0\x8C\x01a<\x16V[a\x08\x01a\x01@\x8C\x01a\x01 \x8D\x01a<\x16V[`@Q\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\xE0\x85\x90\x1B\x16\x81Ra\x08D\x92\x91\x90a\x01`\x8E\x015\x90\x88\x90`\x04\x01a<\x8DV[_`@Q\x80\x83\x03\x81_\x87\x80;\x15\x80\x15a\x08[W_\x80\xFD[PZ\xF1\x15\x80\x15a\x08mW=_\x80>=_\xFD[PPPPa\x08\xB4\x88\x8B\x83\x8C_\x14\x80a\x08\x89WPa\x01`\x8C\x015\x8D\x11[a\x08\x93W\x8Ca\x08\x9AV[a\x01`\x8C\x015[\x89\x8C\x8C`\x02a\x08\xAF`@\x8E\x01` \x8F\x01a=aV[a\x1E\x01V[a\x08\xC1`@\x89\x01\x89a=\x7FV[`@Qa\x08\xCF\x92\x91\x90a=\xE0V[`@Q\x90\x81\x90\x03\x90 \x7F\x0F\xCE\0|8\xC6\xC8\xED\x9ET[:\x14\x80\x95v'8a\x8F\x8C!\xB6s\"&\x13\xE4\xD4W4\xB6a\t\t`\xA0\x8B\x01`\x80\x8C\x01a<\x16V[a\t\x19`\xC0\x8C\x01`\xA0\x8D\x01a<\x16V[a\t+a\x01@\x8D\x01a\x01 \x8E\x01a<\x16V[a\t=a\x01\xE0\x8E\x01a\x01\xC0\x8F\x01a<\x16V[\x8E\x15\x80a\tNWPa\x01`\x8E\x015\x8F\x11[a\tXW\x8Ea\t_V[a\x01@\x8E\x015[a\x02\0\x8F\x015\x88\x14a\tqW\x87a\txV[a\x01\xE0\x8F\x015[\x8F`\xE0\x01` \x81\x01\x90a\t\x8B\x91\x90a<\x16V[`@\x80Q`\x01`\x01`\xA0\x1B\x03\x98\x89\x16\x81R\x96\x88\x16` \x88\x01R\x94\x87\x16\x94\x86\x01\x94\x90\x94R\x91\x85\x16``\x85\x01R`\x80\x84\x01R`\xA0\x83\x01R\x90\x91\x16`\xC0\x82\x01R`\xE0\x01`@Q\x80\x91\x03\x90\xA2Pa\t\xDD`\x01\x80UV[PPPPPPPPPV[_[a\t\xF4\x82\x80a=\xEFV[\x90P\x81\x10\x15a\x0B\x06W6a\n\x08\x83\x80a=\xEFV[\x83\x81\x81\x10a\n\x18Wa\n\x18a>SV[\x90P` \x02\x81\x01\x90a\n*\x91\x90a>\x80V[\x90P`\x01`\x01`\xA0\x1B\x03\x84\x16c\xA8\xC4\xBC\x95a\nH` \x84\x01\x84a<\x16V[`@Q\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\xE0\x84\x90\x1B\x16\x81R`\x01`\x01`\xA0\x1B\x03\x90\x91\x16`\x04\x82\x01R`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\n\xA2W=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\n\xC6\x91\x90a;\xFBV[\x15a\n\xFDW`@Q\x7F\xC9\x9E\x88r\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[P`\x01\x01a\t\xEAV[P_[a\x0B\x16` \x83\x01\x83a=\xEFV[\x90P\x81\x10\x15a\x0C+W6a\x0B-` \x84\x01\x84a=\xEFV[\x83\x81\x81\x10a\x0B=Wa\x0B=a>SV[\x90P` \x02\x81\x01\x90a\x0BO\x91\x90a>\x80V[\x90P`\x01`\x01`\xA0\x1B\x03\x84\x16c\xA8\xC4\xBC\x95a\x0Bm` \x84\x01\x84a<\x16V[`@Q\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\xE0\x84\x90\x1B\x16\x81R`\x01`\x01`\xA0\x1B\x03\x90\x91\x16`\x04\x82\x01R`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x0B\xC7W=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x0B\xEB\x91\x90a;\xFBV[\x15a\x0C\"W`@Q\x7F\xC9\x9E\x88r\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[P`\x01\x01a\x0B\tV[PPPV[_\x7F\xAEgk\xF6\x91:\xC2h\x9Bs1)<\x98\x9F\xE7r1$\xFA\xF8\xB5\xD2u\xF0o\xBC\xEB\xC7yP\ta\x05\xF5` \x84\x01\x84a<\x16V[a\x0Ci\x84\x82a\"\x12V[a\x0Cx\x85\x85\x85\x85`\x01\x80a\"\xC9V[PPPPPV[a\x0C\x893\x82a&\x1EV[PV[a\x0C\xBF`@Q\x80`\xC0\x01`@R\x80_\x81R` \x01_\x81R` \x01_\x81R` \x01_\x81R` \x01_\x81R` \x01_\x81RP\x90V[_[\x82\x81\x10\x15a\x10SW6\x84\x84\x83\x81\x81\x10a\x0C\xDCWa\x0C\xDCa>SV[\x90P` \x02\x81\x01\x90a\x0C\xEE\x91\x90a>\x80V[\x90P6_a\x0C\xFF`@\x84\x01\x84a=\x7FV[\x90\x92P\x90P`\x01`\x01`\xA0\x1B\x03\x8B\x16c\xA8\xC4\xBC\x95a\r ` \x86\x01\x86a<\x16V[`@Q\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\xE0\x84\x90\x1B\x16\x81R`\x01`\x01`\xA0\x1B\x03\x90\x91\x16`\x04\x82\x01R`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\rzW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\r\x9E\x91\x90a;\xFBV[\x15a\x10EW_\x89\x15a\r\xDCW`@Q\x7F}a{\xB3\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[`\x04\x82\x10a\r\xE8WP\x815[\x7F\xC0:\x9D\xE9\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x82\x16\x01a\x0EVWa\x0E=\x83\x83\x8D\x8Ca&\xC4V[\x86` \x01\x81\x81Qa\x0EN\x91\x90a>\xBCV[\x90RPa\x10CV[\x7F$:K\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x82\x16\x01a\x0E\xBCWa\x0E\xAB\x83\x83\x8D\x8Ca'\xDAV[\x86``\x01\x81\x81Qa\x0EN\x91\x90a>\xBCV[\x7F}\xC4\xF4X\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x82\x16\x01a\x0FGWa\x0F\x11\x83\x83\x8D\x8Ca)NV[`\xA0\x88\x01R`\x80\x87\x01\x81\x90R``\x87\x01\x80Qa\x0F.\x90\x83\x90a>\xBCV[\x90RP`\xA0\x86\x01Q` \x87\x01\x80Qa\x0EN\x90\x83\x90a>\xBCV[\x7Fh\x93\x1Bk\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x82\x16\x01a\x0F\xABWa\x0F\x9C\x83\x83\x8D\x8Ca+\xB6V[\x86Q\x87\x90a\x0EN\x90\x83\x90a>\xBCV[\x7F\x0C\x9B\xE7\xE4\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x82\x16\x01a\x10\x11Wa\x10\0\x83\x83\x8D\x8Ca,\xC0V[\x86`@\x01\x81\x81Qa\x0EN\x91\x90a>\xBCV[`@Q\x7F\x05a\xD8\xB3\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[P[PPP\x80`\x01\x01\x90Pa\x0C\xC1V[Pa\x10^\x84\x82a.)V[PPPPPPPV[`\x03a\x10v` \x83\x01\x83a?!V[`\x03\x81\x11\x15a\x10\x87Wa\x10\x87a>\xF4V[\x03a\x10\xECW`\x01`\x01`\xA0\x1B\x03\x83\x16a\x10\xAC\x83a\x10\xA7`@\x85\x01\x85a=\x7FV[a\x1BPV[`\x01`\x01`\xA0\x1B\x03\x16\x14a\x0C+W`@Q\x7F\xB8\x1DX\xE7\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[`\x01a\x10\xFB` \x83\x01\x83a?!V[`\x03\x81\x11\x15a\x11\x0CWa\x11\x0Ca>\xF4V[\x03a\x11\xA0W\x7F\x19Ethereum Signed Message:\n32\0\0\0\0_\x90\x81R`\x1C\x83\x90R`<\x90 `\x01`\x01`\xA0\x1B\x03\x84\x16a\x11Z\x82a\x10\xA7`@\x86\x01\x86a=\x7FV[`\x01`\x01`\xA0\x1B\x03\x16\x14a\x11\x9AW`@Q\x7FdJ\xE6\xC3\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[PPPPV[`\x02a\x11\xAF` \x83\x01\x83a?!V[`\x03\x81\x11\x15a\x11\xC0Wa\x11\xC0a>\xF4V[\x03a\x12\xB9W\x7F\x16&\xBA~\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\x01`\x01`\xA0\x1B\x03\x84\x16c\x16&\xBA~\x84a\x12\x03`@\x86\x01\x86a=\x7FV[`@Q\x84c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01a\x12!\x93\x92\x91\x90a??V[` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x12<W=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x12`\x91\x90a?XV[\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x14a\x0C+W`@Q\x7F]R\xCB\xE3\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[`@Q\x7F`\xCD@-\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[_a\x12\xF4a\x04\xD7V[\x7F\xD2\x8E\x80\x9Bp\x8F^\xE3\x8B\xE84}m\x86\x9D\x822I<\tJ\xB2\xDD\xE9\x83i\xE4\x10#i\xA9\x9Da\x13\x1F\x84\x80a=\x7FV[`@Q` \x01a\x130\x92\x91\x90a?\x97V[`@Q` \x81\x83\x03\x03\x81R\x90`@R\x80Q\x90` \x01 \x84` \x015\x85`@\x01` \x81\x01\x90a\x13^\x91\x90a<\x16V[`@\x80Q` \x81\x01\x95\x90\x95R\x84\x01\x92\x90\x92R``\x83\x01R`\x01`\x01`\xA0\x1B\x03\x16`\x80\x82\x01R`\xA0\x01`@\x80Q\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x81\x84\x03\x01\x81R\x91\x90Ra\x13\xC4`\x80\x85\x01``\x86\x01a<\x16V[a\x13\xD4`\xA0\x86\x01`\x80\x87\x01a<\x16V[a\x13\xE4`\xC0\x87\x01`\xA0\x88\x01a<\x16V[`\xC0\x87\x015`\xE0\x88\x015a\x01\0\x89\x015a\x01 \x8A\x015a\x14\x0Ca\x01`\x8C\x01a\x01@\x8D\x01a<\x16V[`@\x80Q`\x01`\x01`\xA0\x1B\x03\x99\x8A\x16` \x82\x01R\x97\x89\x16\x90\x88\x01R\x94\x87\x16``\x87\x01R`\x80\x86\x01\x93\x90\x93R`\xA0\x85\x01\x91\x90\x91R`\xC0\x84\x01R`\xE0\x83\x01R\x90\x91\x16a\x01\0\x82\x01Ra\x01 \x01`@\x80Q\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x81\x84\x03\x01\x81R\x90\x82\x90Ra\x14\x92\x92\x91` \x01a?\xD7V[`@Q` \x81\x83\x03\x03\x81R\x90`@R\x80Q\x90` \x01 `@Q` \x01a\x06J\x92\x91\x90\x7F\x19\x01\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x02\x81\x01\x92\x90\x92R`\"\x82\x01R`B\x01\x90V[a\x14\xF2a\x1B\xDBV[`@Q\x7F\x02\xCC%\r\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R3`\x04\x82\x01R\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\x01`\x01`\xA0\x1B\x03\x16\x90c\x02\xCC%\r\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x15mW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x15\x91\x91\x90a;\xFBV[a\x15\xC7W`@Q\x7F\xC19\xEA\xBD\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[`@Q\x7F\xE7V\0\xC3\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x89\x81\x16`\x04\x83\x01R\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x90c\xE7V\0\xC3\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x16DW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x16h\x91\x90a;\xFBV[a\x16\x9EW`@Q\x7F\xB31\xE4!\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\x02\0\x86\x015\x87\x15\x80\x15\x90a\x16\xB8WPa\x01`\x87\x015\x88\x11\x15[\x15a\x16\xD7Wa\x16\xD4\x88a\x02\0\x89\x015a\x01`\x8A\x015`\x01a\x1C\x1EV[\x90P[a\x17\x08\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x8A\x83\x8A\x8A\x8A\x8A\x8A\x8Aa\x1CiV[a\x17E\x87\x8A\x83\x8B\x15\x80a\x17\x1FWPa\x01`\x8B\x015\x8C\x11[a\x17)W\x8Ba\x170V[a\x01`\x8B\x015[\x88\x8B\x8B`\x01a\x08\xAF`@\x8D\x01` \x8E\x01a=aV[a\x17R`@\x88\x01\x88a=\x7FV[`@Qa\x17`\x92\x91\x90a=\xE0V[`@Q\x90\x81\x90\x03\x90 \x7F\x0F\xCE\0|8\xC6\xC8\xED\x9ET[:\x14\x80\x95v'8a\x8F\x8C!\xB6s\"&\x13\xE4\xD4W4\xB6a\x17\x9A`\xA0\x8A\x01`\x80\x8B\x01a<\x16V[a\x17\xAA`\xC0\x8B\x01`\xA0\x8C\x01a<\x16V[a\x17\xBCa\x01@\x8C\x01a\x01 \x8D\x01a<\x16V[a\x17\xCEa\x01\xE0\x8D\x01a\x01\xC0\x8E\x01a<\x16V[\x8D\x15\x80a\x17\xDFWPa\x01`\x8D\x015\x8E\x11[a\x17\xE9W\x8Da\x17\xF0V[a\x01@\x8D\x015[a\x02\0\x8E\x015\x88\x14a\x18\x02W\x87a\x18\tV[a\x01\xE0\x8E\x015[\x8E`\xE0\x01` \x81\x01\x90a\x18\x1C\x91\x90a<\x16V[`@\x80Q`\x01`\x01`\xA0\x1B\x03\x98\x89\x16\x81R\x96\x88\x16` \x88\x01R\x94\x87\x16\x94\x86\x01\x94\x90\x94R\x91\x85\x16``\x85\x01R`\x80\x84\x01R`\xA0\x83\x01R\x90\x91\x16`\xC0\x82\x01R`\xE0\x01`@Q\x80\x91\x03\x90\xA2Pa\x18n`\x01\x80UV[PPPPPPPPV[a\x18\x82\x85\x83a\"\x12V[`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16c\xBC\x11x\xE6a\x18\xC1`\x80\x88\x01``\x89\x01a<\x16V[a\x18\xD1`\xA0\x89\x01`\x80\x8A\x01a<\x16V[\x88`\xC0\x015\x85`@Q\x85c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01a\x18\xF6\x94\x93\x92\x91\x90a<\x8DV[_`@Q\x80\x83\x03\x81_\x87\x80;\x15\x80\x15a\x19\rW_\x80\xFD[PZ\xF1\x15\x80\x15a\x19\x1FW=_\x80>=_\xFD[PPPPa\x19D\x86\x86\x86\x86`\x02\x89` \x01` \x81\x01\x90a\x19?\x91\x90a=aV[a\"\xC9V[PPPPPPV[_a\x19Ua\x04\xD7V[\x7F\xC9\x94\xD2\xCA\x03u\xD6\xD4sx^\x0C\xE0\xB1\xD2\x03\xF0i\x12\x1B\xAC\x13\x14\xF7,\\\x0F\xE6\x01\xEB9\x10a\x19\x83`@\x85\x01\x85a=\x7FV[`@Q` \x01a\x19\x94\x92\x91\x90a?\x97V[`@\x80Q\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x81\x84\x03\x01\x81R\x91\x90R\x80Q` \x90\x91\x01 ``\x85\x015a\x19\xDF`\xA0\x87\x01`\x80\x88\x01a<\x16V[a\x19\xEF`\xC0\x88\x01`\xA0\x89\x01a<\x16V[`\xC0\x88\x015a\x1A\x05a\x01\0\x8A\x01`\xE0\x8B\x01a<\x16V[`@\x80Q` \x81\x01\x98\x90\x98R\x87\x01\x95\x90\x95R``\x86\x01\x93\x90\x93R`\x01`\x01`\xA0\x1B\x03\x91\x82\x16`\x80\x86\x01R\x81\x16`\xA0\x85\x01R`\xC0\x84\x01\x91\x90\x91R\x16`\xE0\x82\x01Ra\x01\0\x80\x85\x015\x90\x82\x01Ra\x01 \x01`@Q` \x81\x83\x03\x03\x81R\x90`@Ra\x1Ao\x84a\x01 \x01a\x0C0V[a\x1A|\x85a\x01\xC0\x01a\x05\xC6V[`@Q` \x01a\x14\x92\x93\x92\x91\x90a?\xEBV[a\x01\xA0\x81\x015a\x1A\xA8a\x01\x80\x83\x015a\x01`\x84\x015a>\xBCV[a\x1A\xB2\x91\x90a>\xBCV[a\x01@\x82\x015\x14a\x1A\xEFW`@Q\x7F\xC0Cw\xD3\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\x02@\x81\x015a\x1B\ta\x02 \x83\x015a\x02\0\x84\x015a>\xBCV[a\x1B\x13\x91\x90a>\xBCV[a\x01\xE0\x82\x015\x14a\x0C\x89W`@Q\x7F\x87v0\xBE\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[_\x80a\x1B\x91\x85\x85\x85\x80\x80`\x1F\x01` \x80\x91\x04\x02` \x01`@Q\x90\x81\x01`@R\x80\x93\x92\x91\x90\x81\x81R` \x01\x83\x83\x80\x82\x847_\x92\x01\x91\x90\x91RPa.\xD6\x92PPPV[\x90P`\x01`\x01`\xA0\x1B\x03\x81\x16a\x1B\xD3W`@Q\x7F\x81^\x1Dd\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x94\x93PPPPV[`\x02`\x01T\x03a\x1C\x17W`@Q\x7F>\xE5\xAE\xB5\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[`\x02`\x01UV[_a\x1CKa\x1C+\x83a/\0V[\x80\x15a\x1CFWP_\x84\x80a\x1CAWa\x1CAa@\x08V[\x86\x88\t\x11[\x15\x15\x90V[a\x1CV\x86\x86\x86a/,V[a\x1C`\x91\x90a>\xBCV[\x95\x94PPPPPV[_a\x1Cs\x87a\x19LV[\x90Pa\x1C\x80\x89\x82\x85a\x10gV[a\x1C\x90`\xC0\x88\x01`\xA0\x89\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x163`\x01`\x01`\xA0\x1B\x03\x16\x14a\x1C\xC7Wa\x1C\xC2a\x1C\xBB`\xC0\x89\x01`\xA0\x8A\x01a<\x16V[\x82\x84a\x10gV[a\x1D\x0EV[a\x1C\xD4`@\x83\x01\x83a=\x7FV[\x90P_\x03a\x1D\x0EW`@Q\x7F\x0E6N\xFC\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\x1D+a\x1D!`\xC0\x89\x01`\xA0\x8A\x01a<\x16V[\x88``\x015a&\x1EV[\x86`\xC0\x015B\x11\x15a\x1DiW`@Q\x7F\x13=\xF0)\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[_\x88\x11\x80\x15a\x1D|WP\x86a\x01\0\x015\x88\x10[\x15a\x1D\xB3W`@Q\x7F\x94itD\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\x1D\xBC\x87a\x1A\x8EV[a\x1D\xC6\x8A\x85a\t\xE8V[a\x1D\xE7\x8A\x8A_\x8B\x11\x80\x15a\x1D\xDFWPa\x02\0\x8A\x015\x8B\x14\x15[\x8A\x8A\x8Aa\x0C\x8CV[a\x1D\xF5\x89\x88``\x015a&\x1EV[PPPPPPPPPPV[a\x1E\x13a\x1E\x0E\x86\x80a=\xEFV[a0\x01V[_a\x1E(a\x01\x80\x8B\x015a\x01\xA0\x8C\x015a>\xBCV[\x90P\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\x01`\x01`\xA0\x1B\x03\x16c\xB5\x19\xD3i`@Q\x80`\xA0\x01`@R\x80\x8D`\xA0\x01` \x81\x01\x90a\x1Ew\x91\x90a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x010`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01\x8Da\x01 \x01_\x01` \x81\x01\x90a\x1E\xAA\x91\x90a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01\x84\x81R` \x01\x86`\x02\x81\x11\x15a\x1E\xD0Wa\x1E\xD0a>\xF4V[\x81RP`@Q\x82c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01a\x1E\xEF\x91\x90a@5V[_`@Q\x80\x83\x03\x81_\x87\x80;\x15\x80\x15a\x1F\x06W_\x80\xFD[PZ\xF1\x15\x80\x15a\x1F\x18W=_\x80>=_\xFD[PPPP\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\x01`\x01`\xA0\x1B\x03\x16c\xB5\x19\xD3i`@Q\x80`\xA0\x01`@R\x80\x8D`\xA0\x01` \x81\x01\x90a\x1Fi\x91\x90a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01\x8D`\xE0\x01` \x81\x01\x90a\x1F\x8A\x91\x90a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01\x8Da\x01 \x01_\x01` \x81\x01\x90a\x1F\xAE\x91\x90a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01\x8A\x81R` \x01\x86`\x02\x81\x11\x15a\x1F\xD4Wa\x1F\xD4a>\xF4V[\x81RP`@Q\x82c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01a\x1F\xF3\x91\x90a@5V[_`@Q\x80\x83\x03\x81_\x87\x80;\x15\x80\x15a \nW_\x80\xFD[PZ\xF1\x15\x80\x15a \x1CW=_\x80>=_\xFD[PPPPa *\x85\x85a0\x01V[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\x01`\x01`\xA0\x1B\x03\x16c\xB5\x19\xD3i`@Q\x80`\xA0\x01`@R\x80\x8C`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01\x8D`\x80\x01` \x81\x01\x90a \x86\x91\x90a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01\x8Da\x01\xC0\x01_\x01` \x81\x01\x90a \xAA\x91\x90a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01\x8B\x81R` \x01\x85`\x02\x81\x11\x15a \xD0Wa \xD0a>\xF4V[\x81RP`@Q\x82c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01a \xEF\x91\x90a@5V[_`@Q\x80\x83\x03\x81_\x87\x80;\x15\x80\x15a!\x06W_\x80\xFD[PZ\xF1\x15\x80\x15a!\x18W=_\x80>=_\xFD[Pa!.\x92Pa\x1E\x0E\x91PP` \x88\x01\x88a=\xEFV[_a!Aa\x01@\x8C\x01a\x01 \x8D\x01a<\x16V[`@Q\x7Fp\xA0\x821\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R0`\x04\x82\x01R`\x01`\x01`\xA0\x1B\x03\x91\x90\x91\x16\x90cp\xA0\x821\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a!\x9EW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a!\xC2\x91\x90a@\xB4V[\x90P\x80\x15a\"\x05Wa\"\x05a!\xDEa\x01\0\x8D\x01`\xE0\x8E\x01a<\x16V[\x82\x8Da\x01 \x01_\x01` \x81\x01\x90a!\xF5\x91\x90a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x91\x90a19V[PPPPPPPPPPPV[a\"\"`\x80\x83\x01``\x84\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x163`\x01`\x01`\xA0\x1B\x03\x16\x14a\"aWa\"\\a\"M`\x80\x84\x01``\x85\x01a<\x16V[a\"V\x84a\x12\xEBV[\x83a\x10gV[a\"\xA8V[a\"n`@\x82\x01\x82a=\x7FV[\x90P_\x03a\"\xA8W`@Q\x7F\x0E6N\xFC\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\"\xC5a\"\xBB`\x80\x84\x01``\x85\x01a<\x16V[\x83` \x015a&\x1EV[PPV[`\xE0\x85\x015\x83\x15\x80\x15\x90a\"\xE0WP\x85`\xC0\x015\x84\x10[\x15a\"\xFDWa\"\xFA\x84\x87`\xE0\x015\x88`\xC0\x015`\x01a\x1C\x1EV[\x90P[a#\t\x87\x82\x88\x88a1\xB9V[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\x01`\x01`\xA0\x1B\x03\x16c\xB5\x19\xD3i`@Q\x80`\xA0\x01`@R\x80\x89``\x01` \x81\x01\x90a#V\x91\x90a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01a#va\x01`\x8B\x01a\x01@\x8C\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01a#\x94`\xA0\x8B\x01`\x80\x8C\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01\x87\x15\x80a#\xB2WP\x89`\xC0\x015\x88\x11[a#\xBCW\x87a#\xC2V[\x89`\xC0\x015[\x81R` \x01\x86`\x02\x81\x11\x15a#\xD9Wa#\xD9a>\xF4V[\x81RP`@Q\x82c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01a#\xF8\x91\x90a@5V[_`@Q\x80\x83\x03\x81_\x87\x80;\x15\x80\x15a$\x0FW_\x80\xFD[PZ\xF1\x15\x80\x15a$!W=_\x80>=_\xFD[PPPP\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\x01`\x01`\xA0\x1B\x03\x16c\xB5\x19\xD3i`@Q\x80`\xA0\x01`@R\x80\x8A`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01\x89`@\x01` \x81\x01\x90a$\x81\x91\x90a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01a$\x9F`\xC0\x8B\x01`\xA0\x8C\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01\x84\x81R` \x01\x85`\x02\x81\x11\x15a$\xC5Wa$\xC5a>\xF4V[\x81RP`@Q\x82c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01a$\xE4\x91\x90a@5V[_`@Q\x80\x83\x03\x81_\x87\x80;\x15\x80\x15a$\xFBW_\x80\xFD[PZ\xF1\x15\x80\x15a%\rW=_\x80>=_\xFD[Pa%\x1E\x92P\x88\x91P\x81\x90Pa=\x7FV[`@Qa%,\x92\x91\x90a=\xE0V[`@Q\x80\x91\x03\x90 \x7F\x0F\xCE\0|8\xC6\xC8\xED\x9ET[:\x14\x80\x95v'8a\x8F\x8C!\xB6s\"&\x13\xE4\xD4W4\xB6\x87`@\x01` \x81\x01\x90a%h\x91\x90a<\x16V[a%x`\x80\x8A\x01``\x8B\x01a<\x16V[a%\x88`\xA0\x8B\x01`\x80\x8C\x01a<\x16V[a%\x98`\xC0\x8C\x01`\xA0\x8D\x01a<\x16V[\x89\x15\x80a%\xA8WP\x8B`\xC0\x015\x8A\x11[a%\xB2W\x89a%\xB8V[\x8B`\xC0\x015[\x87\x8Da\x01@\x01` \x81\x01\x90a%\xCD\x91\x90a<\x16V[`@\x80Q`\x01`\x01`\xA0\x1B\x03\x98\x89\x16\x81R\x96\x88\x16` \x88\x01R\x94\x87\x16\x94\x86\x01\x94\x90\x94R\x91\x85\x16``\x85\x01R`\x80\x84\x01R`\xA0\x83\x01R\x90\x91\x16`\xC0\x82\x01R`\xE0\x01`@Q\x80\x91\x03\x90\xA2PPPPPPPV[`\x01`\x01`\xA0\x1B\x03\x82\x16_\x90\x81R` \x81\x81R`@\x80\x83 \x84\x84R\x90\x91R\x90 T`\xFF\x16\x15a&yW`@Q\x7F\xBC\r\xA7\xD6\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[`\x01`\x01`\xA0\x1B\x03\x90\x91\x16_\x90\x81R` \x81\x81R`@\x80\x83 \x93\x83R\x92\x90R \x80T\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\0\x16`\x01\x17\x90UV[_\x80\x80\x80a&\xD5\x87`\x04\x81\x8Ba@\xCBV[\x81\x01\x90a&\xE2\x91\x90a@\xF2V[P\x91\x94P\x92P\x90Pa&\xFCa\x01@\x86\x01a\x01 \x87\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x83`\x01`\x01`\xA0\x1B\x03\x16\x14a'FW`@Q\x7F\xC8\x91\xAD\xD2\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x81`\x01`\x01`\xA0\x1B\x03\x16\x86`\x01`\x01`\xA0\x1B\x03\x16\x14a'\x91W`@Q\x7F\x81^\x1Dd\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\x01\xA0\x85\x015\x81\x14a'\xCFW`@Q\x7F,R\x11\xC6\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x97\x96PPPPPPPV[_\x80\x80\x80\x80a'\xEC\x88`\x04\x81\x8Ca@\xCBV[\x81\x01\x90a'\xF9\x91\x90aABV[\x92\x96P\x90\x94P\x92P\x90Pa(\x15a\x01\xE0\x87\x01a\x01\xC0\x88\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x84`\x01`\x01`\xA0\x1B\x03\x16\x14a(_W`@Q\x7F\xC8\x91\xAD\xD2\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x82`\x01`\x01`\xA0\x1B\x03\x16\x87`\x01`\x01`\xA0\x1B\x03\x16\x14a(\xAAW`@Q\x7F\x81^\x1Dd\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a(\xBA`\xA0\x87\x01`\x80\x88\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x82`\x01`\x01`\xA0\x1B\x03\x16\x14a)\x04W`@Q\x7F\xACk\x05\xF5\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\x02@\x86\x015\x81\x14a)BW`@Q\x7F,R\x11\xC6\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x98\x97PPPPPPPPV[_\x80_\x80_\x80_\x80_\x8C\x8C`\x04\x90\x80\x92a)j\x93\x92\x91\x90a@\xCBV[\x81\x01\x90a)w\x91\x90aA\x90V[\x95\x9CP\x93\x9AP\x91\x98P\x96P\x94P\x92P\x90Pa)\x9Aa\x01\xE0\x8B\x01a\x01\xC0\x8C\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x87`\x01`\x01`\xA0\x1B\x03\x16\x14a)\xE4W`@Q\x7F\xC8\x91\xAD\xD2\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x85`\x01`\x01`\xA0\x1B\x03\x16\x8B`\x01`\x01`\xA0\x1B\x03\x16\x14a*/W`@Q\x7F\x81^\x1Dd\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[`\x01`\x01`\xA0\x1B\x03\x85\x160\x14a*qW`@Q\x7F\x81T7K\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a*\x81`\xA0\x8B\x01`\x80\x8C\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x84`\x01`\x01`\xA0\x1B\x03\x16\x14a*\xCBW`@Q\x7F\xACk\x05\xF5\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\x02@\x8A\x015\x83\x14a+\tW`@Q\x7F,R\x11\xC6\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a+\x1Ba\x01@\x8B\x01a\x01 \x8C\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x82`\x01`\x01`\xA0\x1B\x03\x16\x14a+eW`@Q\x7F\xC8\x91\xAD\xD2\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\x01\xA0\x8A\x015\x81\x14a+\xA3W`@Q\x7F,R\x11\xC6\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x91\x9C\x91\x9BP\x90\x99PPPPPPPPPPV[_\x80\x80\x80a+\xC7\x87`\x04\x81\x8Ba@\xCBV[\x81\x01\x90a+\xD4\x91\x90aB\x0FV[\x91\x94P\x92P\x90Pa+\xEDa\x01@\x86\x01a\x01 \x87\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x83`\x01`\x01`\xA0\x1B\x03\x16\x14a,7W`@Q\x7F\xC8\x91\xAD\xD2\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x81`\x01`\x01`\xA0\x1B\x03\x16\x86`\x01`\x01`\xA0\x1B\x03\x16\x14a,\x82W`@Q\x7F\x81^\x1Dd\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\x01\x80\x85\x015\x81\x14a'\xCFW`@Q\x7F,R\x11\xC6\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[_\x80\x80\x80\x80a,\xD2\x88`\x04\x81\x8Ca@\xCBV[\x81\x01\x90a,\xDF\x91\x90aBMV[P\x92\x96P\x90\x94P\x92P\x90Pa,\xFCa\x01\xE0\x87\x01a\x01\xC0\x88\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x84`\x01`\x01`\xA0\x1B\x03\x16\x14a-FW`@Q\x7F\xC8\x91\xAD\xD2\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x82`\x01`\x01`\xA0\x1B\x03\x16\x87`\x01`\x01`\xA0\x1B\x03\x16\x14a-\x91W`@Q\x7F\x81^\x1Dd\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a-\xA1`\xA0\x87\x01`\x80\x88\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x82`\x01`\x01`\xA0\x1B\x03\x16\x14a-\xEBW`@Q\x7F\xACk\x05\xF5\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\x02 \x86\x015\x81\x14a)BW`@Q\x7F,R\x11\xC6\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x80Qa\x01\x80\x83\x015\x14\x15\x80a.GWP` \x81\x01Qa\x01\xA0\x83\x015\x14\x15[\x15a.~W`@Q\x7FJU\xDA \0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[`@\x81\x01Qa\x02 \x83\x015\x14\x15\x80a.\x9FWP``\x81\x01Qa\x02@\x83\x015\x14\x15[\x15a\"\xC5W`@Q\x7Fw\xA5\x92\x03\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[_\x80_\x80a.\xE4\x86\x86a2\\V[\x92P\x92P\x92Pa.\xF4\x82\x82a2\xA5V[P\x90\x91PP[\x92\x91PPV[_`\x02\x82`\x03\x81\x11\x15a/\x15Wa/\x15a>\xF4V[a/\x1F\x91\x90aB\xB1V[`\xFF\x16`\x01\x14\x90P\x91\x90PV[_\x83\x83\x02\x81\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x85\x87\t\x82\x81\x10\x83\x82\x03\x03\x91PP\x80_\x03a/\x7FW\x83\x82\x81a/uWa/ua@\x08V[\x04\x92PPPa\x04\xD0V[\x80\x84\x11a/\x96Wa/\x96`\x03\x85\x15\x02`\x11\x18a3\xADV[_\x84\x86\x88\t_\x86\x81\x03\x87\x16\x96\x87\x90\x04\x96`\x02`\x03\x89\x02\x81\x18\x80\x8A\x02\x82\x03\x02\x80\x8A\x02\x82\x03\x02\x80\x8A\x02\x82\x03\x02\x80\x8A\x02\x82\x03\x02\x80\x8A\x02\x82\x03\x02\x80\x8A\x02\x90\x91\x03\x02\x91\x81\x90\x03\x81\x90\x04`\x01\x01\x86\x84\x11\x90\x95\x03\x94\x90\x94\x02\x91\x90\x94\x03\x92\x90\x92\x04\x91\x90\x91\x17\x91\x90\x91\x02\x91PP\x93\x92PPPV[_[\x81\x81\x10\x15a\x0C+W6\x83\x83\x83\x81\x81\x10a0\x1EWa0\x1Ea>SV[\x90P` \x02\x81\x01\x90a00\x91\x90a>\x80V[\x90P`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16a0i` \x83\x01\x83a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x03a0\xA9W`@Q\x7Fy\xA1\xBF\xF0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a0\xB2\x81a3\xBEV[a0\xBF` \x82\x01\x82a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x7F\xED\x99\x82~\xFB7\x01o\"u\xF9\x8CK\xCFq\xC7U\x1Cu\xD5\x9E\x9BE\x0Fy\xFA2\xE6\x0B\xE6r\xC2\x82` \x015a0\xF7\x84a4\x01V[`@\x80Q\x92\x83R\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90\x91\x16` \x83\x01R\x01`@Q\x80\x91\x03\x90\xA2P`\x01\x01a0\x03V[`@\x80Q`\x01`\x01`\xA0\x1B\x03\x84\x16`$\x82\x01R`D\x80\x82\x01\x84\x90R\x82Q\x80\x83\x03\x90\x91\x01\x81R`d\x90\x91\x01\x90\x91R` \x81\x01\x80Q{\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x7F\xA9\x05\x9C\xBB\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x17\x90Ra\x0C+\x90\x84\x90a4*V[_\x83\x11\x80\x15a1\xCCWP\x81a\x01\0\x015\x83\x10[\x15a2\x03W`@Q\x7F\x94itD\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a2\x10\x84a\"V\x84a\x12\xEBV[a2\x1E\x84\x83` \x015a&\x1EV[B\x82a\x01 \x015\x11a\x11\x9AW`@Q\x7F\xC5hs\xBA\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[_\x80_\x83Q`A\x03a2\x93W` \x84\x01Q`@\x85\x01Q``\x86\x01Q_\x1Aa2\x85\x88\x82\x85\x85a4\xAFV[\x95P\x95P\x95PPPPa2\x9EV[PP\x81Q_\x91P`\x02\x90[\x92P\x92P\x92V[_\x82`\x03\x81\x11\x15a2\xB8Wa2\xB8a>\xF4V[\x03a2\xC1WPPV[`\x01\x82`\x03\x81\x11\x15a2\xD5Wa2\xD5a>\xF4V[\x03a3\x0CW`@Q\x7F\xF6E\xEE\xDF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[`\x02\x82`\x03\x81\x11\x15a3 Wa3 a>\xF4V[\x03a3_W`@Q\x7F\xFC\xE6\x98\xF7\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x81\x01\x82\x90R`$\x01[`@Q\x80\x91\x03\x90\xFD[`\x03\x82`\x03\x81\x11\x15a3sWa3sa>\xF4V[\x03a\"\xC5W`@Q\x7F\xD7\x8B\xCE\x0C\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x81\x01\x82\x90R`$\x01a3VV[cNH{q_R\x80` R`$`\x1C\xFD[_a3\xCC` \x83\x01\x83a<\x16V[\x90P` \x82\x0156_a3\xE2`@\x86\x01\x86a=\x7FV[\x91P\x91P`@Q\x81\x83\x827_\x80\x83\x83\x87\x89Z\xF1a\x19DW=_\x80>=_\xFD[_6\x81a4\x11`@\x85\x01\x85a=\x7FV[\x90\x92P\x90P`\x04\x81\x10a4#W\x815\x92P[PP\x91\x90PV[_\x80` _\x84Q` \x86\x01_\x88Z\xF1\x80a4IW`@Q=_\x82>=\x81\xFD[PP_Q=\x91P\x81\x15a4`W\x80`\x01\x14\x15a4mV[`\x01`\x01`\xA0\x1B\x03\x84\x16;\x15[\x15a\x11\x9AW`@Q\x7FRt\xAF\xE7\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x85\x16`\x04\x82\x01R`$\x01a3VV[_\x80\x80\x7F\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF]WnsW\xA4P\x1D\xDF\xE9/Fh\x1B \xA0\x84\x11\x15a4\xE8WP_\x91P`\x03\x90P\x82a5\x8BV[`@\x80Q_\x80\x82R` \x82\x01\x80\x84R\x8A\x90R`\xFF\x89\x16\x92\x82\x01\x92\x90\x92R``\x81\x01\x87\x90R`\x80\x81\x01\x86\x90R`\x01\x90`\xA0\x01` `@Q` \x81\x03\x90\x80\x84\x03\x90\x85Z\xFA\x15\x80\x15a59W=_\x80>=_\xFD[PP`@Q\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x01Q\x91PP`\x01`\x01`\xA0\x1B\x03\x81\x16a5\x82WP_\x92P`\x01\x91P\x82\x90Pa5\x8BV[\x92P_\x91P\x81\x90P[\x94P\x94P\x94\x91PPV[_\x80_`@\x84\x86\x03\x12\x15a5\xA7W_\x80\xFD[\x835\x92P` \x84\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a5\xC5W_\x80\xFD[\x81\x86\x01\x91P\x86`\x1F\x83\x01\x12a5\xD8W_\x80\xFD[\x815\x81\x81\x11\x15a5\xE6W_\x80\xFD[\x87` \x82\x85\x01\x01\x11\x15a5\xF7W_\x80\xFD[` \x83\x01\x94P\x80\x93PPPP\x92P\x92P\x92V[_`\xA0\x82\x84\x03\x12\x15a6\x1AW_\x80\xFD[P\x91\x90PV[_`\xA0\x82\x84\x03\x12\x15a60W_\x80\xFD[a\x04\xD0\x83\x83a6\nV[`\x01`\x01`\xA0\x1B\x03\x81\x16\x81\x14a\x0C\x89W_\x80\xFD[\x805a6Y\x81a6:V[\x91\x90PV[_a\x02`\x82\x84\x03\x12\x15a6\x1AW_\x80\xFD[_\x80\x83`\x1F\x84\x01\x12a6\x7FW_\x80\xFD[P\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a6\x96W_\x80\xFD[` \x83\x01\x91P\x83` \x82`\x05\x1B\x85\x01\x01\x11\x15a6\xB0W_\x80\xFD[\x92P\x92\x90PV[_`@\x82\x84\x03\x12\x15a6\x1AW_\x80\xFD[_``\x82\x84\x03\x12\x15a6\x1AW_\x80\xFD[_\x80_\x80_\x80_\x80_a\x01\0\x8A\x8C\x03\x12\x15a6\xF0W_\x80\xFD[a6\xF9\x8Aa6NV[\x98P` \x8A\x015\x97P`@\x8A\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a7\x1CW_\x80\xFD[a7(\x8D\x83\x8E\x01a6^V[\x98P``\x8C\x015\x91P\x80\x82\x11\x15a7=W_\x80\xFD[a7I\x8D\x83\x8E\x01a6oV[\x90\x98P\x96P`\x80\x8C\x015\x91P\x80\x82\x11\x15a7aW_\x80\xFD[a7m\x8D\x83\x8E\x01a6\xB7V[\x95P`\xA0\x8C\x015\x91P\x80\x82\x11\x15a7\x82W_\x80\xFD[a7\x8E\x8D\x83\x8E\x01a6\xC7V[\x94P`\xC0\x8C\x015\x91P\x80\x82\x11\x15a7\xA3W_\x80\xFD[a7\xAF\x8D\x83\x8E\x01a6\xC7V[\x93P`\xE0\x8C\x015\x91P\x80\x82\x11\x15a7\xC4W_\x80\xFD[Pa7\xD1\x8C\x82\x8D\x01a6\xC7V[\x91PP\x92\x95\x98P\x92\x95\x98P\x92\x95\x98V[_\x80`@\x83\x85\x03\x12\x15a7\xF2W_\x80\xFD[\x825a7\xFD\x81a6:V[\x91P` \x83\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a8\x18W_\x80\xFD[a8$\x85\x82\x86\x01a6\xB7V[\x91PP\x92P\x92\x90PV[_a\x01`\x82\x84\x03\x12\x15a6\x1AW_\x80\xFD[_\x80_\x80_`\xA0\x86\x88\x03\x12\x15a8SW_\x80\xFD[\x855a8^\x81a6:V[\x94P` \x86\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a8zW_\x80\xFD[a8\x86\x89\x83\x8A\x01a8.V[\x95P`@\x88\x015\x91P\x80\x82\x11\x15a8\x9BW_\x80\xFD[a8\xA7\x89\x83\x8A\x01a6\xC7V[\x94P``\x88\x015\x93P`\x80\x88\x015\x91P\x80\x82\x11\x15a8\xC3W_\x80\xFD[Pa8\xD0\x88\x82\x89\x01a6\xC7V[\x91PP\x92\x95P\x92\x95\x90\x93PV[_` \x82\x84\x03\x12\x15a8\xEDW_\x80\xFD[P5\x91\x90PV[\x80\x15\x15\x81\x14a\x0C\x89W_\x80\xFD[_\x80_\x80_\x80`\xA0\x87\x89\x03\x12\x15a9\x16W_\x80\xFD[\x865a9!\x81a6:V[\x95P` \x87\x015a91\x81a6:V[\x94P`@\x87\x015a9A\x81a8\xF4V[\x93P``\x87\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a9]W_\x80\xFD[a9i\x8A\x83\x8B\x01a6^V[\x94P`\x80\x89\x015\x91P\x80\x82\x11\x15a9~W_\x80\xFD[Pa9\x8B\x89\x82\x8A\x01a6oV[\x97\x9A\x96\x99P\x94\x97P\x92\x95\x93\x94\x92PPPV[_\x80_``\x84\x86\x03\x12\x15a9\xAFW_\x80\xFD[\x835a9\xBA\x81a6:V[\x92P` \x84\x015\x91P`@\x84\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a9\xDCW_\x80\xFD[a9\xE8\x86\x82\x87\x01a6\xC7V[\x91PP\x92P\x92P\x92V[_` \x82\x84\x03\x12\x15a:\x02W_\x80\xFD[\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a:\x18W_\x80\xFD[a\x1B\xD3\x84\x82\x85\x01a8.V[_\x80_\x80_\x80_\x80`\xE0\x89\x8B\x03\x12\x15a:;W_\x80\xFD[a:D\x89a6NV[\x97P` \x89\x015\x96P`@\x89\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a:gW_\x80\xFD[a:s\x8C\x83\x8D\x01a6^V[\x97P``\x8B\x015\x91P\x80\x82\x11\x15a:\x88W_\x80\xFD[a:\x94\x8C\x83\x8D\x01a6oV[\x90\x97P\x95P`\x80\x8B\x015\x91P\x80\x82\x11\x15a:\xACW_\x80\xFD[a:\xB8\x8C\x83\x8D\x01a6\xB7V[\x94P`\xA0\x8B\x015\x91P\x80\x82\x11\x15a:\xCDW_\x80\xFD[a:\xD9\x8C\x83\x8D\x01a6\xC7V[\x93P`\xC0\x8B\x015\x91P\x80\x82\x11\x15a:\xEEW_\x80\xFD[Pa:\xFB\x8B\x82\x8C\x01a6\xC7V[\x91PP\x92\x95\x98P\x92\x95\x98\x90\x93\x96PV[_\x80_\x80_\x80`\xC0\x87\x89\x03\x12\x15a; W_\x80\xFD[a;)\x87a6NV[\x95P` \x87\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a;EW_\x80\xFD[a;Q\x8A\x83\x8B\x01a8.V[\x96P`@\x89\x015\x91P\x80\x82\x11\x15a;fW_\x80\xFD[a;r\x8A\x83\x8B\x01a6\xC7V[\x95P``\x89\x015\x94P`\x80\x89\x015\x91P\x80\x82\x11\x15a;\x8EW_\x80\xFD[a;\x9A\x8A\x83\x8B\x01a6\xC7V[\x93P`\xA0\x89\x015\x91P\x80\x82\x11\x15a;\xAFW_\x80\xFD[Pa;\xBC\x89\x82\x8A\x01a6\xC7V[\x91PP\x92\x95P\x92\x95P\x92\x95V[_` \x82\x84\x03\x12\x15a;\xD9W_\x80\xFD[\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a;\xEFW_\x80\xFD[a\x1B\xD3\x84\x82\x85\x01a6^V[_` \x82\x84\x03\x12\x15a<\x0BW_\x80\xFD[\x81Qa\x04\xD0\x81a8\xF4V[_` \x82\x84\x03\x12\x15a<&W_\x80\xFD[\x815a\x04\xD0\x81a6:V[\x81\x83R\x81\x81` \x85\x017P_` \x82\x84\x01\x01R_` \x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`\x1F\x84\x01\x16\x84\x01\x01\x90P\x92\x91PPV[\x805e\xFF\xFF\xFF\xFF\xFF\xFF\x81\x16\x81\x14a6YW_\x80\xFD[_`\x01`\x01`\xA0\x1B\x03\x80\x87\x16\x83R\x80\x86\x16` \x84\x01RP\x83`@\x83\x01R`\x80``\x83\x01R\x825\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE1\x846\x03\x01\x81\x12a<\xE2W_\x80\xFD[\x83\x01` \x81\x01\x905g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a<\xFEW_\x80\xFD[\x806\x03\x82\x13\x15a=\x0CW_\x80\xFD[```\x80\x85\x01Ra=!`\xE0\x85\x01\x82\x84a<1V[\x91PPa=0` \x85\x01a<xV[e\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x16`\xA0\x86\x01R\x80a=L`@\x88\x01a<xV[\x16`\xC0\x86\x01RPP\x80\x91PP\x95\x94PPPPPV[_` \x82\x84\x03\x12\x15a=qW_\x80\xFD[\x815`\x03\x81\x10a\x04\xD0W_\x80\xFD[_\x80\x835\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE1\x846\x03\x01\x81\x12a=\xB2W_\x80\xFD[\x83\x01\x805\x91Pg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11\x15a=\xCCW_\x80\xFD[` \x01\x91P6\x81\x90\x03\x82\x13\x15a6\xB0W_\x80\xFD[\x81\x83\x827_\x91\x01\x90\x81R\x91\x90PV[_\x80\x835\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE1\x846\x03\x01\x81\x12a>\"W_\x80\xFD[\x83\x01\x805\x91Pg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11\x15a><W_\x80\xFD[` \x01\x91P`\x05\x81\x90\x1B6\x03\x82\x13\x15a6\xB0W_\x80\xFD[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`2`\x04R`$_\xFD[_\x825\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xA1\x836\x03\x01\x81\x12a>\xB2W_\x80\xFD[\x91\x90\x91\x01\x92\x91PPV[\x80\x82\x01\x80\x82\x11\x15a.\xFAW\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x11`\x04R`$_\xFD[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`!`\x04R`$_\xFD[_` \x82\x84\x03\x12\x15a?1W_\x80\xFD[\x815`\x04\x81\x10a\x04\xD0W_\x80\xFD[\x83\x81R`@` \x82\x01R_a\x1C``@\x83\x01\x84\x86a<1V[_` \x82\x84\x03\x12\x15a?hW_\x80\xFD[\x81Q\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81\x16\x81\x14a\x04\xD0W_\x80\xFD[` \x81R_a\x1B\xD3` \x83\x01\x84\x86a<1V[_\x81Q_[\x81\x81\x10\x15a?\xC9W` \x81\x85\x01\x81\x01Q\x86\x83\x01R\x01a?\xAFV[P_\x93\x01\x92\x83RP\x90\x91\x90PV[_a\x1B\xD3a?\xE5\x83\x86a?\xAAV[\x84a?\xAAV[_a?\xF6\x82\x86a?\xAAV[\x93\x84RPP` \x82\x01R`@\x01\x91\x90PV[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x12`\x04R`$_\xFD[_`\xA0\x82\x01\x90P`\x01`\x01`\xA0\x1B\x03\x80\x84Q\x16\x83R\x80` \x85\x01Q\x16` \x84\x01R\x80`@\x85\x01Q\x16`@\x84\x01RP``\x83\x01Q``\x83\x01R`\x80\x83\x01Q`\x03\x81\x10a@\xA7W\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`!`\x04R`$_\xFD[\x80`\x80\x84\x01RP\x92\x91PPV[_` \x82\x84\x03\x12\x15a@\xC4W_\x80\xFD[PQ\x91\x90PV[_\x80\x85\x85\x11\x15a@\xD9W_\x80\xFD[\x83\x86\x11\x15a@\xE5W_\x80\xFD[PP\x82\x01\x93\x91\x90\x92\x03\x91PV[_\x80_\x80`\x80\x85\x87\x03\x12\x15aA\x05W_\x80\xFD[\x845aA\x10\x81a6:V[\x93P` \x85\x015aA \x81a6:V[\x92P`@\x85\x015\x91P``\x85\x015aA7\x81a8\xF4V[\x93\x96\x92\x95P\x90\x93PPV[_\x80_\x80`\x80\x85\x87\x03\x12\x15aAUW_\x80\xFD[\x845aA`\x81a6:V[\x93P` \x85\x015aAp\x81a6:V[\x92P`@\x85\x015aA\x80\x81a6:V[\x93\x96\x92\x95P\x92\x93``\x015\x92PPV[_\x80_\x80_\x80_`\xE0\x88\x8A\x03\x12\x15aA\xA6W_\x80\xFD[\x875aA\xB1\x81a6:V[\x96P` \x88\x015aA\xC1\x81a6:V[\x95P`@\x88\x015aA\xD1\x81a6:V[\x94P``\x88\x015aA\xE1\x81a6:V[\x93P`\x80\x88\x015\x92P`\xA0\x88\x015aA\xF8\x81a6:V[\x80\x92PP`\xC0\x88\x015\x90P\x92\x95\x98\x91\x94\x97P\x92\x95PV[_\x80_``\x84\x86\x03\x12\x15aB!W_\x80\xFD[\x835aB,\x81a6:V[\x92P` \x84\x015aB<\x81a6:V[\x92\x95\x92\x94PPP`@\x91\x90\x91\x015\x90V[_\x80_\x80_`\xA0\x86\x88\x03\x12\x15aBaW_\x80\xFD[\x855aBl\x81a6:V[\x94P` \x86\x015aB|\x81a6:V[\x93P`@\x86\x015aB\x8C\x81a6:V[\x92P``\x86\x015\x91P`\x80\x86\x015aB\xA3\x81a8\xF4V[\x80\x91PP\x92\x95P\x92\x95\x90\x93PV[_`\xFF\x83\x16\x80aB\xE8W\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x12`\x04R`$_\xFD[\x80`\xFF\x84\x16\x06\x91PP\x92\x91PPV\xFE\xA2dipfsX\"\x12 \x9B\xE5\x8A\xCA\xDA50a\xA2\xA2\x02\xCCp\x11\xF3\xC9\x13\x93\xE0\xF2\xE3\x05\xE9 $E\xB0B\xFCJL\xE6dsolcC\0\x08\x17\x003`\xC0`@R4\x80\x15a\0\x0FW_\x80\xFD[P`@Qa\tc8\x03\x80a\tc\x839\x81\x01`@\x81\x90Ra\0.\x91a\0`V[`\x01`\x01`\xA0\x1B\x03\x91\x82\x16`\x80R\x16`\xA0Ra\0\x91V[\x80Q`\x01`\x01`\xA0\x1B\x03\x81\x16\x81\x14a\0[W_\x80\xFD[\x91\x90PV[_\x80`@\x83\x85\x03\x12\x15a\0qW_\x80\xFD[a\0z\x83a\0EV[\x91Pa\0\x88` \x84\x01a\0EV[\x90P\x92P\x92\x90PV[`\x80Q`\xA0Qa\x08\x9Ea\0\xC5_9_\x81\x81`H\x01R\x81\x81a\x01\xF2\x01Ra\x03\x81\x01R_\x81\x81`\xD3\x01Ra\x03'\x01Ra\x08\x9E_\xF3\xFE`\x80`@R4\x80\x15a\0\x0FW_\x80\xFD[P`\x046\x10a\0?W_5`\xE0\x1C\x80cj\xFD\xD8P\x14a\0CW\x80c\xB5\x19\xD3i\x14a\0\x93W\x80c\xBC\x11x\xE6\x14a\0\xA8W[_\x80\xFD[a\0j\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[`@Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x90\x91\x16\x81R` \x01`@Q\x80\x91\x03\x90\xF3[a\0\xA6a\0\xA16`\x04a\x06\nV[a\0\xBBV[\0[a\0\xA6a\0\xB66`\x04a\x06HV[a\x03\x0FV[3s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x81\x14a\x01+W`@Q\x7F|!O\x04\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[``\x82\x015\x15a\x01\xAFW`\x01a\x01G`\xA0\x84\x01`\x80\x85\x01a\x06\xDDV[`\x02\x81\x11\x15a\x01XWa\x01Xa\x06\xB0V[\x03a\x01\xB3Wa\x01\xAFa\x01m` \x84\x01\x84a\x07\x02V[a\x01}`@\x85\x01` \x86\x01a\x07\x02V[``\x85\x01\x805\x90a\x01\x91\x90`@\x88\x01a\x07\x02V[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x92\x91\x90a\x04\xCCV[PPV[`\x02a\x01\xC5`\xA0\x84\x01`\x80\x85\x01a\x06\xDDV[`\x02\x81\x11\x15a\x01\xD6Wa\x01\xD6a\x06\xB0V[\x03a\x02\xDDWs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16c6\xC7\x85\x16a\x02$` \x85\x01\x85a\x07\x02V[a\x024`@\x86\x01` \x87\x01a\x07\x02V[``\x86\x01\x805\x90a\x02H\x90`@\x89\x01a\x07\x02V[`@Q`\xE0\x86\x90\x1B\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x81Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x94\x85\x16`\x04\x82\x01R\x92\x84\x16`$\x84\x01R\x90\x83\x16`D\x83\x01R\x90\x91\x16`d\x82\x01R`\x84\x01_`@Q\x80\x83\x03\x81_\x87\x80;\x15\x80\x15a\x02\xC3W_\x80\xFD[PZ\xF1\x15\x80\x15a\x02\xD5W=_\x80>=_\xFD[PPPPPPV[`@Q\x7F\xC7\x9A\xAAD\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[3s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x81\x14a\x03\x7FW`@Q\x7F|!O\x04\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16c+g\xB5p\x86`@Q\x80``\x01`@R\x80`@Q\x80`\x80\x01`@R\x80\x8As\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x89s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x88`@\x01` \x81\x01\x90a\x04\x1D\x91\x90a\x07\x1BV[e\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x88` \x01` \x81\x01\x90a\x04=\x91\x90a\x07\x1BV[e\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90R\x81R0` \x82\x01R`@\x90\x81\x01\x90a\x04e\x90``\x89\x01\x90\x89\x01a\x07\x1BV[e\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90Ra\x04y\x86\x80a\x07@V[`@Q\x85c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01a\x04\x98\x94\x93\x92\x91\x90a\x07\xA8V[_`@Q\x80\x83\x03\x81_\x87\x80;\x15\x80\x15a\x04\xAFW_\x80\xFD[PZ\xF1\x15\x80\x15a\x04\xC1W=_\x80>=_\xFD[PPPPPPPPPV[`@\x80Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x85\x81\x16`$\x83\x01R\x84\x16`D\x82\x01R`d\x80\x82\x01\x84\x90R\x82Q\x80\x83\x03\x90\x91\x01\x81R`\x84\x90\x91\x01\x90\x91R` \x81\x01\x80Q{\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x7F#\xB8r\xDD\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x17\x90Ra\x05a\x90\x85\x90a\x05gV[PPPPV[_\x80` _\x84Q` \x86\x01_\x88Z\xF1\x80a\x05\x86W`@Q=_\x82>=\x81\xFD[PP_Q=\x91P\x81\x15a\x05\x9DW\x80`\x01\x14\x15a\x05\xB7V[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x84\x16;\x15[\x15a\x05aW`@Q\x7FRt\xAF\xE7\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x85\x16`\x04\x82\x01R`$\x01`@Q\x80\x91\x03\x90\xFD[_`\xA0\x82\x84\x03\x12\x15a\x06\x1AW_\x80\xFD[P\x91\x90PV[\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x16\x81\x14a\x06CW_\x80\xFD[\x91\x90PV[_\x80_\x80`\x80\x85\x87\x03\x12\x15a\x06[W_\x80\xFD[a\x06d\x85a\x06 V[\x93Pa\x06r` \x86\x01a\x06 V[\x92P`@\x85\x015\x91P``\x85\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x06\x94W_\x80\xFD[\x85\x01``\x81\x88\x03\x12\x15a\x06\xA5W_\x80\xFD[\x93\x96\x92\x95P\x90\x93PPV[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`!`\x04R`$_\xFD[_` \x82\x84\x03\x12\x15a\x06\xEDW_\x80\xFD[\x815`\x03\x81\x10a\x06\xFBW_\x80\xFD[\x93\x92PPPV[_` \x82\x84\x03\x12\x15a\x07\x12W_\x80\xFD[a\x06\xFB\x82a\x06 V[_` \x82\x84\x03\x12\x15a\x07+W_\x80\xFD[\x815e\xFF\xFF\xFF\xFF\xFF\xFF\x81\x16\x81\x14a\x06\xFBW_\x80\xFD[_\x80\x835\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE1\x846\x03\x01\x81\x12a\x07sW_\x80\xFD[\x83\x01\x805\x91Pg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11\x15a\x07\x8DW_\x80\xFD[` \x01\x91P6\x81\x90\x03\x82\x13\x15a\x07\xA1W_\x80\xFD[\x92P\x92\x90PV[_a\x01\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x88\x16\x84R\x86Q\x81\x81Q\x16` \x86\x01R\x81` \x82\x01Q\x16`@\x86\x01R`@\x81\x01Qe\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x16``\x88\x01R\x80``\x84\x01Q\x16`\x80\x88\x01RPPP\x80` \x88\x01Q\x16`\xA0\x85\x01RP`@\x86\x01Q`\xC0\x84\x01R\x80`\xE0\x84\x01R\x83\x81\x84\x01RPa\x01 \x83\x85\x82\x85\x017_\x83\x85\x01\x82\x01R`\x1F\x90\x93\x01\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x16\x90\x91\x01\x90\x91\x01\x94\x93PPPPV\xFE\xA2dipfsX\"\x12 \xF3V_e\x89P\x02v\xFC\xBBo\xB3=>\xE3\xB54\xD9Vl['2\xFE\x10\xA6\x03\x9Bu<:\x07dsolcC\0\x08\x17\x003",
    );
    /// The runtime bytecode of the contract, as deployed on the network.
    ///
    /// ```text
    ///0x608060405260043610610126575f3560e01c8063a5cdc8fc116100a1578063c618618111610071578063db58772811610057578063db58772814610379578063e242924e1461038c578063fa5cd56c146103ab575f80fd5b8063c618618114610327578063cba673a71461035a575f80fd5b8063a5cdc8fc146102ab578063a7ab49bc146102ca578063ae80c584146102e9578063b11f126214610308575f80fd5b806351d46815116100f65780636f35d2d2116100dc5780636f35d2d214610246578063875530ff146102795780639935c86814610298575f80fd5b806351d46815146102125780635aa0e95d14610227575f80fd5b80631626ba7e1461013157806329bcdc95146101865780633644e515146101d15780634c9e03d3146101f3575f80fd5b3661012d57005b5f80fd5b34801561013c575f80fd5b5061015061014b366004613595565b6103ca565b6040517fffffffff0000000000000000000000000000000000000000000000000000000090911681526020015b60405180910390f35b348015610191575f80fd5b506101b97f000000000000000000000000000000000000000000000000000000000000000081565b6040516001600160a01b03909116815260200161017d565b3480156101dc575f80fd5b506101e56104d7565b60405190815260200161017d565b3480156101fe575f80fd5b506101e561020d366004613620565b6105c6565b6102256102203660046136d7565b610667565b005b348015610232575f80fd5b506102256102413660046137e1565b6109e8565b348015610251575f80fd5b506101b97f000000000000000000000000000000000000000000000000000000000000000081565b348015610284575f80fd5b506101e5610293366004613620565b610c30565b6102256102a636600461383f565b610c5f565b3480156102b6575f80fd5b506102256102c53660046138dd565b610c7f565b3480156102d5575f80fd5b506102256102e4366004613901565b610c8c565b3480156102f4575f80fd5b5061022561030336600461399d565b611067565b348015610313575f80fd5b506101e56103223660046139f2565b6112eb565b348015610332575f80fd5b506101b97f000000000000000000000000000000000000000000000000000000000000000081565b348015610365575f80fd5b50610225610374366004613a24565b6114ea565b610225610387366004613b0b565b611878565b348015610397575f80fd5b506101e56103a6366004613bc9565b61194c565b3480156103b6575f80fd5b506102256103c5366004613bc9565b611a8e565b5f806103d7858585611b50565b6040517fe75600c30000000000000000000000000000000000000000000000000000000081526001600160a01b0380831660048301529192507f00000000000000000000000000000000000000000000000000000000000000009091169063e75600c390602401602060405180830381865afa158015610459573d5f803e3d5ffd5b505050506040513d601f19601f8201168201806040525081019061047d9190613bfb565b156104ab577f1626ba7e356f5979dd355a3d2bfb43e80420a480c3b854edce286a82d74968699150506104d0565b507fffffffff0000000000000000000000000000000000000000000000000000000090505b9392505050565b5f7f000000000000000000000000000000000000000000000000000000000000000046146105a157604080517f8b73c3c69bb8fe3d512ecc4cf759cc79239f7b179b0ffacaa9a75d522b39400f60208201527f64afec7be651c92f86754beb2bd5eeaf2fa95e83faf4aee989877dde08e4498c918101919091527fc89efdaa54c0f20c7adf612882df0950f5a951637e0307cdcb4c672f298b8bc660608201524660808201523060a082015260c00160405160208183030381529060405280519060200120905090565b507f000000000000000000000000000000000000000000000000000000000000000090565b5f7f68b8e94dc077458241d6c8d89f0a7665c7cda2cfe70c9eb4437efee1663c66fe6105f56020840184613c16565b836020013584604001358560600135866080013560405160200161064a969594939291909586526001600160a01b0394909416602086015260408501929092526060840152608083015260a082015260c00190565b604051602081830303815290604052805190602001209050919050565b61066f611bdb565b6040517fe75600c30000000000000000000000000000000000000000000000000000000081526001600160a01b038a811660048301527f0000000000000000000000000000000000000000000000000000000000000000169063e75600c390602401602060405180830381865afa1580156106ec573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906107109190613bfb565b610746576040517fb331e42100000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b610200870135881580159061076057506101608801358911155b1561077f5761077c896102008a01356101608b01356001611c1e565b90505b6107b07f00000000000000000000000000000000000000000000000000000000000000008b838b8b8b8b8b8b611c69565b6001600160a01b037f00000000000000000000000000000000000000000000000000000000000000001663bc1178e66107ef60c08b0160a08c01613c16565b6108016101408c016101208d01613c16565b6040517fffffffff0000000000000000000000000000000000000000000000000000000060e085901b1681526108449291906101608e0135908890600401613c8d565b5f604051808303815f87803b15801561085b575f80fd5b505af115801561086d573d5f803e3d5ffd5b505050506108b4888b838c5f148061088957506101608c01358d115b610893578c61089a565b6101608c01355b898c8c60026108af60408e0160208f01613d61565b611e01565b6108c16040890189613d7f565b6040516108cf929190613de0565b6040519081900390207f0fce007c38c6c8ed9e545b3a148095762738618f8c21b673222613e4d45734b661090960a08b0160808c01613c16565b61091960c08c0160a08d01613c16565b61092b6101408d016101208e01613c16565b61093d6101e08e016101c08f01613c16565b8e158061094e57506101608e01358f115b610958578e61095f565b6101408e01355b6102008f013588146109715787610978565b6101e08f01355b8f60e001602081019061098b9190613c16565b604080516001600160a01b0398891681529688166020880152948716948601949094529185166060850152608084015260a083015290911660c082015260e00160405180910390a2506109dd60018055565b505050505050505050565b5f5b6109f48280613def565b9050811015610b065736610a088380613def565b83818110610a1857610a18613e53565b9050602002810190610a2a9190613e80565b90506001600160a01b03841663a8c4bc95610a486020840184613c16565b6040517fffffffff0000000000000000000000000000000000000000000000000000000060e084901b1681526001600160a01b039091166004820152602401602060405180830381865afa158015610aa2573d5f803e3d5ffd5b505050506040513d601f19601f82011682018060405250810190610ac69190613bfb565b15610afd576040517fc99e887200000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b506001016109ea565b505f5b610b166020830183613def565b9050811015610c2b5736610b2d6020840184613def565b83818110610b3d57610b3d613e53565b9050602002810190610b4f9190613e80565b90506001600160a01b03841663a8c4bc95610b6d6020840184613c16565b6040517fffffffff0000000000000000000000000000000000000000000000000000000060e084901b1681526001600160a01b039091166004820152602401602060405180830381865afa158015610bc7573d5f803e3d5ffd5b505050506040513d601f19601f82011682018060405250810190610beb9190613bfb565b15610c22576040517fc99e887200000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b50600101610b09565b505050565b5f7fae676bf6913ac2689b7331293c989fe7723124faf8b5d275f06fbcebc77950096105f56020840184613c16565b610c698482612212565b610c78858585856001806122c9565b5050505050565b610c89338261261e565b50565b610cbf6040518060c001604052805f81526020015f81526020015f81526020015f81526020015f81526020015f81525090565b5f5b828110156110535736848483818110610cdc57610cdc613e53565b9050602002810190610cee9190613e80565b9050365f610cff6040840184613d7f565b90925090506001600160a01b038b1663a8c4bc95610d206020860186613c16565b6040517fffffffff0000000000000000000000000000000000000000000000000000000060e084901b1681526001600160a01b039091166004820152602401602060405180830381865afa158015610d7a573d5f803e3d5ffd5b505050506040513d601f19601f82011682018060405250810190610d9e9190613bfb565b15611045575f8915610ddc576040517f7d617bb300000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b60048210610de8575081355b7fc03a9de9000000000000000000000000000000000000000000000000000000007fffffffff00000000000000000000000000000000000000000000000000000000821601610e5657610e3d83838d8c6126c4565b86602001818151610e4e9190613ebc565b905250611043565b7f243a4b7f000000000000000000000000000000000000000000000000000000007fffffffff00000000000000000000000000000000000000000000000000000000821601610ebc57610eab83838d8c6127da565b86606001818151610e4e9190613ebc565b7f7dc4f458000000000000000000000000000000000000000000000000000000007fffffffff00000000000000000000000000000000000000000000000000000000821601610f4757610f1183838d8c61294e565b60a088015260808701819052606087018051610f2e908390613ebc565b90525060a0860151602087018051610e4e908390613ebc565b7f68931b6b000000000000000000000000000000000000000000000000000000007fffffffff00000000000000000000000000000000000000000000000000000000821601610fab57610f9c83838d8c612bb6565b86518790610e4e908390613ebc565b7f0c9be7e4000000000000000000000000000000000000000000000000000000007fffffffff000000000000000000000000000000000000000000000000000000008216016110115761100083838d8c612cc0565b86604001818151610e4e9190613ebc565b6040517f0561d8b300000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b505b505050806001019050610cc1565b5061105e8482612e29565b50505050505050565b60036110766020830183613f21565b600381111561108757611087613ef4565b036110ec576001600160a01b0383166110ac836110a76040850185613d7f565b611b50565b6001600160a01b031614610c2b576040517fb81d58e700000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b60016110fb6020830183613f21565b600381111561110c5761110c613ef4565b036111a0577f19457468657265756d205369676e6564204d6573736167653a0a3332000000005f908152601c839052603c90206001600160a01b03841661115a826110a76040860186613d7f565b6001600160a01b03161461119a576040517f644ae6c300000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b50505050565b60026111af6020830183613f21565b60038111156111c0576111c0613ef4565b036112b9577f1626ba7e000000000000000000000000000000000000000000000000000000006001600160a01b038416631626ba7e846112036040860186613d7f565b6040518463ffffffff1660e01b815260040161122193929190613f3f565b602060405180830381865afa15801561123c573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906112609190613f58565b7fffffffff000000000000000000000000000000000000000000000000000000001614610c2b576040517f5d52cbe300000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6040517f60cd402d00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b5f6112f46104d7565b7fd28e809b708f5ee38be8347d6d869d8232493c094ab2dde98369e4102369a99d61131f8480613d7f565b604051602001611330929190613f97565b60405160208183030381529060405280519060200120846020013585604001602081019061135e9190613c16565b60408051602081019590955284019290925260608301526001600160a01b0316608082015260a001604080517fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe08184030181529190526113c46080850160608601613c16565b6113d460a0860160808701613c16565b6113e460c0870160a08801613c16565b60c087013560e08801356101008901356101208a013561140c6101608c016101408d01613c16565b604080516001600160a01b03998a166020820152978916908801529487166060870152608086019390935260a085019190915260c084015260e083015290911661010082015261012001604080517fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0818403018152908290526114929291602001613fd7565b6040516020818303038152906040528051906020012060405160200161064a9291907f190100000000000000000000000000000000000000000000000000000000000081526002810192909252602282015260420190565b6114f2611bdb565b6040517f02cc250d0000000000000000000000000000000000000000000000000000000081523360048201527f00000000000000000000000000000000000000000000000000000000000000006001600160a01b0316906302cc250d90602401602060405180830381865afa15801561156d573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906115919190613bfb565b6115c7576040517fc139eabd00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6040517fe75600c30000000000000000000000000000000000000000000000000000000081526001600160a01b0389811660048301527f0000000000000000000000000000000000000000000000000000000000000000169063e75600c390602401602060405180830381865afa158015611644573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906116689190613bfb565b61169e576040517fb331e42100000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b61020086013587158015906116b857506101608701358811155b156116d7576116d4886102008901356101608a01356001611c1e565b90505b6117087f00000000000000000000000000000000000000000000000000000000000000008a838a8a8a8a8a8a611c69565b611745878a838b158061171f57506101608b01358c115b611729578b611730565b6101608b01355b888b8b60016108af60408d0160208e01613d61565b6117526040880188613d7f565b604051611760929190613de0565b6040519081900390207f0fce007c38c6c8ed9e545b3a148095762738618f8c21b673222613e4d45734b661179a60a08a0160808b01613c16565b6117aa60c08b0160a08c01613c16565b6117bc6101408c016101208d01613c16565b6117ce6101e08d016101c08e01613c16565b8d15806117df57506101608d01358e115b6117e9578d6117f0565b6101408d01355b6102008e013588146118025787611809565b6101e08e01355b8e60e001602081019061181c9190613c16565b604080516001600160a01b0398891681529688166020880152948716948601949094529185166060850152608084015260a083015290911660c082015260e00160405180910390a25061186e60018055565b5050505050505050565b6118828583612212565b6001600160a01b037f00000000000000000000000000000000000000000000000000000000000000001663bc1178e66118c16080880160608901613c16565b6118d160a0890160808a01613c16565b8860c00135856040518563ffffffff1660e01b81526004016118f69493929190613c8d565b5f604051808303815f87803b15801561190d575f80fd5b505af115801561191f573d5f803e3d5ffd5b5050505061194486868686600289602001602081019061193f9190613d61565b6122c9565b505050505050565b5f6119556104d7565b7fc994d2ca0375d6d473785e0ce0b1d203f069121bac1314f72c5c0fe601eb39106119836040850185613d7f565b604051602001611994929190613f97565b604080517fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0818403018152919052805160209091012060608501356119df60a0870160808801613c16565b6119ef60c0880160a08901613c16565b60c0880135611a056101008a0160e08b01613c16565b60408051602081019890985287019590955260608601939093526001600160a01b039182166080860152811660a085015260c08401919091521660e0820152610100808501359082015261012001604051602081830303815290604052611a6f8461012001610c30565b611a7c856101c0016105c6565b60405160200161149293929190613feb565b6101a0810135611aa8610180830135610160840135613ebc565b611ab29190613ebc565b61014082013514611aef576040517fc04377d300000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b610240810135611b09610220830135610200840135613ebc565b611b139190613ebc565b6101e082013514610c89576040517f877630be00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b5f80611b918585858080601f0160208091040260200160405190810160405280939291908181526020018383808284375f92019190915250612ed692505050565b90506001600160a01b038116611bd3576040517f815e1d6400000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b949350505050565b600260015403611c17576040517f3ee5aeb500000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6002600155565b5f611c4b611c2b83612f00565b8015611c4657505f8480611c4157611c41614008565b868809115b151590565b611c56868686612f2c565b611c609190613ebc565b95945050505050565b5f611c738761194c565b9050611c80898285611067565b611c9060c0880160a08901613c16565b6001600160a01b0316336001600160a01b031614611cc757611cc2611cbb60c0890160a08a01613c16565b8284611067565b611d0e565b611cd46040830183613d7f565b90505f03611d0e576040517f0e364efc00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b611d2b611d2160c0890160a08a01613c16565b886060013561261e565b8660c00135421115611d69576040517f133df02900000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b5f88118015611d7c575086610100013588105b15611db3576040517f9469744400000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b611dbc87611a8e565b611dc68a856109e8565b611de78a8a5f8b118015611ddf57506102008a01358b14155b8a8a8a610c8c565b611df589886060013561261e565b50505050505050505050565b611e13611e0e8680613def565b613001565b5f611e286101808b01356101a08c0135613ebc565b90507f00000000000000000000000000000000000000000000000000000000000000006001600160a01b031663b519d3696040518060a001604052808d60a0016020810190611e779190613c16565b6001600160a01b03168152602001306001600160a01b031681526020018d610120015f016020810190611eaa9190613c16565b6001600160a01b03168152602001848152602001866002811115611ed057611ed0613ef4565b8152506040518263ffffffff1660e01b8152600401611eef9190614035565b5f604051808303815f87803b158015611f06575f80fd5b505af1158015611f18573d5f803e3d5ffd5b505050507f00000000000000000000000000000000000000000000000000000000000000006001600160a01b031663b519d3696040518060a001604052808d60a0016020810190611f699190613c16565b6001600160a01b031681526020018d60e0016020810190611f8a9190613c16565b6001600160a01b031681526020018d610120015f016020810190611fae9190613c16565b6001600160a01b031681526020018a8152602001866002811115611fd457611fd4613ef4565b8152506040518263ffffffff1660e01b8152600401611ff39190614035565b5f604051808303815f87803b15801561200a575f80fd5b505af115801561201c573d5f803e3d5ffd5b5050505061202a8585613001565b7f00000000000000000000000000000000000000000000000000000000000000006001600160a01b031663b519d3696040518060a001604052808c6001600160a01b031681526020018d60800160208101906120869190613c16565b6001600160a01b031681526020018d6101c0015f0160208101906120aa9190613c16565b6001600160a01b031681526020018b81526020018560028111156120d0576120d0613ef4565b8152506040518263ffffffff1660e01b81526004016120ef9190614035565b5f604051808303815f87803b158015612106575f80fd5b505af1158015612118573d5f803e3d5ffd5b5061212e9250611e0e9150506020880188613def565b5f6121416101408c016101208d01613c16565b6040517f70a082310000000000000000000000000000000000000000000000000000000081523060048201526001600160a01b0391909116906370a0823190602401602060405180830381865afa15801561219e573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906121c291906140b4565b90508015612205576122056121de6101008d0160e08e01613c16565b828d610120015f0160208101906121f59190613c16565b6001600160a01b03169190613139565b5050505050505050505050565b6122226080830160608401613c16565b6001600160a01b0316336001600160a01b0316146122615761225c61224d6080840160608501613c16565b612256846112eb565b83611067565b6122a8565b61226e6040820182613d7f565b90505f036122a8576040517f0e364efc00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6122c56122bb6080840160608501613c16565b836020013561261e565b5050565b60e085013583158015906122e057508560c0013584105b156122fd576122fa848760e001358860c001356001611c1e565b90505b612309878288886131b9565b7f00000000000000000000000000000000000000000000000000000000000000006001600160a01b031663b519d3696040518060a001604052808960600160208101906123569190613c16565b6001600160a01b031681526020016123766101608b016101408c01613c16565b6001600160a01b0316815260200161239460a08b0160808c01613c16565b6001600160a01b031681526020018715806123b257508960c0013588115b6123bc57876123c2565b8960c001355b81526020018660028111156123d9576123d9613ef4565b8152506040518263ffffffff1660e01b81526004016123f89190614035565b5f604051808303815f87803b15801561240f575f80fd5b505af1158015612421573d5f803e3d5ffd5b505050507f00000000000000000000000000000000000000000000000000000000000000006001600160a01b031663b519d3696040518060a001604052808a6001600160a01b031681526020018960400160208101906124819190613c16565b6001600160a01b0316815260200161249f60c08b0160a08c01613c16565b6001600160a01b031681526020018481526020018560028111156124c5576124c5613ef4565b8152506040518263ffffffff1660e01b81526004016124e49190614035565b5f604051808303815f87803b1580156124fb575f80fd5b505af115801561250d573d5f803e3d5ffd5b5061251e9250889150819050613d7f565b60405161252c929190613de0565b60405180910390207f0fce007c38c6c8ed9e545b3a148095762738618f8c21b673222613e4d45734b68760400160208101906125689190613c16565b61257860808a0160608b01613c16565b61258860a08b0160808c01613c16565b61259860c08c0160a08d01613c16565b8915806125a857508b60c001358a115b6125b257896125b8565b8b60c001355b878d6101400160208101906125cd9190613c16565b604080516001600160a01b0398891681529688166020880152948716948601949094529185166060850152608084015260a083015290911660c082015260e00160405180910390a250505050505050565b6001600160a01b0382165f9081526020818152604080832084845290915290205460ff1615612679576040517fbc0da7d600000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6001600160a01b039091165f908152602081815260408083209383529290522080547fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff00166001179055565b5f8080806126d5876004818b6140cb565b8101906126e291906140f2565b50919450925090506126fc61014086016101208701613c16565b6001600160a01b0316836001600160a01b031614612746576040517fc891add200000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b816001600160a01b0316866001600160a01b031614612791576040517f815e1d6400000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6101a085013581146127cf576040517f2c5211c600000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b979650505050505050565b5f808080806127ec886004818c6140cb565b8101906127f99190614142565b929650909450925090506128156101e087016101c08801613c16565b6001600160a01b0316846001600160a01b03161461285f576040517fc891add200000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b826001600160a01b0316876001600160a01b0316146128aa576040517f815e1d6400000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6128ba60a0870160808801613c16565b6001600160a01b0316826001600160a01b031614612904576040517fac6b05f500000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6102408601358114612942576040517f2c5211c600000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b98975050505050505050565b5f805f805f805f805f8c8c600490809261296a939291906140cb565b8101906129779190614190565b959c50939a50919850965094509250905061299a6101e08b016101c08c01613c16565b6001600160a01b0316876001600160a01b0316146129e4576040517fc891add200000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b856001600160a01b03168b6001600160a01b031614612a2f576040517f815e1d6400000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6001600160a01b0385163014612a71576040517f8154374b00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b612a8160a08b0160808c01613c16565b6001600160a01b0316846001600160a01b031614612acb576040517fac6b05f500000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6102408a01358314612b09576040517f2c5211c600000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b612b1b6101408b016101208c01613c16565b6001600160a01b0316826001600160a01b031614612b65576040517fc891add200000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6101a08a01358114612ba3576040517f2c5211c600000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b919c919b50909950505050505050505050565b5f808080612bc7876004818b6140cb565b810190612bd4919061420f565b91945092509050612bed61014086016101208701613c16565b6001600160a01b0316836001600160a01b031614612c37576040517fc891add200000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b816001600160a01b0316866001600160a01b031614612c82576040517f815e1d6400000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b61018085013581146127cf576040517f2c5211c600000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b5f80808080612cd2886004818c6140cb565b810190612cdf919061424d565b5092965090945092509050612cfc6101e087016101c08801613c16565b6001600160a01b0316846001600160a01b031614612d46576040517fc891add200000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b826001600160a01b0316876001600160a01b031614612d91576040517f815e1d6400000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b612da160a0870160808801613c16565b6001600160a01b0316826001600160a01b031614612deb576040517fac6b05f500000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6102208601358114612942576040517f2c5211c600000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b8051610180830135141580612e47575060208101516101a083013514155b15612e7e576040517f4a55da2000000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6040810151610220830135141580612e9f5750606081015161024083013514155b156122c5576040517f77a5920300000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b5f805f80612ee4868661325c565b925092509250612ef482826132a5565b50909150505b92915050565b5f6002826003811115612f1557612f15613ef4565b612f1f91906142b1565b60ff166001149050919050565b5f838302817fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff85870982811083820303915050805f03612f7f57838281612f7557612f75614008565b04925050506104d0565b808411612f9657612f9660038515026011186133ad565b5f848688095f868103871696879004966002600389028118808a02820302808a02820302808a02820302808a02820302808a02820302808a02909103029181900381900460010186841190950394909402919094039290920491909117919091029150509392505050565b5f5b81811015610c2b573683838381811061301e5761301e613e53565b90506020028101906130309190613e80565b90506001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000166130696020830183613c16565b6001600160a01b0316036130a9576040517f79a1bff000000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b6130b2816133be565b6130bf6020820182613c16565b6001600160a01b03167fed99827efb37016f2275f98c4bcf71c7551c75d59e9b450f79fa32e60be672c282602001356130f784613401565b604080519283527fffffffff0000000000000000000000000000000000000000000000000000000090911660208301520160405180910390a250600101613003565b604080516001600160a01b038416602482015260448082018490528251808303909101815260649091019091526020810180517bffffffffffffffffffffffffffffffffffffffffffffffffffffffff167fa9059cbb00000000000000000000000000000000000000000000000000000000179052610c2b90849061342a565b5f831180156131cc575081610100013583105b15613203576040517f9469744400000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b61321084612256846112eb565b61321e84836020013561261e565b428261012001351161119a576040517fc56873ba00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b5f805f8351604103613293576020840151604085015160608601515f1a613285888285856134af565b95509550955050505061329e565b505081515f91506002905b9250925092565b5f8260038111156132b8576132b8613ef4565b036132c1575050565b60018260038111156132d5576132d5613ef4565b0361330c576040517ff645eedf00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b600282600381111561332057613320613ef4565b0361335f576040517ffce698f7000000000000000000000000000000000000000000000000000000008152600481018290526024015b60405180910390fd5b600382600381111561337357613373613ef4565b036122c5576040517fd78bce0c00000000000000000000000000000000000000000000000000000000815260048101829052602401613356565b634e487b715f52806020526024601cfd5b5f6133cc6020830183613c16565b90506020820135365f6133e26040860186613d7f565b91509150604051818382375f80838387895af1611944573d5f803e3d5ffd5b5f36816134116040850185613d7f565b90925090506004811061342357813592505b5050919050565b5f8060205f8451602086015f885af180613449576040513d5f823e3d81fd5b50505f513d9150811561346057806001141561346d565b6001600160a01b0384163b155b1561119a576040517f5274afe70000000000000000000000000000000000000000000000000000000081526001600160a01b0385166004820152602401613356565b5f80807f7fffffffffffffffffffffffffffffff5d576e7357a4501ddfe92f46681b20a08411156134e857505f9150600390508261358b565b604080515f808252602082018084528a905260ff891692820192909252606081018790526080810186905260019060a0016020604051602081039080840390855afa158015613539573d5f803e3d5ffd5b50506040517fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe001519150506001600160a01b03811661358257505f92506001915082905061358b565b92505f91508190505b9450945094915050565b5f805f604084860312156135a7575f80fd5b83359250602084013567ffffffffffffffff808211156135c5575f80fd5b818601915086601f8301126135d8575f80fd5b8135818111156135e6575f80fd5b8760208285010111156135f7575f80fd5b6020830194508093505050509250925092565b5f60a0828403121561361a575f80fd5b50919050565b5f60a08284031215613630575f80fd5b6104d0838361360a565b6001600160a01b0381168114610c89575f80fd5b80356136598161363a565b919050565b5f610260828403121561361a575f80fd5b5f8083601f84011261367f575f80fd5b50813567ffffffffffffffff811115613696575f80fd5b6020830191508360208260051b85010111156136b0575f80fd5b9250929050565b5f6040828403121561361a575f80fd5b5f6060828403121561361a575f80fd5b5f805f805f805f805f6101008a8c0312156136f0575f80fd5b6136f98a61364e565b985060208a0135975060408a013567ffffffffffffffff8082111561371c575f80fd5b6137288d838e0161365e565b985060608c013591508082111561373d575f80fd5b6137498d838e0161366f565b909850965060808c0135915080821115613761575f80fd5b61376d8d838e016136b7565b955060a08c0135915080821115613782575f80fd5b61378e8d838e016136c7565b945060c08c01359150808211156137a3575f80fd5b6137af8d838e016136c7565b935060e08c01359150808211156137c4575f80fd5b506137d18c828d016136c7565b9150509295985092959850929598565b5f80604083850312156137f2575f80fd5b82356137fd8161363a565b9150602083013567ffffffffffffffff811115613818575f80fd5b613824858286016136b7565b9150509250929050565b5f610160828403121561361a575f80fd5b5f805f805f60a08688031215613853575f80fd5b853561385e8161363a565b9450602086013567ffffffffffffffff8082111561387a575f80fd5b61388689838a0161382e565b9550604088013591508082111561389b575f80fd5b6138a789838a016136c7565b94506060880135935060808801359150808211156138c3575f80fd5b506138d0888289016136c7565b9150509295509295909350565b5f602082840312156138ed575f80fd5b5035919050565b8015158114610c89575f80fd5b5f805f805f8060a08789031215613916575f80fd5b86356139218161363a565b955060208701356139318161363a565b94506040870135613941816138f4565b9350606087013567ffffffffffffffff8082111561395d575f80fd5b6139698a838b0161365e565b9450608089013591508082111561397e575f80fd5b5061398b89828a0161366f565b979a9699509497509295939492505050565b5f805f606084860312156139af575f80fd5b83356139ba8161363a565b925060208401359150604084013567ffffffffffffffff8111156139dc575f80fd5b6139e8868287016136c7565b9150509250925092565b5f60208284031215613a02575f80fd5b813567ffffffffffffffff811115613a18575f80fd5b611bd38482850161382e565b5f805f805f805f8060e0898b031215613a3b575f80fd5b613a448961364e565b975060208901359650604089013567ffffffffffffffff80821115613a67575f80fd5b613a738c838d0161365e565b975060608b0135915080821115613a88575f80fd5b613a948c838d0161366f565b909750955060808b0135915080821115613aac575f80fd5b613ab88c838d016136b7565b945060a08b0135915080821115613acd575f80fd5b613ad98c838d016136c7565b935060c08b0135915080821115613aee575f80fd5b50613afb8b828c016136c7565b9150509295985092959890939650565b5f805f805f8060c08789031215613b20575f80fd5b613b298761364e565b9550602087013567ffffffffffffffff80821115613b45575f80fd5b613b518a838b0161382e565b96506040890135915080821115613b66575f80fd5b613b728a838b016136c7565b9550606089013594506080890135915080821115613b8e575f80fd5b613b9a8a838b016136c7565b935060a0890135915080821115613baf575f80fd5b50613bbc89828a016136c7565b9150509295509295509295565b5f60208284031215613bd9575f80fd5b813567ffffffffffffffff811115613bef575f80fd5b611bd38482850161365e565b5f60208284031215613c0b575f80fd5b81516104d0816138f4565b5f60208284031215613c26575f80fd5b81356104d08161363a565b81835281816020850137505f602082840101525f60207fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f840116840101905092915050565b803565ffffffffffff81168114613659575f80fd5b5f6001600160a01b0380871683528086166020840152508360408301526080606083015282357fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe1843603018112613ce2575f80fd5b830160208101903567ffffffffffffffff811115613cfe575f80fd5b803603821315613d0c575f80fd5b60606080850152613d2160e085018284613c31565b915050613d3060208501613c78565b65ffffffffffff80821660a086015280613d4c60408801613c78565b1660c086015250508091505095945050505050565b5f60208284031215613d71575f80fd5b8135600381106104d0575f80fd5b5f8083357fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe1843603018112613db2575f80fd5b83018035915067ffffffffffffffff821115613dcc575f80fd5b6020019150368190038213156136b0575f80fd5b818382375f9101908152919050565b5f8083357fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe1843603018112613e22575f80fd5b83018035915067ffffffffffffffff821115613e3c575f80fd5b6020019150600581901b36038213156136b0575f80fd5b7f4e487b71000000000000000000000000000000000000000000000000000000005f52603260045260245ffd5b5f82357fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffa1833603018112613eb2575f80fd5b9190910192915050565b80820180821115612efa577f4e487b71000000000000000000000000000000000000000000000000000000005f52601160045260245ffd5b7f4e487b71000000000000000000000000000000000000000000000000000000005f52602160045260245ffd5b5f60208284031215613f31575f80fd5b8135600481106104d0575f80fd5b838152604060208201525f611c60604083018486613c31565b5f60208284031215613f68575f80fd5b81517fffffffff00000000000000000000000000000000000000000000000000000000811681146104d0575f80fd5b602081525f611bd3602083018486613c31565b5f81515f5b81811015613fc95760208185018101518683015201613faf565b505f93019283525090919050565b5f611bd3613fe58386613faa565b84613faa565b5f613ff68286613faa565b93845250506020820152604001919050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52601260045260245ffd5b5f60a0820190506001600160a01b0380845116835280602085015116602084015280604085015116604084015250606083015160608301526080830151600381106140a7577f4e487b71000000000000000000000000000000000000000000000000000000005f52602160045260245ffd5b8060808401525092915050565b5f602082840312156140c4575f80fd5b5051919050565b5f80858511156140d9575f80fd5b838611156140e5575f80fd5b5050820193919092039150565b5f805f8060808587031215614105575f80fd5b84356141108161363a565b935060208501356141208161363a565b9250604085013591506060850135614137816138f4565b939692955090935050565b5f805f8060808587031215614155575f80fd5b84356141608161363a565b935060208501356141708161363a565b925060408501356141808161363a565b9396929550929360600135925050565b5f805f805f805f60e0888a0312156141a6575f80fd5b87356141b18161363a565b965060208801356141c18161363a565b955060408801356141d18161363a565b945060608801356141e18161363a565b93506080880135925060a08801356141f88161363a565b8092505060c0880135905092959891949750929550565b5f805f60608486031215614221575f80fd5b833561422c8161363a565b9250602084013561423c8161363a565b929592945050506040919091013590565b5f805f805f60a08688031215614261575f80fd5b853561426c8161363a565b9450602086013561427c8161363a565b9350604086013561428c8161363a565b92506060860135915060808601356142a3816138f4565b809150509295509295909350565b5f60ff8316806142e8577f4e487b71000000000000000000000000000000000000000000000000000000005f52601260045260245ffd5b8060ff8416069150509291505056fea26469706673582212209be58acada353061a2a202cc7011f3c91393e0f2e305e9202445b042fc4a4ce664736f6c63430008170033
    /// ```
    #[rustfmt::skip]
    #[allow(clippy::all)]
    pub static DEPLOYED_BYTECODE: alloy_sol_types::private::Bytes = alloy_sol_types::private::Bytes::from_static(
        b"`\x80`@R`\x046\x10a\x01&W_5`\xE0\x1C\x80c\xA5\xCD\xC8\xFC\x11a\0\xA1W\x80c\xC6\x18a\x81\x11a\0qW\x80c\xDBXw(\x11a\0WW\x80c\xDBXw(\x14a\x03yW\x80c\xE2B\x92N\x14a\x03\x8CW\x80c\xFA\\\xD5l\x14a\x03\xABW_\x80\xFD[\x80c\xC6\x18a\x81\x14a\x03'W\x80c\xCB\xA6s\xA7\x14a\x03ZW_\x80\xFD[\x80c\xA5\xCD\xC8\xFC\x14a\x02\xABW\x80c\xA7\xABI\xBC\x14a\x02\xCAW\x80c\xAE\x80\xC5\x84\x14a\x02\xE9W\x80c\xB1\x1F\x12b\x14a\x03\x08W_\x80\xFD[\x80cQ\xD4h\x15\x11a\0\xF6W\x80co5\xD2\xD2\x11a\0\xDCW\x80co5\xD2\xD2\x14a\x02FW\x80c\x87U0\xFF\x14a\x02yW\x80c\x995\xC8h\x14a\x02\x98W_\x80\xFD[\x80cQ\xD4h\x15\x14a\x02\x12W\x80cZ\xA0\xE9]\x14a\x02'W_\x80\xFD[\x80c\x16&\xBA~\x14a\x011W\x80c)\xBC\xDC\x95\x14a\x01\x86W\x80c6D\xE5\x15\x14a\x01\xD1W\x80cL\x9E\x03\xD3\x14a\x01\xF3W_\x80\xFD[6a\x01-W\0[_\x80\xFD[4\x80\x15a\x01<W_\x80\xFD[Pa\x01Pa\x01K6`\x04a5\x95V[a\x03\xCAV[`@Q\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90\x91\x16\x81R` \x01[`@Q\x80\x91\x03\x90\xF3[4\x80\x15a\x01\x91W_\x80\xFD[Pa\x01\xB9\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[`@Q`\x01`\x01`\xA0\x1B\x03\x90\x91\x16\x81R` \x01a\x01}V[4\x80\x15a\x01\xDCW_\x80\xFD[Pa\x01\xE5a\x04\xD7V[`@Q\x90\x81R` \x01a\x01}V[4\x80\x15a\x01\xFEW_\x80\xFD[Pa\x01\xE5a\x02\r6`\x04a6 V[a\x05\xC6V[a\x02%a\x02 6`\x04a6\xD7V[a\x06gV[\0[4\x80\x15a\x022W_\x80\xFD[Pa\x02%a\x02A6`\x04a7\xE1V[a\t\xE8V[4\x80\x15a\x02QW_\x80\xFD[Pa\x01\xB9\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[4\x80\x15a\x02\x84W_\x80\xFD[Pa\x01\xE5a\x02\x936`\x04a6 V[a\x0C0V[a\x02%a\x02\xA66`\x04a8?V[a\x0C_V[4\x80\x15a\x02\xB6W_\x80\xFD[Pa\x02%a\x02\xC56`\x04a8\xDDV[a\x0C\x7FV[4\x80\x15a\x02\xD5W_\x80\xFD[Pa\x02%a\x02\xE46`\x04a9\x01V[a\x0C\x8CV[4\x80\x15a\x02\xF4W_\x80\xFD[Pa\x02%a\x03\x036`\x04a9\x9DV[a\x10gV[4\x80\x15a\x03\x13W_\x80\xFD[Pa\x01\xE5a\x03\"6`\x04a9\xF2V[a\x12\xEBV[4\x80\x15a\x032W_\x80\xFD[Pa\x01\xB9\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[4\x80\x15a\x03eW_\x80\xFD[Pa\x02%a\x03t6`\x04a:$V[a\x14\xEAV[a\x02%a\x03\x876`\x04a;\x0BV[a\x18xV[4\x80\x15a\x03\x97W_\x80\xFD[Pa\x01\xE5a\x03\xA66`\x04a;\xC9V[a\x19LV[4\x80\x15a\x03\xB6W_\x80\xFD[Pa\x02%a\x03\xC56`\x04a;\xC9V[a\x1A\x8EV[_\x80a\x03\xD7\x85\x85\x85a\x1BPV[`@Q\x7F\xE7V\0\xC3\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x80\x83\x16`\x04\x83\x01R\x91\x92P\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90\x91\x16\x90c\xE7V\0\xC3\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x04YW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x04}\x91\x90a;\xFBV[\x15a\x04\xABW\x7F\x16&\xBA~5oYy\xDD5Z=+\xFBC\xE8\x04 \xA4\x80\xC3\xB8T\xED\xCE(j\x82\xD7Ihi\x91PPa\x04\xD0V[P\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90P[\x93\x92PPPV[_\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0F\x14a\x05\xA1W`@\x80Q\x7F\x8Bs\xC3\xC6\x9B\xB8\xFE=Q.\xCCL\xF7Y\xCCy#\x9F{\x17\x9B\x0F\xFA\xCA\xA9\xA7]R+9@\x0F` \x82\x01R\x7Fd\xAF\xEC{\xE6Q\xC9/\x86uK\xEB+\xD5\xEE\xAF/\xA9^\x83\xFA\xF4\xAE\xE9\x89\x87}\xDE\x08\xE4I\x8C\x91\x81\x01\x91\x90\x91R\x7F\xC8\x9E\xFD\xAAT\xC0\xF2\x0Cz\xDFa(\x82\xDF\tP\xF5\xA9Qc~\x03\x07\xCD\xCBLg/)\x8B\x8B\xC6``\x82\x01RF`\x80\x82\x01R0`\xA0\x82\x01R`\xC0\x01`@Q` \x81\x83\x03\x03\x81R\x90`@R\x80Q\x90` \x01 \x90P\x90V[P\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90V[_\x7Fh\xB8\xE9M\xC0wE\x82A\xD6\xC8\xD8\x9F\nve\xC7\xCD\xA2\xCF\xE7\x0C\x9E\xB4C~\xFE\xE1f<f\xFEa\x05\xF5` \x84\x01\x84a<\x16V[\x83` \x015\x84`@\x015\x85``\x015\x86`\x80\x015`@Q` \x01a\x06J\x96\x95\x94\x93\x92\x91\x90\x95\x86R`\x01`\x01`\xA0\x1B\x03\x94\x90\x94\x16` \x86\x01R`@\x85\x01\x92\x90\x92R``\x84\x01R`\x80\x83\x01R`\xA0\x82\x01R`\xC0\x01\x90V[`@Q` \x81\x83\x03\x03\x81R\x90`@R\x80Q\x90` \x01 \x90P\x91\x90PV[a\x06oa\x1B\xDBV[`@Q\x7F\xE7V\0\xC3\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x8A\x81\x16`\x04\x83\x01R\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x90c\xE7V\0\xC3\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x06\xECW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x07\x10\x91\x90a;\xFBV[a\x07FW`@Q\x7F\xB31\xE4!\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\x02\0\x87\x015\x88\x15\x80\x15\x90a\x07`WPa\x01`\x88\x015\x89\x11\x15[\x15a\x07\x7FWa\x07|\x89a\x02\0\x8A\x015a\x01`\x8B\x015`\x01a\x1C\x1EV[\x90P[a\x07\xB0\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x8B\x83\x8B\x8B\x8B\x8B\x8B\x8Ba\x1CiV[`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16c\xBC\x11x\xE6a\x07\xEF`\xC0\x8B\x01`\xA0\x8C\x01a<\x16V[a\x08\x01a\x01@\x8C\x01a\x01 \x8D\x01a<\x16V[`@Q\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\xE0\x85\x90\x1B\x16\x81Ra\x08D\x92\x91\x90a\x01`\x8E\x015\x90\x88\x90`\x04\x01a<\x8DV[_`@Q\x80\x83\x03\x81_\x87\x80;\x15\x80\x15a\x08[W_\x80\xFD[PZ\xF1\x15\x80\x15a\x08mW=_\x80>=_\xFD[PPPPa\x08\xB4\x88\x8B\x83\x8C_\x14\x80a\x08\x89WPa\x01`\x8C\x015\x8D\x11[a\x08\x93W\x8Ca\x08\x9AV[a\x01`\x8C\x015[\x89\x8C\x8C`\x02a\x08\xAF`@\x8E\x01` \x8F\x01a=aV[a\x1E\x01V[a\x08\xC1`@\x89\x01\x89a=\x7FV[`@Qa\x08\xCF\x92\x91\x90a=\xE0V[`@Q\x90\x81\x90\x03\x90 \x7F\x0F\xCE\0|8\xC6\xC8\xED\x9ET[:\x14\x80\x95v'8a\x8F\x8C!\xB6s\"&\x13\xE4\xD4W4\xB6a\t\t`\xA0\x8B\x01`\x80\x8C\x01a<\x16V[a\t\x19`\xC0\x8C\x01`\xA0\x8D\x01a<\x16V[a\t+a\x01@\x8D\x01a\x01 \x8E\x01a<\x16V[a\t=a\x01\xE0\x8E\x01a\x01\xC0\x8F\x01a<\x16V[\x8E\x15\x80a\tNWPa\x01`\x8E\x015\x8F\x11[a\tXW\x8Ea\t_V[a\x01@\x8E\x015[a\x02\0\x8F\x015\x88\x14a\tqW\x87a\txV[a\x01\xE0\x8F\x015[\x8F`\xE0\x01` \x81\x01\x90a\t\x8B\x91\x90a<\x16V[`@\x80Q`\x01`\x01`\xA0\x1B\x03\x98\x89\x16\x81R\x96\x88\x16` \x88\x01R\x94\x87\x16\x94\x86\x01\x94\x90\x94R\x91\x85\x16``\x85\x01R`\x80\x84\x01R`\xA0\x83\x01R\x90\x91\x16`\xC0\x82\x01R`\xE0\x01`@Q\x80\x91\x03\x90\xA2Pa\t\xDD`\x01\x80UV[PPPPPPPPPV[_[a\t\xF4\x82\x80a=\xEFV[\x90P\x81\x10\x15a\x0B\x06W6a\n\x08\x83\x80a=\xEFV[\x83\x81\x81\x10a\n\x18Wa\n\x18a>SV[\x90P` \x02\x81\x01\x90a\n*\x91\x90a>\x80V[\x90P`\x01`\x01`\xA0\x1B\x03\x84\x16c\xA8\xC4\xBC\x95a\nH` \x84\x01\x84a<\x16V[`@Q\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\xE0\x84\x90\x1B\x16\x81R`\x01`\x01`\xA0\x1B\x03\x90\x91\x16`\x04\x82\x01R`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\n\xA2W=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\n\xC6\x91\x90a;\xFBV[\x15a\n\xFDW`@Q\x7F\xC9\x9E\x88r\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[P`\x01\x01a\t\xEAV[P_[a\x0B\x16` \x83\x01\x83a=\xEFV[\x90P\x81\x10\x15a\x0C+W6a\x0B-` \x84\x01\x84a=\xEFV[\x83\x81\x81\x10a\x0B=Wa\x0B=a>SV[\x90P` \x02\x81\x01\x90a\x0BO\x91\x90a>\x80V[\x90P`\x01`\x01`\xA0\x1B\x03\x84\x16c\xA8\xC4\xBC\x95a\x0Bm` \x84\x01\x84a<\x16V[`@Q\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\xE0\x84\x90\x1B\x16\x81R`\x01`\x01`\xA0\x1B\x03\x90\x91\x16`\x04\x82\x01R`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x0B\xC7W=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x0B\xEB\x91\x90a;\xFBV[\x15a\x0C\"W`@Q\x7F\xC9\x9E\x88r\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[P`\x01\x01a\x0B\tV[PPPV[_\x7F\xAEgk\xF6\x91:\xC2h\x9Bs1)<\x98\x9F\xE7r1$\xFA\xF8\xB5\xD2u\xF0o\xBC\xEB\xC7yP\ta\x05\xF5` \x84\x01\x84a<\x16V[a\x0Ci\x84\x82a\"\x12V[a\x0Cx\x85\x85\x85\x85`\x01\x80a\"\xC9V[PPPPPV[a\x0C\x893\x82a&\x1EV[PV[a\x0C\xBF`@Q\x80`\xC0\x01`@R\x80_\x81R` \x01_\x81R` \x01_\x81R` \x01_\x81R` \x01_\x81R` \x01_\x81RP\x90V[_[\x82\x81\x10\x15a\x10SW6\x84\x84\x83\x81\x81\x10a\x0C\xDCWa\x0C\xDCa>SV[\x90P` \x02\x81\x01\x90a\x0C\xEE\x91\x90a>\x80V[\x90P6_a\x0C\xFF`@\x84\x01\x84a=\x7FV[\x90\x92P\x90P`\x01`\x01`\xA0\x1B\x03\x8B\x16c\xA8\xC4\xBC\x95a\r ` \x86\x01\x86a<\x16V[`@Q\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\xE0\x84\x90\x1B\x16\x81R`\x01`\x01`\xA0\x1B\x03\x90\x91\x16`\x04\x82\x01R`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\rzW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\r\x9E\x91\x90a;\xFBV[\x15a\x10EW_\x89\x15a\r\xDCW`@Q\x7F}a{\xB3\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[`\x04\x82\x10a\r\xE8WP\x815[\x7F\xC0:\x9D\xE9\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x82\x16\x01a\x0EVWa\x0E=\x83\x83\x8D\x8Ca&\xC4V[\x86` \x01\x81\x81Qa\x0EN\x91\x90a>\xBCV[\x90RPa\x10CV[\x7F$:K\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x82\x16\x01a\x0E\xBCWa\x0E\xAB\x83\x83\x8D\x8Ca'\xDAV[\x86``\x01\x81\x81Qa\x0EN\x91\x90a>\xBCV[\x7F}\xC4\xF4X\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x82\x16\x01a\x0FGWa\x0F\x11\x83\x83\x8D\x8Ca)NV[`\xA0\x88\x01R`\x80\x87\x01\x81\x90R``\x87\x01\x80Qa\x0F.\x90\x83\x90a>\xBCV[\x90RP`\xA0\x86\x01Q` \x87\x01\x80Qa\x0EN\x90\x83\x90a>\xBCV[\x7Fh\x93\x1Bk\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x82\x16\x01a\x0F\xABWa\x0F\x9C\x83\x83\x8D\x8Ca+\xB6V[\x86Q\x87\x90a\x0EN\x90\x83\x90a>\xBCV[\x7F\x0C\x9B\xE7\xE4\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x82\x16\x01a\x10\x11Wa\x10\0\x83\x83\x8D\x8Ca,\xC0V[\x86`@\x01\x81\x81Qa\x0EN\x91\x90a>\xBCV[`@Q\x7F\x05a\xD8\xB3\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[P[PPP\x80`\x01\x01\x90Pa\x0C\xC1V[Pa\x10^\x84\x82a.)V[PPPPPPPV[`\x03a\x10v` \x83\x01\x83a?!V[`\x03\x81\x11\x15a\x10\x87Wa\x10\x87a>\xF4V[\x03a\x10\xECW`\x01`\x01`\xA0\x1B\x03\x83\x16a\x10\xAC\x83a\x10\xA7`@\x85\x01\x85a=\x7FV[a\x1BPV[`\x01`\x01`\xA0\x1B\x03\x16\x14a\x0C+W`@Q\x7F\xB8\x1DX\xE7\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[`\x01a\x10\xFB` \x83\x01\x83a?!V[`\x03\x81\x11\x15a\x11\x0CWa\x11\x0Ca>\xF4V[\x03a\x11\xA0W\x7F\x19Ethereum Signed Message:\n32\0\0\0\0_\x90\x81R`\x1C\x83\x90R`<\x90 `\x01`\x01`\xA0\x1B\x03\x84\x16a\x11Z\x82a\x10\xA7`@\x86\x01\x86a=\x7FV[`\x01`\x01`\xA0\x1B\x03\x16\x14a\x11\x9AW`@Q\x7FdJ\xE6\xC3\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[PPPPV[`\x02a\x11\xAF` \x83\x01\x83a?!V[`\x03\x81\x11\x15a\x11\xC0Wa\x11\xC0a>\xF4V[\x03a\x12\xB9W\x7F\x16&\xBA~\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\x01`\x01`\xA0\x1B\x03\x84\x16c\x16&\xBA~\x84a\x12\x03`@\x86\x01\x86a=\x7FV[`@Q\x84c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01a\x12!\x93\x92\x91\x90a??V[` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x12<W=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x12`\x91\x90a?XV[\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x14a\x0C+W`@Q\x7F]R\xCB\xE3\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[`@Q\x7F`\xCD@-\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[_a\x12\xF4a\x04\xD7V[\x7F\xD2\x8E\x80\x9Bp\x8F^\xE3\x8B\xE84}m\x86\x9D\x822I<\tJ\xB2\xDD\xE9\x83i\xE4\x10#i\xA9\x9Da\x13\x1F\x84\x80a=\x7FV[`@Q` \x01a\x130\x92\x91\x90a?\x97V[`@Q` \x81\x83\x03\x03\x81R\x90`@R\x80Q\x90` \x01 \x84` \x015\x85`@\x01` \x81\x01\x90a\x13^\x91\x90a<\x16V[`@\x80Q` \x81\x01\x95\x90\x95R\x84\x01\x92\x90\x92R``\x83\x01R`\x01`\x01`\xA0\x1B\x03\x16`\x80\x82\x01R`\xA0\x01`@\x80Q\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x81\x84\x03\x01\x81R\x91\x90Ra\x13\xC4`\x80\x85\x01``\x86\x01a<\x16V[a\x13\xD4`\xA0\x86\x01`\x80\x87\x01a<\x16V[a\x13\xE4`\xC0\x87\x01`\xA0\x88\x01a<\x16V[`\xC0\x87\x015`\xE0\x88\x015a\x01\0\x89\x015a\x01 \x8A\x015a\x14\x0Ca\x01`\x8C\x01a\x01@\x8D\x01a<\x16V[`@\x80Q`\x01`\x01`\xA0\x1B\x03\x99\x8A\x16` \x82\x01R\x97\x89\x16\x90\x88\x01R\x94\x87\x16``\x87\x01R`\x80\x86\x01\x93\x90\x93R`\xA0\x85\x01\x91\x90\x91R`\xC0\x84\x01R`\xE0\x83\x01R\x90\x91\x16a\x01\0\x82\x01Ra\x01 \x01`@\x80Q\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x81\x84\x03\x01\x81R\x90\x82\x90Ra\x14\x92\x92\x91` \x01a?\xD7V[`@Q` \x81\x83\x03\x03\x81R\x90`@R\x80Q\x90` \x01 `@Q` \x01a\x06J\x92\x91\x90\x7F\x19\x01\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x02\x81\x01\x92\x90\x92R`\"\x82\x01R`B\x01\x90V[a\x14\xF2a\x1B\xDBV[`@Q\x7F\x02\xCC%\r\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R3`\x04\x82\x01R\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\x01`\x01`\xA0\x1B\x03\x16\x90c\x02\xCC%\r\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x15mW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x15\x91\x91\x90a;\xFBV[a\x15\xC7W`@Q\x7F\xC19\xEA\xBD\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[`@Q\x7F\xE7V\0\xC3\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x89\x81\x16`\x04\x83\x01R\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x90c\xE7V\0\xC3\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x16DW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x16h\x91\x90a;\xFBV[a\x16\x9EW`@Q\x7F\xB31\xE4!\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\x02\0\x86\x015\x87\x15\x80\x15\x90a\x16\xB8WPa\x01`\x87\x015\x88\x11\x15[\x15a\x16\xD7Wa\x16\xD4\x88a\x02\0\x89\x015a\x01`\x8A\x015`\x01a\x1C\x1EV[\x90P[a\x17\x08\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x8A\x83\x8A\x8A\x8A\x8A\x8A\x8Aa\x1CiV[a\x17E\x87\x8A\x83\x8B\x15\x80a\x17\x1FWPa\x01`\x8B\x015\x8C\x11[a\x17)W\x8Ba\x170V[a\x01`\x8B\x015[\x88\x8B\x8B`\x01a\x08\xAF`@\x8D\x01` \x8E\x01a=aV[a\x17R`@\x88\x01\x88a=\x7FV[`@Qa\x17`\x92\x91\x90a=\xE0V[`@Q\x90\x81\x90\x03\x90 \x7F\x0F\xCE\0|8\xC6\xC8\xED\x9ET[:\x14\x80\x95v'8a\x8F\x8C!\xB6s\"&\x13\xE4\xD4W4\xB6a\x17\x9A`\xA0\x8A\x01`\x80\x8B\x01a<\x16V[a\x17\xAA`\xC0\x8B\x01`\xA0\x8C\x01a<\x16V[a\x17\xBCa\x01@\x8C\x01a\x01 \x8D\x01a<\x16V[a\x17\xCEa\x01\xE0\x8D\x01a\x01\xC0\x8E\x01a<\x16V[\x8D\x15\x80a\x17\xDFWPa\x01`\x8D\x015\x8E\x11[a\x17\xE9W\x8Da\x17\xF0V[a\x01@\x8D\x015[a\x02\0\x8E\x015\x88\x14a\x18\x02W\x87a\x18\tV[a\x01\xE0\x8E\x015[\x8E`\xE0\x01` \x81\x01\x90a\x18\x1C\x91\x90a<\x16V[`@\x80Q`\x01`\x01`\xA0\x1B\x03\x98\x89\x16\x81R\x96\x88\x16` \x88\x01R\x94\x87\x16\x94\x86\x01\x94\x90\x94R\x91\x85\x16``\x85\x01R`\x80\x84\x01R`\xA0\x83\x01R\x90\x91\x16`\xC0\x82\x01R`\xE0\x01`@Q\x80\x91\x03\x90\xA2Pa\x18n`\x01\x80UV[PPPPPPPPV[a\x18\x82\x85\x83a\"\x12V[`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16c\xBC\x11x\xE6a\x18\xC1`\x80\x88\x01``\x89\x01a<\x16V[a\x18\xD1`\xA0\x89\x01`\x80\x8A\x01a<\x16V[\x88`\xC0\x015\x85`@Q\x85c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01a\x18\xF6\x94\x93\x92\x91\x90a<\x8DV[_`@Q\x80\x83\x03\x81_\x87\x80;\x15\x80\x15a\x19\rW_\x80\xFD[PZ\xF1\x15\x80\x15a\x19\x1FW=_\x80>=_\xFD[PPPPa\x19D\x86\x86\x86\x86`\x02\x89` \x01` \x81\x01\x90a\x19?\x91\x90a=aV[a\"\xC9V[PPPPPPV[_a\x19Ua\x04\xD7V[\x7F\xC9\x94\xD2\xCA\x03u\xD6\xD4sx^\x0C\xE0\xB1\xD2\x03\xF0i\x12\x1B\xAC\x13\x14\xF7,\\\x0F\xE6\x01\xEB9\x10a\x19\x83`@\x85\x01\x85a=\x7FV[`@Q` \x01a\x19\x94\x92\x91\x90a?\x97V[`@\x80Q\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x81\x84\x03\x01\x81R\x91\x90R\x80Q` \x90\x91\x01 ``\x85\x015a\x19\xDF`\xA0\x87\x01`\x80\x88\x01a<\x16V[a\x19\xEF`\xC0\x88\x01`\xA0\x89\x01a<\x16V[`\xC0\x88\x015a\x1A\x05a\x01\0\x8A\x01`\xE0\x8B\x01a<\x16V[`@\x80Q` \x81\x01\x98\x90\x98R\x87\x01\x95\x90\x95R``\x86\x01\x93\x90\x93R`\x01`\x01`\xA0\x1B\x03\x91\x82\x16`\x80\x86\x01R\x81\x16`\xA0\x85\x01R`\xC0\x84\x01\x91\x90\x91R\x16`\xE0\x82\x01Ra\x01\0\x80\x85\x015\x90\x82\x01Ra\x01 \x01`@Q` \x81\x83\x03\x03\x81R\x90`@Ra\x1Ao\x84a\x01 \x01a\x0C0V[a\x1A|\x85a\x01\xC0\x01a\x05\xC6V[`@Q` \x01a\x14\x92\x93\x92\x91\x90a?\xEBV[a\x01\xA0\x81\x015a\x1A\xA8a\x01\x80\x83\x015a\x01`\x84\x015a>\xBCV[a\x1A\xB2\x91\x90a>\xBCV[a\x01@\x82\x015\x14a\x1A\xEFW`@Q\x7F\xC0Cw\xD3\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\x02@\x81\x015a\x1B\ta\x02 \x83\x015a\x02\0\x84\x015a>\xBCV[a\x1B\x13\x91\x90a>\xBCV[a\x01\xE0\x82\x015\x14a\x0C\x89W`@Q\x7F\x87v0\xBE\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[_\x80a\x1B\x91\x85\x85\x85\x80\x80`\x1F\x01` \x80\x91\x04\x02` \x01`@Q\x90\x81\x01`@R\x80\x93\x92\x91\x90\x81\x81R` \x01\x83\x83\x80\x82\x847_\x92\x01\x91\x90\x91RPa.\xD6\x92PPPV[\x90P`\x01`\x01`\xA0\x1B\x03\x81\x16a\x1B\xD3W`@Q\x7F\x81^\x1Dd\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x94\x93PPPPV[`\x02`\x01T\x03a\x1C\x17W`@Q\x7F>\xE5\xAE\xB5\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[`\x02`\x01UV[_a\x1CKa\x1C+\x83a/\0V[\x80\x15a\x1CFWP_\x84\x80a\x1CAWa\x1CAa@\x08V[\x86\x88\t\x11[\x15\x15\x90V[a\x1CV\x86\x86\x86a/,V[a\x1C`\x91\x90a>\xBCV[\x95\x94PPPPPV[_a\x1Cs\x87a\x19LV[\x90Pa\x1C\x80\x89\x82\x85a\x10gV[a\x1C\x90`\xC0\x88\x01`\xA0\x89\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x163`\x01`\x01`\xA0\x1B\x03\x16\x14a\x1C\xC7Wa\x1C\xC2a\x1C\xBB`\xC0\x89\x01`\xA0\x8A\x01a<\x16V[\x82\x84a\x10gV[a\x1D\x0EV[a\x1C\xD4`@\x83\x01\x83a=\x7FV[\x90P_\x03a\x1D\x0EW`@Q\x7F\x0E6N\xFC\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\x1D+a\x1D!`\xC0\x89\x01`\xA0\x8A\x01a<\x16V[\x88``\x015a&\x1EV[\x86`\xC0\x015B\x11\x15a\x1DiW`@Q\x7F\x13=\xF0)\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[_\x88\x11\x80\x15a\x1D|WP\x86a\x01\0\x015\x88\x10[\x15a\x1D\xB3W`@Q\x7F\x94itD\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\x1D\xBC\x87a\x1A\x8EV[a\x1D\xC6\x8A\x85a\t\xE8V[a\x1D\xE7\x8A\x8A_\x8B\x11\x80\x15a\x1D\xDFWPa\x02\0\x8A\x015\x8B\x14\x15[\x8A\x8A\x8Aa\x0C\x8CV[a\x1D\xF5\x89\x88``\x015a&\x1EV[PPPPPPPPPPV[a\x1E\x13a\x1E\x0E\x86\x80a=\xEFV[a0\x01V[_a\x1E(a\x01\x80\x8B\x015a\x01\xA0\x8C\x015a>\xBCV[\x90P\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\x01`\x01`\xA0\x1B\x03\x16c\xB5\x19\xD3i`@Q\x80`\xA0\x01`@R\x80\x8D`\xA0\x01` \x81\x01\x90a\x1Ew\x91\x90a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x010`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01\x8Da\x01 \x01_\x01` \x81\x01\x90a\x1E\xAA\x91\x90a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01\x84\x81R` \x01\x86`\x02\x81\x11\x15a\x1E\xD0Wa\x1E\xD0a>\xF4V[\x81RP`@Q\x82c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01a\x1E\xEF\x91\x90a@5V[_`@Q\x80\x83\x03\x81_\x87\x80;\x15\x80\x15a\x1F\x06W_\x80\xFD[PZ\xF1\x15\x80\x15a\x1F\x18W=_\x80>=_\xFD[PPPP\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\x01`\x01`\xA0\x1B\x03\x16c\xB5\x19\xD3i`@Q\x80`\xA0\x01`@R\x80\x8D`\xA0\x01` \x81\x01\x90a\x1Fi\x91\x90a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01\x8D`\xE0\x01` \x81\x01\x90a\x1F\x8A\x91\x90a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01\x8Da\x01 \x01_\x01` \x81\x01\x90a\x1F\xAE\x91\x90a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01\x8A\x81R` \x01\x86`\x02\x81\x11\x15a\x1F\xD4Wa\x1F\xD4a>\xF4V[\x81RP`@Q\x82c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01a\x1F\xF3\x91\x90a@5V[_`@Q\x80\x83\x03\x81_\x87\x80;\x15\x80\x15a \nW_\x80\xFD[PZ\xF1\x15\x80\x15a \x1CW=_\x80>=_\xFD[PPPPa *\x85\x85a0\x01V[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\x01`\x01`\xA0\x1B\x03\x16c\xB5\x19\xD3i`@Q\x80`\xA0\x01`@R\x80\x8C`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01\x8D`\x80\x01` \x81\x01\x90a \x86\x91\x90a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01\x8Da\x01\xC0\x01_\x01` \x81\x01\x90a \xAA\x91\x90a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01\x8B\x81R` \x01\x85`\x02\x81\x11\x15a \xD0Wa \xD0a>\xF4V[\x81RP`@Q\x82c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01a \xEF\x91\x90a@5V[_`@Q\x80\x83\x03\x81_\x87\x80;\x15\x80\x15a!\x06W_\x80\xFD[PZ\xF1\x15\x80\x15a!\x18W=_\x80>=_\xFD[Pa!.\x92Pa\x1E\x0E\x91PP` \x88\x01\x88a=\xEFV[_a!Aa\x01@\x8C\x01a\x01 \x8D\x01a<\x16V[`@Q\x7Fp\xA0\x821\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R0`\x04\x82\x01R`\x01`\x01`\xA0\x1B\x03\x91\x90\x91\x16\x90cp\xA0\x821\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a!\x9EW=_\x80>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a!\xC2\x91\x90a@\xB4V[\x90P\x80\x15a\"\x05Wa\"\x05a!\xDEa\x01\0\x8D\x01`\xE0\x8E\x01a<\x16V[\x82\x8Da\x01 \x01_\x01` \x81\x01\x90a!\xF5\x91\x90a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x91\x90a19V[PPPPPPPPPPPV[a\"\"`\x80\x83\x01``\x84\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x163`\x01`\x01`\xA0\x1B\x03\x16\x14a\"aWa\"\\a\"M`\x80\x84\x01``\x85\x01a<\x16V[a\"V\x84a\x12\xEBV[\x83a\x10gV[a\"\xA8V[a\"n`@\x82\x01\x82a=\x7FV[\x90P_\x03a\"\xA8W`@Q\x7F\x0E6N\xFC\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\"\xC5a\"\xBB`\x80\x84\x01``\x85\x01a<\x16V[\x83` \x015a&\x1EV[PPV[`\xE0\x85\x015\x83\x15\x80\x15\x90a\"\xE0WP\x85`\xC0\x015\x84\x10[\x15a\"\xFDWa\"\xFA\x84\x87`\xE0\x015\x88`\xC0\x015`\x01a\x1C\x1EV[\x90P[a#\t\x87\x82\x88\x88a1\xB9V[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\x01`\x01`\xA0\x1B\x03\x16c\xB5\x19\xD3i`@Q\x80`\xA0\x01`@R\x80\x89``\x01` \x81\x01\x90a#V\x91\x90a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01a#va\x01`\x8B\x01a\x01@\x8C\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01a#\x94`\xA0\x8B\x01`\x80\x8C\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01\x87\x15\x80a#\xB2WP\x89`\xC0\x015\x88\x11[a#\xBCW\x87a#\xC2V[\x89`\xC0\x015[\x81R` \x01\x86`\x02\x81\x11\x15a#\xD9Wa#\xD9a>\xF4V[\x81RP`@Q\x82c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01a#\xF8\x91\x90a@5V[_`@Q\x80\x83\x03\x81_\x87\x80;\x15\x80\x15a$\x0FW_\x80\xFD[PZ\xF1\x15\x80\x15a$!W=_\x80>=_\xFD[PPPP\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\x01`\x01`\xA0\x1B\x03\x16c\xB5\x19\xD3i`@Q\x80`\xA0\x01`@R\x80\x8A`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01\x89`@\x01` \x81\x01\x90a$\x81\x91\x90a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01a$\x9F`\xC0\x8B\x01`\xA0\x8C\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x81R` \x01\x84\x81R` \x01\x85`\x02\x81\x11\x15a$\xC5Wa$\xC5a>\xF4V[\x81RP`@Q\x82c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01a$\xE4\x91\x90a@5V[_`@Q\x80\x83\x03\x81_\x87\x80;\x15\x80\x15a$\xFBW_\x80\xFD[PZ\xF1\x15\x80\x15a%\rW=_\x80>=_\xFD[Pa%\x1E\x92P\x88\x91P\x81\x90Pa=\x7FV[`@Qa%,\x92\x91\x90a=\xE0V[`@Q\x80\x91\x03\x90 \x7F\x0F\xCE\0|8\xC6\xC8\xED\x9ET[:\x14\x80\x95v'8a\x8F\x8C!\xB6s\"&\x13\xE4\xD4W4\xB6\x87`@\x01` \x81\x01\x90a%h\x91\x90a<\x16V[a%x`\x80\x8A\x01``\x8B\x01a<\x16V[a%\x88`\xA0\x8B\x01`\x80\x8C\x01a<\x16V[a%\x98`\xC0\x8C\x01`\xA0\x8D\x01a<\x16V[\x89\x15\x80a%\xA8WP\x8B`\xC0\x015\x8A\x11[a%\xB2W\x89a%\xB8V[\x8B`\xC0\x015[\x87\x8Da\x01@\x01` \x81\x01\x90a%\xCD\x91\x90a<\x16V[`@\x80Q`\x01`\x01`\xA0\x1B\x03\x98\x89\x16\x81R\x96\x88\x16` \x88\x01R\x94\x87\x16\x94\x86\x01\x94\x90\x94R\x91\x85\x16``\x85\x01R`\x80\x84\x01R`\xA0\x83\x01R\x90\x91\x16`\xC0\x82\x01R`\xE0\x01`@Q\x80\x91\x03\x90\xA2PPPPPPPV[`\x01`\x01`\xA0\x1B\x03\x82\x16_\x90\x81R` \x81\x81R`@\x80\x83 \x84\x84R\x90\x91R\x90 T`\xFF\x16\x15a&yW`@Q\x7F\xBC\r\xA7\xD6\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[`\x01`\x01`\xA0\x1B\x03\x90\x91\x16_\x90\x81R` \x81\x81R`@\x80\x83 \x93\x83R\x92\x90R \x80T\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\0\x16`\x01\x17\x90UV[_\x80\x80\x80a&\xD5\x87`\x04\x81\x8Ba@\xCBV[\x81\x01\x90a&\xE2\x91\x90a@\xF2V[P\x91\x94P\x92P\x90Pa&\xFCa\x01@\x86\x01a\x01 \x87\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x83`\x01`\x01`\xA0\x1B\x03\x16\x14a'FW`@Q\x7F\xC8\x91\xAD\xD2\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x81`\x01`\x01`\xA0\x1B\x03\x16\x86`\x01`\x01`\xA0\x1B\x03\x16\x14a'\x91W`@Q\x7F\x81^\x1Dd\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\x01\xA0\x85\x015\x81\x14a'\xCFW`@Q\x7F,R\x11\xC6\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x97\x96PPPPPPPV[_\x80\x80\x80\x80a'\xEC\x88`\x04\x81\x8Ca@\xCBV[\x81\x01\x90a'\xF9\x91\x90aABV[\x92\x96P\x90\x94P\x92P\x90Pa(\x15a\x01\xE0\x87\x01a\x01\xC0\x88\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x84`\x01`\x01`\xA0\x1B\x03\x16\x14a(_W`@Q\x7F\xC8\x91\xAD\xD2\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x82`\x01`\x01`\xA0\x1B\x03\x16\x87`\x01`\x01`\xA0\x1B\x03\x16\x14a(\xAAW`@Q\x7F\x81^\x1Dd\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a(\xBA`\xA0\x87\x01`\x80\x88\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x82`\x01`\x01`\xA0\x1B\x03\x16\x14a)\x04W`@Q\x7F\xACk\x05\xF5\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\x02@\x86\x015\x81\x14a)BW`@Q\x7F,R\x11\xC6\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x98\x97PPPPPPPPV[_\x80_\x80_\x80_\x80_\x8C\x8C`\x04\x90\x80\x92a)j\x93\x92\x91\x90a@\xCBV[\x81\x01\x90a)w\x91\x90aA\x90V[\x95\x9CP\x93\x9AP\x91\x98P\x96P\x94P\x92P\x90Pa)\x9Aa\x01\xE0\x8B\x01a\x01\xC0\x8C\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x87`\x01`\x01`\xA0\x1B\x03\x16\x14a)\xE4W`@Q\x7F\xC8\x91\xAD\xD2\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x85`\x01`\x01`\xA0\x1B\x03\x16\x8B`\x01`\x01`\xA0\x1B\x03\x16\x14a*/W`@Q\x7F\x81^\x1Dd\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[`\x01`\x01`\xA0\x1B\x03\x85\x160\x14a*qW`@Q\x7F\x81T7K\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a*\x81`\xA0\x8B\x01`\x80\x8C\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x84`\x01`\x01`\xA0\x1B\x03\x16\x14a*\xCBW`@Q\x7F\xACk\x05\xF5\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\x02@\x8A\x015\x83\x14a+\tW`@Q\x7F,R\x11\xC6\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a+\x1Ba\x01@\x8B\x01a\x01 \x8C\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x82`\x01`\x01`\xA0\x1B\x03\x16\x14a+eW`@Q\x7F\xC8\x91\xAD\xD2\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\x01\xA0\x8A\x015\x81\x14a+\xA3W`@Q\x7F,R\x11\xC6\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x91\x9C\x91\x9BP\x90\x99PPPPPPPPPPV[_\x80\x80\x80a+\xC7\x87`\x04\x81\x8Ba@\xCBV[\x81\x01\x90a+\xD4\x91\x90aB\x0FV[\x91\x94P\x92P\x90Pa+\xEDa\x01@\x86\x01a\x01 \x87\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x83`\x01`\x01`\xA0\x1B\x03\x16\x14a,7W`@Q\x7F\xC8\x91\xAD\xD2\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x81`\x01`\x01`\xA0\x1B\x03\x16\x86`\x01`\x01`\xA0\x1B\x03\x16\x14a,\x82W`@Q\x7F\x81^\x1Dd\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\x01\x80\x85\x015\x81\x14a'\xCFW`@Q\x7F,R\x11\xC6\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[_\x80\x80\x80\x80a,\xD2\x88`\x04\x81\x8Ca@\xCBV[\x81\x01\x90a,\xDF\x91\x90aBMV[P\x92\x96P\x90\x94P\x92P\x90Pa,\xFCa\x01\xE0\x87\x01a\x01\xC0\x88\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x84`\x01`\x01`\xA0\x1B\x03\x16\x14a-FW`@Q\x7F\xC8\x91\xAD\xD2\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x82`\x01`\x01`\xA0\x1B\x03\x16\x87`\x01`\x01`\xA0\x1B\x03\x16\x14a-\x91W`@Q\x7F\x81^\x1Dd\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a-\xA1`\xA0\x87\x01`\x80\x88\x01a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x82`\x01`\x01`\xA0\x1B\x03\x16\x14a-\xEBW`@Q\x7F\xACk\x05\xF5\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a\x02 \x86\x015\x81\x14a)BW`@Q\x7F,R\x11\xC6\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[\x80Qa\x01\x80\x83\x015\x14\x15\x80a.GWP` \x81\x01Qa\x01\xA0\x83\x015\x14\x15[\x15a.~W`@Q\x7FJU\xDA \0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[`@\x81\x01Qa\x02 \x83\x015\x14\x15\x80a.\x9FWP``\x81\x01Qa\x02@\x83\x015\x14\x15[\x15a\"\xC5W`@Q\x7Fw\xA5\x92\x03\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[_\x80_\x80a.\xE4\x86\x86a2\\V[\x92P\x92P\x92Pa.\xF4\x82\x82a2\xA5V[P\x90\x91PP[\x92\x91PPV[_`\x02\x82`\x03\x81\x11\x15a/\x15Wa/\x15a>\xF4V[a/\x1F\x91\x90aB\xB1V[`\xFF\x16`\x01\x14\x90P\x91\x90PV[_\x83\x83\x02\x81\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x85\x87\t\x82\x81\x10\x83\x82\x03\x03\x91PP\x80_\x03a/\x7FW\x83\x82\x81a/uWa/ua@\x08V[\x04\x92PPPa\x04\xD0V[\x80\x84\x11a/\x96Wa/\x96`\x03\x85\x15\x02`\x11\x18a3\xADV[_\x84\x86\x88\t_\x86\x81\x03\x87\x16\x96\x87\x90\x04\x96`\x02`\x03\x89\x02\x81\x18\x80\x8A\x02\x82\x03\x02\x80\x8A\x02\x82\x03\x02\x80\x8A\x02\x82\x03\x02\x80\x8A\x02\x82\x03\x02\x80\x8A\x02\x82\x03\x02\x80\x8A\x02\x90\x91\x03\x02\x91\x81\x90\x03\x81\x90\x04`\x01\x01\x86\x84\x11\x90\x95\x03\x94\x90\x94\x02\x91\x90\x94\x03\x92\x90\x92\x04\x91\x90\x91\x17\x91\x90\x91\x02\x91PP\x93\x92PPPV[_[\x81\x81\x10\x15a\x0C+W6\x83\x83\x83\x81\x81\x10a0\x1EWa0\x1Ea>SV[\x90P` \x02\x81\x01\x90a00\x91\x90a>\x80V[\x90P`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16a0i` \x83\x01\x83a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x03a0\xA9W`@Q\x7Fy\xA1\xBF\xF0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a0\xB2\x81a3\xBEV[a0\xBF` \x82\x01\x82a<\x16V[`\x01`\x01`\xA0\x1B\x03\x16\x7F\xED\x99\x82~\xFB7\x01o\"u\xF9\x8CK\xCFq\xC7U\x1Cu\xD5\x9E\x9BE\x0Fy\xFA2\xE6\x0B\xE6r\xC2\x82` \x015a0\xF7\x84a4\x01V[`@\x80Q\x92\x83R\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90\x91\x16` \x83\x01R\x01`@Q\x80\x91\x03\x90\xA2P`\x01\x01a0\x03V[`@\x80Q`\x01`\x01`\xA0\x1B\x03\x84\x16`$\x82\x01R`D\x80\x82\x01\x84\x90R\x82Q\x80\x83\x03\x90\x91\x01\x81R`d\x90\x91\x01\x90\x91R` \x81\x01\x80Q{\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x7F\xA9\x05\x9C\xBB\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x17\x90Ra\x0C+\x90\x84\x90a4*V[_\x83\x11\x80\x15a1\xCCWP\x81a\x01\0\x015\x83\x10[\x15a2\x03W`@Q\x7F\x94itD\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[a2\x10\x84a\"V\x84a\x12\xEBV[a2\x1E\x84\x83` \x015a&\x1EV[B\x82a\x01 \x015\x11a\x11\x9AW`@Q\x7F\xC5hs\xBA\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[_\x80_\x83Q`A\x03a2\x93W` \x84\x01Q`@\x85\x01Q``\x86\x01Q_\x1Aa2\x85\x88\x82\x85\x85a4\xAFV[\x95P\x95P\x95PPPPa2\x9EV[PP\x81Q_\x91P`\x02\x90[\x92P\x92P\x92V[_\x82`\x03\x81\x11\x15a2\xB8Wa2\xB8a>\xF4V[\x03a2\xC1WPPV[`\x01\x82`\x03\x81\x11\x15a2\xD5Wa2\xD5a>\xF4V[\x03a3\x0CW`@Q\x7F\xF6E\xEE\xDF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01`@Q\x80\x91\x03\x90\xFD[`\x02\x82`\x03\x81\x11\x15a3 Wa3 a>\xF4V[\x03a3_W`@Q\x7F\xFC\xE6\x98\xF7\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x81\x01\x82\x90R`$\x01[`@Q\x80\x91\x03\x90\xFD[`\x03\x82`\x03\x81\x11\x15a3sWa3sa>\xF4V[\x03a\"\xC5W`@Q\x7F\xD7\x8B\xCE\x0C\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x81\x01\x82\x90R`$\x01a3VV[cNH{q_R\x80` R`$`\x1C\xFD[_a3\xCC` \x83\x01\x83a<\x16V[\x90P` \x82\x0156_a3\xE2`@\x86\x01\x86a=\x7FV[\x91P\x91P`@Q\x81\x83\x827_\x80\x83\x83\x87\x89Z\xF1a\x19DW=_\x80>=_\xFD[_6\x81a4\x11`@\x85\x01\x85a=\x7FV[\x90\x92P\x90P`\x04\x81\x10a4#W\x815\x92P[PP\x91\x90PV[_\x80` _\x84Q` \x86\x01_\x88Z\xF1\x80a4IW`@Q=_\x82>=\x81\xFD[PP_Q=\x91P\x81\x15a4`W\x80`\x01\x14\x15a4mV[`\x01`\x01`\xA0\x1B\x03\x84\x16;\x15[\x15a\x11\x9AW`@Q\x7FRt\xAF\xE7\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x85\x16`\x04\x82\x01R`$\x01a3VV[_\x80\x80\x7F\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF]WnsW\xA4P\x1D\xDF\xE9/Fh\x1B \xA0\x84\x11\x15a4\xE8WP_\x91P`\x03\x90P\x82a5\x8BV[`@\x80Q_\x80\x82R` \x82\x01\x80\x84R\x8A\x90R`\xFF\x89\x16\x92\x82\x01\x92\x90\x92R``\x81\x01\x87\x90R`\x80\x81\x01\x86\x90R`\x01\x90`\xA0\x01` `@Q` \x81\x03\x90\x80\x84\x03\x90\x85Z\xFA\x15\x80\x15a59W=_\x80>=_\xFD[PP`@Q\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0\x01Q\x91PP`\x01`\x01`\xA0\x1B\x03\x81\x16a5\x82WP_\x92P`\x01\x91P\x82\x90Pa5\x8BV[\x92P_\x91P\x81\x90P[\x94P\x94P\x94\x91PPV[_\x80_`@\x84\x86\x03\x12\x15a5\xA7W_\x80\xFD[\x835\x92P` \x84\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a5\xC5W_\x80\xFD[\x81\x86\x01\x91P\x86`\x1F\x83\x01\x12a5\xD8W_\x80\xFD[\x815\x81\x81\x11\x15a5\xE6W_\x80\xFD[\x87` \x82\x85\x01\x01\x11\x15a5\xF7W_\x80\xFD[` \x83\x01\x94P\x80\x93PPPP\x92P\x92P\x92V[_`\xA0\x82\x84\x03\x12\x15a6\x1AW_\x80\xFD[P\x91\x90PV[_`\xA0\x82\x84\x03\x12\x15a60W_\x80\xFD[a\x04\xD0\x83\x83a6\nV[`\x01`\x01`\xA0\x1B\x03\x81\x16\x81\x14a\x0C\x89W_\x80\xFD[\x805a6Y\x81a6:V[\x91\x90PV[_a\x02`\x82\x84\x03\x12\x15a6\x1AW_\x80\xFD[_\x80\x83`\x1F\x84\x01\x12a6\x7FW_\x80\xFD[P\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a6\x96W_\x80\xFD[` \x83\x01\x91P\x83` \x82`\x05\x1B\x85\x01\x01\x11\x15a6\xB0W_\x80\xFD[\x92P\x92\x90PV[_`@\x82\x84\x03\x12\x15a6\x1AW_\x80\xFD[_``\x82\x84\x03\x12\x15a6\x1AW_\x80\xFD[_\x80_\x80_\x80_\x80_a\x01\0\x8A\x8C\x03\x12\x15a6\xF0W_\x80\xFD[a6\xF9\x8Aa6NV[\x98P` \x8A\x015\x97P`@\x8A\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a7\x1CW_\x80\xFD[a7(\x8D\x83\x8E\x01a6^V[\x98P``\x8C\x015\x91P\x80\x82\x11\x15a7=W_\x80\xFD[a7I\x8D\x83\x8E\x01a6oV[\x90\x98P\x96P`\x80\x8C\x015\x91P\x80\x82\x11\x15a7aW_\x80\xFD[a7m\x8D\x83\x8E\x01a6\xB7V[\x95P`\xA0\x8C\x015\x91P\x80\x82\x11\x15a7\x82W_\x80\xFD[a7\x8E\x8D\x83\x8E\x01a6\xC7V[\x94P`\xC0\x8C\x015\x91P\x80\x82\x11\x15a7\xA3W_\x80\xFD[a7\xAF\x8D\x83\x8E\x01a6\xC7V[\x93P`\xE0\x8C\x015\x91P\x80\x82\x11\x15a7\xC4W_\x80\xFD[Pa7\xD1\x8C\x82\x8D\x01a6\xC7V[\x91PP\x92\x95\x98P\x92\x95\x98P\x92\x95\x98V[_\x80`@\x83\x85\x03\x12\x15a7\xF2W_\x80\xFD[\x825a7\xFD\x81a6:V[\x91P` \x83\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a8\x18W_\x80\xFD[a8$\x85\x82\x86\x01a6\xB7V[\x91PP\x92P\x92\x90PV[_a\x01`\x82\x84\x03\x12\x15a6\x1AW_\x80\xFD[_\x80_\x80_`\xA0\x86\x88\x03\x12\x15a8SW_\x80\xFD[\x855a8^\x81a6:V[\x94P` \x86\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a8zW_\x80\xFD[a8\x86\x89\x83\x8A\x01a8.V[\x95P`@\x88\x015\x91P\x80\x82\x11\x15a8\x9BW_\x80\xFD[a8\xA7\x89\x83\x8A\x01a6\xC7V[\x94P``\x88\x015\x93P`\x80\x88\x015\x91P\x80\x82\x11\x15a8\xC3W_\x80\xFD[Pa8\xD0\x88\x82\x89\x01a6\xC7V[\x91PP\x92\x95P\x92\x95\x90\x93PV[_` \x82\x84\x03\x12\x15a8\xEDW_\x80\xFD[P5\x91\x90PV[\x80\x15\x15\x81\x14a\x0C\x89W_\x80\xFD[_\x80_\x80_\x80`\xA0\x87\x89\x03\x12\x15a9\x16W_\x80\xFD[\x865a9!\x81a6:V[\x95P` \x87\x015a91\x81a6:V[\x94P`@\x87\x015a9A\x81a8\xF4V[\x93P``\x87\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a9]W_\x80\xFD[a9i\x8A\x83\x8B\x01a6^V[\x94P`\x80\x89\x015\x91P\x80\x82\x11\x15a9~W_\x80\xFD[Pa9\x8B\x89\x82\x8A\x01a6oV[\x97\x9A\x96\x99P\x94\x97P\x92\x95\x93\x94\x92PPPV[_\x80_``\x84\x86\x03\x12\x15a9\xAFW_\x80\xFD[\x835a9\xBA\x81a6:V[\x92P` \x84\x015\x91P`@\x84\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a9\xDCW_\x80\xFD[a9\xE8\x86\x82\x87\x01a6\xC7V[\x91PP\x92P\x92P\x92V[_` \x82\x84\x03\x12\x15a:\x02W_\x80\xFD[\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a:\x18W_\x80\xFD[a\x1B\xD3\x84\x82\x85\x01a8.V[_\x80_\x80_\x80_\x80`\xE0\x89\x8B\x03\x12\x15a:;W_\x80\xFD[a:D\x89a6NV[\x97P` \x89\x015\x96P`@\x89\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a:gW_\x80\xFD[a:s\x8C\x83\x8D\x01a6^V[\x97P``\x8B\x015\x91P\x80\x82\x11\x15a:\x88W_\x80\xFD[a:\x94\x8C\x83\x8D\x01a6oV[\x90\x97P\x95P`\x80\x8B\x015\x91P\x80\x82\x11\x15a:\xACW_\x80\xFD[a:\xB8\x8C\x83\x8D\x01a6\xB7V[\x94P`\xA0\x8B\x015\x91P\x80\x82\x11\x15a:\xCDW_\x80\xFD[a:\xD9\x8C\x83\x8D\x01a6\xC7V[\x93P`\xC0\x8B\x015\x91P\x80\x82\x11\x15a:\xEEW_\x80\xFD[Pa:\xFB\x8B\x82\x8C\x01a6\xC7V[\x91PP\x92\x95\x98P\x92\x95\x98\x90\x93\x96PV[_\x80_\x80_\x80`\xC0\x87\x89\x03\x12\x15a; W_\x80\xFD[a;)\x87a6NV[\x95P` \x87\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x11\x15a;EW_\x80\xFD[a;Q\x8A\x83\x8B\x01a8.V[\x96P`@\x89\x015\x91P\x80\x82\x11\x15a;fW_\x80\xFD[a;r\x8A\x83\x8B\x01a6\xC7V[\x95P``\x89\x015\x94P`\x80\x89\x015\x91P\x80\x82\x11\x15a;\x8EW_\x80\xFD[a;\x9A\x8A\x83\x8B\x01a6\xC7V[\x93P`\xA0\x89\x015\x91P\x80\x82\x11\x15a;\xAFW_\x80\xFD[Pa;\xBC\x89\x82\x8A\x01a6\xC7V[\x91PP\x92\x95P\x92\x95P\x92\x95V[_` \x82\x84\x03\x12\x15a;\xD9W_\x80\xFD[\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a;\xEFW_\x80\xFD[a\x1B\xD3\x84\x82\x85\x01a6^V[_` \x82\x84\x03\x12\x15a<\x0BW_\x80\xFD[\x81Qa\x04\xD0\x81a8\xF4V[_` \x82\x84\x03\x12\x15a<&W_\x80\xFD[\x815a\x04\xD0\x81a6:V[\x81\x83R\x81\x81` \x85\x017P_` \x82\x84\x01\x01R_` \x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`\x1F\x84\x01\x16\x84\x01\x01\x90P\x92\x91PPV[\x805e\xFF\xFF\xFF\xFF\xFF\xFF\x81\x16\x81\x14a6YW_\x80\xFD[_`\x01`\x01`\xA0\x1B\x03\x80\x87\x16\x83R\x80\x86\x16` \x84\x01RP\x83`@\x83\x01R`\x80``\x83\x01R\x825\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE1\x846\x03\x01\x81\x12a<\xE2W_\x80\xFD[\x83\x01` \x81\x01\x905g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a<\xFEW_\x80\xFD[\x806\x03\x82\x13\x15a=\x0CW_\x80\xFD[```\x80\x85\x01Ra=!`\xE0\x85\x01\x82\x84a<1V[\x91PPa=0` \x85\x01a<xV[e\xFF\xFF\xFF\xFF\xFF\xFF\x80\x82\x16`\xA0\x86\x01R\x80a=L`@\x88\x01a<xV[\x16`\xC0\x86\x01RPP\x80\x91PP\x95\x94PPPPPV[_` \x82\x84\x03\x12\x15a=qW_\x80\xFD[\x815`\x03\x81\x10a\x04\xD0W_\x80\xFD[_\x80\x835\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE1\x846\x03\x01\x81\x12a=\xB2W_\x80\xFD[\x83\x01\x805\x91Pg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11\x15a=\xCCW_\x80\xFD[` \x01\x91P6\x81\x90\x03\x82\x13\x15a6\xB0W_\x80\xFD[\x81\x83\x827_\x91\x01\x90\x81R\x91\x90PV[_\x80\x835\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE1\x846\x03\x01\x81\x12a>\"W_\x80\xFD[\x83\x01\x805\x91Pg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11\x15a><W_\x80\xFD[` \x01\x91P`\x05\x81\x90\x1B6\x03\x82\x13\x15a6\xB0W_\x80\xFD[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`2`\x04R`$_\xFD[_\x825\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xA1\x836\x03\x01\x81\x12a>\xB2W_\x80\xFD[\x91\x90\x91\x01\x92\x91PPV[\x80\x82\x01\x80\x82\x11\x15a.\xFAW\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x11`\x04R`$_\xFD[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`!`\x04R`$_\xFD[_` \x82\x84\x03\x12\x15a?1W_\x80\xFD[\x815`\x04\x81\x10a\x04\xD0W_\x80\xFD[\x83\x81R`@` \x82\x01R_a\x1C``@\x83\x01\x84\x86a<1V[_` \x82\x84\x03\x12\x15a?hW_\x80\xFD[\x81Q\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81\x16\x81\x14a\x04\xD0W_\x80\xFD[` \x81R_a\x1B\xD3` \x83\x01\x84\x86a<1V[_\x81Q_[\x81\x81\x10\x15a?\xC9W` \x81\x85\x01\x81\x01Q\x86\x83\x01R\x01a?\xAFV[P_\x93\x01\x92\x83RP\x90\x91\x90PV[_a\x1B\xD3a?\xE5\x83\x86a?\xAAV[\x84a?\xAAV[_a?\xF6\x82\x86a?\xAAV[\x93\x84RPP` \x82\x01R`@\x01\x91\x90PV[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x12`\x04R`$_\xFD[_`\xA0\x82\x01\x90P`\x01`\x01`\xA0\x1B\x03\x80\x84Q\x16\x83R\x80` \x85\x01Q\x16` \x84\x01R\x80`@\x85\x01Q\x16`@\x84\x01RP``\x83\x01Q``\x83\x01R`\x80\x83\x01Q`\x03\x81\x10a@\xA7W\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`!`\x04R`$_\xFD[\x80`\x80\x84\x01RP\x92\x91PPV[_` \x82\x84\x03\x12\x15a@\xC4W_\x80\xFD[PQ\x91\x90PV[_\x80\x85\x85\x11\x15a@\xD9W_\x80\xFD[\x83\x86\x11\x15a@\xE5W_\x80\xFD[PP\x82\x01\x93\x91\x90\x92\x03\x91PV[_\x80_\x80`\x80\x85\x87\x03\x12\x15aA\x05W_\x80\xFD[\x845aA\x10\x81a6:V[\x93P` \x85\x015aA \x81a6:V[\x92P`@\x85\x015\x91P``\x85\x015aA7\x81a8\xF4V[\x93\x96\x92\x95P\x90\x93PPV[_\x80_\x80`\x80\x85\x87\x03\x12\x15aAUW_\x80\xFD[\x845aA`\x81a6:V[\x93P` \x85\x015aAp\x81a6:V[\x92P`@\x85\x015aA\x80\x81a6:V[\x93\x96\x92\x95P\x92\x93``\x015\x92PPV[_\x80_\x80_\x80_`\xE0\x88\x8A\x03\x12\x15aA\xA6W_\x80\xFD[\x875aA\xB1\x81a6:V[\x96P` \x88\x015aA\xC1\x81a6:V[\x95P`@\x88\x015aA\xD1\x81a6:V[\x94P``\x88\x015aA\xE1\x81a6:V[\x93P`\x80\x88\x015\x92P`\xA0\x88\x015aA\xF8\x81a6:V[\x80\x92PP`\xC0\x88\x015\x90P\x92\x95\x98\x91\x94\x97P\x92\x95PV[_\x80_``\x84\x86\x03\x12\x15aB!W_\x80\xFD[\x835aB,\x81a6:V[\x92P` \x84\x015aB<\x81a6:V[\x92\x95\x92\x94PPP`@\x91\x90\x91\x015\x90V[_\x80_\x80_`\xA0\x86\x88\x03\x12\x15aBaW_\x80\xFD[\x855aBl\x81a6:V[\x94P` \x86\x015aB|\x81a6:V[\x93P`@\x86\x015aB\x8C\x81a6:V[\x92P``\x86\x015\x91P`\x80\x86\x015aB\xA3\x81a8\xF4V[\x80\x91PP\x92\x95P\x92\x95\x90\x93PV[_`\xFF\x83\x16\x80aB\xE8W\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x12`\x04R`$_\xFD[\x80`\xFF\x84\x16\x06\x91PP\x92\x91PPV\xFE\xA2dipfsX\"\x12 \x9B\xE5\x8A\xCA\xDA50a\xA2\xA2\x02\xCCp\x11\xF3\xC9\x13\x93\xE0\xF2\xE3\x05\xE9 $E\xB0B\xFCJL\xE6dsolcC\0\x08\x17\x003",
    );
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Custom error with signature `ECDSAInvalidSignature()` and selector `0xf645eedf`.
    ```solidity
    error ECDSAInvalidSignature();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct ECDSAInvalidSignature;
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
        impl ::core::convert::From<ECDSAInvalidSignature> for UnderlyingRustTuple<'_> {
            fn from(value: ECDSAInvalidSignature) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for ECDSAInvalidSignature {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for ECDSAInvalidSignature {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [246u8, 69u8, 238u8, 223u8];
            const SIGNATURE: &'static str = "ECDSAInvalidSignature()";

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
    /**Custom error with signature `ECDSAInvalidSignatureLength(uint256)` and selector `0xfce698f7`.
    ```solidity
    error ECDSAInvalidSignatureLength(uint256 length);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct ECDSAInvalidSignatureLength {
        #[allow(missing_docs)]
        pub length: alloy_sol_types::private::primitives::aliases::U256,
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
        impl ::core::convert::From<ECDSAInvalidSignatureLength> for UnderlyingRustTuple<'_> {
            fn from(value: ECDSAInvalidSignatureLength) -> Self {
                (value.length,)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for ECDSAInvalidSignatureLength {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self { length: tuple.0 }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for ECDSAInvalidSignatureLength {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [252u8, 230u8, 152u8, 247u8];
            const SIGNATURE: &'static str = "ECDSAInvalidSignatureLength(uint256)";

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
                        &self.length,
                    ),
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
    /**Custom error with signature `ECDSAInvalidSignatureS(bytes32)` and selector `0xd78bce0c`.
    ```solidity
    error ECDSAInvalidSignatureS(bytes32 s);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct ECDSAInvalidSignatureS {
        #[allow(missing_docs)]
        pub s: alloy_sol_types::private::FixedBytes<32>,
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
        impl ::core::convert::From<ECDSAInvalidSignatureS> for UnderlyingRustTuple<'_> {
            fn from(value: ECDSAInvalidSignatureS) -> Self {
                (value.s,)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for ECDSAInvalidSignatureS {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self { s: tuple.0 }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for ECDSAInvalidSignatureS {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [215u8, 139u8, 206u8, 12u8];
            const SIGNATURE: &'static str = "ECDSAInvalidSignatureS(bytes32)";

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
                    > as alloy_sol_types::SolType>::tokenize(&self.s),
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
    /**Custom error with signature `InvalidAmount()` and selector `0x2c5211c6`.
    ```solidity
    error InvalidAmount();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct InvalidAmount;
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
        impl ::core::convert::From<InvalidAmount> for UnderlyingRustTuple<'_> {
            fn from(value: InvalidAmount) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for InvalidAmount {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for InvalidAmount {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [44u8, 82u8, 17u8, 198u8];
            const SIGNATURE: &'static str = "InvalidAmount()";

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
    /**Custom error with signature `InvalidAsset()` and selector `0xc891add2`.
    ```solidity
    error InvalidAsset();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct InvalidAsset;
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
        impl ::core::convert::From<InvalidAsset> for UnderlyingRustTuple<'_> {
            fn from(value: InvalidAsset) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for InvalidAsset {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for InvalidAsset {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [200u8, 145u8, 173u8, 210u8];
            const SIGNATURE: &'static str = "InvalidAsset()";

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
    /**Custom error with signature `InvalidBaseTokenAmounts()` and selector `0xc04377d3`.
    ```solidity
    error InvalidBaseTokenAmounts();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct InvalidBaseTokenAmounts;
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
        impl ::core::convert::From<InvalidBaseTokenAmounts> for UnderlyingRustTuple<'_> {
            fn from(value: InvalidBaseTokenAmounts) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for InvalidBaseTokenAmounts {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for InvalidBaseTokenAmounts {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [192u8, 67u8, 119u8, 211u8];
            const SIGNATURE: &'static str = "InvalidBaseTokenAmounts()";

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
    /**Custom error with signature `InvalidDestination()` and selector `0xac6b05f5`.
    ```solidity
    error InvalidDestination();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct InvalidDestination;
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
        impl ::core::convert::From<InvalidDestination> for UnderlyingRustTuple<'_> {
            fn from(value: InvalidDestination) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for InvalidDestination {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for InvalidDestination {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [172u8, 107u8, 5u8, 245u8];
            const SIGNATURE: &'static str = "InvalidDestination()";

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
    /**Custom error with signature `InvalidEIP1271Signature()` and selector `0x5d52cbe3`.
    ```solidity
    error InvalidEIP1271Signature();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct InvalidEIP1271Signature;
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
        impl ::core::convert::From<InvalidEIP1271Signature> for UnderlyingRustTuple<'_> {
            fn from(value: InvalidEIP1271Signature) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for InvalidEIP1271Signature {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for InvalidEIP1271Signature {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [93u8, 82u8, 203u8, 227u8];
            const SIGNATURE: &'static str = "InvalidEIP1271Signature()";

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
    /**Custom error with signature `InvalidEIP712Signature()` and selector `0xb81d58e7`.
    ```solidity
    error InvalidEIP712Signature();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct InvalidEIP712Signature;
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
        impl ::core::convert::From<InvalidEIP712Signature> for UnderlyingRustTuple<'_> {
            fn from(value: InvalidEIP712Signature) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for InvalidEIP712Signature {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for InvalidEIP712Signature {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [184u8, 29u8, 88u8, 231u8];
            const SIGNATURE: &'static str = "InvalidEIP712Signature()";

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
    /**Custom error with signature `InvalidETHSignSignature()` and selector `0x644ae6c3`.
    ```solidity
    error InvalidETHSignSignature();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct InvalidETHSignSignature;
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
        impl ::core::convert::From<InvalidETHSignSignature> for UnderlyingRustTuple<'_> {
            fn from(value: InvalidETHSignSignature) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for InvalidETHSignSignature {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for InvalidETHSignSignature {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [100u8, 74u8, 230u8, 195u8];
            const SIGNATURE: &'static str = "InvalidETHSignSignature()";

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
    /**Custom error with signature `InvalidFillAmount()` and selector `0x94697444`.
    ```solidity
    error InvalidFillAmount();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct InvalidFillAmount;
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
        impl ::core::convert::From<InvalidFillAmount> for UnderlyingRustTuple<'_> {
            fn from(value: InvalidFillAmount) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for InvalidFillAmount {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for InvalidFillAmount {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [148u8, 105u8, 116u8, 68u8];
            const SIGNATURE: &'static str = "InvalidFillAmount()";

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
    /**Custom error with signature `InvalidHooksTarget()` and selector `0xc99e8872`.
    ```solidity
    error InvalidHooksTarget();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct InvalidHooksTarget;
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
        impl ::core::convert::From<InvalidHooksTarget> for UnderlyingRustTuple<'_> {
            fn from(value: InvalidHooksTarget) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for InvalidHooksTarget {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for InvalidHooksTarget {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [201u8, 158u8, 136u8, 114u8];
            const SIGNATURE: &'static str = "InvalidHooksTarget()";

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
    /**Custom error with signature `InvalidInteractionsBaseTokenAmounts()` and selector `0x4a55da20`.
    ```solidity
    error InvalidInteractionsBaseTokenAmounts();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct InvalidInteractionsBaseTokenAmounts;
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
        impl ::core::convert::From<InvalidInteractionsBaseTokenAmounts> for UnderlyingRustTuple<'_> {
            fn from(value: InvalidInteractionsBaseTokenAmounts) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for InvalidInteractionsBaseTokenAmounts {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for InvalidInteractionsBaseTokenAmounts {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [74u8, 85u8, 218u8, 32u8];
            const SIGNATURE: &'static str = "InvalidInteractionsBaseTokenAmounts()";

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
    /**Custom error with signature `InvalidInteractionsQuoteTokenAmounts()` and selector `0x77a59203`.
    ```solidity
    error InvalidInteractionsQuoteTokenAmounts();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct InvalidInteractionsQuoteTokenAmounts;
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
        impl ::core::convert::From<InvalidInteractionsQuoteTokenAmounts> for UnderlyingRustTuple<'_> {
            fn from(value: InvalidInteractionsQuoteTokenAmounts) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for InvalidInteractionsQuoteTokenAmounts {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for InvalidInteractionsQuoteTokenAmounts {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [119u8, 165u8, 146u8, 3u8];
            const SIGNATURE: &'static str = "InvalidInteractionsQuoteTokenAmounts()";

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
    /**Custom error with signature `InvalidLendingPoolInteraction()` and selector `0x0561d8b3`.
    ```solidity
    error InvalidLendingPoolInteraction();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct InvalidLendingPoolInteraction;
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
        impl ::core::convert::From<InvalidLendingPoolInteraction> for UnderlyingRustTuple<'_> {
            fn from(value: InvalidLendingPoolInteraction) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for InvalidLendingPoolInteraction {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for InvalidLendingPoolInteraction {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [5u8, 97u8, 216u8, 179u8];
            const SIGNATURE: &'static str = "InvalidLendingPoolInteraction()";

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
    /**Custom error with signature `InvalidQuoteTokenAmounts()` and selector `0x877630be`.
    ```solidity
    error InvalidQuoteTokenAmounts();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct InvalidQuoteTokenAmounts;
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
        impl ::core::convert::From<InvalidQuoteTokenAmounts> for UnderlyingRustTuple<'_> {
            fn from(value: InvalidQuoteTokenAmounts) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for InvalidQuoteTokenAmounts {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for InvalidQuoteTokenAmounts {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [135u8, 118u8, 48u8, 190u8];
            const SIGNATURE: &'static str = "InvalidQuoteTokenAmounts()";

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
    /**Custom error with signature `InvalidSignatureType()` and selector `0x60cd402d`.
    ```solidity
    error InvalidSignatureType();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct InvalidSignatureType;
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
        impl ::core::convert::From<InvalidSignatureType> for UnderlyingRustTuple<'_> {
            fn from(value: InvalidSignatureType) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for InvalidSignatureType {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for InvalidSignatureType {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [96u8, 205u8, 64u8, 45u8];
            const SIGNATURE: &'static str = "InvalidSignatureType()";

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
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [129u8, 94u8, 29u8, 100u8];
            const SIGNATURE: &'static str = "InvalidSigner()";

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
    /**Custom error with signature `InvalidSource()` and selector `0x8154374b`.
    ```solidity
    error InvalidSource();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct InvalidSource;
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
        impl ::core::convert::From<InvalidSource> for UnderlyingRustTuple<'_> {
            fn from(value: InvalidSource) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for InvalidSource {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for InvalidSource {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [129u8, 84u8, 55u8, 75u8];
            const SIGNATURE: &'static str = "InvalidSource()";

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
    /**Custom error with signature `NonceInvalid()` and selector `0xbc0da7d6`.
    ```solidity
    error NonceInvalid();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct NonceInvalid;
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
        impl ::core::convert::From<NonceInvalid> for UnderlyingRustTuple<'_> {
            fn from(value: NonceInvalid) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for NonceInvalid {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for NonceInvalid {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [188u8, 13u8, 167u8, 214u8];
            const SIGNATURE: &'static str = "NonceInvalid()";

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
    /**Custom error with signature `NotMaker()` and selector `0xb331e421`.
    ```solidity
    error NotMaker();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct NotMaker;
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
        impl ::core::convert::From<NotMaker> for UnderlyingRustTuple<'_> {
            fn from(value: NotMaker) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for NotMaker {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for NotMaker {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [179u8, 49u8, 228u8, 33u8];
            const SIGNATURE: &'static str = "NotMaker()";

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
    /**Custom error with signature `NotSolver()` and selector `0xc139eabd`.
    ```solidity
    error NotSolver();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct NotSolver;
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
        impl ::core::convert::From<NotSolver> for UnderlyingRustTuple<'_> {
            fn from(value: NotSolver) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for NotSolver {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for NotSolver {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [193u8, 57u8, 234u8, 189u8];
            const SIGNATURE: &'static str = "NotSolver()";

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
    /**Custom error with signature `OrderExpired()` and selector `0xc56873ba`.
    ```solidity
    error OrderExpired();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct OrderExpired;
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
        impl ::core::convert::From<OrderExpired> for UnderlyingRustTuple<'_> {
            fn from(value: OrderExpired) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for OrderExpired {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for OrderExpired {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [197u8, 104u8, 115u8, 186u8];
            const SIGNATURE: &'static str = "OrderExpired()";

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
    /**Custom error with signature `PartialFillNotSupported()` and selector `0x7d617bb3`.
    ```solidity
    error PartialFillNotSupported();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct PartialFillNotSupported;
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
        impl ::core::convert::From<PartialFillNotSupported> for UnderlyingRustTuple<'_> {
            fn from(value: PartialFillNotSupported) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for PartialFillNotSupported {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for PartialFillNotSupported {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [125u8, 97u8, 123u8, 179u8];
            const SIGNATURE: &'static str = "PartialFillNotSupported()";

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
    /**Custom error with signature `ReceiverNotManager()` and selector `0x79a1bff0`.
    ```solidity
    error ReceiverNotManager();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct ReceiverNotManager;
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
        impl ::core::convert::From<ReceiverNotManager> for UnderlyingRustTuple<'_> {
            fn from(value: ReceiverNotManager) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for ReceiverNotManager {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for ReceiverNotManager {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [121u8, 161u8, 191u8, 240u8];
            const SIGNATURE: &'static str = "ReceiverNotManager()";

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
    /**Custom error with signature `ReentrancyGuardReentrantCall()` and selector `0x3ee5aeb5`.
    ```solidity
    error ReentrancyGuardReentrantCall();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct ReentrancyGuardReentrantCall;
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
        impl ::core::convert::From<ReentrancyGuardReentrantCall> for UnderlyingRustTuple<'_> {
            fn from(value: ReentrancyGuardReentrantCall) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for ReentrancyGuardReentrantCall {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for ReentrancyGuardReentrantCall {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [62u8, 229u8, 174u8, 181u8];
            const SIGNATURE: &'static str = "ReentrancyGuardReentrantCall()";

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
    /**Custom error with signature `SafeERC20FailedOperation(address)` and selector `0x5274afe7`.
    ```solidity
    error SafeERC20FailedOperation(address token);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct SafeERC20FailedOperation {
        #[allow(missing_docs)]
        pub token: alloy_sol_types::private::Address,
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
        impl ::core::convert::From<SafeERC20FailedOperation> for UnderlyingRustTuple<'_> {
            fn from(value: SafeERC20FailedOperation) -> Self {
                (value.token,)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for SafeERC20FailedOperation {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self { token: tuple.0 }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for SafeERC20FailedOperation {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [82u8, 116u8, 175u8, 231u8];
            const SIGNATURE: &'static str = "SafeERC20FailedOperation(address)";

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
    /**Custom error with signature `SignatureIsExpired()` and selector `0x133df029`.
    ```solidity
    error SignatureIsExpired();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct SignatureIsExpired;
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
        impl ::core::convert::From<SignatureIsExpired> for UnderlyingRustTuple<'_> {
            fn from(value: SignatureIsExpired) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for SignatureIsExpired {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for SignatureIsExpired {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [19u8, 61u8, 240u8, 41u8];
            const SIGNATURE: &'static str = "SignatureIsExpired()";

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
    /**Custom error with signature `SignatureIsNotEmpty()` and selector `0x0e364efc`.
    ```solidity
    error SignatureIsNotEmpty();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct SignatureIsNotEmpty;
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
        impl ::core::convert::From<SignatureIsNotEmpty> for UnderlyingRustTuple<'_> {
            fn from(value: SignatureIsNotEmpty) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for SignatureIsNotEmpty {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for SignatureIsNotEmpty {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [14u8, 54u8, 78u8, 252u8];
            const SIGNATURE: &'static str = "SignatureIsNotEmpty()";

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
    /**Custom error with signature `UpdatedMakerAmountsTooLow()` and selector `0x711dbe4a`.
    ```solidity
    error UpdatedMakerAmountsTooLow();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct UpdatedMakerAmountsTooLow;
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
        impl ::core::convert::From<UpdatedMakerAmountsTooLow> for UnderlyingRustTuple<'_> {
            fn from(value: UpdatedMakerAmountsTooLow) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for UpdatedMakerAmountsTooLow {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for UpdatedMakerAmountsTooLow {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [113u8, 29u8, 190u8, 74u8];
            const SIGNATURE: &'static str = "UpdatedMakerAmountsTooLow()";

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
    /**Custom error with signature `ZeroMakerAmount()` and selector `0xb2f300d0`.
    ```solidity
    error ZeroMakerAmount();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct ZeroMakerAmount;
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
        impl ::core::convert::From<ZeroMakerAmount> for UnderlyingRustTuple<'_> {
            fn from(value: ZeroMakerAmount) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for ZeroMakerAmount {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for ZeroMakerAmount {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [178u8, 243u8, 0u8, 208u8];
            const SIGNATURE: &'static str = "ZeroMakerAmount()";

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
        use alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::SolEvent for Interaction {
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::FixedBytes<4>,
            );
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
            );

            const ANONYMOUS: bool = false;
            const SIGNATURE: &'static str = "Interaction(address,uint256,bytes4)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    237u8, 153u8, 130u8, 126u8, 251u8, 55u8, 1u8, 111u8, 34u8, 117u8, 249u8, 140u8,
                    75u8, 207u8, 113u8, 199u8, 85u8, 28u8, 117u8, 213u8, 158u8, 155u8, 69u8, 15u8,
                    121u8, 250u8, 50u8, 230u8, 11u8, 230u8, 114u8, 194u8,
                ]);

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
                out[0usize] = alloy_sol_types::abi::token::WordToken(Self::SIGNATURE_HASH);
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
    /**Event with signature `TradeOrder(string,address,address,address,address,uint256,uint256,address)` and selector `0x0fce007c38c6c8ed9e545b3a148095762738618f8c21b673222613e4d45734b6`.
    ```solidity
    event TradeOrder(string indexed rfqId, address trader, address effectiveTrader, address baseToken, address quoteToken, uint256 baseTokenAmount, uint256 quoteTokenAmount, address recipient);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct TradeOrder {
        #[allow(missing_docs)]
        pub rfqId: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub trader: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub effectiveTrader: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub baseToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub quoteToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub baseTokenAmount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub quoteTokenAmount: alloy_sol_types::private::primitives::aliases::U256,
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
        use alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::SolEvent for TradeOrder {
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Address,
            );
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::FixedBytes<32>,
            );

            const ANONYMOUS: bool = false;
            const SIGNATURE: &'static str =
                "TradeOrder(string,address,address,address,address,uint256,uint256,address)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    15u8, 206u8, 0u8, 124u8, 56u8, 198u8, 200u8, 237u8, 158u8, 84u8, 91u8, 58u8,
                    20u8, 128u8, 149u8, 118u8, 39u8, 56u8, 97u8, 143u8, 140u8, 33u8, 182u8, 115u8,
                    34u8, 38u8, 19u8, 228u8, 212u8, 87u8, 52u8, 182u8,
                ]);

            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    rfqId: topics.1,
                    trader: data.0,
                    effectiveTrader: data.1,
                    baseToken: data.2,
                    quoteToken: data.3,
                    baseTokenAmount: data.4,
                    quoteTokenAmount: data.5,
                    recipient: data.6,
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
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.trader,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.effectiveTrader,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.baseToken,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.quoteToken,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.baseTokenAmount,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.quoteTokenAmount,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.recipient,
                    ),
                )
            }

            #[inline]
            fn topics(&self) -> <Self::TopicList as alloy_sol_types::SolType>::RustType {
                (Self::SIGNATURE_HASH.into(), self.rfqId.clone())
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
                out[1usize] = <alloy_sol_types::sol_data::FixedBytes<
                    32,
                > as alloy_sol_types::EventTopic>::encode_topic(&self.rfqId);
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for TradeOrder {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }

            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&TradeOrder> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &TradeOrder) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    /**Constructor`.
    ```solidity
    constructor(address authenticator_, address repository_, address permit2_);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct constructorCall {
        #[allow(missing_docs)]
        pub authenticator_: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub repository_: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub permit2_: alloy_sol_types::private::Address,
    }
    const _: () = {
        use alloy_sol_types;
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
                    (value.authenticator_, value.repository_, value.permit2_)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for constructorCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        authenticator_: tuple.0,
                        repository_: tuple.1,
                        permit2_: tuple.2,
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
                        &self.authenticator_,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.repository_,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.permit2_,
                    ),
                )
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `AUTHENTICATOR()` and selector `0xc6186181`.
    ```solidity
    function AUTHENTICATOR() external view returns (address);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct AUTHENTICATORCall;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`AUTHENTICATOR()`](AUTHENTICATORCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct AUTHENTICATORReturn {
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
            impl ::core::convert::From<AUTHENTICATORCall> for UnderlyingRustTuple<'_> {
                fn from(value: AUTHENTICATORCall) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for AUTHENTICATORCall {
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
            impl ::core::convert::From<AUTHENTICATORReturn> for UnderlyingRustTuple<'_> {
                fn from(value: AUTHENTICATORReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for AUTHENTICATORReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for AUTHENTICATORCall {
            type Parameters<'a> = ();
            type Return = alloy_sol_types::private::Address;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [198u8, 24u8, 97u8, 129u8];
            const SIGNATURE: &'static str = "AUTHENTICATOR()";

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
                        let r: AUTHENTICATORReturn = r.into();
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
                    let r: AUTHENTICATORReturn = r.into();
                    r._0
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `BALANCE_MANAGER()` and selector `0x29bcdc95`.
    ```solidity
    function BALANCE_MANAGER() external view returns (address);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct BALANCE_MANAGERCall;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`BALANCE_MANAGER()`](BALANCE_MANAGERCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct BALANCE_MANAGERReturn {
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
            impl ::core::convert::From<BALANCE_MANAGERCall> for UnderlyingRustTuple<'_> {
                fn from(value: BALANCE_MANAGERCall) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for BALANCE_MANAGERCall {
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
            impl ::core::convert::From<BALANCE_MANAGERReturn> for UnderlyingRustTuple<'_> {
                fn from(value: BALANCE_MANAGERReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for BALANCE_MANAGERReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for BALANCE_MANAGERCall {
            type Parameters<'a> = ();
            type Return = alloy_sol_types::private::Address;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [41u8, 188u8, 220u8, 149u8];
            const SIGNATURE: &'static str = "BALANCE_MANAGER()";

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
                        let r: BALANCE_MANAGERReturn = r.into();
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
                    let r: BALANCE_MANAGERReturn = r.into();
                    r._0
                })
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
    ///Container type for the return parameters of the
    /// [`DOMAIN_SEPARATOR()`](DOMAIN_SEPARATORCall) function.
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
            impl ::core::convert::From<DOMAIN_SEPARATORCall> for UnderlyingRustTuple<'_> {
                fn from(value: DOMAIN_SEPARATORCall) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for DOMAIN_SEPARATORCall {
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
            fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<DOMAIN_SEPARATORReturn> for UnderlyingRustTuple<'_> {
                fn from(value: DOMAIN_SEPARATORReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for DOMAIN_SEPARATORReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for DOMAIN_SEPARATORCall {
            type Parameters<'a> = ();
            type Return = alloy_sol_types::private::FixedBytes<32>;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::FixedBytes<32>,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [54u8, 68u8, 229u8, 21u8];
            const SIGNATURE: &'static str = "DOMAIN_SEPARATOR()";

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
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence(data).map(
                    |r| {
                        let r: DOMAIN_SEPARATORReturn = r.into();
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
                    let r: DOMAIN_SEPARATORReturn = r.into();
                    r._0
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `REPOSITORY()` and selector `0x6f35d2d2`.
    ```solidity
    function REPOSITORY() external view returns (address);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct REPOSITORYCall;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`REPOSITORY()`](REPOSITORYCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct REPOSITORYReturn {
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
            impl ::core::convert::From<REPOSITORYCall> for UnderlyingRustTuple<'_> {
                fn from(value: REPOSITORYCall) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for REPOSITORYCall {
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
            impl ::core::convert::From<REPOSITORYReturn> for UnderlyingRustTuple<'_> {
                fn from(value: REPOSITORYReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for REPOSITORYReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for REPOSITORYCall {
            type Parameters<'a> = ();
            type Return = alloy_sol_types::private::Address;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [111u8, 53u8, 210u8, 210u8];
            const SIGNATURE: &'static str = "REPOSITORY()";

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
                        let r: REPOSITORYReturn = r.into();
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
                    let r: REPOSITORYReturn = r.into();
                    r._0
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `cancelLimitOrder(uint256)` and selector `0xa5cdc8fc`.
    ```solidity
    function cancelLimitOrder(uint256 nonce) external;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct cancelLimitOrderCall {
        #[allow(missing_docs)]
        pub nonce: alloy_sol_types::private::primitives::aliases::U256,
    }
    ///Container type for the return parameters of the
    /// [`cancelLimitOrder(uint256)`](cancelLimitOrderCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct cancelLimitOrderReturn {}
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
            impl ::core::convert::From<cancelLimitOrderCall> for UnderlyingRustTuple<'_> {
                fn from(value: cancelLimitOrderCall) -> Self {
                    (value.nonce,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for cancelLimitOrderCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { nonce: tuple.0 }
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
            impl ::core::convert::From<cancelLimitOrderReturn> for UnderlyingRustTuple<'_> {
                fn from(value: cancelLimitOrderReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for cancelLimitOrderReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl cancelLimitOrderReturn {
            fn _tokenize(
                &self,
            ) -> <cancelLimitOrderCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for cancelLimitOrderCall {
            type Parameters<'a> = (alloy_sol_types::sol_data::Uint<256>,);
            type Return = cancelLimitOrderReturn;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = ();
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [165u8, 205u8, 200u8, 252u8];
            const SIGNATURE: &'static str = "cancelLimitOrder(uint256)";

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
                        &self.nonce,
                    ),
                )
            }

            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                cancelLimitOrderReturn::_tokenize(ret)
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
    /**Function with signature `hashBaseTokenData((address,uint256,uint256,uint256,uint256))` and selector `0x875530ff`.
    ```solidity
    function hashBaseTokenData(ILiquoriceSettlement.BaseTokenData memory _baseTokenData) external pure returns (bytes32);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct hashBaseTokenDataCall {
        #[allow(missing_docs)]
        pub _baseTokenData:
            <ILiquoriceSettlement::BaseTokenData as alloy_sol_types::SolType>::RustType,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`hashBaseTokenData((address,uint256,uint256,uint256,
    /// uint256))`](hashBaseTokenDataCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct hashBaseTokenDataReturn {
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
        use alloy_sol_types;
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (ILiquoriceSettlement::BaseTokenData,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> =
                (<ILiquoriceSettlement::BaseTokenData as alloy_sol_types::SolType>::RustType,);
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
            impl ::core::convert::From<hashBaseTokenDataCall> for UnderlyingRustTuple<'_> {
                fn from(value: hashBaseTokenDataCall) -> Self {
                    (value._baseTokenData,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for hashBaseTokenDataCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        _baseTokenData: tuple.0,
                    }
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
            impl ::core::convert::From<hashBaseTokenDataReturn> for UnderlyingRustTuple<'_> {
                fn from(value: hashBaseTokenDataReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for hashBaseTokenDataReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for hashBaseTokenDataCall {
            type Parameters<'a> = (ILiquoriceSettlement::BaseTokenData,);
            type Return = alloy_sol_types::private::FixedBytes<32>;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::FixedBytes<32>,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [135u8, 85u8, 48u8, 255u8];
            const SIGNATURE: &'static str =
                "hashBaseTokenData((address,uint256,uint256,uint256,uint256))";

            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }

            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <ILiquoriceSettlement::BaseTokenData as alloy_sol_types::SolType>::tokenize(
                        &self._baseTokenData,
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
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence(data).map(
                    |r| {
                        let r: hashBaseTokenDataReturn = r.into();
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
                    let r: hashBaseTokenDataReturn = r.into();
                    r._0
                })
            }
        }
    };
    #[derive()]
    /**Function with signature `hashOrder((address,uint256,string,uint256,address,address,uint256,address,uint256,(address,uint256,uint256,uint256,uint256),(address,uint256,uint256,uint256,uint256)))` and selector `0xe242924e`.
    ```solidity
    function hashOrder(ILiquoriceSettlement.Order memory _order) external view returns (bytes32);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct hashOrderCall {
        #[allow(missing_docs)]
        pub _order: <ILiquoriceSettlement::Order as alloy_sol_types::SolType>::RustType,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`hashOrder((address,uint256,string,uint256,address,address,uint256,
    /// address,uint256,(address,uint256,uint256,uint256,uint256),(address,
    /// uint256,uint256,uint256,uint256)))`](hashOrderCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct hashOrderReturn {
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
        use alloy_sol_types;
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (ILiquoriceSettlement::Order,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> =
                (<ILiquoriceSettlement::Order as alloy_sol_types::SolType>::RustType,);
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
            impl ::core::convert::From<hashOrderCall> for UnderlyingRustTuple<'_> {
                fn from(value: hashOrderCall) -> Self {
                    (value._order,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for hashOrderCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _order: tuple.0 }
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
            impl ::core::convert::From<hashOrderReturn> for UnderlyingRustTuple<'_> {
                fn from(value: hashOrderReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for hashOrderReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for hashOrderCall {
            type Parameters<'a> = (ILiquoriceSettlement::Order,);
            type Return = alloy_sol_types::private::FixedBytes<32>;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::FixedBytes<32>,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [226u8, 66u8, 146u8, 78u8];
            const SIGNATURE: &'static str = "hashOrder((address,uint256,string,uint256,address,\
                                             address,uint256,address,uint256,(address,uint256,\
                                             uint256,uint256,uint256),(address,uint256,uint256,\
                                             uint256,uint256)))";

            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }

            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <ILiquoriceSettlement::Order as alloy_sol_types::SolType>::tokenize(
                        &self._order,
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
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence(data).map(
                    |r| {
                        let r: hashOrderReturn = r.into();
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
                    let r: hashOrderReturn = r.into();
                    r._0
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `hashQuoteTokenData((address,uint256,uint256,uint256,uint256))` and selector `0x4c9e03d3`.
    ```solidity
    function hashQuoteTokenData(ILiquoriceSettlement.QuoteTokenData memory _quoteTokenData) external pure returns (bytes32);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct hashQuoteTokenDataCall {
        #[allow(missing_docs)]
        pub _quoteTokenData:
            <ILiquoriceSettlement::QuoteTokenData as alloy_sol_types::SolType>::RustType,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`hashQuoteTokenData((address,uint256,uint256,uint256,
    /// uint256))`](hashQuoteTokenDataCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct hashQuoteTokenDataReturn {
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
        use alloy_sol_types;
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (ILiquoriceSettlement::QuoteTokenData,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> =
                (<ILiquoriceSettlement::QuoteTokenData as alloy_sol_types::SolType>::RustType,);
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
            impl ::core::convert::From<hashQuoteTokenDataCall> for UnderlyingRustTuple<'_> {
                fn from(value: hashQuoteTokenDataCall) -> Self {
                    (value._quoteTokenData,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for hashQuoteTokenDataCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        _quoteTokenData: tuple.0,
                    }
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
            impl ::core::convert::From<hashQuoteTokenDataReturn> for UnderlyingRustTuple<'_> {
                fn from(value: hashQuoteTokenDataReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for hashQuoteTokenDataReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for hashQuoteTokenDataCall {
            type Parameters<'a> = (ILiquoriceSettlement::QuoteTokenData,);
            type Return = alloy_sol_types::private::FixedBytes<32>;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::FixedBytes<32>,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [76u8, 158u8, 3u8, 211u8];
            const SIGNATURE: &'static str =
                "hashQuoteTokenData((address,uint256,uint256,uint256,uint256))";

            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }

            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <ILiquoriceSettlement::QuoteTokenData as alloy_sol_types::SolType>::tokenize(
                        &self._quoteTokenData,
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
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence(data).map(
                    |r| {
                        let r: hashQuoteTokenDataReturn = r.into();
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
                    let r: hashQuoteTokenDataReturn = r.into();
                    r._0
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `hashSingleOrder((string,uint256,address,address,address,address,uint256,uint256,uint256,uint256,address))` and selector `0xb11f1262`.
    ```solidity
    function hashSingleOrder(ILiquoriceSettlement.Single memory _order) external view returns (bytes32);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct hashSingleOrderCall {
        #[allow(missing_docs)]
        pub _order: <ILiquoriceSettlement::Single as alloy_sol_types::SolType>::RustType,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`hashSingleOrder((string,uint256,address,address,address,address,
    /// uint256,uint256,uint256,uint256,address))`](hashSingleOrderCall)
    /// function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct hashSingleOrderReturn {
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
        use alloy_sol_types;
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (ILiquoriceSettlement::Single,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> =
                (<ILiquoriceSettlement::Single as alloy_sol_types::SolType>::RustType,);
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
            impl ::core::convert::From<hashSingleOrderCall> for UnderlyingRustTuple<'_> {
                fn from(value: hashSingleOrderCall) -> Self {
                    (value._order,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for hashSingleOrderCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _order: tuple.0 }
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
            impl ::core::convert::From<hashSingleOrderReturn> for UnderlyingRustTuple<'_> {
                fn from(value: hashSingleOrderReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for hashSingleOrderReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for hashSingleOrderCall {
            type Parameters<'a> = (ILiquoriceSettlement::Single,);
            type Return = alloy_sol_types::private::FixedBytes<32>;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::FixedBytes<32>,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [177u8, 31u8, 18u8, 98u8];
            const SIGNATURE: &'static str = "hashSingleOrder((string,uint256,address,address,\
                                             address,address,uint256,uint256,uint256,uint256,\
                                             address))";

            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }

            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <ILiquoriceSettlement::Single as alloy_sol_types::SolType>::tokenize(
                        &self._order,
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
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence(data).map(
                    |r| {
                        let r: hashSingleOrderReturn = r.into();
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
                    let r: hashSingleOrderReturn = r.into();
                    r._0
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `isValidSignature(bytes32,bytes)` and selector `0x1626ba7e`.
    ```solidity
    function isValidSignature(bytes32 _hash, bytes memory _signature) external view returns (bytes4);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct isValidSignatureCall {
        #[allow(missing_docs)]
        pub _hash: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub _signature: alloy_sol_types::private::Bytes,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`isValidSignature(bytes32,bytes)`](isValidSignatureCall) function.
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
                    (value._hash, value._signature)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for isValidSignatureCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        _hash: tuple.0,
                        _signature: tuple.1,
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
            type Return = alloy_sol_types::private::FixedBytes<4>;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::FixedBytes<4>,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [22u8, 38u8, 186u8, 126u8];
            const SIGNATURE: &'static str = "isValidSignature(bytes32,bytes)";

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
                        &self._signature,
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
    #[derive()]
    /**Function with signature `settle(address,uint256,(address,uint256,string,uint256,address,address,uint256,address,uint256,(address,uint256,uint256,uint256,uint256),(address,uint256,uint256,uint256,uint256)),(address,uint256,bytes)[],((address,uint256,bytes)[],(address,uint256,bytes)[]),(uint8,uint8,bytes),(uint8,uint8,bytes))` and selector `0xcba673a7`.
    ```solidity
    function settle(address _signer, uint256 _filledTakerAmount, ILiquoriceSettlement.Order memory _order, GPv2Interaction.Data[] memory _interactions, GPv2Interaction.Hooks memory _hooks, Signature.TypedSignature memory _makerSignature, Signature.TypedSignature memory _takerSignature) external;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct settleCall {
        #[allow(missing_docs)]
        pub _signer: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub _filledTakerAmount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub _order: <ILiquoriceSettlement::Order as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub _interactions: alloy_sol_types::private::Vec<
            <GPv2Interaction::Data as alloy_sol_types::SolType>::RustType,
        >,
        #[allow(missing_docs)]
        pub _hooks: <GPv2Interaction::Hooks as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub _makerSignature: <Signature::TypedSignature as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub _takerSignature: <Signature::TypedSignature as alloy_sol_types::SolType>::RustType,
    }
    ///Container type for the return parameters of the
    /// [`settle(address,uint256,(address,uint256,string,uint256,address,
    /// address,uint256,address,uint256,(address,uint256,uint256,uint256,
    /// uint256),(address,uint256,uint256,uint256,uint256)),(address,uint256,
    /// bytes)[],((address,uint256,bytes)[],(address,uint256,bytes)[]),(uint8,
    /// uint8,bytes),(uint8,uint8,bytes))`](settleCall) function.
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
        use alloy_sol_types;
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                ILiquoriceSettlement::Order,
                alloy_sol_types::sol_data::Array<GPv2Interaction::Data>,
                GPv2Interaction::Hooks,
                Signature::TypedSignature,
                Signature::TypedSignature,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
                alloy_sol_types::private::primitives::aliases::U256,
                <ILiquoriceSettlement::Order as alloy_sol_types::SolType>::RustType,
                alloy_sol_types::private::Vec<
                    <GPv2Interaction::Data as alloy_sol_types::SolType>::RustType,
                >,
                <GPv2Interaction::Hooks as alloy_sol_types::SolType>::RustType,
                <Signature::TypedSignature as alloy_sol_types::SolType>::RustType,
                <Signature::TypedSignature as alloy_sol_types::SolType>::RustType,
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
            impl ::core::convert::From<settleCall> for UnderlyingRustTuple<'_> {
                fn from(value: settleCall) -> Self {
                    (
                        value._signer,
                        value._filledTakerAmount,
                        value._order,
                        value._interactions,
                        value._hooks,
                        value._makerSignature,
                        value._takerSignature,
                    )
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for settleCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        _signer: tuple.0,
                        _filledTakerAmount: tuple.1,
                        _order: tuple.2,
                        _interactions: tuple.3,
                        _hooks: tuple.4,
                        _makerSignature: tuple.5,
                        _takerSignature: tuple.6,
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
            fn _tokenize(&self) -> <settleCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for settleCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                ILiquoriceSettlement::Order,
                alloy_sol_types::sol_data::Array<GPv2Interaction::Data>,
                GPv2Interaction::Hooks,
                Signature::TypedSignature,
                Signature::TypedSignature,
            );
            type Return = settleReturn;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = ();
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [203u8, 166u8, 115u8, 167u8];
            const SIGNATURE: &'static str =
                "settle(address,uint256,(address,uint256,string,uint256,address,address,uint256,\
                 address,uint256,(address,uint256,uint256,uint256,uint256),(address,uint256,\
                 uint256,uint256,uint256)),(address,uint256,bytes)[],((address,uint256,bytes)[],\
                 (address,uint256,bytes)[]),(uint8,uint8,bytes),(uint8,uint8,bytes))";

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
                        &self._signer,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self._filledTakerAmount),
                    <ILiquoriceSettlement::Order as alloy_sol_types::SolType>::tokenize(
                        &self._order,
                    ),
                    <alloy_sol_types::sol_data::Array<
                        GPv2Interaction::Data,
                    > as alloy_sol_types::SolType>::tokenize(&self._interactions),
                    <GPv2Interaction::Hooks as alloy_sol_types::SolType>::tokenize(
                        &self._hooks,
                    ),
                    <Signature::TypedSignature as alloy_sol_types::SolType>::tokenize(
                        &self._makerSignature,
                    ),
                    <Signature::TypedSignature as alloy_sol_types::SolType>::tokenize(
                        &self._takerSignature,
                    ),
                )
            }

            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                settleReturn::_tokenize(ret)
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
    /**Function with signature `settleSingle(address,(string,uint256,address,address,address,address,uint256,uint256,uint256,uint256,address),(uint8,uint8,bytes),uint256,(uint8,uint8,bytes))` and selector `0x9935c868`.
    ```solidity
    function settleSingle(address _signer, ILiquoriceSettlement.Single memory _order, Signature.TypedSignature memory _makerSignature, uint256 _filledTakerAmount, Signature.TypedSignature memory _takerSignature) external payable;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct settleSingleCall {
        #[allow(missing_docs)]
        pub _signer: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub _order: <ILiquoriceSettlement::Single as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub _makerSignature: <Signature::TypedSignature as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub _filledTakerAmount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub _takerSignature: <Signature::TypedSignature as alloy_sol_types::SolType>::RustType,
    }
    ///Container type for the return parameters of the
    /// [`settleSingle(address,(string,uint256,address,address,address,address,
    /// uint256,uint256,uint256,uint256,address),(uint8,uint8,bytes),uint256,
    /// (uint8,uint8,bytes))`](settleSingleCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct settleSingleReturn {}
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
                alloy_sol_types::sol_data::Address,
                ILiquoriceSettlement::Single,
                Signature::TypedSignature,
                alloy_sol_types::sol_data::Uint<256>,
                Signature::TypedSignature,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
                <ILiquoriceSettlement::Single as alloy_sol_types::SolType>::RustType,
                <Signature::TypedSignature as alloy_sol_types::SolType>::RustType,
                alloy_sol_types::private::primitives::aliases::U256,
                <Signature::TypedSignature as alloy_sol_types::SolType>::RustType,
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
            impl ::core::convert::From<settleSingleCall> for UnderlyingRustTuple<'_> {
                fn from(value: settleSingleCall) -> Self {
                    (
                        value._signer,
                        value._order,
                        value._makerSignature,
                        value._filledTakerAmount,
                        value._takerSignature,
                    )
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for settleSingleCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        _signer: tuple.0,
                        _order: tuple.1,
                        _makerSignature: tuple.2,
                        _filledTakerAmount: tuple.3,
                        _takerSignature: tuple.4,
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
            impl ::core::convert::From<settleSingleReturn> for UnderlyingRustTuple<'_> {
                fn from(value: settleSingleReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for settleSingleReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl settleSingleReturn {
            fn _tokenize(&self) -> <settleSingleCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for settleSingleCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                ILiquoriceSettlement::Single,
                Signature::TypedSignature,
                alloy_sol_types::sol_data::Uint<256>,
                Signature::TypedSignature,
            );
            type Return = settleSingleReturn;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = ();
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [153u8, 53u8, 200u8, 104u8];
            const SIGNATURE: &'static str =
                "settleSingle(address,(string,uint256,address,address,address,address,uint256,\
                 uint256,uint256,uint256,address),(uint8,uint8,bytes),uint256,(uint8,uint8,bytes))";

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
                        &self._signer,
                    ),
                    <ILiquoriceSettlement::Single as alloy_sol_types::SolType>::tokenize(
                        &self._order,
                    ),
                    <Signature::TypedSignature as alloy_sol_types::SolType>::tokenize(
                        &self._makerSignature,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self._filledTakerAmount,
                    ),
                    <Signature::TypedSignature as alloy_sol_types::SolType>::tokenize(
                        &self._takerSignature,
                    ),
                )
            }

            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                settleSingleReturn::_tokenize(ret)
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
    /**Function with signature `settleSingleWithPermitsSignatures(address,(string,uint256,address,address,address,address,uint256,uint256,uint256,uint256,address),(uint8,uint8,bytes),uint256,(uint8,uint8,bytes),(bytes,uint48,uint48))` and selector `0xdb587728`.
    ```solidity
    function settleSingleWithPermitsSignatures(address _signer, ILiquoriceSettlement.Single memory _order, Signature.TypedSignature memory _makerSignature, uint256 _filledTakerAmount, Signature.TypedSignature memory _takerSignature, Signature.TakerPermitInfo memory _takerPermitInfo) external payable;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct settleSingleWithPermitsSignaturesCall {
        #[allow(missing_docs)]
        pub _signer: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub _order: <ILiquoriceSettlement::Single as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub _makerSignature: <Signature::TypedSignature as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub _filledTakerAmount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub _takerSignature: <Signature::TypedSignature as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub _takerPermitInfo: <Signature::TakerPermitInfo as alloy_sol_types::SolType>::RustType,
    }
    ///Container type for the return parameters of the
    /// [`settleSingleWithPermitsSignatures(address,(string,uint256,address,
    /// address,address,address,uint256,uint256,uint256,uint256,address),(uint8,
    /// uint8,bytes),uint256,(uint8,uint8,bytes),(bytes,uint48,
    /// uint48))`](settleSingleWithPermitsSignaturesCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct settleSingleWithPermitsSignaturesReturn {}
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
                alloy_sol_types::sol_data::Address,
                ILiquoriceSettlement::Single,
                Signature::TypedSignature,
                alloy_sol_types::sol_data::Uint<256>,
                Signature::TypedSignature,
                Signature::TakerPermitInfo,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
                <ILiquoriceSettlement::Single as alloy_sol_types::SolType>::RustType,
                <Signature::TypedSignature as alloy_sol_types::SolType>::RustType,
                alloy_sol_types::private::primitives::aliases::U256,
                <Signature::TypedSignature as alloy_sol_types::SolType>::RustType,
                <Signature::TakerPermitInfo as alloy_sol_types::SolType>::RustType,
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
            impl ::core::convert::From<settleSingleWithPermitsSignaturesCall> for UnderlyingRustTuple<'_> {
                fn from(value: settleSingleWithPermitsSignaturesCall) -> Self {
                    (
                        value._signer,
                        value._order,
                        value._makerSignature,
                        value._filledTakerAmount,
                        value._takerSignature,
                        value._takerPermitInfo,
                    )
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for settleSingleWithPermitsSignaturesCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        _signer: tuple.0,
                        _order: tuple.1,
                        _makerSignature: tuple.2,
                        _filledTakerAmount: tuple.3,
                        _takerSignature: tuple.4,
                        _takerPermitInfo: tuple.5,
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
            impl ::core::convert::From<settleSingleWithPermitsSignaturesReturn> for UnderlyingRustTuple<'_> {
                fn from(value: settleSingleWithPermitsSignaturesReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for settleSingleWithPermitsSignaturesReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl settleSingleWithPermitsSignaturesReturn {
            fn _tokenize(
                &self,
            ) -> <settleSingleWithPermitsSignaturesCall as alloy_sol_types::SolCall>::ReturnToken<'_>
            {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for settleSingleWithPermitsSignaturesCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                ILiquoriceSettlement::Single,
                Signature::TypedSignature,
                alloy_sol_types::sol_data::Uint<256>,
                Signature::TypedSignature,
                Signature::TakerPermitInfo,
            );
            type Return = settleSingleWithPermitsSignaturesReturn;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = ();
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [219u8, 88u8, 119u8, 40u8];
            const SIGNATURE: &'static str = "settleSingleWithPermitsSignatures(address,(string,\
                                             uint256,address,address,address,address,uint256,\
                                             uint256,uint256,uint256,address),(uint8,uint8,bytes),\
                                             uint256,(uint8,uint8,bytes),(bytes,uint48,uint48))";

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
                        &self._signer,
                    ),
                    <ILiquoriceSettlement::Single as alloy_sol_types::SolType>::tokenize(
                        &self._order,
                    ),
                    <Signature::TypedSignature as alloy_sol_types::SolType>::tokenize(
                        &self._makerSignature,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self._filledTakerAmount,
                    ),
                    <Signature::TypedSignature as alloy_sol_types::SolType>::tokenize(
                        &self._takerSignature,
                    ),
                    <Signature::TakerPermitInfo as alloy_sol_types::SolType>::tokenize(
                        &self._takerPermitInfo,
                    ),
                )
            }

            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                settleSingleWithPermitsSignaturesReturn::_tokenize(ret)
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
    /**Function with signature `settleWithPermitsSignatures(address,uint256,(address,uint256,string,uint256,address,address,uint256,address,uint256,(address,uint256,uint256,uint256,uint256),(address,uint256,uint256,uint256,uint256)),(address,uint256,bytes)[],((address,uint256,bytes)[],(address,uint256,bytes)[]),(uint8,uint8,bytes),(uint8,uint8,bytes),(bytes,uint48,uint48))` and selector `0x51d46815`.
    ```solidity
    function settleWithPermitsSignatures(address _signer, uint256 _filledTakerAmount, ILiquoriceSettlement.Order memory _order, GPv2Interaction.Data[] memory _interactions, GPv2Interaction.Hooks memory _hooks, Signature.TypedSignature memory _makerSignature, Signature.TypedSignature memory _takerSignature, Signature.TakerPermitInfo memory _takerPermitInfo) external payable;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct settleWithPermitsSignaturesCall {
        #[allow(missing_docs)]
        pub _signer: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub _filledTakerAmount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub _order: <ILiquoriceSettlement::Order as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub _interactions: alloy_sol_types::private::Vec<
            <GPv2Interaction::Data as alloy_sol_types::SolType>::RustType,
        >,
        #[allow(missing_docs)]
        pub _hooks: <GPv2Interaction::Hooks as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub _makerSignature: <Signature::TypedSignature as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub _takerSignature: <Signature::TypedSignature as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub _takerPermitInfo: <Signature::TakerPermitInfo as alloy_sol_types::SolType>::RustType,
    }
    ///Container type for the return parameters of the
    /// [`settleWithPermitsSignatures(address,uint256,(address,uint256,string,
    /// uint256,address,address,uint256,address,uint256,(address,uint256,
    /// uint256,uint256,uint256),(address,uint256,uint256,uint256,uint256)),
    /// (address,uint256,bytes)[],((address,uint256,bytes)[],(address,uint256,
    /// bytes)[]),(uint8,uint8,bytes),(uint8,uint8,bytes),(bytes,uint48,
    /// uint48))`](settleWithPermitsSignaturesCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct settleWithPermitsSignaturesReturn {}
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
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                ILiquoriceSettlement::Order,
                alloy_sol_types::sol_data::Array<GPv2Interaction::Data>,
                GPv2Interaction::Hooks,
                Signature::TypedSignature,
                Signature::TypedSignature,
                Signature::TakerPermitInfo,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
                alloy_sol_types::private::primitives::aliases::U256,
                <ILiquoriceSettlement::Order as alloy_sol_types::SolType>::RustType,
                alloy_sol_types::private::Vec<
                    <GPv2Interaction::Data as alloy_sol_types::SolType>::RustType,
                >,
                <GPv2Interaction::Hooks as alloy_sol_types::SolType>::RustType,
                <Signature::TypedSignature as alloy_sol_types::SolType>::RustType,
                <Signature::TypedSignature as alloy_sol_types::SolType>::RustType,
                <Signature::TakerPermitInfo as alloy_sol_types::SolType>::RustType,
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
            impl ::core::convert::From<settleWithPermitsSignaturesCall> for UnderlyingRustTuple<'_> {
                fn from(value: settleWithPermitsSignaturesCall) -> Self {
                    (
                        value._signer,
                        value._filledTakerAmount,
                        value._order,
                        value._interactions,
                        value._hooks,
                        value._makerSignature,
                        value._takerSignature,
                        value._takerPermitInfo,
                    )
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for settleWithPermitsSignaturesCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        _signer: tuple.0,
                        _filledTakerAmount: tuple.1,
                        _order: tuple.2,
                        _interactions: tuple.3,
                        _hooks: tuple.4,
                        _makerSignature: tuple.5,
                        _takerSignature: tuple.6,
                        _takerPermitInfo: tuple.7,
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
            impl ::core::convert::From<settleWithPermitsSignaturesReturn> for UnderlyingRustTuple<'_> {
                fn from(value: settleWithPermitsSignaturesReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for settleWithPermitsSignaturesReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl settleWithPermitsSignaturesReturn {
            fn _tokenize(
                &self,
            ) -> <settleWithPermitsSignaturesCall as alloy_sol_types::SolCall>::ReturnToken<'_>
            {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for settleWithPermitsSignaturesCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                ILiquoriceSettlement::Order,
                alloy_sol_types::sol_data::Array<GPv2Interaction::Data>,
                GPv2Interaction::Hooks,
                Signature::TypedSignature,
                Signature::TypedSignature,
                Signature::TakerPermitInfo,
            );
            type Return = settleWithPermitsSignaturesReturn;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = ();
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [81u8, 212u8, 104u8, 21u8];
            const SIGNATURE: &'static str =
                "settleWithPermitsSignatures(address,uint256,(address,uint256,string,uint256,\
                 address,address,uint256,address,uint256,(address,uint256,uint256,uint256,\
                 uint256),(address,uint256,uint256,uint256,uint256)),(address,uint256,bytes)[],\
                 ((address,uint256,bytes)[],(address,uint256,bytes)[]),(uint8,uint8,bytes),(uint8,\
                 uint8,bytes),(bytes,uint48,uint48))";

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
                        &self._signer,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self._filledTakerAmount),
                    <ILiquoriceSettlement::Order as alloy_sol_types::SolType>::tokenize(
                        &self._order,
                    ),
                    <alloy_sol_types::sol_data::Array<
                        GPv2Interaction::Data,
                    > as alloy_sol_types::SolType>::tokenize(&self._interactions),
                    <GPv2Interaction::Hooks as alloy_sol_types::SolType>::tokenize(
                        &self._hooks,
                    ),
                    <Signature::TypedSignature as alloy_sol_types::SolType>::tokenize(
                        &self._makerSignature,
                    ),
                    <Signature::TypedSignature as alloy_sol_types::SolType>::tokenize(
                        &self._takerSignature,
                    ),
                    <Signature::TakerPermitInfo as alloy_sol_types::SolType>::tokenize(
                        &self._takerPermitInfo,
                    ),
                )
            }

            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                settleWithPermitsSignaturesReturn::_tokenize(ret)
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
    /**Function with signature `validateHooks(address,((address,uint256,bytes)[],(address,uint256,bytes)[]))` and selector `0x5aa0e95d`.
    ```solidity
    function validateHooks(address _repository, GPv2Interaction.Hooks memory _hooks) external view;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct validateHooksCall {
        #[allow(missing_docs)]
        pub _repository: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub _hooks: <GPv2Interaction::Hooks as alloy_sol_types::SolType>::RustType,
    }
    ///Container type for the return parameters of the
    /// [`validateHooks(address,((address,uint256,bytes)[],(address,uint256,
    /// bytes)[]))`](validateHooksCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct validateHooksReturn {}
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
            type UnderlyingSolTuple<'a> =
                (alloy_sol_types::sol_data::Address, GPv2Interaction::Hooks);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
                <GPv2Interaction::Hooks as alloy_sol_types::SolType>::RustType,
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
            impl ::core::convert::From<validateHooksCall> for UnderlyingRustTuple<'_> {
                fn from(value: validateHooksCall) -> Self {
                    (value._repository, value._hooks)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for validateHooksCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        _repository: tuple.0,
                        _hooks: tuple.1,
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
            impl ::core::convert::From<validateHooksReturn> for UnderlyingRustTuple<'_> {
                fn from(value: validateHooksReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for validateHooksReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl validateHooksReturn {
            fn _tokenize(
                &self,
            ) -> <validateHooksCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for validateHooksCall {
            type Parameters<'a> = (alloy_sol_types::sol_data::Address, GPv2Interaction::Hooks);
            type Return = validateHooksReturn;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = ();
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [90u8, 160u8, 233u8, 93u8];
            const SIGNATURE: &'static str =
                "validateHooks(address,((address,uint256,bytes)[],(address,uint256,bytes)[]))";

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
                        &self._repository,
                    ),
                    <GPv2Interaction::Hooks as alloy_sol_types::SolType>::tokenize(&self._hooks),
                )
            }

            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                validateHooksReturn::_tokenize(ret)
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
    /**Function with signature `validateInteractions(address,address,bool,(address,uint256,string,uint256,address,address,uint256,address,uint256,(address,uint256,uint256,uint256,uint256),(address,uint256,uint256,uint256,uint256)),(address,uint256,bytes)[])` and selector `0xa7ab49bc`.
    ```solidity
    function validateInteractions(address _repository, address _signer, bool _isPartialFill, ILiquoriceSettlement.Order memory _order, GPv2Interaction.Data[] memory _interactions) external view;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct validateInteractionsCall {
        #[allow(missing_docs)]
        pub _repository: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub _signer: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub _isPartialFill: bool,
        #[allow(missing_docs)]
        pub _order: <ILiquoriceSettlement::Order as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub _interactions: alloy_sol_types::private::Vec<
            <GPv2Interaction::Data as alloy_sol_types::SolType>::RustType,
        >,
    }
    ///Container type for the return parameters of the
    /// [`validateInteractions(address,address,bool,(address,uint256,string,
    /// uint256,address,address,uint256,address,uint256,(address,uint256,
    /// uint256,uint256,uint256),(address,uint256,uint256,uint256,uint256)),
    /// (address,uint256,bytes)[])`](validateInteractionsCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct validateInteractionsReturn {}
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
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Bool,
                ILiquoriceSettlement::Order,
                alloy_sol_types::sol_data::Array<GPv2Interaction::Data>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
                alloy_sol_types::private::Address,
                bool,
                <ILiquoriceSettlement::Order as alloy_sol_types::SolType>::RustType,
                alloy_sol_types::private::Vec<
                    <GPv2Interaction::Data as alloy_sol_types::SolType>::RustType,
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
            impl ::core::convert::From<validateInteractionsCall> for UnderlyingRustTuple<'_> {
                fn from(value: validateInteractionsCall) -> Self {
                    (
                        value._repository,
                        value._signer,
                        value._isPartialFill,
                        value._order,
                        value._interactions,
                    )
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for validateInteractionsCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        _repository: tuple.0,
                        _signer: tuple.1,
                        _isPartialFill: tuple.2,
                        _order: tuple.3,
                        _interactions: tuple.4,
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
            impl ::core::convert::From<validateInteractionsReturn> for UnderlyingRustTuple<'_> {
                fn from(value: validateInteractionsReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for validateInteractionsReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl validateInteractionsReturn {
            fn _tokenize(
                &self,
            ) -> <validateInteractionsCall as alloy_sol_types::SolCall>::ReturnToken<'_>
            {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for validateInteractionsCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Bool,
                ILiquoriceSettlement::Order,
                alloy_sol_types::sol_data::Array<GPv2Interaction::Data>,
            );
            type Return = validateInteractionsReturn;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = ();
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [167u8, 171u8, 73u8, 188u8];
            const SIGNATURE: &'static str =
                "validateInteractions(address,address,bool,(address,uint256,string,uint256,\
                 address,address,uint256,address,uint256,(address,uint256,uint256,uint256,\
                 uint256),(address,uint256,uint256,uint256,uint256)),(address,uint256,bytes)[])";

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
                        &self._repository,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self._signer,
                    ),
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::tokenize(
                        &self._isPartialFill,
                    ),
                    <ILiquoriceSettlement::Order as alloy_sol_types::SolType>::tokenize(
                        &self._order,
                    ),
                    <alloy_sol_types::sol_data::Array<
                        GPv2Interaction::Data,
                    > as alloy_sol_types::SolType>::tokenize(&self._interactions),
                )
            }

            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                validateInteractionsReturn::_tokenize(ret)
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
    /**Function with signature `validateOrderAmounts((address,uint256,string,uint256,address,address,uint256,address,uint256,(address,uint256,uint256,uint256,uint256),(address,uint256,uint256,uint256,uint256)))` and selector `0xfa5cd56c`.
    ```solidity
    function validateOrderAmounts(ILiquoriceSettlement.Order memory _order) external pure;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct validateOrderAmountsCall {
        #[allow(missing_docs)]
        pub _order: <ILiquoriceSettlement::Order as alloy_sol_types::SolType>::RustType,
    }
    ///Container type for the return parameters of the
    /// [`validateOrderAmounts((address,uint256,string,uint256,address,address,
    /// uint256,address,uint256,(address,uint256,uint256,uint256,uint256),
    /// (address,uint256,uint256,uint256,uint256)))`](validateOrderAmountsCall)
    /// function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct validateOrderAmountsReturn {}
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
            type UnderlyingSolTuple<'a> = (ILiquoriceSettlement::Order,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> =
                (<ILiquoriceSettlement::Order as alloy_sol_types::SolType>::RustType,);
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
            impl ::core::convert::From<validateOrderAmountsCall> for UnderlyingRustTuple<'_> {
                fn from(value: validateOrderAmountsCall) -> Self {
                    (value._order,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for validateOrderAmountsCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _order: tuple.0 }
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
            impl ::core::convert::From<validateOrderAmountsReturn> for UnderlyingRustTuple<'_> {
                fn from(value: validateOrderAmountsReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for validateOrderAmountsReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl validateOrderAmountsReturn {
            fn _tokenize(
                &self,
            ) -> <validateOrderAmountsCall as alloy_sol_types::SolCall>::ReturnToken<'_>
            {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for validateOrderAmountsCall {
            type Parameters<'a> = (ILiquoriceSettlement::Order,);
            type Return = validateOrderAmountsReturn;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = ();
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [250u8, 92u8, 213u8, 108u8];
            const SIGNATURE: &'static str = "validateOrderAmounts((address,uint256,string,uint256,\
                                             address,address,uint256,address,uint256,(address,\
                                             uint256,uint256,uint256,uint256),(address,uint256,\
                                             uint256,uint256,uint256)))";

            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }

            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <ILiquoriceSettlement::Order as alloy_sol_types::SolType>::tokenize(
                        &self._order,
                    ),
                )
            }

            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                validateOrderAmountsReturn::_tokenize(ret)
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
    /**Function with signature `validateSignature(address,bytes32,(uint8,uint8,bytes))` and selector `0xae80c584`.
    ```solidity
    function validateSignature(address _validationAddress, bytes32 _hash, Signature.TypedSignature memory _signature) external view;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct validateSignatureCall {
        #[allow(missing_docs)]
        pub _validationAddress: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub _hash: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub _signature: <Signature::TypedSignature as alloy_sol_types::SolType>::RustType,
    }
    ///Container type for the return parameters of the
    /// [`validateSignature(address,bytes32,(uint8,uint8,
    /// bytes))`](validateSignatureCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct validateSignatureReturn {}
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
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::FixedBytes<32>,
                Signature::TypedSignature,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
                alloy_sol_types::private::FixedBytes<32>,
                <Signature::TypedSignature as alloy_sol_types::SolType>::RustType,
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
            impl ::core::convert::From<validateSignatureCall> for UnderlyingRustTuple<'_> {
                fn from(value: validateSignatureCall) -> Self {
                    (value._validationAddress, value._hash, value._signature)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for validateSignatureCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        _validationAddress: tuple.0,
                        _hash: tuple.1,
                        _signature: tuple.2,
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
            impl ::core::convert::From<validateSignatureReturn> for UnderlyingRustTuple<'_> {
                fn from(value: validateSignatureReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for validateSignatureReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl validateSignatureReturn {
            fn _tokenize(
                &self,
            ) -> <validateSignatureCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for validateSignatureCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::FixedBytes<32>,
                Signature::TypedSignature,
            );
            type Return = validateSignatureReturn;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = ();
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [174u8, 128u8, 197u8, 132u8];
            const SIGNATURE: &'static str =
                "validateSignature(address,bytes32,(uint8,uint8,bytes))";

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
                        &self._validationAddress,
                    ),
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self._hash),
                    <Signature::TypedSignature as alloy_sol_types::SolType>::tokenize(
                        &self._signature,
                    ),
                )
            }

            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                validateSignatureReturn::_tokenize(ret)
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
    ///Container for all the [`LiquoriceSettlement`](self) function calls.
    #[derive(Clone)]
    pub enum LiquoriceSettlementCalls {
        #[allow(missing_docs)]
        AUTHENTICATOR(AUTHENTICATORCall),
        #[allow(missing_docs)]
        BALANCE_MANAGER(BALANCE_MANAGERCall),
        #[allow(missing_docs)]
        DOMAIN_SEPARATOR(DOMAIN_SEPARATORCall),
        #[allow(missing_docs)]
        REPOSITORY(REPOSITORYCall),
        #[allow(missing_docs)]
        cancelLimitOrder(cancelLimitOrderCall),
        #[allow(missing_docs)]
        hashBaseTokenData(hashBaseTokenDataCall),
        #[allow(missing_docs)]
        hashOrder(hashOrderCall),
        #[allow(missing_docs)]
        hashQuoteTokenData(hashQuoteTokenDataCall),
        #[allow(missing_docs)]
        hashSingleOrder(hashSingleOrderCall),
        #[allow(missing_docs)]
        isValidSignature(isValidSignatureCall),
        #[allow(missing_docs)]
        settle(settleCall),
        #[allow(missing_docs)]
        settleSingle(settleSingleCall),
        #[allow(missing_docs)]
        settleSingleWithPermitsSignatures(settleSingleWithPermitsSignaturesCall),
        #[allow(missing_docs)]
        settleWithPermitsSignatures(settleWithPermitsSignaturesCall),
        #[allow(missing_docs)]
        validateHooks(validateHooksCall),
        #[allow(missing_docs)]
        validateInteractions(validateInteractionsCall),
        #[allow(missing_docs)]
        validateOrderAmounts(validateOrderAmountsCall),
        #[allow(missing_docs)]
        validateSignature(validateSignatureCall),
    }
    impl LiquoriceSettlementCalls {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the
        /// variants. No guarantees are made about the order of the
        /// selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 4usize]] = &[
            [22u8, 38u8, 186u8, 126u8],
            [41u8, 188u8, 220u8, 149u8],
            [54u8, 68u8, 229u8, 21u8],
            [76u8, 158u8, 3u8, 211u8],
            [81u8, 212u8, 104u8, 21u8],
            [90u8, 160u8, 233u8, 93u8],
            [111u8, 53u8, 210u8, 210u8],
            [135u8, 85u8, 48u8, 255u8],
            [153u8, 53u8, 200u8, 104u8],
            [165u8, 205u8, 200u8, 252u8],
            [167u8, 171u8, 73u8, 188u8],
            [174u8, 128u8, 197u8, 132u8],
            [177u8, 31u8, 18u8, 98u8],
            [198u8, 24u8, 97u8, 129u8],
            [203u8, 166u8, 115u8, 167u8],
            [219u8, 88u8, 119u8, 40u8],
            [226u8, 66u8, 146u8, 78u8],
            [250u8, 92u8, 213u8, 108u8],
        ];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <isValidSignatureCall as alloy_sol_types::SolCall>::SIGNATURE,
            <BALANCE_MANAGERCall as alloy_sol_types::SolCall>::SIGNATURE,
            <DOMAIN_SEPARATORCall as alloy_sol_types::SolCall>::SIGNATURE,
            <hashQuoteTokenDataCall as alloy_sol_types::SolCall>::SIGNATURE,
            <settleWithPermitsSignaturesCall as alloy_sol_types::SolCall>::SIGNATURE,
            <validateHooksCall as alloy_sol_types::SolCall>::SIGNATURE,
            <REPOSITORYCall as alloy_sol_types::SolCall>::SIGNATURE,
            <hashBaseTokenDataCall as alloy_sol_types::SolCall>::SIGNATURE,
            <settleSingleCall as alloy_sol_types::SolCall>::SIGNATURE,
            <cancelLimitOrderCall as alloy_sol_types::SolCall>::SIGNATURE,
            <validateInteractionsCall as alloy_sol_types::SolCall>::SIGNATURE,
            <validateSignatureCall as alloy_sol_types::SolCall>::SIGNATURE,
            <hashSingleOrderCall as alloy_sol_types::SolCall>::SIGNATURE,
            <AUTHENTICATORCall as alloy_sol_types::SolCall>::SIGNATURE,
            <settleCall as alloy_sol_types::SolCall>::SIGNATURE,
            <settleSingleWithPermitsSignaturesCall as alloy_sol_types::SolCall>::SIGNATURE,
            <hashOrderCall as alloy_sol_types::SolCall>::SIGNATURE,
            <validateOrderAmountsCall as alloy_sol_types::SolCall>::SIGNATURE,
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(isValidSignature),
            ::core::stringify!(BALANCE_MANAGER),
            ::core::stringify!(DOMAIN_SEPARATOR),
            ::core::stringify!(hashQuoteTokenData),
            ::core::stringify!(settleWithPermitsSignatures),
            ::core::stringify!(validateHooks),
            ::core::stringify!(REPOSITORY),
            ::core::stringify!(hashBaseTokenData),
            ::core::stringify!(settleSingle),
            ::core::stringify!(cancelLimitOrder),
            ::core::stringify!(validateInteractions),
            ::core::stringify!(validateSignature),
            ::core::stringify!(hashSingleOrder),
            ::core::stringify!(AUTHENTICATOR),
            ::core::stringify!(settle),
            ::core::stringify!(settleSingleWithPermitsSignatures),
            ::core::stringify!(hashOrder),
            ::core::stringify!(validateOrderAmounts),
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
    impl alloy_sol_types::SolInterface for LiquoriceSettlementCalls {
        const COUNT: usize = 18usize;
        const MIN_DATA_LENGTH: usize = 0usize;
        const NAME: &'static str = "LiquoriceSettlementCalls";

        #[inline]
        fn selector(&self) -> [u8; 4] {
            match self {
                Self::AUTHENTICATOR(_) => <AUTHENTICATORCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::BALANCE_MANAGER(_) => {
                    <BALANCE_MANAGERCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::DOMAIN_SEPARATOR(_) => {
                    <DOMAIN_SEPARATORCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::REPOSITORY(_) => <REPOSITORYCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::cancelLimitOrder(_) => {
                    <cancelLimitOrderCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::hashBaseTokenData(_) => {
                    <hashBaseTokenDataCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::hashOrder(_) => <hashOrderCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::hashQuoteTokenData(_) => {
                    <hashQuoteTokenDataCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::hashSingleOrder(_) => {
                    <hashSingleOrderCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::isValidSignature(_) => {
                    <isValidSignatureCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::settle(_) => <settleCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::settleSingle(_) => <settleSingleCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::settleSingleWithPermitsSignatures(_) => {
                    <settleSingleWithPermitsSignaturesCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::settleWithPermitsSignatures(_) => {
                    <settleWithPermitsSignaturesCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::validateHooks(_) => <validateHooksCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::validateInteractions(_) => {
                    <validateInteractionsCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::validateOrderAmounts(_) => {
                    <validateOrderAmountsCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::validateSignature(_) => {
                    <validateSignatureCall as alloy_sol_types::SolCall>::SELECTOR
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
            static DECODE_SHIMS: &[fn(
                &[u8],
            )
                -> alloy_sol_types::Result<LiquoriceSettlementCalls>] = &[
                {
                    fn isValidSignature(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <isValidSignatureCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(LiquoriceSettlementCalls::isValidSignature)
                    }
                    isValidSignature
                },
                {
                    fn BALANCE_MANAGER(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <BALANCE_MANAGERCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(LiquoriceSettlementCalls::BALANCE_MANAGER)
                    }
                    BALANCE_MANAGER
                },
                {
                    fn DOMAIN_SEPARATOR(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <DOMAIN_SEPARATORCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(LiquoriceSettlementCalls::DOMAIN_SEPARATOR)
                    }
                    DOMAIN_SEPARATOR
                },
                {
                    fn hashQuoteTokenData(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <hashQuoteTokenDataCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(LiquoriceSettlementCalls::hashQuoteTokenData)
                    }
                    hashQuoteTokenData
                },
                {
                    fn settleWithPermitsSignatures(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <settleWithPermitsSignaturesCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(LiquoriceSettlementCalls::settleWithPermitsSignatures)
                    }
                    settleWithPermitsSignatures
                },
                {
                    fn validateHooks(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <validateHooksCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(LiquoriceSettlementCalls::validateHooks)
                    }
                    validateHooks
                },
                {
                    fn REPOSITORY(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <REPOSITORYCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(LiquoriceSettlementCalls::REPOSITORY)
                    }
                    REPOSITORY
                },
                {
                    fn hashBaseTokenData(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <hashBaseTokenDataCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(LiquoriceSettlementCalls::hashBaseTokenData)
                    }
                    hashBaseTokenData
                },
                {
                    fn settleSingle(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <settleSingleCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(LiquoriceSettlementCalls::settleSingle)
                    }
                    settleSingle
                },
                {
                    fn cancelLimitOrder(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <cancelLimitOrderCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(LiquoriceSettlementCalls::cancelLimitOrder)
                    }
                    cancelLimitOrder
                },
                {
                    fn validateInteractions(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <validateInteractionsCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(LiquoriceSettlementCalls::validateInteractions)
                    }
                    validateInteractions
                },
                {
                    fn validateSignature(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <validateSignatureCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(LiquoriceSettlementCalls::validateSignature)
                    }
                    validateSignature
                },
                {
                    fn hashSingleOrder(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <hashSingleOrderCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(LiquoriceSettlementCalls::hashSingleOrder)
                    }
                    hashSingleOrder
                },
                {
                    fn AUTHENTICATOR(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <AUTHENTICATORCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(LiquoriceSettlementCalls::AUTHENTICATOR)
                    }
                    AUTHENTICATOR
                },
                {
                    fn settle(data: &[u8]) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <settleCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(LiquoriceSettlementCalls::settle)
                    }
                    settle
                },
                {
                    fn settleSingleWithPermitsSignatures(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <settleSingleWithPermitsSignaturesCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(
                                LiquoriceSettlementCalls::settleSingleWithPermitsSignatures,
                            )
                    }
                    settleSingleWithPermitsSignatures
                },
                {
                    fn hashOrder(data: &[u8]) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <hashOrderCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(LiquoriceSettlementCalls::hashOrder)
                    }
                    hashOrder
                },
                {
                    fn validateOrderAmounts(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <validateOrderAmountsCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(LiquoriceSettlementCalls::validateOrderAmounts)
                    }
                    validateOrderAmounts
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
                LiquoriceSettlementCalls,
            >] = &[
                {
                    fn isValidSignature(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <isValidSignatureCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(LiquoriceSettlementCalls::isValidSignature)
                    }
                    isValidSignature
                },
                {
                    fn BALANCE_MANAGER(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <BALANCE_MANAGERCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(LiquoriceSettlementCalls::BALANCE_MANAGER)
                    }
                    BALANCE_MANAGER
                },
                {
                    fn DOMAIN_SEPARATOR(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <DOMAIN_SEPARATORCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(LiquoriceSettlementCalls::DOMAIN_SEPARATOR)
                    }
                    DOMAIN_SEPARATOR
                },
                {
                    fn hashQuoteTokenData(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <hashQuoteTokenDataCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(LiquoriceSettlementCalls::hashQuoteTokenData)
                    }
                    hashQuoteTokenData
                },
                {
                    fn settleWithPermitsSignatures(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <settleWithPermitsSignaturesCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(LiquoriceSettlementCalls::settleWithPermitsSignatures)
                    }
                    settleWithPermitsSignatures
                },
                {
                    fn validateHooks(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <validateHooksCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(LiquoriceSettlementCalls::validateHooks)
                    }
                    validateHooks
                },
                {
                    fn REPOSITORY(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <REPOSITORYCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(LiquoriceSettlementCalls::REPOSITORY)
                    }
                    REPOSITORY
                },
                {
                    fn hashBaseTokenData(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <hashBaseTokenDataCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(LiquoriceSettlementCalls::hashBaseTokenData)
                    }
                    hashBaseTokenData
                },
                {
                    fn settleSingle(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <settleSingleCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(LiquoriceSettlementCalls::settleSingle)
                    }
                    settleSingle
                },
                {
                    fn cancelLimitOrder(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <cancelLimitOrderCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(LiquoriceSettlementCalls::cancelLimitOrder)
                    }
                    cancelLimitOrder
                },
                {
                    fn validateInteractions(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <validateInteractionsCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(LiquoriceSettlementCalls::validateInteractions)
                    }
                    validateInteractions
                },
                {
                    fn validateSignature(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <validateSignatureCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(LiquoriceSettlementCalls::validateSignature)
                    }
                    validateSignature
                },
                {
                    fn hashSingleOrder(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <hashSingleOrderCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(LiquoriceSettlementCalls::hashSingleOrder)
                    }
                    hashSingleOrder
                },
                {
                    fn AUTHENTICATOR(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <AUTHENTICATORCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(LiquoriceSettlementCalls::AUTHENTICATOR)
                    }
                    AUTHENTICATOR
                },
                {
                    fn settle(data: &[u8]) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <settleCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(LiquoriceSettlementCalls::settle)
                    }
                    settle
                },
                {
                    fn settleSingleWithPermitsSignatures(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <settleSingleWithPermitsSignaturesCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(
                                LiquoriceSettlementCalls::settleSingleWithPermitsSignatures,
                            )
                    }
                    settleSingleWithPermitsSignatures
                },
                {
                    fn hashOrder(data: &[u8]) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <hashOrderCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(LiquoriceSettlementCalls::hashOrder)
                    }
                    hashOrder
                },
                {
                    fn validateOrderAmounts(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementCalls> {
                        <validateOrderAmountsCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(LiquoriceSettlementCalls::validateOrderAmounts)
                    }
                    validateOrderAmounts
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
                Self::AUTHENTICATOR(inner) => {
                    <AUTHENTICATORCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::BALANCE_MANAGER(inner) => {
                    <BALANCE_MANAGERCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::DOMAIN_SEPARATOR(inner) => {
                    <DOMAIN_SEPARATORCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::REPOSITORY(inner) => {
                    <REPOSITORYCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::cancelLimitOrder(inner) => {
                    <cancelLimitOrderCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::hashBaseTokenData(inner) => {
                    <hashBaseTokenDataCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::hashOrder(inner) => {
                    <hashOrderCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::hashQuoteTokenData(inner) => {
                    <hashQuoteTokenDataCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::hashSingleOrder(inner) => {
                    <hashSingleOrderCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::isValidSignature(inner) => {
                    <isValidSignatureCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::settle(inner) => {
                    <settleCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::settleSingle(inner) => {
                    <settleSingleCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::settleSingleWithPermitsSignatures(inner) => {
                    <settleSingleWithPermitsSignaturesCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::settleWithPermitsSignatures(inner) => {
                    <settleWithPermitsSignaturesCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::validateHooks(inner) => {
                    <validateHooksCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::validateInteractions(inner) => {
                    <validateInteractionsCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::validateOrderAmounts(inner) => {
                    <validateOrderAmountsCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::validateSignature(inner) => {
                    <validateSignatureCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
            }
        }

        #[inline]
        fn abi_encode_raw(&self, out: &mut alloy_sol_types::private::Vec<u8>) {
            match self {
                Self::AUTHENTICATOR(inner) => {
                    <AUTHENTICATORCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::BALANCE_MANAGER(inner) => {
                    <BALANCE_MANAGERCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::DOMAIN_SEPARATOR(inner) => {
                    <DOMAIN_SEPARATORCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::REPOSITORY(inner) => {
                    <REPOSITORYCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::cancelLimitOrder(inner) => {
                    <cancelLimitOrderCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::hashBaseTokenData(inner) => {
                    <hashBaseTokenDataCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::hashOrder(inner) => {
                    <hashOrderCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::hashQuoteTokenData(inner) => {
                    <hashQuoteTokenDataCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::hashSingleOrder(inner) => {
                    <hashSingleOrderCall as alloy_sol_types::SolCall>::abi_encode_raw(
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
                Self::settle(inner) => {
                    <settleCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::settleSingle(inner) => {
                    <settleSingleCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::settleSingleWithPermitsSignatures(inner) => {
                    <settleSingleWithPermitsSignaturesCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::settleWithPermitsSignatures(inner) => {
                    <settleWithPermitsSignaturesCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::validateHooks(inner) => {
                    <validateHooksCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::validateInteractions(inner) => {
                    <validateInteractionsCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::validateOrderAmounts(inner) => {
                    <validateOrderAmountsCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::validateSignature(inner) => {
                    <validateSignatureCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
            }
        }
    }
    ///Container for all the [`LiquoriceSettlement`](self) custom errors.
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    pub enum LiquoriceSettlementErrors {
        #[allow(missing_docs)]
        ECDSAInvalidSignature(ECDSAInvalidSignature),
        #[allow(missing_docs)]
        ECDSAInvalidSignatureLength(ECDSAInvalidSignatureLength),
        #[allow(missing_docs)]
        ECDSAInvalidSignatureS(ECDSAInvalidSignatureS),
        #[allow(missing_docs)]
        InvalidAmount(InvalidAmount),
        #[allow(missing_docs)]
        InvalidAsset(InvalidAsset),
        #[allow(missing_docs)]
        InvalidBaseTokenAmounts(InvalidBaseTokenAmounts),
        #[allow(missing_docs)]
        InvalidDestination(InvalidDestination),
        #[allow(missing_docs)]
        InvalidEIP1271Signature(InvalidEIP1271Signature),
        #[allow(missing_docs)]
        InvalidEIP712Signature(InvalidEIP712Signature),
        #[allow(missing_docs)]
        InvalidETHSignSignature(InvalidETHSignSignature),
        #[allow(missing_docs)]
        InvalidFillAmount(InvalidFillAmount),
        #[allow(missing_docs)]
        InvalidHooksTarget(InvalidHooksTarget),
        #[allow(missing_docs)]
        InvalidInteractionsBaseTokenAmounts(InvalidInteractionsBaseTokenAmounts),
        #[allow(missing_docs)]
        InvalidInteractionsQuoteTokenAmounts(InvalidInteractionsQuoteTokenAmounts),
        #[allow(missing_docs)]
        InvalidLendingPoolInteraction(InvalidLendingPoolInteraction),
        #[allow(missing_docs)]
        InvalidQuoteTokenAmounts(InvalidQuoteTokenAmounts),
        #[allow(missing_docs)]
        InvalidSignatureType(InvalidSignatureType),
        #[allow(missing_docs)]
        InvalidSigner(InvalidSigner),
        #[allow(missing_docs)]
        InvalidSource(InvalidSource),
        #[allow(missing_docs)]
        NonceInvalid(NonceInvalid),
        #[allow(missing_docs)]
        NotMaker(NotMaker),
        #[allow(missing_docs)]
        NotSolver(NotSolver),
        #[allow(missing_docs)]
        OrderExpired(OrderExpired),
        #[allow(missing_docs)]
        PartialFillNotSupported(PartialFillNotSupported),
        #[allow(missing_docs)]
        ReceiverNotManager(ReceiverNotManager),
        #[allow(missing_docs)]
        ReentrancyGuardReentrantCall(ReentrancyGuardReentrantCall),
        #[allow(missing_docs)]
        SafeERC20FailedOperation(SafeERC20FailedOperation),
        #[allow(missing_docs)]
        SignatureIsExpired(SignatureIsExpired),
        #[allow(missing_docs)]
        SignatureIsNotEmpty(SignatureIsNotEmpty),
        #[allow(missing_docs)]
        UpdatedMakerAmountsTooLow(UpdatedMakerAmountsTooLow),
        #[allow(missing_docs)]
        ZeroMakerAmount(ZeroMakerAmount),
    }
    impl LiquoriceSettlementErrors {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the
        /// variants. No guarantees are made about the order of the
        /// selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 4usize]] = &[
            [5u8, 97u8, 216u8, 179u8],
            [14u8, 54u8, 78u8, 252u8],
            [19u8, 61u8, 240u8, 41u8],
            [44u8, 82u8, 17u8, 198u8],
            [62u8, 229u8, 174u8, 181u8],
            [74u8, 85u8, 218u8, 32u8],
            [82u8, 116u8, 175u8, 231u8],
            [93u8, 82u8, 203u8, 227u8],
            [96u8, 205u8, 64u8, 45u8],
            [100u8, 74u8, 230u8, 195u8],
            [113u8, 29u8, 190u8, 74u8],
            [119u8, 165u8, 146u8, 3u8],
            [121u8, 161u8, 191u8, 240u8],
            [125u8, 97u8, 123u8, 179u8],
            [129u8, 84u8, 55u8, 75u8],
            [129u8, 94u8, 29u8, 100u8],
            [135u8, 118u8, 48u8, 190u8],
            [148u8, 105u8, 116u8, 68u8],
            [172u8, 107u8, 5u8, 245u8],
            [178u8, 243u8, 0u8, 208u8],
            [179u8, 49u8, 228u8, 33u8],
            [184u8, 29u8, 88u8, 231u8],
            [188u8, 13u8, 167u8, 214u8],
            [192u8, 67u8, 119u8, 211u8],
            [193u8, 57u8, 234u8, 189u8],
            [197u8, 104u8, 115u8, 186u8],
            [200u8, 145u8, 173u8, 210u8],
            [201u8, 158u8, 136u8, 114u8],
            [215u8, 139u8, 206u8, 12u8],
            [246u8, 69u8, 238u8, 223u8],
            [252u8, 230u8, 152u8, 247u8],
        ];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <InvalidLendingPoolInteraction as alloy_sol_types::SolError>::SIGNATURE,
            <SignatureIsNotEmpty as alloy_sol_types::SolError>::SIGNATURE,
            <SignatureIsExpired as alloy_sol_types::SolError>::SIGNATURE,
            <InvalidAmount as alloy_sol_types::SolError>::SIGNATURE,
            <ReentrancyGuardReentrantCall as alloy_sol_types::SolError>::SIGNATURE,
            <InvalidInteractionsBaseTokenAmounts as alloy_sol_types::SolError>::SIGNATURE,
            <SafeERC20FailedOperation as alloy_sol_types::SolError>::SIGNATURE,
            <InvalidEIP1271Signature as alloy_sol_types::SolError>::SIGNATURE,
            <InvalidSignatureType as alloy_sol_types::SolError>::SIGNATURE,
            <InvalidETHSignSignature as alloy_sol_types::SolError>::SIGNATURE,
            <UpdatedMakerAmountsTooLow as alloy_sol_types::SolError>::SIGNATURE,
            <InvalidInteractionsQuoteTokenAmounts as alloy_sol_types::SolError>::SIGNATURE,
            <ReceiverNotManager as alloy_sol_types::SolError>::SIGNATURE,
            <PartialFillNotSupported as alloy_sol_types::SolError>::SIGNATURE,
            <InvalidSource as alloy_sol_types::SolError>::SIGNATURE,
            <InvalidSigner as alloy_sol_types::SolError>::SIGNATURE,
            <InvalidQuoteTokenAmounts as alloy_sol_types::SolError>::SIGNATURE,
            <InvalidFillAmount as alloy_sol_types::SolError>::SIGNATURE,
            <InvalidDestination as alloy_sol_types::SolError>::SIGNATURE,
            <ZeroMakerAmount as alloy_sol_types::SolError>::SIGNATURE,
            <NotMaker as alloy_sol_types::SolError>::SIGNATURE,
            <InvalidEIP712Signature as alloy_sol_types::SolError>::SIGNATURE,
            <NonceInvalid as alloy_sol_types::SolError>::SIGNATURE,
            <InvalidBaseTokenAmounts as alloy_sol_types::SolError>::SIGNATURE,
            <NotSolver as alloy_sol_types::SolError>::SIGNATURE,
            <OrderExpired as alloy_sol_types::SolError>::SIGNATURE,
            <InvalidAsset as alloy_sol_types::SolError>::SIGNATURE,
            <InvalidHooksTarget as alloy_sol_types::SolError>::SIGNATURE,
            <ECDSAInvalidSignatureS as alloy_sol_types::SolError>::SIGNATURE,
            <ECDSAInvalidSignature as alloy_sol_types::SolError>::SIGNATURE,
            <ECDSAInvalidSignatureLength as alloy_sol_types::SolError>::SIGNATURE,
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(InvalidLendingPoolInteraction),
            ::core::stringify!(SignatureIsNotEmpty),
            ::core::stringify!(SignatureIsExpired),
            ::core::stringify!(InvalidAmount),
            ::core::stringify!(ReentrancyGuardReentrantCall),
            ::core::stringify!(InvalidInteractionsBaseTokenAmounts),
            ::core::stringify!(SafeERC20FailedOperation),
            ::core::stringify!(InvalidEIP1271Signature),
            ::core::stringify!(InvalidSignatureType),
            ::core::stringify!(InvalidETHSignSignature),
            ::core::stringify!(UpdatedMakerAmountsTooLow),
            ::core::stringify!(InvalidInteractionsQuoteTokenAmounts),
            ::core::stringify!(ReceiverNotManager),
            ::core::stringify!(PartialFillNotSupported),
            ::core::stringify!(InvalidSource),
            ::core::stringify!(InvalidSigner),
            ::core::stringify!(InvalidQuoteTokenAmounts),
            ::core::stringify!(InvalidFillAmount),
            ::core::stringify!(InvalidDestination),
            ::core::stringify!(ZeroMakerAmount),
            ::core::stringify!(NotMaker),
            ::core::stringify!(InvalidEIP712Signature),
            ::core::stringify!(NonceInvalid),
            ::core::stringify!(InvalidBaseTokenAmounts),
            ::core::stringify!(NotSolver),
            ::core::stringify!(OrderExpired),
            ::core::stringify!(InvalidAsset),
            ::core::stringify!(InvalidHooksTarget),
            ::core::stringify!(ECDSAInvalidSignatureS),
            ::core::stringify!(ECDSAInvalidSignature),
            ::core::stringify!(ECDSAInvalidSignatureLength),
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
    impl alloy_sol_types::SolInterface for LiquoriceSettlementErrors {
        const COUNT: usize = 31usize;
        const MIN_DATA_LENGTH: usize = 0usize;
        const NAME: &'static str = "LiquoriceSettlementErrors";

        #[inline]
        fn selector(&self) -> [u8; 4] {
            match self {
                Self::ECDSAInvalidSignature(_) => {
                    <ECDSAInvalidSignature as alloy_sol_types::SolError>::SELECTOR
                }
                Self::ECDSAInvalidSignatureLength(_) => {
                    <ECDSAInvalidSignatureLength as alloy_sol_types::SolError>::SELECTOR
                }
                Self::ECDSAInvalidSignatureS(_) => {
                    <ECDSAInvalidSignatureS as alloy_sol_types::SolError>::SELECTOR
                }
                Self::InvalidAmount(_) => <InvalidAmount as alloy_sol_types::SolError>::SELECTOR,
                Self::InvalidAsset(_) => <InvalidAsset as alloy_sol_types::SolError>::SELECTOR,
                Self::InvalidBaseTokenAmounts(_) => {
                    <InvalidBaseTokenAmounts as alloy_sol_types::SolError>::SELECTOR
                }
                Self::InvalidDestination(_) => {
                    <InvalidDestination as alloy_sol_types::SolError>::SELECTOR
                }
                Self::InvalidEIP1271Signature(_) => {
                    <InvalidEIP1271Signature as alloy_sol_types::SolError>::SELECTOR
                }
                Self::InvalidEIP712Signature(_) => {
                    <InvalidEIP712Signature as alloy_sol_types::SolError>::SELECTOR
                }
                Self::InvalidETHSignSignature(_) => {
                    <InvalidETHSignSignature as alloy_sol_types::SolError>::SELECTOR
                }
                Self::InvalidFillAmount(_) => {
                    <InvalidFillAmount as alloy_sol_types::SolError>::SELECTOR
                }
                Self::InvalidHooksTarget(_) => {
                    <InvalidHooksTarget as alloy_sol_types::SolError>::SELECTOR
                }
                Self::InvalidInteractionsBaseTokenAmounts(_) => {
                    <InvalidInteractionsBaseTokenAmounts as alloy_sol_types::SolError>::SELECTOR
                }
                Self::InvalidInteractionsQuoteTokenAmounts(_) => {
                    <InvalidInteractionsQuoteTokenAmounts as alloy_sol_types::SolError>::SELECTOR
                }
                Self::InvalidLendingPoolInteraction(_) => {
                    <InvalidLendingPoolInteraction as alloy_sol_types::SolError>::SELECTOR
                }
                Self::InvalidQuoteTokenAmounts(_) => {
                    <InvalidQuoteTokenAmounts as alloy_sol_types::SolError>::SELECTOR
                }
                Self::InvalidSignatureType(_) => {
                    <InvalidSignatureType as alloy_sol_types::SolError>::SELECTOR
                }
                Self::InvalidSigner(_) => <InvalidSigner as alloy_sol_types::SolError>::SELECTOR,
                Self::InvalidSource(_) => <InvalidSource as alloy_sol_types::SolError>::SELECTOR,
                Self::NonceInvalid(_) => <NonceInvalid as alloy_sol_types::SolError>::SELECTOR,
                Self::NotMaker(_) => <NotMaker as alloy_sol_types::SolError>::SELECTOR,
                Self::NotSolver(_) => <NotSolver as alloy_sol_types::SolError>::SELECTOR,
                Self::OrderExpired(_) => <OrderExpired as alloy_sol_types::SolError>::SELECTOR,
                Self::PartialFillNotSupported(_) => {
                    <PartialFillNotSupported as alloy_sol_types::SolError>::SELECTOR
                }
                Self::ReceiverNotManager(_) => {
                    <ReceiverNotManager as alloy_sol_types::SolError>::SELECTOR
                }
                Self::ReentrancyGuardReentrantCall(_) => {
                    <ReentrancyGuardReentrantCall as alloy_sol_types::SolError>::SELECTOR
                }
                Self::SafeERC20FailedOperation(_) => {
                    <SafeERC20FailedOperation as alloy_sol_types::SolError>::SELECTOR
                }
                Self::SignatureIsExpired(_) => {
                    <SignatureIsExpired as alloy_sol_types::SolError>::SELECTOR
                }
                Self::SignatureIsNotEmpty(_) => {
                    <SignatureIsNotEmpty as alloy_sol_types::SolError>::SELECTOR
                }
                Self::UpdatedMakerAmountsTooLow(_) => {
                    <UpdatedMakerAmountsTooLow as alloy_sol_types::SolError>::SELECTOR
                }
                Self::ZeroMakerAmount(_) => {
                    <ZeroMakerAmount as alloy_sol_types::SolError>::SELECTOR
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
            static DECODE_SHIMS: &[fn(
                &[u8],
            )
                -> alloy_sol_types::Result<LiquoriceSettlementErrors>] = &[
                {
                    fn InvalidLendingPoolInteraction(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidLendingPoolInteraction as alloy_sol_types::SolError>::abi_decode_raw(
                                data,
                            )
                            .map(
                                LiquoriceSettlementErrors::InvalidLendingPoolInteraction,
                            )
                    }
                    InvalidLendingPoolInteraction
                },
                {
                    fn SignatureIsNotEmpty(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <SignatureIsNotEmpty as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(LiquoriceSettlementErrors::SignatureIsNotEmpty)
                    }
                    SignatureIsNotEmpty
                },
                {
                    fn SignatureIsExpired(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <SignatureIsExpired as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(LiquoriceSettlementErrors::SignatureIsExpired)
                    }
                    SignatureIsExpired
                },
                {
                    fn InvalidAmount(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidAmount as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(LiquoriceSettlementErrors::InvalidAmount)
                    }
                    InvalidAmount
                },
                {
                    fn ReentrancyGuardReentrantCall(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <ReentrancyGuardReentrantCall as alloy_sol_types::SolError>::abi_decode_raw(
                            data,
                        )
                        .map(LiquoriceSettlementErrors::ReentrancyGuardReentrantCall)
                    }
                    ReentrancyGuardReentrantCall
                },
                {
                    fn InvalidInteractionsBaseTokenAmounts(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidInteractionsBaseTokenAmounts as alloy_sol_types::SolError>::abi_decode_raw(
                                data,
                            )
                            .map(
                                LiquoriceSettlementErrors::InvalidInteractionsBaseTokenAmounts,
                            )
                    }
                    InvalidInteractionsBaseTokenAmounts
                },
                {
                    fn SafeERC20FailedOperation(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <SafeERC20FailedOperation as alloy_sol_types::SolError>::abi_decode_raw(
                            data,
                        )
                        .map(LiquoriceSettlementErrors::SafeERC20FailedOperation)
                    }
                    SafeERC20FailedOperation
                },
                {
                    fn InvalidEIP1271Signature(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidEIP1271Signature as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(LiquoriceSettlementErrors::InvalidEIP1271Signature)
                    }
                    InvalidEIP1271Signature
                },
                {
                    fn InvalidSignatureType(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidSignatureType as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(LiquoriceSettlementErrors::InvalidSignatureType)
                    }
                    InvalidSignatureType
                },
                {
                    fn InvalidETHSignSignature(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidETHSignSignature as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(LiquoriceSettlementErrors::InvalidETHSignSignature)
                    }
                    InvalidETHSignSignature
                },
                {
                    fn UpdatedMakerAmountsTooLow(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <UpdatedMakerAmountsTooLow as alloy_sol_types::SolError>::abi_decode_raw(
                            data,
                        )
                        .map(LiquoriceSettlementErrors::UpdatedMakerAmountsTooLow)
                    }
                    UpdatedMakerAmountsTooLow
                },
                {
                    fn InvalidInteractionsQuoteTokenAmounts(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidInteractionsQuoteTokenAmounts as alloy_sol_types::SolError>::abi_decode_raw(
                                data,
                            )
                            .map(
                                LiquoriceSettlementErrors::InvalidInteractionsQuoteTokenAmounts,
                            )
                    }
                    InvalidInteractionsQuoteTokenAmounts
                },
                {
                    fn ReceiverNotManager(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <ReceiverNotManager as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(LiquoriceSettlementErrors::ReceiverNotManager)
                    }
                    ReceiverNotManager
                },
                {
                    fn PartialFillNotSupported(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <PartialFillNotSupported as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(LiquoriceSettlementErrors::PartialFillNotSupported)
                    }
                    PartialFillNotSupported
                },
                {
                    fn InvalidSource(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidSource as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(LiquoriceSettlementErrors::InvalidSource)
                    }
                    InvalidSource
                },
                {
                    fn InvalidSigner(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidSigner as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(LiquoriceSettlementErrors::InvalidSigner)
                    }
                    InvalidSigner
                },
                {
                    fn InvalidQuoteTokenAmounts(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidQuoteTokenAmounts as alloy_sol_types::SolError>::abi_decode_raw(
                            data,
                        )
                        .map(LiquoriceSettlementErrors::InvalidQuoteTokenAmounts)
                    }
                    InvalidQuoteTokenAmounts
                },
                {
                    fn InvalidFillAmount(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidFillAmount as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(LiquoriceSettlementErrors::InvalidFillAmount)
                    }
                    InvalidFillAmount
                },
                {
                    fn InvalidDestination(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidDestination as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(LiquoriceSettlementErrors::InvalidDestination)
                    }
                    InvalidDestination
                },
                {
                    fn ZeroMakerAmount(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <ZeroMakerAmount as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(LiquoriceSettlementErrors::ZeroMakerAmount)
                    }
                    ZeroMakerAmount
                },
                {
                    fn NotMaker(data: &[u8]) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <NotMaker as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(LiquoriceSettlementErrors::NotMaker)
                    }
                    NotMaker
                },
                {
                    fn InvalidEIP712Signature(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidEIP712Signature as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(LiquoriceSettlementErrors::InvalidEIP712Signature)
                    }
                    InvalidEIP712Signature
                },
                {
                    fn NonceInvalid(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <NonceInvalid as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(LiquoriceSettlementErrors::NonceInvalid)
                    }
                    NonceInvalid
                },
                {
                    fn InvalidBaseTokenAmounts(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidBaseTokenAmounts as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(LiquoriceSettlementErrors::InvalidBaseTokenAmounts)
                    }
                    InvalidBaseTokenAmounts
                },
                {
                    fn NotSolver(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <NotSolver as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(LiquoriceSettlementErrors::NotSolver)
                    }
                    NotSolver
                },
                {
                    fn OrderExpired(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <OrderExpired as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(LiquoriceSettlementErrors::OrderExpired)
                    }
                    OrderExpired
                },
                {
                    fn InvalidAsset(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidAsset as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(LiquoriceSettlementErrors::InvalidAsset)
                    }
                    InvalidAsset
                },
                {
                    fn InvalidHooksTarget(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidHooksTarget as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(LiquoriceSettlementErrors::InvalidHooksTarget)
                    }
                    InvalidHooksTarget
                },
                {
                    fn ECDSAInvalidSignatureS(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <ECDSAInvalidSignatureS as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(LiquoriceSettlementErrors::ECDSAInvalidSignatureS)
                    }
                    ECDSAInvalidSignatureS
                },
                {
                    fn ECDSAInvalidSignature(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <ECDSAInvalidSignature as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(LiquoriceSettlementErrors::ECDSAInvalidSignature)
                    }
                    ECDSAInvalidSignature
                },
                {
                    fn ECDSAInvalidSignatureLength(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <ECDSAInvalidSignatureLength as alloy_sol_types::SolError>::abi_decode_raw(
                            data,
                        )
                        .map(LiquoriceSettlementErrors::ECDSAInvalidSignatureLength)
                    }
                    ECDSAInvalidSignatureLength
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
                LiquoriceSettlementErrors,
            >] = &[
                {
                    fn InvalidLendingPoolInteraction(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidLendingPoolInteraction as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(
                                LiquoriceSettlementErrors::InvalidLendingPoolInteraction,
                            )
                    }
                    InvalidLendingPoolInteraction
                },
                {
                    fn SignatureIsNotEmpty(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <SignatureIsNotEmpty as alloy_sol_types::SolError>::abi_decode_raw_validate(
                            data,
                        )
                        .map(LiquoriceSettlementErrors::SignatureIsNotEmpty)
                    }
                    SignatureIsNotEmpty
                },
                {
                    fn SignatureIsExpired(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <SignatureIsExpired as alloy_sol_types::SolError>::abi_decode_raw_validate(
                            data,
                        )
                        .map(LiquoriceSettlementErrors::SignatureIsExpired)
                    }
                    SignatureIsExpired
                },
                {
                    fn InvalidAmount(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidAmount as alloy_sol_types::SolError>::abi_decode_raw_validate(data)
                            .map(LiquoriceSettlementErrors::InvalidAmount)
                    }
                    InvalidAmount
                },
                {
                    fn ReentrancyGuardReentrantCall(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <ReentrancyGuardReentrantCall as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(LiquoriceSettlementErrors::ReentrancyGuardReentrantCall)
                    }
                    ReentrancyGuardReentrantCall
                },
                {
                    fn InvalidInteractionsBaseTokenAmounts(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidInteractionsBaseTokenAmounts as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(
                                LiquoriceSettlementErrors::InvalidInteractionsBaseTokenAmounts,
                            )
                    }
                    InvalidInteractionsBaseTokenAmounts
                },
                {
                    fn SafeERC20FailedOperation(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <SafeERC20FailedOperation as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(LiquoriceSettlementErrors::SafeERC20FailedOperation)
                    }
                    SafeERC20FailedOperation
                },
                {
                    fn InvalidEIP1271Signature(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidEIP1271Signature as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(LiquoriceSettlementErrors::InvalidEIP1271Signature)
                    }
                    InvalidEIP1271Signature
                },
                {
                    fn InvalidSignatureType(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidSignatureType as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(LiquoriceSettlementErrors::InvalidSignatureType)
                    }
                    InvalidSignatureType
                },
                {
                    fn InvalidETHSignSignature(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidETHSignSignature as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(LiquoriceSettlementErrors::InvalidETHSignSignature)
                    }
                    InvalidETHSignSignature
                },
                {
                    fn UpdatedMakerAmountsTooLow(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <UpdatedMakerAmountsTooLow as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(LiquoriceSettlementErrors::UpdatedMakerAmountsTooLow)
                    }
                    UpdatedMakerAmountsTooLow
                },
                {
                    fn InvalidInteractionsQuoteTokenAmounts(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidInteractionsQuoteTokenAmounts as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(
                                LiquoriceSettlementErrors::InvalidInteractionsQuoteTokenAmounts,
                            )
                    }
                    InvalidInteractionsQuoteTokenAmounts
                },
                {
                    fn ReceiverNotManager(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <ReceiverNotManager as alloy_sol_types::SolError>::abi_decode_raw_validate(
                            data,
                        )
                        .map(LiquoriceSettlementErrors::ReceiverNotManager)
                    }
                    ReceiverNotManager
                },
                {
                    fn PartialFillNotSupported(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <PartialFillNotSupported as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(LiquoriceSettlementErrors::PartialFillNotSupported)
                    }
                    PartialFillNotSupported
                },
                {
                    fn InvalidSource(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidSource as alloy_sol_types::SolError>::abi_decode_raw_validate(data)
                            .map(LiquoriceSettlementErrors::InvalidSource)
                    }
                    InvalidSource
                },
                {
                    fn InvalidSigner(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidSigner as alloy_sol_types::SolError>::abi_decode_raw_validate(data)
                            .map(LiquoriceSettlementErrors::InvalidSigner)
                    }
                    InvalidSigner
                },
                {
                    fn InvalidQuoteTokenAmounts(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidQuoteTokenAmounts as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(LiquoriceSettlementErrors::InvalidQuoteTokenAmounts)
                    }
                    InvalidQuoteTokenAmounts
                },
                {
                    fn InvalidFillAmount(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidFillAmount as alloy_sol_types::SolError>::abi_decode_raw_validate(
                            data,
                        )
                        .map(LiquoriceSettlementErrors::InvalidFillAmount)
                    }
                    InvalidFillAmount
                },
                {
                    fn InvalidDestination(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidDestination as alloy_sol_types::SolError>::abi_decode_raw_validate(
                            data,
                        )
                        .map(LiquoriceSettlementErrors::InvalidDestination)
                    }
                    InvalidDestination
                },
                {
                    fn ZeroMakerAmount(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <ZeroMakerAmount as alloy_sol_types::SolError>::abi_decode_raw_validate(
                            data,
                        )
                        .map(LiquoriceSettlementErrors::ZeroMakerAmount)
                    }
                    ZeroMakerAmount
                },
                {
                    fn NotMaker(data: &[u8]) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <NotMaker as alloy_sol_types::SolError>::abi_decode_raw_validate(data)
                            .map(LiquoriceSettlementErrors::NotMaker)
                    }
                    NotMaker
                },
                {
                    fn InvalidEIP712Signature(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidEIP712Signature as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(LiquoriceSettlementErrors::InvalidEIP712Signature)
                    }
                    InvalidEIP712Signature
                },
                {
                    fn NonceInvalid(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <NonceInvalid as alloy_sol_types::SolError>::abi_decode_raw_validate(data)
                            .map(LiquoriceSettlementErrors::NonceInvalid)
                    }
                    NonceInvalid
                },
                {
                    fn InvalidBaseTokenAmounts(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidBaseTokenAmounts as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(LiquoriceSettlementErrors::InvalidBaseTokenAmounts)
                    }
                    InvalidBaseTokenAmounts
                },
                {
                    fn NotSolver(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <NotSolver as alloy_sol_types::SolError>::abi_decode_raw_validate(data)
                            .map(LiquoriceSettlementErrors::NotSolver)
                    }
                    NotSolver
                },
                {
                    fn OrderExpired(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <OrderExpired as alloy_sol_types::SolError>::abi_decode_raw_validate(data)
                            .map(LiquoriceSettlementErrors::OrderExpired)
                    }
                    OrderExpired
                },
                {
                    fn InvalidAsset(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidAsset as alloy_sol_types::SolError>::abi_decode_raw_validate(data)
                            .map(LiquoriceSettlementErrors::InvalidAsset)
                    }
                    InvalidAsset
                },
                {
                    fn InvalidHooksTarget(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <InvalidHooksTarget as alloy_sol_types::SolError>::abi_decode_raw_validate(
                            data,
                        )
                        .map(LiquoriceSettlementErrors::InvalidHooksTarget)
                    }
                    InvalidHooksTarget
                },
                {
                    fn ECDSAInvalidSignatureS(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <ECDSAInvalidSignatureS as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(LiquoriceSettlementErrors::ECDSAInvalidSignatureS)
                    }
                    ECDSAInvalidSignatureS
                },
                {
                    fn ECDSAInvalidSignature(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <ECDSAInvalidSignature as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(LiquoriceSettlementErrors::ECDSAInvalidSignature)
                    }
                    ECDSAInvalidSignature
                },
                {
                    fn ECDSAInvalidSignatureLength(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<LiquoriceSettlementErrors> {
                        <ECDSAInvalidSignatureLength as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(LiquoriceSettlementErrors::ECDSAInvalidSignatureLength)
                    }
                    ECDSAInvalidSignatureLength
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
                Self::ECDSAInvalidSignature(inner) => {
                    <ECDSAInvalidSignature as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::ECDSAInvalidSignatureLength(inner) => {
                    <ECDSAInvalidSignatureLength as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::ECDSAInvalidSignatureS(inner) => {
                    <ECDSAInvalidSignatureS as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::InvalidAmount(inner) => {
                    <InvalidAmount as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
                Self::InvalidAsset(inner) => {
                    <InvalidAsset as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
                Self::InvalidBaseTokenAmounts(inner) => {
                    <InvalidBaseTokenAmounts as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::InvalidDestination(inner) => {
                    <InvalidDestination as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::InvalidEIP1271Signature(inner) => {
                    <InvalidEIP1271Signature as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::InvalidEIP712Signature(inner) => {
                    <InvalidEIP712Signature as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::InvalidETHSignSignature(inner) => {
                    <InvalidETHSignSignature as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::InvalidFillAmount(inner) => {
                    <InvalidFillAmount as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::InvalidHooksTarget(inner) => {
                    <InvalidHooksTarget as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::InvalidInteractionsBaseTokenAmounts(inner) => {
                    <InvalidInteractionsBaseTokenAmounts as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::InvalidInteractionsQuoteTokenAmounts(inner) => {
                    <InvalidInteractionsQuoteTokenAmounts as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::InvalidLendingPoolInteraction(inner) => {
                    <InvalidLendingPoolInteraction as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::InvalidQuoteTokenAmounts(inner) => {
                    <InvalidQuoteTokenAmounts as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::InvalidSignatureType(inner) => {
                    <InvalidSignatureType as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::InvalidSigner(inner) => {
                    <InvalidSigner as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
                Self::InvalidSource(inner) => {
                    <InvalidSource as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
                Self::NonceInvalid(inner) => {
                    <NonceInvalid as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
                Self::NotMaker(inner) => {
                    <NotMaker as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
                Self::NotSolver(inner) => {
                    <NotSolver as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
                Self::OrderExpired(inner) => {
                    <OrderExpired as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
                Self::PartialFillNotSupported(inner) => {
                    <PartialFillNotSupported as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::ReceiverNotManager(inner) => {
                    <ReceiverNotManager as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::ReentrancyGuardReentrantCall(inner) => {
                    <ReentrancyGuardReentrantCall as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::SafeERC20FailedOperation(inner) => {
                    <SafeERC20FailedOperation as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::SignatureIsExpired(inner) => {
                    <SignatureIsExpired as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::SignatureIsNotEmpty(inner) => {
                    <SignatureIsNotEmpty as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::UpdatedMakerAmountsTooLow(inner) => {
                    <UpdatedMakerAmountsTooLow as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::ZeroMakerAmount(inner) => {
                    <ZeroMakerAmount as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
            }
        }

        #[inline]
        fn abi_encode_raw(&self, out: &mut alloy_sol_types::private::Vec<u8>) {
            match self {
                Self::ECDSAInvalidSignature(inner) => {
                    <ECDSAInvalidSignature as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::ECDSAInvalidSignatureLength(inner) => {
                    <ECDSAInvalidSignatureLength as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::ECDSAInvalidSignatureS(inner) => {
                    <ECDSAInvalidSignatureS as alloy_sol_types::SolError>::abi_encode_raw(
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
                Self::InvalidAsset(inner) => {
                    <InvalidAsset as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::InvalidBaseTokenAmounts(inner) => {
                    <InvalidBaseTokenAmounts as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::InvalidDestination(inner) => {
                    <InvalidDestination as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::InvalidEIP1271Signature(inner) => {
                    <InvalidEIP1271Signature as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::InvalidEIP712Signature(inner) => {
                    <InvalidEIP712Signature as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::InvalidETHSignSignature(inner) => {
                    <InvalidETHSignSignature as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::InvalidFillAmount(inner) => {
                    <InvalidFillAmount as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::InvalidHooksTarget(inner) => {
                    <InvalidHooksTarget as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::InvalidInteractionsBaseTokenAmounts(inner) => {
                    <InvalidInteractionsBaseTokenAmounts as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::InvalidInteractionsQuoteTokenAmounts(inner) => {
                    <InvalidInteractionsQuoteTokenAmounts as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::InvalidLendingPoolInteraction(inner) => {
                    <InvalidLendingPoolInteraction as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::InvalidQuoteTokenAmounts(inner) => {
                    <InvalidQuoteTokenAmounts as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::InvalidSignatureType(inner) => {
                    <InvalidSignatureType as alloy_sol_types::SolError>::abi_encode_raw(
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
                Self::InvalidSource(inner) => {
                    <InvalidSource as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::NonceInvalid(inner) => {
                    <NonceInvalid as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::NotMaker(inner) => {
                    <NotMaker as alloy_sol_types::SolError>::abi_encode_raw(inner, out)
                }
                Self::NotSolver(inner) => {
                    <NotSolver as alloy_sol_types::SolError>::abi_encode_raw(inner, out)
                }
                Self::OrderExpired(inner) => {
                    <OrderExpired as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::PartialFillNotSupported(inner) => {
                    <PartialFillNotSupported as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::ReceiverNotManager(inner) => {
                    <ReceiverNotManager as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::ReentrancyGuardReentrantCall(inner) => {
                    <ReentrancyGuardReentrantCall as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::SafeERC20FailedOperation(inner) => {
                    <SafeERC20FailedOperation as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::SignatureIsExpired(inner) => {
                    <SignatureIsExpired as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::SignatureIsNotEmpty(inner) => {
                    <SignatureIsNotEmpty as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::UpdatedMakerAmountsTooLow(inner) => {
                    <UpdatedMakerAmountsTooLow as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::ZeroMakerAmount(inner) => {
                    <ZeroMakerAmount as alloy_sol_types::SolError>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
            }
        }
    }
    ///Container for all the [`LiquoriceSettlement`](self) events.
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    pub enum LiquoriceSettlementEvents {
        #[allow(missing_docs)]
        Interaction(Interaction),
        #[allow(missing_docs)]
        TradeOrder(TradeOrder),
    }
    impl LiquoriceSettlementEvents {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the
        /// variants. No guarantees are made about the order of the
        /// selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 32usize]] = &[
            [
                15u8, 206u8, 0u8, 124u8, 56u8, 198u8, 200u8, 237u8, 158u8, 84u8, 91u8, 58u8, 20u8,
                128u8, 149u8, 118u8, 39u8, 56u8, 97u8, 143u8, 140u8, 33u8, 182u8, 115u8, 34u8,
                38u8, 19u8, 228u8, 212u8, 87u8, 52u8, 182u8,
            ],
            [
                237u8, 153u8, 130u8, 126u8, 251u8, 55u8, 1u8, 111u8, 34u8, 117u8, 249u8, 140u8,
                75u8, 207u8, 113u8, 199u8, 85u8, 28u8, 117u8, 213u8, 158u8, 155u8, 69u8, 15u8,
                121u8, 250u8, 50u8, 230u8, 11u8, 230u8, 114u8, 194u8,
            ],
        ];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <TradeOrder as alloy_sol_types::SolEvent>::SIGNATURE,
            <Interaction as alloy_sol_types::SolEvent>::SIGNATURE,
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(TradeOrder),
            ::core::stringify!(Interaction),
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
    impl alloy_sol_types::SolEventInterface for LiquoriceSettlementEvents {
        const COUNT: usize = 2usize;
        const NAME: &'static str = "LiquoriceSettlementEvents";

        fn decode_raw_log(
            topics: &[alloy_sol_types::Word],
            data: &[u8],
        ) -> alloy_sol_types::Result<Self> {
            match topics.first().copied() {
                Some(<Interaction as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <Interaction as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::Interaction)
                }
                Some(<TradeOrder as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <TradeOrder as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::TradeOrder)
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
    impl alloy_sol_types::private::IntoLogData for LiquoriceSettlementEvents {
        fn to_log_data(&self) -> alloy_sol_types::private::LogData {
            match self {
                Self::Interaction(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::TradeOrder(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
            }
        }

        fn into_log_data(self) -> alloy_sol_types::private::LogData {
            match self {
                Self::Interaction(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::TradeOrder(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
            }
        }
    }
    use alloy_contract;
    /**Creates a new wrapper around an on-chain [`LiquoriceSettlement`](self) contract instance.

    See the [wrapper's documentation](`LiquoriceSettlementInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> LiquoriceSettlementInstance<P, N> {
        LiquoriceSettlementInstance::<P, N>::new(address, __provider)
    }
    /**Deploys this contract using the given `provider` and constructor arguments, if any.

    Returns a new instance of the contract, if the deployment was successful.

    For more fine-grained control over the deployment process, use [`deploy_builder`] instead.*/
    #[inline]
    pub fn deploy<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>(
        __provider: P,
        authenticator_: alloy_sol_types::private::Address,
        repository_: alloy_sol_types::private::Address,
        permit2_: alloy_sol_types::private::Address,
    ) -> impl ::core::future::Future<Output = alloy_contract::Result<LiquoriceSettlementInstance<P, N>>>
    {
        LiquoriceSettlementInstance::<P, N>::deploy(
            __provider,
            authenticator_,
            repository_,
            permit2_,
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
        authenticator_: alloy_sol_types::private::Address,
        repository_: alloy_sol_types::private::Address,
        permit2_: alloy_sol_types::private::Address,
    ) -> alloy_contract::RawCallBuilder<P, N> {
        LiquoriceSettlementInstance::<P, N>::deploy_builder(
            __provider,
            authenticator_,
            repository_,
            permit2_,
        )
    }
    /**A [`LiquoriceSettlement`](self) instance.

    Contains type-safe methods for interacting with an on-chain instance of the
    [`LiquoriceSettlement`](self) contract located at a given `address`, using a given
    provider `P`.

    If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
    documentation on how to provide it), the `deploy` and `deploy_builder` methods can
    be used to deploy a new instance of the contract.

    See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct LiquoriceSettlementInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for LiquoriceSettlementInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("LiquoriceSettlementInstance")
                .field(&self.address)
                .finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        LiquoriceSettlementInstance<P, N>
    {
        /**Creates a new wrapper around an on-chain [`LiquoriceSettlement`](self) contract instance.

        See the [wrapper's documentation](`LiquoriceSettlementInstance`) for more details.*/
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
            authenticator_: alloy_sol_types::private::Address,
            repository_: alloy_sol_types::private::Address,
            permit2_: alloy_sol_types::private::Address,
        ) -> alloy_contract::Result<LiquoriceSettlementInstance<P, N>> {
            let call_builder =
                Self::deploy_builder(__provider, authenticator_, repository_, permit2_);
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
            repository_: alloy_sol_types::private::Address,
            permit2_: alloy_sol_types::private::Address,
        ) -> alloy_contract::RawCallBuilder<P, N> {
            alloy_contract::RawCallBuilder::new_raw_deploy(
                __provider,
                [
                    &BYTECODE[..],
                    &alloy_sol_types::SolConstructor::abi_encode(&constructorCall {
                        authenticator_,
                        repository_,
                        permit2_,
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
    impl<P: ::core::clone::Clone, N> LiquoriceSettlementInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned
        /// provider.
        #[inline]
        pub fn with_cloned_provider(self) -> LiquoriceSettlementInstance<P, N> {
            LiquoriceSettlementInstance {
                address: self.address,
                provider: ::core::clone::Clone::clone(&self.provider),
                _network: ::core::marker::PhantomData,
            }
        }
    }
    /// Function calls.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        LiquoriceSettlementInstance<P, N>
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

        ///Creates a new call builder for the [`AUTHENTICATOR`] function.
        pub fn AUTHENTICATOR(&self) -> alloy_contract::SolCallBuilder<&P, AUTHENTICATORCall, N> {
            self.call_builder(&AUTHENTICATORCall)
        }

        ///Creates a new call builder for the [`BALANCE_MANAGER`] function.
        pub fn BALANCE_MANAGER(
            &self,
        ) -> alloy_contract::SolCallBuilder<&P, BALANCE_MANAGERCall, N> {
            self.call_builder(&BALANCE_MANAGERCall)
        }

        ///Creates a new call builder for the [`DOMAIN_SEPARATOR`] function.
        pub fn DOMAIN_SEPARATOR(
            &self,
        ) -> alloy_contract::SolCallBuilder<&P, DOMAIN_SEPARATORCall, N> {
            self.call_builder(&DOMAIN_SEPARATORCall)
        }

        ///Creates a new call builder for the [`REPOSITORY`] function.
        pub fn REPOSITORY(&self) -> alloy_contract::SolCallBuilder<&P, REPOSITORYCall, N> {
            self.call_builder(&REPOSITORYCall)
        }

        ///Creates a new call builder for the [`cancelLimitOrder`] function.
        pub fn cancelLimitOrder(
            &self,
            nonce: alloy_sol_types::private::primitives::aliases::U256,
        ) -> alloy_contract::SolCallBuilder<&P, cancelLimitOrderCall, N> {
            self.call_builder(&cancelLimitOrderCall { nonce })
        }

        ///Creates a new call builder for the [`hashBaseTokenData`] function.
        pub fn hashBaseTokenData(
            &self,
            _baseTokenData: <ILiquoriceSettlement::BaseTokenData as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, hashBaseTokenDataCall, N> {
            self.call_builder(&hashBaseTokenDataCall { _baseTokenData })
        }

        ///Creates a new call builder for the [`hashOrder`] function.
        pub fn hashOrder(
            &self,
            _order: <ILiquoriceSettlement::Order as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, hashOrderCall, N> {
            self.call_builder(&hashOrderCall { _order })
        }

        ///Creates a new call builder for the [`hashQuoteTokenData`] function.
        pub fn hashQuoteTokenData(
            &self,
            _quoteTokenData: <ILiquoriceSettlement::QuoteTokenData as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, hashQuoteTokenDataCall, N> {
            self.call_builder(&hashQuoteTokenDataCall { _quoteTokenData })
        }

        ///Creates a new call builder for the [`hashSingleOrder`] function.
        pub fn hashSingleOrder(
            &self,
            _order: <ILiquoriceSettlement::Single as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, hashSingleOrderCall, N> {
            self.call_builder(&hashSingleOrderCall { _order })
        }

        ///Creates a new call builder for the [`isValidSignature`] function.
        pub fn isValidSignature(
            &self,
            _hash: alloy_sol_types::private::FixedBytes<32>,
            _signature: alloy_sol_types::private::Bytes,
        ) -> alloy_contract::SolCallBuilder<&P, isValidSignatureCall, N> {
            self.call_builder(&isValidSignatureCall { _hash, _signature })
        }

        ///Creates a new call builder for the [`settle`] function.
        pub fn settle(
            &self,
            _signer: alloy_sol_types::private::Address,
            _filledTakerAmount: alloy_sol_types::private::primitives::aliases::U256,
            _order: <ILiquoriceSettlement::Order as alloy_sol_types::SolType>::RustType,
            _interactions: alloy_sol_types::private::Vec<
                <GPv2Interaction::Data as alloy_sol_types::SolType>::RustType,
            >,
            _hooks: <GPv2Interaction::Hooks as alloy_sol_types::SolType>::RustType,
            _makerSignature: <Signature::TypedSignature as alloy_sol_types::SolType>::RustType,
            _takerSignature: <Signature::TypedSignature as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, settleCall, N> {
            self.call_builder(&settleCall {
                _signer,
                _filledTakerAmount,
                _order,
                _interactions,
                _hooks,
                _makerSignature,
                _takerSignature,
            })
        }

        ///Creates a new call builder for the [`settleSingle`] function.
        pub fn settleSingle(
            &self,
            _signer: alloy_sol_types::private::Address,
            _order: <ILiquoriceSettlement::Single as alloy_sol_types::SolType>::RustType,
            _makerSignature: <Signature::TypedSignature as alloy_sol_types::SolType>::RustType,
            _filledTakerAmount: alloy_sol_types::private::primitives::aliases::U256,
            _takerSignature: <Signature::TypedSignature as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, settleSingleCall, N> {
            self.call_builder(&settleSingleCall {
                _signer,
                _order,
                _makerSignature,
                _filledTakerAmount,
                _takerSignature,
            })
        }

        ///Creates a new call builder for the
        /// [`settleSingleWithPermitsSignatures`] function.
        pub fn settleSingleWithPermitsSignatures(
            &self,
            _signer: alloy_sol_types::private::Address,
            _order: <ILiquoriceSettlement::Single as alloy_sol_types::SolType>::RustType,
            _makerSignature: <Signature::TypedSignature as alloy_sol_types::SolType>::RustType,
            _filledTakerAmount: alloy_sol_types::private::primitives::aliases::U256,
            _takerSignature: <Signature::TypedSignature as alloy_sol_types::SolType>::RustType,
            _takerPermitInfo: <Signature::TakerPermitInfo as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, settleSingleWithPermitsSignaturesCall, N> {
            self.call_builder(&settleSingleWithPermitsSignaturesCall {
                _signer,
                _order,
                _makerSignature,
                _filledTakerAmount,
                _takerSignature,
                _takerPermitInfo,
            })
        }

        ///Creates a new call builder for the [`settleWithPermitsSignatures`]
        /// function.
        pub fn settleWithPermitsSignatures(
            &self,
            _signer: alloy_sol_types::private::Address,
            _filledTakerAmount: alloy_sol_types::private::primitives::aliases::U256,
            _order: <ILiquoriceSettlement::Order as alloy_sol_types::SolType>::RustType,
            _interactions: alloy_sol_types::private::Vec<
                <GPv2Interaction::Data as alloy_sol_types::SolType>::RustType,
            >,
            _hooks: <GPv2Interaction::Hooks as alloy_sol_types::SolType>::RustType,
            _makerSignature: <Signature::TypedSignature as alloy_sol_types::SolType>::RustType,
            _takerSignature: <Signature::TypedSignature as alloy_sol_types::SolType>::RustType,
            _takerPermitInfo: <Signature::TakerPermitInfo as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, settleWithPermitsSignaturesCall, N> {
            self.call_builder(&settleWithPermitsSignaturesCall {
                _signer,
                _filledTakerAmount,
                _order,
                _interactions,
                _hooks,
                _makerSignature,
                _takerSignature,
                _takerPermitInfo,
            })
        }

        ///Creates a new call builder for the [`validateHooks`] function.
        pub fn validateHooks(
            &self,
            _repository: alloy_sol_types::private::Address,
            _hooks: <GPv2Interaction::Hooks as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, validateHooksCall, N> {
            self.call_builder(&validateHooksCall {
                _repository,
                _hooks,
            })
        }

        ///Creates a new call builder for the [`validateInteractions`]
        /// function.
        pub fn validateInteractions(
            &self,
            _repository: alloy_sol_types::private::Address,
            _signer: alloy_sol_types::private::Address,
            _isPartialFill: bool,
            _order: <ILiquoriceSettlement::Order as alloy_sol_types::SolType>::RustType,
            _interactions: alloy_sol_types::private::Vec<
                <GPv2Interaction::Data as alloy_sol_types::SolType>::RustType,
            >,
        ) -> alloy_contract::SolCallBuilder<&P, validateInteractionsCall, N> {
            self.call_builder(&validateInteractionsCall {
                _repository,
                _signer,
                _isPartialFill,
                _order,
                _interactions,
            })
        }

        ///Creates a new call builder for the [`validateOrderAmounts`]
        /// function.
        pub fn validateOrderAmounts(
            &self,
            _order: <ILiquoriceSettlement::Order as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, validateOrderAmountsCall, N> {
            self.call_builder(&validateOrderAmountsCall { _order })
        }

        ///Creates a new call builder for the [`validateSignature`] function.
        pub fn validateSignature(
            &self,
            _validationAddress: alloy_sol_types::private::Address,
            _hash: alloy_sol_types::private::FixedBytes<32>,
            _signature: <Signature::TypedSignature as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, validateSignatureCall, N> {
            self.call_builder(&validateSignatureCall {
                _validationAddress,
                _hash,
                _signature,
            })
        }
    }
    /// Event filters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        LiquoriceSettlementInstance<P, N>
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

        ///Creates a new event filter for the [`Interaction`] event.
        pub fn Interaction_filter(&self) -> alloy_contract::Event<&P, Interaction, N> {
            self.event_filter::<Interaction>()
        }

        ///Creates a new event filter for the [`TradeOrder`] event.
        pub fn TradeOrder_filter(&self) -> alloy_contract::Event<&P, TradeOrder, N> {
            self.event_filter::<TradeOrder>()
        }
    }
}
pub type Instance = LiquoriceSettlement::LiquoriceSettlementInstance<::alloy_provider::DynProvider>;
use {
    alloy_primitives::{Address, address},
    alloy_provider::{DynProvider, Provider},
    anyhow::{Context, Result},
    std::{collections::HashMap, sync::LazyLock},
};
pub const fn deployment_info(chain_id: u64) -> Option<(Address, Option<u64>)> {
    match chain_id {
        1u64 => Some((
            ::alloy_primitives::address!("0x0448633eb8B0A42EfED924C42069E0DcF08fb552"),
            None,
        )),
        42161u64 => Some((
            ::alloy_primitives::address!("0x0448633eb8B0A42EfED924C42069E0DcF08fb552"),
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
