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
library IAllowanceTransfer {
    struct PermitBatch { PermitDetails[] details; address spender; uint256 sigDeadline; }
    struct PermitDetails { address token; uint160 amount; uint48 expiration; uint48 nonce; }
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
    use {super::*, alloy_sol_types};
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**```solidity
    struct PermitBatch { PermitDetails[] details; address spender; uint256 sigDeadline; }
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct PermitBatch {
        #[allow(missing_docs)]
        pub details:
            alloy_sol_types::private::Vec<<PermitDetails as alloy_sol_types::SolType>::RustType>,
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
        use alloy_sol_types;
        #[doc(hidden)]
        #[allow(dead_code)]
        type UnderlyingSolTuple<'a> = (
            alloy_sol_types::sol_data::Array<PermitDetails>,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Uint<256>,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::Vec<<PermitDetails as alloy_sol_types::SolType>::RustType>,
            alloy_sol_types::private::Address,
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
        impl ::core::convert::From<PermitBatch> for UnderlyingRustTuple<'_> {
            fn from(value: PermitBatch) -> Self {
                (value.details, value.spender, value.sigDeadline)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for PermitBatch {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    details: tuple.0,
                    spender: tuple.1,
                    sigDeadline: tuple.2,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for PermitBatch {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for PermitBatch {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Array<
                        PermitDetails,
                    > as alloy_sol_types::SolType>::tokenize(&self.details),
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
        impl alloy_sol_types::SolType for PermitBatch {
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
        impl alloy_sol_types::SolStruct for PermitBatch {
            const NAME: &'static str = "PermitBatch";

            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "PermitBatch(PermitDetails[] details,address spender,uint256 sigDeadline)",
                )
            }

            #[inline]
            fn eip712_components()
            -> alloy_sol_types::private::Vec<alloy_sol_types::private::Cow<'static, str>>
            {
                let mut components = alloy_sol_types::private::Vec::with_capacity(1);
                components.push(<PermitDetails as alloy_sol_types::SolStruct>::eip712_root_type());
                components
                    .extend(<PermitDetails as alloy_sol_types::SolStruct>::eip712_components());
                components
            }

            #[inline]
            fn eip712_encode_data(&self) -> alloy_sol_types::private::Vec<u8> {
                [
                    <alloy_sol_types::sol_data::Array<
                        PermitDetails,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.details)
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
        impl alloy_sol_types::EventTopic for PermitBatch {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Array<
                        PermitDetails,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
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
                out.reserve(<Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust));
                <alloy_sol_types::sol_data::Array<
                    PermitDetails,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
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
            fn encode_topic(rust: &Self::RustType) -> alloy_sol_types::abi::token::WordToken {
                let mut out = alloy_sol_types::private::Vec::new();
                <Self as alloy_sol_types::EventTopic>::encode_topic_preimage(rust, &mut out);
                alloy_sol_types::abi::token::WordToken(alloy_sol_types::private::keccak256(out))
            }
        }
    };
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
        use alloy_sol_types;
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
        fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
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
                    <alloy_sol_types::sol_data::Uint<160> as alloy_sol_types::SolType>::tokenize(
                        &self.amount,
                    ),
                    <alloy_sol_types::sol_data::Uint<48> as alloy_sol_types::SolType>::tokenize(
                        &self.expiration,
                    ),
                    <alloy_sol_types::sol_data::Uint<48> as alloy_sol_types::SolType>::tokenize(
                        &self.nonce,
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
        impl alloy_sol_types::SolType for PermitDetails {
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
        impl alloy_sol_types::SolStruct for PermitDetails {
            const NAME: &'static str = "PermitDetails";

            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "PermitDetails(address token,uint160 amount,uint48 expiration,uint48 nonce)",
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
                out.reserve(<Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust));
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
            fn encode_topic(rust: &Self::RustType) -> alloy_sol_types::abi::token::WordToken {
                let mut out = alloy_sol_types::private::Vec::new();
                <Self as alloy_sol_types::EventTopic>::encode_topic_preimage(rust, &mut out);
                alloy_sol_types::abi::token::WordToken(alloy_sol_types::private::keccak256(out))
            }
        }
    };
    use alloy_contract;
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
            f.debug_tuple("IAllowanceTransferInstance")
                .field(&self.address)
                .finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        IAllowanceTransferInstance<P, N>
    {
        /**Creates a new wrapper around an on-chain [`IAllowanceTransfer`](self) contract instance.

        See the [wrapper's documentation](`IAllowanceTransferInstance`) for more details.*/
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
    impl<P: ::core::clone::Clone, N> IAllowanceTransferInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned
        /// provider.
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
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        IAllowanceTransferInstance<P, N>
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
        IAllowanceTransferInstance<P, N>
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
library IBatchRouter {
    struct SwapExactInHookParams { address sender; SwapPathExactAmountIn[] paths; uint256 deadline; bool wethIsEth; bytes userData; }
    struct SwapExactOutHookParams { address sender; SwapPathExactAmountOut[] paths; uint256 deadline; bool wethIsEth; bytes userData; }
    struct SwapPathExactAmountIn { address tokenIn; SwapPathStep[] steps; uint256 exactAmountIn; uint256 minAmountOut; }
    struct SwapPathExactAmountOut { address tokenIn; SwapPathStep[] steps; uint256 maxAmountIn; uint256 exactAmountOut; }
    struct SwapPathStep { address pool; address tokenOut; bool isBuffer; }
}
```*/
#[allow(
    non_camel_case_types,
    non_snake_case,
    clippy::pub_underscore_fields,
    clippy::style,
    clippy::empty_structs_with_brackets
)]
pub mod IBatchRouter {
    use {super::*, alloy_sol_types};
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**```solidity
    struct SwapExactInHookParams { address sender; SwapPathExactAmountIn[] paths; uint256 deadline; bool wethIsEth; bytes userData; }
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct SwapExactInHookParams {
        #[allow(missing_docs)]
        pub sender: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub paths: alloy_sol_types::private::Vec<
            <SwapPathExactAmountIn as alloy_sol_types::SolType>::RustType,
        >,
        #[allow(missing_docs)]
        pub deadline: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub wethIsEth: bool,
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
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Array<SwapPathExactAmountIn>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Bool,
            alloy_sol_types::sol_data::Bytes,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::Address,
            alloy_sol_types::private::Vec<
                <SwapPathExactAmountIn as alloy_sol_types::SolType>::RustType,
            >,
            alloy_sol_types::private::primitives::aliases::U256,
            bool,
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
        impl ::core::convert::From<SwapExactInHookParams> for UnderlyingRustTuple<'_> {
            fn from(value: SwapExactInHookParams) -> Self {
                (
                    value.sender,
                    value.paths,
                    value.deadline,
                    value.wethIsEth,
                    value.userData,
                )
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for SwapExactInHookParams {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    sender: tuple.0,
                    paths: tuple.1,
                    deadline: tuple.2,
                    wethIsEth: tuple.3,
                    userData: tuple.4,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for SwapExactInHookParams {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for SwapExactInHookParams {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.sender,
                    ),
                    <alloy_sol_types::sol_data::Array<
                        SwapPathExactAmountIn,
                    > as alloy_sol_types::SolType>::tokenize(&self.paths),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.deadline),
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::tokenize(
                        &self.wethIsEth,
                    ),
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
        impl alloy_sol_types::SolType for SwapExactInHookParams {
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
        impl alloy_sol_types::SolStruct for SwapExactInHookParams {
            const NAME: &'static str = "SwapExactInHookParams";

            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "SwapExactInHookParams(address sender,SwapPathExactAmountIn[] paths,uint256 \
                     deadline,bool wethIsEth,bytes userData)",
                )
            }

            #[inline]
            fn eip712_components()
            -> alloy_sol_types::private::Vec<alloy_sol_types::private::Cow<'static, str>>
            {
                let mut components = alloy_sol_types::private::Vec::with_capacity(1);
                components.push(
                    <SwapPathExactAmountIn as alloy_sol_types::SolStruct>::eip712_root_type(),
                );
                components.extend(
                    <SwapPathExactAmountIn as alloy_sol_types::SolStruct>::eip712_components(),
                );
                components
            }

            #[inline]
            fn eip712_encode_data(&self) -> alloy_sol_types::private::Vec<u8> {
                [
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.sender,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Array<
                        SwapPathExactAmountIn,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.paths)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.deadline)
                        .0,
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::eip712_data_word(
                            &self.wethIsEth,
                        )
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
        impl alloy_sol_types::EventTopic for SwapExactInHookParams {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.sender,
                    )
                    + <alloy_sol_types::sol_data::Array<
                        SwapPathExactAmountIn,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(&rust.paths)
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.deadline,
                    )
                    + <alloy_sol_types::sol_data::Bool as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.wethIsEth,
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
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.sender,
                    out,
                );
                <alloy_sol_types::sol_data::Array<
                    SwapPathExactAmountIn,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.paths,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.deadline,
                    out,
                );
                <alloy_sol_types::sol_data::Bool as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.wethIsEth,
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
    struct SwapExactOutHookParams { address sender; SwapPathExactAmountOut[] paths; uint256 deadline; bool wethIsEth; bytes userData; }
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct SwapExactOutHookParams {
        #[allow(missing_docs)]
        pub sender: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub paths: alloy_sol_types::private::Vec<
            <SwapPathExactAmountOut as alloy_sol_types::SolType>::RustType,
        >,
        #[allow(missing_docs)]
        pub deadline: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub wethIsEth: bool,
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
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Array<SwapPathExactAmountOut>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Bool,
            alloy_sol_types::sol_data::Bytes,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::Address,
            alloy_sol_types::private::Vec<
                <SwapPathExactAmountOut as alloy_sol_types::SolType>::RustType,
            >,
            alloy_sol_types::private::primitives::aliases::U256,
            bool,
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
        impl ::core::convert::From<SwapExactOutHookParams> for UnderlyingRustTuple<'_> {
            fn from(value: SwapExactOutHookParams) -> Self {
                (
                    value.sender,
                    value.paths,
                    value.deadline,
                    value.wethIsEth,
                    value.userData,
                )
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for SwapExactOutHookParams {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    sender: tuple.0,
                    paths: tuple.1,
                    deadline: tuple.2,
                    wethIsEth: tuple.3,
                    userData: tuple.4,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for SwapExactOutHookParams {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for SwapExactOutHookParams {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.sender,
                    ),
                    <alloy_sol_types::sol_data::Array<
                        SwapPathExactAmountOut,
                    > as alloy_sol_types::SolType>::tokenize(&self.paths),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.deadline),
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::tokenize(
                        &self.wethIsEth,
                    ),
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
        impl alloy_sol_types::SolType for SwapExactOutHookParams {
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
        impl alloy_sol_types::SolStruct for SwapExactOutHookParams {
            const NAME: &'static str = "SwapExactOutHookParams";

            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "SwapExactOutHookParams(address sender,SwapPathExactAmountOut[] paths,uint256 \
                     deadline,bool wethIsEth,bytes userData)",
                )
            }

            #[inline]
            fn eip712_components()
            -> alloy_sol_types::private::Vec<alloy_sol_types::private::Cow<'static, str>>
            {
                let mut components = alloy_sol_types::private::Vec::with_capacity(1);
                components.push(
                    <SwapPathExactAmountOut as alloy_sol_types::SolStruct>::eip712_root_type(),
                );
                components.extend(
                    <SwapPathExactAmountOut as alloy_sol_types::SolStruct>::eip712_components(),
                );
                components
            }

            #[inline]
            fn eip712_encode_data(&self) -> alloy_sol_types::private::Vec<u8> {
                [
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.sender,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Array<
                        SwapPathExactAmountOut,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.paths)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.deadline)
                        .0,
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::eip712_data_word(
                            &self.wethIsEth,
                        )
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
        impl alloy_sol_types::EventTopic for SwapExactOutHookParams {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.sender,
                    )
                    + <alloy_sol_types::sol_data::Array<
                        SwapPathExactAmountOut,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(&rust.paths)
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.deadline,
                    )
                    + <alloy_sol_types::sol_data::Bool as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.wethIsEth,
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
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.sender,
                    out,
                );
                <alloy_sol_types::sol_data::Array<
                    SwapPathExactAmountOut,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.paths,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.deadline,
                    out,
                );
                <alloy_sol_types::sol_data::Bool as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.wethIsEth,
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
    struct SwapPathExactAmountIn { address tokenIn; SwapPathStep[] steps; uint256 exactAmountIn; uint256 minAmountOut; }
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct SwapPathExactAmountIn {
        #[allow(missing_docs)]
        pub tokenIn: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub steps:
            alloy_sol_types::private::Vec<<SwapPathStep as alloy_sol_types::SolType>::RustType>,
        #[allow(missing_docs)]
        pub exactAmountIn: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub minAmountOut: alloy_sol_types::private::primitives::aliases::U256,
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
            alloy_sol_types::sol_data::Array<SwapPathStep>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<256>,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::Address,
            alloy_sol_types::private::Vec<<SwapPathStep as alloy_sol_types::SolType>::RustType>,
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
        impl ::core::convert::From<SwapPathExactAmountIn> for UnderlyingRustTuple<'_> {
            fn from(value: SwapPathExactAmountIn) -> Self {
                (
                    value.tokenIn,
                    value.steps,
                    value.exactAmountIn,
                    value.minAmountOut,
                )
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for SwapPathExactAmountIn {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    tokenIn: tuple.0,
                    steps: tuple.1,
                    exactAmountIn: tuple.2,
                    minAmountOut: tuple.3,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for SwapPathExactAmountIn {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for SwapPathExactAmountIn {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.tokenIn,
                    ),
                    <alloy_sol_types::sol_data::Array<
                        SwapPathStep,
                    > as alloy_sol_types::SolType>::tokenize(&self.steps),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.exactAmountIn),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.minAmountOut),
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
        impl alloy_sol_types::SolType for SwapPathExactAmountIn {
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
        impl alloy_sol_types::SolStruct for SwapPathExactAmountIn {
            const NAME: &'static str = "SwapPathExactAmountIn";

            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "SwapPathExactAmountIn(address tokenIn,SwapPathStep[] steps,uint256 \
                     exactAmountIn,uint256 minAmountOut)",
                )
            }

            #[inline]
            fn eip712_components()
            -> alloy_sol_types::private::Vec<alloy_sol_types::private::Cow<'static, str>>
            {
                let mut components = alloy_sol_types::private::Vec::with_capacity(1);
                components.push(<SwapPathStep as alloy_sol_types::SolStruct>::eip712_root_type());
                components
                    .extend(<SwapPathStep as alloy_sol_types::SolStruct>::eip712_components());
                components
            }

            #[inline]
            fn eip712_encode_data(&self) -> alloy_sol_types::private::Vec<u8> {
                [
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.tokenIn,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Array<
                        SwapPathStep,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.steps)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.exactAmountIn)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.minAmountOut)
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for SwapPathExactAmountIn {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.tokenIn,
                    )
                    + <alloy_sol_types::sol_data::Array<
                        SwapPathStep,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(&rust.steps)
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.exactAmountIn,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.minAmountOut,
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
                <alloy_sol_types::sol_data::Array<
                    SwapPathStep,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.steps,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.exactAmountIn,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.minAmountOut,
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
    struct SwapPathExactAmountOut { address tokenIn; SwapPathStep[] steps; uint256 maxAmountIn; uint256 exactAmountOut; }
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct SwapPathExactAmountOut {
        #[allow(missing_docs)]
        pub tokenIn: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub steps:
            alloy_sol_types::private::Vec<<SwapPathStep as alloy_sol_types::SolType>::RustType>,
        #[allow(missing_docs)]
        pub maxAmountIn: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub exactAmountOut: alloy_sol_types::private::primitives::aliases::U256,
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
            alloy_sol_types::sol_data::Array<SwapPathStep>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<256>,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::Address,
            alloy_sol_types::private::Vec<<SwapPathStep as alloy_sol_types::SolType>::RustType>,
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
        impl ::core::convert::From<SwapPathExactAmountOut> for UnderlyingRustTuple<'_> {
            fn from(value: SwapPathExactAmountOut) -> Self {
                (
                    value.tokenIn,
                    value.steps,
                    value.maxAmountIn,
                    value.exactAmountOut,
                )
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for SwapPathExactAmountOut {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    tokenIn: tuple.0,
                    steps: tuple.1,
                    maxAmountIn: tuple.2,
                    exactAmountOut: tuple.3,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for SwapPathExactAmountOut {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for SwapPathExactAmountOut {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.tokenIn,
                    ),
                    <alloy_sol_types::sol_data::Array<
                        SwapPathStep,
                    > as alloy_sol_types::SolType>::tokenize(&self.steps),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.maxAmountIn),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.exactAmountOut),
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
        impl alloy_sol_types::SolType for SwapPathExactAmountOut {
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
        impl alloy_sol_types::SolStruct for SwapPathExactAmountOut {
            const NAME: &'static str = "SwapPathExactAmountOut";

            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "SwapPathExactAmountOut(address tokenIn,SwapPathStep[] steps,uint256 \
                     maxAmountIn,uint256 exactAmountOut)",
                )
            }

            #[inline]
            fn eip712_components()
            -> alloy_sol_types::private::Vec<alloy_sol_types::private::Cow<'static, str>>
            {
                let mut components = alloy_sol_types::private::Vec::with_capacity(1);
                components.push(<SwapPathStep as alloy_sol_types::SolStruct>::eip712_root_type());
                components
                    .extend(<SwapPathStep as alloy_sol_types::SolStruct>::eip712_components());
                components
            }

            #[inline]
            fn eip712_encode_data(&self) -> alloy_sol_types::private::Vec<u8> {
                [
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.tokenIn,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Array<
                        SwapPathStep,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.steps)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.maxAmountIn)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(
                            &self.exactAmountOut,
                        )
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for SwapPathExactAmountOut {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.tokenIn,
                    )
                    + <alloy_sol_types::sol_data::Array<
                        SwapPathStep,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(&rust.steps)
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.maxAmountIn,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.exactAmountOut,
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
                <alloy_sol_types::sol_data::Array<
                    SwapPathStep,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.steps,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.maxAmountIn,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.exactAmountOut,
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
    struct SwapPathStep { address pool; address tokenOut; bool isBuffer; }
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct SwapPathStep {
        #[allow(missing_docs)]
        pub pool: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub tokenOut: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub isBuffer: bool,
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
        fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
            match _t {
                alloy_sol_types::private::AssertTypeEq::<
                    <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                >(_) => {}
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<SwapPathStep> for UnderlyingRustTuple<'_> {
            fn from(value: SwapPathStep) -> Self {
                (value.pool, value.tokenOut, value.isBuffer)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for SwapPathStep {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    pool: tuple.0,
                    tokenOut: tuple.1,
                    isBuffer: tuple.2,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for SwapPathStep {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for SwapPathStep {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.pool,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.tokenOut,
                    ),
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::tokenize(
                        &self.isBuffer,
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
        impl alloy_sol_types::SolType for SwapPathStep {
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
        impl alloy_sol_types::SolStruct for SwapPathStep {
            const NAME: &'static str = "SwapPathStep";

            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "SwapPathStep(address pool,address tokenOut,bool isBuffer)",
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
                            &self.pool,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.tokenOut,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::eip712_data_word(
                            &self.isBuffer,
                        )
                        .0,
                ]
                    .concat()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::EventTopic for SwapPathStep {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.pool,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.tokenOut,
                    )
                    + <alloy_sol_types::sol_data::Bool as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.isBuffer,
                    )
            }

            #[inline]
            fn encode_topic_preimage(
                rust: &Self::RustType,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                out.reserve(<Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust));
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.pool,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.tokenOut,
                    out,
                );
                <alloy_sol_types::sol_data::Bool as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.isBuffer,
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
    /**Creates a new wrapper around an on-chain [`IBatchRouter`](self) contract instance.

    See the [wrapper's documentation](`IBatchRouterInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> IBatchRouterInstance<P, N> {
        IBatchRouterInstance::<P, N>::new(address, __provider)
    }
    /**A [`IBatchRouter`](self) instance.

    Contains type-safe methods for interacting with an on-chain instance of the
    [`IBatchRouter`](self) contract located at a given `address`, using a given
    provider `P`.

    If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
    documentation on how to provide it), the `deploy` and `deploy_builder` methods can
    be used to deploy a new instance of the contract.

    See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct IBatchRouterInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for IBatchRouterInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("IBatchRouterInstance")
                .field(&self.address)
                .finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        IBatchRouterInstance<P, N>
    {
        /**Creates a new wrapper around an on-chain [`IBatchRouter`](self) contract instance.

        See the [wrapper's documentation](`IBatchRouterInstance`) for more details.*/
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
    impl<P: ::core::clone::Clone, N> IBatchRouterInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned
        /// provider.
        #[inline]
        pub fn with_cloned_provider(self) -> IBatchRouterInstance<P, N> {
            IBatchRouterInstance {
                address: self.address,
                provider: ::core::clone::Clone::clone(&self.provider),
                _network: ::core::marker::PhantomData,
            }
        }
    }
    /// Function calls.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        IBatchRouterInstance<P, N>
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
        IBatchRouterInstance<P, N>
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
library IRouterCommon {
    struct PermitApproval { address token; address owner; address spender; uint256 amount; uint256 nonce; uint256 deadline; }
}
```*/
#[allow(
    non_camel_case_types,
    non_snake_case,
    clippy::pub_underscore_fields,
    clippy::style,
    clippy::empty_structs_with_brackets
)]
pub mod IRouterCommon {
    use {super::*, alloy_sol_types};
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**```solidity
    struct PermitApproval { address token; address owner; address spender; uint256 amount; uint256 nonce; uint256 deadline; }
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct PermitApproval {
        #[allow(missing_docs)]
        pub token: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub owner: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub spender: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub amount: alloy_sol_types::private::primitives::aliases::U256,
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
        use alloy_sol_types;
        #[doc(hidden)]
        #[allow(dead_code)]
        type UnderlyingSolTuple<'a> = (
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<256>,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::Address,
            alloy_sol_types::private::Address,
            alloy_sol_types::private::Address,
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
        impl ::core::convert::From<PermitApproval> for UnderlyingRustTuple<'_> {
            fn from(value: PermitApproval) -> Self {
                (
                    value.token,
                    value.owner,
                    value.spender,
                    value.amount,
                    value.nonce,
                    value.deadline,
                )
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for PermitApproval {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    token: tuple.0,
                    owner: tuple.1,
                    spender: tuple.2,
                    amount: tuple.3,
                    nonce: tuple.4,
                    deadline: tuple.5,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for PermitApproval {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for PermitApproval {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.token,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.owner,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.spender,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.amount,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.nonce,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
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
        impl alloy_sol_types::SolType for PermitApproval {
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
        impl alloy_sol_types::SolStruct for PermitApproval {
            const NAME: &'static str = "PermitApproval";

            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "PermitApproval(address token,address owner,address spender,uint256 \
                     amount,uint256 nonce,uint256 deadline)",
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
                            &self.token,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.owner,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.spender,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.amount)
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
        impl alloy_sol_types::EventTopic for PermitApproval {
            #[inline]
            fn topic_preimage_length(rust: &Self::RustType) -> usize {
                0usize
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.token,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.owner,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.spender,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.amount,
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
                out.reserve(<Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust));
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.token,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.owner,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.spender,
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
            fn encode_topic(rust: &Self::RustType) -> alloy_sol_types::abi::token::WordToken {
                let mut out = alloy_sol_types::private::Vec::new();
                <Self as alloy_sol_types::EventTopic>::encode_topic_preimage(rust, &mut out);
                alloy_sol_types::abi::token::WordToken(alloy_sol_types::private::keccak256(out))
            }
        }
    };
    use alloy_contract;
    /**Creates a new wrapper around an on-chain [`IRouterCommon`](self) contract instance.

    See the [wrapper's documentation](`IRouterCommonInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> IRouterCommonInstance<P, N> {
        IRouterCommonInstance::<P, N>::new(address, __provider)
    }
    /**A [`IRouterCommon`](self) instance.

    Contains type-safe methods for interacting with an on-chain instance of the
    [`IRouterCommon`](self) contract located at a given `address`, using a given
    provider `P`.

    If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
    documentation on how to provide it), the `deploy` and `deploy_builder` methods can
    be used to deploy a new instance of the contract.

    See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct IRouterCommonInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for IRouterCommonInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("IRouterCommonInstance")
                .field(&self.address)
                .finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        IRouterCommonInstance<P, N>
    {
        /**Creates a new wrapper around an on-chain [`IRouterCommon`](self) contract instance.

        See the [wrapper's documentation](`IRouterCommonInstance`) for more details.*/
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
    impl<P: ::core::clone::Clone, N> IRouterCommonInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned
        /// provider.
        #[inline]
        pub fn with_cloned_provider(self) -> IRouterCommonInstance<P, N> {
            IRouterCommonInstance {
                address: self.address,
                provider: ::core::clone::Clone::clone(&self.provider),
                _network: ::core::marker::PhantomData,
            }
        }
    }
    /// Function calls.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        IRouterCommonInstance<P, N>
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
        IRouterCommonInstance<P, N>
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
library IAllowanceTransfer {
    struct PermitBatch {
        PermitDetails[] details;
        address spender;
        uint256 sigDeadline;
    }
    struct PermitDetails {
        address token;
        uint160 amount;
        uint48 expiration;
        uint48 nonce;
    }
}

library IBatchRouter {
    struct SwapExactInHookParams {
        address sender;
        SwapPathExactAmountIn[] paths;
        uint256 deadline;
        bool wethIsEth;
        bytes userData;
    }
    struct SwapExactOutHookParams {
        address sender;
        SwapPathExactAmountOut[] paths;
        uint256 deadline;
        bool wethIsEth;
        bytes userData;
    }
    struct SwapPathExactAmountIn {
        address tokenIn;
        SwapPathStep[] steps;
        uint256 exactAmountIn;
        uint256 minAmountOut;
    }
    struct SwapPathExactAmountOut {
        address tokenIn;
        SwapPathStep[] steps;
        uint256 maxAmountIn;
        uint256 exactAmountOut;
    }
    struct SwapPathStep {
        address pool;
        address tokenOut;
        bool isBuffer;
    }
}

library IRouterCommon {
    struct PermitApproval {
        address token;
        address owner;
        address spender;
        uint256 amount;
        uint256 nonce;
        uint256 deadline;
    }
}

interface BalancerV3BatchRouter {
    error AddressEmptyCode(address target);
    error AddressInsufficientBalance(address account);
    error ErrorSelectorNotFound();
    error EthTransfer();
    error FailedInnerCall();
    error InputLengthMismatch();
    error InsufficientEth();
    error ReentrancyGuardReentrantCall();
    error SafeCastOverflowedUintDowncast(uint8 bits, uint256 value);
    error SafeERC20FailedOperation(address token);
    error SenderIsNotVault(address sender);
    error SwapDeadline();
    error TransientIndexOutOfBounds();

    constructor(address vault, address weth, address permit2, string routerVersion);

    receive() external payable;

    function getSender() external view returns (address);
    function multicall(bytes[] memory data) external payable returns (bytes[] memory results);
    function permitBatchAndCall(IRouterCommon.PermitApproval[] memory permitBatch, bytes[] memory permitSignatures, IAllowanceTransfer.PermitBatch memory permit2Batch, bytes memory permit2Signature, bytes[] memory multicallData) external payable returns (bytes[] memory results);
    function querySwapExactIn(IBatchRouter.SwapPathExactAmountIn[] memory paths, address sender, bytes memory userData) external returns (uint256[] memory pathAmountsOut, address[] memory tokensOut, uint256[] memory amountsOut);
    function querySwapExactInHook(IBatchRouter.SwapExactInHookParams memory params) external returns (uint256[] memory pathAmountsOut, address[] memory tokensOut, uint256[] memory amountsOut);
    function querySwapExactOut(IBatchRouter.SwapPathExactAmountOut[] memory paths, address sender, bytes memory userData) external returns (uint256[] memory pathAmountsIn, address[] memory tokensIn, uint256[] memory amountsIn);
    function querySwapExactOutHook(IBatchRouter.SwapExactOutHookParams memory params) external returns (uint256[] memory pathAmountsIn, address[] memory tokensIn, uint256[] memory amountsIn);
    function swapExactIn(IBatchRouter.SwapPathExactAmountIn[] memory paths, uint256 deadline, bool wethIsEth, bytes memory userData) external payable returns (uint256[] memory pathAmountsOut, address[] memory tokensOut, uint256[] memory amountsOut);
    function swapExactInHook(IBatchRouter.SwapExactInHookParams memory params) external returns (uint256[] memory pathAmountsOut, address[] memory tokensOut, uint256[] memory amountsOut);
    function swapExactOut(IBatchRouter.SwapPathExactAmountOut[] memory paths, uint256 deadline, bool wethIsEth, bytes memory userData) external payable returns (uint256[] memory pathAmountsIn, address[] memory tokensIn, uint256[] memory amountsIn);
    function swapExactOutHook(IBatchRouter.SwapExactOutHookParams memory params) external returns (uint256[] memory pathAmountsIn, address[] memory tokensIn, uint256[] memory amountsIn);
    function version() external view returns (string memory);
}
```

...which was generated by the following JSON ABI:
```json
[
  {
    "type": "constructor",
    "inputs": [
      {
        "name": "vault",
        "type": "address",
        "internalType": "contract IVault"
      },
      {
        "name": "weth",
        "type": "address",
        "internalType": "contract IWETH"
      },
      {
        "name": "permit2",
        "type": "address",
        "internalType": "contract IPermit2"
      },
      {
        "name": "routerVersion",
        "type": "string",
        "internalType": "string"
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
    "name": "getSender",
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
    "name": "multicall",
    "inputs": [
      {
        "name": "data",
        "type": "bytes[]",
        "internalType": "bytes[]"
      }
    ],
    "outputs": [
      {
        "name": "results",
        "type": "bytes[]",
        "internalType": "bytes[]"
      }
    ],
    "stateMutability": "payable"
  },
  {
    "type": "function",
    "name": "permitBatchAndCall",
    "inputs": [
      {
        "name": "permitBatch",
        "type": "tuple[]",
        "internalType": "struct IRouterCommon.PermitApproval[]",
        "components": [
          {
            "name": "token",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "owner",
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
            "type": "uint256",
            "internalType": "uint256"
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
        "name": "permitSignatures",
        "type": "bytes[]",
        "internalType": "bytes[]"
      },
      {
        "name": "permit2Batch",
        "type": "tuple",
        "internalType": "struct IAllowanceTransfer.PermitBatch",
        "components": [
          {
            "name": "details",
            "type": "tuple[]",
            "internalType": "struct IAllowanceTransfer.PermitDetails[]",
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
        "name": "permit2Signature",
        "type": "bytes",
        "internalType": "bytes"
      },
      {
        "name": "multicallData",
        "type": "bytes[]",
        "internalType": "bytes[]"
      }
    ],
    "outputs": [
      {
        "name": "results",
        "type": "bytes[]",
        "internalType": "bytes[]"
      }
    ],
    "stateMutability": "payable"
  },
  {
    "type": "function",
    "name": "querySwapExactIn",
    "inputs": [
      {
        "name": "paths",
        "type": "tuple[]",
        "internalType": "struct IBatchRouter.SwapPathExactAmountIn[]",
        "components": [
          {
            "name": "tokenIn",
            "type": "address",
            "internalType": "contract IERC20"
          },
          {
            "name": "steps",
            "type": "tuple[]",
            "internalType": "struct IBatchRouter.SwapPathStep[]",
            "components": [
              {
                "name": "pool",
                "type": "address",
                "internalType": "address"
              },
              {
                "name": "tokenOut",
                "type": "address",
                "internalType": "contract IERC20"
              },
              {
                "name": "isBuffer",
                "type": "bool",
                "internalType": "bool"
              }
            ]
          },
          {
            "name": "exactAmountIn",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "minAmountOut",
            "type": "uint256",
            "internalType": "uint256"
          }
        ]
      },
      {
        "name": "sender",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "userData",
        "type": "bytes",
        "internalType": "bytes"
      }
    ],
    "outputs": [
      {
        "name": "pathAmountsOut",
        "type": "uint256[]",
        "internalType": "uint256[]"
      },
      {
        "name": "tokensOut",
        "type": "address[]",
        "internalType": "address[]"
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
    "name": "querySwapExactInHook",
    "inputs": [
      {
        "name": "params",
        "type": "tuple",
        "internalType": "struct IBatchRouter.SwapExactInHookParams",
        "components": [
          {
            "name": "sender",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "paths",
            "type": "tuple[]",
            "internalType": "struct IBatchRouter.SwapPathExactAmountIn[]",
            "components": [
              {
                "name": "tokenIn",
                "type": "address",
                "internalType": "contract IERC20"
              },
              {
                "name": "steps",
                "type": "tuple[]",
                "internalType": "struct IBatchRouter.SwapPathStep[]",
                "components": [
                  {
                    "name": "pool",
                    "type": "address",
                    "internalType": "address"
                  },
                  {
                    "name": "tokenOut",
                    "type": "address",
                    "internalType": "contract IERC20"
                  },
                  {
                    "name": "isBuffer",
                    "type": "bool",
                    "internalType": "bool"
                  }
                ]
              },
              {
                "name": "exactAmountIn",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "minAmountOut",
                "type": "uint256",
                "internalType": "uint256"
              }
            ]
          },
          {
            "name": "deadline",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "wethIsEth",
            "type": "bool",
            "internalType": "bool"
          },
          {
            "name": "userData",
            "type": "bytes",
            "internalType": "bytes"
          }
        ]
      }
    ],
    "outputs": [
      {
        "name": "pathAmountsOut",
        "type": "uint256[]",
        "internalType": "uint256[]"
      },
      {
        "name": "tokensOut",
        "type": "address[]",
        "internalType": "address[]"
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
    "name": "querySwapExactOut",
    "inputs": [
      {
        "name": "paths",
        "type": "tuple[]",
        "internalType": "struct IBatchRouter.SwapPathExactAmountOut[]",
        "components": [
          {
            "name": "tokenIn",
            "type": "address",
            "internalType": "contract IERC20"
          },
          {
            "name": "steps",
            "type": "tuple[]",
            "internalType": "struct IBatchRouter.SwapPathStep[]",
            "components": [
              {
                "name": "pool",
                "type": "address",
                "internalType": "address"
              },
              {
                "name": "tokenOut",
                "type": "address",
                "internalType": "contract IERC20"
              },
              {
                "name": "isBuffer",
                "type": "bool",
                "internalType": "bool"
              }
            ]
          },
          {
            "name": "maxAmountIn",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "exactAmountOut",
            "type": "uint256",
            "internalType": "uint256"
          }
        ]
      },
      {
        "name": "sender",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "userData",
        "type": "bytes",
        "internalType": "bytes"
      }
    ],
    "outputs": [
      {
        "name": "pathAmountsIn",
        "type": "uint256[]",
        "internalType": "uint256[]"
      },
      {
        "name": "tokensIn",
        "type": "address[]",
        "internalType": "address[]"
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
    "name": "querySwapExactOutHook",
    "inputs": [
      {
        "name": "params",
        "type": "tuple",
        "internalType": "struct IBatchRouter.SwapExactOutHookParams",
        "components": [
          {
            "name": "sender",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "paths",
            "type": "tuple[]",
            "internalType": "struct IBatchRouter.SwapPathExactAmountOut[]",
            "components": [
              {
                "name": "tokenIn",
                "type": "address",
                "internalType": "contract IERC20"
              },
              {
                "name": "steps",
                "type": "tuple[]",
                "internalType": "struct IBatchRouter.SwapPathStep[]",
                "components": [
                  {
                    "name": "pool",
                    "type": "address",
                    "internalType": "address"
                  },
                  {
                    "name": "tokenOut",
                    "type": "address",
                    "internalType": "contract IERC20"
                  },
                  {
                    "name": "isBuffer",
                    "type": "bool",
                    "internalType": "bool"
                  }
                ]
              },
              {
                "name": "maxAmountIn",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "exactAmountOut",
                "type": "uint256",
                "internalType": "uint256"
              }
            ]
          },
          {
            "name": "deadline",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "wethIsEth",
            "type": "bool",
            "internalType": "bool"
          },
          {
            "name": "userData",
            "type": "bytes",
            "internalType": "bytes"
          }
        ]
      }
    ],
    "outputs": [
      {
        "name": "pathAmountsIn",
        "type": "uint256[]",
        "internalType": "uint256[]"
      },
      {
        "name": "tokensIn",
        "type": "address[]",
        "internalType": "address[]"
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
    "name": "swapExactIn",
    "inputs": [
      {
        "name": "paths",
        "type": "tuple[]",
        "internalType": "struct IBatchRouter.SwapPathExactAmountIn[]",
        "components": [
          {
            "name": "tokenIn",
            "type": "address",
            "internalType": "contract IERC20"
          },
          {
            "name": "steps",
            "type": "tuple[]",
            "internalType": "struct IBatchRouter.SwapPathStep[]",
            "components": [
              {
                "name": "pool",
                "type": "address",
                "internalType": "address"
              },
              {
                "name": "tokenOut",
                "type": "address",
                "internalType": "contract IERC20"
              },
              {
                "name": "isBuffer",
                "type": "bool",
                "internalType": "bool"
              }
            ]
          },
          {
            "name": "exactAmountIn",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "minAmountOut",
            "type": "uint256",
            "internalType": "uint256"
          }
        ]
      },
      {
        "name": "deadline",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "wethIsEth",
        "type": "bool",
        "internalType": "bool"
      },
      {
        "name": "userData",
        "type": "bytes",
        "internalType": "bytes"
      }
    ],
    "outputs": [
      {
        "name": "pathAmountsOut",
        "type": "uint256[]",
        "internalType": "uint256[]"
      },
      {
        "name": "tokensOut",
        "type": "address[]",
        "internalType": "address[]"
      },
      {
        "name": "amountsOut",
        "type": "uint256[]",
        "internalType": "uint256[]"
      }
    ],
    "stateMutability": "payable"
  },
  {
    "type": "function",
    "name": "swapExactInHook",
    "inputs": [
      {
        "name": "params",
        "type": "tuple",
        "internalType": "struct IBatchRouter.SwapExactInHookParams",
        "components": [
          {
            "name": "sender",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "paths",
            "type": "tuple[]",
            "internalType": "struct IBatchRouter.SwapPathExactAmountIn[]",
            "components": [
              {
                "name": "tokenIn",
                "type": "address",
                "internalType": "contract IERC20"
              },
              {
                "name": "steps",
                "type": "tuple[]",
                "internalType": "struct IBatchRouter.SwapPathStep[]",
                "components": [
                  {
                    "name": "pool",
                    "type": "address",
                    "internalType": "address"
                  },
                  {
                    "name": "tokenOut",
                    "type": "address",
                    "internalType": "contract IERC20"
                  },
                  {
                    "name": "isBuffer",
                    "type": "bool",
                    "internalType": "bool"
                  }
                ]
              },
              {
                "name": "exactAmountIn",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "minAmountOut",
                "type": "uint256",
                "internalType": "uint256"
              }
            ]
          },
          {
            "name": "deadline",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "wethIsEth",
            "type": "bool",
            "internalType": "bool"
          },
          {
            "name": "userData",
            "type": "bytes",
            "internalType": "bytes"
          }
        ]
      }
    ],
    "outputs": [
      {
        "name": "pathAmountsOut",
        "type": "uint256[]",
        "internalType": "uint256[]"
      },
      {
        "name": "tokensOut",
        "type": "address[]",
        "internalType": "address[]"
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
    "name": "swapExactOut",
    "inputs": [
      {
        "name": "paths",
        "type": "tuple[]",
        "internalType": "struct IBatchRouter.SwapPathExactAmountOut[]",
        "components": [
          {
            "name": "tokenIn",
            "type": "address",
            "internalType": "contract IERC20"
          },
          {
            "name": "steps",
            "type": "tuple[]",
            "internalType": "struct IBatchRouter.SwapPathStep[]",
            "components": [
              {
                "name": "pool",
                "type": "address",
                "internalType": "address"
              },
              {
                "name": "tokenOut",
                "type": "address",
                "internalType": "contract IERC20"
              },
              {
                "name": "isBuffer",
                "type": "bool",
                "internalType": "bool"
              }
            ]
          },
          {
            "name": "maxAmountIn",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "exactAmountOut",
            "type": "uint256",
            "internalType": "uint256"
          }
        ]
      },
      {
        "name": "deadline",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "wethIsEth",
        "type": "bool",
        "internalType": "bool"
      },
      {
        "name": "userData",
        "type": "bytes",
        "internalType": "bytes"
      }
    ],
    "outputs": [
      {
        "name": "pathAmountsIn",
        "type": "uint256[]",
        "internalType": "uint256[]"
      },
      {
        "name": "tokensIn",
        "type": "address[]",
        "internalType": "address[]"
      },
      {
        "name": "amountsIn",
        "type": "uint256[]",
        "internalType": "uint256[]"
      }
    ],
    "stateMutability": "payable"
  },
  {
    "type": "function",
    "name": "swapExactOutHook",
    "inputs": [
      {
        "name": "params",
        "type": "tuple",
        "internalType": "struct IBatchRouter.SwapExactOutHookParams",
        "components": [
          {
            "name": "sender",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "paths",
            "type": "tuple[]",
            "internalType": "struct IBatchRouter.SwapPathExactAmountOut[]",
            "components": [
              {
                "name": "tokenIn",
                "type": "address",
                "internalType": "contract IERC20"
              },
              {
                "name": "steps",
                "type": "tuple[]",
                "internalType": "struct IBatchRouter.SwapPathStep[]",
                "components": [
                  {
                    "name": "pool",
                    "type": "address",
                    "internalType": "address"
                  },
                  {
                    "name": "tokenOut",
                    "type": "address",
                    "internalType": "contract IERC20"
                  },
                  {
                    "name": "isBuffer",
                    "type": "bool",
                    "internalType": "bool"
                  }
                ]
              },
              {
                "name": "maxAmountIn",
                "type": "uint256",
                "internalType": "uint256"
              },
              {
                "name": "exactAmountOut",
                "type": "uint256",
                "internalType": "uint256"
              }
            ]
          },
          {
            "name": "deadline",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "wethIsEth",
            "type": "bool",
            "internalType": "bool"
          },
          {
            "name": "userData",
            "type": "bytes",
            "internalType": "bytes"
          }
        ]
      }
    ],
    "outputs": [
      {
        "name": "pathAmountsIn",
        "type": "uint256[]",
        "internalType": "uint256[]"
      },
      {
        "name": "tokensIn",
        "type": "address[]",
        "internalType": "address[]"
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
    "name": "version",
    "inputs": [],
    "outputs": [
      {
        "name": "",
        "type": "string",
        "internalType": "string"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "error",
    "name": "AddressEmptyCode",
    "inputs": [
      {
        "name": "target",
        "type": "address",
        "internalType": "address"
      }
    ]
  },
  {
    "type": "error",
    "name": "AddressInsufficientBalance",
    "inputs": [
      {
        "name": "account",
        "type": "address",
        "internalType": "address"
      }
    ]
  },
  {
    "type": "error",
    "name": "ErrorSelectorNotFound",
    "inputs": []
  },
  {
    "type": "error",
    "name": "EthTransfer",
    "inputs": []
  },
  {
    "type": "error",
    "name": "FailedInnerCall",
    "inputs": []
  },
  {
    "type": "error",
    "name": "InputLengthMismatch",
    "inputs": []
  },
  {
    "type": "error",
    "name": "InsufficientEth",
    "inputs": []
  },
  {
    "type": "error",
    "name": "ReentrancyGuardReentrantCall",
    "inputs": []
  },
  {
    "type": "error",
    "name": "SafeCastOverflowedUintDowncast",
    "inputs": [
      {
        "name": "bits",
        "type": "uint8",
        "internalType": "uint8"
      },
      {
        "name": "value",
        "type": "uint256",
        "internalType": "uint256"
      }
    ]
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
    "name": "SenderIsNotVault",
    "inputs": [
      {
        "name": "sender",
        "type": "address",
        "internalType": "address"
      }
    ]
  },
  {
    "type": "error",
    "name": "SwapDeadline",
    "inputs": []
  },
  {
    "type": "error",
    "name": "TransientIndexOutOfBounds",
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
pub mod BalancerV3BatchRouter {
    use {super::*, alloy_sol_types};
    /// The creation / init bytecode of the contract.
    ///
    /// ```text
    ///0x6101c0604090808252346105c957614bbd803803809161001f82856105e8565b83398101916080828403126105c95781516001600160a01b03939084811681036105c957602093848101519286841684036105c9578482015196871687036105c95760608201516001600160401b03928382116105c9570192601f908282860112156105c95784518481116105b557601f19958851946100a58b898786011601876105e8565b8286528a83830101116105c957815f928b8093018388015e8501015260805281519283116105b5575f54916001928381811c911680156105ab575b8982101461059757828111610554575b50879184116001146104f757839450908392915f946104ec575b50501b915f199060031b1c1916175f555b61014961012661060b565b835190610132826105cd565b600682526539b2b73232b960d11b86830152610669565b60a05261018561015761060b565b835190610163826105cd565b60118252701a5cd4995d1d5c9b915d1a131bd8dad959607a1b86830152610669565b60c05260e0526101009283526101cd815161019f816105cd565b601381527f63757272656e7453776170546f6b656e73496e0000000000000000000000000084820152610633565b9161012092835261021082516101e2816105cd565b601481527f63757272656e7453776170546f6b656e734f757400000000000000000000000083820152610633565b6101409081526102528351610224816105cd565b601981527f63757272656e7453776170546f6b656e496e416d6f756e74730000000000000084820152610633565b906101609182526102d8610298855161026a816105cd565b601a81527f63757272656e7453776170546f6b656e4f7574416d6f756e747300000000000086820152610633565b936101809485527f736574746c6564546f6b656e416d6f756e7473000000000000000000000000008651916102cc836105cd565b60138352820152610633565b936101a094855251946144a1968761071c88396080518781816102460152818161197c01528181611be001528181611e22015281816120790152818161221201528181612323015281816123b10152818161247301528181612aad01528181612c8c01528181612cd401528181612d5201528181612df901528181612f0701528181612f840152818161321901528181613348015281816133e5015281816134ab01528181613b6c01528181613c9101528181613ed0015281816140150152614271015260a0518781816102aa015281816105350152818161181f01526128be015260c0518781816117a901526136ba015260e051878181602201528181613afe01528181613de401528181613f5801526140af0152518681816109f001528181610b0401528181611f6e01528181611ff4015281816130790152613c6d015251858181612569015281816127500152818161295a01526135d8015251848181611c4301528181611e8f01528181612275015281816124d7015281816125ce0152818161272c01528181612b1201526135a8015251838181611d43015281816125950152818161277c0152818161328e01528181613509015261362b015251828181611c6c01528181611ec00152818161250101528181612621015281816127b401528181612b5101526132d001525181818161229f015281816125ff01528181612b8201528181612e5601526136090152f35b015192505f8061010a565b91938316915f805283885f20935f5b8a8883831061053d5750505010610525575b505050811b015f5561011b565b01515f1960f88460031b161c191690555f8080610518565b868601518855909601959485019487935001610506565b5f8052885f208380870160051c8201928b881061058e575b0160051c019084905b8281106105835750506100f0565b5f8155018490610575565b9250819261056c565b634e487b7160e01b5f52602260045260245ffd5b90607f16906100e0565b634e487b7160e01b5f52604160045260245ffd5b5f80fd5b604081019081106001600160401b038211176105b557604052565b601f909101601f19168101906001600160401b038211908210176105b557604052565b60405190610618826105cd565b600c82526b2937baba32b921b7b6b6b7b760a11b6020830152565b61066690604051610643816105cd565b60118152702130ba31b42937baba32b921b7b6b6b7b760791b6020820152610669565b90565b906106d6603a60209260405193849181808401977f62616c616e6365722d6c6162732e76332e73746f726167652e000000000000008952805191829101603986015e830190601760f91b60398301528051928391018583015e015f8382015203601a8101845201826105e8565b5190205f198101908111610707576040519060208201908152602082526106fc826105cd565b9051902060ff191690565b634e487b7160e01b5f52601160045260245ffdfe60806040526004361015610072575b3615610018575f80fd5b6001600160a01b037f000000000000000000000000000000000000000000000000000000000000000016330361004a57005b7f0540ddf6000000000000000000000000000000000000000000000000000000005f5260045ffd5b5f3560e01c806308a465f614610e9d57806319c6989f1461084e578063286f580d146107b75780632950286e146106cc57806354fd4d501461058f5780635a3c3987146105665780635e01eb5a146105215780638a12a08c146104c65780638eb1b65e146103bf578063945ed33f14610344578063ac9650d8146103005763e3b5dff40361000e57346102fc576060806003193601126102fc5767ffffffffffffffff6004358181116102fc5761012d9036906004016112c4565b6101356111a1565b6044359283116102fc57610150610158933690600401610fcd565b9390916128b9565b905f5b835181101561017c57805f8761017360019488611691565b5101520161015b565b506101f06101fe610239946101b65f94886040519361019a8561111a565b30855260208501525f1960408501528660608501523691611381565b60808201526040519283917f8a12a08c0000000000000000000000000000000000000000000000000000000060208401526024830161143e565b03601f198101835282611152565b604051809481927fedfa3568000000000000000000000000000000000000000000000000000000008352602060048401526024830190610ffb565b0381836001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af19182156102f1576102a39261028e915f916102cf575b50602080825183010191016115d4565b909391926102a7575b60405193849384610f2f565b0390f35b5f7f00000000000000000000000000000000000000000000000000000000000000005d610297565b6102eb91503d805f833e6102e38183611152565b81019061154d565b8461027e565b6040513d5f823e3d90fd5b5f80fd5b60206003193601126102fc5760043567ffffffffffffffff81116102fc576103386103326102a3923690600401610f9c565b9061179b565b60405191829182611020565b346102fc5761035236610eca565b61035a611945565b610362611972565b6103906102a3610371836128fb565b9193909461038a606061038383611344565b9201611358565b90612729565b5f7f9b779b17422d0df92223018b32b4d1fa46e071723d6817e2486d003becc55f005d60405193849384610f2f565b60806003193601126102fc5767ffffffffffffffff6004358181116102fc576103ec9036906004016112c4565b906103f56111b7565b906064359081116102fc576101f061048b6102399461045161041c5f953690600401610fcd565b610425336128b9565b97604051946104338661111a565b33865260208601526024356040860152151560608501523691611381565b60808201526040519283917f945ed33f000000000000000000000000000000000000000000000000000000006020840152602483016116d2565b604051809481927f48c89491000000000000000000000000000000000000000000000000000000008352602060048401526024830190610ffb565b346102fc576102a36104ef6104da36610eca565b6104e2611945565b6104ea611972565b611a3b565b5f7f9b779b17422d0df92223018b32b4d1fa46e071723d6817e2486d003becc55f009492945d60405193849384610f2f565b346102fc575f6003193601126102fc5760207f00000000000000000000000000000000000000000000000000000000000000005c6001600160a01b0360405191168152f35b346102fc576102a36104ef61057a36610eca565b610582611945565b61058a611972565b6128fb565b346102fc575f6003193601126102fc576040515f80549060018260011c91600184169182156106c2575b60209485851084146106955785879486865291825f146106575750506001146105fe575b506105ea92500383611152565b6102a3604051928284938452830190610ffb565b5f808052859250907f290decd9548b62a8d60345a988386fc84ba6bc95484008f6362f93160ef3e5635b85831061063f5750506105ea9350820101856105dd565b80548389018501528794508693909201918101610628565b7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff0016858201526105ea95151560051b85010192508791506105dd9050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52602260045260245ffd5b92607f16926105b9565b346102fc5760606003193601126102fc5767ffffffffffffffff6004358181116102fc576106fe9036906004016112c4565b906107076111a1565b6044359182116102fc5761072261072a923690600401610fcd565b9290916128b9565b905f5b845181101561075f57806fffffffffffffffffffffffffffffffff604061075660019489611691565b5101520161072d565b506101f06101fe8561077d5f94610239976040519361019a8561111a565b60808201526040519283917f5a3c3987000000000000000000000000000000000000000000000000000000006020840152602483016116d2565b60806003193601126102fc5767ffffffffffffffff6004358181116102fc576107e49036906004016112c4565b906107ed6111b7565b906064359081116102fc576101f061048b6102399461081461041c5f953690600401610fcd565b60808201526040519283917f08a465f60000000000000000000000000000000000000000000000000000000060208401526024830161143e565b60a06003193601126102fc5767ffffffffffffffff600435116102fc573660236004350112156102fc5767ffffffffffffffff60043560040135116102fc5736602460c060043560040135026004350101116102fc5760243567ffffffffffffffff81116102fc576108c4903690600401610f9c565b67ffffffffffffffff604435116102fc576060600319604435360301126102fc5760643567ffffffffffffffff81116102fc57610905903690600401610fcd565b60843567ffffffffffffffff81116102fc57610925903690600401610f9c565b949093610930611945565b806004356004013503610e75575f5b600435600401358110610bd25750505060443560040135907fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffdd6044353603018212156102fc57816044350160048101359067ffffffffffffffff82116102fc5760248260071b36039101136102fc576109e3575b6102a361033886865f7f9b779b17422d0df92223018b32b4d1fa46e071723d6817e2486d003becc55f005d61179b565b6001600160a01b039492947f0000000000000000000000000000000000000000000000000000000000000000163b156102fc57604051947f2a2d80d10000000000000000000000000000000000000000000000000000000086523360048701526060602487015260c486019260443501602481019367ffffffffffffffff6004830135116102fc57600482013560071b360385136102fc5760606064890152600482013590529192869260e484019291905f905b60048101358210610b5457505050602091601f19601f865f9787956001600160a01b03610ac860246044350161118d565b16608488015260448035013560a48801526003198787030160448801528186528786013787868286010152011601030181836001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af19182156102f1576102a39361033893610b45575b8294508193506109b3565b610b4e90611106565b84610b3a565b9195945091926001600160a01b03610b6b8761118d565b168152602080870135916001600160a01b0383168093036102fc57600492600192820152610b9b604089016128a6565b65ffffffffffff8091166040830152610bb660608a016128a6565b1660608201526080809101970193019050889495939291610a97565b610be7610be082848661192a565b3691611381565b604051610bf3816110a1565b5f81526020915f838301525f60408301528281015190606060408201519101515f1a91835283830152604082015260c07fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffdc81850260043501360301126102fc5760405190610c60826110ea565b610c73602460c08602600435010161118d565b808352610c89604460c08702600435010161118d565b908185850152610ca2606460c08802600435010161118d565b60408581019190915260043560c08802016084810135606087015260a4810135608087015260c4013560a086015283015183519386015160ff91909116926001600160a01b0383163b156102fc575f6001600160a01b03809460e4948b98849860c460c06040519c8d9b8c9a7fd505accf000000000000000000000000000000000000000000000000000000008c521660048b01523060248b0152608482820260043501013560448b0152026004350101356064880152608487015260a486015260c4850152165af19081610e66575b50610e5c57610d7f612877565b906001600160a01b0381511690836001600160a01b0381830151166044604051809581937fdd62ed3e00000000000000000000000000000000000000000000000000000000835260048301523060248301525afa9182156102f1575f92610e2c575b506060015103610df75750506001905b0161093f565b805115610e045780519101fd5b7fa7285689000000000000000000000000000000000000000000000000000000005f5260045ffd5b9091508381813d8311610e55575b610e448183611152565b810103126102fc5751906060610de1565b503d610e3a565b5050600190610df1565b610e6f90611106565b8a610d72565b7faaad13f7000000000000000000000000000000000000000000000000000000005f5260045ffd5b346102fc57610eab36610eca565b610eb3611945565b610ebb611972565b6103906102a361037183611a3b565b600319906020828201126102fc576004359167ffffffffffffffff83116102fc578260a0920301126102fc5760040190565b9081518082526020808093019301915f5b828110610f1b575050505090565b835185529381019392810192600101610f0d565b939290610f4490606086526060860190610efc565b936020948181036020830152602080855192838152019401905f5b818110610f7f57505050610f7c9394506040818403910152610efc565b90565b82516001600160a01b031686529487019491870191600101610f5f565b9181601f840112156102fc5782359167ffffffffffffffff83116102fc576020808501948460051b0101116102fc57565b9181601f840112156102fc5782359167ffffffffffffffff83116102fc57602083818601950101116102fc57565b90601f19601f602080948051918291828752018686015e5f8582860101520116010190565b6020808201906020835283518092526040830192602060408460051b8301019501935f915b8483106110555750505050505090565b9091929394958480611091837fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffc086600196030187528a51610ffb565b9801930193019194939290611045565b6060810190811067ffffffffffffffff8211176110bd57604052565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52604160045260245ffd5b60c0810190811067ffffffffffffffff8211176110bd57604052565b67ffffffffffffffff81116110bd57604052565b60a0810190811067ffffffffffffffff8211176110bd57604052565b60e0810190811067ffffffffffffffff8211176110bd57604052565b90601f601f19910116810190811067ffffffffffffffff8211176110bd57604052565b67ffffffffffffffff81116110bd5760051b60200190565b35906001600160a01b03821682036102fc57565b602435906001600160a01b03821682036102fc57565b6044359081151582036102fc57565b9190916080818403126102fc57604090815191608083019467ffffffffffffffff95848110878211176110bd57825283956112008461118d565b8552602090818501359081116102fc57840182601f820112156102fc5780359061122982611175565b9361123686519586611152565b82855283850190846060809502840101928184116102fc578501915b8383106112745750505050508401528181013590830152606090810135910152565b84838303126102fc57875190611289826110a1565b6112928461118d565b825261129f87850161118d565b87830152888401359081151582036102fc578288928b89950152815201920191611252565b81601f820112156102fc578035916020916112de84611175565b936112ec6040519586611152565b808552838086019160051b830101928084116102fc57848301915b8483106113175750505050505090565b823567ffffffffffffffff81116102fc578691611339848480948901016111c6565b815201920191611307565b356001600160a01b03811681036102fc5790565b3580151581036102fc5790565b67ffffffffffffffff81116110bd57601f01601f191660200190565b92919261138d82611365565b9161139b6040519384611152565b8294818452818301116102fc578281602093845f960137010152565b9060808101916001600160a01b03808251168352602093848301519460808186015285518092528060a086019601925f905b83821061140b5750505050506060816040829301516040850152015191015290565b845180518216895280840151821689850152604090810151151590890152606090970196938201936001909101906113e9565b91909160209081815260c08101916001600160a01b0385511681830152808501519260a06040840152835180915260e08301918060e08360051b8601019501925f905b8382106114bd5750505050506080846040610f7c959601516060840152606081015115158284015201519060a0601f1982850301910152610ffb565b909192939583806114f8837fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff208a600196030186528a516113b7565b98019201920190939291611481565b81601f820112156102fc5780519061151e82611365565b9261152c6040519485611152565b828452602083830101116102fc57815f9260208093018386015e8301015290565b906020828203126102fc57815167ffffffffffffffff81116102fc57610f7c9201611507565b9080601f830112156102fc5781519060209161158e81611175565b9361159c6040519586611152565b81855260208086019260051b8201019283116102fc57602001905b8282106115c5575050505090565b815181529083019083016115b7565b90916060828403126102fc5781519167ffffffffffffffff928381116102fc5784611600918301611573565b936020808301518581116102fc5783019082601f830112156102fc5781519161162883611175565b926116366040519485611152565b808452828085019160051b830101918583116102fc578301905b82821061167257505050509360408301519081116102fc57610f7c9201611573565b81516001600160a01b03811681036102fc578152908301908301611650565b80518210156116a55760209160051b010190565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52603260045260245ffd5b91909160209081815260c08101916001600160a01b0385511681830152808501519260a06040840152835180915260e08301918060e08360051b8601019501925f905b8382106117515750505050506080846040610f7c959601516060840152606081015115158284015201519060a0601f1982850301910152610ffb565b9091929395838061178c837fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff208a600196030186528a516113b7565b98019201920190939291611715565b91906117a6336128b9565b907f000000000000000000000000000000000000000000000000000000000000000093845c6118b1576001906001865d6117df83611175565b926117ed6040519485611152565b808452601f196117fc82611175565b015f5b8181106118a05750505f5b8181106118575750505050905f61184c92945d7f0000000000000000000000000000000000000000000000000000000000000000805c9161184e575b506136b1565b565b5f905d5f611846565b806118845f8061186c610be08996888a61192a565b602081519101305af461187d612877565b903061415c565b61188e8288611691565b526118998187611691565b500161180a565b8060606020809389010152016117ff565b7f3ee5aeb5000000000000000000000000000000000000000000000000000000005f5260045ffd5b9035907fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe1813603018212156102fc570180359067ffffffffffffffff82116102fc576020019181360383136102fc57565b908210156116a5576119419160051b8101906118d9565b9091565b7f9b779b17422d0df92223018b32b4d1fa46e071723d6817e2486d003becc55f00805c6118b1576001905d565b6001600160a01b037f00000000000000000000000000000000000000000000000000000000000000001633036119a457565b7f089676d5000000000000000000000000000000000000000000000000000000005f523360045260245ffd5b906119da82611175565b6119e76040519182611152565b828152601f196119f78294611175565b0190602036910137565b91908201809211611a0e57565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52601160045260245ffd5b604081013542116126c35790611a5e611a5760208401846136f4565b90506119d0565b915f5b611a6e60208301836136f4565b90508110156125c757611a9881611a93611a8b60208601866136f4565b369391613748565b6111c6565b936040850151936001600160a01b038651169060208701518051156116a55760200151604001511515806125be575b1561256357611aec611ad886611344565b8784611ae660608a01611358565b92613add565b5f5b60208801515181101561255357611b03613788565b6020890151515f198101908111611a0e578214806020830152821582525f1461254c576060890151905b611b3b8360208c0151611691565b51604081015190919015611cee57611bd36001600160a01b03835116936001600160a01b03881685145f14611ce7576001945b60405195611b7b8761111a565b5f8752611b87816137be565b6020870152604086015260609485918d838301526080820152604051809381927f43583be500000000000000000000000000000000000000000000000000000000835260048301613a22565b03815f6001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af19384156102f1575f94611cb0575b50506020015115611c9657816001600160a01b036020611c909360019695611c388c8c611691565b5201611c67828251167f00000000000000000000000000000000000000000000000000000000000000006141c0565b5051167f000000000000000000000000000000000000000000000000000000000000000061420a565b01611aee565b602001519097506001600160a01b03169250600190611c90565b60209294509081611cd592903d10611ce0575b611ccd8183611152565b8101906137f5565b91505092905f611c10565b503d611cc3565b5f94611b6e565b888a6001600160a01b038495945116806001600160a01b038a16145f14612132575050815115905061206e57888a80151580612053575b611f4d575b6001600160a01b03939291611ddd82611e15978b5f95897f0000000000000000000000000000000000000000000000000000000000000000921680885282602052604088205c611f3c575b5050505b6001611d9c8983511660208401998b8b51169080158a14611f3657508391614223565b999092511694611db1608091828101906118d9565b93909460405197611dc1896110ea565b8852306020890152604088015260608701528501523691611381565b60a0820152604051809681927f21457897000000000000000000000000000000000000000000000000000000008352600483016139b1565b0381836001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af19384156102f1575f94611f0c575b506020015115611ee95791611ebc826001600160a01b0360019695611e7a611ee49686611691565b51611e858d8d611691565b52611eb3828251167f00000000000000000000000000000000000000000000000000000000000000006141c0565b50511692611691565b51907f000000000000000000000000000000000000000000000000000000000000000061420a565b611c90565b98506001929450611f02906001600160a01b0392611691565b5197511692611c90565b6020919450611f2c903d805f833e611f248183611152565b810190613969565b5094919050611e52565b91614223565b611f4592614341565b5f8281611d75565b50611f5a90929192611344565b91611f648b6142fd565b6001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000163b156102fc576040517f36c785160000000000000000000000000000000000000000000000000000000081526001600160a01b039485166004820152306024820152908416604482015292871660648401525f8380608481010381836001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af180156102f1578a611ddd8d611e15976001600160a01b03975f95612044575b50975092505091929350611d2a565b61204d90611106565b5f612035565b5061205d82611344565b6001600160a01b0316301415611d25565b906001600160a01b037f000000000000000000000000000000000000000000000000000000000000000016916001600160a01b0384511692803b156102fc576040517fae6393290000000000000000000000000000000000000000000000000000000081526001600160a01b03949094166004850152306024850152604484018c90525f908490606490829084905af180156102f1578a611ddd8d611e15976001600160a01b03975f95612123575b50611d79565b61212c90611106565b5f61211d565b6001600160a01b0360208796949701511690898183145f146123d7576121cd925061220597915060016121735f96956001600160a01b0393848b5116614223565b509282895116956020890151151588146123ae5761219082611344565b945b6121a1608093848101906118d9565b959096604051996121b18b6110ea565b8a52166020890152604088015260608701528501523691611381565b60a0820152604051809581927f4af29ec4000000000000000000000000000000000000000000000000000000008352600483016138f8565b0381836001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af19283156102f1575f93612384575b5060200151156122c357816001600160a01b036020611ee493600196956122698c8c611691565b526122998383830151167f00000000000000000000000000000000000000000000000000000000000000006141c0565b500151167f000000000000000000000000000000000000000000000000000000000000000061420a565b60208181015191516040517f15afd4090000000000000000000000000000000000000000000000000000000081526001600160a01b03918216600482015260248101859052939a50909116945081806044810103815f6001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af180156102f157612359575b50600190611c90565b602090813d831161237d575b61236f8183611152565b810103126102fc575f612350565b503d612365565b60209193506123a4903d805f833e61239c8183611152565b81019061387c565b5093919050612242565b837f00000000000000000000000000000000000000000000000000000000000000001694612192565b6001600160a01b036124669561242e9394956123f860809b8c8101906118d9565b9390946040519761240889611136565b5f8952602089015216604087015260609a8b978888015286015260a08501523691611381565b60c0820152604051809381927f2bfb780c00000000000000000000000000000000000000000000000000000000835260048301613810565b03815f6001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af19384156102f1575f94612525575b50506020015115611c9657816001600160a01b036020611ee493600196956124cb8c8c611691565b526124fb8383830151167f00000000000000000000000000000000000000000000000000000000000000006141c0565b500151167f000000000000000000000000000000000000000000000000000000000000000061420a565b6020929450908161254192903d10611ce057611ccd8183611152565b91505092905f6124a3565b5f90611b2d565b5091955090935050600101611a61565b61258d827f00000000000000000000000000000000000000000000000000000000000000006141c0565b506125b986837f000000000000000000000000000000000000000000000000000000000000000061420a565b611aec565b50321515611ac7565b50506125f27f0000000000000000000000000000000000000000000000000000000000000000613a71565b916125fd83516119d0565b7f0000000000000000000000000000000000000000000000000000000000000000917f000000000000000000000000000000000000000000000000000000000000000091905f5b86518110156126ba576001906001600160a01b0380612663838b611691565b51165f528560205261269160405f205c8261267e858d611691565b51165f528860205260405f205c90611a01565b61269b8387611691565b526126a6828a611691565b51165f52856020525f604081205d01612644565b50949391509150565b7fe08b8af0000000000000000000000000000000000000000000000000000000005f5260045ffd5b905f198201918213600116611a0e57565b7f80000000000000000000000000000000000000000000000000000000000000008114611a0e575f190190565b907f000000000000000000000000000000000000000000000000000000000000000090815c7f0000000000000000000000000000000000000000000000000000000000000000612779815c6126eb565b907f0000000000000000000000000000000000000000000000000000000000000000915b5f81121561283a575050506127b1906126eb565b917f0000000000000000000000000000000000000000000000000000000000000000925b5f8112156127ea575050505061184c906136b1565b61283590825f5261282f60205f83828220015c91828252888152886040916128228a8d8587205c906001600160a01b03891690613eb0565b8484525281205d84613e0d565b506126fc565b6127d5565b61287290825f5261282f60205f8a8785848420015c938484528181526128228c6040948587205c906001600160a01b03891690613add565b61279d565b3d156128a1573d9061288882611365565b916128966040519384611152565b82523d5f602084013e565b606090565b359065ffffffffffff821682036102fc57565b905f917f00000000000000000000000000000000000000000000000000000000000000006001600160a01b03815c16156128f1575050565b909192505d600190565b90604082013542116126c357612917611a5760208401846136f4565b915f5b61292760208301836136f4565b90508110156135d15761294481611a93611a8b60208601866136f4565b60608101519061297e6001600160a01b038251167f00000000000000000000000000000000000000000000000000000000000000006141c0565b506020810151515f198101908111611a0e575b5f8112156129a45750505060010161291a565b6129b2816020840151611691565b516129bb613788565b9082156020830152602084015151805f19810111611a0e575f1901831480835261358f575b6020820151156135545760408401516001600160a01b03855116915b604081015115612c1d5783916001600160a01b036060926020612aa0970151151580612c14575b612bed575b5116906001600160a01b0385168203612be6576001915b60405192612a4c8461111a565b60018452612a59816137be565b6020840152604083015288838301526080820152604051809581927f43583be500000000000000000000000000000000000000000000000000000000835260048301613a22565b03815f6001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af19283156102f15787918b915f95612bbf575b506020015115612bb057612ba69284612b02612bab979694612b7594611691565b52612b366001600160a01b0382167f00000000000000000000000000000000000000000000000000000000000000006141c0565b506001600160a01b03612b4d8460408a01516137b1565b91167f000000000000000000000000000000000000000000000000000000000000000061420a565b6001600160a01b038551167f000000000000000000000000000000000000000000000000000000000000000061420a565b6126fc565b612991565b505050612bab919350926126fc565b6020919550612bdc9060603d606011611ce057611ccd8183611152565b5095919050612ae1565b5f91612a3f565b612c0f612bf98d611344565b8d8b611ae6886040888451169301519301611358565b612a28565b50321515612a23565b906001600160a01b03825116806001600160a01b038516145f14613137575060208401516130495750604051927f967870920000000000000000000000000000000000000000000000000000000084526001600160a01b03831660048501526020846024816001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165afa9384156102f1575f94613015575b5083916001600160a01b038151166001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000163b156102fc576040517fae6393290000000000000000000000000000000000000000000000000000000081526001600160a01b03909116600482015230602482015260448101959095525f8580606481010381836001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af19081156102f157612dec955f92613006575b505b611ddd6001600160a01b03612da88b828551168360208701511690614223565b50925116918c6002612dbf608092838101906118d9565b92909360405196612dcf886110ea565b875230602088015289604088015260608701528501523691611381565b0381836001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af19384156102f1575f94612fe3575b506020015115612ecf57908291612bab9493612e45898d611691565b52612e7a836001600160a01b0384167f000000000000000000000000000000000000000000000000000000000000000061420a565b80831080612eb4575b612e90575b5050506126fc565b612ea6612eac93612ea08b611344565b926137b1565b91614356565b5f8080612e88565b50306001600160a01b03612ec78b611344565b161415612e83565b9450908094808210612ee8575b505050612bab906126fc565b91612ef8602092612f77946137b1565b90612f2d826001600160a01b037f00000000000000000000000000000000000000000000000000000000000000001683614356565b60405193849283927f15afd40900000000000000000000000000000000000000000000000000000000845260048401602090939291936001600160a01b0360408201951681520152565b03815f6001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af180156102f157612fb8575b8080612edc565b602090813d8311612fdc575b612fce8183611152565b810103126102fc575f612fb1565b503d612fc4565b6020919450612ffb903d805f833e611f248183611152565b509094919050612e29565b61300f90611106565b5f612d86565b9093506020813d602011613041575b8161303160209383611152565b810103126102fc5751925f612cbc565b3d9150613024565b909261305489611344565b6001600160a01b033091160361306f575b5f612dec94612d88565b6001600160a01b037f000000000000000000000000000000000000000000000000000000000000000016936130a38a611344565b6130ac846142fd565b90863b156102fc576040517f36c785160000000000000000000000000000000000000000000000000000000081526001600160a01b039182166004820152306024820152918116604483015285166064820152945f908690608490829084905af19081156102f157612dec955f92613128575b50945050613065565b61313190611106565b5f61311f565b6001600160a01b036020849695940151168a8282145f1461340b5750505061320c61316e5f92846001600160a01b03885116614223565b92906131d48c6001600160a01b03808a5116938951151586146133df576131a361319784611344565b935b60808101906118d9565b929093604051966131b3886110ea565b875216602086015260408501528c6060850152600260808501523691611381565b60a0820152604051809381927f4af29ec4000000000000000000000000000000000000000000000000000000008352600483016138f8565b0381836001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af19081156102f1575f916133c4575b5060208401518c908a90156133aa5783836001600160a01b03936132836132899461327c8f9c9b9a98996132b29a611691565b5192611691565b52611691565b5191167f000000000000000000000000000000000000000000000000000000000000000061420a565b51156132f457612bab92916001600160a01b036020612ba6930151167f0000000000000000000000000000000000000000000000000000000000000000614341565b516040517f15afd4090000000000000000000000000000000000000000000000000000000081526001600160a01b0390911660048201526024810191909152602081806044810103815f6001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af180156102f15761337f575b50612bab906126fc565b602090813d83116133a3575b6133958183611152565b810103126102fc575f613375565b503d61338b565b50509091506133bb92939650611691565b519384916132b2565b6133d891503d805f833e61239c8183611152565b9050613249565b6131a3827f00000000000000000000000000000000000000000000000000000000000000001693613199565b61349e965090613466916060948b61342b608099989993848101906118d9565b9390946040519761343b89611136565b6001895260208901526001600160a01b038b1660408901528888015286015260a08501523691611381565b60c0820152604051809581927f2bfb780c00000000000000000000000000000000000000000000000000000000835260048301613810565b03815f6001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af19283156102f15787918b915f9561352d575b506020015115612bb057612ba69284613505612bab9796946001600160a01b0394611691565b52167f000000000000000000000000000000000000000000000000000000000000000061420a565b602091955061354a9060603d606011611ce057611ccd8183611152565b50959190506134df565b6fffffffffffffffffffffffffffffffff6001600160a01b0360206135858188015161357f886126eb565b90611691565b51015116916129fc565b6135cc856001600160a01b0360208401611c67828251167f00000000000000000000000000000000000000000000000000000000000000006141c0565b6129e0565b50506135fc7f0000000000000000000000000000000000000000000000000000000000000000613a71565b9161360783516119d0565b7f0000000000000000000000000000000000000000000000000000000000000000917f000000000000000000000000000000000000000000000000000000000000000091905f5b86518110156126ba576001906001600160a01b038061366d838b611691565b51165f528560205261368860405f205c8261267e858d611691565b6136928387611691565b5261369d828a611691565b51165f52856020525f604081205d0161364e565b4780156136f0577f00000000000000000000000000000000000000000000000000000000000000005c6136f0576001600160a01b0361184c92166140e0565b5050565b9035907fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe1813603018212156102fc570180359067ffffffffffffffff82116102fc57602001918160051b360383136102fc57565b91908110156116a55760051b810135907fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff81813603018212156102fc570190565b604051906040820182811067ffffffffffffffff8211176110bd576040525f6020838281520152565b91908203918211611a0e57565b600211156137c857565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52602160045260245ffd5b908160609103126102fc578051916040602083015192015190565b61010060c0610f7c93602084528051613828816137be565b602085015260208101516001600160a01b0380911660408601528060408301511660608601526060820151166080850152608081015160a085015260a08101518285015201519160e0808201520190610ffb565b90916060828403126102fc5781519167ffffffffffffffff928381116102fc57846138a8918301611573565b9360208201519360408301519081116102fc57610f7c9201611507565b9081518082526020808093019301915f5b8281106138e4575050505090565b8351855293810193928101926001016138d6565b602081526001600160a01b038083511660208301526020830151166040820152613931604083015160c0606084015260e08301906138c5565b9060608301516080820152608083015160058110156137c857610f7c9360a0918284015201519060c0601f1982850301910152610ffb565b916060838303126102fc5782519260208101519267ffffffffffffffff938481116102fc578161399a918401611573565b9360408301519081116102fc57610f7c9201611507565b602081526001600160a01b038083511660208301526020830151166040820152604082015160608201526139f4606083015160c0608084015260e08301906138c5565b90608083015160048110156137c857610f7c9360a0918284015201519060c0601f1982850301910152610ffb565b91909160808060a08301948051613a38816137be565b84526020810151613a48816137be565b60208501526001600160a01b036040820151166040850152606081015160608501520151910152565b90815c613a7d81611175565b613a8a6040519182611152565b818152613a9682611175565b601f196020910136602084013781945f5b848110613ab5575050505050565b600190825f5280845f20015c6001600160a01b03613ad38388611691565b9116905201613aa7565b919280613dd8575b15613c51575050804710613c29576001600160a01b03807f00000000000000000000000000000000000000000000000000000000000000001691823b156102fc57604051907fd0e30db00000000000000000000000000000000000000000000000000000000082525f915f8160048185895af180156102f157613c12575b506044602092937f00000000000000000000000000000000000000000000000000000000000000001694613b98838783614356565b8460405196879485937f15afd409000000000000000000000000000000000000000000000000000000008552600485015260248401525af1908115613c065750613bdf5750565b602090813d8311613bff575b613bf58183611152565b810103126102fc57565b503d613beb565b604051903d90823e3d90fd5b60209250613c1f90611106565b60445f9250613b63565b7fa01a9df6000000000000000000000000000000000000000000000000000000005f5260045ffd5b90915f9080613c61575b50505050565b6001600160a01b0393847f00000000000000000000000000000000000000000000000000000000000000001694807f00000000000000000000000000000000000000000000000000000000000000001691613cbb846142fd565b96803b156102fc576040517f36c785160000000000000000000000000000000000000000000000000000000081526001600160a01b039283166004820152848316602482015297821660448901529186161660648701525f908690608490829084905af19485156102f157613d8095613dc4575b5082936020936040518097819582947f15afd40900000000000000000000000000000000000000000000000000000000845260048401602090939291936001600160a01b0360408201951681520152565b03925af1908115613c065750613d99575b808080613c5b565b602090813d8311613dbd575b613daf8183611152565b810103126102fc575f613d91565b503d613da5565b60209350613dd190611106565b5f92613d2f565b506001600160a01b03807f00000000000000000000000000000000000000000000000000000000000000001690821614613ae5565b6001810191805f5260209183835260405f205c8015155f14613ea7575f1990818101835c8380820191828403613e6a575b5050505050815c81810192818411611a0e575f93815d835284832001015d5f52525f604081205d600190565b613e77613e87938861443a565b865f52885f2001015c918561443a565b835f52808383885f2001015d5f5285855260405f205d5f80808381613e3e565b50505050505f90565b5f949383156140d857806140a3575b15614007576001600160a01b0391827f000000000000000000000000000000000000000000000000000000000000000016803b156102fc576040517fae6393290000000000000000000000000000000000000000000000000000000081526001600160a01b03929092166004830152306024830152604482018590525f908290606490829084905af180156102f157613ff4575b5084827f000000000000000000000000000000000000000000000000000000000000000016803b15613ff05781906024604051809481937f2e1a7d4d0000000000000000000000000000000000000000000000000000000083528960048401525af18015613fe557613fcd575b5061184c939450166140e0565b613fd78691611106565b613fe15784613fc0565b8480fd5b6040513d88823e3d90fd5b5080fd5b613fff919550611106565b5f935f613f53565b929350906001600160a01b037f000000000000000000000000000000000000000000000000000000000000000016803b156102fc576040517fae6393290000000000000000000000000000000000000000000000000000000081526001600160a01b03938416600482015293909216602484015260448301525f908290606490829084905af180156102f15761409a5750565b61184c90611106565b506001600160a01b03807f00000000000000000000000000000000000000000000000000000000000000001690831614613ebf565b505050509050565b814710614130575f8080936001600160a01b038294165af1614100612877565b501561410857565b7f1425ea42000000000000000000000000000000000000000000000000000000005f5260045ffd5b7fcd786059000000000000000000000000000000000000000000000000000000005f523060045260245ffd5b90614171575080511561410857805190602001fd5b815115806141b7575b614182575090565b6001600160a01b03907f9996b315000000000000000000000000000000000000000000000000000000005f521660045260245ffd5b50803b1561417a565b6001810190825f528160205260405f205c155f1461420357805c815f52838160205f20015d60018101809111611a0e57815d5c915f5260205260405f205d600190565b5050505f90565b905f5260205261421f60405f2091825c611a01565b905d565b916044929391936001600160a01b03604094859282808551998a9586947fc9c1661b0000000000000000000000000000000000000000000000000000000086521660048501521660248301527f0000000000000000000000000000000000000000000000000000000000000000165afa9384156142f3575f935f956142bc575b50506142b96142b285946119d0565b9485611691565b52565b809295508194503d83116142ec575b6142d58183611152565b810103126102fc5760208251920151925f806142a3565b503d6142cb565b83513d5f823e3d90fd5b6001600160a01b0390818111614311571690565b7f6dfcc650000000000000000000000000000000000000000000000000000000005f5260a060045260245260445ffd5b905f5260205261421f60405f2091825c6137b1565b6040519260208401907fa9059cbb0000000000000000000000000000000000000000000000000000000082526001600160a01b038094166024860152604485015260448452608084019084821067ffffffffffffffff8311176110bd576143d5935f9384936040521694519082865af16143ce612877565b908361415c565b8051908115159182614416575b50506143eb5750565b7f5274afe7000000000000000000000000000000000000000000000000000000005f5260045260245ffd5b81925090602091810103126102fc57602001518015908115036102fc575f806143e2565b5c111561444357565b7f0f4ae0e4000000000000000000000000000000000000000000000000000000005f5260045ffdfea2646970667358221220229a5cf89aa7c2d0a4b4d5db20bba6c2b3a74b080303fc6ec00ba582a5dcf75164736f6c634300081a0033
    /// ```
    #[rustfmt::skip]
    #[allow(clippy::all)]
    pub static BYTECODE: alloy_sol_types::private::Bytes = alloy_sol_types::private::Bytes::from_static(
        b"a\x01\xC0`@\x90\x80\x82R4a\x05\xC9WaK\xBD\x808\x03\x80\x91a\0\x1F\x82\x85a\x05\xE8V[\x839\x81\x01\x91`\x80\x82\x84\x03\x12a\x05\xC9W\x81Q`\x01`\x01`\xA0\x1B\x03\x93\x90\x84\x81\x16\x81\x03a\x05\xC9W` \x93\x84\x81\x01Q\x92\x86\x84\x16\x84\x03a\x05\xC9W\x84\x82\x01Q\x96\x87\x16\x87\x03a\x05\xC9W``\x82\x01Q`\x01`\x01`@\x1B\x03\x92\x83\x82\x11a\x05\xC9W\x01\x92`\x1F\x90\x82\x82\x86\x01\x12\x15a\x05\xC9W\x84Q\x84\x81\x11a\x05\xB5W`\x1F\x19\x95\x88Q\x94a\0\xA5\x8B\x89\x87\x86\x01\x16\x01\x87a\x05\xE8V[\x82\x86R\x8A\x83\x83\x01\x01\x11a\x05\xC9W\x81_\x92\x8B\x80\x93\x01\x83\x88\x01^\x85\x01\x01R`\x80R\x81Q\x92\x83\x11a\x05\xB5W_T\x91`\x01\x92\x83\x81\x81\x1C\x91\x16\x80\x15a\x05\xABW[\x89\x82\x10\x14a\x05\x97W\x82\x81\x11a\x05TW[P\x87\x91\x84\x11`\x01\x14a\x04\xF7W\x83\x94P\x90\x83\x92\x91_\x94a\x04\xECW[PP\x1B\x91_\x19\x90`\x03\x1B\x1C\x19\x16\x17_U[a\x01Ia\x01&a\x06\x0BV[\x83Q\x90a\x012\x82a\x05\xCDV[`\x06\x82Re9\xB2\xB722\xB9`\xD1\x1B\x86\x83\x01Ra\x06iV[`\xA0Ra\x01\x85a\x01Wa\x06\x0BV[\x83Q\x90a\x01c\x82a\x05\xCDV[`\x11\x82Rp\x1A\\\xD4\x99]\x1D\\\x9B\x91]\x1A\x13\x1B\xD8\xDA\xD9Y`z\x1B\x86\x83\x01Ra\x06iV[`\xC0R`\xE0Ra\x01\0\x92\x83Ra\x01\xCD\x81Qa\x01\x9F\x81a\x05\xCDV[`\x13\x81R\x7FcurrentSwapTokensIn\0\0\0\0\0\0\0\0\0\0\0\0\0\x84\x82\x01Ra\x063V[\x91a\x01 \x92\x83Ra\x02\x10\x82Qa\x01\xE2\x81a\x05\xCDV[`\x14\x81R\x7FcurrentSwapTokensOut\0\0\0\0\0\0\0\0\0\0\0\0\x83\x82\x01Ra\x063V[a\x01@\x90\x81Ra\x02R\x83Qa\x02$\x81a\x05\xCDV[`\x19\x81R\x7FcurrentSwapTokenInAmounts\0\0\0\0\0\0\0\x84\x82\x01Ra\x063V[\x90a\x01`\x91\x82Ra\x02\xD8a\x02\x98\x85Qa\x02j\x81a\x05\xCDV[`\x1A\x81R\x7FcurrentSwapTokenOutAmounts\0\0\0\0\0\0\x86\x82\x01Ra\x063V[\x93a\x01\x80\x94\x85R\x7FsettledTokenAmounts\0\0\0\0\0\0\0\0\0\0\0\0\0\x86Q\x91a\x02\xCC\x83a\x05\xCDV[`\x13\x83R\x82\x01Ra\x063V[\x93a\x01\xA0\x94\x85RQ\x94aD\xA1\x96\x87a\x07\x1C\x889`\x80Q\x87\x81\x81a\x02F\x01R\x81\x81a\x19|\x01R\x81\x81a\x1B\xE0\x01R\x81\x81a\x1E\"\x01R\x81\x81a y\x01R\x81\x81a\"\x12\x01R\x81\x81a##\x01R\x81\x81a#\xB1\x01R\x81\x81a$s\x01R\x81\x81a*\xAD\x01R\x81\x81a,\x8C\x01R\x81\x81a,\xD4\x01R\x81\x81a-R\x01R\x81\x81a-\xF9\x01R\x81\x81a/\x07\x01R\x81\x81a/\x84\x01R\x81\x81a2\x19\x01R\x81\x81a3H\x01R\x81\x81a3\xE5\x01R\x81\x81a4\xAB\x01R\x81\x81a;l\x01R\x81\x81a<\x91\x01R\x81\x81a>\xD0\x01R\x81\x81a@\x15\x01RaBq\x01R`\xA0Q\x87\x81\x81a\x02\xAA\x01R\x81\x81a\x055\x01R\x81\x81a\x18\x1F\x01Ra(\xBE\x01R`\xC0Q\x87\x81\x81a\x17\xA9\x01Ra6\xBA\x01R`\xE0Q\x87\x81\x81`\"\x01R\x81\x81a:\xFE\x01R\x81\x81a=\xE4\x01R\x81\x81a?X\x01Ra@\xAF\x01RQ\x86\x81\x81a\t\xF0\x01R\x81\x81a\x0B\x04\x01R\x81\x81a\x1Fn\x01R\x81\x81a\x1F\xF4\x01R\x81\x81a0y\x01Ra<m\x01RQ\x85\x81\x81a%i\x01R\x81\x81a'P\x01R\x81\x81a)Z\x01Ra5\xD8\x01RQ\x84\x81\x81a\x1CC\x01R\x81\x81a\x1E\x8F\x01R\x81\x81a\"u\x01R\x81\x81a$\xD7\x01R\x81\x81a%\xCE\x01R\x81\x81a',\x01R\x81\x81a+\x12\x01Ra5\xA8\x01RQ\x83\x81\x81a\x1DC\x01R\x81\x81a%\x95\x01R\x81\x81a'|\x01R\x81\x81a2\x8E\x01R\x81\x81a5\t\x01Ra6+\x01RQ\x82\x81\x81a\x1Cl\x01R\x81\x81a\x1E\xC0\x01R\x81\x81a%\x01\x01R\x81\x81a&!\x01R\x81\x81a'\xB4\x01R\x81\x81a+Q\x01Ra2\xD0\x01RQ\x81\x81\x81a\"\x9F\x01R\x81\x81a%\xFF\x01R\x81\x81a+\x82\x01R\x81\x81a.V\x01Ra6\t\x01R\xF3[\x01Q\x92P_\x80a\x01\nV[\x91\x93\x83\x16\x91_\x80R\x83\x88_ \x93_[\x8A\x88\x83\x83\x10a\x05=WPPP\x10a\x05%W[PPP\x81\x1B\x01_Ua\x01\x1BV[\x01Q_\x19`\xF8\x84`\x03\x1B\x16\x1C\x19\x16\x90U_\x80\x80a\x05\x18V[\x86\x86\x01Q\x88U\x90\x96\x01\x95\x94\x85\x01\x94\x87\x93P\x01a\x05\x06V[_\x80R\x88_ \x83\x80\x87\x01`\x05\x1C\x82\x01\x92\x8B\x88\x10a\x05\x8EW[\x01`\x05\x1C\x01\x90\x84\x90[\x82\x81\x10a\x05\x83WPPa\0\xF0V[_\x81U\x01\x84\x90a\x05uV[\x92P\x81\x92a\x05lV[cNH{q`\xE0\x1B_R`\"`\x04R`$_\xFD[\x90`\x7F\x16\x90a\0\xE0V[cNH{q`\xE0\x1B_R`A`\x04R`$_\xFD[_\x80\xFD[`@\x81\x01\x90\x81\x10`\x01`\x01`@\x1B\x03\x82\x11\x17a\x05\xB5W`@RV[`\x1F\x90\x91\x01`\x1F\x19\x16\x81\x01\x90`\x01`\x01`@\x1B\x03\x82\x11\x90\x82\x10\x17a\x05\xB5W`@RV[`@Q\x90a\x06\x18\x82a\x05\xCDV[`\x0C\x82Rk)7\xBA\xBA2\xB9!\xB7\xB6\xB6\xB7\xB7`\xA1\x1B` \x83\x01RV[a\x06f\x90`@Qa\x06C\x81a\x05\xCDV[`\x11\x81Rp!0\xBA1\xB4)7\xBA\xBA2\xB9!\xB7\xB6\xB6\xB7\xB7`y\x1B` \x82\x01Ra\x06iV[\x90V[\x90a\x06\xD6`:` \x92`@Q\x93\x84\x91\x81\x80\x84\x01\x97\x7Fbalancer-labs.v3.storage.\0\0\0\0\0\0\0\x89R\x80Q\x91\x82\x91\x01`9\x86\x01^\x83\x01\x90`\x17`\xF9\x1B`9\x83\x01R\x80Q\x92\x83\x91\x01\x85\x83\x01^\x01_\x83\x82\x01R\x03`\x1A\x81\x01\x84R\x01\x82a\x05\xE8V[Q\x90 _\x19\x81\x01\x90\x81\x11a\x07\x07W`@Q\x90` \x82\x01\x90\x81R` \x82Ra\x06\xFC\x82a\x05\xCDV[\x90Q\x90 `\xFF\x19\x16\x90V[cNH{q`\xE0\x1B_R`\x11`\x04R`$_\xFD\xFE`\x80`@R`\x046\x10\x15a\0rW[6\x15a\0\x18W_\x80\xFD[`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x163\x03a\0JW\0[\x7F\x05@\xDD\xF6\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x04_\xFD[_5`\xE0\x1C\x80c\x08\xA4e\xF6\x14a\x0E\x9DW\x80c\x19\xC6\x98\x9F\x14a\x08NW\x80c(oX\r\x14a\x07\xB7W\x80c)P(n\x14a\x06\xCCW\x80cT\xFDMP\x14a\x05\x8FW\x80cZ<9\x87\x14a\x05fW\x80c^\x01\xEBZ\x14a\x05!W\x80c\x8A\x12\xA0\x8C\x14a\x04\xC6W\x80c\x8E\xB1\xB6^\x14a\x03\xBFW\x80c\x94^\xD3?\x14a\x03DW\x80c\xAC\x96P\xD8\x14a\x03\0Wc\xE3\xB5\xDF\xF4\x03a\0\x0EW4a\x02\xFCW``\x80`\x03\x196\x01\x12a\x02\xFCWg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF`\x045\x81\x81\x11a\x02\xFCWa\x01-\x906\x90`\x04\x01a\x12\xC4V[a\x015a\x11\xA1V[`D5\x92\x83\x11a\x02\xFCWa\x01Pa\x01X\x936\x90`\x04\x01a\x0F\xCDV[\x93\x90\x91a(\xB9V[\x90_[\x83Q\x81\x10\x15a\x01|W\x80_\x87a\x01s`\x01\x94\x88a\x16\x91V[Q\x01R\x01a\x01[V[Pa\x01\xF0a\x01\xFEa\x029\x94a\x01\xB6_\x94\x88`@Q\x93a\x01\x9A\x85a\x11\x1AV[0\x85R` \x85\x01R_\x19`@\x85\x01R\x86``\x85\x01R6\x91a\x13\x81V[`\x80\x82\x01R`@Q\x92\x83\x91\x7F\x8A\x12\xA0\x8C\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0` \x84\x01R`$\x83\x01a\x14>V[\x03`\x1F\x19\x81\x01\x83R\x82a\x11RV[`@Q\x80\x94\x81\x92\x7F\xED\xFA5h\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x83R` `\x04\x84\x01R`$\x83\x01\x90a\x0F\xFBV[\x03\x81\x83`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x91\x82\x15a\x02\xF1Wa\x02\xA3\x92a\x02\x8E\x91_\x91a\x02\xCFW[P` \x80\x82Q\x83\x01\x01\x91\x01a\x15\xD4V[\x90\x93\x91\x92a\x02\xA7W[`@Q\x93\x84\x93\x84a\x0F/V[\x03\x90\xF3[_\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0]a\x02\x97V[a\x02\xEB\x91P=\x80_\x83>a\x02\xE3\x81\x83a\x11RV[\x81\x01\x90a\x15MV[\x84a\x02~V[`@Q=_\x82>=\x90\xFD[_\x80\xFD[` `\x03\x196\x01\x12a\x02\xFCW`\x045g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11a\x02\xFCWa\x038a\x032a\x02\xA3\x926\x90`\x04\x01a\x0F\x9CV[\x90a\x17\x9BV[`@Q\x91\x82\x91\x82a\x10 V[4a\x02\xFCWa\x03R6a\x0E\xCAV[a\x03Za\x19EV[a\x03ba\x19rV[a\x03\x90a\x02\xA3a\x03q\x83a(\xFBV[\x91\x93\x90\x94a\x03\x8A``a\x03\x83\x83a\x13DV[\x92\x01a\x13XV[\x90a')V[_\x7F\x9Bw\x9B\x17B-\r\xF9\"#\x01\x8B2\xB4\xD1\xFAF\xE0qr=h\x17\xE2Hm\0;\xEC\xC5_\0]`@Q\x93\x84\x93\x84a\x0F/V[`\x80`\x03\x196\x01\x12a\x02\xFCWg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF`\x045\x81\x81\x11a\x02\xFCWa\x03\xEC\x906\x90`\x04\x01a\x12\xC4V[\x90a\x03\xF5a\x11\xB7V[\x90`d5\x90\x81\x11a\x02\xFCWa\x01\xF0a\x04\x8Ba\x029\x94a\x04Qa\x04\x1C_\x956\x90`\x04\x01a\x0F\xCDV[a\x04%3a(\xB9V[\x97`@Q\x94a\x043\x86a\x11\x1AV[3\x86R` \x86\x01R`$5`@\x86\x01R\x15\x15``\x85\x01R6\x91a\x13\x81V[`\x80\x82\x01R`@Q\x92\x83\x91\x7F\x94^\xD3?\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0` \x84\x01R`$\x83\x01a\x16\xD2V[`@Q\x80\x94\x81\x92\x7FH\xC8\x94\x91\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x83R` `\x04\x84\x01R`$\x83\x01\x90a\x0F\xFBV[4a\x02\xFCWa\x02\xA3a\x04\xEFa\x04\xDA6a\x0E\xCAV[a\x04\xE2a\x19EV[a\x04\xEAa\x19rV[a\x1A;V[_\x7F\x9Bw\x9B\x17B-\r\xF9\"#\x01\x8B2\xB4\xD1\xFAF\xE0qr=h\x17\xE2Hm\0;\xEC\xC5_\0\x94\x92\x94]`@Q\x93\x84\x93\x84a\x0F/V[4a\x02\xFCW_`\x03\x196\x01\x12a\x02\xFCW` \x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\\`\x01`\x01`\xA0\x1B\x03`@Q\x91\x16\x81R\xF3[4a\x02\xFCWa\x02\xA3a\x04\xEFa\x05z6a\x0E\xCAV[a\x05\x82a\x19EV[a\x05\x8Aa\x19rV[a(\xFBV[4a\x02\xFCW_`\x03\x196\x01\x12a\x02\xFCW`@Q_\x80T\x90`\x01\x82`\x01\x1C\x91`\x01\x84\x16\x91\x82\x15a\x06\xC2W[` \x94\x85\x85\x10\x84\x14a\x06\x95W\x85\x87\x94\x86\x86R\x91\x82_\x14a\x06WWPP`\x01\x14a\x05\xFEW[Pa\x05\xEA\x92P\x03\x83a\x11RV[a\x02\xA3`@Q\x92\x82\x84\x93\x84R\x83\x01\x90a\x0F\xFBV[_\x80\x80R\x85\x92P\x90\x7F)\r\xEC\xD9T\x8Bb\xA8\xD6\x03E\xA9\x888o\xC8K\xA6\xBC\x95H@\x08\xF66/\x93\x16\x0E\xF3\xE5c[\x85\x83\x10a\x06?WPPa\x05\xEA\x93P\x82\x01\x01\x85a\x05\xDDV[\x80T\x83\x89\x01\x85\x01R\x87\x94P\x86\x93\x90\x92\x01\x91\x81\x01a\x06(V[\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\0\x16\x85\x82\x01Ra\x05\xEA\x95\x15\x15`\x05\x1B\x85\x01\x01\x92P\x87\x91Pa\x05\xDD\x90PV[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\"`\x04R`$_\xFD[\x92`\x7F\x16\x92a\x05\xB9V[4a\x02\xFCW```\x03\x196\x01\x12a\x02\xFCWg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF`\x045\x81\x81\x11a\x02\xFCWa\x06\xFE\x906\x90`\x04\x01a\x12\xC4V[\x90a\x07\x07a\x11\xA1V[`D5\x91\x82\x11a\x02\xFCWa\x07\"a\x07*\x926\x90`\x04\x01a\x0F\xCDV[\x92\x90\x91a(\xB9V[\x90_[\x84Q\x81\x10\x15a\x07_W\x80o\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF`@a\x07V`\x01\x94\x89a\x16\x91V[Q\x01R\x01a\x07-V[Pa\x01\xF0a\x01\xFE\x85a\x07}_\x94a\x029\x97`@Q\x93a\x01\x9A\x85a\x11\x1AV[`\x80\x82\x01R`@Q\x92\x83\x91\x7FZ<9\x87\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0` \x84\x01R`$\x83\x01a\x16\xD2V[`\x80`\x03\x196\x01\x12a\x02\xFCWg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF`\x045\x81\x81\x11a\x02\xFCWa\x07\xE4\x906\x90`\x04\x01a\x12\xC4V[\x90a\x07\xEDa\x11\xB7V[\x90`d5\x90\x81\x11a\x02\xFCWa\x01\xF0a\x04\x8Ba\x029\x94a\x08\x14a\x04\x1C_\x956\x90`\x04\x01a\x0F\xCDV[`\x80\x82\x01R`@Q\x92\x83\x91\x7F\x08\xA4e\xF6\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0` \x84\x01R`$\x83\x01a\x14>V[`\xA0`\x03\x196\x01\x12a\x02\xFCWg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF`\x045\x11a\x02\xFCW6`#`\x045\x01\x12\x15a\x02\xFCWg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF`\x045`\x04\x015\x11a\x02\xFCW6`$`\xC0`\x045`\x04\x015\x02`\x045\x01\x01\x11a\x02\xFCW`$5g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11a\x02\xFCWa\x08\xC4\x906\x90`\x04\x01a\x0F\x9CV[g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF`D5\x11a\x02\xFCW```\x03\x19`D56\x03\x01\x12a\x02\xFCW`d5g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11a\x02\xFCWa\t\x05\x906\x90`\x04\x01a\x0F\xCDV[`\x845g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11a\x02\xFCWa\t%\x906\x90`\x04\x01a\x0F\x9CV[\x94\x90\x93a\t0a\x19EV[\x80`\x045`\x04\x015\x03a\x0EuW_[`\x045`\x04\x015\x81\x10a\x0B\xD2WPPP`D5`\x04\x015\x90\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xDD`D56\x03\x01\x82\x12\x15a\x02\xFCW\x81`D5\x01`\x04\x81\x015\x90g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11a\x02\xFCW`$\x82`\x07\x1B6\x03\x91\x01\x13a\x02\xFCWa\t\xE3W[a\x02\xA3a\x038\x86\x86_\x7F\x9Bw\x9B\x17B-\r\xF9\"#\x01\x8B2\xB4\xD1\xFAF\xE0qr=h\x17\xE2Hm\0;\xEC\xC5_\0]a\x17\x9BV[`\x01`\x01`\xA0\x1B\x03\x94\x92\x94\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16;\x15a\x02\xFCW`@Q\x94\x7F*-\x80\xD1\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x86R3`\x04\x87\x01R```$\x87\x01R`\xC4\x86\x01\x92`D5\x01`$\x81\x01\x93g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF`\x04\x83\x015\x11a\x02\xFCW`\x04\x82\x015`\x07\x1B6\x03\x85\x13a\x02\xFCW```d\x89\x01R`\x04\x82\x015\x90R\x91\x92\x86\x92`\xE4\x84\x01\x92\x91\x90_\x90[`\x04\x81\x015\x82\x10a\x0BTWPPP` \x91`\x1F\x19`\x1F\x86_\x97\x87\x95`\x01`\x01`\xA0\x1B\x03a\n\xC8`$`D5\x01a\x11\x8DV[\x16`\x84\x88\x01R`D\x805\x015`\xA4\x88\x01R`\x03\x19\x87\x87\x03\x01`D\x88\x01R\x81\x86R\x87\x86\x017\x87\x86\x82\x86\x01\x01R\x01\x16\x01\x03\x01\x81\x83`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x91\x82\x15a\x02\xF1Wa\x02\xA3\x93a\x038\x93a\x0BEW[\x82\x94P\x81\x93Pa\t\xB3V[a\x0BN\x90a\x11\x06V[\x84a\x0B:V[\x91\x95\x94P\x91\x92`\x01`\x01`\xA0\x1B\x03a\x0Bk\x87a\x11\x8DV[\x16\x81R` \x80\x87\x015\x91`\x01`\x01`\xA0\x1B\x03\x83\x16\x80\x93\x03a\x02\xFCW`\x04\x92`\x01\x92\x82\x01Ra\x0B\x9B`@\x89\x01a(\xA6V[e\xFF\xFF\xFF\xFF\xFF\xFF\x80\x91\x16`@\x83\x01Ra\x0B\xB6``\x8A\x01a(\xA6V[\x16``\x82\x01R`\x80\x80\x91\x01\x97\x01\x93\x01\x90P\x88\x94\x95\x93\x92\x91a\n\x97V[a\x0B\xE7a\x0B\xE0\x82\x84\x86a\x19*V[6\x91a\x13\x81V[`@Qa\x0B\xF3\x81a\x10\xA1V[_\x81R` \x91_\x83\x83\x01R_`@\x83\x01R\x82\x81\x01Q\x90```@\x82\x01Q\x91\x01Q_\x1A\x91\x83R\x83\x83\x01R`@\x82\x01R`\xC0\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xDC\x81\x85\x02`\x045\x016\x03\x01\x12a\x02\xFCW`@Q\x90a\x0C`\x82a\x10\xEAV[a\x0Cs`$`\xC0\x86\x02`\x045\x01\x01a\x11\x8DV[\x80\x83Ra\x0C\x89`D`\xC0\x87\x02`\x045\x01\x01a\x11\x8DV[\x90\x81\x85\x85\x01Ra\x0C\xA2`d`\xC0\x88\x02`\x045\x01\x01a\x11\x8DV[`@\x85\x81\x01\x91\x90\x91R`\x045`\xC0\x88\x02\x01`\x84\x81\x015``\x87\x01R`\xA4\x81\x015`\x80\x87\x01R`\xC4\x015`\xA0\x86\x01R\x83\x01Q\x83Q\x93\x86\x01Q`\xFF\x91\x90\x91\x16\x92`\x01`\x01`\xA0\x1B\x03\x83\x16;\x15a\x02\xFCW_`\x01`\x01`\xA0\x1B\x03\x80\x94`\xE4\x94\x8B\x98\x84\x98`\xC4`\xC0`@Q\x9C\x8D\x9B\x8C\x9A\x7F\xD5\x05\xAC\xCF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x8CR\x16`\x04\x8B\x01R0`$\x8B\x01R`\x84\x82\x82\x02`\x045\x01\x015`D\x8B\x01R\x02`\x045\x01\x015`d\x88\x01R`\x84\x87\x01R`\xA4\x86\x01R`\xC4\x85\x01R\x16Z\xF1\x90\x81a\x0EfW[Pa\x0E\\Wa\r\x7Fa(wV[\x90`\x01`\x01`\xA0\x1B\x03\x81Q\x16\x90\x83`\x01`\x01`\xA0\x1B\x03\x81\x83\x01Q\x16`D`@Q\x80\x95\x81\x93\x7F\xDDb\xED>\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x83R`\x04\x83\x01R0`$\x83\x01RZ\xFA\x91\x82\x15a\x02\xF1W_\x92a\x0E,W[P``\x01Q\x03a\r\xF7WPP`\x01\x90[\x01a\t?V[\x80Q\x15a\x0E\x04W\x80Q\x91\x01\xFD[\x7F\xA7(V\x89\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x04_\xFD[\x90\x91P\x83\x81\x81=\x83\x11a\x0EUW[a\x0ED\x81\x83a\x11RV[\x81\x01\x03\x12a\x02\xFCWQ\x90``a\r\xE1V[P=a\x0E:V[PP`\x01\x90a\r\xF1V[a\x0Eo\x90a\x11\x06V[\x8Aa\rrV[\x7F\xAA\xAD\x13\xF7\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x04_\xFD[4a\x02\xFCWa\x0E\xAB6a\x0E\xCAV[a\x0E\xB3a\x19EV[a\x0E\xBBa\x19rV[a\x03\x90a\x02\xA3a\x03q\x83a\x1A;V[`\x03\x19\x90` \x82\x82\x01\x12a\x02\xFCW`\x045\x91g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x83\x11a\x02\xFCW\x82`\xA0\x92\x03\x01\x12a\x02\xFCW`\x04\x01\x90V[\x90\x81Q\x80\x82R` \x80\x80\x93\x01\x93\x01\x91_[\x82\x81\x10a\x0F\x1BWPPPP\x90V[\x83Q\x85R\x93\x81\x01\x93\x92\x81\x01\x92`\x01\x01a\x0F\rV[\x93\x92\x90a\x0FD\x90``\x86R``\x86\x01\x90a\x0E\xFCV[\x93` \x94\x81\x81\x03` \x83\x01R` \x80\x85Q\x92\x83\x81R\x01\x94\x01\x90_[\x81\x81\x10a\x0F\x7FWPPPa\x0F|\x93\x94P`@\x81\x84\x03\x91\x01Ra\x0E\xFCV[\x90V[\x82Q`\x01`\x01`\xA0\x1B\x03\x16\x86R\x94\x87\x01\x94\x91\x87\x01\x91`\x01\x01a\x0F_V[\x91\x81`\x1F\x84\x01\x12\x15a\x02\xFCW\x825\x91g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x83\x11a\x02\xFCW` \x80\x85\x01\x94\x84`\x05\x1B\x01\x01\x11a\x02\xFCWV[\x91\x81`\x1F\x84\x01\x12\x15a\x02\xFCW\x825\x91g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x83\x11a\x02\xFCW` \x83\x81\x86\x01\x95\x01\x01\x11a\x02\xFCWV[\x90`\x1F\x19`\x1F` \x80\x94\x80Q\x91\x82\x91\x82\x87R\x01\x86\x86\x01^_\x85\x82\x86\x01\x01R\x01\x16\x01\x01\x90V[` \x80\x82\x01\x90` \x83R\x83Q\x80\x92R`@\x83\x01\x92` `@\x84`\x05\x1B\x83\x01\x01\x95\x01\x93_\x91[\x84\x83\x10a\x10UWPPPPPP\x90V[\x90\x91\x92\x93\x94\x95\x84\x80a\x10\x91\x83\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xC0\x86`\x01\x96\x03\x01\x87R\x8AQa\x0F\xFBV[\x98\x01\x93\x01\x93\x01\x91\x94\x93\x92\x90a\x10EV[``\x81\x01\x90\x81\x10g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11\x17a\x10\xBDW`@RV[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`A`\x04R`$_\xFD[`\xC0\x81\x01\x90\x81\x10g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11\x17a\x10\xBDW`@RV[g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11a\x10\xBDW`@RV[`\xA0\x81\x01\x90\x81\x10g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11\x17a\x10\xBDW`@RV[`\xE0\x81\x01\x90\x81\x10g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11\x17a\x10\xBDW`@RV[\x90`\x1F`\x1F\x19\x91\x01\x16\x81\x01\x90\x81\x10g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11\x17a\x10\xBDW`@RV[g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11a\x10\xBDW`\x05\x1B` \x01\x90V[5\x90`\x01`\x01`\xA0\x1B\x03\x82\x16\x82\x03a\x02\xFCWV[`$5\x90`\x01`\x01`\xA0\x1B\x03\x82\x16\x82\x03a\x02\xFCWV[`D5\x90\x81\x15\x15\x82\x03a\x02\xFCWV[\x91\x90\x91`\x80\x81\x84\x03\x12a\x02\xFCW`@\x90\x81Q\x91`\x80\x83\x01\x94g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x95\x84\x81\x10\x87\x82\x11\x17a\x10\xBDW\x82R\x83\x95a\x12\0\x84a\x11\x8DV[\x85R` \x90\x81\x85\x015\x90\x81\x11a\x02\xFCW\x84\x01\x82`\x1F\x82\x01\x12\x15a\x02\xFCW\x805\x90a\x12)\x82a\x11uV[\x93a\x126\x86Q\x95\x86a\x11RV[\x82\x85R\x83\x85\x01\x90\x84``\x80\x95\x02\x84\x01\x01\x92\x81\x84\x11a\x02\xFCW\x85\x01\x91[\x83\x83\x10a\x12tWPPPPP\x84\x01R\x81\x81\x015\x90\x83\x01R``\x90\x81\x015\x91\x01RV[\x84\x83\x83\x03\x12a\x02\xFCW\x87Q\x90a\x12\x89\x82a\x10\xA1V[a\x12\x92\x84a\x11\x8DV[\x82Ra\x12\x9F\x87\x85\x01a\x11\x8DV[\x87\x83\x01R\x88\x84\x015\x90\x81\x15\x15\x82\x03a\x02\xFCW\x82\x88\x92\x8B\x89\x95\x01R\x81R\x01\x92\x01\x91a\x12RV[\x81`\x1F\x82\x01\x12\x15a\x02\xFCW\x805\x91` \x91a\x12\xDE\x84a\x11uV[\x93a\x12\xEC`@Q\x95\x86a\x11RV[\x80\x85R\x83\x80\x86\x01\x91`\x05\x1B\x83\x01\x01\x92\x80\x84\x11a\x02\xFCW\x84\x83\x01\x91[\x84\x83\x10a\x13\x17WPPPPPP\x90V[\x825g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11a\x02\xFCW\x86\x91a\x139\x84\x84\x80\x94\x89\x01\x01a\x11\xC6V[\x81R\x01\x92\x01\x91a\x13\x07V[5`\x01`\x01`\xA0\x1B\x03\x81\x16\x81\x03a\x02\xFCW\x90V[5\x80\x15\x15\x81\x03a\x02\xFCW\x90V[g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11a\x10\xBDW`\x1F\x01`\x1F\x19\x16` \x01\x90V[\x92\x91\x92a\x13\x8D\x82a\x13eV[\x91a\x13\x9B`@Q\x93\x84a\x11RV[\x82\x94\x81\x84R\x81\x83\x01\x11a\x02\xFCW\x82\x81` \x93\x84_\x96\x017\x01\x01RV[\x90`\x80\x81\x01\x91`\x01`\x01`\xA0\x1B\x03\x80\x82Q\x16\x83R` \x93\x84\x83\x01Q\x94`\x80\x81\x86\x01R\x85Q\x80\x92R\x80`\xA0\x86\x01\x96\x01\x92_\x90[\x83\x82\x10a\x14\x0BWPPPPP``\x81`@\x82\x93\x01Q`@\x85\x01R\x01Q\x91\x01R\x90V[\x84Q\x80Q\x82\x16\x89R\x80\x84\x01Q\x82\x16\x89\x85\x01R`@\x90\x81\x01Q\x15\x15\x90\x89\x01R``\x90\x97\x01\x96\x93\x82\x01\x93`\x01\x90\x91\x01\x90a\x13\xE9V[\x91\x90\x91` \x90\x81\x81R`\xC0\x81\x01\x91`\x01`\x01`\xA0\x1B\x03\x85Q\x16\x81\x83\x01R\x80\x85\x01Q\x92`\xA0`@\x84\x01R\x83Q\x80\x91R`\xE0\x83\x01\x91\x80`\xE0\x83`\x05\x1B\x86\x01\x01\x95\x01\x92_\x90[\x83\x82\x10a\x14\xBDWPPPPP`\x80\x84`@a\x0F|\x95\x96\x01Q``\x84\x01R``\x81\x01Q\x15\x15\x82\x84\x01R\x01Q\x90`\xA0`\x1F\x19\x82\x85\x03\x01\x91\x01Ra\x0F\xFBV[\x90\x91\x92\x93\x95\x83\x80a\x14\xF8\x83\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF \x8A`\x01\x96\x03\x01\x86R\x8AQa\x13\xB7V[\x98\x01\x92\x01\x92\x01\x90\x93\x92\x91a\x14\x81V[\x81`\x1F\x82\x01\x12\x15a\x02\xFCW\x80Q\x90a\x15\x1E\x82a\x13eV[\x92a\x15,`@Q\x94\x85a\x11RV[\x82\x84R` \x83\x83\x01\x01\x11a\x02\xFCW\x81_\x92` \x80\x93\x01\x83\x86\x01^\x83\x01\x01R\x90V[\x90` \x82\x82\x03\x12a\x02\xFCW\x81Qg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11a\x02\xFCWa\x0F|\x92\x01a\x15\x07V[\x90\x80`\x1F\x83\x01\x12\x15a\x02\xFCW\x81Q\x90` \x91a\x15\x8E\x81a\x11uV[\x93a\x15\x9C`@Q\x95\x86a\x11RV[\x81\x85R` \x80\x86\x01\x92`\x05\x1B\x82\x01\x01\x92\x83\x11a\x02\xFCW` \x01\x90[\x82\x82\x10a\x15\xC5WPPPP\x90V[\x81Q\x81R\x90\x83\x01\x90\x83\x01a\x15\xB7V[\x90\x91``\x82\x84\x03\x12a\x02\xFCW\x81Q\x91g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x92\x83\x81\x11a\x02\xFCW\x84a\x16\0\x91\x83\x01a\x15sV[\x93` \x80\x83\x01Q\x85\x81\x11a\x02\xFCW\x83\x01\x90\x82`\x1F\x83\x01\x12\x15a\x02\xFCW\x81Q\x91a\x16(\x83a\x11uV[\x92a\x166`@Q\x94\x85a\x11RV[\x80\x84R\x82\x80\x85\x01\x91`\x05\x1B\x83\x01\x01\x91\x85\x83\x11a\x02\xFCW\x83\x01\x90[\x82\x82\x10a\x16rWPPPP\x93`@\x83\x01Q\x90\x81\x11a\x02\xFCWa\x0F|\x92\x01a\x15sV[\x81Q`\x01`\x01`\xA0\x1B\x03\x81\x16\x81\x03a\x02\xFCW\x81R\x90\x83\x01\x90\x83\x01a\x16PV[\x80Q\x82\x10\x15a\x16\xA5W` \x91`\x05\x1B\x01\x01\x90V[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`2`\x04R`$_\xFD[\x91\x90\x91` \x90\x81\x81R`\xC0\x81\x01\x91`\x01`\x01`\xA0\x1B\x03\x85Q\x16\x81\x83\x01R\x80\x85\x01Q\x92`\xA0`@\x84\x01R\x83Q\x80\x91R`\xE0\x83\x01\x91\x80`\xE0\x83`\x05\x1B\x86\x01\x01\x95\x01\x92_\x90[\x83\x82\x10a\x17QWPPPPP`\x80\x84`@a\x0F|\x95\x96\x01Q``\x84\x01R``\x81\x01Q\x15\x15\x82\x84\x01R\x01Q\x90`\xA0`\x1F\x19\x82\x85\x03\x01\x91\x01Ra\x0F\xFBV[\x90\x91\x92\x93\x95\x83\x80a\x17\x8C\x83\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF \x8A`\x01\x96\x03\x01\x86R\x8AQa\x13\xB7V[\x98\x01\x92\x01\x92\x01\x90\x93\x92\x91a\x17\x15V[\x91\x90a\x17\xA63a(\xB9V[\x90\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x93\x84\\a\x18\xB1W`\x01\x90`\x01\x86]a\x17\xDF\x83a\x11uV[\x92a\x17\xED`@Q\x94\x85a\x11RV[\x80\x84R`\x1F\x19a\x17\xFC\x82a\x11uV[\x01_[\x81\x81\x10a\x18\xA0WPP_[\x81\x81\x10a\x18WWPPPP\x90_a\x18L\x92\x94]\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x80\\\x91a\x18NW[Pa6\xB1V[V[_\x90]_a\x18FV[\x80a\x18\x84_\x80a\x18la\x0B\xE0\x89\x96\x88\x8Aa\x19*V[` \x81Q\x91\x010Z\xF4a\x18}a(wV[\x900aA\\V[a\x18\x8E\x82\x88a\x16\x91V[Ra\x18\x99\x81\x87a\x16\x91V[P\x01a\x18\nV[\x80``` \x80\x93\x89\x01\x01R\x01a\x17\xFFV[\x7F>\xE5\xAE\xB5\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x04_\xFD[\x905\x90\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE1\x816\x03\x01\x82\x12\x15a\x02\xFCW\x01\x805\x90g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11a\x02\xFCW` \x01\x91\x816\x03\x83\x13a\x02\xFCWV[\x90\x82\x10\x15a\x16\xA5Wa\x19A\x91`\x05\x1B\x81\x01\x90a\x18\xD9V[\x90\x91V[\x7F\x9Bw\x9B\x17B-\r\xF9\"#\x01\x8B2\xB4\xD1\xFAF\xE0qr=h\x17\xE2Hm\0;\xEC\xC5_\0\x80\\a\x18\xB1W`\x01\x90]V[`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x163\x03a\x19\xA4WV[\x7F\x08\x96v\xD5\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R3`\x04R`$_\xFD[\x90a\x19\xDA\x82a\x11uV[a\x19\xE7`@Q\x91\x82a\x11RV[\x82\x81R`\x1F\x19a\x19\xF7\x82\x94a\x11uV[\x01\x90` 6\x91\x017V[\x91\x90\x82\x01\x80\x92\x11a\x1A\x0EWV[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x11`\x04R`$_\xFD[`@\x81\x015B\x11a&\xC3W\x90a\x1A^a\x1AW` \x84\x01\x84a6\xF4V[\x90Pa\x19\xD0V[\x91_[a\x1An` \x83\x01\x83a6\xF4V[\x90P\x81\x10\x15a%\xC7Wa\x1A\x98\x81a\x1A\x93a\x1A\x8B` \x86\x01\x86a6\xF4V[6\x93\x91a7HV[a\x11\xC6V[\x93`@\x85\x01Q\x93`\x01`\x01`\xA0\x1B\x03\x86Q\x16\x90` \x87\x01Q\x80Q\x15a\x16\xA5W` \x01Q`@\x01Q\x15\x15\x80a%\xBEW[\x15a%cWa\x1A\xECa\x1A\xD8\x86a\x13DV[\x87\x84a\x1A\xE6``\x8A\x01a\x13XV[\x92a:\xDDV[_[` \x88\x01QQ\x81\x10\x15a%SWa\x1B\x03a7\x88V[` \x89\x01QQ_\x19\x81\x01\x90\x81\x11a\x1A\x0EW\x82\x14\x80` \x83\x01R\x82\x15\x82R_\x14a%LW``\x89\x01Q\x90[a\x1B;\x83` \x8C\x01Qa\x16\x91V[Q`@\x81\x01Q\x90\x91\x90\x15a\x1C\xEEWa\x1B\xD3`\x01`\x01`\xA0\x1B\x03\x83Q\x16\x93`\x01`\x01`\xA0\x1B\x03\x88\x16\x85\x14_\x14a\x1C\xE7W`\x01\x94[`@Q\x95a\x1B{\x87a\x11\x1AV[_\x87Ra\x1B\x87\x81a7\xBEV[` \x87\x01R`@\x86\x01R``\x94\x85\x91\x8D\x83\x83\x01R`\x80\x82\x01R`@Q\x80\x93\x81\x92\x7FCX;\xE5\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x83R`\x04\x83\x01a:\"V[\x03\x81_`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x93\x84\x15a\x02\xF1W_\x94a\x1C\xB0W[PP` \x01Q\x15a\x1C\x96W\x81`\x01`\x01`\xA0\x1B\x03` a\x1C\x90\x93`\x01\x96\x95a\x1C8\x8C\x8Ca\x16\x91V[R\x01a\x1Cg\x82\x82Q\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aA\xC0V[PQ\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aB\nV[\x01a\x1A\xEEV[` \x01Q\x90\x97P`\x01`\x01`\xA0\x1B\x03\x16\x92P`\x01\x90a\x1C\x90V[` \x92\x94P\x90\x81a\x1C\xD5\x92\x90=\x10a\x1C\xE0W[a\x1C\xCD\x81\x83a\x11RV[\x81\x01\x90a7\xF5V[\x91PP\x92\x90_a\x1C\x10V[P=a\x1C\xC3V[_\x94a\x1BnV[\x88\x8A`\x01`\x01`\xA0\x1B\x03\x84\x95\x94Q\x16\x80`\x01`\x01`\xA0\x1B\x03\x8A\x16\x14_\x14a!2WPP\x81Q\x15\x90Pa nW\x88\x8A\x80\x15\x15\x80a SW[a\x1FMW[`\x01`\x01`\xA0\x1B\x03\x93\x92\x91a\x1D\xDD\x82a\x1E\x15\x97\x8B_\x95\x89\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x92\x16\x80\x88R\x82` R`@\x88 \\a\x1F<W[PPP[`\x01a\x1D\x9C\x89\x83Q\x16` \x84\x01\x99\x8B\x8BQ\x16\x90\x80\x15\x8A\x14a\x1F6WP\x83\x91aB#V[\x99\x90\x92Q\x16\x94a\x1D\xB1`\x80\x91\x82\x81\x01\x90a\x18\xD9V[\x93\x90\x94`@Q\x97a\x1D\xC1\x89a\x10\xEAV[\x88R0` \x89\x01R`@\x88\x01R``\x87\x01R\x85\x01R6\x91a\x13\x81V[`\xA0\x82\x01R`@Q\x80\x96\x81\x92\x7F!Ex\x97\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x83R`\x04\x83\x01a9\xB1V[\x03\x81\x83`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x93\x84\x15a\x02\xF1W_\x94a\x1F\x0CW[P` \x01Q\x15a\x1E\xE9W\x91a\x1E\xBC\x82`\x01`\x01`\xA0\x1B\x03`\x01\x96\x95a\x1Eza\x1E\xE4\x96\x86a\x16\x91V[Qa\x1E\x85\x8D\x8Da\x16\x91V[Ra\x1E\xB3\x82\x82Q\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aA\xC0V[PQ\x16\x92a\x16\x91V[Q\x90\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aB\nV[a\x1C\x90V[\x98P`\x01\x92\x94Pa\x1F\x02\x90`\x01`\x01`\xA0\x1B\x03\x92a\x16\x91V[Q\x97Q\x16\x92a\x1C\x90V[` \x91\x94Pa\x1F,\x90=\x80_\x83>a\x1F$\x81\x83a\x11RV[\x81\x01\x90a9iV[P\x94\x91\x90Pa\x1ERV[\x91aB#V[a\x1FE\x92aCAV[_\x82\x81a\x1DuV[Pa\x1FZ\x90\x92\x91\x92a\x13DV[\x91a\x1Fd\x8BaB\xFDV[`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16;\x15a\x02\xFCW`@Q\x7F6\xC7\x85\x16\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x94\x85\x16`\x04\x82\x01R0`$\x82\x01R\x90\x84\x16`D\x82\x01R\x92\x87\x16`d\x84\x01R_\x83\x80`\x84\x81\x01\x03\x81\x83`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x80\x15a\x02\xF1W\x8Aa\x1D\xDD\x8Da\x1E\x15\x97`\x01`\x01`\xA0\x1B\x03\x97_\x95a DW[P\x97P\x92PP\x91\x92\x93Pa\x1D*V[a M\x90a\x11\x06V[_a 5V[Pa ]\x82a\x13DV[`\x01`\x01`\xA0\x1B\x03\x160\x14\x15a\x1D%V[\x90`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x91`\x01`\x01`\xA0\x1B\x03\x84Q\x16\x92\x80;\x15a\x02\xFCW`@Q\x7F\xAEc\x93)\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x94\x90\x94\x16`\x04\x85\x01R0`$\x85\x01R`D\x84\x01\x8C\x90R_\x90\x84\x90`d\x90\x82\x90\x84\x90Z\xF1\x80\x15a\x02\xF1W\x8Aa\x1D\xDD\x8Da\x1E\x15\x97`\x01`\x01`\xA0\x1B\x03\x97_\x95a!#W[Pa\x1DyV[a!,\x90a\x11\x06V[_a!\x1DV[`\x01`\x01`\xA0\x1B\x03` \x87\x96\x94\x97\x01Q\x16\x90\x89\x81\x83\x14_\x14a#\xD7Wa!\xCD\x92Pa\"\x05\x97\x91P`\x01a!s_\x96\x95`\x01`\x01`\xA0\x1B\x03\x93\x84\x8BQ\x16aB#V[P\x92\x82\x89Q\x16\x95` \x89\x01Q\x15\x15\x88\x14a#\xAEWa!\x90\x82a\x13DV[\x94[a!\xA1`\x80\x93\x84\x81\x01\x90a\x18\xD9V[\x95\x90\x96`@Q\x99a!\xB1\x8Ba\x10\xEAV[\x8AR\x16` \x89\x01R`@\x88\x01R``\x87\x01R\x85\x01R6\x91a\x13\x81V[`\xA0\x82\x01R`@Q\x80\x95\x81\x92\x7FJ\xF2\x9E\xC4\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x83R`\x04\x83\x01a8\xF8V[\x03\x81\x83`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x92\x83\x15a\x02\xF1W_\x93a#\x84W[P` \x01Q\x15a\"\xC3W\x81`\x01`\x01`\xA0\x1B\x03` a\x1E\xE4\x93`\x01\x96\x95a\"i\x8C\x8Ca\x16\x91V[Ra\"\x99\x83\x83\x83\x01Q\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aA\xC0V[P\x01Q\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aB\nV[` \x81\x81\x01Q\x91Q`@Q\x7F\x15\xAF\xD4\t\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x91\x82\x16`\x04\x82\x01R`$\x81\x01\x85\x90R\x93\x9AP\x90\x91\x16\x94P\x81\x80`D\x81\x01\x03\x81_`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x80\x15a\x02\xF1Wa#YW[P`\x01\x90a\x1C\x90V[` \x90\x81=\x83\x11a#}W[a#o\x81\x83a\x11RV[\x81\x01\x03\x12a\x02\xFCW_a#PV[P=a#eV[` \x91\x93Pa#\xA4\x90=\x80_\x83>a#\x9C\x81\x83a\x11RV[\x81\x01\x90a8|V[P\x93\x91\x90Pa\"BV[\x83\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x94a!\x92V[`\x01`\x01`\xA0\x1B\x03a$f\x95a$.\x93\x94\x95a#\xF8`\x80\x9B\x8C\x81\x01\x90a\x18\xD9V[\x93\x90\x94`@Q\x97a$\x08\x89a\x116V[_\x89R` \x89\x01R\x16`@\x87\x01R``\x9A\x8B\x97\x88\x88\x01R\x86\x01R`\xA0\x85\x01R6\x91a\x13\x81V[`\xC0\x82\x01R`@Q\x80\x93\x81\x92\x7F+\xFBx\x0C\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x83R`\x04\x83\x01a8\x10V[\x03\x81_`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x93\x84\x15a\x02\xF1W_\x94a%%W[PP` \x01Q\x15a\x1C\x96W\x81`\x01`\x01`\xA0\x1B\x03` a\x1E\xE4\x93`\x01\x96\x95a$\xCB\x8C\x8Ca\x16\x91V[Ra$\xFB\x83\x83\x83\x01Q\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aA\xC0V[P\x01Q\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aB\nV[` \x92\x94P\x90\x81a%A\x92\x90=\x10a\x1C\xE0Wa\x1C\xCD\x81\x83a\x11RV[\x91PP\x92\x90_a$\xA3V[_\x90a\x1B-V[P\x91\x95P\x90\x93PP`\x01\x01a\x1AaV[a%\x8D\x82\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aA\xC0V[Pa%\xB9\x86\x83\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aB\nV[a\x1A\xECV[P2\x15\x15a\x1A\xC7V[PPa%\xF2\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0a:qV[\x91a%\xFD\x83Qa\x19\xD0V[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x91\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x91\x90_[\x86Q\x81\x10\x15a&\xBAW`\x01\x90`\x01`\x01`\xA0\x1B\x03\x80a&c\x83\x8Ba\x16\x91V[Q\x16_R\x85` Ra&\x91`@_ \\\x82a&~\x85\x8Da\x16\x91V[Q\x16_R\x88` R`@_ \\\x90a\x1A\x01V[a&\x9B\x83\x87a\x16\x91V[Ra&\xA6\x82\x8Aa\x16\x91V[Q\x16_R\x85` R_`@\x81 ]\x01a&DV[P\x94\x93\x91P\x91PV[\x7F\xE0\x8B\x8A\xF0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x04_\xFD[\x90_\x19\x82\x01\x91\x82\x13`\x01\x16a\x1A\x0EWV[\x7F\x80\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81\x14a\x1A\x0EW_\x19\x01\x90V[\x90\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90\x81\\\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0a'y\x81\\a&\xEBV[\x90\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x91[_\x81\x12\x15a(:WPPPa'\xB1\x90a&\xEBV[\x91\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x92[_\x81\x12\x15a'\xEAWPPPPa\x18L\x90a6\xB1V[a(5\x90\x82_Ra(/` _\x83\x82\x82 \x01\\\x91\x82\x82R\x88\x81R\x88`@\x91a(\"\x8A\x8D\x85\x87 \\\x90`\x01`\x01`\xA0\x1B\x03\x89\x16\x90a>\xB0V[\x84\x84RR\x81 ]\x84a>\rV[Pa&\xFCV[a'\xD5V[a(r\x90\x82_Ra(/` _\x8A\x87\x85\x84\x84 \x01\\\x93\x84\x84R\x81\x81Ra(\"\x8C`@\x94\x85\x87 \\\x90`\x01`\x01`\xA0\x1B\x03\x89\x16\x90a:\xDDV[a'\x9DV[=\x15a(\xA1W=\x90a(\x88\x82a\x13eV[\x91a(\x96`@Q\x93\x84a\x11RV[\x82R=_` \x84\x01>V[``\x90V[5\x90e\xFF\xFF\xFF\xFF\xFF\xFF\x82\x16\x82\x03a\x02\xFCWV[\x90_\x91\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\x01`\x01`\xA0\x1B\x03\x81\\\x16\x15a(\xF1WPPV[\x90\x91\x92P]`\x01\x90V[\x90`@\x82\x015B\x11a&\xC3Wa)\x17a\x1AW` \x84\x01\x84a6\xF4V[\x91_[a)'` \x83\x01\x83a6\xF4V[\x90P\x81\x10\x15a5\xD1Wa)D\x81a\x1A\x93a\x1A\x8B` \x86\x01\x86a6\xF4V[``\x81\x01Q\x90a)~`\x01`\x01`\xA0\x1B\x03\x82Q\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aA\xC0V[P` \x81\x01QQ_\x19\x81\x01\x90\x81\x11a\x1A\x0EW[_\x81\x12\x15a)\xA4WPPP`\x01\x01a)\x1AV[a)\xB2\x81` \x84\x01Qa\x16\x91V[Qa)\xBBa7\x88V[\x90\x82\x15` \x83\x01R` \x84\x01QQ\x80_\x19\x81\x01\x11a\x1A\x0EW_\x19\x01\x83\x14\x80\x83Ra5\x8FW[` \x82\x01Q\x15a5TW`@\x84\x01Q`\x01`\x01`\xA0\x1B\x03\x85Q\x16\x91[`@\x81\x01Q\x15a,\x1DW\x83\x91`\x01`\x01`\xA0\x1B\x03``\x92` a*\xA0\x97\x01Q\x15\x15\x80a,\x14W[a+\xEDW[Q\x16\x90`\x01`\x01`\xA0\x1B\x03\x85\x16\x82\x03a+\xE6W`\x01\x91[`@Q\x92a*L\x84a\x11\x1AV[`\x01\x84Ra*Y\x81a7\xBEV[` \x84\x01R`@\x83\x01R\x88\x83\x83\x01R`\x80\x82\x01R`@Q\x80\x95\x81\x92\x7FCX;\xE5\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x83R`\x04\x83\x01a:\"V[\x03\x81_`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x92\x83\x15a\x02\xF1W\x87\x91\x8B\x91_\x95a+\xBFW[P` \x01Q\x15a+\xB0Wa+\xA6\x92\x84a+\x02a+\xAB\x97\x96\x94a+u\x94a\x16\x91V[Ra+6`\x01`\x01`\xA0\x1B\x03\x82\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aA\xC0V[P`\x01`\x01`\xA0\x1B\x03a+M\x84`@\x8A\x01Qa7\xB1V[\x91\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aB\nV[`\x01`\x01`\xA0\x1B\x03\x85Q\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aB\nV[a&\xFCV[a)\x91V[PPPa+\xAB\x91\x93P\x92a&\xFCV[` \x91\x95Pa+\xDC\x90``=``\x11a\x1C\xE0Wa\x1C\xCD\x81\x83a\x11RV[P\x95\x91\x90Pa*\xE1V[_\x91a*?V[a,\x0Fa+\xF9\x8Da\x13DV[\x8D\x8Ba\x1A\xE6\x88`@\x88\x84Q\x16\x93\x01Q\x93\x01a\x13XV[a*(V[P2\x15\x15a*#V[\x90`\x01`\x01`\xA0\x1B\x03\x82Q\x16\x80`\x01`\x01`\xA0\x1B\x03\x85\x16\x14_\x14a17WP` \x84\x01Qa0IWP`@Q\x92\x7F\x96xp\x92\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x84R`\x01`\x01`\xA0\x1B\x03\x83\x16`\x04\x85\x01R` \x84`$\x81`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xFA\x93\x84\x15a\x02\xF1W_\x94a0\x15W[P\x83\x91`\x01`\x01`\xA0\x1B\x03\x81Q\x16`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16;\x15a\x02\xFCW`@Q\x7F\xAEc\x93)\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x90\x91\x16`\x04\x82\x01R0`$\x82\x01R`D\x81\x01\x95\x90\x95R_\x85\x80`d\x81\x01\x03\x81\x83`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x90\x81\x15a\x02\xF1Wa-\xEC\x95_\x92a0\x06W[P[a\x1D\xDD`\x01`\x01`\xA0\x1B\x03a-\xA8\x8B\x82\x85Q\x16\x83` \x87\x01Q\x16\x90aB#V[P\x92Q\x16\x91\x8C`\x02a-\xBF`\x80\x92\x83\x81\x01\x90a\x18\xD9V[\x92\x90\x93`@Q\x96a-\xCF\x88a\x10\xEAV[\x87R0` \x88\x01R\x89`@\x88\x01R``\x87\x01R\x85\x01R6\x91a\x13\x81V[\x03\x81\x83`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x93\x84\x15a\x02\xF1W_\x94a/\xE3W[P` \x01Q\x15a.\xCFW\x90\x82\x91a+\xAB\x94\x93a.E\x89\x8Da\x16\x91V[Ra.z\x83`\x01`\x01`\xA0\x1B\x03\x84\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aB\nV[\x80\x83\x10\x80a.\xB4W[a.\x90W[PPPa&\xFCV[a.\xA6a.\xAC\x93a.\xA0\x8Ba\x13DV[\x92a7\xB1V[\x91aCVV[_\x80\x80a.\x88V[P0`\x01`\x01`\xA0\x1B\x03a.\xC7\x8Ba\x13DV[\x16\x14\x15a.\x83V[\x94P\x90\x80\x94\x80\x82\x10a.\xE8W[PPPa+\xAB\x90a&\xFCV[\x91a.\xF8` \x92a/w\x94a7\xB1V[\x90a/-\x82`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x83aCVV[`@Q\x93\x84\x92\x83\x92\x7F\x15\xAF\xD4\t\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x84R`\x04\x84\x01` \x90\x93\x92\x91\x93`\x01`\x01`\xA0\x1B\x03`@\x82\x01\x95\x16\x81R\x01RV[\x03\x81_`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x80\x15a\x02\xF1Wa/\xB8W[\x80\x80a.\xDCV[` \x90\x81=\x83\x11a/\xDCW[a/\xCE\x81\x83a\x11RV[\x81\x01\x03\x12a\x02\xFCW_a/\xB1V[P=a/\xC4V[` \x91\x94Pa/\xFB\x90=\x80_\x83>a\x1F$\x81\x83a\x11RV[P\x90\x94\x91\x90Pa.)V[a0\x0F\x90a\x11\x06V[_a-\x86V[\x90\x93P` \x81=` \x11a0AW[\x81a01` \x93\x83a\x11RV[\x81\x01\x03\x12a\x02\xFCWQ\x92_a,\xBCV[=\x91Pa0$V[\x90\x92a0T\x89a\x13DV[`\x01`\x01`\xA0\x1B\x030\x91\x16\x03a0oW[_a-\xEC\x94a-\x88V[`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x93a0\xA3\x8Aa\x13DV[a0\xAC\x84aB\xFDV[\x90\x86;\x15a\x02\xFCW`@Q\x7F6\xC7\x85\x16\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x91\x82\x16`\x04\x82\x01R0`$\x82\x01R\x91\x81\x16`D\x83\x01R\x85\x16`d\x82\x01R\x94_\x90\x86\x90`\x84\x90\x82\x90\x84\x90Z\xF1\x90\x81\x15a\x02\xF1Wa-\xEC\x95_\x92a1(W[P\x94PPa0eV[a11\x90a\x11\x06V[_a1\x1FV[`\x01`\x01`\xA0\x1B\x03` \x84\x96\x95\x94\x01Q\x16\x8A\x82\x82\x14_\x14a4\x0BWPPPa2\x0Ca1n_\x92\x84`\x01`\x01`\xA0\x1B\x03\x88Q\x16aB#V[\x92\x90a1\xD4\x8C`\x01`\x01`\xA0\x1B\x03\x80\x8AQ\x16\x93\x89Q\x15\x15\x86\x14a3\xDFWa1\xA3a1\x97\x84a\x13DV[\x93[`\x80\x81\x01\x90a\x18\xD9V[\x92\x90\x93`@Q\x96a1\xB3\x88a\x10\xEAV[\x87R\x16` \x86\x01R`@\x85\x01R\x8C``\x85\x01R`\x02`\x80\x85\x01R6\x91a\x13\x81V[`\xA0\x82\x01R`@Q\x80\x93\x81\x92\x7FJ\xF2\x9E\xC4\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x83R`\x04\x83\x01a8\xF8V[\x03\x81\x83`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x90\x81\x15a\x02\xF1W_\x91a3\xC4W[P` \x84\x01Q\x8C\x90\x8A\x90\x15a3\xAAW\x83\x83`\x01`\x01`\xA0\x1B\x03\x93a2\x83a2\x89\x94a2|\x8F\x9C\x9B\x9A\x98\x99a2\xB2\x9Aa\x16\x91V[Q\x92a\x16\x91V[Ra\x16\x91V[Q\x91\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aB\nV[Q\x15a2\xF4Wa+\xAB\x92\x91`\x01`\x01`\xA0\x1B\x03` a+\xA6\x93\x01Q\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aCAV[Q`@Q\x7F\x15\xAF\xD4\t\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x90\x91\x16`\x04\x82\x01R`$\x81\x01\x91\x90\x91R` \x81\x80`D\x81\x01\x03\x81_`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x80\x15a\x02\xF1Wa3\x7FW[Pa+\xAB\x90a&\xFCV[` \x90\x81=\x83\x11a3\xA3W[a3\x95\x81\x83a\x11RV[\x81\x01\x03\x12a\x02\xFCW_a3uV[P=a3\x8BV[PP\x90\x91Pa3\xBB\x92\x93\x96Pa\x16\x91V[Q\x93\x84\x91a2\xB2V[a3\xD8\x91P=\x80_\x83>a#\x9C\x81\x83a\x11RV[\x90Pa2IV[a1\xA3\x82\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x93a1\x99V[a4\x9E\x96P\x90a4f\x91``\x94\x8Ba4+`\x80\x99\x98\x99\x93\x84\x81\x01\x90a\x18\xD9V[\x93\x90\x94`@Q\x97a4;\x89a\x116V[`\x01\x89R` \x89\x01R`\x01`\x01`\xA0\x1B\x03\x8B\x16`@\x89\x01R\x88\x88\x01R\x86\x01R`\xA0\x85\x01R6\x91a\x13\x81V[`\xC0\x82\x01R`@Q\x80\x95\x81\x92\x7F+\xFBx\x0C\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x83R`\x04\x83\x01a8\x10V[\x03\x81_`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x92\x83\x15a\x02\xF1W\x87\x91\x8B\x91_\x95a5-W[P` \x01Q\x15a+\xB0Wa+\xA6\x92\x84a5\x05a+\xAB\x97\x96\x94`\x01`\x01`\xA0\x1B\x03\x94a\x16\x91V[R\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aB\nV[` \x91\x95Pa5J\x90``=``\x11a\x1C\xE0Wa\x1C\xCD\x81\x83a\x11RV[P\x95\x91\x90Pa4\xDFV[o\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF`\x01`\x01`\xA0\x1B\x03` a5\x85\x81\x88\x01Qa5\x7F\x88a&\xEBV[\x90a\x16\x91V[Q\x01Q\x16\x91a)\xFCV[a5\xCC\x85`\x01`\x01`\xA0\x1B\x03` \x84\x01a\x1Cg\x82\x82Q\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aA\xC0V[a)\xE0V[PPa5\xFC\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0a:qV[\x91a6\x07\x83Qa\x19\xD0V[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x91\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x91\x90_[\x86Q\x81\x10\x15a&\xBAW`\x01\x90`\x01`\x01`\xA0\x1B\x03\x80a6m\x83\x8Ba\x16\x91V[Q\x16_R\x85` Ra6\x88`@_ \\\x82a&~\x85\x8Da\x16\x91V[a6\x92\x83\x87a\x16\x91V[Ra6\x9D\x82\x8Aa\x16\x91V[Q\x16_R\x85` R_`@\x81 ]\x01a6NV[G\x80\x15a6\xF0W\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\\a6\xF0W`\x01`\x01`\xA0\x1B\x03a\x18L\x92\x16a@\xE0V[PPV[\x905\x90\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE1\x816\x03\x01\x82\x12\x15a\x02\xFCW\x01\x805\x90g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11a\x02\xFCW` \x01\x91\x81`\x05\x1B6\x03\x83\x13a\x02\xFCWV[\x91\x90\x81\x10\x15a\x16\xA5W`\x05\x1B\x81\x015\x90\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x816\x03\x01\x82\x12\x15a\x02\xFCW\x01\x90V[`@Q\x90`@\x82\x01\x82\x81\x10g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11\x17a\x10\xBDW`@R_` \x83\x82\x81R\x01RV[\x91\x90\x82\x03\x91\x82\x11a\x1A\x0EWV[`\x02\x11\x15a7\xC8WV[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`!`\x04R`$_\xFD[\x90\x81``\x91\x03\x12a\x02\xFCW\x80Q\x91`@` \x83\x01Q\x92\x01Q\x90V[a\x01\0`\xC0a\x0F|\x93` \x84R\x80Qa8(\x81a7\xBEV[` \x85\x01R` \x81\x01Q`\x01`\x01`\xA0\x1B\x03\x80\x91\x16`@\x86\x01R\x80`@\x83\x01Q\x16``\x86\x01R``\x82\x01Q\x16`\x80\x85\x01R`\x80\x81\x01Q`\xA0\x85\x01R`\xA0\x81\x01Q\x82\x85\x01R\x01Q\x91`\xE0\x80\x82\x01R\x01\x90a\x0F\xFBV[\x90\x91``\x82\x84\x03\x12a\x02\xFCW\x81Q\x91g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x92\x83\x81\x11a\x02\xFCW\x84a8\xA8\x91\x83\x01a\x15sV[\x93` \x82\x01Q\x93`@\x83\x01Q\x90\x81\x11a\x02\xFCWa\x0F|\x92\x01a\x15\x07V[\x90\x81Q\x80\x82R` \x80\x80\x93\x01\x93\x01\x91_[\x82\x81\x10a8\xE4WPPPP\x90V[\x83Q\x85R\x93\x81\x01\x93\x92\x81\x01\x92`\x01\x01a8\xD6V[` \x81R`\x01`\x01`\xA0\x1B\x03\x80\x83Q\x16` \x83\x01R` \x83\x01Q\x16`@\x82\x01Ra91`@\x83\x01Q`\xC0``\x84\x01R`\xE0\x83\x01\x90a8\xC5V[\x90``\x83\x01Q`\x80\x82\x01R`\x80\x83\x01Q`\x05\x81\x10\x15a7\xC8Wa\x0F|\x93`\xA0\x91\x82\x84\x01R\x01Q\x90`\xC0`\x1F\x19\x82\x85\x03\x01\x91\x01Ra\x0F\xFBV[\x91``\x83\x83\x03\x12a\x02\xFCW\x82Q\x92` \x81\x01Q\x92g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x93\x84\x81\x11a\x02\xFCW\x81a9\x9A\x91\x84\x01a\x15sV[\x93`@\x83\x01Q\x90\x81\x11a\x02\xFCWa\x0F|\x92\x01a\x15\x07V[` \x81R`\x01`\x01`\xA0\x1B\x03\x80\x83Q\x16` \x83\x01R` \x83\x01Q\x16`@\x82\x01R`@\x82\x01Q``\x82\x01Ra9\xF4``\x83\x01Q`\xC0`\x80\x84\x01R`\xE0\x83\x01\x90a8\xC5V[\x90`\x80\x83\x01Q`\x04\x81\x10\x15a7\xC8Wa\x0F|\x93`\xA0\x91\x82\x84\x01R\x01Q\x90`\xC0`\x1F\x19\x82\x85\x03\x01\x91\x01Ra\x0F\xFBV[\x91\x90\x91`\x80\x80`\xA0\x83\x01\x94\x80Qa:8\x81a7\xBEV[\x84R` \x81\x01Qa:H\x81a7\xBEV[` \x85\x01R`\x01`\x01`\xA0\x1B\x03`@\x82\x01Q\x16`@\x85\x01R``\x81\x01Q``\x85\x01R\x01Q\x91\x01RV[\x90\x81\\a:}\x81a\x11uV[a:\x8A`@Q\x91\x82a\x11RV[\x81\x81Ra:\x96\x82a\x11uV[`\x1F\x19` \x91\x016` \x84\x017\x81\x94_[\x84\x81\x10a:\xB5WPPPPPV[`\x01\x90\x82_R\x80\x84_ \x01\\`\x01`\x01`\xA0\x1B\x03a:\xD3\x83\x88a\x16\x91V[\x91\x16\x90R\x01a:\xA7V[\x91\x92\x80a=\xD8W[\x15a<QWPP\x80G\x10a<)W`\x01`\x01`\xA0\x1B\x03\x80\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x91\x82;\x15a\x02\xFCW`@Q\x90\x7F\xD0\xE3\r\xB0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x82R_\x91_\x81`\x04\x81\x85\x89Z\xF1\x80\x15a\x02\xF1Wa<\x12W[P`D` \x92\x93\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x94a;\x98\x83\x87\x83aCVV[\x84`@Q\x96\x87\x94\x85\x93\x7F\x15\xAF\xD4\t\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x85R`\x04\x85\x01R`$\x84\x01RZ\xF1\x90\x81\x15a<\x06WPa;\xDFWPV[` \x90\x81=\x83\x11a;\xFFW[a;\xF5\x81\x83a\x11RV[\x81\x01\x03\x12a\x02\xFCWV[P=a;\xEBV[`@Q\x90=\x90\x82>=\x90\xFD[` \x92Pa<\x1F\x90a\x11\x06V[`D_\x92Pa;cV[\x7F\xA0\x1A\x9D\xF6\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x04_\xFD[\x90\x91_\x90\x80a<aW[PPPPV[`\x01`\x01`\xA0\x1B\x03\x93\x84\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x94\x80\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x91a<\xBB\x84aB\xFDV[\x96\x80;\x15a\x02\xFCW`@Q\x7F6\xC7\x85\x16\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x92\x83\x16`\x04\x82\x01R\x84\x83\x16`$\x82\x01R\x97\x82\x16`D\x89\x01R\x91\x86\x16\x16`d\x87\x01R_\x90\x86\x90`\x84\x90\x82\x90\x84\x90Z\xF1\x94\x85\x15a\x02\xF1Wa=\x80\x95a=\xC4W[P\x82\x93` \x93`@Q\x80\x97\x81\x95\x82\x94\x7F\x15\xAF\xD4\t\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x84R`\x04\x84\x01` \x90\x93\x92\x91\x93`\x01`\x01`\xA0\x1B\x03`@\x82\x01\x95\x16\x81R\x01RV[\x03\x92Z\xF1\x90\x81\x15a<\x06WPa=\x99W[\x80\x80\x80a<[V[` \x90\x81=\x83\x11a=\xBDW[a=\xAF\x81\x83a\x11RV[\x81\x01\x03\x12a\x02\xFCW_a=\x91V[P=a=\xA5V[` \x93Pa=\xD1\x90a\x11\x06V[_\x92a=/V[P`\x01`\x01`\xA0\x1B\x03\x80\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x90\x82\x16\x14a:\xE5V[`\x01\x81\x01\x91\x80_R` \x91\x83\x83R`@_ \\\x80\x15\x15_\x14a>\xA7W_\x19\x90\x81\x81\x01\x83\\\x83\x80\x82\x01\x91\x82\x84\x03a>jW[PPPPP\x81\\\x81\x81\x01\x92\x81\x84\x11a\x1A\x0EW_\x93\x81]\x83R\x84\x83 \x01\x01]_RR_`@\x81 ]`\x01\x90V[a>wa>\x87\x93\x88aD:V[\x86_R\x88_ \x01\x01\\\x91\x85aD:V[\x83_R\x80\x83\x83\x88_ \x01\x01]_R\x85\x85R`@_ ]_\x80\x80\x83\x81a>>V[PPPPP_\x90V[_\x94\x93\x83\x15a@\xD8W\x80a@\xA3W[\x15a@\x07W`\x01`\x01`\xA0\x1B\x03\x91\x82\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x80;\x15a\x02\xFCW`@Q\x7F\xAEc\x93)\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x92\x90\x92\x16`\x04\x83\x01R0`$\x83\x01R`D\x82\x01\x85\x90R_\x90\x82\x90`d\x90\x82\x90\x84\x90Z\xF1\x80\x15a\x02\xF1Wa?\xF4W[P\x84\x82\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x80;\x15a?\xF0W\x81\x90`$`@Q\x80\x94\x81\x93\x7F.\x1A}M\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x83R\x89`\x04\x84\x01RZ\xF1\x80\x15a?\xE5Wa?\xCDW[Pa\x18L\x93\x94P\x16a@\xE0V[a?\xD7\x86\x91a\x11\x06V[a?\xE1W\x84a?\xC0V[\x84\x80\xFD[`@Q=\x88\x82>=\x90\xFD[P\x80\xFD[a?\xFF\x91\x95Pa\x11\x06V[_\x93_a?SV[\x92\x93P\x90`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x80;\x15a\x02\xFCW`@Q\x7F\xAEc\x93)\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x93\x84\x16`\x04\x82\x01R\x93\x90\x92\x16`$\x84\x01R`D\x83\x01R_\x90\x82\x90`d\x90\x82\x90\x84\x90Z\xF1\x80\x15a\x02\xF1Wa@\x9AWPV[a\x18L\x90a\x11\x06V[P`\x01`\x01`\xA0\x1B\x03\x80\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x90\x83\x16\x14a>\xBFV[PPPP\x90PV[\x81G\x10aA0W_\x80\x80\x93`\x01`\x01`\xA0\x1B\x03\x82\x94\x16Z\xF1aA\0a(wV[P\x15aA\x08WV[\x7F\x14%\xEAB\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x04_\xFD[\x7F\xCDx`Y\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R0`\x04R`$_\xFD[\x90aAqWP\x80Q\x15aA\x08W\x80Q\x90` \x01\xFD[\x81Q\x15\x80aA\xB7W[aA\x82WP\x90V[`\x01`\x01`\xA0\x1B\x03\x90\x7F\x99\x96\xB3\x15\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R\x16`\x04R`$_\xFD[P\x80;\x15aAzV[`\x01\x81\x01\x90\x82_R\x81` R`@_ \\\x15_\x14aB\x03W\x80\\\x81_R\x83\x81` _ \x01]`\x01\x81\x01\x80\x91\x11a\x1A\x0EW\x81]\\\x91_R` R`@_ ]`\x01\x90V[PPP_\x90V[\x90_R` RaB\x1F`@_ \x91\x82\\a\x1A\x01V[\x90]V[\x91`D\x92\x93\x91\x93`\x01`\x01`\xA0\x1B\x03`@\x94\x85\x92\x82\x80\x85Q\x99\x8A\x95\x86\x94\x7F\xC9\xC1f\x1B\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x86R\x16`\x04\x85\x01R\x16`$\x83\x01R\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xFA\x93\x84\x15aB\xF3W_\x93_\x95aB\xBCW[PPaB\xB9aB\xB2\x85\x94a\x19\xD0V[\x94\x85a\x16\x91V[RV[\x80\x92\x95P\x81\x94P=\x83\x11aB\xECW[aB\xD5\x81\x83a\x11RV[\x81\x01\x03\x12a\x02\xFCW` \x82Q\x92\x01Q\x92_\x80aB\xA3V[P=aB\xCBV[\x83Q=_\x82>=\x90\xFD[`\x01`\x01`\xA0\x1B\x03\x90\x81\x81\x11aC\x11W\x16\x90V[\x7Fm\xFC\xC6P\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\xA0`\x04R`$R`D_\xFD[\x90_R` RaB\x1F`@_ \x91\x82\\a7\xB1V[`@Q\x92` \x84\x01\x90\x7F\xA9\x05\x9C\xBB\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x82R`\x01`\x01`\xA0\x1B\x03\x80\x94\x16`$\x86\x01R`D\x85\x01R`D\x84R`\x80\x84\x01\x90\x84\x82\x10g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x83\x11\x17a\x10\xBDWaC\xD5\x93_\x93\x84\x93`@R\x16\x94Q\x90\x82\x86Z\xF1aC\xCEa(wV[\x90\x83aA\\V[\x80Q\x90\x81\x15\x15\x91\x82aD\x16W[PPaC\xEBWPV[\x7FRt\xAF\xE7\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x04R`$_\xFD[\x81\x92P\x90` \x91\x81\x01\x03\x12a\x02\xFCW` \x01Q\x80\x15\x90\x81\x15\x03a\x02\xFCW_\x80aC\xE2V[\\\x11\x15aDCWV[\x7F\x0FJ\xE0\xE4\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x04_\xFD\xFE\xA2dipfsX\"\x12 \"\x9A\\\xF8\x9A\xA7\xC2\xD0\xA4\xB4\xD5\xDB \xBB\xA6\xC2\xB3\xA7K\x08\x03\x03\xFCn\xC0\x0B\xA5\x82\xA5\xDC\xF7QdsolcC\0\x08\x1A\x003",
    );
    /// The runtime bytecode of the contract, as deployed on the network.
    ///
    /// ```text
    ///0x60806040526004361015610072575b3615610018575f80fd5b6001600160a01b037f000000000000000000000000000000000000000000000000000000000000000016330361004a57005b7f0540ddf6000000000000000000000000000000000000000000000000000000005f5260045ffd5b5f3560e01c806308a465f614610e9d57806319c6989f1461084e578063286f580d146107b75780632950286e146106cc57806354fd4d501461058f5780635a3c3987146105665780635e01eb5a146105215780638a12a08c146104c65780638eb1b65e146103bf578063945ed33f14610344578063ac9650d8146103005763e3b5dff40361000e57346102fc576060806003193601126102fc5767ffffffffffffffff6004358181116102fc5761012d9036906004016112c4565b6101356111a1565b6044359283116102fc57610150610158933690600401610fcd565b9390916128b9565b905f5b835181101561017c57805f8761017360019488611691565b5101520161015b565b506101f06101fe610239946101b65f94886040519361019a8561111a565b30855260208501525f1960408501528660608501523691611381565b60808201526040519283917f8a12a08c0000000000000000000000000000000000000000000000000000000060208401526024830161143e565b03601f198101835282611152565b604051809481927fedfa3568000000000000000000000000000000000000000000000000000000008352602060048401526024830190610ffb565b0381836001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af19182156102f1576102a39261028e915f916102cf575b50602080825183010191016115d4565b909391926102a7575b60405193849384610f2f565b0390f35b5f7f00000000000000000000000000000000000000000000000000000000000000005d610297565b6102eb91503d805f833e6102e38183611152565b81019061154d565b8461027e565b6040513d5f823e3d90fd5b5f80fd5b60206003193601126102fc5760043567ffffffffffffffff81116102fc576103386103326102a3923690600401610f9c565b9061179b565b60405191829182611020565b346102fc5761035236610eca565b61035a611945565b610362611972565b6103906102a3610371836128fb565b9193909461038a606061038383611344565b9201611358565b90612729565b5f7f9b779b17422d0df92223018b32b4d1fa46e071723d6817e2486d003becc55f005d60405193849384610f2f565b60806003193601126102fc5767ffffffffffffffff6004358181116102fc576103ec9036906004016112c4565b906103f56111b7565b906064359081116102fc576101f061048b6102399461045161041c5f953690600401610fcd565b610425336128b9565b97604051946104338661111a565b33865260208601526024356040860152151560608501523691611381565b60808201526040519283917f945ed33f000000000000000000000000000000000000000000000000000000006020840152602483016116d2565b604051809481927f48c89491000000000000000000000000000000000000000000000000000000008352602060048401526024830190610ffb565b346102fc576102a36104ef6104da36610eca565b6104e2611945565b6104ea611972565b611a3b565b5f7f9b779b17422d0df92223018b32b4d1fa46e071723d6817e2486d003becc55f009492945d60405193849384610f2f565b346102fc575f6003193601126102fc5760207f00000000000000000000000000000000000000000000000000000000000000005c6001600160a01b0360405191168152f35b346102fc576102a36104ef61057a36610eca565b610582611945565b61058a611972565b6128fb565b346102fc575f6003193601126102fc576040515f80549060018260011c91600184169182156106c2575b60209485851084146106955785879486865291825f146106575750506001146105fe575b506105ea92500383611152565b6102a3604051928284938452830190610ffb565b5f808052859250907f290decd9548b62a8d60345a988386fc84ba6bc95484008f6362f93160ef3e5635b85831061063f5750506105ea9350820101856105dd565b80548389018501528794508693909201918101610628565b7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff0016858201526105ea95151560051b85010192508791506105dd9050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52602260045260245ffd5b92607f16926105b9565b346102fc5760606003193601126102fc5767ffffffffffffffff6004358181116102fc576106fe9036906004016112c4565b906107076111a1565b6044359182116102fc5761072261072a923690600401610fcd565b9290916128b9565b905f5b845181101561075f57806fffffffffffffffffffffffffffffffff604061075660019489611691565b5101520161072d565b506101f06101fe8561077d5f94610239976040519361019a8561111a565b60808201526040519283917f5a3c3987000000000000000000000000000000000000000000000000000000006020840152602483016116d2565b60806003193601126102fc5767ffffffffffffffff6004358181116102fc576107e49036906004016112c4565b906107ed6111b7565b906064359081116102fc576101f061048b6102399461081461041c5f953690600401610fcd565b60808201526040519283917f08a465f60000000000000000000000000000000000000000000000000000000060208401526024830161143e565b60a06003193601126102fc5767ffffffffffffffff600435116102fc573660236004350112156102fc5767ffffffffffffffff60043560040135116102fc5736602460c060043560040135026004350101116102fc5760243567ffffffffffffffff81116102fc576108c4903690600401610f9c565b67ffffffffffffffff604435116102fc576060600319604435360301126102fc5760643567ffffffffffffffff81116102fc57610905903690600401610fcd565b60843567ffffffffffffffff81116102fc57610925903690600401610f9c565b949093610930611945565b806004356004013503610e75575f5b600435600401358110610bd25750505060443560040135907fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffdd6044353603018212156102fc57816044350160048101359067ffffffffffffffff82116102fc5760248260071b36039101136102fc576109e3575b6102a361033886865f7f9b779b17422d0df92223018b32b4d1fa46e071723d6817e2486d003becc55f005d61179b565b6001600160a01b039492947f0000000000000000000000000000000000000000000000000000000000000000163b156102fc57604051947f2a2d80d10000000000000000000000000000000000000000000000000000000086523360048701526060602487015260c486019260443501602481019367ffffffffffffffff6004830135116102fc57600482013560071b360385136102fc5760606064890152600482013590529192869260e484019291905f905b60048101358210610b5457505050602091601f19601f865f9787956001600160a01b03610ac860246044350161118d565b16608488015260448035013560a48801526003198787030160448801528186528786013787868286010152011601030181836001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af19182156102f1576102a39361033893610b45575b8294508193506109b3565b610b4e90611106565b84610b3a565b9195945091926001600160a01b03610b6b8761118d565b168152602080870135916001600160a01b0383168093036102fc57600492600192820152610b9b604089016128a6565b65ffffffffffff8091166040830152610bb660608a016128a6565b1660608201526080809101970193019050889495939291610a97565b610be7610be082848661192a565b3691611381565b604051610bf3816110a1565b5f81526020915f838301525f60408301528281015190606060408201519101515f1a91835283830152604082015260c07fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffdc81850260043501360301126102fc5760405190610c60826110ea565b610c73602460c08602600435010161118d565b808352610c89604460c08702600435010161118d565b908185850152610ca2606460c08802600435010161118d565b60408581019190915260043560c08802016084810135606087015260a4810135608087015260c4013560a086015283015183519386015160ff91909116926001600160a01b0383163b156102fc575f6001600160a01b03809460e4948b98849860c460c06040519c8d9b8c9a7fd505accf000000000000000000000000000000000000000000000000000000008c521660048b01523060248b0152608482820260043501013560448b0152026004350101356064880152608487015260a486015260c4850152165af19081610e66575b50610e5c57610d7f612877565b906001600160a01b0381511690836001600160a01b0381830151166044604051809581937fdd62ed3e00000000000000000000000000000000000000000000000000000000835260048301523060248301525afa9182156102f1575f92610e2c575b506060015103610df75750506001905b0161093f565b805115610e045780519101fd5b7fa7285689000000000000000000000000000000000000000000000000000000005f5260045ffd5b9091508381813d8311610e55575b610e448183611152565b810103126102fc5751906060610de1565b503d610e3a565b5050600190610df1565b610e6f90611106565b8a610d72565b7faaad13f7000000000000000000000000000000000000000000000000000000005f5260045ffd5b346102fc57610eab36610eca565b610eb3611945565b610ebb611972565b6103906102a361037183611a3b565b600319906020828201126102fc576004359167ffffffffffffffff83116102fc578260a0920301126102fc5760040190565b9081518082526020808093019301915f5b828110610f1b575050505090565b835185529381019392810192600101610f0d565b939290610f4490606086526060860190610efc565b936020948181036020830152602080855192838152019401905f5b818110610f7f57505050610f7c9394506040818403910152610efc565b90565b82516001600160a01b031686529487019491870191600101610f5f565b9181601f840112156102fc5782359167ffffffffffffffff83116102fc576020808501948460051b0101116102fc57565b9181601f840112156102fc5782359167ffffffffffffffff83116102fc57602083818601950101116102fc57565b90601f19601f602080948051918291828752018686015e5f8582860101520116010190565b6020808201906020835283518092526040830192602060408460051b8301019501935f915b8483106110555750505050505090565b9091929394958480611091837fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffc086600196030187528a51610ffb565b9801930193019194939290611045565b6060810190811067ffffffffffffffff8211176110bd57604052565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52604160045260245ffd5b60c0810190811067ffffffffffffffff8211176110bd57604052565b67ffffffffffffffff81116110bd57604052565b60a0810190811067ffffffffffffffff8211176110bd57604052565b60e0810190811067ffffffffffffffff8211176110bd57604052565b90601f601f19910116810190811067ffffffffffffffff8211176110bd57604052565b67ffffffffffffffff81116110bd5760051b60200190565b35906001600160a01b03821682036102fc57565b602435906001600160a01b03821682036102fc57565b6044359081151582036102fc57565b9190916080818403126102fc57604090815191608083019467ffffffffffffffff95848110878211176110bd57825283956112008461118d565b8552602090818501359081116102fc57840182601f820112156102fc5780359061122982611175565b9361123686519586611152565b82855283850190846060809502840101928184116102fc578501915b8383106112745750505050508401528181013590830152606090810135910152565b84838303126102fc57875190611289826110a1565b6112928461118d565b825261129f87850161118d565b87830152888401359081151582036102fc578288928b89950152815201920191611252565b81601f820112156102fc578035916020916112de84611175565b936112ec6040519586611152565b808552838086019160051b830101928084116102fc57848301915b8483106113175750505050505090565b823567ffffffffffffffff81116102fc578691611339848480948901016111c6565b815201920191611307565b356001600160a01b03811681036102fc5790565b3580151581036102fc5790565b67ffffffffffffffff81116110bd57601f01601f191660200190565b92919261138d82611365565b9161139b6040519384611152565b8294818452818301116102fc578281602093845f960137010152565b9060808101916001600160a01b03808251168352602093848301519460808186015285518092528060a086019601925f905b83821061140b5750505050506060816040829301516040850152015191015290565b845180518216895280840151821689850152604090810151151590890152606090970196938201936001909101906113e9565b91909160209081815260c08101916001600160a01b0385511681830152808501519260a06040840152835180915260e08301918060e08360051b8601019501925f905b8382106114bd5750505050506080846040610f7c959601516060840152606081015115158284015201519060a0601f1982850301910152610ffb565b909192939583806114f8837fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff208a600196030186528a516113b7565b98019201920190939291611481565b81601f820112156102fc5780519061151e82611365565b9261152c6040519485611152565b828452602083830101116102fc57815f9260208093018386015e8301015290565b906020828203126102fc57815167ffffffffffffffff81116102fc57610f7c9201611507565b9080601f830112156102fc5781519060209161158e81611175565b9361159c6040519586611152565b81855260208086019260051b8201019283116102fc57602001905b8282106115c5575050505090565b815181529083019083016115b7565b90916060828403126102fc5781519167ffffffffffffffff928381116102fc5784611600918301611573565b936020808301518581116102fc5783019082601f830112156102fc5781519161162883611175565b926116366040519485611152565b808452828085019160051b830101918583116102fc578301905b82821061167257505050509360408301519081116102fc57610f7c9201611573565b81516001600160a01b03811681036102fc578152908301908301611650565b80518210156116a55760209160051b010190565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52603260045260245ffd5b91909160209081815260c08101916001600160a01b0385511681830152808501519260a06040840152835180915260e08301918060e08360051b8601019501925f905b8382106117515750505050506080846040610f7c959601516060840152606081015115158284015201519060a0601f1982850301910152610ffb565b9091929395838061178c837fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff208a600196030186528a516113b7565b98019201920190939291611715565b91906117a6336128b9565b907f000000000000000000000000000000000000000000000000000000000000000093845c6118b1576001906001865d6117df83611175565b926117ed6040519485611152565b808452601f196117fc82611175565b015f5b8181106118a05750505f5b8181106118575750505050905f61184c92945d7f0000000000000000000000000000000000000000000000000000000000000000805c9161184e575b506136b1565b565b5f905d5f611846565b806118845f8061186c610be08996888a61192a565b602081519101305af461187d612877565b903061415c565b61188e8288611691565b526118998187611691565b500161180a565b8060606020809389010152016117ff565b7f3ee5aeb5000000000000000000000000000000000000000000000000000000005f5260045ffd5b9035907fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe1813603018212156102fc570180359067ffffffffffffffff82116102fc576020019181360383136102fc57565b908210156116a5576119419160051b8101906118d9565b9091565b7f9b779b17422d0df92223018b32b4d1fa46e071723d6817e2486d003becc55f00805c6118b1576001905d565b6001600160a01b037f00000000000000000000000000000000000000000000000000000000000000001633036119a457565b7f089676d5000000000000000000000000000000000000000000000000000000005f523360045260245ffd5b906119da82611175565b6119e76040519182611152565b828152601f196119f78294611175565b0190602036910137565b91908201809211611a0e57565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52601160045260245ffd5b604081013542116126c35790611a5e611a5760208401846136f4565b90506119d0565b915f5b611a6e60208301836136f4565b90508110156125c757611a9881611a93611a8b60208601866136f4565b369391613748565b6111c6565b936040850151936001600160a01b038651169060208701518051156116a55760200151604001511515806125be575b1561256357611aec611ad886611344565b8784611ae660608a01611358565b92613add565b5f5b60208801515181101561255357611b03613788565b6020890151515f198101908111611a0e578214806020830152821582525f1461254c576060890151905b611b3b8360208c0151611691565b51604081015190919015611cee57611bd36001600160a01b03835116936001600160a01b03881685145f14611ce7576001945b60405195611b7b8761111a565b5f8752611b87816137be565b6020870152604086015260609485918d838301526080820152604051809381927f43583be500000000000000000000000000000000000000000000000000000000835260048301613a22565b03815f6001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af19384156102f1575f94611cb0575b50506020015115611c9657816001600160a01b036020611c909360019695611c388c8c611691565b5201611c67828251167f00000000000000000000000000000000000000000000000000000000000000006141c0565b5051167f000000000000000000000000000000000000000000000000000000000000000061420a565b01611aee565b602001519097506001600160a01b03169250600190611c90565b60209294509081611cd592903d10611ce0575b611ccd8183611152565b8101906137f5565b91505092905f611c10565b503d611cc3565b5f94611b6e565b888a6001600160a01b038495945116806001600160a01b038a16145f14612132575050815115905061206e57888a80151580612053575b611f4d575b6001600160a01b03939291611ddd82611e15978b5f95897f0000000000000000000000000000000000000000000000000000000000000000921680885282602052604088205c611f3c575b5050505b6001611d9c8983511660208401998b8b51169080158a14611f3657508391614223565b999092511694611db1608091828101906118d9565b93909460405197611dc1896110ea565b8852306020890152604088015260608701528501523691611381565b60a0820152604051809681927f21457897000000000000000000000000000000000000000000000000000000008352600483016139b1565b0381836001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af19384156102f1575f94611f0c575b506020015115611ee95791611ebc826001600160a01b0360019695611e7a611ee49686611691565b51611e858d8d611691565b52611eb3828251167f00000000000000000000000000000000000000000000000000000000000000006141c0565b50511692611691565b51907f000000000000000000000000000000000000000000000000000000000000000061420a565b611c90565b98506001929450611f02906001600160a01b0392611691565b5197511692611c90565b6020919450611f2c903d805f833e611f248183611152565b810190613969565b5094919050611e52565b91614223565b611f4592614341565b5f8281611d75565b50611f5a90929192611344565b91611f648b6142fd565b6001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000163b156102fc576040517f36c785160000000000000000000000000000000000000000000000000000000081526001600160a01b039485166004820152306024820152908416604482015292871660648401525f8380608481010381836001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af180156102f1578a611ddd8d611e15976001600160a01b03975f95612044575b50975092505091929350611d2a565b61204d90611106565b5f612035565b5061205d82611344565b6001600160a01b0316301415611d25565b906001600160a01b037f000000000000000000000000000000000000000000000000000000000000000016916001600160a01b0384511692803b156102fc576040517fae6393290000000000000000000000000000000000000000000000000000000081526001600160a01b03949094166004850152306024850152604484018c90525f908490606490829084905af180156102f1578a611ddd8d611e15976001600160a01b03975f95612123575b50611d79565b61212c90611106565b5f61211d565b6001600160a01b0360208796949701511690898183145f146123d7576121cd925061220597915060016121735f96956001600160a01b0393848b5116614223565b509282895116956020890151151588146123ae5761219082611344565b945b6121a1608093848101906118d9565b959096604051996121b18b6110ea565b8a52166020890152604088015260608701528501523691611381565b60a0820152604051809581927f4af29ec4000000000000000000000000000000000000000000000000000000008352600483016138f8565b0381836001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af19283156102f1575f93612384575b5060200151156122c357816001600160a01b036020611ee493600196956122698c8c611691565b526122998383830151167f00000000000000000000000000000000000000000000000000000000000000006141c0565b500151167f000000000000000000000000000000000000000000000000000000000000000061420a565b60208181015191516040517f15afd4090000000000000000000000000000000000000000000000000000000081526001600160a01b03918216600482015260248101859052939a50909116945081806044810103815f6001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af180156102f157612359575b50600190611c90565b602090813d831161237d575b61236f8183611152565b810103126102fc575f612350565b503d612365565b60209193506123a4903d805f833e61239c8183611152565b81019061387c565b5093919050612242565b837f00000000000000000000000000000000000000000000000000000000000000001694612192565b6001600160a01b036124669561242e9394956123f860809b8c8101906118d9565b9390946040519761240889611136565b5f8952602089015216604087015260609a8b978888015286015260a08501523691611381565b60c0820152604051809381927f2bfb780c00000000000000000000000000000000000000000000000000000000835260048301613810565b03815f6001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af19384156102f1575f94612525575b50506020015115611c9657816001600160a01b036020611ee493600196956124cb8c8c611691565b526124fb8383830151167f00000000000000000000000000000000000000000000000000000000000000006141c0565b500151167f000000000000000000000000000000000000000000000000000000000000000061420a565b6020929450908161254192903d10611ce057611ccd8183611152565b91505092905f6124a3565b5f90611b2d565b5091955090935050600101611a61565b61258d827f00000000000000000000000000000000000000000000000000000000000000006141c0565b506125b986837f000000000000000000000000000000000000000000000000000000000000000061420a565b611aec565b50321515611ac7565b50506125f27f0000000000000000000000000000000000000000000000000000000000000000613a71565b916125fd83516119d0565b7f0000000000000000000000000000000000000000000000000000000000000000917f000000000000000000000000000000000000000000000000000000000000000091905f5b86518110156126ba576001906001600160a01b0380612663838b611691565b51165f528560205261269160405f205c8261267e858d611691565b51165f528860205260405f205c90611a01565b61269b8387611691565b526126a6828a611691565b51165f52856020525f604081205d01612644565b50949391509150565b7fe08b8af0000000000000000000000000000000000000000000000000000000005f5260045ffd5b905f198201918213600116611a0e57565b7f80000000000000000000000000000000000000000000000000000000000000008114611a0e575f190190565b907f000000000000000000000000000000000000000000000000000000000000000090815c7f0000000000000000000000000000000000000000000000000000000000000000612779815c6126eb565b907f0000000000000000000000000000000000000000000000000000000000000000915b5f81121561283a575050506127b1906126eb565b917f0000000000000000000000000000000000000000000000000000000000000000925b5f8112156127ea575050505061184c906136b1565b61283590825f5261282f60205f83828220015c91828252888152886040916128228a8d8587205c906001600160a01b03891690613eb0565b8484525281205d84613e0d565b506126fc565b6127d5565b61287290825f5261282f60205f8a8785848420015c938484528181526128228c6040948587205c906001600160a01b03891690613add565b61279d565b3d156128a1573d9061288882611365565b916128966040519384611152565b82523d5f602084013e565b606090565b359065ffffffffffff821682036102fc57565b905f917f00000000000000000000000000000000000000000000000000000000000000006001600160a01b03815c16156128f1575050565b909192505d600190565b90604082013542116126c357612917611a5760208401846136f4565b915f5b61292760208301836136f4565b90508110156135d15761294481611a93611a8b60208601866136f4565b60608101519061297e6001600160a01b038251167f00000000000000000000000000000000000000000000000000000000000000006141c0565b506020810151515f198101908111611a0e575b5f8112156129a45750505060010161291a565b6129b2816020840151611691565b516129bb613788565b9082156020830152602084015151805f19810111611a0e575f1901831480835261358f575b6020820151156135545760408401516001600160a01b03855116915b604081015115612c1d5783916001600160a01b036060926020612aa0970151151580612c14575b612bed575b5116906001600160a01b0385168203612be6576001915b60405192612a4c8461111a565b60018452612a59816137be565b6020840152604083015288838301526080820152604051809581927f43583be500000000000000000000000000000000000000000000000000000000835260048301613a22565b03815f6001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af19283156102f15787918b915f95612bbf575b506020015115612bb057612ba69284612b02612bab979694612b7594611691565b52612b366001600160a01b0382167f00000000000000000000000000000000000000000000000000000000000000006141c0565b506001600160a01b03612b4d8460408a01516137b1565b91167f000000000000000000000000000000000000000000000000000000000000000061420a565b6001600160a01b038551167f000000000000000000000000000000000000000000000000000000000000000061420a565b6126fc565b612991565b505050612bab919350926126fc565b6020919550612bdc9060603d606011611ce057611ccd8183611152565b5095919050612ae1565b5f91612a3f565b612c0f612bf98d611344565b8d8b611ae6886040888451169301519301611358565b612a28565b50321515612a23565b906001600160a01b03825116806001600160a01b038516145f14613137575060208401516130495750604051927f967870920000000000000000000000000000000000000000000000000000000084526001600160a01b03831660048501526020846024816001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165afa9384156102f1575f94613015575b5083916001600160a01b038151166001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000163b156102fc576040517fae6393290000000000000000000000000000000000000000000000000000000081526001600160a01b03909116600482015230602482015260448101959095525f8580606481010381836001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af19081156102f157612dec955f92613006575b505b611ddd6001600160a01b03612da88b828551168360208701511690614223565b50925116918c6002612dbf608092838101906118d9565b92909360405196612dcf886110ea565b875230602088015289604088015260608701528501523691611381565b0381836001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af19384156102f1575f94612fe3575b506020015115612ecf57908291612bab9493612e45898d611691565b52612e7a836001600160a01b0384167f000000000000000000000000000000000000000000000000000000000000000061420a565b80831080612eb4575b612e90575b5050506126fc565b612ea6612eac93612ea08b611344565b926137b1565b91614356565b5f8080612e88565b50306001600160a01b03612ec78b611344565b161415612e83565b9450908094808210612ee8575b505050612bab906126fc565b91612ef8602092612f77946137b1565b90612f2d826001600160a01b037f00000000000000000000000000000000000000000000000000000000000000001683614356565b60405193849283927f15afd40900000000000000000000000000000000000000000000000000000000845260048401602090939291936001600160a01b0360408201951681520152565b03815f6001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af180156102f157612fb8575b8080612edc565b602090813d8311612fdc575b612fce8183611152565b810103126102fc575f612fb1565b503d612fc4565b6020919450612ffb903d805f833e611f248183611152565b509094919050612e29565b61300f90611106565b5f612d86565b9093506020813d602011613041575b8161303160209383611152565b810103126102fc5751925f612cbc565b3d9150613024565b909261305489611344565b6001600160a01b033091160361306f575b5f612dec94612d88565b6001600160a01b037f000000000000000000000000000000000000000000000000000000000000000016936130a38a611344565b6130ac846142fd565b90863b156102fc576040517f36c785160000000000000000000000000000000000000000000000000000000081526001600160a01b039182166004820152306024820152918116604483015285166064820152945f908690608490829084905af19081156102f157612dec955f92613128575b50945050613065565b61313190611106565b5f61311f565b6001600160a01b036020849695940151168a8282145f1461340b5750505061320c61316e5f92846001600160a01b03885116614223565b92906131d48c6001600160a01b03808a5116938951151586146133df576131a361319784611344565b935b60808101906118d9565b929093604051966131b3886110ea565b875216602086015260408501528c6060850152600260808501523691611381565b60a0820152604051809381927f4af29ec4000000000000000000000000000000000000000000000000000000008352600483016138f8565b0381836001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af19081156102f1575f916133c4575b5060208401518c908a90156133aa5783836001600160a01b03936132836132899461327c8f9c9b9a98996132b29a611691565b5192611691565b52611691565b5191167f000000000000000000000000000000000000000000000000000000000000000061420a565b51156132f457612bab92916001600160a01b036020612ba6930151167f0000000000000000000000000000000000000000000000000000000000000000614341565b516040517f15afd4090000000000000000000000000000000000000000000000000000000081526001600160a01b0390911660048201526024810191909152602081806044810103815f6001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af180156102f15761337f575b50612bab906126fc565b602090813d83116133a3575b6133958183611152565b810103126102fc575f613375565b503d61338b565b50509091506133bb92939650611691565b519384916132b2565b6133d891503d805f833e61239c8183611152565b9050613249565b6131a3827f00000000000000000000000000000000000000000000000000000000000000001693613199565b61349e965090613466916060948b61342b608099989993848101906118d9565b9390946040519761343b89611136565b6001895260208901526001600160a01b038b1660408901528888015286015260a08501523691611381565b60c0820152604051809581927f2bfb780c00000000000000000000000000000000000000000000000000000000835260048301613810565b03815f6001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000165af19283156102f15787918b915f9561352d575b506020015115612bb057612ba69284613505612bab9796946001600160a01b0394611691565b52167f000000000000000000000000000000000000000000000000000000000000000061420a565b602091955061354a9060603d606011611ce057611ccd8183611152565b50959190506134df565b6fffffffffffffffffffffffffffffffff6001600160a01b0360206135858188015161357f886126eb565b90611691565b51015116916129fc565b6135cc856001600160a01b0360208401611c67828251167f00000000000000000000000000000000000000000000000000000000000000006141c0565b6129e0565b50506135fc7f0000000000000000000000000000000000000000000000000000000000000000613a71565b9161360783516119d0565b7f0000000000000000000000000000000000000000000000000000000000000000917f000000000000000000000000000000000000000000000000000000000000000091905f5b86518110156126ba576001906001600160a01b038061366d838b611691565b51165f528560205261368860405f205c8261267e858d611691565b6136928387611691565b5261369d828a611691565b51165f52856020525f604081205d0161364e565b4780156136f0577f00000000000000000000000000000000000000000000000000000000000000005c6136f0576001600160a01b0361184c92166140e0565b5050565b9035907fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe1813603018212156102fc570180359067ffffffffffffffff82116102fc57602001918160051b360383136102fc57565b91908110156116a55760051b810135907fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff81813603018212156102fc570190565b604051906040820182811067ffffffffffffffff8211176110bd576040525f6020838281520152565b91908203918211611a0e57565b600211156137c857565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52602160045260245ffd5b908160609103126102fc578051916040602083015192015190565b61010060c0610f7c93602084528051613828816137be565b602085015260208101516001600160a01b0380911660408601528060408301511660608601526060820151166080850152608081015160a085015260a08101518285015201519160e0808201520190610ffb565b90916060828403126102fc5781519167ffffffffffffffff928381116102fc57846138a8918301611573565b9360208201519360408301519081116102fc57610f7c9201611507565b9081518082526020808093019301915f5b8281106138e4575050505090565b8351855293810193928101926001016138d6565b602081526001600160a01b038083511660208301526020830151166040820152613931604083015160c0606084015260e08301906138c5565b9060608301516080820152608083015160058110156137c857610f7c9360a0918284015201519060c0601f1982850301910152610ffb565b916060838303126102fc5782519260208101519267ffffffffffffffff938481116102fc578161399a918401611573565b9360408301519081116102fc57610f7c9201611507565b602081526001600160a01b038083511660208301526020830151166040820152604082015160608201526139f4606083015160c0608084015260e08301906138c5565b90608083015160048110156137c857610f7c9360a0918284015201519060c0601f1982850301910152610ffb565b91909160808060a08301948051613a38816137be565b84526020810151613a48816137be565b60208501526001600160a01b036040820151166040850152606081015160608501520151910152565b90815c613a7d81611175565b613a8a6040519182611152565b818152613a9682611175565b601f196020910136602084013781945f5b848110613ab5575050505050565b600190825f5280845f20015c6001600160a01b03613ad38388611691565b9116905201613aa7565b919280613dd8575b15613c51575050804710613c29576001600160a01b03807f00000000000000000000000000000000000000000000000000000000000000001691823b156102fc57604051907fd0e30db00000000000000000000000000000000000000000000000000000000082525f915f8160048185895af180156102f157613c12575b506044602092937f00000000000000000000000000000000000000000000000000000000000000001694613b98838783614356565b8460405196879485937f15afd409000000000000000000000000000000000000000000000000000000008552600485015260248401525af1908115613c065750613bdf5750565b602090813d8311613bff575b613bf58183611152565b810103126102fc57565b503d613beb565b604051903d90823e3d90fd5b60209250613c1f90611106565b60445f9250613b63565b7fa01a9df6000000000000000000000000000000000000000000000000000000005f5260045ffd5b90915f9080613c61575b50505050565b6001600160a01b0393847f00000000000000000000000000000000000000000000000000000000000000001694807f00000000000000000000000000000000000000000000000000000000000000001691613cbb846142fd565b96803b156102fc576040517f36c785160000000000000000000000000000000000000000000000000000000081526001600160a01b039283166004820152848316602482015297821660448901529186161660648701525f908690608490829084905af19485156102f157613d8095613dc4575b5082936020936040518097819582947f15afd40900000000000000000000000000000000000000000000000000000000845260048401602090939291936001600160a01b0360408201951681520152565b03925af1908115613c065750613d99575b808080613c5b565b602090813d8311613dbd575b613daf8183611152565b810103126102fc575f613d91565b503d613da5565b60209350613dd190611106565b5f92613d2f565b506001600160a01b03807f00000000000000000000000000000000000000000000000000000000000000001690821614613ae5565b6001810191805f5260209183835260405f205c8015155f14613ea7575f1990818101835c8380820191828403613e6a575b5050505050815c81810192818411611a0e575f93815d835284832001015d5f52525f604081205d600190565b613e77613e87938861443a565b865f52885f2001015c918561443a565b835f52808383885f2001015d5f5285855260405f205d5f80808381613e3e565b50505050505f90565b5f949383156140d857806140a3575b15614007576001600160a01b0391827f000000000000000000000000000000000000000000000000000000000000000016803b156102fc576040517fae6393290000000000000000000000000000000000000000000000000000000081526001600160a01b03929092166004830152306024830152604482018590525f908290606490829084905af180156102f157613ff4575b5084827f000000000000000000000000000000000000000000000000000000000000000016803b15613ff05781906024604051809481937f2e1a7d4d0000000000000000000000000000000000000000000000000000000083528960048401525af18015613fe557613fcd575b5061184c939450166140e0565b613fd78691611106565b613fe15784613fc0565b8480fd5b6040513d88823e3d90fd5b5080fd5b613fff919550611106565b5f935f613f53565b929350906001600160a01b037f000000000000000000000000000000000000000000000000000000000000000016803b156102fc576040517fae6393290000000000000000000000000000000000000000000000000000000081526001600160a01b03938416600482015293909216602484015260448301525f908290606490829084905af180156102f15761409a5750565b61184c90611106565b506001600160a01b03807f00000000000000000000000000000000000000000000000000000000000000001690831614613ebf565b505050509050565b814710614130575f8080936001600160a01b038294165af1614100612877565b501561410857565b7f1425ea42000000000000000000000000000000000000000000000000000000005f5260045ffd5b7fcd786059000000000000000000000000000000000000000000000000000000005f523060045260245ffd5b90614171575080511561410857805190602001fd5b815115806141b7575b614182575090565b6001600160a01b03907f9996b315000000000000000000000000000000000000000000000000000000005f521660045260245ffd5b50803b1561417a565b6001810190825f528160205260405f205c155f1461420357805c815f52838160205f20015d60018101809111611a0e57815d5c915f5260205260405f205d600190565b5050505f90565b905f5260205261421f60405f2091825c611a01565b905d565b916044929391936001600160a01b03604094859282808551998a9586947fc9c1661b0000000000000000000000000000000000000000000000000000000086521660048501521660248301527f0000000000000000000000000000000000000000000000000000000000000000165afa9384156142f3575f935f956142bc575b50506142b96142b285946119d0565b9485611691565b52565b809295508194503d83116142ec575b6142d58183611152565b810103126102fc5760208251920151925f806142a3565b503d6142cb565b83513d5f823e3d90fd5b6001600160a01b0390818111614311571690565b7f6dfcc650000000000000000000000000000000000000000000000000000000005f5260a060045260245260445ffd5b905f5260205261421f60405f2091825c6137b1565b6040519260208401907fa9059cbb0000000000000000000000000000000000000000000000000000000082526001600160a01b038094166024860152604485015260448452608084019084821067ffffffffffffffff8311176110bd576143d5935f9384936040521694519082865af16143ce612877565b908361415c565b8051908115159182614416575b50506143eb5750565b7f5274afe7000000000000000000000000000000000000000000000000000000005f5260045260245ffd5b81925090602091810103126102fc57602001518015908115036102fc575f806143e2565b5c111561444357565b7f0f4ae0e4000000000000000000000000000000000000000000000000000000005f5260045ffdfea2646970667358221220229a5cf89aa7c2d0a4b4d5db20bba6c2b3a74b080303fc6ec00ba582a5dcf75164736f6c634300081a0033
    /// ```
    #[rustfmt::skip]
    #[allow(clippy::all)]
    pub static DEPLOYED_BYTECODE: alloy_sol_types::private::Bytes = alloy_sol_types::private::Bytes::from_static(
        b"`\x80`@R`\x046\x10\x15a\0rW[6\x15a\0\x18W_\x80\xFD[`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x163\x03a\0JW\0[\x7F\x05@\xDD\xF6\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x04_\xFD[_5`\xE0\x1C\x80c\x08\xA4e\xF6\x14a\x0E\x9DW\x80c\x19\xC6\x98\x9F\x14a\x08NW\x80c(oX\r\x14a\x07\xB7W\x80c)P(n\x14a\x06\xCCW\x80cT\xFDMP\x14a\x05\x8FW\x80cZ<9\x87\x14a\x05fW\x80c^\x01\xEBZ\x14a\x05!W\x80c\x8A\x12\xA0\x8C\x14a\x04\xC6W\x80c\x8E\xB1\xB6^\x14a\x03\xBFW\x80c\x94^\xD3?\x14a\x03DW\x80c\xAC\x96P\xD8\x14a\x03\0Wc\xE3\xB5\xDF\xF4\x03a\0\x0EW4a\x02\xFCW``\x80`\x03\x196\x01\x12a\x02\xFCWg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF`\x045\x81\x81\x11a\x02\xFCWa\x01-\x906\x90`\x04\x01a\x12\xC4V[a\x015a\x11\xA1V[`D5\x92\x83\x11a\x02\xFCWa\x01Pa\x01X\x936\x90`\x04\x01a\x0F\xCDV[\x93\x90\x91a(\xB9V[\x90_[\x83Q\x81\x10\x15a\x01|W\x80_\x87a\x01s`\x01\x94\x88a\x16\x91V[Q\x01R\x01a\x01[V[Pa\x01\xF0a\x01\xFEa\x029\x94a\x01\xB6_\x94\x88`@Q\x93a\x01\x9A\x85a\x11\x1AV[0\x85R` \x85\x01R_\x19`@\x85\x01R\x86``\x85\x01R6\x91a\x13\x81V[`\x80\x82\x01R`@Q\x92\x83\x91\x7F\x8A\x12\xA0\x8C\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0` \x84\x01R`$\x83\x01a\x14>V[\x03`\x1F\x19\x81\x01\x83R\x82a\x11RV[`@Q\x80\x94\x81\x92\x7F\xED\xFA5h\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x83R` `\x04\x84\x01R`$\x83\x01\x90a\x0F\xFBV[\x03\x81\x83`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x91\x82\x15a\x02\xF1Wa\x02\xA3\x92a\x02\x8E\x91_\x91a\x02\xCFW[P` \x80\x82Q\x83\x01\x01\x91\x01a\x15\xD4V[\x90\x93\x91\x92a\x02\xA7W[`@Q\x93\x84\x93\x84a\x0F/V[\x03\x90\xF3[_\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0]a\x02\x97V[a\x02\xEB\x91P=\x80_\x83>a\x02\xE3\x81\x83a\x11RV[\x81\x01\x90a\x15MV[\x84a\x02~V[`@Q=_\x82>=\x90\xFD[_\x80\xFD[` `\x03\x196\x01\x12a\x02\xFCW`\x045g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11a\x02\xFCWa\x038a\x032a\x02\xA3\x926\x90`\x04\x01a\x0F\x9CV[\x90a\x17\x9BV[`@Q\x91\x82\x91\x82a\x10 V[4a\x02\xFCWa\x03R6a\x0E\xCAV[a\x03Za\x19EV[a\x03ba\x19rV[a\x03\x90a\x02\xA3a\x03q\x83a(\xFBV[\x91\x93\x90\x94a\x03\x8A``a\x03\x83\x83a\x13DV[\x92\x01a\x13XV[\x90a')V[_\x7F\x9Bw\x9B\x17B-\r\xF9\"#\x01\x8B2\xB4\xD1\xFAF\xE0qr=h\x17\xE2Hm\0;\xEC\xC5_\0]`@Q\x93\x84\x93\x84a\x0F/V[`\x80`\x03\x196\x01\x12a\x02\xFCWg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF`\x045\x81\x81\x11a\x02\xFCWa\x03\xEC\x906\x90`\x04\x01a\x12\xC4V[\x90a\x03\xF5a\x11\xB7V[\x90`d5\x90\x81\x11a\x02\xFCWa\x01\xF0a\x04\x8Ba\x029\x94a\x04Qa\x04\x1C_\x956\x90`\x04\x01a\x0F\xCDV[a\x04%3a(\xB9V[\x97`@Q\x94a\x043\x86a\x11\x1AV[3\x86R` \x86\x01R`$5`@\x86\x01R\x15\x15``\x85\x01R6\x91a\x13\x81V[`\x80\x82\x01R`@Q\x92\x83\x91\x7F\x94^\xD3?\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0` \x84\x01R`$\x83\x01a\x16\xD2V[`@Q\x80\x94\x81\x92\x7FH\xC8\x94\x91\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x83R` `\x04\x84\x01R`$\x83\x01\x90a\x0F\xFBV[4a\x02\xFCWa\x02\xA3a\x04\xEFa\x04\xDA6a\x0E\xCAV[a\x04\xE2a\x19EV[a\x04\xEAa\x19rV[a\x1A;V[_\x7F\x9Bw\x9B\x17B-\r\xF9\"#\x01\x8B2\xB4\xD1\xFAF\xE0qr=h\x17\xE2Hm\0;\xEC\xC5_\0\x94\x92\x94]`@Q\x93\x84\x93\x84a\x0F/V[4a\x02\xFCW_`\x03\x196\x01\x12a\x02\xFCW` \x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\\`\x01`\x01`\xA0\x1B\x03`@Q\x91\x16\x81R\xF3[4a\x02\xFCWa\x02\xA3a\x04\xEFa\x05z6a\x0E\xCAV[a\x05\x82a\x19EV[a\x05\x8Aa\x19rV[a(\xFBV[4a\x02\xFCW_`\x03\x196\x01\x12a\x02\xFCW`@Q_\x80T\x90`\x01\x82`\x01\x1C\x91`\x01\x84\x16\x91\x82\x15a\x06\xC2W[` \x94\x85\x85\x10\x84\x14a\x06\x95W\x85\x87\x94\x86\x86R\x91\x82_\x14a\x06WWPP`\x01\x14a\x05\xFEW[Pa\x05\xEA\x92P\x03\x83a\x11RV[a\x02\xA3`@Q\x92\x82\x84\x93\x84R\x83\x01\x90a\x0F\xFBV[_\x80\x80R\x85\x92P\x90\x7F)\r\xEC\xD9T\x8Bb\xA8\xD6\x03E\xA9\x888o\xC8K\xA6\xBC\x95H@\x08\xF66/\x93\x16\x0E\xF3\xE5c[\x85\x83\x10a\x06?WPPa\x05\xEA\x93P\x82\x01\x01\x85a\x05\xDDV[\x80T\x83\x89\x01\x85\x01R\x87\x94P\x86\x93\x90\x92\x01\x91\x81\x01a\x06(V[\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\0\x16\x85\x82\x01Ra\x05\xEA\x95\x15\x15`\x05\x1B\x85\x01\x01\x92P\x87\x91Pa\x05\xDD\x90PV[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\"`\x04R`$_\xFD[\x92`\x7F\x16\x92a\x05\xB9V[4a\x02\xFCW```\x03\x196\x01\x12a\x02\xFCWg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF`\x045\x81\x81\x11a\x02\xFCWa\x06\xFE\x906\x90`\x04\x01a\x12\xC4V[\x90a\x07\x07a\x11\xA1V[`D5\x91\x82\x11a\x02\xFCWa\x07\"a\x07*\x926\x90`\x04\x01a\x0F\xCDV[\x92\x90\x91a(\xB9V[\x90_[\x84Q\x81\x10\x15a\x07_W\x80o\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF`@a\x07V`\x01\x94\x89a\x16\x91V[Q\x01R\x01a\x07-V[Pa\x01\xF0a\x01\xFE\x85a\x07}_\x94a\x029\x97`@Q\x93a\x01\x9A\x85a\x11\x1AV[`\x80\x82\x01R`@Q\x92\x83\x91\x7FZ<9\x87\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0` \x84\x01R`$\x83\x01a\x16\xD2V[`\x80`\x03\x196\x01\x12a\x02\xFCWg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF`\x045\x81\x81\x11a\x02\xFCWa\x07\xE4\x906\x90`\x04\x01a\x12\xC4V[\x90a\x07\xEDa\x11\xB7V[\x90`d5\x90\x81\x11a\x02\xFCWa\x01\xF0a\x04\x8Ba\x029\x94a\x08\x14a\x04\x1C_\x956\x90`\x04\x01a\x0F\xCDV[`\x80\x82\x01R`@Q\x92\x83\x91\x7F\x08\xA4e\xF6\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0` \x84\x01R`$\x83\x01a\x14>V[`\xA0`\x03\x196\x01\x12a\x02\xFCWg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF`\x045\x11a\x02\xFCW6`#`\x045\x01\x12\x15a\x02\xFCWg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF`\x045`\x04\x015\x11a\x02\xFCW6`$`\xC0`\x045`\x04\x015\x02`\x045\x01\x01\x11a\x02\xFCW`$5g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11a\x02\xFCWa\x08\xC4\x906\x90`\x04\x01a\x0F\x9CV[g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF`D5\x11a\x02\xFCW```\x03\x19`D56\x03\x01\x12a\x02\xFCW`d5g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11a\x02\xFCWa\t\x05\x906\x90`\x04\x01a\x0F\xCDV[`\x845g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11a\x02\xFCWa\t%\x906\x90`\x04\x01a\x0F\x9CV[\x94\x90\x93a\t0a\x19EV[\x80`\x045`\x04\x015\x03a\x0EuW_[`\x045`\x04\x015\x81\x10a\x0B\xD2WPPP`D5`\x04\x015\x90\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xDD`D56\x03\x01\x82\x12\x15a\x02\xFCW\x81`D5\x01`\x04\x81\x015\x90g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11a\x02\xFCW`$\x82`\x07\x1B6\x03\x91\x01\x13a\x02\xFCWa\t\xE3W[a\x02\xA3a\x038\x86\x86_\x7F\x9Bw\x9B\x17B-\r\xF9\"#\x01\x8B2\xB4\xD1\xFAF\xE0qr=h\x17\xE2Hm\0;\xEC\xC5_\0]a\x17\x9BV[`\x01`\x01`\xA0\x1B\x03\x94\x92\x94\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16;\x15a\x02\xFCW`@Q\x94\x7F*-\x80\xD1\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x86R3`\x04\x87\x01R```$\x87\x01R`\xC4\x86\x01\x92`D5\x01`$\x81\x01\x93g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF`\x04\x83\x015\x11a\x02\xFCW`\x04\x82\x015`\x07\x1B6\x03\x85\x13a\x02\xFCW```d\x89\x01R`\x04\x82\x015\x90R\x91\x92\x86\x92`\xE4\x84\x01\x92\x91\x90_\x90[`\x04\x81\x015\x82\x10a\x0BTWPPP` \x91`\x1F\x19`\x1F\x86_\x97\x87\x95`\x01`\x01`\xA0\x1B\x03a\n\xC8`$`D5\x01a\x11\x8DV[\x16`\x84\x88\x01R`D\x805\x015`\xA4\x88\x01R`\x03\x19\x87\x87\x03\x01`D\x88\x01R\x81\x86R\x87\x86\x017\x87\x86\x82\x86\x01\x01R\x01\x16\x01\x03\x01\x81\x83`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x91\x82\x15a\x02\xF1Wa\x02\xA3\x93a\x038\x93a\x0BEW[\x82\x94P\x81\x93Pa\t\xB3V[a\x0BN\x90a\x11\x06V[\x84a\x0B:V[\x91\x95\x94P\x91\x92`\x01`\x01`\xA0\x1B\x03a\x0Bk\x87a\x11\x8DV[\x16\x81R` \x80\x87\x015\x91`\x01`\x01`\xA0\x1B\x03\x83\x16\x80\x93\x03a\x02\xFCW`\x04\x92`\x01\x92\x82\x01Ra\x0B\x9B`@\x89\x01a(\xA6V[e\xFF\xFF\xFF\xFF\xFF\xFF\x80\x91\x16`@\x83\x01Ra\x0B\xB6``\x8A\x01a(\xA6V[\x16``\x82\x01R`\x80\x80\x91\x01\x97\x01\x93\x01\x90P\x88\x94\x95\x93\x92\x91a\n\x97V[a\x0B\xE7a\x0B\xE0\x82\x84\x86a\x19*V[6\x91a\x13\x81V[`@Qa\x0B\xF3\x81a\x10\xA1V[_\x81R` \x91_\x83\x83\x01R_`@\x83\x01R\x82\x81\x01Q\x90```@\x82\x01Q\x91\x01Q_\x1A\x91\x83R\x83\x83\x01R`@\x82\x01R`\xC0\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xDC\x81\x85\x02`\x045\x016\x03\x01\x12a\x02\xFCW`@Q\x90a\x0C`\x82a\x10\xEAV[a\x0Cs`$`\xC0\x86\x02`\x045\x01\x01a\x11\x8DV[\x80\x83Ra\x0C\x89`D`\xC0\x87\x02`\x045\x01\x01a\x11\x8DV[\x90\x81\x85\x85\x01Ra\x0C\xA2`d`\xC0\x88\x02`\x045\x01\x01a\x11\x8DV[`@\x85\x81\x01\x91\x90\x91R`\x045`\xC0\x88\x02\x01`\x84\x81\x015``\x87\x01R`\xA4\x81\x015`\x80\x87\x01R`\xC4\x015`\xA0\x86\x01R\x83\x01Q\x83Q\x93\x86\x01Q`\xFF\x91\x90\x91\x16\x92`\x01`\x01`\xA0\x1B\x03\x83\x16;\x15a\x02\xFCW_`\x01`\x01`\xA0\x1B\x03\x80\x94`\xE4\x94\x8B\x98\x84\x98`\xC4`\xC0`@Q\x9C\x8D\x9B\x8C\x9A\x7F\xD5\x05\xAC\xCF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x8CR\x16`\x04\x8B\x01R0`$\x8B\x01R`\x84\x82\x82\x02`\x045\x01\x015`D\x8B\x01R\x02`\x045\x01\x015`d\x88\x01R`\x84\x87\x01R`\xA4\x86\x01R`\xC4\x85\x01R\x16Z\xF1\x90\x81a\x0EfW[Pa\x0E\\Wa\r\x7Fa(wV[\x90`\x01`\x01`\xA0\x1B\x03\x81Q\x16\x90\x83`\x01`\x01`\xA0\x1B\x03\x81\x83\x01Q\x16`D`@Q\x80\x95\x81\x93\x7F\xDDb\xED>\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x83R`\x04\x83\x01R0`$\x83\x01RZ\xFA\x91\x82\x15a\x02\xF1W_\x92a\x0E,W[P``\x01Q\x03a\r\xF7WPP`\x01\x90[\x01a\t?V[\x80Q\x15a\x0E\x04W\x80Q\x91\x01\xFD[\x7F\xA7(V\x89\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x04_\xFD[\x90\x91P\x83\x81\x81=\x83\x11a\x0EUW[a\x0ED\x81\x83a\x11RV[\x81\x01\x03\x12a\x02\xFCWQ\x90``a\r\xE1V[P=a\x0E:V[PP`\x01\x90a\r\xF1V[a\x0Eo\x90a\x11\x06V[\x8Aa\rrV[\x7F\xAA\xAD\x13\xF7\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x04_\xFD[4a\x02\xFCWa\x0E\xAB6a\x0E\xCAV[a\x0E\xB3a\x19EV[a\x0E\xBBa\x19rV[a\x03\x90a\x02\xA3a\x03q\x83a\x1A;V[`\x03\x19\x90` \x82\x82\x01\x12a\x02\xFCW`\x045\x91g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x83\x11a\x02\xFCW\x82`\xA0\x92\x03\x01\x12a\x02\xFCW`\x04\x01\x90V[\x90\x81Q\x80\x82R` \x80\x80\x93\x01\x93\x01\x91_[\x82\x81\x10a\x0F\x1BWPPPP\x90V[\x83Q\x85R\x93\x81\x01\x93\x92\x81\x01\x92`\x01\x01a\x0F\rV[\x93\x92\x90a\x0FD\x90``\x86R``\x86\x01\x90a\x0E\xFCV[\x93` \x94\x81\x81\x03` \x83\x01R` \x80\x85Q\x92\x83\x81R\x01\x94\x01\x90_[\x81\x81\x10a\x0F\x7FWPPPa\x0F|\x93\x94P`@\x81\x84\x03\x91\x01Ra\x0E\xFCV[\x90V[\x82Q`\x01`\x01`\xA0\x1B\x03\x16\x86R\x94\x87\x01\x94\x91\x87\x01\x91`\x01\x01a\x0F_V[\x91\x81`\x1F\x84\x01\x12\x15a\x02\xFCW\x825\x91g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x83\x11a\x02\xFCW` \x80\x85\x01\x94\x84`\x05\x1B\x01\x01\x11a\x02\xFCWV[\x91\x81`\x1F\x84\x01\x12\x15a\x02\xFCW\x825\x91g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x83\x11a\x02\xFCW` \x83\x81\x86\x01\x95\x01\x01\x11a\x02\xFCWV[\x90`\x1F\x19`\x1F` \x80\x94\x80Q\x91\x82\x91\x82\x87R\x01\x86\x86\x01^_\x85\x82\x86\x01\x01R\x01\x16\x01\x01\x90V[` \x80\x82\x01\x90` \x83R\x83Q\x80\x92R`@\x83\x01\x92` `@\x84`\x05\x1B\x83\x01\x01\x95\x01\x93_\x91[\x84\x83\x10a\x10UWPPPPPP\x90V[\x90\x91\x92\x93\x94\x95\x84\x80a\x10\x91\x83\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xC0\x86`\x01\x96\x03\x01\x87R\x8AQa\x0F\xFBV[\x98\x01\x93\x01\x93\x01\x91\x94\x93\x92\x90a\x10EV[``\x81\x01\x90\x81\x10g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11\x17a\x10\xBDW`@RV[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`A`\x04R`$_\xFD[`\xC0\x81\x01\x90\x81\x10g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11\x17a\x10\xBDW`@RV[g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11a\x10\xBDW`@RV[`\xA0\x81\x01\x90\x81\x10g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11\x17a\x10\xBDW`@RV[`\xE0\x81\x01\x90\x81\x10g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11\x17a\x10\xBDW`@RV[\x90`\x1F`\x1F\x19\x91\x01\x16\x81\x01\x90\x81\x10g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11\x17a\x10\xBDW`@RV[g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11a\x10\xBDW`\x05\x1B` \x01\x90V[5\x90`\x01`\x01`\xA0\x1B\x03\x82\x16\x82\x03a\x02\xFCWV[`$5\x90`\x01`\x01`\xA0\x1B\x03\x82\x16\x82\x03a\x02\xFCWV[`D5\x90\x81\x15\x15\x82\x03a\x02\xFCWV[\x91\x90\x91`\x80\x81\x84\x03\x12a\x02\xFCW`@\x90\x81Q\x91`\x80\x83\x01\x94g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x95\x84\x81\x10\x87\x82\x11\x17a\x10\xBDW\x82R\x83\x95a\x12\0\x84a\x11\x8DV[\x85R` \x90\x81\x85\x015\x90\x81\x11a\x02\xFCW\x84\x01\x82`\x1F\x82\x01\x12\x15a\x02\xFCW\x805\x90a\x12)\x82a\x11uV[\x93a\x126\x86Q\x95\x86a\x11RV[\x82\x85R\x83\x85\x01\x90\x84``\x80\x95\x02\x84\x01\x01\x92\x81\x84\x11a\x02\xFCW\x85\x01\x91[\x83\x83\x10a\x12tWPPPPP\x84\x01R\x81\x81\x015\x90\x83\x01R``\x90\x81\x015\x91\x01RV[\x84\x83\x83\x03\x12a\x02\xFCW\x87Q\x90a\x12\x89\x82a\x10\xA1V[a\x12\x92\x84a\x11\x8DV[\x82Ra\x12\x9F\x87\x85\x01a\x11\x8DV[\x87\x83\x01R\x88\x84\x015\x90\x81\x15\x15\x82\x03a\x02\xFCW\x82\x88\x92\x8B\x89\x95\x01R\x81R\x01\x92\x01\x91a\x12RV[\x81`\x1F\x82\x01\x12\x15a\x02\xFCW\x805\x91` \x91a\x12\xDE\x84a\x11uV[\x93a\x12\xEC`@Q\x95\x86a\x11RV[\x80\x85R\x83\x80\x86\x01\x91`\x05\x1B\x83\x01\x01\x92\x80\x84\x11a\x02\xFCW\x84\x83\x01\x91[\x84\x83\x10a\x13\x17WPPPPPP\x90V[\x825g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11a\x02\xFCW\x86\x91a\x139\x84\x84\x80\x94\x89\x01\x01a\x11\xC6V[\x81R\x01\x92\x01\x91a\x13\x07V[5`\x01`\x01`\xA0\x1B\x03\x81\x16\x81\x03a\x02\xFCW\x90V[5\x80\x15\x15\x81\x03a\x02\xFCW\x90V[g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11a\x10\xBDW`\x1F\x01`\x1F\x19\x16` \x01\x90V[\x92\x91\x92a\x13\x8D\x82a\x13eV[\x91a\x13\x9B`@Q\x93\x84a\x11RV[\x82\x94\x81\x84R\x81\x83\x01\x11a\x02\xFCW\x82\x81` \x93\x84_\x96\x017\x01\x01RV[\x90`\x80\x81\x01\x91`\x01`\x01`\xA0\x1B\x03\x80\x82Q\x16\x83R` \x93\x84\x83\x01Q\x94`\x80\x81\x86\x01R\x85Q\x80\x92R\x80`\xA0\x86\x01\x96\x01\x92_\x90[\x83\x82\x10a\x14\x0BWPPPPP``\x81`@\x82\x93\x01Q`@\x85\x01R\x01Q\x91\x01R\x90V[\x84Q\x80Q\x82\x16\x89R\x80\x84\x01Q\x82\x16\x89\x85\x01R`@\x90\x81\x01Q\x15\x15\x90\x89\x01R``\x90\x97\x01\x96\x93\x82\x01\x93`\x01\x90\x91\x01\x90a\x13\xE9V[\x91\x90\x91` \x90\x81\x81R`\xC0\x81\x01\x91`\x01`\x01`\xA0\x1B\x03\x85Q\x16\x81\x83\x01R\x80\x85\x01Q\x92`\xA0`@\x84\x01R\x83Q\x80\x91R`\xE0\x83\x01\x91\x80`\xE0\x83`\x05\x1B\x86\x01\x01\x95\x01\x92_\x90[\x83\x82\x10a\x14\xBDWPPPPP`\x80\x84`@a\x0F|\x95\x96\x01Q``\x84\x01R``\x81\x01Q\x15\x15\x82\x84\x01R\x01Q\x90`\xA0`\x1F\x19\x82\x85\x03\x01\x91\x01Ra\x0F\xFBV[\x90\x91\x92\x93\x95\x83\x80a\x14\xF8\x83\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF \x8A`\x01\x96\x03\x01\x86R\x8AQa\x13\xB7V[\x98\x01\x92\x01\x92\x01\x90\x93\x92\x91a\x14\x81V[\x81`\x1F\x82\x01\x12\x15a\x02\xFCW\x80Q\x90a\x15\x1E\x82a\x13eV[\x92a\x15,`@Q\x94\x85a\x11RV[\x82\x84R` \x83\x83\x01\x01\x11a\x02\xFCW\x81_\x92` \x80\x93\x01\x83\x86\x01^\x83\x01\x01R\x90V[\x90` \x82\x82\x03\x12a\x02\xFCW\x81Qg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11a\x02\xFCWa\x0F|\x92\x01a\x15\x07V[\x90\x80`\x1F\x83\x01\x12\x15a\x02\xFCW\x81Q\x90` \x91a\x15\x8E\x81a\x11uV[\x93a\x15\x9C`@Q\x95\x86a\x11RV[\x81\x85R` \x80\x86\x01\x92`\x05\x1B\x82\x01\x01\x92\x83\x11a\x02\xFCW` \x01\x90[\x82\x82\x10a\x15\xC5WPPPP\x90V[\x81Q\x81R\x90\x83\x01\x90\x83\x01a\x15\xB7V[\x90\x91``\x82\x84\x03\x12a\x02\xFCW\x81Q\x91g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x92\x83\x81\x11a\x02\xFCW\x84a\x16\0\x91\x83\x01a\x15sV[\x93` \x80\x83\x01Q\x85\x81\x11a\x02\xFCW\x83\x01\x90\x82`\x1F\x83\x01\x12\x15a\x02\xFCW\x81Q\x91a\x16(\x83a\x11uV[\x92a\x166`@Q\x94\x85a\x11RV[\x80\x84R\x82\x80\x85\x01\x91`\x05\x1B\x83\x01\x01\x91\x85\x83\x11a\x02\xFCW\x83\x01\x90[\x82\x82\x10a\x16rWPPPP\x93`@\x83\x01Q\x90\x81\x11a\x02\xFCWa\x0F|\x92\x01a\x15sV[\x81Q`\x01`\x01`\xA0\x1B\x03\x81\x16\x81\x03a\x02\xFCW\x81R\x90\x83\x01\x90\x83\x01a\x16PV[\x80Q\x82\x10\x15a\x16\xA5W` \x91`\x05\x1B\x01\x01\x90V[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`2`\x04R`$_\xFD[\x91\x90\x91` \x90\x81\x81R`\xC0\x81\x01\x91`\x01`\x01`\xA0\x1B\x03\x85Q\x16\x81\x83\x01R\x80\x85\x01Q\x92`\xA0`@\x84\x01R\x83Q\x80\x91R`\xE0\x83\x01\x91\x80`\xE0\x83`\x05\x1B\x86\x01\x01\x95\x01\x92_\x90[\x83\x82\x10a\x17QWPPPPP`\x80\x84`@a\x0F|\x95\x96\x01Q``\x84\x01R``\x81\x01Q\x15\x15\x82\x84\x01R\x01Q\x90`\xA0`\x1F\x19\x82\x85\x03\x01\x91\x01Ra\x0F\xFBV[\x90\x91\x92\x93\x95\x83\x80a\x17\x8C\x83\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF \x8A`\x01\x96\x03\x01\x86R\x8AQa\x13\xB7V[\x98\x01\x92\x01\x92\x01\x90\x93\x92\x91a\x17\x15V[\x91\x90a\x17\xA63a(\xB9V[\x90\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x93\x84\\a\x18\xB1W`\x01\x90`\x01\x86]a\x17\xDF\x83a\x11uV[\x92a\x17\xED`@Q\x94\x85a\x11RV[\x80\x84R`\x1F\x19a\x17\xFC\x82a\x11uV[\x01_[\x81\x81\x10a\x18\xA0WPP_[\x81\x81\x10a\x18WWPPPP\x90_a\x18L\x92\x94]\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x80\\\x91a\x18NW[Pa6\xB1V[V[_\x90]_a\x18FV[\x80a\x18\x84_\x80a\x18la\x0B\xE0\x89\x96\x88\x8Aa\x19*V[` \x81Q\x91\x010Z\xF4a\x18}a(wV[\x900aA\\V[a\x18\x8E\x82\x88a\x16\x91V[Ra\x18\x99\x81\x87a\x16\x91V[P\x01a\x18\nV[\x80``` \x80\x93\x89\x01\x01R\x01a\x17\xFFV[\x7F>\xE5\xAE\xB5\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x04_\xFD[\x905\x90\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE1\x816\x03\x01\x82\x12\x15a\x02\xFCW\x01\x805\x90g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11a\x02\xFCW` \x01\x91\x816\x03\x83\x13a\x02\xFCWV[\x90\x82\x10\x15a\x16\xA5Wa\x19A\x91`\x05\x1B\x81\x01\x90a\x18\xD9V[\x90\x91V[\x7F\x9Bw\x9B\x17B-\r\xF9\"#\x01\x8B2\xB4\xD1\xFAF\xE0qr=h\x17\xE2Hm\0;\xEC\xC5_\0\x80\\a\x18\xB1W`\x01\x90]V[`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x163\x03a\x19\xA4WV[\x7F\x08\x96v\xD5\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R3`\x04R`$_\xFD[\x90a\x19\xDA\x82a\x11uV[a\x19\xE7`@Q\x91\x82a\x11RV[\x82\x81R`\x1F\x19a\x19\xF7\x82\x94a\x11uV[\x01\x90` 6\x91\x017V[\x91\x90\x82\x01\x80\x92\x11a\x1A\x0EWV[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x11`\x04R`$_\xFD[`@\x81\x015B\x11a&\xC3W\x90a\x1A^a\x1AW` \x84\x01\x84a6\xF4V[\x90Pa\x19\xD0V[\x91_[a\x1An` \x83\x01\x83a6\xF4V[\x90P\x81\x10\x15a%\xC7Wa\x1A\x98\x81a\x1A\x93a\x1A\x8B` \x86\x01\x86a6\xF4V[6\x93\x91a7HV[a\x11\xC6V[\x93`@\x85\x01Q\x93`\x01`\x01`\xA0\x1B\x03\x86Q\x16\x90` \x87\x01Q\x80Q\x15a\x16\xA5W` \x01Q`@\x01Q\x15\x15\x80a%\xBEW[\x15a%cWa\x1A\xECa\x1A\xD8\x86a\x13DV[\x87\x84a\x1A\xE6``\x8A\x01a\x13XV[\x92a:\xDDV[_[` \x88\x01QQ\x81\x10\x15a%SWa\x1B\x03a7\x88V[` \x89\x01QQ_\x19\x81\x01\x90\x81\x11a\x1A\x0EW\x82\x14\x80` \x83\x01R\x82\x15\x82R_\x14a%LW``\x89\x01Q\x90[a\x1B;\x83` \x8C\x01Qa\x16\x91V[Q`@\x81\x01Q\x90\x91\x90\x15a\x1C\xEEWa\x1B\xD3`\x01`\x01`\xA0\x1B\x03\x83Q\x16\x93`\x01`\x01`\xA0\x1B\x03\x88\x16\x85\x14_\x14a\x1C\xE7W`\x01\x94[`@Q\x95a\x1B{\x87a\x11\x1AV[_\x87Ra\x1B\x87\x81a7\xBEV[` \x87\x01R`@\x86\x01R``\x94\x85\x91\x8D\x83\x83\x01R`\x80\x82\x01R`@Q\x80\x93\x81\x92\x7FCX;\xE5\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x83R`\x04\x83\x01a:\"V[\x03\x81_`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x93\x84\x15a\x02\xF1W_\x94a\x1C\xB0W[PP` \x01Q\x15a\x1C\x96W\x81`\x01`\x01`\xA0\x1B\x03` a\x1C\x90\x93`\x01\x96\x95a\x1C8\x8C\x8Ca\x16\x91V[R\x01a\x1Cg\x82\x82Q\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aA\xC0V[PQ\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aB\nV[\x01a\x1A\xEEV[` \x01Q\x90\x97P`\x01`\x01`\xA0\x1B\x03\x16\x92P`\x01\x90a\x1C\x90V[` \x92\x94P\x90\x81a\x1C\xD5\x92\x90=\x10a\x1C\xE0W[a\x1C\xCD\x81\x83a\x11RV[\x81\x01\x90a7\xF5V[\x91PP\x92\x90_a\x1C\x10V[P=a\x1C\xC3V[_\x94a\x1BnV[\x88\x8A`\x01`\x01`\xA0\x1B\x03\x84\x95\x94Q\x16\x80`\x01`\x01`\xA0\x1B\x03\x8A\x16\x14_\x14a!2WPP\x81Q\x15\x90Pa nW\x88\x8A\x80\x15\x15\x80a SW[a\x1FMW[`\x01`\x01`\xA0\x1B\x03\x93\x92\x91a\x1D\xDD\x82a\x1E\x15\x97\x8B_\x95\x89\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x92\x16\x80\x88R\x82` R`@\x88 \\a\x1F<W[PPP[`\x01a\x1D\x9C\x89\x83Q\x16` \x84\x01\x99\x8B\x8BQ\x16\x90\x80\x15\x8A\x14a\x1F6WP\x83\x91aB#V[\x99\x90\x92Q\x16\x94a\x1D\xB1`\x80\x91\x82\x81\x01\x90a\x18\xD9V[\x93\x90\x94`@Q\x97a\x1D\xC1\x89a\x10\xEAV[\x88R0` \x89\x01R`@\x88\x01R``\x87\x01R\x85\x01R6\x91a\x13\x81V[`\xA0\x82\x01R`@Q\x80\x96\x81\x92\x7F!Ex\x97\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x83R`\x04\x83\x01a9\xB1V[\x03\x81\x83`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x93\x84\x15a\x02\xF1W_\x94a\x1F\x0CW[P` \x01Q\x15a\x1E\xE9W\x91a\x1E\xBC\x82`\x01`\x01`\xA0\x1B\x03`\x01\x96\x95a\x1Eza\x1E\xE4\x96\x86a\x16\x91V[Qa\x1E\x85\x8D\x8Da\x16\x91V[Ra\x1E\xB3\x82\x82Q\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aA\xC0V[PQ\x16\x92a\x16\x91V[Q\x90\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aB\nV[a\x1C\x90V[\x98P`\x01\x92\x94Pa\x1F\x02\x90`\x01`\x01`\xA0\x1B\x03\x92a\x16\x91V[Q\x97Q\x16\x92a\x1C\x90V[` \x91\x94Pa\x1F,\x90=\x80_\x83>a\x1F$\x81\x83a\x11RV[\x81\x01\x90a9iV[P\x94\x91\x90Pa\x1ERV[\x91aB#V[a\x1FE\x92aCAV[_\x82\x81a\x1DuV[Pa\x1FZ\x90\x92\x91\x92a\x13DV[\x91a\x1Fd\x8BaB\xFDV[`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16;\x15a\x02\xFCW`@Q\x7F6\xC7\x85\x16\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x94\x85\x16`\x04\x82\x01R0`$\x82\x01R\x90\x84\x16`D\x82\x01R\x92\x87\x16`d\x84\x01R_\x83\x80`\x84\x81\x01\x03\x81\x83`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x80\x15a\x02\xF1W\x8Aa\x1D\xDD\x8Da\x1E\x15\x97`\x01`\x01`\xA0\x1B\x03\x97_\x95a DW[P\x97P\x92PP\x91\x92\x93Pa\x1D*V[a M\x90a\x11\x06V[_a 5V[Pa ]\x82a\x13DV[`\x01`\x01`\xA0\x1B\x03\x160\x14\x15a\x1D%V[\x90`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x91`\x01`\x01`\xA0\x1B\x03\x84Q\x16\x92\x80;\x15a\x02\xFCW`@Q\x7F\xAEc\x93)\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x94\x90\x94\x16`\x04\x85\x01R0`$\x85\x01R`D\x84\x01\x8C\x90R_\x90\x84\x90`d\x90\x82\x90\x84\x90Z\xF1\x80\x15a\x02\xF1W\x8Aa\x1D\xDD\x8Da\x1E\x15\x97`\x01`\x01`\xA0\x1B\x03\x97_\x95a!#W[Pa\x1DyV[a!,\x90a\x11\x06V[_a!\x1DV[`\x01`\x01`\xA0\x1B\x03` \x87\x96\x94\x97\x01Q\x16\x90\x89\x81\x83\x14_\x14a#\xD7Wa!\xCD\x92Pa\"\x05\x97\x91P`\x01a!s_\x96\x95`\x01`\x01`\xA0\x1B\x03\x93\x84\x8BQ\x16aB#V[P\x92\x82\x89Q\x16\x95` \x89\x01Q\x15\x15\x88\x14a#\xAEWa!\x90\x82a\x13DV[\x94[a!\xA1`\x80\x93\x84\x81\x01\x90a\x18\xD9V[\x95\x90\x96`@Q\x99a!\xB1\x8Ba\x10\xEAV[\x8AR\x16` \x89\x01R`@\x88\x01R``\x87\x01R\x85\x01R6\x91a\x13\x81V[`\xA0\x82\x01R`@Q\x80\x95\x81\x92\x7FJ\xF2\x9E\xC4\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x83R`\x04\x83\x01a8\xF8V[\x03\x81\x83`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x92\x83\x15a\x02\xF1W_\x93a#\x84W[P` \x01Q\x15a\"\xC3W\x81`\x01`\x01`\xA0\x1B\x03` a\x1E\xE4\x93`\x01\x96\x95a\"i\x8C\x8Ca\x16\x91V[Ra\"\x99\x83\x83\x83\x01Q\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aA\xC0V[P\x01Q\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aB\nV[` \x81\x81\x01Q\x91Q`@Q\x7F\x15\xAF\xD4\t\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x91\x82\x16`\x04\x82\x01R`$\x81\x01\x85\x90R\x93\x9AP\x90\x91\x16\x94P\x81\x80`D\x81\x01\x03\x81_`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x80\x15a\x02\xF1Wa#YW[P`\x01\x90a\x1C\x90V[` \x90\x81=\x83\x11a#}W[a#o\x81\x83a\x11RV[\x81\x01\x03\x12a\x02\xFCW_a#PV[P=a#eV[` \x91\x93Pa#\xA4\x90=\x80_\x83>a#\x9C\x81\x83a\x11RV[\x81\x01\x90a8|V[P\x93\x91\x90Pa\"BV[\x83\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x94a!\x92V[`\x01`\x01`\xA0\x1B\x03a$f\x95a$.\x93\x94\x95a#\xF8`\x80\x9B\x8C\x81\x01\x90a\x18\xD9V[\x93\x90\x94`@Q\x97a$\x08\x89a\x116V[_\x89R` \x89\x01R\x16`@\x87\x01R``\x9A\x8B\x97\x88\x88\x01R\x86\x01R`\xA0\x85\x01R6\x91a\x13\x81V[`\xC0\x82\x01R`@Q\x80\x93\x81\x92\x7F+\xFBx\x0C\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x83R`\x04\x83\x01a8\x10V[\x03\x81_`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x93\x84\x15a\x02\xF1W_\x94a%%W[PP` \x01Q\x15a\x1C\x96W\x81`\x01`\x01`\xA0\x1B\x03` a\x1E\xE4\x93`\x01\x96\x95a$\xCB\x8C\x8Ca\x16\x91V[Ra$\xFB\x83\x83\x83\x01Q\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aA\xC0V[P\x01Q\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aB\nV[` \x92\x94P\x90\x81a%A\x92\x90=\x10a\x1C\xE0Wa\x1C\xCD\x81\x83a\x11RV[\x91PP\x92\x90_a$\xA3V[_\x90a\x1B-V[P\x91\x95P\x90\x93PP`\x01\x01a\x1AaV[a%\x8D\x82\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aA\xC0V[Pa%\xB9\x86\x83\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aB\nV[a\x1A\xECV[P2\x15\x15a\x1A\xC7V[PPa%\xF2\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0a:qV[\x91a%\xFD\x83Qa\x19\xD0V[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x91\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x91\x90_[\x86Q\x81\x10\x15a&\xBAW`\x01\x90`\x01`\x01`\xA0\x1B\x03\x80a&c\x83\x8Ba\x16\x91V[Q\x16_R\x85` Ra&\x91`@_ \\\x82a&~\x85\x8Da\x16\x91V[Q\x16_R\x88` R`@_ \\\x90a\x1A\x01V[a&\x9B\x83\x87a\x16\x91V[Ra&\xA6\x82\x8Aa\x16\x91V[Q\x16_R\x85` R_`@\x81 ]\x01a&DV[P\x94\x93\x91P\x91PV[\x7F\xE0\x8B\x8A\xF0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x04_\xFD[\x90_\x19\x82\x01\x91\x82\x13`\x01\x16a\x1A\x0EWV[\x7F\x80\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81\x14a\x1A\x0EW_\x19\x01\x90V[\x90\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x90\x81\\\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0a'y\x81\\a&\xEBV[\x90\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x91[_\x81\x12\x15a(:WPPPa'\xB1\x90a&\xEBV[\x91\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x92[_\x81\x12\x15a'\xEAWPPPPa\x18L\x90a6\xB1V[a(5\x90\x82_Ra(/` _\x83\x82\x82 \x01\\\x91\x82\x82R\x88\x81R\x88`@\x91a(\"\x8A\x8D\x85\x87 \\\x90`\x01`\x01`\xA0\x1B\x03\x89\x16\x90a>\xB0V[\x84\x84RR\x81 ]\x84a>\rV[Pa&\xFCV[a'\xD5V[a(r\x90\x82_Ra(/` _\x8A\x87\x85\x84\x84 \x01\\\x93\x84\x84R\x81\x81Ra(\"\x8C`@\x94\x85\x87 \\\x90`\x01`\x01`\xA0\x1B\x03\x89\x16\x90a:\xDDV[a'\x9DV[=\x15a(\xA1W=\x90a(\x88\x82a\x13eV[\x91a(\x96`@Q\x93\x84a\x11RV[\x82R=_` \x84\x01>V[``\x90V[5\x90e\xFF\xFF\xFF\xFF\xFF\xFF\x82\x16\x82\x03a\x02\xFCWV[\x90_\x91\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0`\x01`\x01`\xA0\x1B\x03\x81\\\x16\x15a(\xF1WPPV[\x90\x91\x92P]`\x01\x90V[\x90`@\x82\x015B\x11a&\xC3Wa)\x17a\x1AW` \x84\x01\x84a6\xF4V[\x91_[a)'` \x83\x01\x83a6\xF4V[\x90P\x81\x10\x15a5\xD1Wa)D\x81a\x1A\x93a\x1A\x8B` \x86\x01\x86a6\xF4V[``\x81\x01Q\x90a)~`\x01`\x01`\xA0\x1B\x03\x82Q\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aA\xC0V[P` \x81\x01QQ_\x19\x81\x01\x90\x81\x11a\x1A\x0EW[_\x81\x12\x15a)\xA4WPPP`\x01\x01a)\x1AV[a)\xB2\x81` \x84\x01Qa\x16\x91V[Qa)\xBBa7\x88V[\x90\x82\x15` \x83\x01R` \x84\x01QQ\x80_\x19\x81\x01\x11a\x1A\x0EW_\x19\x01\x83\x14\x80\x83Ra5\x8FW[` \x82\x01Q\x15a5TW`@\x84\x01Q`\x01`\x01`\xA0\x1B\x03\x85Q\x16\x91[`@\x81\x01Q\x15a,\x1DW\x83\x91`\x01`\x01`\xA0\x1B\x03``\x92` a*\xA0\x97\x01Q\x15\x15\x80a,\x14W[a+\xEDW[Q\x16\x90`\x01`\x01`\xA0\x1B\x03\x85\x16\x82\x03a+\xE6W`\x01\x91[`@Q\x92a*L\x84a\x11\x1AV[`\x01\x84Ra*Y\x81a7\xBEV[` \x84\x01R`@\x83\x01R\x88\x83\x83\x01R`\x80\x82\x01R`@Q\x80\x95\x81\x92\x7FCX;\xE5\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x83R`\x04\x83\x01a:\"V[\x03\x81_`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x92\x83\x15a\x02\xF1W\x87\x91\x8B\x91_\x95a+\xBFW[P` \x01Q\x15a+\xB0Wa+\xA6\x92\x84a+\x02a+\xAB\x97\x96\x94a+u\x94a\x16\x91V[Ra+6`\x01`\x01`\xA0\x1B\x03\x82\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aA\xC0V[P`\x01`\x01`\xA0\x1B\x03a+M\x84`@\x8A\x01Qa7\xB1V[\x91\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aB\nV[`\x01`\x01`\xA0\x1B\x03\x85Q\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aB\nV[a&\xFCV[a)\x91V[PPPa+\xAB\x91\x93P\x92a&\xFCV[` \x91\x95Pa+\xDC\x90``=``\x11a\x1C\xE0Wa\x1C\xCD\x81\x83a\x11RV[P\x95\x91\x90Pa*\xE1V[_\x91a*?V[a,\x0Fa+\xF9\x8Da\x13DV[\x8D\x8Ba\x1A\xE6\x88`@\x88\x84Q\x16\x93\x01Q\x93\x01a\x13XV[a*(V[P2\x15\x15a*#V[\x90`\x01`\x01`\xA0\x1B\x03\x82Q\x16\x80`\x01`\x01`\xA0\x1B\x03\x85\x16\x14_\x14a17WP` \x84\x01Qa0IWP`@Q\x92\x7F\x96xp\x92\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x84R`\x01`\x01`\xA0\x1B\x03\x83\x16`\x04\x85\x01R` \x84`$\x81`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xFA\x93\x84\x15a\x02\xF1W_\x94a0\x15W[P\x83\x91`\x01`\x01`\xA0\x1B\x03\x81Q\x16`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16;\x15a\x02\xFCW`@Q\x7F\xAEc\x93)\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x90\x91\x16`\x04\x82\x01R0`$\x82\x01R`D\x81\x01\x95\x90\x95R_\x85\x80`d\x81\x01\x03\x81\x83`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x90\x81\x15a\x02\xF1Wa-\xEC\x95_\x92a0\x06W[P[a\x1D\xDD`\x01`\x01`\xA0\x1B\x03a-\xA8\x8B\x82\x85Q\x16\x83` \x87\x01Q\x16\x90aB#V[P\x92Q\x16\x91\x8C`\x02a-\xBF`\x80\x92\x83\x81\x01\x90a\x18\xD9V[\x92\x90\x93`@Q\x96a-\xCF\x88a\x10\xEAV[\x87R0` \x88\x01R\x89`@\x88\x01R``\x87\x01R\x85\x01R6\x91a\x13\x81V[\x03\x81\x83`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x93\x84\x15a\x02\xF1W_\x94a/\xE3W[P` \x01Q\x15a.\xCFW\x90\x82\x91a+\xAB\x94\x93a.E\x89\x8Da\x16\x91V[Ra.z\x83`\x01`\x01`\xA0\x1B\x03\x84\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aB\nV[\x80\x83\x10\x80a.\xB4W[a.\x90W[PPPa&\xFCV[a.\xA6a.\xAC\x93a.\xA0\x8Ba\x13DV[\x92a7\xB1V[\x91aCVV[_\x80\x80a.\x88V[P0`\x01`\x01`\xA0\x1B\x03a.\xC7\x8Ba\x13DV[\x16\x14\x15a.\x83V[\x94P\x90\x80\x94\x80\x82\x10a.\xE8W[PPPa+\xAB\x90a&\xFCV[\x91a.\xF8` \x92a/w\x94a7\xB1V[\x90a/-\x82`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x83aCVV[`@Q\x93\x84\x92\x83\x92\x7F\x15\xAF\xD4\t\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x84R`\x04\x84\x01` \x90\x93\x92\x91\x93`\x01`\x01`\xA0\x1B\x03`@\x82\x01\x95\x16\x81R\x01RV[\x03\x81_`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x80\x15a\x02\xF1Wa/\xB8W[\x80\x80a.\xDCV[` \x90\x81=\x83\x11a/\xDCW[a/\xCE\x81\x83a\x11RV[\x81\x01\x03\x12a\x02\xFCW_a/\xB1V[P=a/\xC4V[` \x91\x94Pa/\xFB\x90=\x80_\x83>a\x1F$\x81\x83a\x11RV[P\x90\x94\x91\x90Pa.)V[a0\x0F\x90a\x11\x06V[_a-\x86V[\x90\x93P` \x81=` \x11a0AW[\x81a01` \x93\x83a\x11RV[\x81\x01\x03\x12a\x02\xFCWQ\x92_a,\xBCV[=\x91Pa0$V[\x90\x92a0T\x89a\x13DV[`\x01`\x01`\xA0\x1B\x030\x91\x16\x03a0oW[_a-\xEC\x94a-\x88V[`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x93a0\xA3\x8Aa\x13DV[a0\xAC\x84aB\xFDV[\x90\x86;\x15a\x02\xFCW`@Q\x7F6\xC7\x85\x16\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x91\x82\x16`\x04\x82\x01R0`$\x82\x01R\x91\x81\x16`D\x83\x01R\x85\x16`d\x82\x01R\x94_\x90\x86\x90`\x84\x90\x82\x90\x84\x90Z\xF1\x90\x81\x15a\x02\xF1Wa-\xEC\x95_\x92a1(W[P\x94PPa0eV[a11\x90a\x11\x06V[_a1\x1FV[`\x01`\x01`\xA0\x1B\x03` \x84\x96\x95\x94\x01Q\x16\x8A\x82\x82\x14_\x14a4\x0BWPPPa2\x0Ca1n_\x92\x84`\x01`\x01`\xA0\x1B\x03\x88Q\x16aB#V[\x92\x90a1\xD4\x8C`\x01`\x01`\xA0\x1B\x03\x80\x8AQ\x16\x93\x89Q\x15\x15\x86\x14a3\xDFWa1\xA3a1\x97\x84a\x13DV[\x93[`\x80\x81\x01\x90a\x18\xD9V[\x92\x90\x93`@Q\x96a1\xB3\x88a\x10\xEAV[\x87R\x16` \x86\x01R`@\x85\x01R\x8C``\x85\x01R`\x02`\x80\x85\x01R6\x91a\x13\x81V[`\xA0\x82\x01R`@Q\x80\x93\x81\x92\x7FJ\xF2\x9E\xC4\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x83R`\x04\x83\x01a8\xF8V[\x03\x81\x83`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x90\x81\x15a\x02\xF1W_\x91a3\xC4W[P` \x84\x01Q\x8C\x90\x8A\x90\x15a3\xAAW\x83\x83`\x01`\x01`\xA0\x1B\x03\x93a2\x83a2\x89\x94a2|\x8F\x9C\x9B\x9A\x98\x99a2\xB2\x9Aa\x16\x91V[Q\x92a\x16\x91V[Ra\x16\x91V[Q\x91\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aB\nV[Q\x15a2\xF4Wa+\xAB\x92\x91`\x01`\x01`\xA0\x1B\x03` a+\xA6\x93\x01Q\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aCAV[Q`@Q\x7F\x15\xAF\xD4\t\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x90\x91\x16`\x04\x82\x01R`$\x81\x01\x91\x90\x91R` \x81\x80`D\x81\x01\x03\x81_`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x80\x15a\x02\xF1Wa3\x7FW[Pa+\xAB\x90a&\xFCV[` \x90\x81=\x83\x11a3\xA3W[a3\x95\x81\x83a\x11RV[\x81\x01\x03\x12a\x02\xFCW_a3uV[P=a3\x8BV[PP\x90\x91Pa3\xBB\x92\x93\x96Pa\x16\x91V[Q\x93\x84\x91a2\xB2V[a3\xD8\x91P=\x80_\x83>a#\x9C\x81\x83a\x11RV[\x90Pa2IV[a1\xA3\x82\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x93a1\x99V[a4\x9E\x96P\x90a4f\x91``\x94\x8Ba4+`\x80\x99\x98\x99\x93\x84\x81\x01\x90a\x18\xD9V[\x93\x90\x94`@Q\x97a4;\x89a\x116V[`\x01\x89R` \x89\x01R`\x01`\x01`\xA0\x1B\x03\x8B\x16`@\x89\x01R\x88\x88\x01R\x86\x01R`\xA0\x85\x01R6\x91a\x13\x81V[`\xC0\x82\x01R`@Q\x80\x95\x81\x92\x7F+\xFBx\x0C\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x83R`\x04\x83\x01a8\x10V[\x03\x81_`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xF1\x92\x83\x15a\x02\xF1W\x87\x91\x8B\x91_\x95a5-W[P` \x01Q\x15a+\xB0Wa+\xA6\x92\x84a5\x05a+\xAB\x97\x96\x94`\x01`\x01`\xA0\x1B\x03\x94a\x16\x91V[R\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aB\nV[` \x91\x95Pa5J\x90``=``\x11a\x1C\xE0Wa\x1C\xCD\x81\x83a\x11RV[P\x95\x91\x90Pa4\xDFV[o\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF`\x01`\x01`\xA0\x1B\x03` a5\x85\x81\x88\x01Qa5\x7F\x88a&\xEBV[\x90a\x16\x91V[Q\x01Q\x16\x91a)\xFCV[a5\xCC\x85`\x01`\x01`\xA0\x1B\x03` \x84\x01a\x1Cg\x82\x82Q\x16\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0aA\xC0V[a)\xE0V[PPa5\xFC\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0a:qV[\x91a6\x07\x83Qa\x19\xD0V[\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x91\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x91\x90_[\x86Q\x81\x10\x15a&\xBAW`\x01\x90`\x01`\x01`\xA0\x1B\x03\x80a6m\x83\x8Ba\x16\x91V[Q\x16_R\x85` Ra6\x88`@_ \\\x82a&~\x85\x8Da\x16\x91V[a6\x92\x83\x87a\x16\x91V[Ra6\x9D\x82\x8Aa\x16\x91V[Q\x16_R\x85` R_`@\x81 ]\x01a6NV[G\x80\x15a6\xF0W\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\\a6\xF0W`\x01`\x01`\xA0\x1B\x03a\x18L\x92\x16a@\xE0V[PPV[\x905\x90\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE1\x816\x03\x01\x82\x12\x15a\x02\xFCW\x01\x805\x90g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11a\x02\xFCW` \x01\x91\x81`\x05\x1B6\x03\x83\x13a\x02\xFCWV[\x91\x90\x81\x10\x15a\x16\xA5W`\x05\x1B\x81\x015\x90\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x816\x03\x01\x82\x12\x15a\x02\xFCW\x01\x90V[`@Q\x90`@\x82\x01\x82\x81\x10g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11\x17a\x10\xBDW`@R_` \x83\x82\x81R\x01RV[\x91\x90\x82\x03\x91\x82\x11a\x1A\x0EWV[`\x02\x11\x15a7\xC8WV[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`!`\x04R`$_\xFD[\x90\x81``\x91\x03\x12a\x02\xFCW\x80Q\x91`@` \x83\x01Q\x92\x01Q\x90V[a\x01\0`\xC0a\x0F|\x93` \x84R\x80Qa8(\x81a7\xBEV[` \x85\x01R` \x81\x01Q`\x01`\x01`\xA0\x1B\x03\x80\x91\x16`@\x86\x01R\x80`@\x83\x01Q\x16``\x86\x01R``\x82\x01Q\x16`\x80\x85\x01R`\x80\x81\x01Q`\xA0\x85\x01R`\xA0\x81\x01Q\x82\x85\x01R\x01Q\x91`\xE0\x80\x82\x01R\x01\x90a\x0F\xFBV[\x90\x91``\x82\x84\x03\x12a\x02\xFCW\x81Q\x91g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x92\x83\x81\x11a\x02\xFCW\x84a8\xA8\x91\x83\x01a\x15sV[\x93` \x82\x01Q\x93`@\x83\x01Q\x90\x81\x11a\x02\xFCWa\x0F|\x92\x01a\x15\x07V[\x90\x81Q\x80\x82R` \x80\x80\x93\x01\x93\x01\x91_[\x82\x81\x10a8\xE4WPPPP\x90V[\x83Q\x85R\x93\x81\x01\x93\x92\x81\x01\x92`\x01\x01a8\xD6V[` \x81R`\x01`\x01`\xA0\x1B\x03\x80\x83Q\x16` \x83\x01R` \x83\x01Q\x16`@\x82\x01Ra91`@\x83\x01Q`\xC0``\x84\x01R`\xE0\x83\x01\x90a8\xC5V[\x90``\x83\x01Q`\x80\x82\x01R`\x80\x83\x01Q`\x05\x81\x10\x15a7\xC8Wa\x0F|\x93`\xA0\x91\x82\x84\x01R\x01Q\x90`\xC0`\x1F\x19\x82\x85\x03\x01\x91\x01Ra\x0F\xFBV[\x91``\x83\x83\x03\x12a\x02\xFCW\x82Q\x92` \x81\x01Q\x92g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x93\x84\x81\x11a\x02\xFCW\x81a9\x9A\x91\x84\x01a\x15sV[\x93`@\x83\x01Q\x90\x81\x11a\x02\xFCWa\x0F|\x92\x01a\x15\x07V[` \x81R`\x01`\x01`\xA0\x1B\x03\x80\x83Q\x16` \x83\x01R` \x83\x01Q\x16`@\x82\x01R`@\x82\x01Q``\x82\x01Ra9\xF4``\x83\x01Q`\xC0`\x80\x84\x01R`\xE0\x83\x01\x90a8\xC5V[\x90`\x80\x83\x01Q`\x04\x81\x10\x15a7\xC8Wa\x0F|\x93`\xA0\x91\x82\x84\x01R\x01Q\x90`\xC0`\x1F\x19\x82\x85\x03\x01\x91\x01Ra\x0F\xFBV[\x91\x90\x91`\x80\x80`\xA0\x83\x01\x94\x80Qa:8\x81a7\xBEV[\x84R` \x81\x01Qa:H\x81a7\xBEV[` \x85\x01R`\x01`\x01`\xA0\x1B\x03`@\x82\x01Q\x16`@\x85\x01R``\x81\x01Q``\x85\x01R\x01Q\x91\x01RV[\x90\x81\\a:}\x81a\x11uV[a:\x8A`@Q\x91\x82a\x11RV[\x81\x81Ra:\x96\x82a\x11uV[`\x1F\x19` \x91\x016` \x84\x017\x81\x94_[\x84\x81\x10a:\xB5WPPPPPV[`\x01\x90\x82_R\x80\x84_ \x01\\`\x01`\x01`\xA0\x1B\x03a:\xD3\x83\x88a\x16\x91V[\x91\x16\x90R\x01a:\xA7V[\x91\x92\x80a=\xD8W[\x15a<QWPP\x80G\x10a<)W`\x01`\x01`\xA0\x1B\x03\x80\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x91\x82;\x15a\x02\xFCW`@Q\x90\x7F\xD0\xE3\r\xB0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x82R_\x91_\x81`\x04\x81\x85\x89Z\xF1\x80\x15a\x02\xF1Wa<\x12W[P`D` \x92\x93\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x94a;\x98\x83\x87\x83aCVV[\x84`@Q\x96\x87\x94\x85\x93\x7F\x15\xAF\xD4\t\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x85R`\x04\x85\x01R`$\x84\x01RZ\xF1\x90\x81\x15a<\x06WPa;\xDFWPV[` \x90\x81=\x83\x11a;\xFFW[a;\xF5\x81\x83a\x11RV[\x81\x01\x03\x12a\x02\xFCWV[P=a;\xEBV[`@Q\x90=\x90\x82>=\x90\xFD[` \x92Pa<\x1F\x90a\x11\x06V[`D_\x92Pa;cV[\x7F\xA0\x1A\x9D\xF6\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x04_\xFD[\x90\x91_\x90\x80a<aW[PPPPV[`\x01`\x01`\xA0\x1B\x03\x93\x84\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x94\x80\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x91a<\xBB\x84aB\xFDV[\x96\x80;\x15a\x02\xFCW`@Q\x7F6\xC7\x85\x16\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x92\x83\x16`\x04\x82\x01R\x84\x83\x16`$\x82\x01R\x97\x82\x16`D\x89\x01R\x91\x86\x16\x16`d\x87\x01R_\x90\x86\x90`\x84\x90\x82\x90\x84\x90Z\xF1\x94\x85\x15a\x02\xF1Wa=\x80\x95a=\xC4W[P\x82\x93` \x93`@Q\x80\x97\x81\x95\x82\x94\x7F\x15\xAF\xD4\t\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x84R`\x04\x84\x01` \x90\x93\x92\x91\x93`\x01`\x01`\xA0\x1B\x03`@\x82\x01\x95\x16\x81R\x01RV[\x03\x92Z\xF1\x90\x81\x15a<\x06WPa=\x99W[\x80\x80\x80a<[V[` \x90\x81=\x83\x11a=\xBDW[a=\xAF\x81\x83a\x11RV[\x81\x01\x03\x12a\x02\xFCW_a=\x91V[P=a=\xA5V[` \x93Pa=\xD1\x90a\x11\x06V[_\x92a=/V[P`\x01`\x01`\xA0\x1B\x03\x80\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x90\x82\x16\x14a:\xE5V[`\x01\x81\x01\x91\x80_R` \x91\x83\x83R`@_ \\\x80\x15\x15_\x14a>\xA7W_\x19\x90\x81\x81\x01\x83\\\x83\x80\x82\x01\x91\x82\x84\x03a>jW[PPPPP\x81\\\x81\x81\x01\x92\x81\x84\x11a\x1A\x0EW_\x93\x81]\x83R\x84\x83 \x01\x01]_RR_`@\x81 ]`\x01\x90V[a>wa>\x87\x93\x88aD:V[\x86_R\x88_ \x01\x01\\\x91\x85aD:V[\x83_R\x80\x83\x83\x88_ \x01\x01]_R\x85\x85R`@_ ]_\x80\x80\x83\x81a>>V[PPPPP_\x90V[_\x94\x93\x83\x15a@\xD8W\x80a@\xA3W[\x15a@\x07W`\x01`\x01`\xA0\x1B\x03\x91\x82\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x80;\x15a\x02\xFCW`@Q\x7F\xAEc\x93)\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x92\x90\x92\x16`\x04\x83\x01R0`$\x83\x01R`D\x82\x01\x85\x90R_\x90\x82\x90`d\x90\x82\x90\x84\x90Z\xF1\x80\x15a\x02\xF1Wa?\xF4W[P\x84\x82\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x80;\x15a?\xF0W\x81\x90`$`@Q\x80\x94\x81\x93\x7F.\x1A}M\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x83R\x89`\x04\x84\x01RZ\xF1\x80\x15a?\xE5Wa?\xCDW[Pa\x18L\x93\x94P\x16a@\xE0V[a?\xD7\x86\x91a\x11\x06V[a?\xE1W\x84a?\xC0V[\x84\x80\xFD[`@Q=\x88\x82>=\x90\xFD[P\x80\xFD[a?\xFF\x91\x95Pa\x11\x06V[_\x93_a?SV[\x92\x93P\x90`\x01`\x01`\xA0\x1B\x03\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x80;\x15a\x02\xFCW`@Q\x7F\xAEc\x93)\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x01`\x01`\xA0\x1B\x03\x93\x84\x16`\x04\x82\x01R\x93\x90\x92\x16`$\x84\x01R`D\x83\x01R_\x90\x82\x90`d\x90\x82\x90\x84\x90Z\xF1\x80\x15a\x02\xF1Wa@\x9AWPV[a\x18L\x90a\x11\x06V[P`\x01`\x01`\xA0\x1B\x03\x80\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x90\x83\x16\x14a>\xBFV[PPPP\x90PV[\x81G\x10aA0W_\x80\x80\x93`\x01`\x01`\xA0\x1B\x03\x82\x94\x16Z\xF1aA\0a(wV[P\x15aA\x08WV[\x7F\x14%\xEAB\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x04_\xFD[\x7F\xCDx`Y\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R0`\x04R`$_\xFD[\x90aAqWP\x80Q\x15aA\x08W\x80Q\x90` \x01\xFD[\x81Q\x15\x80aA\xB7W[aA\x82WP\x90V[`\x01`\x01`\xA0\x1B\x03\x90\x7F\x99\x96\xB3\x15\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R\x16`\x04R`$_\xFD[P\x80;\x15aAzV[`\x01\x81\x01\x90\x82_R\x81` R`@_ \\\x15_\x14aB\x03W\x80\\\x81_R\x83\x81` _ \x01]`\x01\x81\x01\x80\x91\x11a\x1A\x0EW\x81]\\\x91_R` R`@_ ]`\x01\x90V[PPP_\x90V[\x90_R` RaB\x1F`@_ \x91\x82\\a\x1A\x01V[\x90]V[\x91`D\x92\x93\x91\x93`\x01`\x01`\xA0\x1B\x03`@\x94\x85\x92\x82\x80\x85Q\x99\x8A\x95\x86\x94\x7F\xC9\xC1f\x1B\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x86R\x16`\x04\x85\x01R\x16`$\x83\x01R\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16Z\xFA\x93\x84\x15aB\xF3W_\x93_\x95aB\xBCW[PPaB\xB9aB\xB2\x85\x94a\x19\xD0V[\x94\x85a\x16\x91V[RV[\x80\x92\x95P\x81\x94P=\x83\x11aB\xECW[aB\xD5\x81\x83a\x11RV[\x81\x01\x03\x12a\x02\xFCW` \x82Q\x92\x01Q\x92_\x80aB\xA3V[P=aB\xCBV[\x83Q=_\x82>=\x90\xFD[`\x01`\x01`\xA0\x1B\x03\x90\x81\x81\x11aC\x11W\x16\x90V[\x7Fm\xFC\xC6P\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\xA0`\x04R`$R`D_\xFD[\x90_R` RaB\x1F`@_ \x91\x82\\a7\xB1V[`@Q\x92` \x84\x01\x90\x7F\xA9\x05\x9C\xBB\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x82R`\x01`\x01`\xA0\x1B\x03\x80\x94\x16`$\x86\x01R`D\x85\x01R`D\x84R`\x80\x84\x01\x90\x84\x82\x10g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x83\x11\x17a\x10\xBDWaC\xD5\x93_\x93\x84\x93`@R\x16\x94Q\x90\x82\x86Z\xF1aC\xCEa(wV[\x90\x83aA\\V[\x80Q\x90\x81\x15\x15\x91\x82aD\x16W[PPaC\xEBWPV[\x7FRt\xAF\xE7\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x04R`$_\xFD[\x81\x92P\x90` \x91\x81\x01\x03\x12a\x02\xFCW` \x01Q\x80\x15\x90\x81\x15\x03a\x02\xFCW_\x80aC\xE2V[\\\x11\x15aDCWV[\x7F\x0FJ\xE0\xE4\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x04_\xFD\xFE\xA2dipfsX\"\x12 \"\x9A\\\xF8\x9A\xA7\xC2\xD0\xA4\xB4\xD5\xDB \xBB\xA6\xC2\xB3\xA7K\x08\x03\x03\xFCn\xC0\x0B\xA5\x82\xA5\xDC\xF7QdsolcC\0\x08\x1A\x003",
    );
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Custom error with signature `AddressEmptyCode(address)` and selector `0x9996b315`.
    ```solidity
    error AddressEmptyCode(address target);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct AddressEmptyCode {
        #[allow(missing_docs)]
        pub target: alloy_sol_types::private::Address,
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
        impl ::core::convert::From<AddressEmptyCode> for UnderlyingRustTuple<'_> {
            fn from(value: AddressEmptyCode) -> Self {
                (value.target,)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for AddressEmptyCode {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self { target: tuple.0 }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for AddressEmptyCode {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [153u8, 150u8, 179u8, 21u8];
            const SIGNATURE: &'static str = "AddressEmptyCode(address)";

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
                        &self.target,
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
    /**Custom error with signature `AddressInsufficientBalance(address)` and selector `0xcd786059`.
    ```solidity
    error AddressInsufficientBalance(address account);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct AddressInsufficientBalance {
        #[allow(missing_docs)]
        pub account: alloy_sol_types::private::Address,
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
        impl ::core::convert::From<AddressInsufficientBalance> for UnderlyingRustTuple<'_> {
            fn from(value: AddressInsufficientBalance) -> Self {
                (value.account,)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for AddressInsufficientBalance {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self { account: tuple.0 }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for AddressInsufficientBalance {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [205u8, 120u8, 96u8, 89u8];
            const SIGNATURE: &'static str = "AddressInsufficientBalance(address)";

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
                        &self.account,
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
    /**Custom error with signature `ErrorSelectorNotFound()` and selector `0xa7285689`.
    ```solidity
    error ErrorSelectorNotFound();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct ErrorSelectorNotFound;
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
        impl ::core::convert::From<ErrorSelectorNotFound> for UnderlyingRustTuple<'_> {
            fn from(value: ErrorSelectorNotFound) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for ErrorSelectorNotFound {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for ErrorSelectorNotFound {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [167u8, 40u8, 86u8, 137u8];
            const SIGNATURE: &'static str = "ErrorSelectorNotFound()";

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
    /**Custom error with signature `EthTransfer()` and selector `0x0540ddf6`.
    ```solidity
    error EthTransfer();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct EthTransfer;
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
        impl ::core::convert::From<EthTransfer> for UnderlyingRustTuple<'_> {
            fn from(value: EthTransfer) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for EthTransfer {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for EthTransfer {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [5u8, 64u8, 221u8, 246u8];
            const SIGNATURE: &'static str = "EthTransfer()";

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
    /**Custom error with signature `FailedInnerCall()` and selector `0x1425ea42`.
    ```solidity
    error FailedInnerCall();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct FailedInnerCall;
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
        impl ::core::convert::From<FailedInnerCall> for UnderlyingRustTuple<'_> {
            fn from(value: FailedInnerCall) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for FailedInnerCall {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for FailedInnerCall {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [20u8, 37u8, 234u8, 66u8];
            const SIGNATURE: &'static str = "FailedInnerCall()";

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
    /**Custom error with signature `InputLengthMismatch()` and selector `0xaaad13f7`.
    ```solidity
    error InputLengthMismatch();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct InputLengthMismatch;
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
        impl ::core::convert::From<InputLengthMismatch> for UnderlyingRustTuple<'_> {
            fn from(value: InputLengthMismatch) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for InputLengthMismatch {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for InputLengthMismatch {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [170u8, 173u8, 19u8, 247u8];
            const SIGNATURE: &'static str = "InputLengthMismatch()";

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
    /**Custom error with signature `InsufficientEth()` and selector `0xa01a9df6`.
    ```solidity
    error InsufficientEth();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct InsufficientEth;
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
        impl ::core::convert::From<InsufficientEth> for UnderlyingRustTuple<'_> {
            fn from(value: InsufficientEth) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for InsufficientEth {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for InsufficientEth {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [160u8, 26u8, 157u8, 246u8];
            const SIGNATURE: &'static str = "InsufficientEth()";

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
    /**Custom error with signature `SafeCastOverflowedUintDowncast(uint8,uint256)` and selector `0x6dfcc650`.
    ```solidity
    error SafeCastOverflowedUintDowncast(uint8 bits, uint256 value);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct SafeCastOverflowedUintDowncast {
        #[allow(missing_docs)]
        pub bits: u8,
        #[allow(missing_docs)]
        pub value: alloy_sol_types::private::primitives::aliases::U256,
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
            alloy_sol_types::sol_data::Uint<8>,
            alloy_sol_types::sol_data::Uint<256>,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (u8, alloy_sol_types::private::primitives::aliases::U256);
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
        impl ::core::convert::From<SafeCastOverflowedUintDowncast> for UnderlyingRustTuple<'_> {
            fn from(value: SafeCastOverflowedUintDowncast) -> Self {
                (value.bits, value.value)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for SafeCastOverflowedUintDowncast {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    bits: tuple.0,
                    value: tuple.1,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for SafeCastOverflowedUintDowncast {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [109u8, 252u8, 198u8, 80u8];
            const SIGNATURE: &'static str = "SafeCastOverflowedUintDowncast(uint8,uint256)";

            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }

            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Uint<8> as alloy_sol_types::SolType>::tokenize(
                        &self.bits,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.value,
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
    /**Custom error with signature `SenderIsNotVault(address)` and selector `0x089676d5`.
    ```solidity
    error SenderIsNotVault(address sender);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct SenderIsNotVault {
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
        impl ::core::convert::From<SenderIsNotVault> for UnderlyingRustTuple<'_> {
            fn from(value: SenderIsNotVault) -> Self {
                (value.sender,)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for SenderIsNotVault {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self { sender: tuple.0 }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for SenderIsNotVault {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [8u8, 150u8, 118u8, 213u8];
            const SIGNATURE: &'static str = "SenderIsNotVault(address)";

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
    /**Custom error with signature `SwapDeadline()` and selector `0xe08b8af0`.
    ```solidity
    error SwapDeadline();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct SwapDeadline;
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
        impl ::core::convert::From<SwapDeadline> for UnderlyingRustTuple<'_> {
            fn from(value: SwapDeadline) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for SwapDeadline {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for SwapDeadline {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [224u8, 139u8, 138u8, 240u8];
            const SIGNATURE: &'static str = "SwapDeadline()";

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
    /**Custom error with signature `TransientIndexOutOfBounds()` and selector `0x0f4ae0e4`.
    ```solidity
    error TransientIndexOutOfBounds();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct TransientIndexOutOfBounds;
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
        impl ::core::convert::From<TransientIndexOutOfBounds> for UnderlyingRustTuple<'_> {
            fn from(value: TransientIndexOutOfBounds) -> Self {
                ()
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for TransientIndexOutOfBounds {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolError for TransientIndexOutOfBounds {
            type Parameters<'a> = UnderlyingSolTuple<'a>;
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [15u8, 74u8, 224u8, 228u8];
            const SIGNATURE: &'static str = "TransientIndexOutOfBounds()";

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
    /**Constructor`.
    ```solidity
    constructor(address vault, address weth, address permit2, string routerVersion);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct constructorCall {
        #[allow(missing_docs)]
        pub vault: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub weth: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub permit2: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub routerVersion: alloy_sol_types::private::String,
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
                alloy_sol_types::sol_data::String,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
                alloy_sol_types::private::Address,
                alloy_sol_types::private::Address,
                alloy_sol_types::private::String,
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
                    (value.vault, value.weth, value.permit2, value.routerVersion)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for constructorCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        vault: tuple.0,
                        weth: tuple.1,
                        permit2: tuple.2,
                        routerVersion: tuple.3,
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
                alloy_sol_types::sol_data::String,
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
                        &self.vault,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.weth,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.permit2,
                    ),
                    <alloy_sol_types::sol_data::String as alloy_sol_types::SolType>::tokenize(
                        &self.routerVersion,
                    ),
                )
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `getSender()` and selector `0x5e01eb5a`.
    ```solidity
    function getSender() external view returns (address);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct getSenderCall;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`getSender()`](getSenderCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct getSenderReturn {
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
            impl ::core::convert::From<getSenderCall> for UnderlyingRustTuple<'_> {
                fn from(value: getSenderCall) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for getSenderCall {
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
            impl ::core::convert::From<getSenderReturn> for UnderlyingRustTuple<'_> {
                fn from(value: getSenderReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for getSenderReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for getSenderCall {
            type Parameters<'a> = ();
            type Return = alloy_sol_types::private::Address;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [94u8, 1u8, 235u8, 90u8];
            const SIGNATURE: &'static str = "getSender()";

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
                        let r: getSenderReturn = r.into();
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
                    let r: getSenderReturn = r.into();
                    r._0
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `multicall(bytes[])` and selector `0xac9650d8`.
    ```solidity
    function multicall(bytes[] memory data) external payable returns (bytes[] memory results);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct multicallCall {
        #[allow(missing_docs)]
        pub data: alloy_sol_types::private::Vec<alloy_sol_types::private::Bytes>,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`multicall(bytes[])`](multicallCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct multicallReturn {
        #[allow(missing_docs)]
        pub results: alloy_sol_types::private::Vec<alloy_sol_types::private::Bytes>,
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
            type UnderlyingSolTuple<'a> =
                (alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Bytes>,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> =
                (alloy_sol_types::private::Vec<alloy_sol_types::private::Bytes>,);
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
            impl ::core::convert::From<multicallCall> for UnderlyingRustTuple<'_> {
                fn from(value: multicallCall) -> Self {
                    (value.data,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for multicallCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { data: tuple.0 }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> =
                (alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Bytes>,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> =
                (alloy_sol_types::private::Vec<alloy_sol_types::private::Bytes>,);
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
            impl ::core::convert::From<multicallReturn> for UnderlyingRustTuple<'_> {
                fn from(value: multicallReturn) -> Self {
                    (value.results,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for multicallReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { results: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for multicallCall {
            type Parameters<'a> =
                (alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Bytes>,);
            type Return = alloy_sol_types::private::Vec<alloy_sol_types::private::Bytes>;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> =
                (alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Bytes>,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [172u8, 150u8, 80u8, 216u8];
            const SIGNATURE: &'static str = "multicall(bytes[])";

            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }

            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (<alloy_sol_types::sol_data::Array<
                    alloy_sol_types::sol_data::Bytes,
                > as alloy_sol_types::SolType>::tokenize(
                    &self.data
                ),)
            }

            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                (<alloy_sol_types::sol_data::Array<
                    alloy_sol_types::sol_data::Bytes,
                > as alloy_sol_types::SolType>::tokenize(ret),)
            }

            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence(data).map(
                    |r| {
                        let r: multicallReturn = r.into();
                        r.results
                    },
                )
            }

            #[inline]
            fn abi_decode_returns_validate(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence_validate(
                    data,
                )
                .map(|r| {
                    let r: multicallReturn = r.into();
                    r.results
                })
            }
        }
    };
    #[derive()]
    /**Function with signature `permitBatchAndCall((address,address,address,uint256,uint256,uint256)[],bytes[],((address,uint160,uint48,uint48)[],address,uint256),bytes,bytes[])` and selector `0x19c6989f`.
    ```solidity
    function permitBatchAndCall(IRouterCommon.PermitApproval[] memory permitBatch, bytes[] memory permitSignatures, IAllowanceTransfer.PermitBatch memory permit2Batch, bytes memory permit2Signature, bytes[] memory multicallData) external payable returns (bytes[] memory results);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct permitBatchAndCallCall {
        #[allow(missing_docs)]
        pub permitBatch: alloy_sol_types::private::Vec<
            <IRouterCommon::PermitApproval as alloy_sol_types::SolType>::RustType,
        >,
        #[allow(missing_docs)]
        pub permitSignatures: alloy_sol_types::private::Vec<alloy_sol_types::private::Bytes>,
        #[allow(missing_docs)]
        pub permit2Batch: <IAllowanceTransfer::PermitBatch as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub permit2Signature: alloy_sol_types::private::Bytes,
        #[allow(missing_docs)]
        pub multicallData: alloy_sol_types::private::Vec<alloy_sol_types::private::Bytes>,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`permitBatchAndCall((address,address,address,uint256,uint256,
    /// uint256)[],bytes[],((address,uint160,uint48,uint48)[],address,uint256),
    /// bytes,bytes[])`](permitBatchAndCallCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct permitBatchAndCallReturn {
        #[allow(missing_docs)]
        pub results: alloy_sol_types::private::Vec<alloy_sol_types::private::Bytes>,
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
                alloy_sol_types::sol_data::Array<IRouterCommon::PermitApproval>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Bytes>,
                IAllowanceTransfer::PermitBatch,
                alloy_sol_types::sol_data::Bytes,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Bytes>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Vec<
                    <IRouterCommon::PermitApproval as alloy_sol_types::SolType>::RustType,
                >,
                alloy_sol_types::private::Vec<alloy_sol_types::private::Bytes>,
                <IAllowanceTransfer::PermitBatch as alloy_sol_types::SolType>::RustType,
                alloy_sol_types::private::Bytes,
                alloy_sol_types::private::Vec<alloy_sol_types::private::Bytes>,
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
            impl ::core::convert::From<permitBatchAndCallCall> for UnderlyingRustTuple<'_> {
                fn from(value: permitBatchAndCallCall) -> Self {
                    (
                        value.permitBatch,
                        value.permitSignatures,
                        value.permit2Batch,
                        value.permit2Signature,
                        value.multicallData,
                    )
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for permitBatchAndCallCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        permitBatch: tuple.0,
                        permitSignatures: tuple.1,
                        permit2Batch: tuple.2,
                        permit2Signature: tuple.3,
                        multicallData: tuple.4,
                    }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> =
                (alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Bytes>,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> =
                (alloy_sol_types::private::Vec<alloy_sol_types::private::Bytes>,);
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
            impl ::core::convert::From<permitBatchAndCallReturn> for UnderlyingRustTuple<'_> {
                fn from(value: permitBatchAndCallReturn) -> Self {
                    (value.results,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for permitBatchAndCallReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { results: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for permitBatchAndCallCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Array<IRouterCommon::PermitApproval>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Bytes>,
                IAllowanceTransfer::PermitBatch,
                alloy_sol_types::sol_data::Bytes,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Bytes>,
            );
            type Return = alloy_sol_types::private::Vec<alloy_sol_types::private::Bytes>;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> =
                (alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Bytes>,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [25u8, 198u8, 152u8, 159u8];
            const SIGNATURE: &'static str = "permitBatchAndCall((address,address,address,uint256,\
                                             uint256,uint256)[],bytes[],((address,uint160,uint48,\
                                             uint48)[],address,uint256),bytes,bytes[])";

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
                        IRouterCommon::PermitApproval,
                    > as alloy_sol_types::SolType>::tokenize(&self.permitBatch),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Bytes,
                    > as alloy_sol_types::SolType>::tokenize(&self.permitSignatures),
                    <IAllowanceTransfer::PermitBatch as alloy_sol_types::SolType>::tokenize(
                        &self.permit2Batch,
                    ),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.permit2Signature,
                    ),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Bytes,
                    > as alloy_sol_types::SolType>::tokenize(&self.multicallData),
                )
            }

            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                (<alloy_sol_types::sol_data::Array<
                    alloy_sol_types::sol_data::Bytes,
                > as alloy_sol_types::SolType>::tokenize(ret),)
            }

            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence(data).map(
                    |r| {
                        let r: permitBatchAndCallReturn = r.into();
                        r.results
                    },
                )
            }

            #[inline]
            fn abi_decode_returns_validate(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence_validate(
                    data,
                )
                .map(|r| {
                    let r: permitBatchAndCallReturn = r.into();
                    r.results
                })
            }
        }
    };
    #[derive()]
    /**Function with signature `querySwapExactIn((address,(address,address,bool)[],uint256,uint256)[],address,bytes)` and selector `0xe3b5dff4`.
    ```solidity
    function querySwapExactIn(IBatchRouter.SwapPathExactAmountIn[] memory paths, address sender, bytes memory userData) external returns (uint256[] memory pathAmountsOut, address[] memory tokensOut, uint256[] memory amountsOut);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct querySwapExactInCall {
        #[allow(missing_docs)]
        pub paths: alloy_sol_types::private::Vec<
            <IBatchRouter::SwapPathExactAmountIn as alloy_sol_types::SolType>::RustType,
        >,
        #[allow(missing_docs)]
        pub sender: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub userData: alloy_sol_types::private::Bytes,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`querySwapExactIn((address,(address,address,bool)[],uint256,uint256)[],
    /// address,bytes)`](querySwapExactInCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct querySwapExactInReturn {
        #[allow(missing_docs)]
        pub pathAmountsOut:
            alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U256>,
        #[allow(missing_docs)]
        pub tokensOut: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
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
                alloy_sol_types::sol_data::Array<IBatchRouter::SwapPathExactAmountIn>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Bytes,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Vec<
                    <IBatchRouter::SwapPathExactAmountIn as alloy_sol_types::SolType>::RustType,
                >,
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
            impl ::core::convert::From<querySwapExactInCall> for UnderlyingRustTuple<'_> {
                fn from(value: querySwapExactInCall) -> Self {
                    (value.paths, value.sender, value.userData)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for querySwapExactInCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        paths: tuple.0,
                        sender: tuple.1,
                        userData: tuple.2,
                    }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U256>,
                alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
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
            impl ::core::convert::From<querySwapExactInReturn> for UnderlyingRustTuple<'_> {
                fn from(value: querySwapExactInReturn) -> Self {
                    (value.pathAmountsOut, value.tokensOut, value.amountsOut)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for querySwapExactInReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        pathAmountsOut: tuple.0,
                        tokensOut: tuple.1,
                        amountsOut: tuple.2,
                    }
                }
            }
        }
        impl querySwapExactInReturn {
            fn _tokenize(
                &self,
            ) -> <querySwapExactInCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                (
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::SolType>::tokenize(&self.pathAmountsOut),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Address,
                    > as alloy_sol_types::SolType>::tokenize(&self.tokensOut),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::SolType>::tokenize(&self.amountsOut),
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for querySwapExactInCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Array<IBatchRouter::SwapPathExactAmountIn>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Bytes,
            );
            type Return = querySwapExactInReturn;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [227u8, 181u8, 223u8, 244u8];
            const SIGNATURE: &'static str = "querySwapExactIn((address,(address,address,bool)[],\
                                             uint256,uint256)[],address,bytes)";

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
                        IBatchRouter::SwapPathExactAmountIn,
                    > as alloy_sol_types::SolType>::tokenize(&self.paths),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.sender,
                    ),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.userData,
                    ),
                )
            }

            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                querySwapExactInReturn::_tokenize(ret)
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
    /**Function with signature `querySwapExactInHook((address,(address,(address,address,bool)[],uint256,uint256)[],uint256,bool,bytes))` and selector `0x8a12a08c`.
    ```solidity
    function querySwapExactInHook(IBatchRouter.SwapExactInHookParams memory params) external returns (uint256[] memory pathAmountsOut, address[] memory tokensOut, uint256[] memory amountsOut);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct querySwapExactInHookCall {
        #[allow(missing_docs)]
        pub params: <IBatchRouter::SwapExactInHookParams as alloy_sol_types::SolType>::RustType,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`querySwapExactInHook((address,(address,(address,address,bool)[],
    /// uint256,uint256)[],uint256,bool,bytes))`](querySwapExactInHookCall)
    /// function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct querySwapExactInHookReturn {
        #[allow(missing_docs)]
        pub pathAmountsOut:
            alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U256>,
        #[allow(missing_docs)]
        pub tokensOut: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
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
            type UnderlyingSolTuple<'a> = (IBatchRouter::SwapExactInHookParams,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> =
                (<IBatchRouter::SwapExactInHookParams as alloy_sol_types::SolType>::RustType,);
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
            impl ::core::convert::From<querySwapExactInHookCall> for UnderlyingRustTuple<'_> {
                fn from(value: querySwapExactInHookCall) -> Self {
                    (value.params,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for querySwapExactInHookCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { params: tuple.0 }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U256>,
                alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
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
            impl ::core::convert::From<querySwapExactInHookReturn> for UnderlyingRustTuple<'_> {
                fn from(value: querySwapExactInHookReturn) -> Self {
                    (value.pathAmountsOut, value.tokensOut, value.amountsOut)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for querySwapExactInHookReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        pathAmountsOut: tuple.0,
                        tokensOut: tuple.1,
                        amountsOut: tuple.2,
                    }
                }
            }
        }
        impl querySwapExactInHookReturn {
            fn _tokenize(
                &self,
            ) -> <querySwapExactInHookCall as alloy_sol_types::SolCall>::ReturnToken<'_>
            {
                (
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::SolType>::tokenize(&self.pathAmountsOut),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Address,
                    > as alloy_sol_types::SolType>::tokenize(&self.tokensOut),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::SolType>::tokenize(&self.amountsOut),
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for querySwapExactInHookCall {
            type Parameters<'a> = (IBatchRouter::SwapExactInHookParams,);
            type Return = querySwapExactInHookReturn;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [138u8, 18u8, 160u8, 140u8];
            const SIGNATURE: &'static str = "querySwapExactInHook((address,(address,(address,\
                                             address,bool)[],uint256,uint256)[],uint256,bool,\
                                             bytes))";

            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }

            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <IBatchRouter::SwapExactInHookParams as alloy_sol_types::SolType>::tokenize(
                        &self.params,
                    ),
                )
            }

            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                querySwapExactInHookReturn::_tokenize(ret)
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
    /**Function with signature `querySwapExactOut((address,(address,address,bool)[],uint256,uint256)[],address,bytes)` and selector `0x2950286e`.
    ```solidity
    function querySwapExactOut(IBatchRouter.SwapPathExactAmountOut[] memory paths, address sender, bytes memory userData) external returns (uint256[] memory pathAmountsIn, address[] memory tokensIn, uint256[] memory amountsIn);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct querySwapExactOutCall {
        #[allow(missing_docs)]
        pub paths: alloy_sol_types::private::Vec<
            <IBatchRouter::SwapPathExactAmountOut as alloy_sol_types::SolType>::RustType,
        >,
        #[allow(missing_docs)]
        pub sender: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub userData: alloy_sol_types::private::Bytes,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`querySwapExactOut((address,(address,address,bool)[],uint256,
    /// uint256)[],address,bytes)`](querySwapExactOutCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct querySwapExactOutReturn {
        #[allow(missing_docs)]
        pub pathAmountsIn:
            alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U256>,
        #[allow(missing_docs)]
        pub tokensIn: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
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
                alloy_sol_types::sol_data::Array<IBatchRouter::SwapPathExactAmountOut>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Bytes,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Vec<
                    <IBatchRouter::SwapPathExactAmountOut as alloy_sol_types::SolType>::RustType,
                >,
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
            impl ::core::convert::From<querySwapExactOutCall> for UnderlyingRustTuple<'_> {
                fn from(value: querySwapExactOutCall) -> Self {
                    (value.paths, value.sender, value.userData)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for querySwapExactOutCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        paths: tuple.0,
                        sender: tuple.1,
                        userData: tuple.2,
                    }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U256>,
                alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
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
            impl ::core::convert::From<querySwapExactOutReturn> for UnderlyingRustTuple<'_> {
                fn from(value: querySwapExactOutReturn) -> Self {
                    (value.pathAmountsIn, value.tokensIn, value.amountsIn)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for querySwapExactOutReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        pathAmountsIn: tuple.0,
                        tokensIn: tuple.1,
                        amountsIn: tuple.2,
                    }
                }
            }
        }
        impl querySwapExactOutReturn {
            fn _tokenize(
                &self,
            ) -> <querySwapExactOutCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                (
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::SolType>::tokenize(&self.pathAmountsIn),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Address,
                    > as alloy_sol_types::SolType>::tokenize(&self.tokensIn),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::SolType>::tokenize(&self.amountsIn),
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for querySwapExactOutCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Array<IBatchRouter::SwapPathExactAmountOut>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Bytes,
            );
            type Return = querySwapExactOutReturn;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [41u8, 80u8, 40u8, 110u8];
            const SIGNATURE: &'static str = "querySwapExactOut((address,(address,address,bool)[],\
                                             uint256,uint256)[],address,bytes)";

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
                        IBatchRouter::SwapPathExactAmountOut,
                    > as alloy_sol_types::SolType>::tokenize(&self.paths),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.sender,
                    ),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.userData,
                    ),
                )
            }

            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                querySwapExactOutReturn::_tokenize(ret)
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
    /**Function with signature `querySwapExactOutHook((address,(address,(address,address,bool)[],uint256,uint256)[],uint256,bool,bytes))` and selector `0x5a3c3987`.
    ```solidity
    function querySwapExactOutHook(IBatchRouter.SwapExactOutHookParams memory params) external returns (uint256[] memory pathAmountsIn, address[] memory tokensIn, uint256[] memory amountsIn);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct querySwapExactOutHookCall {
        #[allow(missing_docs)]
        pub params: <IBatchRouter::SwapExactOutHookParams as alloy_sol_types::SolType>::RustType,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`querySwapExactOutHook((address,(address,(address,address,bool)[],
    /// uint256,uint256)[],uint256,bool,bytes))`](querySwapExactOutHookCall)
    /// function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct querySwapExactOutHookReturn {
        #[allow(missing_docs)]
        pub pathAmountsIn:
            alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U256>,
        #[allow(missing_docs)]
        pub tokensIn: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
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
            type UnderlyingSolTuple<'a> = (IBatchRouter::SwapExactOutHookParams,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> =
                (<IBatchRouter::SwapExactOutHookParams as alloy_sol_types::SolType>::RustType,);
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
            impl ::core::convert::From<querySwapExactOutHookCall> for UnderlyingRustTuple<'_> {
                fn from(value: querySwapExactOutHookCall) -> Self {
                    (value.params,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for querySwapExactOutHookCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { params: tuple.0 }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U256>,
                alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
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
            impl ::core::convert::From<querySwapExactOutHookReturn> for UnderlyingRustTuple<'_> {
                fn from(value: querySwapExactOutHookReturn) -> Self {
                    (value.pathAmountsIn, value.tokensIn, value.amountsIn)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for querySwapExactOutHookReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        pathAmountsIn: tuple.0,
                        tokensIn: tuple.1,
                        amountsIn: tuple.2,
                    }
                }
            }
        }
        impl querySwapExactOutHookReturn {
            fn _tokenize(
                &self,
            ) -> <querySwapExactOutHookCall as alloy_sol_types::SolCall>::ReturnToken<'_>
            {
                (
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::SolType>::tokenize(&self.pathAmountsIn),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Address,
                    > as alloy_sol_types::SolType>::tokenize(&self.tokensIn),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::SolType>::tokenize(&self.amountsIn),
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for querySwapExactOutHookCall {
            type Parameters<'a> = (IBatchRouter::SwapExactOutHookParams,);
            type Return = querySwapExactOutHookReturn;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [90u8, 60u8, 57u8, 135u8];
            const SIGNATURE: &'static str = "querySwapExactOutHook((address,(address,(address,\
                                             address,bool)[],uint256,uint256)[],uint256,bool,\
                                             bytes))";

            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }

            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <IBatchRouter::SwapExactOutHookParams as alloy_sol_types::SolType>::tokenize(
                        &self.params,
                    ),
                )
            }

            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                querySwapExactOutHookReturn::_tokenize(ret)
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
    /**Function with signature `swapExactIn((address,(address,address,bool)[],uint256,uint256)[],uint256,bool,bytes)` and selector `0x286f580d`.
    ```solidity
    function swapExactIn(IBatchRouter.SwapPathExactAmountIn[] memory paths, uint256 deadline, bool wethIsEth, bytes memory userData) external payable returns (uint256[] memory pathAmountsOut, address[] memory tokensOut, uint256[] memory amountsOut);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct swapExactInCall {
        #[allow(missing_docs)]
        pub paths: alloy_sol_types::private::Vec<
            <IBatchRouter::SwapPathExactAmountIn as alloy_sol_types::SolType>::RustType,
        >,
        #[allow(missing_docs)]
        pub deadline: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub wethIsEth: bool,
        #[allow(missing_docs)]
        pub userData: alloy_sol_types::private::Bytes,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`swapExactIn((address,(address,address,bool)[],uint256,uint256)[],
    /// uint256,bool,bytes)`](swapExactInCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct swapExactInReturn {
        #[allow(missing_docs)]
        pub pathAmountsOut:
            alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U256>,
        #[allow(missing_docs)]
        pub tokensOut: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
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
                alloy_sol_types::sol_data::Array<IBatchRouter::SwapPathExactAmountIn>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Bool,
                alloy_sol_types::sol_data::Bytes,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Vec<
                    <IBatchRouter::SwapPathExactAmountIn as alloy_sol_types::SolType>::RustType,
                >,
                alloy_sol_types::private::primitives::aliases::U256,
                bool,
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
            impl ::core::convert::From<swapExactInCall> for UnderlyingRustTuple<'_> {
                fn from(value: swapExactInCall) -> Self {
                    (value.paths, value.deadline, value.wethIsEth, value.userData)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for swapExactInCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        paths: tuple.0,
                        deadline: tuple.1,
                        wethIsEth: tuple.2,
                        userData: tuple.3,
                    }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U256>,
                alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
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
            impl ::core::convert::From<swapExactInReturn> for UnderlyingRustTuple<'_> {
                fn from(value: swapExactInReturn) -> Self {
                    (value.pathAmountsOut, value.tokensOut, value.amountsOut)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for swapExactInReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        pathAmountsOut: tuple.0,
                        tokensOut: tuple.1,
                        amountsOut: tuple.2,
                    }
                }
            }
        }
        impl swapExactInReturn {
            fn _tokenize(&self) -> <swapExactInCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                (
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::SolType>::tokenize(&self.pathAmountsOut),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Address,
                    > as alloy_sol_types::SolType>::tokenize(&self.tokensOut),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::SolType>::tokenize(&self.amountsOut),
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for swapExactInCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Array<IBatchRouter::SwapPathExactAmountIn>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Bool,
                alloy_sol_types::sol_data::Bytes,
            );
            type Return = swapExactInReturn;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [40u8, 111u8, 88u8, 13u8];
            const SIGNATURE: &'static str = "swapExactIn((address,(address,address,bool)[],\
                                             uint256,uint256)[],uint256,bool,bytes)";

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
                        IBatchRouter::SwapPathExactAmountIn,
                    > as alloy_sol_types::SolType>::tokenize(&self.paths),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.deadline),
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::tokenize(
                        &self.wethIsEth,
                    ),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.userData,
                    ),
                )
            }

            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                swapExactInReturn::_tokenize(ret)
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
    /**Function with signature `swapExactInHook((address,(address,(address,address,bool)[],uint256,uint256)[],uint256,bool,bytes))` and selector `0x08a465f6`.
    ```solidity
    function swapExactInHook(IBatchRouter.SwapExactInHookParams memory params) external returns (uint256[] memory pathAmountsOut, address[] memory tokensOut, uint256[] memory amountsOut);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct swapExactInHookCall {
        #[allow(missing_docs)]
        pub params: <IBatchRouter::SwapExactInHookParams as alloy_sol_types::SolType>::RustType,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`swapExactInHook((address,(address,(address,address,bool)[],uint256,
    /// uint256)[],uint256,bool,bytes))`](swapExactInHookCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct swapExactInHookReturn {
        #[allow(missing_docs)]
        pub pathAmountsOut:
            alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U256>,
        #[allow(missing_docs)]
        pub tokensOut: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
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
            type UnderlyingSolTuple<'a> = (IBatchRouter::SwapExactInHookParams,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> =
                (<IBatchRouter::SwapExactInHookParams as alloy_sol_types::SolType>::RustType,);
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
            impl ::core::convert::From<swapExactInHookCall> for UnderlyingRustTuple<'_> {
                fn from(value: swapExactInHookCall) -> Self {
                    (value.params,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for swapExactInHookCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { params: tuple.0 }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U256>,
                alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
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
            impl ::core::convert::From<swapExactInHookReturn> for UnderlyingRustTuple<'_> {
                fn from(value: swapExactInHookReturn) -> Self {
                    (value.pathAmountsOut, value.tokensOut, value.amountsOut)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for swapExactInHookReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        pathAmountsOut: tuple.0,
                        tokensOut: tuple.1,
                        amountsOut: tuple.2,
                    }
                }
            }
        }
        impl swapExactInHookReturn {
            fn _tokenize(
                &self,
            ) -> <swapExactInHookCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                (
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::SolType>::tokenize(&self.pathAmountsOut),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Address,
                    > as alloy_sol_types::SolType>::tokenize(&self.tokensOut),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::SolType>::tokenize(&self.amountsOut),
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for swapExactInHookCall {
            type Parameters<'a> = (IBatchRouter::SwapExactInHookParams,);
            type Return = swapExactInHookReturn;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [8u8, 164u8, 101u8, 246u8];
            const SIGNATURE: &'static str = "swapExactInHook((address,(address,(address,address,\
                                             bool)[],uint256,uint256)[],uint256,bool,bytes))";

            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }

            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <IBatchRouter::SwapExactInHookParams as alloy_sol_types::SolType>::tokenize(
                        &self.params,
                    ),
                )
            }

            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                swapExactInHookReturn::_tokenize(ret)
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
    /**Function with signature `swapExactOut((address,(address,address,bool)[],uint256,uint256)[],uint256,bool,bytes)` and selector `0x8eb1b65e`.
    ```solidity
    function swapExactOut(IBatchRouter.SwapPathExactAmountOut[] memory paths, uint256 deadline, bool wethIsEth, bytes memory userData) external payable returns (uint256[] memory pathAmountsIn, address[] memory tokensIn, uint256[] memory amountsIn);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct swapExactOutCall {
        #[allow(missing_docs)]
        pub paths: alloy_sol_types::private::Vec<
            <IBatchRouter::SwapPathExactAmountOut as alloy_sol_types::SolType>::RustType,
        >,
        #[allow(missing_docs)]
        pub deadline: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub wethIsEth: bool,
        #[allow(missing_docs)]
        pub userData: alloy_sol_types::private::Bytes,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`swapExactOut((address,(address,address,bool)[],uint256,uint256)[],
    /// uint256,bool,bytes)`](swapExactOutCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct swapExactOutReturn {
        #[allow(missing_docs)]
        pub pathAmountsIn:
            alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U256>,
        #[allow(missing_docs)]
        pub tokensIn: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
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
                alloy_sol_types::sol_data::Array<IBatchRouter::SwapPathExactAmountOut>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Bool,
                alloy_sol_types::sol_data::Bytes,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Vec<
                    <IBatchRouter::SwapPathExactAmountOut as alloy_sol_types::SolType>::RustType,
                >,
                alloy_sol_types::private::primitives::aliases::U256,
                bool,
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
            impl ::core::convert::From<swapExactOutCall> for UnderlyingRustTuple<'_> {
                fn from(value: swapExactOutCall) -> Self {
                    (value.paths, value.deadline, value.wethIsEth, value.userData)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for swapExactOutCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        paths: tuple.0,
                        deadline: tuple.1,
                        wethIsEth: tuple.2,
                        userData: tuple.3,
                    }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U256>,
                alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
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
            impl ::core::convert::From<swapExactOutReturn> for UnderlyingRustTuple<'_> {
                fn from(value: swapExactOutReturn) -> Self {
                    (value.pathAmountsIn, value.tokensIn, value.amountsIn)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for swapExactOutReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        pathAmountsIn: tuple.0,
                        tokensIn: tuple.1,
                        amountsIn: tuple.2,
                    }
                }
            }
        }
        impl swapExactOutReturn {
            fn _tokenize(&self) -> <swapExactOutCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                (
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::SolType>::tokenize(&self.pathAmountsIn),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Address,
                    > as alloy_sol_types::SolType>::tokenize(&self.tokensIn),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::SolType>::tokenize(&self.amountsIn),
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for swapExactOutCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Array<IBatchRouter::SwapPathExactAmountOut>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Bool,
                alloy_sol_types::sol_data::Bytes,
            );
            type Return = swapExactOutReturn;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [142u8, 177u8, 182u8, 94u8];
            const SIGNATURE: &'static str = "swapExactOut((address,(address,address,bool)[],\
                                             uint256,uint256)[],uint256,bool,bytes)";

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
                        IBatchRouter::SwapPathExactAmountOut,
                    > as alloy_sol_types::SolType>::tokenize(&self.paths),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.deadline),
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::tokenize(
                        &self.wethIsEth,
                    ),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.userData,
                    ),
                )
            }

            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                swapExactOutReturn::_tokenize(ret)
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
    /**Function with signature `swapExactOutHook((address,(address,(address,address,bool)[],uint256,uint256)[],uint256,bool,bytes))` and selector `0x945ed33f`.
    ```solidity
    function swapExactOutHook(IBatchRouter.SwapExactOutHookParams memory params) external returns (uint256[] memory pathAmountsIn, address[] memory tokensIn, uint256[] memory amountsIn);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct swapExactOutHookCall {
        #[allow(missing_docs)]
        pub params: <IBatchRouter::SwapExactOutHookParams as alloy_sol_types::SolType>::RustType,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`swapExactOutHook((address,(address,(address,address,bool)[],uint256,
    /// uint256)[],uint256,bool,bytes))`](swapExactOutHookCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct swapExactOutHookReturn {
        #[allow(missing_docs)]
        pub pathAmountsIn:
            alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U256>,
        #[allow(missing_docs)]
        pub tokensIn: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
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
            type UnderlyingSolTuple<'a> = (IBatchRouter::SwapExactOutHookParams,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> =
                (<IBatchRouter::SwapExactOutHookParams as alloy_sol_types::SolType>::RustType,);
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
            impl ::core::convert::From<swapExactOutHookCall> for UnderlyingRustTuple<'_> {
                fn from(value: swapExactOutHookCall) -> Self {
                    (value.params,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for swapExactOutHookCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { params: tuple.0 }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Vec<alloy_sol_types::private::primitives::aliases::U256>,
                alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
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
            impl ::core::convert::From<swapExactOutHookReturn> for UnderlyingRustTuple<'_> {
                fn from(value: swapExactOutHookReturn) -> Self {
                    (value.pathAmountsIn, value.tokensIn, value.amountsIn)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for swapExactOutHookReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        pathAmountsIn: tuple.0,
                        tokensIn: tuple.1,
                        amountsIn: tuple.2,
                    }
                }
            }
        }
        impl swapExactOutHookReturn {
            fn _tokenize(
                &self,
            ) -> <swapExactOutHookCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                (
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::SolType>::tokenize(&self.pathAmountsIn),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Address,
                    > as alloy_sol_types::SolType>::tokenize(&self.tokensIn),
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Uint<256>,
                    > as alloy_sol_types::SolType>::tokenize(&self.amountsIn),
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for swapExactOutHookCall {
            type Parameters<'a> = (IBatchRouter::SwapExactOutHookParams,);
            type Return = swapExactOutHookReturn;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Uint<256>>,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [148u8, 94u8, 211u8, 63u8];
            const SIGNATURE: &'static str = "swapExactOutHook((address,(address,(address,address,\
                                             bool)[],uint256,uint256)[],uint256,bool,bytes))";

            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }

            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <IBatchRouter::SwapExactOutHookParams as alloy_sol_types::SolType>::tokenize(
                        &self.params,
                    ),
                )
            }

            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                swapExactOutHookReturn::_tokenize(ret)
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
    /**Function with signature `version()` and selector `0x54fd4d50`.
    ```solidity
    function version() external view returns (string memory);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct versionCall;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`version()`](versionCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct versionReturn {
        #[allow(missing_docs)]
        pub _0: alloy_sol_types::private::String,
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
            impl ::core::convert::From<versionCall> for UnderlyingRustTuple<'_> {
                fn from(value: versionCall) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for versionCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (alloy_sol_types::sol_data::String,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (alloy_sol_types::private::String,);
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
            impl ::core::convert::From<versionReturn> for UnderlyingRustTuple<'_> {
                fn from(value: versionReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for versionReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for versionCall {
            type Parameters<'a> = ();
            type Return = alloy_sol_types::private::String;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::String,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [84u8, 253u8, 77u8, 80u8];
            const SIGNATURE: &'static str = "version()";

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
                (<alloy_sol_types::sol_data::String as alloy_sol_types::SolType>::tokenize(ret),)
            }

            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence(data).map(
                    |r| {
                        let r: versionReturn = r.into();
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
                    let r: versionReturn = r.into();
                    r._0
                })
            }
        }
    };
    ///Container for all the [`BalancerV3BatchRouter`](self) function calls.
    #[derive(Clone)]
    pub enum BalancerV3BatchRouterCalls {
        #[allow(missing_docs)]
        getSender(getSenderCall),
        #[allow(missing_docs)]
        multicall(multicallCall),
        #[allow(missing_docs)]
        permitBatchAndCall(permitBatchAndCallCall),
        #[allow(missing_docs)]
        querySwapExactIn(querySwapExactInCall),
        #[allow(missing_docs)]
        querySwapExactInHook(querySwapExactInHookCall),
        #[allow(missing_docs)]
        querySwapExactOut(querySwapExactOutCall),
        #[allow(missing_docs)]
        querySwapExactOutHook(querySwapExactOutHookCall),
        #[allow(missing_docs)]
        swapExactIn(swapExactInCall),
        #[allow(missing_docs)]
        swapExactInHook(swapExactInHookCall),
        #[allow(missing_docs)]
        swapExactOut(swapExactOutCall),
        #[allow(missing_docs)]
        swapExactOutHook(swapExactOutHookCall),
        #[allow(missing_docs)]
        version(versionCall),
    }
    impl BalancerV3BatchRouterCalls {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the
        /// variants. No guarantees are made about the order of the
        /// selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 4usize]] = &[
            [8u8, 164u8, 101u8, 246u8],
            [25u8, 198u8, 152u8, 159u8],
            [40u8, 111u8, 88u8, 13u8],
            [41u8, 80u8, 40u8, 110u8],
            [84u8, 253u8, 77u8, 80u8],
            [90u8, 60u8, 57u8, 135u8],
            [94u8, 1u8, 235u8, 90u8],
            [138u8, 18u8, 160u8, 140u8],
            [142u8, 177u8, 182u8, 94u8],
            [148u8, 94u8, 211u8, 63u8],
            [172u8, 150u8, 80u8, 216u8],
            [227u8, 181u8, 223u8, 244u8],
        ];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <swapExactInHookCall as alloy_sol_types::SolCall>::SIGNATURE,
            <permitBatchAndCallCall as alloy_sol_types::SolCall>::SIGNATURE,
            <swapExactInCall as alloy_sol_types::SolCall>::SIGNATURE,
            <querySwapExactOutCall as alloy_sol_types::SolCall>::SIGNATURE,
            <versionCall as alloy_sol_types::SolCall>::SIGNATURE,
            <querySwapExactOutHookCall as alloy_sol_types::SolCall>::SIGNATURE,
            <getSenderCall as alloy_sol_types::SolCall>::SIGNATURE,
            <querySwapExactInHookCall as alloy_sol_types::SolCall>::SIGNATURE,
            <swapExactOutCall as alloy_sol_types::SolCall>::SIGNATURE,
            <swapExactOutHookCall as alloy_sol_types::SolCall>::SIGNATURE,
            <multicallCall as alloy_sol_types::SolCall>::SIGNATURE,
            <querySwapExactInCall as alloy_sol_types::SolCall>::SIGNATURE,
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(swapExactInHook),
            ::core::stringify!(permitBatchAndCall),
            ::core::stringify!(swapExactIn),
            ::core::stringify!(querySwapExactOut),
            ::core::stringify!(version),
            ::core::stringify!(querySwapExactOutHook),
            ::core::stringify!(getSender),
            ::core::stringify!(querySwapExactInHook),
            ::core::stringify!(swapExactOut),
            ::core::stringify!(swapExactOutHook),
            ::core::stringify!(multicall),
            ::core::stringify!(querySwapExactIn),
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
    impl alloy_sol_types::SolInterface for BalancerV3BatchRouterCalls {
        const COUNT: usize = 12usize;
        const MIN_DATA_LENGTH: usize = 0usize;
        const NAME: &'static str = "BalancerV3BatchRouterCalls";

        #[inline]
        fn selector(&self) -> [u8; 4] {
            match self {
                Self::getSender(_) => <getSenderCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::multicall(_) => <multicallCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::permitBatchAndCall(_) => {
                    <permitBatchAndCallCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::querySwapExactIn(_) => {
                    <querySwapExactInCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::querySwapExactInHook(_) => {
                    <querySwapExactInHookCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::querySwapExactOut(_) => {
                    <querySwapExactOutCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::querySwapExactOutHook(_) => {
                    <querySwapExactOutHookCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::swapExactIn(_) => <swapExactInCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::swapExactInHook(_) => {
                    <swapExactInHookCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::swapExactOut(_) => <swapExactOutCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::swapExactOutHook(_) => {
                    <swapExactOutHookCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::version(_) => <versionCall as alloy_sol_types::SolCall>::SELECTOR,
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
                -> alloy_sol_types::Result<BalancerV3BatchRouterCalls>] = &[
                {
                    fn swapExactInHook(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterCalls> {
                        <swapExactInHookCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(BalancerV3BatchRouterCalls::swapExactInHook)
                    }
                    swapExactInHook
                },
                {
                    fn permitBatchAndCall(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterCalls> {
                        <permitBatchAndCallCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(BalancerV3BatchRouterCalls::permitBatchAndCall)
                    }
                    permitBatchAndCall
                },
                {
                    fn swapExactIn(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterCalls> {
                        <swapExactInCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(BalancerV3BatchRouterCalls::swapExactIn)
                    }
                    swapExactIn
                },
                {
                    fn querySwapExactOut(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterCalls> {
                        <querySwapExactOutCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(BalancerV3BatchRouterCalls::querySwapExactOut)
                    }
                    querySwapExactOut
                },
                {
                    fn version(data: &[u8]) -> alloy_sol_types::Result<BalancerV3BatchRouterCalls> {
                        <versionCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(BalancerV3BatchRouterCalls::version)
                    }
                    version
                },
                {
                    fn querySwapExactOutHook(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterCalls> {
                        <querySwapExactOutHookCall as alloy_sol_types::SolCall>::abi_decode_raw(
                            data,
                        )
                        .map(BalancerV3BatchRouterCalls::querySwapExactOutHook)
                    }
                    querySwapExactOutHook
                },
                {
                    fn getSender(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterCalls> {
                        <getSenderCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(BalancerV3BatchRouterCalls::getSender)
                    }
                    getSender
                },
                {
                    fn querySwapExactInHook(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterCalls> {
                        <querySwapExactInHookCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(BalancerV3BatchRouterCalls::querySwapExactInHook)
                    }
                    querySwapExactInHook
                },
                {
                    fn swapExactOut(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterCalls> {
                        <swapExactOutCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(BalancerV3BatchRouterCalls::swapExactOut)
                    }
                    swapExactOut
                },
                {
                    fn swapExactOutHook(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterCalls> {
                        <swapExactOutHookCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(BalancerV3BatchRouterCalls::swapExactOutHook)
                    }
                    swapExactOutHook
                },
                {
                    fn multicall(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterCalls> {
                        <multicallCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(BalancerV3BatchRouterCalls::multicall)
                    }
                    multicall
                },
                {
                    fn querySwapExactIn(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterCalls> {
                        <querySwapExactInCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(BalancerV3BatchRouterCalls::querySwapExactIn)
                    }
                    querySwapExactIn
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
                BalancerV3BatchRouterCalls,
            >] = &[
                {
                    fn swapExactInHook(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterCalls> {
                        <swapExactInHookCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(BalancerV3BatchRouterCalls::swapExactInHook)
                    }
                    swapExactInHook
                },
                {
                    fn permitBatchAndCall(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterCalls> {
                        <permitBatchAndCallCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(BalancerV3BatchRouterCalls::permitBatchAndCall)
                    }
                    permitBatchAndCall
                },
                {
                    fn swapExactIn(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterCalls> {
                        <swapExactInCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(BalancerV3BatchRouterCalls::swapExactIn)
                    }
                    swapExactIn
                },
                {
                    fn querySwapExactOut(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterCalls> {
                        <querySwapExactOutCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(BalancerV3BatchRouterCalls::querySwapExactOut)
                    }
                    querySwapExactOut
                },
                {
                    fn version(data: &[u8]) -> alloy_sol_types::Result<BalancerV3BatchRouterCalls> {
                        <versionCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(BalancerV3BatchRouterCalls::version)
                    }
                    version
                },
                {
                    fn querySwapExactOutHook(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterCalls> {
                        <querySwapExactOutHookCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(BalancerV3BatchRouterCalls::querySwapExactOutHook)
                    }
                    querySwapExactOutHook
                },
                {
                    fn getSender(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterCalls> {
                        <getSenderCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(BalancerV3BatchRouterCalls::getSender)
                    }
                    getSender
                },
                {
                    fn querySwapExactInHook(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterCalls> {
                        <querySwapExactInHookCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(BalancerV3BatchRouterCalls::querySwapExactInHook)
                    }
                    querySwapExactInHook
                },
                {
                    fn swapExactOut(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterCalls> {
                        <swapExactOutCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(BalancerV3BatchRouterCalls::swapExactOut)
                    }
                    swapExactOut
                },
                {
                    fn swapExactOutHook(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterCalls> {
                        <swapExactOutHookCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(BalancerV3BatchRouterCalls::swapExactOutHook)
                    }
                    swapExactOutHook
                },
                {
                    fn multicall(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterCalls> {
                        <multicallCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(BalancerV3BatchRouterCalls::multicall)
                    }
                    multicall
                },
                {
                    fn querySwapExactIn(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterCalls> {
                        <querySwapExactInCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(BalancerV3BatchRouterCalls::querySwapExactIn)
                    }
                    querySwapExactIn
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
                Self::getSender(inner) => {
                    <getSenderCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::multicall(inner) => {
                    <multicallCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::permitBatchAndCall(inner) => {
                    <permitBatchAndCallCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::querySwapExactIn(inner) => {
                    <querySwapExactInCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::querySwapExactInHook(inner) => {
                    <querySwapExactInHookCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::querySwapExactOut(inner) => {
                    <querySwapExactOutCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::querySwapExactOutHook(inner) => {
                    <querySwapExactOutHookCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::swapExactIn(inner) => {
                    <swapExactInCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::swapExactInHook(inner) => {
                    <swapExactInHookCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::swapExactOut(inner) => {
                    <swapExactOutCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::swapExactOutHook(inner) => {
                    <swapExactOutHookCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::version(inner) => {
                    <versionCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
            }
        }

        #[inline]
        fn abi_encode_raw(&self, out: &mut alloy_sol_types::private::Vec<u8>) {
            match self {
                Self::getSender(inner) => {
                    <getSenderCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::multicall(inner) => {
                    <multicallCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::permitBatchAndCall(inner) => {
                    <permitBatchAndCallCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::querySwapExactIn(inner) => {
                    <querySwapExactInCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::querySwapExactInHook(inner) => {
                    <querySwapExactInHookCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner, out,
                    )
                }
                Self::querySwapExactOut(inner) => {
                    <querySwapExactOutCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::querySwapExactOutHook(inner) => {
                    <querySwapExactOutHookCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner, out,
                    )
                }
                Self::swapExactIn(inner) => {
                    <swapExactInCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::swapExactInHook(inner) => {
                    <swapExactInHookCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::swapExactOut(inner) => {
                    <swapExactOutCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::swapExactOutHook(inner) => {
                    <swapExactOutHookCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::version(inner) => {
                    <versionCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
            }
        }
    }
    ///Container for all the [`BalancerV3BatchRouter`](self) custom errors.
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    pub enum BalancerV3BatchRouterErrors {
        #[allow(missing_docs)]
        AddressEmptyCode(AddressEmptyCode),
        #[allow(missing_docs)]
        AddressInsufficientBalance(AddressInsufficientBalance),
        #[allow(missing_docs)]
        ErrorSelectorNotFound(ErrorSelectorNotFound),
        #[allow(missing_docs)]
        EthTransfer(EthTransfer),
        #[allow(missing_docs)]
        FailedInnerCall(FailedInnerCall),
        #[allow(missing_docs)]
        InputLengthMismatch(InputLengthMismatch),
        #[allow(missing_docs)]
        InsufficientEth(InsufficientEth),
        #[allow(missing_docs)]
        ReentrancyGuardReentrantCall(ReentrancyGuardReentrantCall),
        #[allow(missing_docs)]
        SafeCastOverflowedUintDowncast(SafeCastOverflowedUintDowncast),
        #[allow(missing_docs)]
        SafeERC20FailedOperation(SafeERC20FailedOperation),
        #[allow(missing_docs)]
        SenderIsNotVault(SenderIsNotVault),
        #[allow(missing_docs)]
        SwapDeadline(SwapDeadline),
        #[allow(missing_docs)]
        TransientIndexOutOfBounds(TransientIndexOutOfBounds),
    }
    impl BalancerV3BatchRouterErrors {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the
        /// variants. No guarantees are made about the order of the
        /// selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 4usize]] = &[
            [5u8, 64u8, 221u8, 246u8],
            [8u8, 150u8, 118u8, 213u8],
            [15u8, 74u8, 224u8, 228u8],
            [20u8, 37u8, 234u8, 66u8],
            [62u8, 229u8, 174u8, 181u8],
            [82u8, 116u8, 175u8, 231u8],
            [109u8, 252u8, 198u8, 80u8],
            [153u8, 150u8, 179u8, 21u8],
            [160u8, 26u8, 157u8, 246u8],
            [167u8, 40u8, 86u8, 137u8],
            [170u8, 173u8, 19u8, 247u8],
            [205u8, 120u8, 96u8, 89u8],
            [224u8, 139u8, 138u8, 240u8],
        ];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <EthTransfer as alloy_sol_types::SolError>::SIGNATURE,
            <SenderIsNotVault as alloy_sol_types::SolError>::SIGNATURE,
            <TransientIndexOutOfBounds as alloy_sol_types::SolError>::SIGNATURE,
            <FailedInnerCall as alloy_sol_types::SolError>::SIGNATURE,
            <ReentrancyGuardReentrantCall as alloy_sol_types::SolError>::SIGNATURE,
            <SafeERC20FailedOperation as alloy_sol_types::SolError>::SIGNATURE,
            <SafeCastOverflowedUintDowncast as alloy_sol_types::SolError>::SIGNATURE,
            <AddressEmptyCode as alloy_sol_types::SolError>::SIGNATURE,
            <InsufficientEth as alloy_sol_types::SolError>::SIGNATURE,
            <ErrorSelectorNotFound as alloy_sol_types::SolError>::SIGNATURE,
            <InputLengthMismatch as alloy_sol_types::SolError>::SIGNATURE,
            <AddressInsufficientBalance as alloy_sol_types::SolError>::SIGNATURE,
            <SwapDeadline as alloy_sol_types::SolError>::SIGNATURE,
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(EthTransfer),
            ::core::stringify!(SenderIsNotVault),
            ::core::stringify!(TransientIndexOutOfBounds),
            ::core::stringify!(FailedInnerCall),
            ::core::stringify!(ReentrancyGuardReentrantCall),
            ::core::stringify!(SafeERC20FailedOperation),
            ::core::stringify!(SafeCastOverflowedUintDowncast),
            ::core::stringify!(AddressEmptyCode),
            ::core::stringify!(InsufficientEth),
            ::core::stringify!(ErrorSelectorNotFound),
            ::core::stringify!(InputLengthMismatch),
            ::core::stringify!(AddressInsufficientBalance),
            ::core::stringify!(SwapDeadline),
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
    impl alloy_sol_types::SolInterface for BalancerV3BatchRouterErrors {
        const COUNT: usize = 13usize;
        const MIN_DATA_LENGTH: usize = 0usize;
        const NAME: &'static str = "BalancerV3BatchRouterErrors";

        #[inline]
        fn selector(&self) -> [u8; 4] {
            match self {
                Self::AddressEmptyCode(_) => {
                    <AddressEmptyCode as alloy_sol_types::SolError>::SELECTOR
                }
                Self::AddressInsufficientBalance(_) => {
                    <AddressInsufficientBalance as alloy_sol_types::SolError>::SELECTOR
                }
                Self::ErrorSelectorNotFound(_) => {
                    <ErrorSelectorNotFound as alloy_sol_types::SolError>::SELECTOR
                }
                Self::EthTransfer(_) => <EthTransfer as alloy_sol_types::SolError>::SELECTOR,
                Self::FailedInnerCall(_) => {
                    <FailedInnerCall as alloy_sol_types::SolError>::SELECTOR
                }
                Self::InputLengthMismatch(_) => {
                    <InputLengthMismatch as alloy_sol_types::SolError>::SELECTOR
                }
                Self::InsufficientEth(_) => {
                    <InsufficientEth as alloy_sol_types::SolError>::SELECTOR
                }
                Self::ReentrancyGuardReentrantCall(_) => {
                    <ReentrancyGuardReentrantCall as alloy_sol_types::SolError>::SELECTOR
                }
                Self::SafeCastOverflowedUintDowncast(_) => {
                    <SafeCastOverflowedUintDowncast as alloy_sol_types::SolError>::SELECTOR
                }
                Self::SafeERC20FailedOperation(_) => {
                    <SafeERC20FailedOperation as alloy_sol_types::SolError>::SELECTOR
                }
                Self::SenderIsNotVault(_) => {
                    <SenderIsNotVault as alloy_sol_types::SolError>::SELECTOR
                }
                Self::SwapDeadline(_) => <SwapDeadline as alloy_sol_types::SolError>::SELECTOR,
                Self::TransientIndexOutOfBounds(_) => {
                    <TransientIndexOutOfBounds as alloy_sol_types::SolError>::SELECTOR
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
                -> alloy_sol_types::Result<BalancerV3BatchRouterErrors>] = &[
                {
                    fn EthTransfer(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterErrors> {
                        <EthTransfer as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(BalancerV3BatchRouterErrors::EthTransfer)
                    }
                    EthTransfer
                },
                {
                    fn SenderIsNotVault(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterErrors> {
                        <SenderIsNotVault as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(BalancerV3BatchRouterErrors::SenderIsNotVault)
                    }
                    SenderIsNotVault
                },
                {
                    fn TransientIndexOutOfBounds(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterErrors> {
                        <TransientIndexOutOfBounds as alloy_sol_types::SolError>::abi_decode_raw(
                            data,
                        )
                        .map(BalancerV3BatchRouterErrors::TransientIndexOutOfBounds)
                    }
                    TransientIndexOutOfBounds
                },
                {
                    fn FailedInnerCall(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterErrors> {
                        <FailedInnerCall as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(BalancerV3BatchRouterErrors::FailedInnerCall)
                    }
                    FailedInnerCall
                },
                {
                    fn ReentrancyGuardReentrantCall(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterErrors> {
                        <ReentrancyGuardReentrantCall as alloy_sol_types::SolError>::abi_decode_raw(
                            data,
                        )
                        .map(BalancerV3BatchRouterErrors::ReentrancyGuardReentrantCall)
                    }
                    ReentrancyGuardReentrantCall
                },
                {
                    fn SafeERC20FailedOperation(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterErrors> {
                        <SafeERC20FailedOperation as alloy_sol_types::SolError>::abi_decode_raw(
                            data,
                        )
                        .map(BalancerV3BatchRouterErrors::SafeERC20FailedOperation)
                    }
                    SafeERC20FailedOperation
                },
                {
                    fn SafeCastOverflowedUintDowncast(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterErrors> {
                        <SafeCastOverflowedUintDowncast as alloy_sol_types::SolError>::abi_decode_raw(
                                data,
                            )
                            .map(
                                BalancerV3BatchRouterErrors::SafeCastOverflowedUintDowncast,
                            )
                    }
                    SafeCastOverflowedUintDowncast
                },
                {
                    fn AddressEmptyCode(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterErrors> {
                        <AddressEmptyCode as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(BalancerV3BatchRouterErrors::AddressEmptyCode)
                    }
                    AddressEmptyCode
                },
                {
                    fn InsufficientEth(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterErrors> {
                        <InsufficientEth as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(BalancerV3BatchRouterErrors::InsufficientEth)
                    }
                    InsufficientEth
                },
                {
                    fn ErrorSelectorNotFound(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterErrors> {
                        <ErrorSelectorNotFound as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(BalancerV3BatchRouterErrors::ErrorSelectorNotFound)
                    }
                    ErrorSelectorNotFound
                },
                {
                    fn InputLengthMismatch(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterErrors> {
                        <InputLengthMismatch as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(BalancerV3BatchRouterErrors::InputLengthMismatch)
                    }
                    InputLengthMismatch
                },
                {
                    fn AddressInsufficientBalance(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterErrors> {
                        <AddressInsufficientBalance as alloy_sol_types::SolError>::abi_decode_raw(
                            data,
                        )
                        .map(BalancerV3BatchRouterErrors::AddressInsufficientBalance)
                    }
                    AddressInsufficientBalance
                },
                {
                    fn SwapDeadline(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterErrors> {
                        <SwapDeadline as alloy_sol_types::SolError>::abi_decode_raw(data)
                            .map(BalancerV3BatchRouterErrors::SwapDeadline)
                    }
                    SwapDeadline
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
                BalancerV3BatchRouterErrors,
            >] = &[
                {
                    fn EthTransfer(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterErrors> {
                        <EthTransfer as alloy_sol_types::SolError>::abi_decode_raw_validate(data)
                            .map(BalancerV3BatchRouterErrors::EthTransfer)
                    }
                    EthTransfer
                },
                {
                    fn SenderIsNotVault(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterErrors> {
                        <SenderIsNotVault as alloy_sol_types::SolError>::abi_decode_raw_validate(
                            data,
                        )
                        .map(BalancerV3BatchRouterErrors::SenderIsNotVault)
                    }
                    SenderIsNotVault
                },
                {
                    fn TransientIndexOutOfBounds(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterErrors> {
                        <TransientIndexOutOfBounds as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(BalancerV3BatchRouterErrors::TransientIndexOutOfBounds)
                    }
                    TransientIndexOutOfBounds
                },
                {
                    fn FailedInnerCall(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterErrors> {
                        <FailedInnerCall as alloy_sol_types::SolError>::abi_decode_raw_validate(
                            data,
                        )
                        .map(BalancerV3BatchRouterErrors::FailedInnerCall)
                    }
                    FailedInnerCall
                },
                {
                    fn ReentrancyGuardReentrantCall(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterErrors> {
                        <ReentrancyGuardReentrantCall as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(
                                BalancerV3BatchRouterErrors::ReentrancyGuardReentrantCall,
                            )
                    }
                    ReentrancyGuardReentrantCall
                },
                {
                    fn SafeERC20FailedOperation(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterErrors> {
                        <SafeERC20FailedOperation as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(BalancerV3BatchRouterErrors::SafeERC20FailedOperation)
                    }
                    SafeERC20FailedOperation
                },
                {
                    fn SafeCastOverflowedUintDowncast(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterErrors> {
                        <SafeCastOverflowedUintDowncast as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(
                                BalancerV3BatchRouterErrors::SafeCastOverflowedUintDowncast,
                            )
                    }
                    SafeCastOverflowedUintDowncast
                },
                {
                    fn AddressEmptyCode(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterErrors> {
                        <AddressEmptyCode as alloy_sol_types::SolError>::abi_decode_raw_validate(
                            data,
                        )
                        .map(BalancerV3BatchRouterErrors::AddressEmptyCode)
                    }
                    AddressEmptyCode
                },
                {
                    fn InsufficientEth(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterErrors> {
                        <InsufficientEth as alloy_sol_types::SolError>::abi_decode_raw_validate(
                            data,
                        )
                        .map(BalancerV3BatchRouterErrors::InsufficientEth)
                    }
                    InsufficientEth
                },
                {
                    fn ErrorSelectorNotFound(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterErrors> {
                        <ErrorSelectorNotFound as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(BalancerV3BatchRouterErrors::ErrorSelectorNotFound)
                    }
                    ErrorSelectorNotFound
                },
                {
                    fn InputLengthMismatch(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterErrors> {
                        <InputLengthMismatch as alloy_sol_types::SolError>::abi_decode_raw_validate(
                            data,
                        )
                        .map(BalancerV3BatchRouterErrors::InputLengthMismatch)
                    }
                    InputLengthMismatch
                },
                {
                    fn AddressInsufficientBalance(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterErrors> {
                        <AddressInsufficientBalance as alloy_sol_types::SolError>::abi_decode_raw_validate(
                                data,
                            )
                            .map(BalancerV3BatchRouterErrors::AddressInsufficientBalance)
                    }
                    AddressInsufficientBalance
                },
                {
                    fn SwapDeadline(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<BalancerV3BatchRouterErrors> {
                        <SwapDeadline as alloy_sol_types::SolError>::abi_decode_raw_validate(data)
                            .map(BalancerV3BatchRouterErrors::SwapDeadline)
                    }
                    SwapDeadline
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
                Self::AddressEmptyCode(inner) => {
                    <AddressEmptyCode as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
                Self::AddressInsufficientBalance(inner) => {
                    <AddressInsufficientBalance as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::ErrorSelectorNotFound(inner) => {
                    <ErrorSelectorNotFound as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
                Self::EthTransfer(inner) => {
                    <EthTransfer as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
                Self::FailedInnerCall(inner) => {
                    <FailedInnerCall as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
                Self::InputLengthMismatch(inner) => {
                    <InputLengthMismatch as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
                Self::InsufficientEth(inner) => {
                    <InsufficientEth as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
                Self::ReentrancyGuardReentrantCall(inner) => {
                    <ReentrancyGuardReentrantCall as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::SafeCastOverflowedUintDowncast(inner) => {
                    <SafeCastOverflowedUintDowncast as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
                Self::SafeERC20FailedOperation(inner) => {
                    <SafeERC20FailedOperation as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
                Self::SenderIsNotVault(inner) => {
                    <SenderIsNotVault as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
                Self::SwapDeadline(inner) => {
                    <SwapDeadline as alloy_sol_types::SolError>::abi_encoded_size(inner)
                }
                Self::TransientIndexOutOfBounds(inner) => {
                    <TransientIndexOutOfBounds as alloy_sol_types::SolError>::abi_encoded_size(
                        inner,
                    )
                }
            }
        }

        #[inline]
        fn abi_encode_raw(&self, out: &mut alloy_sol_types::private::Vec<u8>) {
            match self {
                Self::AddressEmptyCode(inner) => {
                    <AddressEmptyCode as alloy_sol_types::SolError>::abi_encode_raw(inner, out)
                }
                Self::AddressInsufficientBalance(inner) => {
                    <AddressInsufficientBalance as alloy_sol_types::SolError>::abi_encode_raw(
                        inner, out,
                    )
                }
                Self::ErrorSelectorNotFound(inner) => {
                    <ErrorSelectorNotFound as alloy_sol_types::SolError>::abi_encode_raw(inner, out)
                }
                Self::EthTransfer(inner) => {
                    <EthTransfer as alloy_sol_types::SolError>::abi_encode_raw(inner, out)
                }
                Self::FailedInnerCall(inner) => {
                    <FailedInnerCall as alloy_sol_types::SolError>::abi_encode_raw(inner, out)
                }
                Self::InputLengthMismatch(inner) => {
                    <InputLengthMismatch as alloy_sol_types::SolError>::abi_encode_raw(inner, out)
                }
                Self::InsufficientEth(inner) => {
                    <InsufficientEth as alloy_sol_types::SolError>::abi_encode_raw(inner, out)
                }
                Self::ReentrancyGuardReentrantCall(inner) => {
                    <ReentrancyGuardReentrantCall as alloy_sol_types::SolError>::abi_encode_raw(
                        inner, out,
                    )
                }
                Self::SafeCastOverflowedUintDowncast(inner) => {
                    <SafeCastOverflowedUintDowncast as alloy_sol_types::SolError>::abi_encode_raw(
                        inner, out,
                    )
                }
                Self::SafeERC20FailedOperation(inner) => {
                    <SafeERC20FailedOperation as alloy_sol_types::SolError>::abi_encode_raw(
                        inner, out,
                    )
                }
                Self::SenderIsNotVault(inner) => {
                    <SenderIsNotVault as alloy_sol_types::SolError>::abi_encode_raw(inner, out)
                }
                Self::SwapDeadline(inner) => {
                    <SwapDeadline as alloy_sol_types::SolError>::abi_encode_raw(inner, out)
                }
                Self::TransientIndexOutOfBounds(inner) => {
                    <TransientIndexOutOfBounds as alloy_sol_types::SolError>::abi_encode_raw(
                        inner, out,
                    )
                }
            }
        }
    }
    use alloy_contract;
    /**Creates a new wrapper around an on-chain [`BalancerV3BatchRouter`](self) contract instance.

    See the [wrapper's documentation](`BalancerV3BatchRouterInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> BalancerV3BatchRouterInstance<P, N> {
        BalancerV3BatchRouterInstance::<P, N>::new(address, __provider)
    }
    /**Deploys this contract using the given `provider` and constructor arguments, if any.

    Returns a new instance of the contract, if the deployment was successful.

    For more fine-grained control over the deployment process, use [`deploy_builder`] instead.*/
    #[inline]
    pub fn deploy<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>(
        __provider: P,
        vault: alloy_sol_types::private::Address,
        weth: alloy_sol_types::private::Address,
        permit2: alloy_sol_types::private::Address,
        routerVersion: alloy_sol_types::private::String,
    ) -> impl ::core::future::Future<Output = alloy_contract::Result<BalancerV3BatchRouterInstance<P, N>>>
    {
        BalancerV3BatchRouterInstance::<P, N>::deploy(
            __provider,
            vault,
            weth,
            permit2,
            routerVersion,
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
        vault: alloy_sol_types::private::Address,
        weth: alloy_sol_types::private::Address,
        permit2: alloy_sol_types::private::Address,
        routerVersion: alloy_sol_types::private::String,
    ) -> alloy_contract::RawCallBuilder<P, N> {
        BalancerV3BatchRouterInstance::<P, N>::deploy_builder(
            __provider,
            vault,
            weth,
            permit2,
            routerVersion,
        )
    }
    /**A [`BalancerV3BatchRouter`](self) instance.

    Contains type-safe methods for interacting with an on-chain instance of the
    [`BalancerV3BatchRouter`](self) contract located at a given `address`, using a given
    provider `P`.

    If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
    documentation on how to provide it), the `deploy` and `deploy_builder` methods can
    be used to deploy a new instance of the contract.

    See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct BalancerV3BatchRouterInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for BalancerV3BatchRouterInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("BalancerV3BatchRouterInstance")
                .field(&self.address)
                .finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        BalancerV3BatchRouterInstance<P, N>
    {
        /**Creates a new wrapper around an on-chain [`BalancerV3BatchRouter`](self) contract instance.

        See the [wrapper's documentation](`BalancerV3BatchRouterInstance`) for more details.*/
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
            vault: alloy_sol_types::private::Address,
            weth: alloy_sol_types::private::Address,
            permit2: alloy_sol_types::private::Address,
            routerVersion: alloy_sol_types::private::String,
        ) -> alloy_contract::Result<BalancerV3BatchRouterInstance<P, N>> {
            let call_builder =
                Self::deploy_builder(__provider, vault, weth, permit2, routerVersion);
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
            vault: alloy_sol_types::private::Address,
            weth: alloy_sol_types::private::Address,
            permit2: alloy_sol_types::private::Address,
            routerVersion: alloy_sol_types::private::String,
        ) -> alloy_contract::RawCallBuilder<P, N> {
            alloy_contract::RawCallBuilder::new_raw_deploy(
                __provider,
                [
                    &BYTECODE[..],
                    &alloy_sol_types::SolConstructor::abi_encode(&constructorCall {
                        vault,
                        weth,
                        permit2,
                        routerVersion,
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
    impl<P: ::core::clone::Clone, N> BalancerV3BatchRouterInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned
        /// provider.
        #[inline]
        pub fn with_cloned_provider(self) -> BalancerV3BatchRouterInstance<P, N> {
            BalancerV3BatchRouterInstance {
                address: self.address,
                provider: ::core::clone::Clone::clone(&self.provider),
                _network: ::core::marker::PhantomData,
            }
        }
    }
    /// Function calls.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        BalancerV3BatchRouterInstance<P, N>
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

        ///Creates a new call builder for the [`getSender`] function.
        pub fn getSender(&self) -> alloy_contract::SolCallBuilder<&P, getSenderCall, N> {
            self.call_builder(&getSenderCall)
        }

        ///Creates a new call builder for the [`multicall`] function.
        pub fn multicall(
            &self,
            data: alloy_sol_types::private::Vec<alloy_sol_types::private::Bytes>,
        ) -> alloy_contract::SolCallBuilder<&P, multicallCall, N> {
            self.call_builder(&multicallCall { data })
        }

        ///Creates a new call builder for the [`permitBatchAndCall`] function.
        pub fn permitBatchAndCall(
            &self,
            permitBatch: alloy_sol_types::private::Vec<
                <IRouterCommon::PermitApproval as alloy_sol_types::SolType>::RustType,
            >,
            permitSignatures: alloy_sol_types::private::Vec<alloy_sol_types::private::Bytes>,
            permit2Batch: <IAllowanceTransfer::PermitBatch as alloy_sol_types::SolType>::RustType,
            permit2Signature: alloy_sol_types::private::Bytes,
            multicallData: alloy_sol_types::private::Vec<alloy_sol_types::private::Bytes>,
        ) -> alloy_contract::SolCallBuilder<&P, permitBatchAndCallCall, N> {
            self.call_builder(&permitBatchAndCallCall {
                permitBatch,
                permitSignatures,
                permit2Batch,
                permit2Signature,
                multicallData,
            })
        }

        ///Creates a new call builder for the [`querySwapExactIn`] function.
        pub fn querySwapExactIn(
            &self,
            paths: alloy_sol_types::private::Vec<
                <IBatchRouter::SwapPathExactAmountIn as alloy_sol_types::SolType>::RustType,
            >,
            sender: alloy_sol_types::private::Address,
            userData: alloy_sol_types::private::Bytes,
        ) -> alloy_contract::SolCallBuilder<&P, querySwapExactInCall, N> {
            self.call_builder(&querySwapExactInCall {
                paths,
                sender,
                userData,
            })
        }

        ///Creates a new call builder for the [`querySwapExactInHook`]
        /// function.
        pub fn querySwapExactInHook(
            &self,
            params: <IBatchRouter::SwapExactInHookParams as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, querySwapExactInHookCall, N> {
            self.call_builder(&querySwapExactInHookCall { params })
        }

        ///Creates a new call builder for the [`querySwapExactOut`] function.
        pub fn querySwapExactOut(
            &self,
            paths: alloy_sol_types::private::Vec<
                <IBatchRouter::SwapPathExactAmountOut as alloy_sol_types::SolType>::RustType,
            >,
            sender: alloy_sol_types::private::Address,
            userData: alloy_sol_types::private::Bytes,
        ) -> alloy_contract::SolCallBuilder<&P, querySwapExactOutCall, N> {
            self.call_builder(&querySwapExactOutCall {
                paths,
                sender,
                userData,
            })
        }

        ///Creates a new call builder for the [`querySwapExactOutHook`]
        /// function.
        pub fn querySwapExactOutHook(
            &self,
            params: <IBatchRouter::SwapExactOutHookParams as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, querySwapExactOutHookCall, N> {
            self.call_builder(&querySwapExactOutHookCall { params })
        }

        ///Creates a new call builder for the [`swapExactIn`] function.
        pub fn swapExactIn(
            &self,
            paths: alloy_sol_types::private::Vec<
                <IBatchRouter::SwapPathExactAmountIn as alloy_sol_types::SolType>::RustType,
            >,
            deadline: alloy_sol_types::private::primitives::aliases::U256,
            wethIsEth: bool,
            userData: alloy_sol_types::private::Bytes,
        ) -> alloy_contract::SolCallBuilder<&P, swapExactInCall, N> {
            self.call_builder(&swapExactInCall {
                paths,
                deadline,
                wethIsEth,
                userData,
            })
        }

        ///Creates a new call builder for the [`swapExactInHook`] function.
        pub fn swapExactInHook(
            &self,
            params: <IBatchRouter::SwapExactInHookParams as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, swapExactInHookCall, N> {
            self.call_builder(&swapExactInHookCall { params })
        }

        ///Creates a new call builder for the [`swapExactOut`] function.
        pub fn swapExactOut(
            &self,
            paths: alloy_sol_types::private::Vec<
                <IBatchRouter::SwapPathExactAmountOut as alloy_sol_types::SolType>::RustType,
            >,
            deadline: alloy_sol_types::private::primitives::aliases::U256,
            wethIsEth: bool,
            userData: alloy_sol_types::private::Bytes,
        ) -> alloy_contract::SolCallBuilder<&P, swapExactOutCall, N> {
            self.call_builder(&swapExactOutCall {
                paths,
                deadline,
                wethIsEth,
                userData,
            })
        }

        ///Creates a new call builder for the [`swapExactOutHook`] function.
        pub fn swapExactOutHook(
            &self,
            params: <IBatchRouter::SwapExactOutHookParams as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, swapExactOutHookCall, N> {
            self.call_builder(&swapExactOutHookCall { params })
        }

        ///Creates a new call builder for the [`version`] function.
        pub fn version(&self) -> alloy_contract::SolCallBuilder<&P, versionCall, N> {
            self.call_builder(&versionCall)
        }
    }
    /// Event filters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        BalancerV3BatchRouterInstance<P, N>
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
pub type Instance =
    BalancerV3BatchRouter::BalancerV3BatchRouterInstance<::alloy_provider::DynProvider>;
use {
    alloy_primitives::{Address, address},
    alloy_provider::{DynProvider, Provider},
    anyhow::{Context, Result},
    std::{collections::HashMap, sync::LazyLock},
};
pub const fn deployment_info(chain_id: u64) -> Option<(Address, Option<u64>)> {
    match chain_id {
        1u64 => Some((
            ::alloy_primitives::address!("0x136f1EFcC3f8f88516B9E94110D56FDBfB1778d1"),
            Some(21339510u64),
        )),
        10u64 => Some((
            ::alloy_primitives::address!("0xaD89051bEd8d96f045E8912aE1672c6C0bF8a85E"),
            Some(133969588u64),
        )),
        100u64 => Some((
            ::alloy_primitives::address!("0xe2fa4e1d17725e72dcdAfe943Ecf45dF4B9E285b"),
            Some(37377506u64),
        )),
        8453u64 => Some((
            ::alloy_primitives::address!("0x85a80afee867aDf27B50BdB7b76DA70f1E853062"),
            Some(25347205u64),
        )),
        9745u64 => Some((
            ::alloy_primitives::address!("0x85a80afee867aDf27B50BdB7b76DA70f1E853062"),
            Some(782312u64),
        )),
        42161u64 => Some((
            ::alloy_primitives::address!("0xaD89051bEd8d96f045E8912aE1672c6C0bF8a85E"),
            Some(297828544u64),
        )),
        43114u64 => Some((
            ::alloy_primitives::address!("0xc9b36096f5201ea332Db35d6D195774ea0D5988f"),
            Some(59965747u64),
        )),
        11155111u64 => Some((
            ::alloy_primitives::address!("0xC85b652685567C1B074e8c0D4389f83a2E458b1C"),
            Some(7219301u64),
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
