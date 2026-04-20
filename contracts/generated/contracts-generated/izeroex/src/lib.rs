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
library LibNFTOrder {
    type TradeDirection is uint8;
    struct Fee { address recipient; uint256 amount; bytes feeData; }
    struct Property { address propertyValidator; bytes propertyData; }
}
```*/
#[allow(
    non_camel_case_types,
    non_snake_case,
    clippy::pub_underscore_fields,
    clippy::style,
    clippy::empty_structs_with_brackets
)]
pub mod LibNFTOrder {
    use {super::*, alloy_sol_types};
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct TradeDirection(u8);
    const _: () = {
        use alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<TradeDirection> for u8 {
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
        impl TradeDirection {
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
        impl From<u8> for TradeDirection {
            fn from(value: u8) -> Self {
                Self::from_underlying(value)
            }
        }
        #[automatically_derived]
        impl From<TradeDirection> for u8 {
            fn from(value: TradeDirection) -> Self {
                value.into_underlying()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolType for TradeDirection {
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
        impl alloy_sol_types::EventTopic for TradeDirection {
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
    struct Fee { address recipient; uint256 amount; bytes feeData; }
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct Fee {
        #[allow(missing_docs)]
        pub recipient: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub amount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub feeData: alloy_sol_types::private::Bytes,
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
        impl ::core::convert::From<Fee> for UnderlyingRustTuple<'_> {
            fn from(value: Fee) -> Self {
                (value.recipient, value.amount, value.feeData)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for Fee {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    recipient: tuple.0,
                    amount: tuple.1,
                    feeData: tuple.2,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for Fee {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for Fee {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.recipient,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.amount,
                    ),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.feeData,
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
        impl alloy_sol_types::SolType for Fee {
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
        impl alloy_sol_types::SolStruct for Fee {
            const NAME: &'static str = "Fee";

            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "Fee(address recipient,uint256 amount,bytes feeData)",
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
                            &self.recipient,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.amount)
                        .0,
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::eip712_data_word(
                            &self.feeData,
                        )
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for Fee {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.recipient,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.amount,
                    )
                    + <alloy_sol_types::sol_data::Bytes as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.feeData,
                    )
            }

            #[inline]
            fn encode_topic_preimage(
                rust: &Self::RustType,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                out.reserve(<Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust));
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.recipient,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.amount,
                    out,
                );
                <alloy_sol_types::sol_data::Bytes as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.feeData,
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
    struct Property { address propertyValidator; bytes propertyData; }
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct Property {
        #[allow(missing_docs)]
        pub propertyValidator: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub propertyData: alloy_sol_types::private::Bytes,
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
            alloy_sol_types::sol_data::Bytes,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::Address,
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
        impl ::core::convert::From<Property> for UnderlyingRustTuple<'_> {
            fn from(value: Property) -> Self {
                (value.propertyValidator, value.propertyData)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for Property {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    propertyValidator: tuple.0,
                    propertyData: tuple.1,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for Property {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for Property {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.propertyValidator,
                    ),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.propertyData,
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
        impl alloy_sol_types::SolType for Property {
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
        impl alloy_sol_types::SolStruct for Property {
            const NAME: &'static str = "Property";

            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "Property(address propertyValidator,bytes propertyData)",
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
                            &self.propertyValidator,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::eip712_data_word(
                            &self.propertyData,
                        )
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for Property {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.propertyValidator,
                    )
                    + <alloy_sol_types::sol_data::Bytes as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.propertyData,
                    )
            }

            #[inline]
            fn encode_topic_preimage(
                rust: &Self::RustType,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                out.reserve(<Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust));
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.propertyValidator,
                    out,
                );
                <alloy_sol_types::sol_data::Bytes as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.propertyData,
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
    /**Creates a new wrapper around an on-chain [`LibNFTOrder`](self) contract instance.

    See the [wrapper's documentation](`LibNFTOrderInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> LibNFTOrderInstance<P, N> {
        LibNFTOrderInstance::<P, N>::new(address, __provider)
    }
    /**A [`LibNFTOrder`](self) instance.

    Contains type-safe methods for interacting with an on-chain instance of the
    [`LibNFTOrder`](self) contract located at a given `address`, using a given
    provider `P`.

    If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
    documentation on how to provide it), the `deploy` and `deploy_builder` methods can
    be used to deploy a new instance of the contract.

    See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct LibNFTOrderInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for LibNFTOrderInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("LibNFTOrderInstance")
                .field(&self.address)
                .finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        LibNFTOrderInstance<P, N>
    {
        /**Creates a new wrapper around an on-chain [`LibNFTOrder`](self) contract instance.

        See the [wrapper's documentation](`LibNFTOrderInstance`) for more details.*/
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
    impl<P: ::core::clone::Clone, N> LibNFTOrderInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned
        /// provider.
        #[inline]
        pub fn with_cloned_provider(self) -> LibNFTOrderInstance<P, N> {
            LibNFTOrderInstance {
                address: self.address,
                provider: ::core::clone::Clone::clone(&self.provider),
                _network: ::core::marker::PhantomData,
            }
        }
    }
    /// Function calls.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        LibNFTOrderInstance<P, N>
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
        LibNFTOrderInstance<P, N>
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
library LibNativeOrder {
    type OrderStatus is uint8;
    struct LimitOrder { address makerToken; address takerToken; uint128 makerAmount; uint128 takerAmount; uint128 takerTokenFeeAmount; address maker; address taker; address sender; address feeRecipient; bytes32 pool; uint64 expiry; uint256 salt; }
    struct OrderInfo { bytes32 orderHash; OrderStatus status; uint128 takerTokenFilledAmount; }
}
```*/
#[allow(
    non_camel_case_types,
    non_snake_case,
    clippy::pub_underscore_fields,
    clippy::style,
    clippy::empty_structs_with_brackets
)]
pub mod LibNativeOrder {
    use {super::*, alloy_sol_types};
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct OrderStatus(u8);
    const _: () = {
        use alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<OrderStatus> for u8 {
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
        impl OrderStatus {
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
        impl From<u8> for OrderStatus {
            fn from(value: u8) -> Self {
                Self::from_underlying(value)
            }
        }
        #[automatically_derived]
        impl From<OrderStatus> for u8 {
            fn from(value: OrderStatus) -> Self {
                value.into_underlying()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolType for OrderStatus {
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
        impl alloy_sol_types::EventTopic for OrderStatus {
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
    struct LimitOrder { address makerToken; address takerToken; uint128 makerAmount; uint128 takerAmount; uint128 takerTokenFeeAmount; address maker; address taker; address sender; address feeRecipient; bytes32 pool; uint64 expiry; uint256 salt; }
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct LimitOrder {
        #[allow(missing_docs)]
        pub makerToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub takerToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub makerAmount: u128,
        #[allow(missing_docs)]
        pub takerAmount: u128,
        #[allow(missing_docs)]
        pub takerTokenFeeAmount: u128,
        #[allow(missing_docs)]
        pub maker: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub taker: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub sender: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub feeRecipient: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub pool: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub expiry: u64,
        #[allow(missing_docs)]
        pub salt: alloy_sol_types::private::primitives::aliases::U256,
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
            alloy_sol_types::sol_data::Uint<128>,
            alloy_sol_types::sol_data::Uint<128>,
            alloy_sol_types::sol_data::Uint<128>,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::FixedBytes<32>,
            alloy_sol_types::sol_data::Uint<64>,
            alloy_sol_types::sol_data::Uint<256>,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::Address,
            alloy_sol_types::private::Address,
            u128,
            u128,
            u128,
            alloy_sol_types::private::Address,
            alloy_sol_types::private::Address,
            alloy_sol_types::private::Address,
            alloy_sol_types::private::Address,
            alloy_sol_types::private::FixedBytes<32>,
            u64,
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
        impl ::core::convert::From<LimitOrder> for UnderlyingRustTuple<'_> {
            fn from(value: LimitOrder) -> Self {
                (
                    value.makerToken,
                    value.takerToken,
                    value.makerAmount,
                    value.takerAmount,
                    value.takerTokenFeeAmount,
                    value.maker,
                    value.taker,
                    value.sender,
                    value.feeRecipient,
                    value.pool,
                    value.expiry,
                    value.salt,
                )
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for LimitOrder {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    makerToken: tuple.0,
                    takerToken: tuple.1,
                    makerAmount: tuple.2,
                    takerAmount: tuple.3,
                    takerTokenFeeAmount: tuple.4,
                    maker: tuple.5,
                    taker: tuple.6,
                    sender: tuple.7,
                    feeRecipient: tuple.8,
                    pool: tuple.9,
                    expiry: tuple.10,
                    salt: tuple.11,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for LimitOrder {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for LimitOrder {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.makerToken,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.takerToken,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        128,
                    > as alloy_sol_types::SolType>::tokenize(&self.makerAmount),
                    <alloy_sol_types::sol_data::Uint<
                        128,
                    > as alloy_sol_types::SolType>::tokenize(&self.takerAmount),
                    <alloy_sol_types::sol_data::Uint<
                        128,
                    > as alloy_sol_types::SolType>::tokenize(&self.takerTokenFeeAmount),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.maker,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.taker,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.sender,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.feeRecipient,
                    ),
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self.pool),
                    <alloy_sol_types::sol_data::Uint<
                        64,
                    > as alloy_sol_types::SolType>::tokenize(&self.expiry),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.salt),
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
        impl alloy_sol_types::SolType for LimitOrder {
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
        impl alloy_sol_types::SolStruct for LimitOrder {
            const NAME: &'static str = "LimitOrder";

            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "LimitOrder(address makerToken,address takerToken,uint128 makerAmount,uint128 \
                     takerAmount,uint128 takerTokenFeeAmount,address maker,address taker,address \
                     sender,address feeRecipient,bytes32 pool,uint64 expiry,uint256 salt)",
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
                            &self.makerToken,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.takerToken,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        128,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.makerAmount)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        128,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.takerAmount)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        128,
                    > as alloy_sol_types::SolType>::eip712_data_word(
                            &self.takerTokenFeeAmount,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.maker,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.taker,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.sender,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.feeRecipient,
                        )
                        .0,
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.pool)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        64,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.expiry)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.salt)
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for LimitOrder {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.makerToken,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.takerToken,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        128,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.makerAmount,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        128,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.takerAmount,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        128,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.takerTokenFeeAmount,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.maker,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.taker,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.sender,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.feeRecipient,
                    )
                    + <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(&rust.pool)
                    + <alloy_sol_types::sol_data::Uint<
                        64,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.expiry,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(&rust.salt)
            }

            #[inline]
            fn encode_topic_preimage(
                rust: &Self::RustType,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                out.reserve(<Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust));
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.makerToken,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.takerToken,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    128,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.makerAmount,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    128,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.takerAmount,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    128,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.takerTokenFeeAmount,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.maker,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.taker,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.sender,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.feeRecipient,
                    out,
                );
                <alloy_sol_types::sol_data::FixedBytes<
                    32,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.pool,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    64,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.expiry,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.salt,
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
    struct OrderInfo { bytes32 orderHash; OrderStatus status; uint128 takerTokenFilledAmount; }
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct OrderInfo {
        #[allow(missing_docs)]
        pub orderHash: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub status: <OrderStatus as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub takerTokenFilledAmount: u128,
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
            OrderStatus,
            alloy_sol_types::sol_data::Uint<128>,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::FixedBytes<32>,
            <OrderStatus as alloy_sol_types::SolType>::RustType,
            u128,
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
        impl ::core::convert::From<OrderInfo> for UnderlyingRustTuple<'_> {
            fn from(value: OrderInfo) -> Self {
                (value.orderHash, value.status, value.takerTokenFilledAmount)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for OrderInfo {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    orderHash: tuple.0,
                    status: tuple.1,
                    takerTokenFilledAmount: tuple.2,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for OrderInfo {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for OrderInfo {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self.orderHash),
                    <OrderStatus as alloy_sol_types::SolType>::tokenize(&self.status),
                    <alloy_sol_types::sol_data::Uint<
                        128,
                    > as alloy_sol_types::SolType>::tokenize(
                        &self.takerTokenFilledAmount,
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
        impl alloy_sol_types::SolType for OrderInfo {
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
        impl alloy_sol_types::SolStruct for OrderInfo {
            const NAME: &'static str = "OrderInfo";

            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "OrderInfo(bytes32 orderHash,uint8 status,uint128 takerTokenFilledAmount)",
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
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.orderHash)
                        .0,
                    <OrderStatus as alloy_sol_types::SolType>::eip712_data_word(
                            &self.status,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        128,
                    > as alloy_sol_types::SolType>::eip712_data_word(
                            &self.takerTokenFilledAmount,
                        )
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for OrderInfo {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.orderHash,
                    )
                    + <OrderStatus as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.status,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        128,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.takerTokenFilledAmount,
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
                    &rust.orderHash,
                    out,
                );
                <OrderStatus as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.status,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    128,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.takerTokenFilledAmount,
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
    /**Creates a new wrapper around an on-chain [`LibNativeOrder`](self) contract instance.

    See the [wrapper's documentation](`LibNativeOrderInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> LibNativeOrderInstance<P, N> {
        LibNativeOrderInstance::<P, N>::new(address, __provider)
    }
    /**A [`LibNativeOrder`](self) instance.

    Contains type-safe methods for interacting with an on-chain instance of the
    [`LibNativeOrder`](self) contract located at a given `address`, using a given
    provider `P`.

    If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
    documentation on how to provide it), the `deploy` and `deploy_builder` methods can
    be used to deploy a new instance of the contract.

    See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct LibNativeOrderInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for LibNativeOrderInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("LibNativeOrderInstance")
                .field(&self.address)
                .finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        LibNativeOrderInstance<P, N>
    {
        /**Creates a new wrapper around an on-chain [`LibNativeOrder`](self) contract instance.

        See the [wrapper's documentation](`LibNativeOrderInstance`) for more details.*/
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
    impl<P: ::core::clone::Clone, N> LibNativeOrderInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned
        /// provider.
        #[inline]
        pub fn with_cloned_provider(self) -> LibNativeOrderInstance<P, N> {
            LibNativeOrderInstance {
                address: self.address,
                provider: ::core::clone::Clone::clone(&self.provider),
                _network: ::core::marker::PhantomData,
            }
        }
    }
    /// Function calls.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        LibNativeOrderInstance<P, N>
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
        LibNativeOrderInstance<P, N>
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
library LibSignature {
    type SignatureType is uint8;
    struct Signature { SignatureType signatureType; uint8 v; bytes32 r; bytes32 s; }
}
```*/
#[allow(
    non_camel_case_types,
    non_snake_case,
    clippy::pub_underscore_fields,
    clippy::style,
    clippy::empty_structs_with_brackets
)]
pub mod LibSignature {
    use {super::*, alloy_sol_types};
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct SignatureType(u8);
    const _: () = {
        use alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<SignatureType> for u8 {
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
        impl SignatureType {
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
        impl From<u8> for SignatureType {
            fn from(value: u8) -> Self {
                Self::from_underlying(value)
            }
        }
        #[automatically_derived]
        impl From<SignatureType> for u8 {
            fn from(value: SignatureType) -> Self {
                value.into_underlying()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolType for SignatureType {
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
        impl alloy_sol_types::EventTopic for SignatureType {
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
    struct Signature { SignatureType signatureType; uint8 v; bytes32 r; bytes32 s; }
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct Signature {
        #[allow(missing_docs)]
        pub signatureType: <SignatureType as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub v: u8,
        #[allow(missing_docs)]
        pub r: alloy_sol_types::private::FixedBytes<32>,
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
        type UnderlyingSolTuple<'a> = (
            SignatureType,
            alloy_sol_types::sol_data::Uint<8>,
            alloy_sol_types::sol_data::FixedBytes<32>,
            alloy_sol_types::sol_data::FixedBytes<32>,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            <SignatureType as alloy_sol_types::SolType>::RustType,
            u8,
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
        impl ::core::convert::From<Signature> for UnderlyingRustTuple<'_> {
            fn from(value: Signature) -> Self {
                (value.signatureType, value.v, value.r, value.s)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for Signature {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    signatureType: tuple.0,
                    v: tuple.1,
                    r: tuple.2,
                    s: tuple.3,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for Signature {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for Signature {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <SignatureType as alloy_sol_types::SolType>::tokenize(
                        &self.signatureType,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        8,
                    > as alloy_sol_types::SolType>::tokenize(&self.v),
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self.r),
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self.s),
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
        impl alloy_sol_types::SolType for Signature {
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
        impl alloy_sol_types::SolStruct for Signature {
            const NAME: &'static str = "Signature";

            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "Signature(uint8 signatureType,uint8 v,bytes32 r,bytes32 s)",
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
                    <SignatureType as alloy_sol_types::SolType>::eip712_data_word(
                            &self.signatureType,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        8,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.v)
                        .0,
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.r)
                        .0,
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.s)
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for Signature {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <SignatureType as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.signatureType,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        8,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(&rust.v)
                    + <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(&rust.r)
                    + <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(&rust.s)
            }

            #[inline]
            fn encode_topic_preimage(
                rust: &Self::RustType,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                out.reserve(<Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust));
                <SignatureType as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.signatureType,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    8,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(&rust.v, out);
                <alloy_sol_types::sol_data::FixedBytes<
                    32,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(&rust.r, out);
                <alloy_sol_types::sol_data::FixedBytes<
                    32,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(&rust.s, out);
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
    /**Creates a new wrapper around an on-chain [`LibSignature`](self) contract instance.

    See the [wrapper's documentation](`LibSignatureInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> LibSignatureInstance<P, N> {
        LibSignatureInstance::<P, N>::new(address, __provider)
    }
    /**A [`LibSignature`](self) instance.

    Contains type-safe methods for interacting with an on-chain instance of the
    [`LibSignature`](self) contract located at a given `address`, using a given
    provider `P`.

    If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
    documentation on how to provide it), the `deploy` and `deploy_builder` methods can
    be used to deploy a new instance of the contract.

    See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct LibSignatureInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for LibSignatureInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("LibSignatureInstance")
                .field(&self.address)
                .finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        LibSignatureInstance<P, N>
    {
        /**Creates a new wrapper around an on-chain [`LibSignature`](self) contract instance.

        See the [wrapper's documentation](`LibSignatureInstance`) for more details.*/
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
    impl<P: ::core::clone::Clone, N> LibSignatureInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned
        /// provider.
        #[inline]
        pub fn with_cloned_provider(self) -> LibSignatureInstance<P, N> {
            LibSignatureInstance {
                address: self.address,
                provider: ::core::clone::Clone::clone(&self.provider),
                _network: ::core::marker::PhantomData,
            }
        }
    }
    /// Function calls.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        LibSignatureInstance<P, N>
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
        LibSignatureInstance<P, N>
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
library LibNFTOrder {
    type TradeDirection is uint8;
    struct Fee {
        address recipient;
        uint256 amount;
        bytes feeData;
    }
    struct Property {
        address propertyValidator;
        bytes propertyData;
    }
}

library LibNativeOrder {
    type OrderStatus is uint8;
    struct LimitOrder {
        address makerToken;
        address takerToken;
        uint128 makerAmount;
        uint128 takerAmount;
        uint128 takerTokenFeeAmount;
        address maker;
        address taker;
        address sender;
        address feeRecipient;
        bytes32 pool;
        uint64 expiry;
        uint256 salt;
    }
    struct OrderInfo {
        bytes32 orderHash;
        OrderStatus status;
        uint128 takerTokenFilledAmount;
    }
}

library LibSignature {
    type SignatureType is uint8;
    struct Signature {
        SignatureType signatureType;
        uint8 v;
        bytes32 r;
        bytes32 s;
    }
}

interface IZeroex {
    event ERC1155OrderCancelled(address maker, uint256 nonce);
    event ERC1155OrderFilled(LibNFTOrder.TradeDirection direction, address maker, address taker, uint256 nonce, address erc20Token, uint256 erc20FillAmount, address erc1155Token, uint256 erc1155TokenId, uint128 erc1155FillAmount, address matcher);
    event ERC1155OrderPreSigned(LibNFTOrder.TradeDirection direction, address maker, address taker, uint256 expiry, uint256 nonce, address erc20Token, uint256 erc20TokenAmount, LibNFTOrder.Fee[] fees, address erc1155Token, uint256 erc1155TokenId, LibNFTOrder.Property[] erc1155TokenProperties, uint128 erc1155TokenAmount);
    event ERC721OrderCancelled(address maker, uint256 nonce);
    event ERC721OrderFilled(LibNFTOrder.TradeDirection direction, address maker, address taker, uint256 nonce, address erc20Token, uint256 erc20TokenAmount, address erc721Token, uint256 erc721TokenId, address matcher);
    event ERC721OrderPreSigned(LibNFTOrder.TradeDirection direction, address maker, address taker, uint256 expiry, uint256 nonce, address erc20Token, uint256 erc20TokenAmount, LibNFTOrder.Fee[] fees, address erc721Token, uint256 erc721TokenId, LibNFTOrder.Property[] erc721TokenProperties);
    event LimitOrderFilled(bytes32 orderHash, address maker, address taker, address feeRecipient, address makerToken, address takerToken, uint128 takerTokenFilledAmount, uint128 makerTokenFilledAmount, uint128 takerTokenFeeFilledAmount, uint256 protocolFeePaid, bytes32 pool);
    event LiquidityProviderSwap(address inputToken, address outputToken, uint256 inputTokenAmount, uint256 outputTokenAmount, address provider, address recipient);
    event MetaTransactionExecuted(bytes32 hash, bytes4 indexed selector, address signer, address sender);
    event Migrated(address caller, address migrator, address newOwner);
    event OrderCancelled(bytes32 orderHash, address maker);
    event OrderSignerRegistered(address maker, address signer, bool allowed);
    event OtcOrderFilled(bytes32 orderHash, address maker, address taker, address makerToken, address takerToken, uint128 makerTokenFilledAmount, uint128 takerTokenFilledAmount);
    event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);
    event PairCancelledLimitOrders(address maker, address makerToken, address takerToken, uint256 minValidSalt);
    event PairCancelledRfqOrders(address maker, address makerToken, address takerToken, uint256 minValidSalt);
    event ProxyFunctionUpdated(bytes4 indexed selector, address oldImpl, address newImpl);
    event QuoteSignerUpdated(address quoteSigner);
    event RfqOrderFilled(bytes32 orderHash, address maker, address taker, address makerToken, address takerToken, uint128 takerTokenFilledAmount, uint128 makerTokenFilledAmount, bytes32 pool);
    event RfqOrderOriginsAllowed(address origin, address[] addrs, bool allowed);
    event TransformedERC20(address indexed taker, address inputToken, address outputToken, uint256 inputTokenAmount, uint256 outputTokenAmount);
    event TransformerDeployerUpdated(address transformerDeployer);

    function extend(bytes4 selector, address impl) external;
    function fillOrKillLimitOrder(LibNativeOrder.LimitOrder memory order, LibSignature.Signature memory signature, uint128 takerTokenFillAmount) external payable returns (uint128 makerTokenFilledAmount);
    function getLimitOrderRelevantState(LibNativeOrder.LimitOrder memory order, LibSignature.Signature memory signature) external view returns (LibNativeOrder.OrderInfo memory orderInfo, uint128 actualFillableTakerTokenAmount, bool isSignatureValid);
    function owner() external view returns (address ownerAddress);
}
```

...which was generated by the following JSON ABI:
```json
[
  {
    "type": "function",
    "name": "extend",
    "inputs": [
      {
        "name": "selector",
        "type": "bytes4",
        "internalType": "bytes4"
      },
      {
        "name": "impl",
        "type": "address",
        "internalType": "address"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "fillOrKillLimitOrder",
    "inputs": [
      {
        "name": "order",
        "type": "tuple",
        "internalType": "struct LibNativeOrder.LimitOrder",
        "components": [
          {
            "name": "makerToken",
            "type": "address",
            "internalType": "contract IERC20TokenV06"
          },
          {
            "name": "takerToken",
            "type": "address",
            "internalType": "contract IERC20TokenV06"
          },
          {
            "name": "makerAmount",
            "type": "uint128",
            "internalType": "uint128"
          },
          {
            "name": "takerAmount",
            "type": "uint128",
            "internalType": "uint128"
          },
          {
            "name": "takerTokenFeeAmount",
            "type": "uint128",
            "internalType": "uint128"
          },
          {
            "name": "maker",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "taker",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "sender",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "feeRecipient",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "pool",
            "type": "bytes32",
            "internalType": "bytes32"
          },
          {
            "name": "expiry",
            "type": "uint64",
            "internalType": "uint64"
          },
          {
            "name": "salt",
            "type": "uint256",
            "internalType": "uint256"
          }
        ]
      },
      {
        "name": "signature",
        "type": "tuple",
        "internalType": "struct LibSignature.Signature",
        "components": [
          {
            "name": "signatureType",
            "type": "uint8",
            "internalType": "enum LibSignature.SignatureType"
          },
          {
            "name": "v",
            "type": "uint8",
            "internalType": "uint8"
          },
          {
            "name": "r",
            "type": "bytes32",
            "internalType": "bytes32"
          },
          {
            "name": "s",
            "type": "bytes32",
            "internalType": "bytes32"
          }
        ]
      },
      {
        "name": "takerTokenFillAmount",
        "type": "uint128",
        "internalType": "uint128"
      }
    ],
    "outputs": [
      {
        "name": "makerTokenFilledAmount",
        "type": "uint128",
        "internalType": "uint128"
      }
    ],
    "stateMutability": "payable"
  },
  {
    "type": "function",
    "name": "getLimitOrderRelevantState",
    "inputs": [
      {
        "name": "order",
        "type": "tuple",
        "internalType": "struct LibNativeOrder.LimitOrder",
        "components": [
          {
            "name": "makerToken",
            "type": "address",
            "internalType": "contract IERC20TokenV06"
          },
          {
            "name": "takerToken",
            "type": "address",
            "internalType": "contract IERC20TokenV06"
          },
          {
            "name": "makerAmount",
            "type": "uint128",
            "internalType": "uint128"
          },
          {
            "name": "takerAmount",
            "type": "uint128",
            "internalType": "uint128"
          },
          {
            "name": "takerTokenFeeAmount",
            "type": "uint128",
            "internalType": "uint128"
          },
          {
            "name": "maker",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "taker",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "sender",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "feeRecipient",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "pool",
            "type": "bytes32",
            "internalType": "bytes32"
          },
          {
            "name": "expiry",
            "type": "uint64",
            "internalType": "uint64"
          },
          {
            "name": "salt",
            "type": "uint256",
            "internalType": "uint256"
          }
        ]
      },
      {
        "name": "signature",
        "type": "tuple",
        "internalType": "struct LibSignature.Signature",
        "components": [
          {
            "name": "signatureType",
            "type": "uint8",
            "internalType": "enum LibSignature.SignatureType"
          },
          {
            "name": "v",
            "type": "uint8",
            "internalType": "uint8"
          },
          {
            "name": "r",
            "type": "bytes32",
            "internalType": "bytes32"
          },
          {
            "name": "s",
            "type": "bytes32",
            "internalType": "bytes32"
          }
        ]
      }
    ],
    "outputs": [
      {
        "name": "orderInfo",
        "type": "tuple",
        "internalType": "struct LibNativeOrder.OrderInfo",
        "components": [
          {
            "name": "orderHash",
            "type": "bytes32",
            "internalType": "bytes32"
          },
          {
            "name": "status",
            "type": "uint8",
            "internalType": "enum LibNativeOrder.OrderStatus"
          },
          {
            "name": "takerTokenFilledAmount",
            "type": "uint128",
            "internalType": "uint128"
          }
        ]
      },
      {
        "name": "actualFillableTakerTokenAmount",
        "type": "uint128",
        "internalType": "uint128"
      },
      {
        "name": "isSignatureValid",
        "type": "bool",
        "internalType": "bool"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "owner",
    "inputs": [],
    "outputs": [
      {
        "name": "ownerAddress",
        "type": "address",
        "internalType": "address"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "event",
    "name": "ERC1155OrderCancelled",
    "inputs": [
      {
        "name": "maker",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "nonce",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "ERC1155OrderFilled",
    "inputs": [
      {
        "name": "direction",
        "type": "uint8",
        "indexed": false,
        "internalType": "enum LibNFTOrder.TradeDirection"
      },
      {
        "name": "maker",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "taker",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "nonce",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "erc20Token",
        "type": "address",
        "indexed": false,
        "internalType": "contract IERC20TokenV06"
      },
      {
        "name": "erc20FillAmount",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "erc1155Token",
        "type": "address",
        "indexed": false,
        "internalType": "contract IERC1155Token"
      },
      {
        "name": "erc1155TokenId",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "erc1155FillAmount",
        "type": "uint128",
        "indexed": false,
        "internalType": "uint128"
      },
      {
        "name": "matcher",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "ERC1155OrderPreSigned",
    "inputs": [
      {
        "name": "direction",
        "type": "uint8",
        "indexed": false,
        "internalType": "enum LibNFTOrder.TradeDirection"
      },
      {
        "name": "maker",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "taker",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "expiry",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "nonce",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "erc20Token",
        "type": "address",
        "indexed": false,
        "internalType": "contract IERC20TokenV06"
      },
      {
        "name": "erc20TokenAmount",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "fees",
        "type": "tuple[]",
        "indexed": false,
        "internalType": "struct LibNFTOrder.Fee[]",
        "components": [
          {
            "name": "recipient",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "amount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "feeData",
            "type": "bytes",
            "internalType": "bytes"
          }
        ]
      },
      {
        "name": "erc1155Token",
        "type": "address",
        "indexed": false,
        "internalType": "contract IERC1155Token"
      },
      {
        "name": "erc1155TokenId",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "erc1155TokenProperties",
        "type": "tuple[]",
        "indexed": false,
        "internalType": "struct LibNFTOrder.Property[]",
        "components": [
          {
            "name": "propertyValidator",
            "type": "address",
            "internalType": "contract IPropertyValidator"
          },
          {
            "name": "propertyData",
            "type": "bytes",
            "internalType": "bytes"
          }
        ]
      },
      {
        "name": "erc1155TokenAmount",
        "type": "uint128",
        "indexed": false,
        "internalType": "uint128"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "ERC721OrderCancelled",
    "inputs": [
      {
        "name": "maker",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "nonce",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "ERC721OrderFilled",
    "inputs": [
      {
        "name": "direction",
        "type": "uint8",
        "indexed": false,
        "internalType": "enum LibNFTOrder.TradeDirection"
      },
      {
        "name": "maker",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "taker",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "nonce",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "erc20Token",
        "type": "address",
        "indexed": false,
        "internalType": "contract IERC20TokenV06"
      },
      {
        "name": "erc20TokenAmount",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "erc721Token",
        "type": "address",
        "indexed": false,
        "internalType": "contract IERC721Token"
      },
      {
        "name": "erc721TokenId",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "matcher",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "ERC721OrderPreSigned",
    "inputs": [
      {
        "name": "direction",
        "type": "uint8",
        "indexed": false,
        "internalType": "enum LibNFTOrder.TradeDirection"
      },
      {
        "name": "maker",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "taker",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "expiry",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "nonce",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "erc20Token",
        "type": "address",
        "indexed": false,
        "internalType": "contract IERC20TokenV06"
      },
      {
        "name": "erc20TokenAmount",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "fees",
        "type": "tuple[]",
        "indexed": false,
        "internalType": "struct LibNFTOrder.Fee[]",
        "components": [
          {
            "name": "recipient",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "amount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "feeData",
            "type": "bytes",
            "internalType": "bytes"
          }
        ]
      },
      {
        "name": "erc721Token",
        "type": "address",
        "indexed": false,
        "internalType": "contract IERC721Token"
      },
      {
        "name": "erc721TokenId",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "erc721TokenProperties",
        "type": "tuple[]",
        "indexed": false,
        "internalType": "struct LibNFTOrder.Property[]",
        "components": [
          {
            "name": "propertyValidator",
            "type": "address",
            "internalType": "contract IPropertyValidator"
          },
          {
            "name": "propertyData",
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
    "name": "LimitOrderFilled",
    "inputs": [
      {
        "name": "orderHash",
        "type": "bytes32",
        "indexed": false,
        "internalType": "bytes32"
      },
      {
        "name": "maker",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "taker",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "feeRecipient",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "makerToken",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "takerToken",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "takerTokenFilledAmount",
        "type": "uint128",
        "indexed": false,
        "internalType": "uint128"
      },
      {
        "name": "makerTokenFilledAmount",
        "type": "uint128",
        "indexed": false,
        "internalType": "uint128"
      },
      {
        "name": "takerTokenFeeFilledAmount",
        "type": "uint128",
        "indexed": false,
        "internalType": "uint128"
      },
      {
        "name": "protocolFeePaid",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "pool",
        "type": "bytes32",
        "indexed": false,
        "internalType": "bytes32"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "LiquidityProviderSwap",
    "inputs": [
      {
        "name": "inputToken",
        "type": "address",
        "indexed": false,
        "internalType": "contract IERC20TokenV06"
      },
      {
        "name": "outputToken",
        "type": "address",
        "indexed": false,
        "internalType": "contract IERC20TokenV06"
      },
      {
        "name": "inputTokenAmount",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "outputTokenAmount",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "provider",
        "type": "address",
        "indexed": false,
        "internalType": "contract ILiquidityProvider"
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
    "type": "event",
    "name": "MetaTransactionExecuted",
    "inputs": [
      {
        "name": "hash",
        "type": "bytes32",
        "indexed": false,
        "internalType": "bytes32"
      },
      {
        "name": "selector",
        "type": "bytes4",
        "indexed": true,
        "internalType": "bytes4"
      },
      {
        "name": "signer",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "sender",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "Migrated",
    "inputs": [
      {
        "name": "caller",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "migrator",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "newOwner",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "OrderCancelled",
    "inputs": [
      {
        "name": "orderHash",
        "type": "bytes32",
        "indexed": false,
        "internalType": "bytes32"
      },
      {
        "name": "maker",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "OrderSignerRegistered",
    "inputs": [
      {
        "name": "maker",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "signer",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "allowed",
        "type": "bool",
        "indexed": false,
        "internalType": "bool"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "OtcOrderFilled",
    "inputs": [
      {
        "name": "orderHash",
        "type": "bytes32",
        "indexed": false,
        "internalType": "bytes32"
      },
      {
        "name": "maker",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "taker",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "makerToken",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "takerToken",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "makerTokenFilledAmount",
        "type": "uint128",
        "indexed": false,
        "internalType": "uint128"
      },
      {
        "name": "takerTokenFilledAmount",
        "type": "uint128",
        "indexed": false,
        "internalType": "uint128"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "OwnershipTransferred",
    "inputs": [
      {
        "name": "previousOwner",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "newOwner",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "PairCancelledLimitOrders",
    "inputs": [
      {
        "name": "maker",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "makerToken",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "takerToken",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "minValidSalt",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "PairCancelledRfqOrders",
    "inputs": [
      {
        "name": "maker",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "makerToken",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "takerToken",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "minValidSalt",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "ProxyFunctionUpdated",
    "inputs": [
      {
        "name": "selector",
        "type": "bytes4",
        "indexed": true,
        "internalType": "bytes4"
      },
      {
        "name": "oldImpl",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "newImpl",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "QuoteSignerUpdated",
    "inputs": [
      {
        "name": "quoteSigner",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "RfqOrderFilled",
    "inputs": [
      {
        "name": "orderHash",
        "type": "bytes32",
        "indexed": false,
        "internalType": "bytes32"
      },
      {
        "name": "maker",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "taker",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "makerToken",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "takerToken",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "takerTokenFilledAmount",
        "type": "uint128",
        "indexed": false,
        "internalType": "uint128"
      },
      {
        "name": "makerTokenFilledAmount",
        "type": "uint128",
        "indexed": false,
        "internalType": "uint128"
      },
      {
        "name": "pool",
        "type": "bytes32",
        "indexed": false,
        "internalType": "bytes32"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "RfqOrderOriginsAllowed",
    "inputs": [
      {
        "name": "origin",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "addrs",
        "type": "address[]",
        "indexed": false,
        "internalType": "address[]"
      },
      {
        "name": "allowed",
        "type": "bool",
        "indexed": false,
        "internalType": "bool"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "TransformedERC20",
    "inputs": [
      {
        "name": "taker",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "inputToken",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "outputToken",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "inputTokenAmount",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "outputTokenAmount",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "TransformerDeployerUpdated",
    "inputs": [
      {
        "name": "transformerDeployer",
        "type": "address",
        "indexed": false,
        "internalType": "address"
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
pub mod IZeroex {
    use {super::*, alloy_sol_types};
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `ERC1155OrderCancelled(address,uint256)` and selector `0x4d5ea7da64f50a4a329921b8d2cab52dff4ebcc58b61d10ff839e28e91445684`.
    ```solidity
    event ERC1155OrderCancelled(address maker, uint256 nonce);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct ERC1155OrderCancelled {
        #[allow(missing_docs)]
        pub maker: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub nonce: alloy_sol_types::private::primitives::aliases::U256,
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
        impl alloy_sol_types::SolEvent for ERC1155OrderCancelled {
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);

            const ANONYMOUS: bool = false;
            const SIGNATURE: &'static str = "ERC1155OrderCancelled(address,uint256)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    77u8, 94u8, 167u8, 218u8, 100u8, 245u8, 10u8, 74u8, 50u8, 153u8, 33u8, 184u8,
                    210u8, 202u8, 181u8, 45u8, 255u8, 78u8, 188u8, 197u8, 139u8, 97u8, 209u8, 15u8,
                    248u8, 57u8, 226u8, 142u8, 145u8, 68u8, 86u8, 132u8,
                ]);

            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    maker: data.0,
                    nonce: data.1,
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
                        &self.maker,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.nonce,
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
        impl alloy_sol_types::private::IntoLogData for ERC1155OrderCancelled {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }

            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&ERC1155OrderCancelled> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &ERC1155OrderCancelled) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `ERC1155OrderFilled(uint8,address,address,uint256,address,uint256,address,uint256,uint128,address)` and selector `0x20cca81b0e269b265b3229d6b537da91ef475ca0ef55caed7dd30731700ba98d`.
    ```solidity
    event ERC1155OrderFilled(LibNFTOrder.TradeDirection direction, address maker, address taker, uint256 nonce, address erc20Token, uint256 erc20FillAmount, address erc1155Token, uint256 erc1155TokenId, uint128 erc1155FillAmount, address matcher);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct ERC1155OrderFilled {
        #[allow(missing_docs)]
        pub direction: <LibNFTOrder::TradeDirection as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub maker: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub taker: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub nonce: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub erc20Token: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub erc20FillAmount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub erc1155Token: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub erc1155TokenId: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub erc1155FillAmount: u128,
        #[allow(missing_docs)]
        pub matcher: alloy_sol_types::private::Address,
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
        impl alloy_sol_types::SolEvent for ERC1155OrderFilled {
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type DataTuple<'a> = (
                LibNFTOrder::TradeDirection,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<128>,
                alloy_sol_types::sol_data::Address,
            );
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);

            const ANONYMOUS: bool = false;
            const SIGNATURE: &'static str = "ERC1155OrderFilled(uint8,address,address,uint256,\
                                             address,uint256,address,uint256,uint128,address)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    32u8, 204u8, 168u8, 27u8, 14u8, 38u8, 155u8, 38u8, 91u8, 50u8, 41u8, 214u8,
                    181u8, 55u8, 218u8, 145u8, 239u8, 71u8, 92u8, 160u8, 239u8, 85u8, 202u8, 237u8,
                    125u8, 211u8, 7u8, 49u8, 112u8, 11u8, 169u8, 141u8,
                ]);

            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    direction: data.0,
                    maker: data.1,
                    taker: data.2,
                    nonce: data.3,
                    erc20Token: data.4,
                    erc20FillAmount: data.5,
                    erc1155Token: data.6,
                    erc1155TokenId: data.7,
                    erc1155FillAmount: data.8,
                    matcher: data.9,
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
                    <LibNFTOrder::TradeDirection as alloy_sol_types::SolType>::tokenize(
                        &self.direction,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.maker,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.taker,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.nonce,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.erc20Token,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.erc20FillAmount,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.erc1155Token,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.erc1155TokenId,
                    ),
                    <alloy_sol_types::sol_data::Uint<128> as alloy_sol_types::SolType>::tokenize(
                        &self.erc1155FillAmount,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.matcher,
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
        impl alloy_sol_types::private::IntoLogData for ERC1155OrderFilled {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }

            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&ERC1155OrderFilled> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &ERC1155OrderFilled) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `ERC1155OrderPreSigned(uint8,address,address,uint256,uint256,address,uint256,(address,uint256,bytes)[],address,uint256,(address,bytes)[],uint128)` and selector `0x5e91ddfeb7bf2e12f7e8ab017d2b63a9217f004a15a53346ad90353ec63d14e4`.
    ```solidity
    event ERC1155OrderPreSigned(LibNFTOrder.TradeDirection direction, address maker, address taker, uint256 expiry, uint256 nonce, address erc20Token, uint256 erc20TokenAmount, LibNFTOrder.Fee[] fees, address erc1155Token, uint256 erc1155TokenId, LibNFTOrder.Property[] erc1155TokenProperties, uint128 erc1155TokenAmount);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct ERC1155OrderPreSigned {
        #[allow(missing_docs)]
        pub direction: <LibNFTOrder::TradeDirection as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub maker: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub taker: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub expiry: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub nonce: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub erc20Token: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub erc20TokenAmount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub fees:
            alloy_sol_types::private::Vec<<LibNFTOrder::Fee as alloy_sol_types::SolType>::RustType>,
        #[allow(missing_docs)]
        pub erc1155Token: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub erc1155TokenId: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub erc1155TokenProperties: alloy_sol_types::private::Vec<
            <LibNFTOrder::Property as alloy_sol_types::SolType>::RustType,
        >,
        #[allow(missing_docs)]
        pub erc1155TokenAmount: u128,
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
        impl alloy_sol_types::SolEvent for ERC1155OrderPreSigned {
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type DataTuple<'a> = (
                LibNFTOrder::TradeDirection,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Array<LibNFTOrder::Fee>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Array<LibNFTOrder::Property>,
                alloy_sol_types::sol_data::Uint<128>,
            );
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);

            const ANONYMOUS: bool = false;
            const SIGNATURE: &'static str = "ERC1155OrderPreSigned(uint8,address,address,uint256,\
                                             uint256,address,uint256,(address,uint256,bytes)[],\
                                             address,uint256,(address,bytes)[],uint128)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    94u8, 145u8, 221u8, 254u8, 183u8, 191u8, 46u8, 18u8, 247u8, 232u8, 171u8, 1u8,
                    125u8, 43u8, 99u8, 169u8, 33u8, 127u8, 0u8, 74u8, 21u8, 165u8, 51u8, 70u8,
                    173u8, 144u8, 53u8, 62u8, 198u8, 61u8, 20u8, 228u8,
                ]);

            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    direction: data.0,
                    maker: data.1,
                    taker: data.2,
                    expiry: data.3,
                    nonce: data.4,
                    erc20Token: data.5,
                    erc20TokenAmount: data.6,
                    fees: data.7,
                    erc1155Token: data.8,
                    erc1155TokenId: data.9,
                    erc1155TokenProperties: data.10,
                    erc1155TokenAmount: data.11,
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
                    <LibNFTOrder::TradeDirection as alloy_sol_types::SolType>::tokenize(
                        &self.direction,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.maker,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.taker,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.expiry),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.nonce),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.erc20Token,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.erc20TokenAmount),
                    <alloy_sol_types::sol_data::Array<
                        LibNFTOrder::Fee,
                    > as alloy_sol_types::SolType>::tokenize(&self.fees),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.erc1155Token,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.erc1155TokenId),
                    <alloy_sol_types::sol_data::Array<
                        LibNFTOrder::Property,
                    > as alloy_sol_types::SolType>::tokenize(
                        &self.erc1155TokenProperties,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        128,
                    > as alloy_sol_types::SolType>::tokenize(&self.erc1155TokenAmount),
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
        impl alloy_sol_types::private::IntoLogData for ERC1155OrderPreSigned {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }

            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&ERC1155OrderPreSigned> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &ERC1155OrderPreSigned) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `ERC721OrderCancelled(address,uint256)` and selector `0xa015ad2dc32f266993958a0fd9884c746b971b254206f3478bc43e2f125c7b9e`.
    ```solidity
    event ERC721OrderCancelled(address maker, uint256 nonce);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct ERC721OrderCancelled {
        #[allow(missing_docs)]
        pub maker: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub nonce: alloy_sol_types::private::primitives::aliases::U256,
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
        impl alloy_sol_types::SolEvent for ERC721OrderCancelled {
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);

            const ANONYMOUS: bool = false;
            const SIGNATURE: &'static str = "ERC721OrderCancelled(address,uint256)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    160u8, 21u8, 173u8, 45u8, 195u8, 47u8, 38u8, 105u8, 147u8, 149u8, 138u8, 15u8,
                    217u8, 136u8, 76u8, 116u8, 107u8, 151u8, 27u8, 37u8, 66u8, 6u8, 243u8, 71u8,
                    139u8, 196u8, 62u8, 47u8, 18u8, 92u8, 123u8, 158u8,
                ]);

            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    maker: data.0,
                    nonce: data.1,
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
                        &self.maker,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.nonce,
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
        impl alloy_sol_types::private::IntoLogData for ERC721OrderCancelled {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }

            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&ERC721OrderCancelled> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &ERC721OrderCancelled) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `ERC721OrderFilled(uint8,address,address,uint256,address,uint256,address,uint256,address)` and selector `0x50273fa02273cceea9cf085b42de5c8af60624140168bd71357db833535877af`.
    ```solidity
    event ERC721OrderFilled(LibNFTOrder.TradeDirection direction, address maker, address taker, uint256 nonce, address erc20Token, uint256 erc20TokenAmount, address erc721Token, uint256 erc721TokenId, address matcher);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct ERC721OrderFilled {
        #[allow(missing_docs)]
        pub direction: <LibNFTOrder::TradeDirection as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub maker: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub taker: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub nonce: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub erc20Token: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub erc20TokenAmount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub erc721Token: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub erc721TokenId: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub matcher: alloy_sol_types::private::Address,
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
        impl alloy_sol_types::SolEvent for ERC721OrderFilled {
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type DataTuple<'a> = (
                LibNFTOrder::TradeDirection,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Address,
            );
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);

            const ANONYMOUS: bool = false;
            const SIGNATURE: &'static str = "ERC721OrderFilled(uint8,address,address,uint256,\
                                             address,uint256,address,uint256,address)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    80u8, 39u8, 63u8, 160u8, 34u8, 115u8, 204u8, 238u8, 169u8, 207u8, 8u8, 91u8,
                    66u8, 222u8, 92u8, 138u8, 246u8, 6u8, 36u8, 20u8, 1u8, 104u8, 189u8, 113u8,
                    53u8, 125u8, 184u8, 51u8, 83u8, 88u8, 119u8, 175u8,
                ]);

            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    direction: data.0,
                    maker: data.1,
                    taker: data.2,
                    nonce: data.3,
                    erc20Token: data.4,
                    erc20TokenAmount: data.5,
                    erc721Token: data.6,
                    erc721TokenId: data.7,
                    matcher: data.8,
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
                    <LibNFTOrder::TradeDirection as alloy_sol_types::SolType>::tokenize(
                        &self.direction,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.maker,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.taker,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.nonce,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.erc20Token,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.erc20TokenAmount,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.erc721Token,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.erc721TokenId,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.matcher,
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
        impl alloy_sol_types::private::IntoLogData for ERC721OrderFilled {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }

            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&ERC721OrderFilled> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &ERC721OrderFilled) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `ERC721OrderPreSigned(uint8,address,address,uint256,uint256,address,uint256,(address,uint256,bytes)[],address,uint256,(address,bytes)[])` and selector `0x8c5d0c41fb16a7317a6c55ff7ba93d9d74f79e434fefa694e50d6028afbfa3f0`.
    ```solidity
    event ERC721OrderPreSigned(LibNFTOrder.TradeDirection direction, address maker, address taker, uint256 expiry, uint256 nonce, address erc20Token, uint256 erc20TokenAmount, LibNFTOrder.Fee[] fees, address erc721Token, uint256 erc721TokenId, LibNFTOrder.Property[] erc721TokenProperties);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct ERC721OrderPreSigned {
        #[allow(missing_docs)]
        pub direction: <LibNFTOrder::TradeDirection as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub maker: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub taker: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub expiry: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub nonce: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub erc20Token: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub erc20TokenAmount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub fees:
            alloy_sol_types::private::Vec<<LibNFTOrder::Fee as alloy_sol_types::SolType>::RustType>,
        #[allow(missing_docs)]
        pub erc721Token: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub erc721TokenId: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub erc721TokenProperties: alloy_sol_types::private::Vec<
            <LibNFTOrder::Property as alloy_sol_types::SolType>::RustType,
        >,
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
        impl alloy_sol_types::SolEvent for ERC721OrderPreSigned {
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type DataTuple<'a> = (
                LibNFTOrder::TradeDirection,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Array<LibNFTOrder::Fee>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Array<LibNFTOrder::Property>,
            );
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);

            const ANONYMOUS: bool = false;
            const SIGNATURE: &'static str = "ERC721OrderPreSigned(uint8,address,address,uint256,\
                                             uint256,address,uint256,(address,uint256,bytes)[],\
                                             address,uint256,(address,bytes)[])";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    140u8, 93u8, 12u8, 65u8, 251u8, 22u8, 167u8, 49u8, 122u8, 108u8, 85u8, 255u8,
                    123u8, 169u8, 61u8, 157u8, 116u8, 247u8, 158u8, 67u8, 79u8, 239u8, 166u8,
                    148u8, 229u8, 13u8, 96u8, 40u8, 175u8, 191u8, 163u8, 240u8,
                ]);

            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    direction: data.0,
                    maker: data.1,
                    taker: data.2,
                    expiry: data.3,
                    nonce: data.4,
                    erc20Token: data.5,
                    erc20TokenAmount: data.6,
                    fees: data.7,
                    erc721Token: data.8,
                    erc721TokenId: data.9,
                    erc721TokenProperties: data.10,
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
                    <LibNFTOrder::TradeDirection as alloy_sol_types::SolType>::tokenize(
                        &self.direction,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.maker,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.taker,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.expiry),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.nonce),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.erc20Token,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.erc20TokenAmount),
                    <alloy_sol_types::sol_data::Array<
                        LibNFTOrder::Fee,
                    > as alloy_sol_types::SolType>::tokenize(&self.fees),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.erc721Token,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.erc721TokenId),
                    <alloy_sol_types::sol_data::Array<
                        LibNFTOrder::Property,
                    > as alloy_sol_types::SolType>::tokenize(&self.erc721TokenProperties),
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
        impl alloy_sol_types::private::IntoLogData for ERC721OrderPreSigned {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }

            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&ERC721OrderPreSigned> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &ERC721OrderPreSigned) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `LimitOrderFilled(bytes32,address,address,address,address,address,uint128,uint128,uint128,uint256,bytes32)` and selector `0xab614d2b738543c0ea21f56347cf696a3a0c42a7cbec3212a5ca22a4dcff2124`.
    ```solidity
    event LimitOrderFilled(bytes32 orderHash, address maker, address taker, address feeRecipient, address makerToken, address takerToken, uint128 takerTokenFilledAmount, uint128 makerTokenFilledAmount, uint128 takerTokenFeeFilledAmount, uint256 protocolFeePaid, bytes32 pool);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct LimitOrderFilled {
        #[allow(missing_docs)]
        pub orderHash: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub maker: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub taker: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub feeRecipient: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub makerToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub takerToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub takerTokenFilledAmount: u128,
        #[allow(missing_docs)]
        pub makerTokenFilledAmount: u128,
        #[allow(missing_docs)]
        pub takerTokenFeeFilledAmount: u128,
        #[allow(missing_docs)]
        pub protocolFeePaid: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub pool: alloy_sol_types::private::FixedBytes<32>,
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
        impl alloy_sol_types::SolEvent for LimitOrderFilled {
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<128>,
                alloy_sol_types::sol_data::Uint<128>,
                alloy_sol_types::sol_data::Uint<128>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::FixedBytes<32>,
            );
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);

            const ANONYMOUS: bool = false;
            const SIGNATURE: &'static str = "LimitOrderFilled(bytes32,address,address,address,\
                                             address,address,uint128,uint128,uint128,uint256,\
                                             bytes32)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    171u8, 97u8, 77u8, 43u8, 115u8, 133u8, 67u8, 192u8, 234u8, 33u8, 245u8, 99u8,
                    71u8, 207u8, 105u8, 106u8, 58u8, 12u8, 66u8, 167u8, 203u8, 236u8, 50u8, 18u8,
                    165u8, 202u8, 34u8, 164u8, 220u8, 255u8, 33u8, 36u8,
                ]);

            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    orderHash: data.0,
                    maker: data.1,
                    taker: data.2,
                    feeRecipient: data.3,
                    makerToken: data.4,
                    takerToken: data.5,
                    takerTokenFilledAmount: data.6,
                    makerTokenFilledAmount: data.7,
                    takerTokenFeeFilledAmount: data.8,
                    protocolFeePaid: data.9,
                    pool: data.10,
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
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self.orderHash),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.maker,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.taker,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.feeRecipient,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.makerToken,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.takerToken,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        128,
                    > as alloy_sol_types::SolType>::tokenize(
                        &self.takerTokenFilledAmount,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        128,
                    > as alloy_sol_types::SolType>::tokenize(
                        &self.makerTokenFilledAmount,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        128,
                    > as alloy_sol_types::SolType>::tokenize(
                        &self.takerTokenFeeFilledAmount,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.protocolFeePaid),
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self.pool),
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
        impl alloy_sol_types::private::IntoLogData for LimitOrderFilled {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }

            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&LimitOrderFilled> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &LimitOrderFilled) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `LiquidityProviderSwap(address,address,uint256,uint256,address,address)` and selector `0x40a6ba9513d09e3488135e0e0d10e2d4382b792720155b144cbea89ac9db6d34`.
    ```solidity
    event LiquidityProviderSwap(address inputToken, address outputToken, uint256 inputTokenAmount, uint256 outputTokenAmount, address provider, address recipient);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct LiquidityProviderSwap {
        #[allow(missing_docs)]
        pub inputToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub outputToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub inputTokenAmount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub outputTokenAmount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub provider: alloy_sol_types::private::Address,
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
        impl alloy_sol_types::SolEvent for LiquidityProviderSwap {
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
            );
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);

            const ANONYMOUS: bool = false;
            const SIGNATURE: &'static str =
                "LiquidityProviderSwap(address,address,uint256,uint256,address,address)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    64u8, 166u8, 186u8, 149u8, 19u8, 208u8, 158u8, 52u8, 136u8, 19u8, 94u8, 14u8,
                    13u8, 16u8, 226u8, 212u8, 56u8, 43u8, 121u8, 39u8, 32u8, 21u8, 91u8, 20u8,
                    76u8, 190u8, 168u8, 154u8, 201u8, 219u8, 109u8, 52u8,
                ]);

            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    inputToken: data.0,
                    outputToken: data.1,
                    inputTokenAmount: data.2,
                    outputTokenAmount: data.3,
                    provider: data.4,
                    recipient: data.5,
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
                        &self.inputToken,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.outputToken,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.inputTokenAmount,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.outputTokenAmount,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.provider,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.recipient,
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
        impl alloy_sol_types::private::IntoLogData for LiquidityProviderSwap {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }

            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&LiquidityProviderSwap> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &LiquidityProviderSwap) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `MetaTransactionExecuted(bytes32,bytes4,address,address)` and selector `0x7f4fe3ff8ae440e1570c558da08440b26f89fb1c1f2910cd91ca6452955f121a`.
    ```solidity
    event MetaTransactionExecuted(bytes32 hash, bytes4 indexed selector, address signer, address sender);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct MetaTransactionExecuted {
        #[allow(missing_docs)]
        pub hash: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub selector: alloy_sol_types::private::FixedBytes<4>,
        #[allow(missing_docs)]
        pub signer: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub sender: alloy_sol_types::private::Address,
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
        impl alloy_sol_types::SolEvent for MetaTransactionExecuted {
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
            );
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::FixedBytes<4>,
            );

            const ANONYMOUS: bool = false;
            const SIGNATURE: &'static str =
                "MetaTransactionExecuted(bytes32,bytes4,address,address)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    127u8, 79u8, 227u8, 255u8, 138u8, 228u8, 64u8, 225u8, 87u8, 12u8, 85u8, 141u8,
                    160u8, 132u8, 64u8, 178u8, 111u8, 137u8, 251u8, 28u8, 31u8, 41u8, 16u8, 205u8,
                    145u8, 202u8, 100u8, 82u8, 149u8, 95u8, 18u8, 26u8,
                ]);

            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    hash: data.0,
                    selector: topics.1,
                    signer: data.1,
                    sender: data.2,
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
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self.hash),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.signer,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.sender,
                    ),
                )
            }

            #[inline]
            fn topics(&self) -> <Self::TopicList as alloy_sol_types::SolType>::RustType {
                (Self::SIGNATURE_HASH.into(), self.selector.clone())
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
                    4,
                > as alloy_sol_types::EventTopic>::encode_topic(&self.selector);
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for MetaTransactionExecuted {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }

            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&MetaTransactionExecuted> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &MetaTransactionExecuted) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `Migrated(address,address,address)` and selector `0xe1b831b0e6f3aa16b4b1a6bd526b5cdeab4940744ca6e0251f5fe5f8caf1c81a`.
    ```solidity
    event Migrated(address caller, address migrator, address newOwner);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct Migrated {
        #[allow(missing_docs)]
        pub caller: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub migrator: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub newOwner: alloy_sol_types::private::Address,
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
        impl alloy_sol_types::SolEvent for Migrated {
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
            );
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);

            const ANONYMOUS: bool = false;
            const SIGNATURE: &'static str = "Migrated(address,address,address)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    225u8, 184u8, 49u8, 176u8, 230u8, 243u8, 170u8, 22u8, 180u8, 177u8, 166u8,
                    189u8, 82u8, 107u8, 92u8, 222u8, 171u8, 73u8, 64u8, 116u8, 76u8, 166u8, 224u8,
                    37u8, 31u8, 95u8, 229u8, 248u8, 202u8, 241u8, 200u8, 26u8,
                ]);

            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    caller: data.0,
                    migrator: data.1,
                    newOwner: data.2,
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
                        &self.caller,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.migrator,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.newOwner,
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
        impl alloy_sol_types::private::IntoLogData for Migrated {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }

            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&Migrated> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &Migrated) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `OrderCancelled(bytes32,address)` and selector `0xa6eb7cdc219e1518ced964e9a34e61d68a94e4f1569db3e84256ba981ba52753`.
    ```solidity
    event OrderCancelled(bytes32 orderHash, address maker);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct OrderCancelled {
        #[allow(missing_docs)]
        pub orderHash: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub maker: alloy_sol_types::private::Address,
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
        impl alloy_sol_types::SolEvent for OrderCancelled {
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
            );
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);

            const ANONYMOUS: bool = false;
            const SIGNATURE: &'static str = "OrderCancelled(bytes32,address)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    166u8, 235u8, 124u8, 220u8, 33u8, 158u8, 21u8, 24u8, 206u8, 217u8, 100u8,
                    233u8, 163u8, 78u8, 97u8, 214u8, 138u8, 148u8, 228u8, 241u8, 86u8, 157u8,
                    179u8, 232u8, 66u8, 86u8, 186u8, 152u8, 27u8, 165u8, 39u8, 83u8,
                ]);

            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    orderHash: data.0,
                    maker: data.1,
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
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self.orderHash),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.maker,
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
        impl alloy_sol_types::private::IntoLogData for OrderCancelled {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }

            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&OrderCancelled> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &OrderCancelled) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `OrderSignerRegistered(address,address,bool)` and selector `0x6ea9dbe8b2cc119348716a9220a0742ad62b7884ecb0ff4b32cd508121fd9379`.
    ```solidity
    event OrderSignerRegistered(address maker, address signer, bool allowed);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct OrderSignerRegistered {
        #[allow(missing_docs)]
        pub maker: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub signer: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub allowed: bool,
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
        impl alloy_sol_types::SolEvent for OrderSignerRegistered {
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Bool,
            );
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);

            const ANONYMOUS: bool = false;
            const SIGNATURE: &'static str = "OrderSignerRegistered(address,address,bool)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    110u8, 169u8, 219u8, 232u8, 178u8, 204u8, 17u8, 147u8, 72u8, 113u8, 106u8,
                    146u8, 32u8, 160u8, 116u8, 42u8, 214u8, 43u8, 120u8, 132u8, 236u8, 176u8,
                    255u8, 75u8, 50u8, 205u8, 80u8, 129u8, 33u8, 253u8, 147u8, 121u8,
                ]);

            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    maker: data.0,
                    signer: data.1,
                    allowed: data.2,
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
                        &self.maker,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.signer,
                    ),
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::tokenize(
                        &self.allowed,
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
        impl alloy_sol_types::private::IntoLogData for OrderSignerRegistered {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }

            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&OrderSignerRegistered> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &OrderSignerRegistered) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `OtcOrderFilled(bytes32,address,address,address,address,uint128,uint128)` and selector `0xac75f773e3a92f1a02b12134d65e1f47f8a14eabe4eaf1e24624918e6a8b269f`.
    ```solidity
    event OtcOrderFilled(bytes32 orderHash, address maker, address taker, address makerToken, address takerToken, uint128 makerTokenFilledAmount, uint128 takerTokenFilledAmount);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct OtcOrderFilled {
        #[allow(missing_docs)]
        pub orderHash: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub maker: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub taker: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub makerToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub takerToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub makerTokenFilledAmount: u128,
        #[allow(missing_docs)]
        pub takerTokenFilledAmount: u128,
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
        impl alloy_sol_types::SolEvent for OtcOrderFilled {
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<128>,
                alloy_sol_types::sol_data::Uint<128>,
            );
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);

            const ANONYMOUS: bool = false;
            const SIGNATURE: &'static str =
                "OtcOrderFilled(bytes32,address,address,address,address,uint128,uint128)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    172u8, 117u8, 247u8, 115u8, 227u8, 169u8, 47u8, 26u8, 2u8, 177u8, 33u8, 52u8,
                    214u8, 94u8, 31u8, 71u8, 248u8, 161u8, 78u8, 171u8, 228u8, 234u8, 241u8, 226u8,
                    70u8, 36u8, 145u8, 142u8, 106u8, 139u8, 38u8, 159u8,
                ]);

            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    orderHash: data.0,
                    maker: data.1,
                    taker: data.2,
                    makerToken: data.3,
                    takerToken: data.4,
                    makerTokenFilledAmount: data.5,
                    takerTokenFilledAmount: data.6,
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
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self.orderHash),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.maker,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.taker,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.makerToken,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.takerToken,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        128,
                    > as alloy_sol_types::SolType>::tokenize(
                        &self.makerTokenFilledAmount,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        128,
                    > as alloy_sol_types::SolType>::tokenize(
                        &self.takerTokenFilledAmount,
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
        impl alloy_sol_types::private::IntoLogData for OtcOrderFilled {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }

            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&OtcOrderFilled> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &OtcOrderFilled) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `OwnershipTransferred(address,address)` and selector `0x8be0079c531659141344cd1fd0a4f28419497f9722a3daafe3b4186f6b6457e0`.
    ```solidity
    event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct OwnershipTransferred {
        #[allow(missing_docs)]
        pub previousOwner: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub newOwner: alloy_sol_types::private::Address,
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
        impl alloy_sol_types::SolEvent for OwnershipTransferred {
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type DataTuple<'a> = ();
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
            );

            const ANONYMOUS: bool = false;
            const SIGNATURE: &'static str = "OwnershipTransferred(address,address)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    139u8, 224u8, 7u8, 156u8, 83u8, 22u8, 89u8, 20u8, 19u8, 68u8, 205u8, 31u8,
                    208u8, 164u8, 242u8, 132u8, 25u8, 73u8, 127u8, 151u8, 34u8, 163u8, 218u8,
                    175u8, 227u8, 180u8, 24u8, 111u8, 107u8, 100u8, 87u8, 224u8,
                ]);

            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    previousOwner: topics.1,
                    newOwner: topics.2,
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
                ()
            }

            #[inline]
            fn topics(&self) -> <Self::TopicList as alloy_sol_types::SolType>::RustType {
                (
                    Self::SIGNATURE_HASH.into(),
                    self.previousOwner.clone(),
                    self.newOwner.clone(),
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
                out[0usize] = alloy_sol_types::abi::token::WordToken(Self::SIGNATURE_HASH);
                out[1usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.previousOwner,
                );
                out[2usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.newOwner,
                );
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for OwnershipTransferred {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }

            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&OwnershipTransferred> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &OwnershipTransferred) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `PairCancelledLimitOrders(address,address,address,uint256)` and selector `0xa91fe7ae62fce669df2c7f880f8c14d178531aae72515558e5c948e37c32a572`.
    ```solidity
    event PairCancelledLimitOrders(address maker, address makerToken, address takerToken, uint256 minValidSalt);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct PairCancelledLimitOrders {
        #[allow(missing_docs)]
        pub maker: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub makerToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub takerToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub minValidSalt: alloy_sol_types::private::primitives::aliases::U256,
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
        impl alloy_sol_types::SolEvent for PairCancelledLimitOrders {
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);

            const ANONYMOUS: bool = false;
            const SIGNATURE: &'static str =
                "PairCancelledLimitOrders(address,address,address,uint256)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    169u8, 31u8, 231u8, 174u8, 98u8, 252u8, 230u8, 105u8, 223u8, 44u8, 127u8,
                    136u8, 15u8, 140u8, 20u8, 209u8, 120u8, 83u8, 26u8, 174u8, 114u8, 81u8, 85u8,
                    88u8, 229u8, 201u8, 72u8, 227u8, 124u8, 50u8, 165u8, 114u8,
                ]);

            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    maker: data.0,
                    makerToken: data.1,
                    takerToken: data.2,
                    minValidSalt: data.3,
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
                        &self.maker,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.makerToken,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.takerToken,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.minValidSalt,
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
        impl alloy_sol_types::private::IntoLogData for PairCancelledLimitOrders {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }

            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&PairCancelledLimitOrders> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &PairCancelledLimitOrders) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `PairCancelledRfqOrders(address,address,address,uint256)` and selector `0xfe7ffb1edfe79f4df716cb2dcad21cf2f31b104d816a7976ba1e6e4653c1efb1`.
    ```solidity
    event PairCancelledRfqOrders(address maker, address makerToken, address takerToken, uint256 minValidSalt);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct PairCancelledRfqOrders {
        #[allow(missing_docs)]
        pub maker: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub makerToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub takerToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub minValidSalt: alloy_sol_types::private::primitives::aliases::U256,
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
        impl alloy_sol_types::SolEvent for PairCancelledRfqOrders {
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);

            const ANONYMOUS: bool = false;
            const SIGNATURE: &'static str =
                "PairCancelledRfqOrders(address,address,address,uint256)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    254u8, 127u8, 251u8, 30u8, 223u8, 231u8, 159u8, 77u8, 247u8, 22u8, 203u8, 45u8,
                    202u8, 210u8, 28u8, 242u8, 243u8, 27u8, 16u8, 77u8, 129u8, 106u8, 121u8, 118u8,
                    186u8, 30u8, 110u8, 70u8, 83u8, 193u8, 239u8, 177u8,
                ]);

            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    maker: data.0,
                    makerToken: data.1,
                    takerToken: data.2,
                    minValidSalt: data.3,
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
                        &self.maker,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.makerToken,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.takerToken,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.minValidSalt,
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
        impl alloy_sol_types::private::IntoLogData for PairCancelledRfqOrders {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }

            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&PairCancelledRfqOrders> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &PairCancelledRfqOrders) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `ProxyFunctionUpdated(bytes4,address,address)` and selector `0x2ae221083467de52078b0096696ab88d8d53a7ecb44bb65b56a2bab687598367`.
    ```solidity
    event ProxyFunctionUpdated(bytes4 indexed selector, address oldImpl, address newImpl);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct ProxyFunctionUpdated {
        #[allow(missing_docs)]
        pub selector: alloy_sol_types::private::FixedBytes<4>,
        #[allow(missing_docs)]
        pub oldImpl: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub newImpl: alloy_sol_types::private::Address,
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
        impl alloy_sol_types::SolEvent for ProxyFunctionUpdated {
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
            );
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::FixedBytes<4>,
            );

            const ANONYMOUS: bool = false;
            const SIGNATURE: &'static str = "ProxyFunctionUpdated(bytes4,address,address)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    42u8, 226u8, 33u8, 8u8, 52u8, 103u8, 222u8, 82u8, 7u8, 139u8, 0u8, 150u8,
                    105u8, 106u8, 184u8, 141u8, 141u8, 83u8, 167u8, 236u8, 180u8, 75u8, 182u8,
                    91u8, 86u8, 162u8, 186u8, 182u8, 135u8, 89u8, 131u8, 103u8,
                ]);

            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    selector: topics.1,
                    oldImpl: data.0,
                    newImpl: data.1,
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
                        &self.oldImpl,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.newImpl,
                    ),
                )
            }

            #[inline]
            fn topics(&self) -> <Self::TopicList as alloy_sol_types::SolType>::RustType {
                (Self::SIGNATURE_HASH.into(), self.selector.clone())
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
                    4,
                > as alloy_sol_types::EventTopic>::encode_topic(&self.selector);
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for ProxyFunctionUpdated {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }

            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&ProxyFunctionUpdated> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &ProxyFunctionUpdated) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `QuoteSignerUpdated(address)` and selector `0xf5550c5eea19b48ac6eb5f03abdc4f59c0a60697abb3d973cd68669703b5c8b9`.
    ```solidity
    event QuoteSignerUpdated(address quoteSigner);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct QuoteSignerUpdated {
        #[allow(missing_docs)]
        pub quoteSigner: alloy_sol_types::private::Address,
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
        impl alloy_sol_types::SolEvent for QuoteSignerUpdated {
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type DataTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);

            const ANONYMOUS: bool = false;
            const SIGNATURE: &'static str = "QuoteSignerUpdated(address)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    245u8, 85u8, 12u8, 94u8, 234u8, 25u8, 180u8, 138u8, 198u8, 235u8, 95u8, 3u8,
                    171u8, 220u8, 79u8, 89u8, 192u8, 166u8, 6u8, 151u8, 171u8, 179u8, 217u8, 115u8,
                    205u8, 104u8, 102u8, 151u8, 3u8, 181u8, 200u8, 185u8,
                ]);

            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    quoteSigner: data.0,
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
                        &self.quoteSigner,
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
        impl alloy_sol_types::private::IntoLogData for QuoteSignerUpdated {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }

            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&QuoteSignerUpdated> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &QuoteSignerUpdated) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `RfqOrderFilled(bytes32,address,address,address,address,uint128,uint128,bytes32)` and selector `0x829fa99d94dc4636925b38632e625736a614c154d55006b7ab6bea979c210c32`.
    ```solidity
    event RfqOrderFilled(bytes32 orderHash, address maker, address taker, address makerToken, address takerToken, uint128 takerTokenFilledAmount, uint128 makerTokenFilledAmount, bytes32 pool);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct RfqOrderFilled {
        #[allow(missing_docs)]
        pub orderHash: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub maker: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub taker: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub makerToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub takerToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub takerTokenFilledAmount: u128,
        #[allow(missing_docs)]
        pub makerTokenFilledAmount: u128,
        #[allow(missing_docs)]
        pub pool: alloy_sol_types::private::FixedBytes<32>,
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
        impl alloy_sol_types::SolEvent for RfqOrderFilled {
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<128>,
                alloy_sol_types::sol_data::Uint<128>,
                alloy_sol_types::sol_data::FixedBytes<32>,
            );
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);

            const ANONYMOUS: bool = false;
            const SIGNATURE: &'static str =
                "RfqOrderFilled(bytes32,address,address,address,address,uint128,uint128,bytes32)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    130u8, 159u8, 169u8, 157u8, 148u8, 220u8, 70u8, 54u8, 146u8, 91u8, 56u8, 99u8,
                    46u8, 98u8, 87u8, 54u8, 166u8, 20u8, 193u8, 84u8, 213u8, 80u8, 6u8, 183u8,
                    171u8, 107u8, 234u8, 151u8, 156u8, 33u8, 12u8, 50u8,
                ]);

            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    orderHash: data.0,
                    maker: data.1,
                    taker: data.2,
                    makerToken: data.3,
                    takerToken: data.4,
                    takerTokenFilledAmount: data.5,
                    makerTokenFilledAmount: data.6,
                    pool: data.7,
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
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self.orderHash),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.maker,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.taker,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.makerToken,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.takerToken,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        128,
                    > as alloy_sol_types::SolType>::tokenize(
                        &self.takerTokenFilledAmount,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        128,
                    > as alloy_sol_types::SolType>::tokenize(
                        &self.makerTokenFilledAmount,
                    ),
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self.pool),
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
        impl alloy_sol_types::private::IntoLogData for RfqOrderFilled {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }

            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&RfqOrderFilled> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &RfqOrderFilled) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `RfqOrderOriginsAllowed(address,address[],bool)` and selector `0x02dfead5eb769b298e82dd9650b31c40559a3d42701dbf53c931bc2682847c31`.
    ```solidity
    event RfqOrderOriginsAllowed(address origin, address[] addrs, bool allowed);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct RfqOrderOriginsAllowed {
        #[allow(missing_docs)]
        pub origin: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub addrs: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
        #[allow(missing_docs)]
        pub allowed: bool,
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
        impl alloy_sol_types::SolEvent for RfqOrderOriginsAllowed {
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Bool,
            );
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);

            const ANONYMOUS: bool = false;
            const SIGNATURE: &'static str = "RfqOrderOriginsAllowed(address,address[],bool)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    2u8, 223u8, 234u8, 213u8, 235u8, 118u8, 155u8, 41u8, 142u8, 130u8, 221u8,
                    150u8, 80u8, 179u8, 28u8, 64u8, 85u8, 154u8, 61u8, 66u8, 112u8, 29u8, 191u8,
                    83u8, 201u8, 49u8, 188u8, 38u8, 130u8, 132u8, 124u8, 49u8,
                ]);

            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    origin: data.0,
                    addrs: data.1,
                    allowed: data.2,
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
                        &self.origin,
                    ),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Address,
                    > as alloy_sol_types::SolType>::tokenize(&self.addrs),
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::tokenize(
                        &self.allowed,
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
        impl alloy_sol_types::private::IntoLogData for RfqOrderOriginsAllowed {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }

            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&RfqOrderOriginsAllowed> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &RfqOrderOriginsAllowed) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `TransformedERC20(address,address,address,uint256,uint256)` and selector `0x0f6672f78a59ba8e5e5b5d38df3ebc67f3c792e2c9259b8d97d7f00dd78ba1b3`.
    ```solidity
    event TransformedERC20(address indexed taker, address inputToken, address outputToken, uint256 inputTokenAmount, uint256 outputTokenAmount);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct TransformedERC20 {
        #[allow(missing_docs)]
        pub taker: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub inputToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub outputToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub inputTokenAmount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub outputTokenAmount: alloy_sol_types::private::primitives::aliases::U256,
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
        impl alloy_sol_types::SolEvent for TransformedERC20 {
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
            );

            const ANONYMOUS: bool = false;
            const SIGNATURE: &'static str =
                "TransformedERC20(address,address,address,uint256,uint256)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    15u8, 102u8, 114u8, 247u8, 138u8, 89u8, 186u8, 142u8, 94u8, 91u8, 93u8, 56u8,
                    223u8, 62u8, 188u8, 103u8, 243u8, 199u8, 146u8, 226u8, 201u8, 37u8, 155u8,
                    141u8, 151u8, 215u8, 240u8, 13u8, 215u8, 139u8, 161u8, 179u8,
                ]);

            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    taker: topics.1,
                    inputToken: data.0,
                    outputToken: data.1,
                    inputTokenAmount: data.2,
                    outputTokenAmount: data.3,
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
                        &self.inputToken,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.outputToken,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.inputTokenAmount,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.outputTokenAmount,
                    ),
                )
            }

            #[inline]
            fn topics(&self) -> <Self::TopicList as alloy_sol_types::SolType>::RustType {
                (Self::SIGNATURE_HASH.into(), self.taker.clone())
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
                    &self.taker,
                );
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for TransformedERC20 {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }

            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&TransformedERC20> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &TransformedERC20) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `TransformerDeployerUpdated(address)` and selector `0xfd45604abad79c16e23348a137ed8292661be1b8eba6e4806ebed6833b1c046a`.
    ```solidity
    event TransformerDeployerUpdated(address transformerDeployer);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct TransformerDeployerUpdated {
        #[allow(missing_docs)]
        pub transformerDeployer: alloy_sol_types::private::Address,
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
        impl alloy_sol_types::SolEvent for TransformerDeployerUpdated {
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type DataTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);

            const ANONYMOUS: bool = false;
            const SIGNATURE: &'static str = "TransformerDeployerUpdated(address)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    253u8, 69u8, 96u8, 74u8, 186u8, 215u8, 156u8, 22u8, 226u8, 51u8, 72u8, 161u8,
                    55u8, 237u8, 130u8, 146u8, 102u8, 27u8, 225u8, 184u8, 235u8, 166u8, 228u8,
                    128u8, 110u8, 190u8, 214u8, 131u8, 59u8, 28u8, 4u8, 106u8,
                ]);

            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    transformerDeployer: data.0,
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
                        &self.transformerDeployer,
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
        impl alloy_sol_types::private::IntoLogData for TransformerDeployerUpdated {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }

            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&TransformerDeployerUpdated> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &TransformerDeployerUpdated) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `extend(bytes4,address)` and selector `0x6eb224cb`.
    ```solidity
    function extend(bytes4 selector, address r#impl) external;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct extendCall {
        #[allow(missing_docs)]
        pub selector: alloy_sol_types::private::FixedBytes<4>,
        #[allow(missing_docs)]
        pub r#impl: alloy_sol_types::private::Address,
    }
    ///Container type for the return parameters of the
    /// [`extend(bytes4,address)`](extendCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct extendReturn {}
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
                alloy_sol_types::sol_data::FixedBytes<4>,
                alloy_sol_types::sol_data::Address,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::FixedBytes<4>,
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
            impl ::core::convert::From<extendCall> for UnderlyingRustTuple<'_> {
                fn from(value: extendCall) -> Self {
                    (value.selector, value.r#impl)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for extendCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        selector: tuple.0,
                        r#impl: tuple.1,
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
            impl ::core::convert::From<extendReturn> for UnderlyingRustTuple<'_> {
                fn from(value: extendReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for extendReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl extendReturn {
            fn _tokenize(&self) -> <extendCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for extendCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::FixedBytes<4>,
                alloy_sol_types::sol_data::Address,
            );
            type Return = extendReturn;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = ();
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [110u8, 178u8, 36u8, 203u8];
            const SIGNATURE: &'static str = "extend(bytes4,address)";

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
                        4,
                    > as alloy_sol_types::SolType>::tokenize(&self.selector),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.r#impl,
                    ),
                )
            }

            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                extendReturn::_tokenize(ret)
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
    /**Function with signature `fillOrKillLimitOrder((address,address,uint128,uint128,uint128,address,address,address,address,bytes32,uint64,uint256),(uint8,uint8,bytes32,bytes32),uint128)` and selector `0x9240529c`.
    ```solidity
    function fillOrKillLimitOrder(LibNativeOrder.LimitOrder memory order, LibSignature.Signature memory signature, uint128 takerTokenFillAmount) external payable returns (uint128 makerTokenFilledAmount);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct fillOrKillLimitOrderCall {
        #[allow(missing_docs)]
        pub order: <LibNativeOrder::LimitOrder as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub signature: <LibSignature::Signature as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub takerTokenFillAmount: u128,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`fillOrKillLimitOrder((address,address,uint128,uint128,uint128,address,
    /// address,address,address,bytes32,uint64,uint256),(uint8,uint8,bytes32,
    /// bytes32),uint128)`](fillOrKillLimitOrderCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct fillOrKillLimitOrderReturn {
        #[allow(missing_docs)]
        pub makerTokenFilledAmount: u128,
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
                LibNativeOrder::LimitOrder,
                LibSignature::Signature,
                alloy_sol_types::sol_data::Uint<128>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                <LibNativeOrder::LimitOrder as alloy_sol_types::SolType>::RustType,
                <LibSignature::Signature as alloy_sol_types::SolType>::RustType,
                u128,
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
            impl ::core::convert::From<fillOrKillLimitOrderCall> for UnderlyingRustTuple<'_> {
                fn from(value: fillOrKillLimitOrderCall) -> Self {
                    (value.order, value.signature, value.takerTokenFillAmount)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for fillOrKillLimitOrderCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        order: tuple.0,
                        signature: tuple.1,
                        takerTokenFillAmount: tuple.2,
                    }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (alloy_sol_types::sol_data::Uint<128>,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (u128,);
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
            impl ::core::convert::From<fillOrKillLimitOrderReturn> for UnderlyingRustTuple<'_> {
                fn from(value: fillOrKillLimitOrderReturn) -> Self {
                    (value.makerTokenFilledAmount,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for fillOrKillLimitOrderReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        makerTokenFilledAmount: tuple.0,
                    }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for fillOrKillLimitOrderCall {
            type Parameters<'a> = (
                LibNativeOrder::LimitOrder,
                LibSignature::Signature,
                alloy_sol_types::sol_data::Uint<128>,
            );
            type Return = u128;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Uint<128>,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [146u8, 64u8, 82u8, 156u8];
            const SIGNATURE: &'static str =
                "fillOrKillLimitOrder((address,address,uint128,uint128,uint128,address,address,\
                 address,address,bytes32,uint64,uint256),(uint8,uint8,bytes32,bytes32),uint128)";

            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }

            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <LibNativeOrder::LimitOrder as alloy_sol_types::SolType>::tokenize(&self.order),
                    <LibSignature::Signature as alloy_sol_types::SolType>::tokenize(
                        &self.signature,
                    ),
                    <alloy_sol_types::sol_data::Uint<128> as alloy_sol_types::SolType>::tokenize(
                        &self.takerTokenFillAmount,
                    ),
                )
            }

            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                (
                    <alloy_sol_types::sol_data::Uint<128> as alloy_sol_types::SolType>::tokenize(
                        ret,
                    ),
                )
            }

            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence(data).map(
                    |r| {
                        let r: fillOrKillLimitOrderReturn = r.into();
                        r.makerTokenFilledAmount
                    },
                )
            }

            #[inline]
            fn abi_decode_returns_validate(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence_validate(
                    data,
                )
                .map(|r| {
                    let r: fillOrKillLimitOrderReturn = r.into();
                    r.makerTokenFilledAmount
                })
            }
        }
    };
    #[derive()]
    /**Function with signature `getLimitOrderRelevantState((address,address,uint128,uint128,uint128,address,address,address,address,bytes32,uint64,uint256),(uint8,uint8,bytes32,bytes32))` and selector `0x1fb09795`.
    ```solidity
    function getLimitOrderRelevantState(LibNativeOrder.LimitOrder memory order, LibSignature.Signature memory signature) external view returns (LibNativeOrder.OrderInfo memory orderInfo, uint128 actualFillableTakerTokenAmount, bool isSignatureValid);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct getLimitOrderRelevantStateCall {
        #[allow(missing_docs)]
        pub order: <LibNativeOrder::LimitOrder as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub signature: <LibSignature::Signature as alloy_sol_types::SolType>::RustType,
    }
    #[derive()]
    ///Container type for the return parameters of the
    /// [`getLimitOrderRelevantState((address,address,uint128,uint128,uint128,
    /// address,address,address,address,bytes32,uint64,uint256),(uint8,uint8,
    /// bytes32,bytes32))`](getLimitOrderRelevantStateCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct getLimitOrderRelevantStateReturn {
        #[allow(missing_docs)]
        pub orderInfo: <LibNativeOrder::OrderInfo as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub actualFillableTakerTokenAmount: u128,
        #[allow(missing_docs)]
        pub isSignatureValid: bool,
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
            type UnderlyingSolTuple<'a> = (LibNativeOrder::LimitOrder, LibSignature::Signature);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                <LibNativeOrder::LimitOrder as alloy_sol_types::SolType>::RustType,
                <LibSignature::Signature as alloy_sol_types::SolType>::RustType,
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
            impl ::core::convert::From<getLimitOrderRelevantStateCall> for UnderlyingRustTuple<'_> {
                fn from(value: getLimitOrderRelevantStateCall) -> Self {
                    (value.order, value.signature)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for getLimitOrderRelevantStateCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        order: tuple.0,
                        signature: tuple.1,
                    }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                LibNativeOrder::OrderInfo,
                alloy_sol_types::sol_data::Uint<128>,
                alloy_sol_types::sol_data::Bool,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                <LibNativeOrder::OrderInfo as alloy_sol_types::SolType>::RustType,
                u128,
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
            impl ::core::convert::From<getLimitOrderRelevantStateReturn> for UnderlyingRustTuple<'_> {
                fn from(value: getLimitOrderRelevantStateReturn) -> Self {
                    (
                        value.orderInfo,
                        value.actualFillableTakerTokenAmount,
                        value.isSignatureValid,
                    )
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for getLimitOrderRelevantStateReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        orderInfo: tuple.0,
                        actualFillableTakerTokenAmount: tuple.1,
                        isSignatureValid: tuple.2,
                    }
                }
            }
        }
        impl getLimitOrderRelevantStateReturn {
            fn _tokenize(
                &self,
            ) -> <getLimitOrderRelevantStateCall as alloy_sol_types::SolCall>::ReturnToken<'_>
            {
                (
                    <LibNativeOrder::OrderInfo as alloy_sol_types::SolType>::tokenize(
                        &self.orderInfo,
                    ),
                    <alloy_sol_types::sol_data::Uint<128> as alloy_sol_types::SolType>::tokenize(
                        &self.actualFillableTakerTokenAmount,
                    ),
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::tokenize(
                        &self.isSignatureValid,
                    ),
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for getLimitOrderRelevantStateCall {
            type Parameters<'a> = (LibNativeOrder::LimitOrder, LibSignature::Signature);
            type Return = getLimitOrderRelevantStateReturn;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (
                LibNativeOrder::OrderInfo,
                alloy_sol_types::sol_data::Uint<128>,
                alloy_sol_types::sol_data::Bool,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [31u8, 176u8, 151u8, 149u8];
            const SIGNATURE: &'static str =
                "getLimitOrderRelevantState((address,address,uint128,uint128,uint128,address,\
                 address,address,address,bytes32,uint64,uint256),(uint8,uint8,bytes32,bytes32))";

            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }

            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <LibNativeOrder::LimitOrder as alloy_sol_types::SolType>::tokenize(&self.order),
                    <LibSignature::Signature as alloy_sol_types::SolType>::tokenize(
                        &self.signature,
                    ),
                )
            }

            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                getLimitOrderRelevantStateReturn::_tokenize(ret)
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
    /**Function with signature `owner()` and selector `0x8da5cb5b`.
    ```solidity
    function owner() external view returns (address ownerAddress);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct ownerCall;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`owner()`](ownerCall)
    /// function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct ownerReturn {
        #[allow(missing_docs)]
        pub ownerAddress: alloy_sol_types::private::Address,
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
            impl ::core::convert::From<ownerCall> for UnderlyingRustTuple<'_> {
                fn from(value: ownerCall) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for ownerCall {
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
            impl ::core::convert::From<ownerReturn> for UnderlyingRustTuple<'_> {
                fn from(value: ownerReturn) -> Self {
                    (value.ownerAddress,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for ownerReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        ownerAddress: tuple.0,
                    }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for ownerCall {
            type Parameters<'a> = ();
            type Return = alloy_sol_types::private::Address;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [141u8, 165u8, 203u8, 91u8];
            const SIGNATURE: &'static str = "owner()";

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
                        let r: ownerReturn = r.into();
                        r.ownerAddress
                    },
                )
            }

            #[inline]
            fn abi_decode_returns_validate(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence_validate(
                    data,
                )
                .map(|r| {
                    let r: ownerReturn = r.into();
                    r.ownerAddress
                })
            }
        }
    };
    ///Container for all the [`IZeroex`](self) function calls.
    #[derive(Clone)]
    pub enum IZeroexCalls {
        #[allow(missing_docs)]
        extend(extendCall),
        #[allow(missing_docs)]
        fillOrKillLimitOrder(fillOrKillLimitOrderCall),
        #[allow(missing_docs)]
        getLimitOrderRelevantState(getLimitOrderRelevantStateCall),
        #[allow(missing_docs)]
        owner(ownerCall),
    }
    impl IZeroexCalls {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the
        /// variants. No guarantees are made about the order of the
        /// selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 4usize]] = &[
            [31u8, 176u8, 151u8, 149u8],
            [110u8, 178u8, 36u8, 203u8],
            [141u8, 165u8, 203u8, 91u8],
            [146u8, 64u8, 82u8, 156u8],
        ];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <getLimitOrderRelevantStateCall as alloy_sol_types::SolCall>::SIGNATURE,
            <extendCall as alloy_sol_types::SolCall>::SIGNATURE,
            <ownerCall as alloy_sol_types::SolCall>::SIGNATURE,
            <fillOrKillLimitOrderCall as alloy_sol_types::SolCall>::SIGNATURE,
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(getLimitOrderRelevantState),
            ::core::stringify!(extend),
            ::core::stringify!(owner),
            ::core::stringify!(fillOrKillLimitOrder),
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
    impl alloy_sol_types::SolInterface for IZeroexCalls {
        const COUNT: usize = 4usize;
        const MIN_DATA_LENGTH: usize = 0usize;
        const NAME: &'static str = "IZeroexCalls";

        #[inline]
        fn selector(&self) -> [u8; 4] {
            match self {
                Self::extend(_) => <extendCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::fillOrKillLimitOrder(_) => {
                    <fillOrKillLimitOrderCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::getLimitOrderRelevantState(_) => {
                    <getLimitOrderRelevantStateCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::owner(_) => <ownerCall as alloy_sol_types::SolCall>::SELECTOR,
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
            static DECODE_SHIMS: &[fn(&[u8]) -> alloy_sol_types::Result<IZeroexCalls>] = &[
                {
                    fn getLimitOrderRelevantState(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<IZeroexCalls> {
                        <getLimitOrderRelevantStateCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(IZeroexCalls::getLimitOrderRelevantState)
                    }
                    getLimitOrderRelevantState
                },
                {
                    fn extend(data: &[u8]) -> alloy_sol_types::Result<IZeroexCalls> {
                        <extendCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(IZeroexCalls::extend)
                    }
                    extend
                },
                {
                    fn owner(data: &[u8]) -> alloy_sol_types::Result<IZeroexCalls> {
                        <ownerCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(IZeroexCalls::owner)
                    }
                    owner
                },
                {
                    fn fillOrKillLimitOrder(data: &[u8]) -> alloy_sol_types::Result<IZeroexCalls> {
                        <fillOrKillLimitOrderCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(IZeroexCalls::fillOrKillLimitOrder)
                    }
                    fillOrKillLimitOrder
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
            static DECODE_VALIDATE_SHIMS: &[fn(&[u8]) -> alloy_sol_types::Result<IZeroexCalls>] = &[
                {
                    fn getLimitOrderRelevantState(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<IZeroexCalls> {
                        <getLimitOrderRelevantStateCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(IZeroexCalls::getLimitOrderRelevantState)
                    }
                    getLimitOrderRelevantState
                },
                {
                    fn extend(data: &[u8]) -> alloy_sol_types::Result<IZeroexCalls> {
                        <extendCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(IZeroexCalls::extend)
                    }
                    extend
                },
                {
                    fn owner(data: &[u8]) -> alloy_sol_types::Result<IZeroexCalls> {
                        <ownerCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(IZeroexCalls::owner)
                    }
                    owner
                },
                {
                    fn fillOrKillLimitOrder(data: &[u8]) -> alloy_sol_types::Result<IZeroexCalls> {
                        <fillOrKillLimitOrderCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(IZeroexCalls::fillOrKillLimitOrder)
                    }
                    fillOrKillLimitOrder
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
                Self::extend(inner) => {
                    <extendCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::fillOrKillLimitOrder(inner) => {
                    <fillOrKillLimitOrderCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::getLimitOrderRelevantState(inner) => {
                    <getLimitOrderRelevantStateCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::owner(inner) => {
                    <ownerCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
            }
        }

        #[inline]
        fn abi_encode_raw(&self, out: &mut alloy_sol_types::private::Vec<u8>) {
            match self {
                Self::extend(inner) => {
                    <extendCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::fillOrKillLimitOrder(inner) => {
                    <fillOrKillLimitOrderCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner, out,
                    )
                }
                Self::getLimitOrderRelevantState(inner) => {
                    <getLimitOrderRelevantStateCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner, out,
                    )
                }
                Self::owner(inner) => {
                    <ownerCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
            }
        }
    }
    ///Container for all the [`IZeroex`](self) events.
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    pub enum IZeroexEvents {
        #[allow(missing_docs)]
        ERC1155OrderCancelled(ERC1155OrderCancelled),
        #[allow(missing_docs)]
        ERC1155OrderFilled(ERC1155OrderFilled),
        #[allow(missing_docs)]
        ERC1155OrderPreSigned(ERC1155OrderPreSigned),
        #[allow(missing_docs)]
        ERC721OrderCancelled(ERC721OrderCancelled),
        #[allow(missing_docs)]
        ERC721OrderFilled(ERC721OrderFilled),
        #[allow(missing_docs)]
        ERC721OrderPreSigned(ERC721OrderPreSigned),
        #[allow(missing_docs)]
        LimitOrderFilled(LimitOrderFilled),
        #[allow(missing_docs)]
        LiquidityProviderSwap(LiquidityProviderSwap),
        #[allow(missing_docs)]
        MetaTransactionExecuted(MetaTransactionExecuted),
        #[allow(missing_docs)]
        Migrated(Migrated),
        #[allow(missing_docs)]
        OrderCancelled(OrderCancelled),
        #[allow(missing_docs)]
        OrderSignerRegistered(OrderSignerRegistered),
        #[allow(missing_docs)]
        OtcOrderFilled(OtcOrderFilled),
        #[allow(missing_docs)]
        OwnershipTransferred(OwnershipTransferred),
        #[allow(missing_docs)]
        PairCancelledLimitOrders(PairCancelledLimitOrders),
        #[allow(missing_docs)]
        PairCancelledRfqOrders(PairCancelledRfqOrders),
        #[allow(missing_docs)]
        ProxyFunctionUpdated(ProxyFunctionUpdated),
        #[allow(missing_docs)]
        QuoteSignerUpdated(QuoteSignerUpdated),
        #[allow(missing_docs)]
        RfqOrderFilled(RfqOrderFilled),
        #[allow(missing_docs)]
        RfqOrderOriginsAllowed(RfqOrderOriginsAllowed),
        #[allow(missing_docs)]
        TransformedERC20(TransformedERC20),
        #[allow(missing_docs)]
        TransformerDeployerUpdated(TransformerDeployerUpdated),
    }
    impl IZeroexEvents {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the
        /// variants. No guarantees are made about the order of the
        /// selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 32usize]] = &[
            [
                2u8, 223u8, 234u8, 213u8, 235u8, 118u8, 155u8, 41u8, 142u8, 130u8, 221u8, 150u8,
                80u8, 179u8, 28u8, 64u8, 85u8, 154u8, 61u8, 66u8, 112u8, 29u8, 191u8, 83u8, 201u8,
                49u8, 188u8, 38u8, 130u8, 132u8, 124u8, 49u8,
            ],
            [
                15u8, 102u8, 114u8, 247u8, 138u8, 89u8, 186u8, 142u8, 94u8, 91u8, 93u8, 56u8,
                223u8, 62u8, 188u8, 103u8, 243u8, 199u8, 146u8, 226u8, 201u8, 37u8, 155u8, 141u8,
                151u8, 215u8, 240u8, 13u8, 215u8, 139u8, 161u8, 179u8,
            ],
            [
                32u8, 204u8, 168u8, 27u8, 14u8, 38u8, 155u8, 38u8, 91u8, 50u8, 41u8, 214u8, 181u8,
                55u8, 218u8, 145u8, 239u8, 71u8, 92u8, 160u8, 239u8, 85u8, 202u8, 237u8, 125u8,
                211u8, 7u8, 49u8, 112u8, 11u8, 169u8, 141u8,
            ],
            [
                42u8, 226u8, 33u8, 8u8, 52u8, 103u8, 222u8, 82u8, 7u8, 139u8, 0u8, 150u8, 105u8,
                106u8, 184u8, 141u8, 141u8, 83u8, 167u8, 236u8, 180u8, 75u8, 182u8, 91u8, 86u8,
                162u8, 186u8, 182u8, 135u8, 89u8, 131u8, 103u8,
            ],
            [
                64u8, 166u8, 186u8, 149u8, 19u8, 208u8, 158u8, 52u8, 136u8, 19u8, 94u8, 14u8, 13u8,
                16u8, 226u8, 212u8, 56u8, 43u8, 121u8, 39u8, 32u8, 21u8, 91u8, 20u8, 76u8, 190u8,
                168u8, 154u8, 201u8, 219u8, 109u8, 52u8,
            ],
            [
                77u8, 94u8, 167u8, 218u8, 100u8, 245u8, 10u8, 74u8, 50u8, 153u8, 33u8, 184u8,
                210u8, 202u8, 181u8, 45u8, 255u8, 78u8, 188u8, 197u8, 139u8, 97u8, 209u8, 15u8,
                248u8, 57u8, 226u8, 142u8, 145u8, 68u8, 86u8, 132u8,
            ],
            [
                80u8, 39u8, 63u8, 160u8, 34u8, 115u8, 204u8, 238u8, 169u8, 207u8, 8u8, 91u8, 66u8,
                222u8, 92u8, 138u8, 246u8, 6u8, 36u8, 20u8, 1u8, 104u8, 189u8, 113u8, 53u8, 125u8,
                184u8, 51u8, 83u8, 88u8, 119u8, 175u8,
            ],
            [
                94u8, 145u8, 221u8, 254u8, 183u8, 191u8, 46u8, 18u8, 247u8, 232u8, 171u8, 1u8,
                125u8, 43u8, 99u8, 169u8, 33u8, 127u8, 0u8, 74u8, 21u8, 165u8, 51u8, 70u8, 173u8,
                144u8, 53u8, 62u8, 198u8, 61u8, 20u8, 228u8,
            ],
            [
                110u8, 169u8, 219u8, 232u8, 178u8, 204u8, 17u8, 147u8, 72u8, 113u8, 106u8, 146u8,
                32u8, 160u8, 116u8, 42u8, 214u8, 43u8, 120u8, 132u8, 236u8, 176u8, 255u8, 75u8,
                50u8, 205u8, 80u8, 129u8, 33u8, 253u8, 147u8, 121u8,
            ],
            [
                127u8, 79u8, 227u8, 255u8, 138u8, 228u8, 64u8, 225u8, 87u8, 12u8, 85u8, 141u8,
                160u8, 132u8, 64u8, 178u8, 111u8, 137u8, 251u8, 28u8, 31u8, 41u8, 16u8, 205u8,
                145u8, 202u8, 100u8, 82u8, 149u8, 95u8, 18u8, 26u8,
            ],
            [
                130u8, 159u8, 169u8, 157u8, 148u8, 220u8, 70u8, 54u8, 146u8, 91u8, 56u8, 99u8,
                46u8, 98u8, 87u8, 54u8, 166u8, 20u8, 193u8, 84u8, 213u8, 80u8, 6u8, 183u8, 171u8,
                107u8, 234u8, 151u8, 156u8, 33u8, 12u8, 50u8,
            ],
            [
                139u8, 224u8, 7u8, 156u8, 83u8, 22u8, 89u8, 20u8, 19u8, 68u8, 205u8, 31u8, 208u8,
                164u8, 242u8, 132u8, 25u8, 73u8, 127u8, 151u8, 34u8, 163u8, 218u8, 175u8, 227u8,
                180u8, 24u8, 111u8, 107u8, 100u8, 87u8, 224u8,
            ],
            [
                140u8, 93u8, 12u8, 65u8, 251u8, 22u8, 167u8, 49u8, 122u8, 108u8, 85u8, 255u8,
                123u8, 169u8, 61u8, 157u8, 116u8, 247u8, 158u8, 67u8, 79u8, 239u8, 166u8, 148u8,
                229u8, 13u8, 96u8, 40u8, 175u8, 191u8, 163u8, 240u8,
            ],
            [
                160u8, 21u8, 173u8, 45u8, 195u8, 47u8, 38u8, 105u8, 147u8, 149u8, 138u8, 15u8,
                217u8, 136u8, 76u8, 116u8, 107u8, 151u8, 27u8, 37u8, 66u8, 6u8, 243u8, 71u8, 139u8,
                196u8, 62u8, 47u8, 18u8, 92u8, 123u8, 158u8,
            ],
            [
                166u8, 235u8, 124u8, 220u8, 33u8, 158u8, 21u8, 24u8, 206u8, 217u8, 100u8, 233u8,
                163u8, 78u8, 97u8, 214u8, 138u8, 148u8, 228u8, 241u8, 86u8, 157u8, 179u8, 232u8,
                66u8, 86u8, 186u8, 152u8, 27u8, 165u8, 39u8, 83u8,
            ],
            [
                169u8, 31u8, 231u8, 174u8, 98u8, 252u8, 230u8, 105u8, 223u8, 44u8, 127u8, 136u8,
                15u8, 140u8, 20u8, 209u8, 120u8, 83u8, 26u8, 174u8, 114u8, 81u8, 85u8, 88u8, 229u8,
                201u8, 72u8, 227u8, 124u8, 50u8, 165u8, 114u8,
            ],
            [
                171u8, 97u8, 77u8, 43u8, 115u8, 133u8, 67u8, 192u8, 234u8, 33u8, 245u8, 99u8, 71u8,
                207u8, 105u8, 106u8, 58u8, 12u8, 66u8, 167u8, 203u8, 236u8, 50u8, 18u8, 165u8,
                202u8, 34u8, 164u8, 220u8, 255u8, 33u8, 36u8,
            ],
            [
                172u8, 117u8, 247u8, 115u8, 227u8, 169u8, 47u8, 26u8, 2u8, 177u8, 33u8, 52u8,
                214u8, 94u8, 31u8, 71u8, 248u8, 161u8, 78u8, 171u8, 228u8, 234u8, 241u8, 226u8,
                70u8, 36u8, 145u8, 142u8, 106u8, 139u8, 38u8, 159u8,
            ],
            [
                225u8, 184u8, 49u8, 176u8, 230u8, 243u8, 170u8, 22u8, 180u8, 177u8, 166u8, 189u8,
                82u8, 107u8, 92u8, 222u8, 171u8, 73u8, 64u8, 116u8, 76u8, 166u8, 224u8, 37u8, 31u8,
                95u8, 229u8, 248u8, 202u8, 241u8, 200u8, 26u8,
            ],
            [
                245u8, 85u8, 12u8, 94u8, 234u8, 25u8, 180u8, 138u8, 198u8, 235u8, 95u8, 3u8, 171u8,
                220u8, 79u8, 89u8, 192u8, 166u8, 6u8, 151u8, 171u8, 179u8, 217u8, 115u8, 205u8,
                104u8, 102u8, 151u8, 3u8, 181u8, 200u8, 185u8,
            ],
            [
                253u8, 69u8, 96u8, 74u8, 186u8, 215u8, 156u8, 22u8, 226u8, 51u8, 72u8, 161u8, 55u8,
                237u8, 130u8, 146u8, 102u8, 27u8, 225u8, 184u8, 235u8, 166u8, 228u8, 128u8, 110u8,
                190u8, 214u8, 131u8, 59u8, 28u8, 4u8, 106u8,
            ],
            [
                254u8, 127u8, 251u8, 30u8, 223u8, 231u8, 159u8, 77u8, 247u8, 22u8, 203u8, 45u8,
                202u8, 210u8, 28u8, 242u8, 243u8, 27u8, 16u8, 77u8, 129u8, 106u8, 121u8, 118u8,
                186u8, 30u8, 110u8, 70u8, 83u8, 193u8, 239u8, 177u8,
            ],
        ];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <RfqOrderOriginsAllowed as alloy_sol_types::SolEvent>::SIGNATURE,
            <TransformedERC20 as alloy_sol_types::SolEvent>::SIGNATURE,
            <ERC1155OrderFilled as alloy_sol_types::SolEvent>::SIGNATURE,
            <ProxyFunctionUpdated as alloy_sol_types::SolEvent>::SIGNATURE,
            <LiquidityProviderSwap as alloy_sol_types::SolEvent>::SIGNATURE,
            <ERC1155OrderCancelled as alloy_sol_types::SolEvent>::SIGNATURE,
            <ERC721OrderFilled as alloy_sol_types::SolEvent>::SIGNATURE,
            <ERC1155OrderPreSigned as alloy_sol_types::SolEvent>::SIGNATURE,
            <OrderSignerRegistered as alloy_sol_types::SolEvent>::SIGNATURE,
            <MetaTransactionExecuted as alloy_sol_types::SolEvent>::SIGNATURE,
            <RfqOrderFilled as alloy_sol_types::SolEvent>::SIGNATURE,
            <OwnershipTransferred as alloy_sol_types::SolEvent>::SIGNATURE,
            <ERC721OrderPreSigned as alloy_sol_types::SolEvent>::SIGNATURE,
            <ERC721OrderCancelled as alloy_sol_types::SolEvent>::SIGNATURE,
            <OrderCancelled as alloy_sol_types::SolEvent>::SIGNATURE,
            <PairCancelledLimitOrders as alloy_sol_types::SolEvent>::SIGNATURE,
            <LimitOrderFilled as alloy_sol_types::SolEvent>::SIGNATURE,
            <OtcOrderFilled as alloy_sol_types::SolEvent>::SIGNATURE,
            <Migrated as alloy_sol_types::SolEvent>::SIGNATURE,
            <QuoteSignerUpdated as alloy_sol_types::SolEvent>::SIGNATURE,
            <TransformerDeployerUpdated as alloy_sol_types::SolEvent>::SIGNATURE,
            <PairCancelledRfqOrders as alloy_sol_types::SolEvent>::SIGNATURE,
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(RfqOrderOriginsAllowed),
            ::core::stringify!(TransformedERC20),
            ::core::stringify!(ERC1155OrderFilled),
            ::core::stringify!(ProxyFunctionUpdated),
            ::core::stringify!(LiquidityProviderSwap),
            ::core::stringify!(ERC1155OrderCancelled),
            ::core::stringify!(ERC721OrderFilled),
            ::core::stringify!(ERC1155OrderPreSigned),
            ::core::stringify!(OrderSignerRegistered),
            ::core::stringify!(MetaTransactionExecuted),
            ::core::stringify!(RfqOrderFilled),
            ::core::stringify!(OwnershipTransferred),
            ::core::stringify!(ERC721OrderPreSigned),
            ::core::stringify!(ERC721OrderCancelled),
            ::core::stringify!(OrderCancelled),
            ::core::stringify!(PairCancelledLimitOrders),
            ::core::stringify!(LimitOrderFilled),
            ::core::stringify!(OtcOrderFilled),
            ::core::stringify!(Migrated),
            ::core::stringify!(QuoteSignerUpdated),
            ::core::stringify!(TransformerDeployerUpdated),
            ::core::stringify!(PairCancelledRfqOrders),
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
    impl alloy_sol_types::SolEventInterface for IZeroexEvents {
        const COUNT: usize = 22usize;
        const NAME: &'static str = "IZeroexEvents";

        fn decode_raw_log(
            topics: &[alloy_sol_types::Word],
            data: &[u8],
        ) -> alloy_sol_types::Result<Self> {
            match topics.first().copied() {
                Some(<ERC1155OrderCancelled as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <ERC1155OrderCancelled as alloy_sol_types::SolEvent>::decode_raw_log(
                        topics, data,
                    )
                    .map(Self::ERC1155OrderCancelled)
                }
                Some(<ERC1155OrderFilled as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <ERC1155OrderFilled as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::ERC1155OrderFilled)
                }
                Some(<ERC1155OrderPreSigned as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <ERC1155OrderPreSigned as alloy_sol_types::SolEvent>::decode_raw_log(
                        topics, data,
                    )
                    .map(Self::ERC1155OrderPreSigned)
                }
                Some(<ERC721OrderCancelled as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <ERC721OrderCancelled as alloy_sol_types::SolEvent>::decode_raw_log(
                        topics, data,
                    )
                    .map(Self::ERC721OrderCancelled)
                }
                Some(<ERC721OrderFilled as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <ERC721OrderFilled as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::ERC721OrderFilled)
                }
                Some(<ERC721OrderPreSigned as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <ERC721OrderPreSigned as alloy_sol_types::SolEvent>::decode_raw_log(
                        topics, data,
                    )
                    .map(Self::ERC721OrderPreSigned)
                }
                Some(<LimitOrderFilled as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <LimitOrderFilled as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::LimitOrderFilled)
                }
                Some(<LiquidityProviderSwap as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <LiquidityProviderSwap as alloy_sol_types::SolEvent>::decode_raw_log(
                        topics, data,
                    )
                    .map(Self::LiquidityProviderSwap)
                }
                Some(<MetaTransactionExecuted as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <MetaTransactionExecuted as alloy_sol_types::SolEvent>::decode_raw_log(
                        topics, data,
                    )
                    .map(Self::MetaTransactionExecuted)
                }
                Some(<Migrated as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <Migrated as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::Migrated)
                }
                Some(<OrderCancelled as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <OrderCancelled as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::OrderCancelled)
                }
                Some(<OrderSignerRegistered as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <OrderSignerRegistered as alloy_sol_types::SolEvent>::decode_raw_log(
                        topics, data,
                    )
                    .map(Self::OrderSignerRegistered)
                }
                Some(<OtcOrderFilled as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <OtcOrderFilled as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::OtcOrderFilled)
                }
                Some(<OwnershipTransferred as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <OwnershipTransferred as alloy_sol_types::SolEvent>::decode_raw_log(
                        topics, data,
                    )
                    .map(Self::OwnershipTransferred)
                }
                Some(<PairCancelledLimitOrders as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <PairCancelledLimitOrders as alloy_sol_types::SolEvent>::decode_raw_log(
                        topics, data,
                    )
                    .map(Self::PairCancelledLimitOrders)
                }
                Some(<PairCancelledRfqOrders as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <PairCancelledRfqOrders as alloy_sol_types::SolEvent>::decode_raw_log(
                        topics, data,
                    )
                    .map(Self::PairCancelledRfqOrders)
                }
                Some(<ProxyFunctionUpdated as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <ProxyFunctionUpdated as alloy_sol_types::SolEvent>::decode_raw_log(
                        topics, data,
                    )
                    .map(Self::ProxyFunctionUpdated)
                }
                Some(<QuoteSignerUpdated as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <QuoteSignerUpdated as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::QuoteSignerUpdated)
                }
                Some(<RfqOrderFilled as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <RfqOrderFilled as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::RfqOrderFilled)
                }
                Some(<RfqOrderOriginsAllowed as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <RfqOrderOriginsAllowed as alloy_sol_types::SolEvent>::decode_raw_log(
                        topics, data,
                    )
                    .map(Self::RfqOrderOriginsAllowed)
                }
                Some(<TransformedERC20 as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <TransformedERC20 as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::TransformedERC20)
                }
                Some(<TransformerDeployerUpdated as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <TransformerDeployerUpdated as alloy_sol_types::SolEvent>::decode_raw_log(
                        topics, data,
                    )
                    .map(Self::TransformerDeployerUpdated)
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
    impl alloy_sol_types::private::IntoLogData for IZeroexEvents {
        fn to_log_data(&self) -> alloy_sol_types::private::LogData {
            match self {
                Self::ERC1155OrderCancelled(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::ERC1155OrderFilled(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::ERC1155OrderPreSigned(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::ERC721OrderCancelled(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::ERC721OrderFilled(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::ERC721OrderPreSigned(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::LimitOrderFilled(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::LiquidityProviderSwap(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::MetaTransactionExecuted(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::Migrated(inner) => alloy_sol_types::private::IntoLogData::to_log_data(inner),
                Self::OrderCancelled(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::OrderSignerRegistered(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::OtcOrderFilled(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::OwnershipTransferred(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::PairCancelledLimitOrders(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::PairCancelledRfqOrders(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::ProxyFunctionUpdated(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::QuoteSignerUpdated(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::RfqOrderFilled(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::RfqOrderOriginsAllowed(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::TransformedERC20(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::TransformerDeployerUpdated(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
            }
        }

        fn into_log_data(self) -> alloy_sol_types::private::LogData {
            match self {
                Self::ERC1155OrderCancelled(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::ERC1155OrderFilled(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::ERC1155OrderPreSigned(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::ERC721OrderCancelled(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::ERC721OrderFilled(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::ERC721OrderPreSigned(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::LimitOrderFilled(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::LiquidityProviderSwap(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::MetaTransactionExecuted(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::Migrated(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::OrderCancelled(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::OrderSignerRegistered(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::OtcOrderFilled(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::OwnershipTransferred(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::PairCancelledLimitOrders(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::PairCancelledRfqOrders(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::ProxyFunctionUpdated(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::QuoteSignerUpdated(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::RfqOrderFilled(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::RfqOrderOriginsAllowed(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::TransformedERC20(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::TransformerDeployerUpdated(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
            }
        }
    }
    use alloy_contract;
    /**Creates a new wrapper around an on-chain [`IZeroex`](self) contract instance.

    See the [wrapper's documentation](`IZeroexInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> IZeroexInstance<P, N> {
        IZeroexInstance::<P, N>::new(address, __provider)
    }
    /**A [`IZeroex`](self) instance.

    Contains type-safe methods for interacting with an on-chain instance of the
    [`IZeroex`](self) contract located at a given `address`, using a given
    provider `P`.

    If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
    documentation on how to provide it), the `deploy` and `deploy_builder` methods can
    be used to deploy a new instance of the contract.

    See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct IZeroexInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for IZeroexInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("IZeroexInstance")
                .field(&self.address)
                .finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        IZeroexInstance<P, N>
    {
        /**Creates a new wrapper around an on-chain [`IZeroex`](self) contract instance.

        See the [wrapper's documentation](`IZeroexInstance`) for more details.*/
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
    impl<P: ::core::clone::Clone, N> IZeroexInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned
        /// provider.
        #[inline]
        pub fn with_cloned_provider(self) -> IZeroexInstance<P, N> {
            IZeroexInstance {
                address: self.address,
                provider: ::core::clone::Clone::clone(&self.provider),
                _network: ::core::marker::PhantomData,
            }
        }
    }
    /// Function calls.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        IZeroexInstance<P, N>
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

        ///Creates a new call builder for the [`extend`] function.
        pub fn extend(
            &self,
            selector: alloy_sol_types::private::FixedBytes<4>,
            r#impl: alloy_sol_types::private::Address,
        ) -> alloy_contract::SolCallBuilder<&P, extendCall, N> {
            self.call_builder(&extendCall { selector, r#impl })
        }

        ///Creates a new call builder for the [`fillOrKillLimitOrder`]
        /// function.
        pub fn fillOrKillLimitOrder(
            &self,
            order: <LibNativeOrder::LimitOrder as alloy_sol_types::SolType>::RustType,
            signature: <LibSignature::Signature as alloy_sol_types::SolType>::RustType,
            takerTokenFillAmount: u128,
        ) -> alloy_contract::SolCallBuilder<&P, fillOrKillLimitOrderCall, N> {
            self.call_builder(&fillOrKillLimitOrderCall {
                order,
                signature,
                takerTokenFillAmount,
            })
        }

        ///Creates a new call builder for the [`getLimitOrderRelevantState`]
        /// function.
        pub fn getLimitOrderRelevantState(
            &self,
            order: <LibNativeOrder::LimitOrder as alloy_sol_types::SolType>::RustType,
            signature: <LibSignature::Signature as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, getLimitOrderRelevantStateCall, N> {
            self.call_builder(&getLimitOrderRelevantStateCall { order, signature })
        }

        ///Creates a new call builder for the [`owner`] function.
        pub fn owner(&self) -> alloy_contract::SolCallBuilder<&P, ownerCall, N> {
            self.call_builder(&ownerCall)
        }
    }
    /// Event filters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        IZeroexInstance<P, N>
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

        ///Creates a new event filter for the [`ERC1155OrderCancelled`] event.
        pub fn ERC1155OrderCancelled_filter(
            &self,
        ) -> alloy_contract::Event<&P, ERC1155OrderCancelled, N> {
            self.event_filter::<ERC1155OrderCancelled>()
        }

        ///Creates a new event filter for the [`ERC1155OrderFilled`] event.
        pub fn ERC1155OrderFilled_filter(
            &self,
        ) -> alloy_contract::Event<&P, ERC1155OrderFilled, N> {
            self.event_filter::<ERC1155OrderFilled>()
        }

        ///Creates a new event filter for the [`ERC1155OrderPreSigned`] event.
        pub fn ERC1155OrderPreSigned_filter(
            &self,
        ) -> alloy_contract::Event<&P, ERC1155OrderPreSigned, N> {
            self.event_filter::<ERC1155OrderPreSigned>()
        }

        ///Creates a new event filter for the [`ERC721OrderCancelled`] event.
        pub fn ERC721OrderCancelled_filter(
            &self,
        ) -> alloy_contract::Event<&P, ERC721OrderCancelled, N> {
            self.event_filter::<ERC721OrderCancelled>()
        }

        ///Creates a new event filter for the [`ERC721OrderFilled`] event.
        pub fn ERC721OrderFilled_filter(&self) -> alloy_contract::Event<&P, ERC721OrderFilled, N> {
            self.event_filter::<ERC721OrderFilled>()
        }

        ///Creates a new event filter for the [`ERC721OrderPreSigned`] event.
        pub fn ERC721OrderPreSigned_filter(
            &self,
        ) -> alloy_contract::Event<&P, ERC721OrderPreSigned, N> {
            self.event_filter::<ERC721OrderPreSigned>()
        }

        ///Creates a new event filter for the [`LimitOrderFilled`] event.
        pub fn LimitOrderFilled_filter(&self) -> alloy_contract::Event<&P, LimitOrderFilled, N> {
            self.event_filter::<LimitOrderFilled>()
        }

        ///Creates a new event filter for the [`LiquidityProviderSwap`] event.
        pub fn LiquidityProviderSwap_filter(
            &self,
        ) -> alloy_contract::Event<&P, LiquidityProviderSwap, N> {
            self.event_filter::<LiquidityProviderSwap>()
        }

        ///Creates a new event filter for the [`MetaTransactionExecuted`]
        /// event.
        pub fn MetaTransactionExecuted_filter(
            &self,
        ) -> alloy_contract::Event<&P, MetaTransactionExecuted, N> {
            self.event_filter::<MetaTransactionExecuted>()
        }

        ///Creates a new event filter for the [`Migrated`] event.
        pub fn Migrated_filter(&self) -> alloy_contract::Event<&P, Migrated, N> {
            self.event_filter::<Migrated>()
        }

        ///Creates a new event filter for the [`OrderCancelled`] event.
        pub fn OrderCancelled_filter(&self) -> alloy_contract::Event<&P, OrderCancelled, N> {
            self.event_filter::<OrderCancelled>()
        }

        ///Creates a new event filter for the [`OrderSignerRegistered`] event.
        pub fn OrderSignerRegistered_filter(
            &self,
        ) -> alloy_contract::Event<&P, OrderSignerRegistered, N> {
            self.event_filter::<OrderSignerRegistered>()
        }

        ///Creates a new event filter for the [`OtcOrderFilled`] event.
        pub fn OtcOrderFilled_filter(&self) -> alloy_contract::Event<&P, OtcOrderFilled, N> {
            self.event_filter::<OtcOrderFilled>()
        }

        ///Creates a new event filter for the [`OwnershipTransferred`] event.
        pub fn OwnershipTransferred_filter(
            &self,
        ) -> alloy_contract::Event<&P, OwnershipTransferred, N> {
            self.event_filter::<OwnershipTransferred>()
        }

        ///Creates a new event filter for the [`PairCancelledLimitOrders`]
        /// event.
        pub fn PairCancelledLimitOrders_filter(
            &self,
        ) -> alloy_contract::Event<&P, PairCancelledLimitOrders, N> {
            self.event_filter::<PairCancelledLimitOrders>()
        }

        ///Creates a new event filter for the [`PairCancelledRfqOrders`] event.
        pub fn PairCancelledRfqOrders_filter(
            &self,
        ) -> alloy_contract::Event<&P, PairCancelledRfqOrders, N> {
            self.event_filter::<PairCancelledRfqOrders>()
        }

        ///Creates a new event filter for the [`ProxyFunctionUpdated`] event.
        pub fn ProxyFunctionUpdated_filter(
            &self,
        ) -> alloy_contract::Event<&P, ProxyFunctionUpdated, N> {
            self.event_filter::<ProxyFunctionUpdated>()
        }

        ///Creates a new event filter for the [`QuoteSignerUpdated`] event.
        pub fn QuoteSignerUpdated_filter(
            &self,
        ) -> alloy_contract::Event<&P, QuoteSignerUpdated, N> {
            self.event_filter::<QuoteSignerUpdated>()
        }

        ///Creates a new event filter for the [`RfqOrderFilled`] event.
        pub fn RfqOrderFilled_filter(&self) -> alloy_contract::Event<&P, RfqOrderFilled, N> {
            self.event_filter::<RfqOrderFilled>()
        }

        ///Creates a new event filter for the [`RfqOrderOriginsAllowed`] event.
        pub fn RfqOrderOriginsAllowed_filter(
            &self,
        ) -> alloy_contract::Event<&P, RfqOrderOriginsAllowed, N> {
            self.event_filter::<RfqOrderOriginsAllowed>()
        }

        ///Creates a new event filter for the [`TransformedERC20`] event.
        pub fn TransformedERC20_filter(&self) -> alloy_contract::Event<&P, TransformedERC20, N> {
            self.event_filter::<TransformedERC20>()
        }

        ///Creates a new event filter for the [`TransformerDeployerUpdated`]
        /// event.
        pub fn TransformerDeployerUpdated_filter(
            &self,
        ) -> alloy_contract::Event<&P, TransformerDeployerUpdated, N> {
            self.event_filter::<TransformerDeployerUpdated>()
        }
    }
}
pub type Instance = IZeroex::IZeroexInstance<::alloy_provider::DynProvider>;
use {
    alloy_primitives::{Address, address},
    alloy_provider::{DynProvider, Provider},
    anyhow::{Context, Result},
    std::{collections::HashMap, sync::LazyLock},
};
pub const fn deployment_info(chain_id: u64) -> Option<(Address, Option<u64>)> {
    match chain_id {
        1u64 => Some((
            ::alloy_primitives::address!("0xdef1c0ded9bec7f1a1670819833240f027b25eff"),
            None,
        )),
        10u64 => Some((
            ::alloy_primitives::address!("0xdef1abe32c034e558cdd535791643c58a13acc10"),
            None,
        )),
        56u64 => Some((
            ::alloy_primitives::address!("0xdef1c0ded9bec7f1a1670819833240f027b25eff"),
            None,
        )),
        137u64 => Some((
            ::alloy_primitives::address!("0xdef1c0ded9bec7f1a1670819833240f027b25eff"),
            None,
        )),
        8453u64 => Some((
            ::alloy_primitives::address!("0xdef1c0ded9bec7f1a1670819833240f027b25eff"),
            None,
        )),
        42161u64 => Some((
            ::alloy_primitives::address!("0xdef1c0ded9bec7f1a1670819833240f027b25eff"),
            None,
        )),
        43114u64 => Some((
            ::alloy_primitives::address!("0xdef1c0ded9bec7f1a1670819833240f027b25eff"),
            None,
        )),
        11155111u64 => Some((
            ::alloy_primitives::address!("0xdef1c0ded9bec7f1a1670819833240f027b25eff"),
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
