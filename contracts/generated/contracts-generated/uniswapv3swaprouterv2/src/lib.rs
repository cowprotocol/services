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
library IV3SwapRouter {
    struct ExactOutputSingleParams { address tokenIn; address tokenOut; uint24 fee; address recipient; uint256 amountOut; uint256 amountInMaximum; uint160 sqrtPriceLimitX96; }
}
```*/
#[allow(
    non_camel_case_types,
    non_snake_case,
    clippy::pub_underscore_fields,
    clippy::style,
    clippy::empty_structs_with_brackets
)]
pub mod IV3SwapRouter {
    use {super::*, alloy_sol_types};
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**```solidity
    struct ExactOutputSingleParams { address tokenIn; address tokenOut; uint24 fee; address recipient; uint256 amountOut; uint256 amountInMaximum; uint160 sqrtPriceLimitX96; }
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct ExactOutputSingleParams {
        #[allow(missing_docs)]
        pub tokenIn: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub tokenOut: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub fee: alloy_sol_types::private::primitives::aliases::U24,
        #[allow(missing_docs)]
        pub recipient: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub amountOut: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub amountInMaximum: alloy_sol_types::private::primitives::aliases::U256,
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
            alloy_sol_types::sol_data::Uint<24>,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<160>,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::Address,
            alloy_sol_types::private::Address,
            alloy_sol_types::private::primitives::aliases::U24,
            alloy_sol_types::private::Address,
            alloy_sol_types::private::primitives::aliases::U256,
            alloy_sol_types::private::primitives::aliases::U256,
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
        impl ::core::convert::From<ExactOutputSingleParams> for UnderlyingRustTuple<'_> {
            fn from(value: ExactOutputSingleParams) -> Self {
                (
                    value.tokenIn,
                    value.tokenOut,
                    value.fee,
                    value.recipient,
                    value.amountOut,
                    value.amountInMaximum,
                    value.sqrtPriceLimitX96,
                )
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for ExactOutputSingleParams {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    tokenIn: tuple.0,
                    tokenOut: tuple.1,
                    fee: tuple.2,
                    recipient: tuple.3,
                    amountOut: tuple.4,
                    amountInMaximum: tuple.5,
                    sqrtPriceLimitX96: tuple.6,
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolValue for ExactOutputSingleParams {
            type SolType = Self;
        }
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Self> for ExactOutputSingleParams {
            #[inline]
            fn stv_to_tokens(&self) -> <Self as alloy_sol_types::SolType>::Token<'_> {
                (
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.tokenIn,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.tokenOut,
                    ),
                    <alloy_sol_types::sol_data::Uint<24> as alloy_sol_types::SolType>::tokenize(
                        &self.fee,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.recipient,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.amountOut,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.amountInMaximum,
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
        impl alloy_sol_types::SolType for ExactOutputSingleParams {
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
        impl alloy_sol_types::SolStruct for ExactOutputSingleParams {
            const NAME: &'static str = "ExactOutputSingleParams";

            #[inline]
            fn eip712_root_type() -> alloy_sol_types::private::Cow<'static, str> {
                alloy_sol_types::private::Cow::Borrowed(
                    "ExactOutputSingleParams(address tokenIn,address tokenOut,uint24 fee,address \
                     recipient,uint256 amountOut,uint256 amountInMaximum,uint160 \
                     sqrtPriceLimitX96)",
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
                        24,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.fee)
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.recipient,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.amountOut)
                        .0,
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(
                            &self.amountInMaximum,
                        )
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
        impl alloy_sol_types::EventTopic for ExactOutputSingleParams {
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
                        24,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(&rust.fee)
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.recipient,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.amountOut,
                    )
                    + <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.amountInMaximum,
                    )
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
                    24,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(&rust.fee, out);
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.recipient,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.amountOut,
                    out,
                );
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.amountInMaximum,
                    out,
                );
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
    /**Creates a new wrapper around an on-chain [`IV3SwapRouter`](self) contract instance.

    See the [wrapper's documentation](`IV3SwapRouterInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> IV3SwapRouterInstance<P, N> {
        IV3SwapRouterInstance::<P, N>::new(address, __provider)
    }
    /**A [`IV3SwapRouter`](self) instance.

    Contains type-safe methods for interacting with an on-chain instance of the
    [`IV3SwapRouter`](self) contract located at a given `address`, using a given
    provider `P`.

    If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
    documentation on how to provide it), the `deploy` and `deploy_builder` methods can
    be used to deploy a new instance of the contract.

    See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct IV3SwapRouterInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for IV3SwapRouterInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("IV3SwapRouterInstance")
                .field(&self.address)
                .finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        IV3SwapRouterInstance<P, N>
    {
        /**Creates a new wrapper around an on-chain [`IV3SwapRouter`](self) contract instance.

        See the [wrapper's documentation](`IV3SwapRouterInstance`) for more details.*/
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
    impl<P: ::core::clone::Clone, N> IV3SwapRouterInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned
        /// provider.
        #[inline]
        pub fn with_cloned_provider(self) -> IV3SwapRouterInstance<P, N> {
            IV3SwapRouterInstance {
                address: self.address,
                provider: ::core::clone::Clone::clone(&self.provider),
                _network: ::core::marker::PhantomData,
            }
        }
    }
    /// Function calls.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        IV3SwapRouterInstance<P, N>
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
        IV3SwapRouterInstance<P, N>
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
library IV3SwapRouter {
    struct ExactOutputSingleParams {
        address tokenIn;
        address tokenOut;
        uint24 fee;
        address recipient;
        uint256 amountOut;
        uint256 amountInMaximum;
        uint160 sqrtPriceLimitX96;
    }
}

interface UniswapV3SwapRouterV2 {
    function exactOutputSingle(IV3SwapRouter.ExactOutputSingleParams memory params) external payable returns (uint256 amountIn);
}
```

...which was generated by the following JSON ABI:
```json
[
  {
    "type": "function",
    "name": "exactOutputSingle",
    "inputs": [
      {
        "name": "params",
        "type": "tuple",
        "internalType": "struct IV3SwapRouter.ExactOutputSingleParams",
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
            "name": "fee",
            "type": "uint24",
            "internalType": "uint24"
          },
          {
            "name": "recipient",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "amountOut",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "amountInMaximum",
            "type": "uint256",
            "internalType": "uint256"
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
      }
    ],
    "stateMutability": "payable"
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
pub mod UniswapV3SwapRouterV2 {
    use {super::*, alloy_sol_types};
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `exactOutputSingle((address,address,uint24,address,uint256,uint256,uint160))` and selector `0x5023b4df`.
    ```solidity
    function exactOutputSingle(IV3SwapRouter.ExactOutputSingleParams memory params) external payable returns (uint256 amountIn);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct exactOutputSingleCall {
        #[allow(missing_docs)]
        pub params: <IV3SwapRouter::ExactOutputSingleParams as alloy_sol_types::SolType>::RustType,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`exactOutputSingle((address,address,uint24,address,uint256,uint256,
    /// uint160))`](exactOutputSingleCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct exactOutputSingleReturn {
        #[allow(missing_docs)]
        pub amountIn: alloy_sol_types::private::primitives::aliases::U256,
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
            type UnderlyingSolTuple<'a> = (IV3SwapRouter::ExactOutputSingleParams,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> =
                (<IV3SwapRouter::ExactOutputSingleParams as alloy_sol_types::SolType>::RustType,);
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
            impl ::core::convert::From<exactOutputSingleCall> for UnderlyingRustTuple<'_> {
                fn from(value: exactOutputSingleCall) -> Self {
                    (value.params,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for exactOutputSingleCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { params: tuple.0 }
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
            impl ::core::convert::From<exactOutputSingleReturn> for UnderlyingRustTuple<'_> {
                fn from(value: exactOutputSingleReturn) -> Self {
                    (value.amountIn,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for exactOutputSingleReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { amountIn: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for exactOutputSingleCall {
            type Parameters<'a> = (IV3SwapRouter::ExactOutputSingleParams,);
            type Return = alloy_sol_types::private::primitives::aliases::U256;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Uint<256>,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [80u8, 35u8, 180u8, 223u8];
            const SIGNATURE: &'static str =
                "exactOutputSingle((address,address,uint24,address,uint256,uint256,uint160))";

            #[inline]
            fn new<'a>(
                tuple: <Self::Parameters<'a> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                tuple.into()
            }

            #[inline]
            fn tokenize(&self) -> Self::Token<'_> {
                (
                    <IV3SwapRouter::ExactOutputSingleParams as alloy_sol_types::SolType>::tokenize(
                        &self.params,
                    ),
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
                        let r: exactOutputSingleReturn = r.into();
                        r.amountIn
                    },
                )
            }

            #[inline]
            fn abi_decode_returns_validate(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence_validate(
                    data,
                )
                .map(|r| {
                    let r: exactOutputSingleReturn = r.into();
                    r.amountIn
                })
            }
        }
    };
    ///Container for all the [`UniswapV3SwapRouterV2`](self) function calls.
    #[derive(Clone)]
    pub enum UniswapV3SwapRouterV2Calls {
        #[allow(missing_docs)]
        exactOutputSingle(exactOutputSingleCall),
    }
    impl UniswapV3SwapRouterV2Calls {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the
        /// variants. No guarantees are made about the order of the
        /// selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 4usize]] = &[[80u8, 35u8, 180u8, 223u8]];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] =
            &[<exactOutputSingleCall as alloy_sol_types::SolCall>::SIGNATURE];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[::core::stringify!(exactOutputSingle)];

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
    impl alloy_sol_types::SolInterface for UniswapV3SwapRouterV2Calls {
        const COUNT: usize = 1usize;
        const MIN_DATA_LENGTH: usize = 224usize;
        const NAME: &'static str = "UniswapV3SwapRouterV2Calls";

        #[inline]
        fn selector(&self) -> [u8; 4] {
            match self {
                Self::exactOutputSingle(_) => {
                    <exactOutputSingleCall as alloy_sol_types::SolCall>::SELECTOR
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
                -> alloy_sol_types::Result<UniswapV3SwapRouterV2Calls>] = &[{
                fn exactOutputSingle(
                    data: &[u8],
                ) -> alloy_sol_types::Result<UniswapV3SwapRouterV2Calls> {
                    <exactOutputSingleCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                        .map(UniswapV3SwapRouterV2Calls::exactOutputSingle)
                }
                exactOutputSingle
            }];
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
                UniswapV3SwapRouterV2Calls,
            >] = &[{
                fn exactOutputSingle(
                    data: &[u8],
                ) -> alloy_sol_types::Result<UniswapV3SwapRouterV2Calls> {
                    <exactOutputSingleCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                        data,
                    )
                    .map(UniswapV3SwapRouterV2Calls::exactOutputSingle)
                }
                exactOutputSingle
            }];
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
                Self::exactOutputSingle(inner) => {
                    <exactOutputSingleCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
            }
        }

        #[inline]
        fn abi_encode_raw(&self, out: &mut alloy_sol_types::private::Vec<u8>) {
            match self {
                Self::exactOutputSingle(inner) => {
                    <exactOutputSingleCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
            }
        }
    }
    use alloy_contract;
    /**Creates a new wrapper around an on-chain [`UniswapV3SwapRouterV2`](self) contract instance.

    See the [wrapper's documentation](`UniswapV3SwapRouterV2Instance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> UniswapV3SwapRouterV2Instance<P, N> {
        UniswapV3SwapRouterV2Instance::<P, N>::new(address, __provider)
    }
    /**A [`UniswapV3SwapRouterV2`](self) instance.

    Contains type-safe methods for interacting with an on-chain instance of the
    [`UniswapV3SwapRouterV2`](self) contract located at a given `address`, using a given
    provider `P`.

    If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
    documentation on how to provide it), the `deploy` and `deploy_builder` methods can
    be used to deploy a new instance of the contract.

    See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct UniswapV3SwapRouterV2Instance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for UniswapV3SwapRouterV2Instance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("UniswapV3SwapRouterV2Instance")
                .field(&self.address)
                .finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        UniswapV3SwapRouterV2Instance<P, N>
    {
        /**Creates a new wrapper around an on-chain [`UniswapV3SwapRouterV2`](self) contract instance.

        See the [wrapper's documentation](`UniswapV3SwapRouterV2Instance`) for more details.*/
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
    impl<P: ::core::clone::Clone, N> UniswapV3SwapRouterV2Instance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned
        /// provider.
        #[inline]
        pub fn with_cloned_provider(self) -> UniswapV3SwapRouterV2Instance<P, N> {
            UniswapV3SwapRouterV2Instance {
                address: self.address,
                provider: ::core::clone::Clone::clone(&self.provider),
                _network: ::core::marker::PhantomData,
            }
        }
    }
    /// Function calls.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        UniswapV3SwapRouterV2Instance<P, N>
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

        ///Creates a new call builder for the [`exactOutputSingle`] function.
        pub fn exactOutputSingle(
            &self,
            params: <IV3SwapRouter::ExactOutputSingleParams as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, exactOutputSingleCall, N> {
            self.call_builder(&exactOutputSingleCall { params })
        }
    }
    /// Event filters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        UniswapV3SwapRouterV2Instance<P, N>
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
    UniswapV3SwapRouterV2::UniswapV3SwapRouterV2Instance<::alloy_provider::DynProvider>;
use {
    alloy_primitives::{Address, address},
    alloy_provider::{DynProvider, Provider},
    anyhow::{Context, Result},
    std::{collections::HashMap, sync::LazyLock},
};
pub const fn deployment_info(chain_id: u64) -> Option<(Address, Option<u64>)> {
    match chain_id {
        1u64 => Some((
            ::alloy_primitives::address!("0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45"),
            None,
        )),
        10u64 => Some((
            ::alloy_primitives::address!("0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45"),
            None,
        )),
        56u64 => Some((
            ::alloy_primitives::address!("0xB971eF87ede563556b2ED4b1C0b0019111Dd85d2"),
            None,
        )),
        100u64 => Some((
            ::alloy_primitives::address!("0xc6D25285D5C5b62b7ca26D6092751A145D50e9Be"),
            None,
        )),
        137u64 => Some((
            ::alloy_primitives::address!("0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45"),
            None,
        )),
        8453u64 => Some((
            ::alloy_primitives::address!("0x2626664c2603336E57B271c5C0b26F421741e481"),
            None,
        )),
        9745u64 => Some((
            ::alloy_primitives::address!("0x807F4E281B7A3B324825C64ca53c69F0b418dE40"),
            None,
        )),
        42161u64 => Some((
            ::alloy_primitives::address!("0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45"),
            None,
        )),
        43114u64 => Some((
            ::alloy_primitives::address!("0xbb00FF08d01D300023C629E8fFfFcb65A5a578cE"),
            None,
        )),
        57073u64 => Some((
            ::alloy_primitives::address!("0x177778F19E89dD1012BdBe603F144088A95C4B53"),
            None,
        )),
        59144u64 => Some((
            ::alloy_primitives::address!("0x3d4e44Eb1374240CE5F1B871ab261CD16335B76a"),
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
