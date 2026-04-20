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
library LoanRequest {
    struct Data { uint256 amount; address borrower; address lender; address token; }
}
```*/
#[allow(
    non_camel_case_types,
    non_snake_case,
    clippy::pub_underscore_fields,
    clippy::style,
    clippy::empty_structs_with_brackets
)]
pub mod LoanRequest {
    use {super::*, alloy_sol_types};
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**```solidity
    struct Data { uint256 amount; address borrower; address lender; address token; }
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct Data {
        #[allow(missing_docs)]
        pub amount: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub borrower: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub lender: alloy_sol_types::private::Address,
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
        type UnderlyingSolTuple<'a> = (
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Address,
        );
        #[doc(hidden)]
        type UnderlyingRustTuple<'a> = (
            alloy_sol_types::private::primitives::aliases::U256,
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
        impl ::core::convert::From<Data> for UnderlyingRustTuple<'_> {
            fn from(value: Data) -> Self {
                (value.amount, value.borrower, value.lender, value.token)
            }
        }
        #[automatically_derived]
        #[doc(hidden)]
        impl ::core::convert::From<UnderlyingRustTuple<'_>> for Data {
            fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                Self {
                    amount: tuple.0,
                    borrower: tuple.1,
                    lender: tuple.2,
                    token: tuple.3,
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
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.amount,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.borrower,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.lender,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.token,
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
                    "Data(uint256 amount,address borrower,address lender,address token)",
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
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::eip712_data_word(&self.amount)
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.borrower,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.lender,
                        )
                        .0,
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::eip712_data_word(
                            &self.token,
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
                        &rust.amount,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.borrower,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.lender,
                    )
                    + <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::topic_preimage_length(
                        &rust.token,
                    )
            }

            #[inline]
            fn encode_topic_preimage(
                rust: &Self::RustType,
                out: &mut alloy_sol_types::private::Vec<u8>,
            ) {
                out.reserve(<Self as alloy_sol_types::EventTopic>::topic_preimage_length(rust));
                <alloy_sol_types::sol_data::Uint<
                    256,
                > as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.amount,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.borrower,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.lender,
                    out,
                );
                <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic_preimage(
                    &rust.token,
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
    /**Creates a new wrapper around an on-chain [`LoanRequest`](self) contract instance.

    See the [wrapper's documentation](`LoanRequestInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> LoanRequestInstance<P, N> {
        LoanRequestInstance::<P, N>::new(address, __provider)
    }
    /**A [`LoanRequest`](self) instance.

    Contains type-safe methods for interacting with an on-chain instance of the
    [`LoanRequest`](self) contract located at a given `address`, using a given
    provider `P`.

    If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
    documentation on how to provide it), the `deploy` and `deploy_builder` methods can
    be used to deploy a new instance of the contract.

    See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct LoanRequestInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for LoanRequestInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("LoanRequestInstance")
                .field(&self.address)
                .finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        LoanRequestInstance<P, N>
    {
        /**Creates a new wrapper around an on-chain [`LoanRequest`](self) contract instance.

        See the [wrapper's documentation](`LoanRequestInstance`) for more details.*/
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
    impl<P: ::core::clone::Clone, N> LoanRequestInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned
        /// provider.
        #[inline]
        pub fn with_cloned_provider(self) -> LoanRequestInstance<P, N> {
            LoanRequestInstance {
                address: self.address,
                provider: ::core::clone::Clone::clone(&self.provider),
                _network: ::core::marker::PhantomData,
            }
        }
    }
    /// Function calls.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        LoanRequestInstance<P, N>
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
        LoanRequestInstance<P, N>
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
library LoanRequest {
    struct Data {
        uint256 amount;
        address borrower;
        address lender;
        address token;
    }
}

interface FlashLoanRouter {
    constructor(address _settlementContract);

    function flashLoanAndSettle(LoanRequest.Data[] memory loans, bytes memory settlement) external;
    function settlementContract() external view returns (address);
}
```

...which was generated by the following JSON ABI:
```json
[
  {
    "type": "constructor",
    "inputs": [
      {
        "name": "_settlementContract",
        "type": "address",
        "internalType": "contract ICowSettlement"
      }
    ],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "flashLoanAndSettle",
    "inputs": [
      {
        "name": "loans",
        "type": "tuple[]",
        "internalType": "struct LoanRequest.Data[]",
        "components": [
          {
            "name": "amount",
            "type": "uint256",
            "internalType": "uint256"
          },
          {
            "name": "borrower",
            "type": "address",
            "internalType": "contract IFlashLoanSolverWrapper"
          },
          {
            "name": "lender",
            "type": "address",
            "internalType": "address"
          },
          {
            "name": "token",
            "type": "address",
            "internalType": "contract IERC20"
          }
        ]
      },
      {
        "name": "settlement",
        "type": "bytes",
        "internalType": "bytes"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "settlementContract",
    "inputs": [],
    "outputs": [
      {
        "name": "",
        "type": "address",
        "internalType": "contract ICowSettlement"
      }
    ],
    "stateMutability": "view"
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
pub mod FlashLoanRouter {
    use {super::*, alloy_sol_types};
    /// The creation / init bytecode of the contract.
    ///
    /// ```text
    ///0x60c06040525f80546001600160a01b031916905534801561001e575f5ffd5b50604051610dc0380380610dc083398101604081905261003d916100d3565b6001600160a01b038116608081905260408051632335c76b60e01b81529051632335c76b9160048082019260209290919082900301815f875af1158015610086573d5f5f3e3d5ffd5b505050506040513d601f19601f820116820180604052508101906100aa91906100d3565b6001600160a01b031660a052506100f5565b6001600160a01b03811681146100d0575f5ffd5b50565b5f602082840312156100e3575f5ffd5b81516100ee816100bc565b9392505050565b60805160a051610c9e6101225f395f81816053015261026001525f818160cb01526106410152610c9e5ff3fe608060405234801561000f575f5ffd5b506004361061004a575f3560e01c806302ebcbea1461004e5780630efb1fb61461009e578063e7c438c9146100b3578063ea42418b146100c6575b5f5ffd5b6100757f000000000000000000000000000000000000000000000000000000000000000081565b60405173ffffffffffffffffffffffffffffffffffffffff909116815260200160405180910390f35b6100b16100ac3660046108b0565b6100ed565b005b6100b16100c13660046109e5565b610232565b6100757f000000000000000000000000000000000000000000000000000000000000000081565b3373ffffffffffffffffffffffffffffffffffffffff5f5c1614610172576040517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601960248201527f4f6e6c792063616c6c61626c6520627920626f72726f7765720000000000000060448201526064015b60405180910390fd5b5f805473ffffffffffffffffffffffffffffffffffffffff16907fffffffffffffffffffffffff0000000000000000000000000000000000000000815c168217905d508051602082012060015c14610226576040517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601660248201527f42616420646174612066726f6d20626f72726f776572000000000000000000006044820152606401610169565b61022f81610363565b50565b6040517f02cc250d0000000000000000000000000000000000000000000000000000000081523360048201527f000000000000000000000000000000000000000000000000000000000000000073ffffffffffffffffffffffffffffffffffffffff16906302cc250d90602401602060405180830381865afa1580156102ba573d5f5f3e3d5ffd5b505050506040513d601f19601f820116820180604052508101906102de9190610a80565b610344576040517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601960248201527f4f6e6c792063616c6c61626c65206279206120736f6c766572000000000000006044820152606401610169565b5f6103518585858561048d565b905061035c81610363565b5050505050565b60208101515f0361037f5761022f61037a82610549565b61058b565b5f61038982610731565b60208181015184519185019190912091925090815f805c7fffffffffffffffffffffffff00000000000000000000000000000000000000001673ffffffffffffffffffffffffffffffffffffffff831617905d50808060015d50604080518082018252606085015173ffffffffffffffffffffffffffffffffffffffff9081168252855160208301528583015192517fe0bbec7700000000000000000000000000000000000000000000000000000000815291929085169163e0bbec779161045991859087908b90600401610aa6565b5f604051808303815f87803b158015610470575f5ffd5b505af1158015610482573d5f5f3e3d5ffd5b505050505050505050565b60606104c461049d605c86610b72565b6104a8846020610b8f565b6104b29190610b8f565b60408051828152918201602001905290565b60208082018681529192506104d99082610b8f565b9050828482376104e98382610b8f565b9050845b801561053f57806104fd81610ba2565b915082905061052c88888481811061051757610517610bd6565b9050608002018261081890919063ffffffff16565b610537605c84610b8f565b9250506104ed565b5050949350505050565b60605f605c610559846020015190565b6105639190610b72565b602084516105719190610c03565b61057b9190610c03565b6020939093019283525090919050565b7f13d79a0b000000000000000000000000000000000000000000000000000000006105b582610867565b7fffffffff00000000000000000000000000000000000000000000000000000000161461063e576040517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601860248201527f4f6e6c7920736574746c65282920697320616c6c6f77656400000000000000006044820152606401610169565b5f7f000000000000000000000000000000000000000000000000000000000000000073ffffffffffffffffffffffffffffffffffffffff16826040516106849190610c16565b5f604051808303815f865af19150503d805f81146106bd576040519150601f19603f3d011682016040523d82523d5f602084013e6106c2565b606091505b505090508061072d576040517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601360248201527f536574746c656d656e74207265766572746564000000000000000000000000006044820152606401610169565b5050565b604080516080810182525f8082526020820181905291810182905260608101829052906001610761846020015190565b61076b9190610c03565b90505f605c845161077c9190610c03565b9050602084015f61078d8383610b8f565b905082865283825261080e81604080516080810182525f80825260208201819052918101829052606081019190915250805160148201516028830151603c909301516040805160808101825293845273ffffffffffffffffffffffffffffffffffffffff928316602085015293821693830193909352909116606082015290565b9695505050505050565b80355f61082b6040840160208501610c4d565b90505f61083e6060850160408601610c4d565b90505f6108516080860160608701610c4d565b603c870152506028850152601484015290915250565b5f80602083019050600483511061087d57805191505b50919050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52604160045260245ffd5b5f602082840312156108c0575f5ffd5b813567ffffffffffffffff8111156108d6575f5ffd5b8201601f810184136108e6575f5ffd5b803567ffffffffffffffff81111561090057610900610883565b6040517fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0603f7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f8501160116810181811067ffffffffffffffff8211171561096c5761096c610883565b604052818152828201602001861015610983575f5ffd5b816020840160208301375f91810160200191909152949350505050565b5f5f83601f8401126109b0575f5ffd5b50813567ffffffffffffffff8111156109c7575f5ffd5b6020830191508360208285010111156109de575f5ffd5b9250929050565b5f5f5f5f604085870312156109f8575f5ffd5b843567ffffffffffffffff811115610a0e575f5ffd5b8501601f81018713610a1e575f5ffd5b803567ffffffffffffffff811115610a34575f5ffd5b8760208260071b8401011115610a48575f5ffd5b60209182019550935085013567ffffffffffffffff811115610a68575f5ffd5b610a74878288016109a0565b95989497509550505050565b5f60208284031215610a90575f5ffd5b81518015158114610a9f575f5ffd5b9392505050565b73ffffffffffffffffffffffffffffffffffffffff8516815273ffffffffffffffffffffffffffffffffffffffff84511660208201526020840151604082015282606082015260a060808201525f82518060a0840152806020850160c085015e5f60c0828501015260c07fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f83011684010191505095945050505050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52601160045260245ffd5b8082028115828204841417610b8957610b89610b45565b92915050565b80820180821115610b8957610b89610b45565b5f81610bb057610bb0610b45565b507fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff0190565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52603260045260245ffd5b81810381811115610b8957610b89610b45565b5f82518060208501845e5f920191825250919050565b73ffffffffffffffffffffffffffffffffffffffff8116811461022f575f5ffd5b5f60208284031215610c5d575f5ffd5b8135610a9f81610c2c56fea264697066735822122040d837bb712363085fa15f0b290e54590843321342f61b2f21d52432d2dc8e7b64736f6c634300081c0033
    /// ```
    #[rustfmt::skip]
    #[allow(clippy::all)]
    pub static BYTECODE: alloy_sol_types::private::Bytes = alloy_sol_types::private::Bytes::from_static(
        b"`\xC0`@R_\x80T`\x01`\x01`\xA0\x1B\x03\x19\x16\x90U4\x80\x15a\0\x1EW__\xFD[P`@Qa\r\xC08\x03\x80a\r\xC0\x839\x81\x01`@\x81\x90Ra\0=\x91a\0\xD3V[`\x01`\x01`\xA0\x1B\x03\x81\x16`\x80\x81\x90R`@\x80Qc#5\xC7k`\xE0\x1B\x81R\x90Qc#5\xC7k\x91`\x04\x80\x82\x01\x92` \x92\x90\x91\x90\x82\x90\x03\x01\x81_\x87Z\xF1\x15\x80\x15a\0\x86W=__>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\0\xAA\x91\x90a\0\xD3V[`\x01`\x01`\xA0\x1B\x03\x16`\xA0RPa\0\xF5V[`\x01`\x01`\xA0\x1B\x03\x81\x16\x81\x14a\0\xD0W__\xFD[PV[_` \x82\x84\x03\x12\x15a\0\xE3W__\xFD[\x81Qa\0\xEE\x81a\0\xBCV[\x93\x92PPPV[`\x80Q`\xA0Qa\x0C\x9Ea\x01\"_9_\x81\x81`S\x01Ra\x02`\x01R_\x81\x81`\xCB\x01Ra\x06A\x01Ra\x0C\x9E_\xF3\xFE`\x80`@R4\x80\x15a\0\x0FW__\xFD[P`\x046\x10a\0JW_5`\xE0\x1C\x80c\x02\xEB\xCB\xEA\x14a\0NW\x80c\x0E\xFB\x1F\xB6\x14a\0\x9EW\x80c\xE7\xC48\xC9\x14a\0\xB3W\x80c\xEABA\x8B\x14a\0\xC6W[__\xFD[a\0u\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[`@Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x90\x91\x16\x81R` \x01`@Q\x80\x91\x03\x90\xF3[a\0\xB1a\0\xAC6`\x04a\x08\xB0V[a\0\xEDV[\0[a\0\xB1a\0\xC16`\x04a\t\xE5V[a\x022V[a\0u\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[3s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF_\\\x16\x14a\x01rW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x19`$\x82\x01R\x7FOnly callable by borrower\0\0\0\0\0\0\0`D\x82\x01R`d\x01[`@Q\x80\x91\x03\x90\xFD[_\x80Ts\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81\\\x16\x82\x17\x90]P\x80Q` \x82\x01 `\x01\\\x14a\x02&W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x16`$\x82\x01R\x7FBad data from borrower\0\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x01iV[a\x02/\x81a\x03cV[PV[`@Q\x7F\x02\xCC%\r\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R3`\x04\x82\x01R\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90c\x02\xCC%\r\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x02\xBAW=__>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x02\xDE\x91\x90a\n\x80V[a\x03DW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x19`$\x82\x01R\x7FOnly callable by a solver\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x01iV[_a\x03Q\x85\x85\x85\x85a\x04\x8DV[\x90Pa\x03\\\x81a\x03cV[PPPPPV[` \x81\x01Q_\x03a\x03\x7FWa\x02/a\x03z\x82a\x05IV[a\x05\x8BV[_a\x03\x89\x82a\x071V[` \x81\x81\x01Q\x84Q\x91\x85\x01\x91\x90\x91 \x91\x92P\x90\x81_\x80\\\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x83\x16\x17\x90]P\x80\x80`\x01]P`@\x80Q\x80\x82\x01\x82R``\x85\x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x90\x81\x16\x82R\x85Q` \x83\x01R\x85\x83\x01Q\x92Q\x7F\xE0\xBB\xECw\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R\x91\x92\x90\x85\x16\x91c\xE0\xBB\xECw\x91a\x04Y\x91\x85\x90\x87\x90\x8B\x90`\x04\x01a\n\xA6V[_`@Q\x80\x83\x03\x81_\x87\x80;\x15\x80\x15a\x04pW__\xFD[PZ\xF1\x15\x80\x15a\x04\x82W=__>=_\xFD[PPPPPPPPPV[``a\x04\xC4a\x04\x9D`\\\x86a\x0BrV[a\x04\xA8\x84` a\x0B\x8FV[a\x04\xB2\x91\x90a\x0B\x8FV[`@\x80Q\x82\x81R\x91\x82\x01` \x01\x90R\x90V[` \x80\x82\x01\x86\x81R\x91\x92Pa\x04\xD9\x90\x82a\x0B\x8FV[\x90P\x82\x84\x827a\x04\xE9\x83\x82a\x0B\x8FV[\x90P\x84[\x80\x15a\x05?W\x80a\x04\xFD\x81a\x0B\xA2V[\x91P\x82\x90Pa\x05,\x88\x88\x84\x81\x81\x10a\x05\x17Wa\x05\x17a\x0B\xD6V[\x90P`\x80\x02\x01\x82a\x08\x18\x90\x91\x90c\xFF\xFF\xFF\xFF\x16V[a\x057`\\\x84a\x0B\x8FV[\x92PPa\x04\xEDV[PP\x94\x93PPPPV[``_`\\a\x05Y\x84` \x01Q\x90V[a\x05c\x91\x90a\x0BrV[` \x84Qa\x05q\x91\x90a\x0C\x03V[a\x05{\x91\x90a\x0C\x03V[` \x93\x90\x93\x01\x92\x83RP\x90\x91\x90PV[\x7F\x13\xD7\x9A\x0B\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0a\x05\xB5\x82a\x08gV[\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x14a\x06>W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x18`$\x82\x01R\x7FOnly settle() is allowed\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x01iV[_\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x82`@Qa\x06\x84\x91\x90a\x0C\x16V[_`@Q\x80\x83\x03\x81_\x86Z\xF1\x91PP=\x80_\x81\x14a\x06\xBDW`@Q\x91P`\x1F\x19`?=\x01\x16\x82\x01`@R=\x82R=_` \x84\x01>a\x06\xC2V[``\x91P[PP\x90P\x80a\x07-W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x13`$\x82\x01R\x7FSettlement reverted\0\0\0\0\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x01iV[PPV[`@\x80Q`\x80\x81\x01\x82R_\x80\x82R` \x82\x01\x81\x90R\x91\x81\x01\x82\x90R``\x81\x01\x82\x90R\x90`\x01a\x07a\x84` \x01Q\x90V[a\x07k\x91\x90a\x0C\x03V[\x90P_`\\\x84Qa\x07|\x91\x90a\x0C\x03V[\x90P` \x84\x01_a\x07\x8D\x83\x83a\x0B\x8FV[\x90P\x82\x86R\x83\x82Ra\x08\x0E\x81`@\x80Q`\x80\x81\x01\x82R_\x80\x82R` \x82\x01\x81\x90R\x91\x81\x01\x82\x90R``\x81\x01\x91\x90\x91RP\x80Q`\x14\x82\x01Q`(\x83\x01Q`<\x90\x93\x01Q`@\x80Q`\x80\x81\x01\x82R\x93\x84Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x92\x83\x16` \x85\x01R\x93\x82\x16\x93\x83\x01\x93\x90\x93R\x90\x91\x16``\x82\x01R\x90V[\x96\x95PPPPPPV[\x805_a\x08+`@\x84\x01` \x85\x01a\x0CMV[\x90P_a\x08>``\x85\x01`@\x86\x01a\x0CMV[\x90P_a\x08Q`\x80\x86\x01``\x87\x01a\x0CMV[`<\x87\x01RP`(\x85\x01R`\x14\x84\x01R\x90\x91RPV[_\x80` \x83\x01\x90P`\x04\x83Q\x10a\x08}W\x80Q\x91P[P\x91\x90PV[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`A`\x04R`$_\xFD[_` \x82\x84\x03\x12\x15a\x08\xC0W__\xFD[\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x08\xD6W__\xFD[\x82\x01`\x1F\x81\x01\x84\x13a\x08\xE6W__\xFD[\x805g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\t\0Wa\t\0a\x08\x83V[`@Q\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`?\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`\x1F\x85\x01\x16\x01\x16\x81\x01\x81\x81\x10g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11\x17\x15a\tlWa\tla\x08\x83V[`@R\x81\x81R\x82\x82\x01` \x01\x86\x10\x15a\t\x83W__\xFD[\x81` \x84\x01` \x83\x017_\x91\x81\x01` \x01\x91\x90\x91R\x94\x93PPPPV[__\x83`\x1F\x84\x01\x12a\t\xB0W__\xFD[P\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\t\xC7W__\xFD[` \x83\x01\x91P\x83` \x82\x85\x01\x01\x11\x15a\t\xDEW__\xFD[\x92P\x92\x90PV[____`@\x85\x87\x03\x12\x15a\t\xF8W__\xFD[\x845g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\n\x0EW__\xFD[\x85\x01`\x1F\x81\x01\x87\x13a\n\x1EW__\xFD[\x805g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\n4W__\xFD[\x87` \x82`\x07\x1B\x84\x01\x01\x11\x15a\nHW__\xFD[` \x91\x82\x01\x95P\x93P\x85\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\nhW__\xFD[a\nt\x87\x82\x88\x01a\t\xA0V[\x95\x98\x94\x97P\x95PPPPV[_` \x82\x84\x03\x12\x15a\n\x90W__\xFD[\x81Q\x80\x15\x15\x81\x14a\n\x9FW__\xFD[\x93\x92PPPV[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x85\x16\x81Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x84Q\x16` \x82\x01R` \x84\x01Q`@\x82\x01R\x82``\x82\x01R`\xA0`\x80\x82\x01R_\x82Q\x80`\xA0\x84\x01R\x80` \x85\x01`\xC0\x85\x01^_`\xC0\x82\x85\x01\x01R`\xC0\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`\x1F\x83\x01\x16\x84\x01\x01\x91PP\x95\x94PPPPPV[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x11`\x04R`$_\xFD[\x80\x82\x02\x81\x15\x82\x82\x04\x84\x14\x17a\x0B\x89Wa\x0B\x89a\x0BEV[\x92\x91PPV[\x80\x82\x01\x80\x82\x11\x15a\x0B\x89Wa\x0B\x89a\x0BEV[_\x81a\x0B\xB0Wa\x0B\xB0a\x0BEV[P\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x01\x90V[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`2`\x04R`$_\xFD[\x81\x81\x03\x81\x81\x11\x15a\x0B\x89Wa\x0B\x89a\x0BEV[_\x82Q\x80` \x85\x01\x84^_\x92\x01\x91\x82RP\x91\x90PV[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x16\x81\x14a\x02/W__\xFD[_` \x82\x84\x03\x12\x15a\x0C]W__\xFD[\x815a\n\x9F\x81a\x0C,V\xFE\xA2dipfsX\"\x12 @\xD87\xBBq#c\x08_\xA1_\x0B)\x0ETY\x08C2\x13B\xF6\x1B/!\xD5$2\xD2\xDC\x8E{dsolcC\0\x08\x1C\x003",
    );
    /// The runtime bytecode of the contract, as deployed on the network.
    ///
    /// ```text
    ///0x608060405234801561000f575f5ffd5b506004361061004a575f3560e01c806302ebcbea1461004e5780630efb1fb61461009e578063e7c438c9146100b3578063ea42418b146100c6575b5f5ffd5b6100757f000000000000000000000000000000000000000000000000000000000000000081565b60405173ffffffffffffffffffffffffffffffffffffffff909116815260200160405180910390f35b6100b16100ac3660046108b0565b6100ed565b005b6100b16100c13660046109e5565b610232565b6100757f000000000000000000000000000000000000000000000000000000000000000081565b3373ffffffffffffffffffffffffffffffffffffffff5f5c1614610172576040517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601960248201527f4f6e6c792063616c6c61626c6520627920626f72726f7765720000000000000060448201526064015b60405180910390fd5b5f805473ffffffffffffffffffffffffffffffffffffffff16907fffffffffffffffffffffffff0000000000000000000000000000000000000000815c168217905d508051602082012060015c14610226576040517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601660248201527f42616420646174612066726f6d20626f72726f776572000000000000000000006044820152606401610169565b61022f81610363565b50565b6040517f02cc250d0000000000000000000000000000000000000000000000000000000081523360048201527f000000000000000000000000000000000000000000000000000000000000000073ffffffffffffffffffffffffffffffffffffffff16906302cc250d90602401602060405180830381865afa1580156102ba573d5f5f3e3d5ffd5b505050506040513d601f19601f820116820180604052508101906102de9190610a80565b610344576040517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601960248201527f4f6e6c792063616c6c61626c65206279206120736f6c766572000000000000006044820152606401610169565b5f6103518585858561048d565b905061035c81610363565b5050505050565b60208101515f0361037f5761022f61037a82610549565b61058b565b5f61038982610731565b60208181015184519185019190912091925090815f805c7fffffffffffffffffffffffff00000000000000000000000000000000000000001673ffffffffffffffffffffffffffffffffffffffff831617905d50808060015d50604080518082018252606085015173ffffffffffffffffffffffffffffffffffffffff9081168252855160208301528583015192517fe0bbec7700000000000000000000000000000000000000000000000000000000815291929085169163e0bbec779161045991859087908b90600401610aa6565b5f604051808303815f87803b158015610470575f5ffd5b505af1158015610482573d5f5f3e3d5ffd5b505050505050505050565b60606104c461049d605c86610b72565b6104a8846020610b8f565b6104b29190610b8f565b60408051828152918201602001905290565b60208082018681529192506104d99082610b8f565b9050828482376104e98382610b8f565b9050845b801561053f57806104fd81610ba2565b915082905061052c88888481811061051757610517610bd6565b9050608002018261081890919063ffffffff16565b610537605c84610b8f565b9250506104ed565b5050949350505050565b60605f605c610559846020015190565b6105639190610b72565b602084516105719190610c03565b61057b9190610c03565b6020939093019283525090919050565b7f13d79a0b000000000000000000000000000000000000000000000000000000006105b582610867565b7fffffffff00000000000000000000000000000000000000000000000000000000161461063e576040517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601860248201527f4f6e6c7920736574746c65282920697320616c6c6f77656400000000000000006044820152606401610169565b5f7f000000000000000000000000000000000000000000000000000000000000000073ffffffffffffffffffffffffffffffffffffffff16826040516106849190610c16565b5f604051808303815f865af19150503d805f81146106bd576040519150601f19603f3d011682016040523d82523d5f602084013e6106c2565b606091505b505090508061072d576040517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601360248201527f536574746c656d656e74207265766572746564000000000000000000000000006044820152606401610169565b5050565b604080516080810182525f8082526020820181905291810182905260608101829052906001610761846020015190565b61076b9190610c03565b90505f605c845161077c9190610c03565b9050602084015f61078d8383610b8f565b905082865283825261080e81604080516080810182525f80825260208201819052918101829052606081019190915250805160148201516028830151603c909301516040805160808101825293845273ffffffffffffffffffffffffffffffffffffffff928316602085015293821693830193909352909116606082015290565b9695505050505050565b80355f61082b6040840160208501610c4d565b90505f61083e6060850160408601610c4d565b90505f6108516080860160608701610c4d565b603c870152506028850152601484015290915250565b5f80602083019050600483511061087d57805191505b50919050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52604160045260245ffd5b5f602082840312156108c0575f5ffd5b813567ffffffffffffffff8111156108d6575f5ffd5b8201601f810184136108e6575f5ffd5b803567ffffffffffffffff81111561090057610900610883565b6040517fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0603f7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f8501160116810181811067ffffffffffffffff8211171561096c5761096c610883565b604052818152828201602001861015610983575f5ffd5b816020840160208301375f91810160200191909152949350505050565b5f5f83601f8401126109b0575f5ffd5b50813567ffffffffffffffff8111156109c7575f5ffd5b6020830191508360208285010111156109de575f5ffd5b9250929050565b5f5f5f5f604085870312156109f8575f5ffd5b843567ffffffffffffffff811115610a0e575f5ffd5b8501601f81018713610a1e575f5ffd5b803567ffffffffffffffff811115610a34575f5ffd5b8760208260071b8401011115610a48575f5ffd5b60209182019550935085013567ffffffffffffffff811115610a68575f5ffd5b610a74878288016109a0565b95989497509550505050565b5f60208284031215610a90575f5ffd5b81518015158114610a9f575f5ffd5b9392505050565b73ffffffffffffffffffffffffffffffffffffffff8516815273ffffffffffffffffffffffffffffffffffffffff84511660208201526020840151604082015282606082015260a060808201525f82518060a0840152806020850160c085015e5f60c0828501015260c07fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f83011684010191505095945050505050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52601160045260245ffd5b8082028115828204841417610b8957610b89610b45565b92915050565b80820180821115610b8957610b89610b45565b5f81610bb057610bb0610b45565b507fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff0190565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52603260045260245ffd5b81810381811115610b8957610b89610b45565b5f82518060208501845e5f920191825250919050565b73ffffffffffffffffffffffffffffffffffffffff8116811461022f575f5ffd5b5f60208284031215610c5d575f5ffd5b8135610a9f81610c2c56fea264697066735822122040d837bb712363085fa15f0b290e54590843321342f61b2f21d52432d2dc8e7b64736f6c634300081c0033
    /// ```
    #[rustfmt::skip]
    #[allow(clippy::all)]
    pub static DEPLOYED_BYTECODE: alloy_sol_types::private::Bytes = alloy_sol_types::private::Bytes::from_static(
        b"`\x80`@R4\x80\x15a\0\x0FW__\xFD[P`\x046\x10a\0JW_5`\xE0\x1C\x80c\x02\xEB\xCB\xEA\x14a\0NW\x80c\x0E\xFB\x1F\xB6\x14a\0\x9EW\x80c\xE7\xC48\xC9\x14a\0\xB3W\x80c\xEABA\x8B\x14a\0\xC6W[__\xFD[a\0u\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[`@Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x90\x91\x16\x81R` \x01`@Q\x80\x91\x03\x90\xF3[a\0\xB1a\0\xAC6`\x04a\x08\xB0V[a\0\xEDV[\0[a\0\xB1a\0\xC16`\x04a\t\xE5V[a\x022V[a\0u\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81V[3s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF_\\\x16\x14a\x01rW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x19`$\x82\x01R\x7FOnly callable by borrower\0\0\0\0\0\0\0`D\x82\x01R`d\x01[`@Q\x80\x91\x03\x90\xFD[_\x80Ts\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81\\\x16\x82\x17\x90]P\x80Q` \x82\x01 `\x01\\\x14a\x02&W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x16`$\x82\x01R\x7FBad data from borrower\0\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x01iV[a\x02/\x81a\x03cV[PV[`@Q\x7F\x02\xCC%\r\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R3`\x04\x82\x01R\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90c\x02\xCC%\r\x90`$\x01` `@Q\x80\x83\x03\x81\x86Z\xFA\x15\x80\x15a\x02\xBAW=__>=_\xFD[PPPP`@Q=`\x1F\x19`\x1F\x82\x01\x16\x82\x01\x80`@RP\x81\x01\x90a\x02\xDE\x91\x90a\n\x80V[a\x03DW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x19`$\x82\x01R\x7FOnly callable by a solver\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x01iV[_a\x03Q\x85\x85\x85\x85a\x04\x8DV[\x90Pa\x03\\\x81a\x03cV[PPPPPV[` \x81\x01Q_\x03a\x03\x7FWa\x02/a\x03z\x82a\x05IV[a\x05\x8BV[_a\x03\x89\x82a\x071V[` \x81\x81\x01Q\x84Q\x91\x85\x01\x91\x90\x91 \x91\x92P\x90\x81_\x80\\\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x83\x16\x17\x90]P\x80\x80`\x01]P`@\x80Q\x80\x82\x01\x82R``\x85\x01Qs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x90\x81\x16\x82R\x85Q` \x83\x01R\x85\x83\x01Q\x92Q\x7F\xE0\xBB\xECw\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R\x91\x92\x90\x85\x16\x91c\xE0\xBB\xECw\x91a\x04Y\x91\x85\x90\x87\x90\x8B\x90`\x04\x01a\n\xA6V[_`@Q\x80\x83\x03\x81_\x87\x80;\x15\x80\x15a\x04pW__\xFD[PZ\xF1\x15\x80\x15a\x04\x82W=__>=_\xFD[PPPPPPPPPV[``a\x04\xC4a\x04\x9D`\\\x86a\x0BrV[a\x04\xA8\x84` a\x0B\x8FV[a\x04\xB2\x91\x90a\x0B\x8FV[`@\x80Q\x82\x81R\x91\x82\x01` \x01\x90R\x90V[` \x80\x82\x01\x86\x81R\x91\x92Pa\x04\xD9\x90\x82a\x0B\x8FV[\x90P\x82\x84\x827a\x04\xE9\x83\x82a\x0B\x8FV[\x90P\x84[\x80\x15a\x05?W\x80a\x04\xFD\x81a\x0B\xA2V[\x91P\x82\x90Pa\x05,\x88\x88\x84\x81\x81\x10a\x05\x17Wa\x05\x17a\x0B\xD6V[\x90P`\x80\x02\x01\x82a\x08\x18\x90\x91\x90c\xFF\xFF\xFF\xFF\x16V[a\x057`\\\x84a\x0B\x8FV[\x92PPa\x04\xEDV[PP\x94\x93PPPPV[``_`\\a\x05Y\x84` \x01Q\x90V[a\x05c\x91\x90a\x0BrV[` \x84Qa\x05q\x91\x90a\x0C\x03V[a\x05{\x91\x90a\x0C\x03V[` \x93\x90\x93\x01\x92\x83RP\x90\x91\x90PV[\x7F\x13\xD7\x9A\x0B\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0a\x05\xB5\x82a\x08gV[\x7F\xFF\xFF\xFF\xFF\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x16\x14a\x06>W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x18`$\x82\x01R\x7FOnly settle() is allowed\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x01iV[_\x7F\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x82`@Qa\x06\x84\x91\x90a\x0C\x16V[_`@Q\x80\x83\x03\x81_\x86Z\xF1\x91PP=\x80_\x81\x14a\x06\xBDW`@Q\x91P`\x1F\x19`?=\x01\x16\x82\x01`@R=\x82R=_` \x84\x01>a\x06\xC2V[``\x91P[PP\x90P\x80a\x07-W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R` `\x04\x82\x01R`\x13`$\x82\x01R\x7FSettlement reverted\0\0\0\0\0\0\0\0\0\0\0\0\0`D\x82\x01R`d\x01a\x01iV[PPV[`@\x80Q`\x80\x81\x01\x82R_\x80\x82R` \x82\x01\x81\x90R\x91\x81\x01\x82\x90R``\x81\x01\x82\x90R\x90`\x01a\x07a\x84` \x01Q\x90V[a\x07k\x91\x90a\x0C\x03V[\x90P_`\\\x84Qa\x07|\x91\x90a\x0C\x03V[\x90P` \x84\x01_a\x07\x8D\x83\x83a\x0B\x8FV[\x90P\x82\x86R\x83\x82Ra\x08\x0E\x81`@\x80Q`\x80\x81\x01\x82R_\x80\x82R` \x82\x01\x81\x90R\x91\x81\x01\x82\x90R``\x81\x01\x91\x90\x91RP\x80Q`\x14\x82\x01Q`(\x83\x01Q`<\x90\x93\x01Q`@\x80Q`\x80\x81\x01\x82R\x93\x84Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x92\x83\x16` \x85\x01R\x93\x82\x16\x93\x83\x01\x93\x90\x93R\x90\x91\x16``\x82\x01R\x90V[\x96\x95PPPPPPV[\x805_a\x08+`@\x84\x01` \x85\x01a\x0CMV[\x90P_a\x08>``\x85\x01`@\x86\x01a\x0CMV[\x90P_a\x08Q`\x80\x86\x01``\x87\x01a\x0CMV[`<\x87\x01RP`(\x85\x01R`\x14\x84\x01R\x90\x91RPV[_\x80` \x83\x01\x90P`\x04\x83Q\x10a\x08}W\x80Q\x91P[P\x91\x90PV[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`A`\x04R`$_\xFD[_` \x82\x84\x03\x12\x15a\x08\xC0W__\xFD[\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\x08\xD6W__\xFD[\x82\x01`\x1F\x81\x01\x84\x13a\x08\xE6W__\xFD[\x805g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\t\0Wa\t\0a\x08\x83V[`@Q\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`?\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`\x1F\x85\x01\x16\x01\x16\x81\x01\x81\x81\x10g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x82\x11\x17\x15a\tlWa\tla\x08\x83V[`@R\x81\x81R\x82\x82\x01` \x01\x86\x10\x15a\t\x83W__\xFD[\x81` \x84\x01` \x83\x017_\x91\x81\x01` \x01\x91\x90\x91R\x94\x93PPPPV[__\x83`\x1F\x84\x01\x12a\t\xB0W__\xFD[P\x815g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\t\xC7W__\xFD[` \x83\x01\x91P\x83` \x82\x85\x01\x01\x11\x15a\t\xDEW__\xFD[\x92P\x92\x90PV[____`@\x85\x87\x03\x12\x15a\t\xF8W__\xFD[\x845g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\n\x0EW__\xFD[\x85\x01`\x1F\x81\x01\x87\x13a\n\x1EW__\xFD[\x805g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\n4W__\xFD[\x87` \x82`\x07\x1B\x84\x01\x01\x11\x15a\nHW__\xFD[` \x91\x82\x01\x95P\x93P\x85\x015g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x15a\nhW__\xFD[a\nt\x87\x82\x88\x01a\t\xA0V[\x95\x98\x94\x97P\x95PPPPV[_` \x82\x84\x03\x12\x15a\n\x90W__\xFD[\x81Q\x80\x15\x15\x81\x14a\n\x9FW__\xFD[\x93\x92PPPV[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x85\x16\x81Rs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x84Q\x16` \x82\x01R` \x84\x01Q`@\x82\x01R\x82``\x82\x01R`\xA0`\x80\x82\x01R_\x82Q\x80`\xA0\x84\x01R\x80` \x85\x01`\xC0\x85\x01^_`\xC0\x82\x85\x01\x01R`\xC0\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xE0`\x1F\x83\x01\x16\x84\x01\x01\x91PP\x95\x94PPPPPV[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`\x11`\x04R`$_\xFD[\x80\x82\x02\x81\x15\x82\x82\x04\x84\x14\x17a\x0B\x89Wa\x0B\x89a\x0BEV[\x92\x91PPV[\x80\x82\x01\x80\x82\x11\x15a\x0B\x89Wa\x0B\x89a\x0BEV[_\x81a\x0B\xB0Wa\x0B\xB0a\x0BEV[P\x7F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x01\x90V[\x7FNH{q\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0_R`2`\x04R`$_\xFD[\x81\x81\x03\x81\x81\x11\x15a\x0B\x89Wa\x0B\x89a\x0BEV[_\x82Q\x80` \x85\x01\x84^_\x92\x01\x91\x82RP\x91\x90PV[s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x16\x81\x14a\x02/W__\xFD[_` \x82\x84\x03\x12\x15a\x0C]W__\xFD[\x815a\n\x9F\x81a\x0C,V\xFE\xA2dipfsX\"\x12 @\xD87\xBBq#c\x08_\xA1_\x0B)\x0ETY\x08C2\x13B\xF6\x1B/!\xD5$2\xD2\xDC\x8E{dsolcC\0\x08\x1C\x003",
    );
    /**Constructor`.
    ```solidity
    constructor(address _settlementContract);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct constructorCall {
        #[allow(missing_docs)]
        pub _settlementContract: alloy_sol_types::private::Address,
    }
    const _: () = {
        use alloy_sol_types;
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
            impl ::core::convert::From<constructorCall> for UnderlyingRustTuple<'_> {
                fn from(value: constructorCall) -> Self {
                    (value._settlementContract,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for constructorCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        _settlementContract: tuple.0,
                    }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolConstructor for constructorCall {
            type Parameters<'a> = (alloy_sol_types::sol_data::Address,);
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
                        &self._settlementContract,
                    ),
                )
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `flashLoanAndSettle((uint256,address,address,address)[],bytes)` and selector `0xe7c438c9`.
    ```solidity
    function flashLoanAndSettle(LoanRequest.Data[] memory loans, bytes memory settlement) external;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct flashLoanAndSettleCall {
        #[allow(missing_docs)]
        pub loans: alloy_sol_types::private::Vec<
            <LoanRequest::Data as alloy_sol_types::SolType>::RustType,
        >,
        #[allow(missing_docs)]
        pub settlement: alloy_sol_types::private::Bytes,
    }
    ///Container type for the return parameters of the
    /// [`flashLoanAndSettle((uint256,address,address,address)[],
    /// bytes)`](flashLoanAndSettleCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct flashLoanAndSettleReturn {}
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
                alloy_sol_types::sol_data::Array<LoanRequest::Data>,
                alloy_sol_types::sol_data::Bytes,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Vec<
                    <LoanRequest::Data as alloy_sol_types::SolType>::RustType,
                >,
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
            impl ::core::convert::From<flashLoanAndSettleCall> for UnderlyingRustTuple<'_> {
                fn from(value: flashLoanAndSettleCall) -> Self {
                    (value.loans, value.settlement)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for flashLoanAndSettleCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        loans: tuple.0,
                        settlement: tuple.1,
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
            impl ::core::convert::From<flashLoanAndSettleReturn> for UnderlyingRustTuple<'_> {
                fn from(value: flashLoanAndSettleReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for flashLoanAndSettleReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl flashLoanAndSettleReturn {
            fn _tokenize(
                &self,
            ) -> <flashLoanAndSettleCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for flashLoanAndSettleCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Array<LoanRequest::Data>,
                alloy_sol_types::sol_data::Bytes,
            );
            type Return = flashLoanAndSettleReturn;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = ();
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [231u8, 196u8, 56u8, 201u8];
            const SIGNATURE: &'static str =
                "flashLoanAndSettle((uint256,address,address,address)[],bytes)";

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
                        LoanRequest::Data,
                    > as alloy_sol_types::SolType>::tokenize(&self.loans),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.settlement,
                    ),
                )
            }

            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                flashLoanAndSettleReturn::_tokenize(ret)
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
    /**Function with signature `settlementContract()` and selector `0xea42418b`.
    ```solidity
    function settlementContract() external view returns (address);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct settlementContractCall;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the
    /// [`settlementContract()`](settlementContractCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct settlementContractReturn {
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
            impl ::core::convert::From<settlementContractCall> for UnderlyingRustTuple<'_> {
                fn from(value: settlementContractCall) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for settlementContractCall {
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
            impl ::core::convert::From<settlementContractReturn> for UnderlyingRustTuple<'_> {
                fn from(value: settlementContractReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for settlementContractReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for settlementContractCall {
            type Parameters<'a> = ();
            type Return = alloy_sol_types::private::Address;
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;

            const SELECTOR: [u8; 4] = [234u8, 66u8, 65u8, 139u8];
            const SIGNATURE: &'static str = "settlementContract()";

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
                        let r: settlementContractReturn = r.into();
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
                    let r: settlementContractReturn = r.into();
                    r._0
                })
            }
        }
    };
    ///Container for all the [`FlashLoanRouter`](self) function calls.
    #[derive(Clone)]
    pub enum FlashLoanRouterCalls {
        #[allow(missing_docs)]
        flashLoanAndSettle(flashLoanAndSettleCall),
        #[allow(missing_docs)]
        settlementContract(settlementContractCall),
    }
    impl FlashLoanRouterCalls {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the
        /// variants. No guarantees are made about the order of the
        /// selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 4usize]] =
            &[[231u8, 196u8, 56u8, 201u8], [234u8, 66u8, 65u8, 139u8]];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <flashLoanAndSettleCall as alloy_sol_types::SolCall>::SIGNATURE,
            <settlementContractCall as alloy_sol_types::SolCall>::SIGNATURE,
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(flashLoanAndSettle),
            ::core::stringify!(settlementContract),
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
    impl alloy_sol_types::SolInterface for FlashLoanRouterCalls {
        const COUNT: usize = 2usize;
        const MIN_DATA_LENGTH: usize = 0usize;
        const NAME: &'static str = "FlashLoanRouterCalls";

        #[inline]
        fn selector(&self) -> [u8; 4] {
            match self {
                Self::flashLoanAndSettle(_) => {
                    <flashLoanAndSettleCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::settlementContract(_) => {
                    <settlementContractCall as alloy_sol_types::SolCall>::SELECTOR
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
            static DECODE_SHIMS: &[fn(&[u8]) -> alloy_sol_types::Result<FlashLoanRouterCalls>] = &[
                {
                    fn flashLoanAndSettle(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<FlashLoanRouterCalls> {
                        <flashLoanAndSettleCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(FlashLoanRouterCalls::flashLoanAndSettle)
                    }
                    flashLoanAndSettle
                },
                {
                    fn settlementContract(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<FlashLoanRouterCalls> {
                        <settlementContractCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(FlashLoanRouterCalls::settlementContract)
                    }
                    settlementContract
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
                -> alloy_sol_types::Result<FlashLoanRouterCalls>] = &[
                {
                    fn flashLoanAndSettle(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<FlashLoanRouterCalls> {
                        <flashLoanAndSettleCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(FlashLoanRouterCalls::flashLoanAndSettle)
                    }
                    flashLoanAndSettle
                },
                {
                    fn settlementContract(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<FlashLoanRouterCalls> {
                        <settlementContractCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(FlashLoanRouterCalls::settlementContract)
                    }
                    settlementContract
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
                Self::flashLoanAndSettle(inner) => {
                    <flashLoanAndSettleCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::settlementContract(inner) => {
                    <settlementContractCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
            }
        }

        #[inline]
        fn abi_encode_raw(&self, out: &mut alloy_sol_types::private::Vec<u8>) {
            match self {
                Self::flashLoanAndSettle(inner) => {
                    <flashLoanAndSettleCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::settlementContract(inner) => {
                    <settlementContractCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
            }
        }
    }
    use alloy_contract;
    /**Creates a new wrapper around an on-chain [`FlashLoanRouter`](self) contract instance.

    See the [wrapper's documentation](`FlashLoanRouterInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> FlashLoanRouterInstance<P, N> {
        FlashLoanRouterInstance::<P, N>::new(address, __provider)
    }
    /**Deploys this contract using the given `provider` and constructor arguments, if any.

    Returns a new instance of the contract, if the deployment was successful.

    For more fine-grained control over the deployment process, use [`deploy_builder`] instead.*/
    #[inline]
    pub fn deploy<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>(
        __provider: P,
        _settlementContract: alloy_sol_types::private::Address,
    ) -> impl ::core::future::Future<Output = alloy_contract::Result<FlashLoanRouterInstance<P, N>>>
    {
        FlashLoanRouterInstance::<P, N>::deploy(__provider, _settlementContract)
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
        _settlementContract: alloy_sol_types::private::Address,
    ) -> alloy_contract::RawCallBuilder<P, N> {
        FlashLoanRouterInstance::<P, N>::deploy_builder(__provider, _settlementContract)
    }
    /**A [`FlashLoanRouter`](self) instance.

    Contains type-safe methods for interacting with an on-chain instance of the
    [`FlashLoanRouter`](self) contract located at a given `address`, using a given
    provider `P`.

    If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
    documentation on how to provide it), the `deploy` and `deploy_builder` methods can
    be used to deploy a new instance of the contract.

    See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct FlashLoanRouterInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for FlashLoanRouterInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("FlashLoanRouterInstance")
                .field(&self.address)
                .finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        FlashLoanRouterInstance<P, N>
    {
        /**Creates a new wrapper around an on-chain [`FlashLoanRouter`](self) contract instance.

        See the [wrapper's documentation](`FlashLoanRouterInstance`) for more details.*/
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
            _settlementContract: alloy_sol_types::private::Address,
        ) -> alloy_contract::Result<FlashLoanRouterInstance<P, N>> {
            let call_builder = Self::deploy_builder(__provider, _settlementContract);
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
            _settlementContract: alloy_sol_types::private::Address,
        ) -> alloy_contract::RawCallBuilder<P, N> {
            alloy_contract::RawCallBuilder::new_raw_deploy(
                __provider,
                [
                    &BYTECODE[..],
                    &alloy_sol_types::SolConstructor::abi_encode(&constructorCall {
                        _settlementContract,
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
    impl<P: ::core::clone::Clone, N> FlashLoanRouterInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned
        /// provider.
        #[inline]
        pub fn with_cloned_provider(self) -> FlashLoanRouterInstance<P, N> {
            FlashLoanRouterInstance {
                address: self.address,
                provider: ::core::clone::Clone::clone(&self.provider),
                _network: ::core::marker::PhantomData,
            }
        }
    }
    /// Function calls.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        FlashLoanRouterInstance<P, N>
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

        ///Creates a new call builder for the [`flashLoanAndSettle`] function.
        pub fn flashLoanAndSettle(
            &self,
            loans: alloy_sol_types::private::Vec<
                <LoanRequest::Data as alloy_sol_types::SolType>::RustType,
            >,
            settlement: alloy_sol_types::private::Bytes,
        ) -> alloy_contract::SolCallBuilder<&P, flashLoanAndSettleCall, N> {
            self.call_builder(&flashLoanAndSettleCall { loans, settlement })
        }

        ///Creates a new call builder for the [`settlementContract`] function.
        pub fn settlementContract(
            &self,
        ) -> alloy_contract::SolCallBuilder<&P, settlementContractCall, N> {
            self.call_builder(&settlementContractCall)
        }
    }
    /// Event filters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        FlashLoanRouterInstance<P, N>
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
pub type Instance = FlashLoanRouter::FlashLoanRouterInstance<::alloy_provider::DynProvider>;
use {
    alloy_primitives::{Address, address},
    alloy_provider::{DynProvider, Provider},
    anyhow::{Context, Result},
    std::{collections::HashMap, sync::LazyLock},
};
pub const fn deployment_info(chain_id: u64) -> Option<(Address, Option<u64>)> {
    match chain_id {
        1u64 => Some((
            ::alloy_primitives::address!("0x9da8b48441583a2b93e2ef8213aad0ec0b392c69"),
            None,
        )),
        100u64 => Some((
            ::alloy_primitives::address!("0x9da8b48441583a2b93e2ef8213aad0ec0b392c69"),
            None,
        )),
        137u64 => Some((
            ::alloy_primitives::address!("0x9da8b48441583a2b93e2ef8213aad0ec0b392c69"),
            None,
        )),
        8453u64 => Some((
            ::alloy_primitives::address!("0x9da8b48441583a2b93e2ef8213aad0ec0b392c69"),
            None,
        )),
        42161u64 => Some((
            ::alloy_primitives::address!("0x9da8b48441583a2b93e2ef8213aad0ec0b392c69"),
            None,
        )),
        43114u64 => Some((
            ::alloy_primitives::address!("0x9da8b48441583a2b93e2ef8213aad0ec0b392c69"),
            None,
        )),
        11155111u64 => Some((
            ::alloy_primitives::address!("0x9da8b48441583a2b93e2ef8213aad0ec0b392c69"),
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
