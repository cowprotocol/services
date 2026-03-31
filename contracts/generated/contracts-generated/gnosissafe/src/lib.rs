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
library Enum {
    type Operation is uint8;
}
```*/
#[allow(
    non_camel_case_types,
    non_snake_case,
    clippy::pub_underscore_fields,
    clippy::style,
    clippy::empty_structs_with_brackets
)]
pub mod Enum {
    use super::*;
    use alloy_sol_types;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct Operation(u8);
    const _: () = {
        use alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::private::SolTypeValue<Operation> for u8 {
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
        impl Operation {
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
        impl From<u8> for Operation {
            fn from(value: u8) -> Self {
                Self::from_underlying(value)
            }
        }
        #[automatically_derived]
        impl From<Operation> for u8 {
            fn from(value: Operation) -> Self {
                value.into_underlying()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolType for Operation {
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
        impl alloy_sol_types::EventTopic for Operation {
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
    use alloy_contract;
    /**Creates a new wrapper around an on-chain [`Enum`](self) contract instance.

    See the [wrapper's documentation](`EnumInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> EnumInstance<P, N> {
        EnumInstance::<P, N>::new(address, __provider)
    }
    /**A [`Enum`](self) instance.

    Contains type-safe methods for interacting with an on-chain instance of the
    [`Enum`](self) contract located at a given `address`, using a given
    provider `P`.

    If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
    documentation on how to provide it), the `deploy` and `deploy_builder` methods can
    be used to deploy a new instance of the contract.

    See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct EnumInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for EnumInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("EnumInstance").field(&self.address).finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        EnumInstance<P, N>
    {
        /**Creates a new wrapper around an on-chain [`Enum`](self) contract instance.

        See the [wrapper's documentation](`EnumInstance`) for more details.*/
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
    impl<P: ::core::clone::Clone, N> EnumInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned provider.
        #[inline]
        pub fn with_cloned_provider(self) -> EnumInstance<P, N> {
            EnumInstance {
                address: self.address,
                provider: ::core::clone::Clone::clone(&self.provider),
                _network: ::core::marker::PhantomData,
            }
        }
    }
    /// Function calls.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        EnumInstance<P, N>
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
        EnumInstance<P, N>
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
library Enum {
    type Operation is uint8;
}

interface GnosisSafe {
    event AddedOwner(address owner);
    event ApproveHash(bytes32 indexed approvedHash, address indexed owner);
    event ChangedFallbackHandler(address handler);
    event ChangedGuard(address guard);
    event ChangedThreshold(uint256 threshold);
    event DisabledModule(address module);
    event EnabledModule(address module);
    event ExecutionFailure(bytes32 txHash, uint256 payment);
    event ExecutionFromModuleFailure(address indexed module);
    event ExecutionFromModuleSuccess(address indexed module);
    event ExecutionSuccess(bytes32 txHash, uint256 payment);
    event RemovedOwner(address owner);
    event SafeReceived(address indexed sender, uint256 value);
    event SafeSetup(address indexed initiator, address[] owners, uint256 threshold, address initializer, address fallbackHandler);
    event SignMsg(bytes32 indexed msgHash);

    constructor();

    fallback() external;

    receive() external payable;

    function VERSION() external view returns (string memory);
    function addOwnerWithThreshold(address owner, uint256 _threshold) external;
    function approveHash(bytes32 hashToApprove) external;
    function approvedHashes(address, bytes32) external view returns (uint256);
    function changeThreshold(uint256 _threshold) external;
    function checkNSignatures(bytes32 dataHash, bytes memory data, bytes memory signatures, uint256 requiredSignatures) external view;
    function checkSignatures(bytes32 dataHash, bytes memory data, bytes memory signatures) external view;
    function disableModule(address prevModule, address module) external;
    function domainSeparator() external view returns (bytes32);
    function enableModule(address module) external;
    function encodeTransactionData(address to, uint256 value, bytes memory data, Enum.Operation operation, uint256 safeTxGas, uint256 baseGas, uint256 gasPrice, address gasToken, address refundReceiver, uint256 _nonce) external view returns (bytes memory);
    function execTransaction(address to, uint256 value, bytes memory data, Enum.Operation operation, uint256 safeTxGas, uint256 baseGas, uint256 gasPrice, address gasToken, address payable refundReceiver, bytes memory signatures) external payable returns (bool success);
    function execTransactionFromModule(address to, uint256 value, bytes memory data, Enum.Operation operation) external returns (bool success);
    function execTransactionFromModuleReturnData(address to, uint256 value, bytes memory data, Enum.Operation operation) external returns (bool success, bytes memory returnData);
    function getChainId() external view returns (uint256);
    function getModulesPaginated(address start, uint256 pageSize) external view returns (address[] memory array, address next);
    function getOwners() external view returns (address[] memory);
    function getStorageAt(uint256 offset, uint256 length) external view returns (bytes memory);
    function getThreshold() external view returns (uint256);
    function getTransactionHash(address to, uint256 value, bytes memory data, Enum.Operation operation, uint256 safeTxGas, uint256 baseGas, uint256 gasPrice, address gasToken, address refundReceiver, uint256 _nonce) external view returns (bytes32);
    function isModuleEnabled(address module) external view returns (bool);
    function isOwner(address owner) external view returns (bool);
    function nonce() external view returns (uint256);
    function removeOwner(address prevOwner, address owner, uint256 _threshold) external;
    function requiredTxGas(address to, uint256 value, bytes memory data, Enum.Operation operation) external returns (uint256);
    function setFallbackHandler(address handler) external;
    function setGuard(address guard) external;
    function setup(address[] memory _owners, uint256 _threshold, address to, bytes memory data, address fallbackHandler, address paymentToken, uint256 payment, address payable paymentReceiver) external;
    function signedMessages(bytes32) external view returns (uint256);
    function simulateAndRevert(address targetContract, bytes memory calldataPayload) external;
    function swapOwner(address prevOwner, address oldOwner, address newOwner) external;
}
```

...which was generated by the following JSON ABI:
```json
[
  {
    "type": "constructor",
    "inputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "fallback",
    "stateMutability": "nonpayable"
  },
  {
    "type": "receive",
    "stateMutability": "payable"
  },
  {
    "type": "function",
    "name": "VERSION",
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
    "type": "function",
    "name": "addOwnerWithThreshold",
    "inputs": [
      {
        "name": "owner",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "_threshold",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "approveHash",
    "inputs": [
      {
        "name": "hashToApprove",
        "type": "bytes32",
        "internalType": "bytes32"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "approvedHashes",
    "inputs": [
      {
        "name": "",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "",
        "type": "bytes32",
        "internalType": "bytes32"
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
    "name": "changeThreshold",
    "inputs": [
      {
        "name": "_threshold",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "checkNSignatures",
    "inputs": [
      {
        "name": "dataHash",
        "type": "bytes32",
        "internalType": "bytes32"
      },
      {
        "name": "data",
        "type": "bytes",
        "internalType": "bytes"
      },
      {
        "name": "signatures",
        "type": "bytes",
        "internalType": "bytes"
      },
      {
        "name": "requiredSignatures",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "outputs": [],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "checkSignatures",
    "inputs": [
      {
        "name": "dataHash",
        "type": "bytes32",
        "internalType": "bytes32"
      },
      {
        "name": "data",
        "type": "bytes",
        "internalType": "bytes"
      },
      {
        "name": "signatures",
        "type": "bytes",
        "internalType": "bytes"
      }
    ],
    "outputs": [],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "disableModule",
    "inputs": [
      {
        "name": "prevModule",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "module",
        "type": "address",
        "internalType": "address"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
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
    "name": "enableModule",
    "inputs": [
      {
        "name": "module",
        "type": "address",
        "internalType": "address"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "encodeTransactionData",
    "inputs": [
      {
        "name": "to",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "value",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "data",
        "type": "bytes",
        "internalType": "bytes"
      },
      {
        "name": "operation",
        "type": "uint8",
        "internalType": "enum Enum.Operation"
      },
      {
        "name": "safeTxGas",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "baseGas",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "gasPrice",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "gasToken",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "refundReceiver",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "_nonce",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "outputs": [
      {
        "name": "",
        "type": "bytes",
        "internalType": "bytes"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "execTransaction",
    "inputs": [
      {
        "name": "to",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "value",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "data",
        "type": "bytes",
        "internalType": "bytes"
      },
      {
        "name": "operation",
        "type": "uint8",
        "internalType": "enum Enum.Operation"
      },
      {
        "name": "safeTxGas",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "baseGas",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "gasPrice",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "gasToken",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "refundReceiver",
        "type": "address",
        "internalType": "address payable"
      },
      {
        "name": "signatures",
        "type": "bytes",
        "internalType": "bytes"
      }
    ],
    "outputs": [
      {
        "name": "success",
        "type": "bool",
        "internalType": "bool"
      }
    ],
    "stateMutability": "payable"
  },
  {
    "type": "function",
    "name": "execTransactionFromModule",
    "inputs": [
      {
        "name": "to",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "value",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "data",
        "type": "bytes",
        "internalType": "bytes"
      },
      {
        "name": "operation",
        "type": "uint8",
        "internalType": "enum Enum.Operation"
      }
    ],
    "outputs": [
      {
        "name": "success",
        "type": "bool",
        "internalType": "bool"
      }
    ],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "execTransactionFromModuleReturnData",
    "inputs": [
      {
        "name": "to",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "value",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "data",
        "type": "bytes",
        "internalType": "bytes"
      },
      {
        "name": "operation",
        "type": "uint8",
        "internalType": "enum Enum.Operation"
      }
    ],
    "outputs": [
      {
        "name": "success",
        "type": "bool",
        "internalType": "bool"
      },
      {
        "name": "returnData",
        "type": "bytes",
        "internalType": "bytes"
      }
    ],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "getChainId",
    "inputs": [],
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
    "name": "getModulesPaginated",
    "inputs": [
      {
        "name": "start",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "pageSize",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "outputs": [
      {
        "name": "array",
        "type": "address[]",
        "internalType": "address[]"
      },
      {
        "name": "next",
        "type": "address",
        "internalType": "address"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "getOwners",
    "inputs": [],
    "outputs": [
      {
        "name": "",
        "type": "address[]",
        "internalType": "address[]"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "getStorageAt",
    "inputs": [
      {
        "name": "offset",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "length",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "outputs": [
      {
        "name": "",
        "type": "bytes",
        "internalType": "bytes"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "getThreshold",
    "inputs": [],
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
    "name": "getTransactionHash",
    "inputs": [
      {
        "name": "to",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "value",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "data",
        "type": "bytes",
        "internalType": "bytes"
      },
      {
        "name": "operation",
        "type": "uint8",
        "internalType": "enum Enum.Operation"
      },
      {
        "name": "safeTxGas",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "baseGas",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "gasPrice",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "gasToken",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "refundReceiver",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "_nonce",
        "type": "uint256",
        "internalType": "uint256"
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
    "name": "isModuleEnabled",
    "inputs": [
      {
        "name": "module",
        "type": "address",
        "internalType": "address"
      }
    ],
    "outputs": [
      {
        "name": "",
        "type": "bool",
        "internalType": "bool"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "isOwner",
    "inputs": [
      {
        "name": "owner",
        "type": "address",
        "internalType": "address"
      }
    ],
    "outputs": [
      {
        "name": "",
        "type": "bool",
        "internalType": "bool"
      }
    ],
    "stateMutability": "view"
  },
  {
    "type": "function",
    "name": "nonce",
    "inputs": [],
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
    "name": "removeOwner",
    "inputs": [
      {
        "name": "prevOwner",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "owner",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "_threshold",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "requiredTxGas",
    "inputs": [
      {
        "name": "to",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "value",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "data",
        "type": "bytes",
        "internalType": "bytes"
      },
      {
        "name": "operation",
        "type": "uint8",
        "internalType": "enum Enum.Operation"
      }
    ],
    "outputs": [
      {
        "name": "",
        "type": "uint256",
        "internalType": "uint256"
      }
    ],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "setFallbackHandler",
    "inputs": [
      {
        "name": "handler",
        "type": "address",
        "internalType": "address"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "setGuard",
    "inputs": [
      {
        "name": "guard",
        "type": "address",
        "internalType": "address"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "setup",
    "inputs": [
      {
        "name": "_owners",
        "type": "address[]",
        "internalType": "address[]"
      },
      {
        "name": "_threshold",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "to",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "data",
        "type": "bytes",
        "internalType": "bytes"
      },
      {
        "name": "fallbackHandler",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "paymentToken",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "payment",
        "type": "uint256",
        "internalType": "uint256"
      },
      {
        "name": "paymentReceiver",
        "type": "address",
        "internalType": "address payable"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "signedMessages",
    "inputs": [
      {
        "name": "",
        "type": "bytes32",
        "internalType": "bytes32"
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
    "name": "simulateAndRevert",
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
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "function",
    "name": "swapOwner",
    "inputs": [
      {
        "name": "prevOwner",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "oldOwner",
        "type": "address",
        "internalType": "address"
      },
      {
        "name": "newOwner",
        "type": "address",
        "internalType": "address"
      }
    ],
    "outputs": [],
    "stateMutability": "nonpayable"
  },
  {
    "type": "event",
    "name": "AddedOwner",
    "inputs": [
      {
        "name": "owner",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "ApproveHash",
    "inputs": [
      {
        "name": "approvedHash",
        "type": "bytes32",
        "indexed": true,
        "internalType": "bytes32"
      },
      {
        "name": "owner",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "ChangedFallbackHandler",
    "inputs": [
      {
        "name": "handler",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "ChangedGuard",
    "inputs": [
      {
        "name": "guard",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "ChangedThreshold",
    "inputs": [
      {
        "name": "threshold",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "DisabledModule",
    "inputs": [
      {
        "name": "module",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "EnabledModule",
    "inputs": [
      {
        "name": "module",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "ExecutionFailure",
    "inputs": [
      {
        "name": "txHash",
        "type": "bytes32",
        "indexed": false,
        "internalType": "bytes32"
      },
      {
        "name": "payment",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "ExecutionFromModuleFailure",
    "inputs": [
      {
        "name": "module",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "ExecutionFromModuleSuccess",
    "inputs": [
      {
        "name": "module",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "ExecutionSuccess",
    "inputs": [
      {
        "name": "txHash",
        "type": "bytes32",
        "indexed": false,
        "internalType": "bytes32"
      },
      {
        "name": "payment",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "RemovedOwner",
    "inputs": [
      {
        "name": "owner",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "SafeReceived",
    "inputs": [
      {
        "name": "sender",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "value",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "SafeSetup",
    "inputs": [
      {
        "name": "initiator",
        "type": "address",
        "indexed": true,
        "internalType": "address"
      },
      {
        "name": "owners",
        "type": "address[]",
        "indexed": false,
        "internalType": "address[]"
      },
      {
        "name": "threshold",
        "type": "uint256",
        "indexed": false,
        "internalType": "uint256"
      },
      {
        "name": "initializer",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      },
      {
        "name": "fallbackHandler",
        "type": "address",
        "indexed": false,
        "internalType": "address"
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "SignMsg",
    "inputs": [
      {
        "name": "msgHash",
        "type": "bytes32",
        "indexed": true,
        "internalType": "bytes32"
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
pub mod GnosisSafe {
    use super::*;
    use alloy_sol_types;
    /// The creation / init bytecode of the contract.
    ///
    /// ```text
    ///0x608060405234801561001057600080fd5b5060016004819055506159ae80620000296000396000f3fe6080604052600436106101dc5760003560e01c8063affed0e011610102578063e19a9dd911610095578063f08a032311610064578063f08a032314611647578063f698da2514611698578063f8dc5dd9146116c3578063ffa1ad741461173e57610231565b8063e19a9dd91461139b578063e318b52b146113ec578063e75235b81461147d578063e86637db146114a857610231565b8063cc2f8452116100d1578063cc2f8452146110e8578063d4d9bdcd146111b5578063d8d11f78146111f0578063e009cfde1461132a57610231565b8063affed0e014610d94578063b4faba0914610dbf578063b63e800d14610ea7578063c4ca3a9c1461101757610231565b80635624b25b1161017a5780636a761202116101495780636a761202146109945780637d83297414610b50578063934f3a1114610bbf578063a0e67e2b14610d2857610231565b80635624b25b146107fb5780635ae6bd37146108b9578063610b592514610908578063694e80c31461095957610231565b80632f54bf6e116101b65780632f54bf6e146104d35780633408e4701461053a578063468721a7146105655780635229073f1461067a57610231565b80630d582f131461029e57806312fb68e0146102f95780632d9ad53d1461046c57610231565b36610231573373ffffffffffffffffffffffffffffffffffffffff167f3d0ce9bfc3ed7d6862dbb28b2dea94561fe714a1b4d019aa8af39730d1ad7c3d346040518082815260200191505060405180910390a2005b34801561023d57600080fd5b5060007f6c9a6c4a39284e37ed1cf53d337577d14212a4870fb976a4366c693b939918d560001b905080548061027257600080f35b36600080373360601b365260008060143601600080855af13d6000803e80610299573d6000fd5b3d6000f35b3480156102aa57600080fd5b506102f7600480360360408110156102c157600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff169060200190929190803590602001909291905050506117ce565b005b34801561030557600080fd5b5061046a6004803603608081101561031c57600080fd5b81019080803590602001909291908035906020019064010000000081111561034357600080fd5b82018360208201111561035557600080fd5b8035906020019184600183028401116401000000008311171561037757600080fd5b91908080601f016020809104026020016040519081016040528093929190818152602001838380828437600081840152601f19601f820116905080830192505050505050509192919290803590602001906401000000008111156103da57600080fd5b8201836020820111156103ec57600080fd5b8035906020019184600183028401116401000000008311171561040e57600080fd5b91908080601f016020809104026020016040519081016040528093929190818152602001838380828437600081840152601f19601f82011690508083019250505050505050919291929080359060200190929190505050611bbe565b005b34801561047857600080fd5b506104bb6004803603602081101561048f57600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff169060200190929190505050612440565b60405180821515815260200191505060405180910390f35b3480156104df57600080fd5b50610522600480360360208110156104f657600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff169060200190929190505050612512565b60405180821515815260200191505060405180910390f35b34801561054657600080fd5b5061054f6125e4565b6040518082815260200191505060405180910390f35b34801561057157600080fd5b506106626004803603608081101561058857600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff16906020019092919080359060200190929190803590602001906401000000008111156105cf57600080fd5b8201836020820111156105e157600080fd5b8035906020019184600183028401116401000000008311171561060357600080fd5b91908080601f016020809104026020016040519081016040528093929190818152602001838380828437600081840152601f19601f820116905080830192505050505050509192919290803560ff1690602001909291905050506125f1565b60405180821515815260200191505060405180910390f35b34801561068657600080fd5b506107776004803603608081101561069d57600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff16906020019092919080359060200190929190803590602001906401000000008111156106e457600080fd5b8201836020820111156106f657600080fd5b8035906020019184600183028401116401000000008311171561071857600080fd5b91908080601f016020809104026020016040519081016040528093929190818152602001838380828437600081840152601f19601f820116905080830192505050505050509192919290803560ff1690602001909291905050506127d7565b60405180831515815260200180602001828103825283818151815260200191508051906020019080838360005b838110156107bf5780820151818401526020810190506107a4565b50505050905090810190601f1680156107ec5780820380516001836020036101000a031916815260200191505b50935050505060405180910390f35b34801561080757600080fd5b5061083e6004803603604081101561081e57600080fd5b81019080803590602001909291908035906020019092919050505061280d565b6040518080602001828103825283818151815260200191508051906020019080838360005b8381101561087e578082015181840152602081019050610863565b50505050905090810190601f1680156108ab5780820380516001836020036101000a031916815260200191505b509250505060405180910390f35b3480156108c557600080fd5b506108f2600480360360208110156108dc57600080fd5b8101908080359060200190929190505050612894565b6040518082815260200191505060405180910390f35b34801561091457600080fd5b506109576004803603602081101561092b57600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff1690602001909291905050506128ac565b005b34801561096557600080fd5b506109926004803603602081101561097c57600080fd5b8101908080359060200190929190505050612c3e565b005b610b3860048036036101408110156109ab57600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff16906020019092919080359060200190929190803590602001906401000000008111156109f257600080fd5b820183602082011115610a0457600080fd5b80359060200191846001830284011164010000000083111715610a2657600080fd5b9091929391929390803560ff169060200190929190803590602001909291908035906020019092919080359060200190929190803573ffffffffffffffffffffffffffffffffffffffff169060200190929190803573ffffffffffffffffffffffffffffffffffffffff16906020019092919080359060200190640100000000811115610ab257600080fd5b820183602082011115610ac457600080fd5b80359060200191846001830284011164010000000083111715610ae657600080fd5b91908080601f016020809104026020016040519081016040528093929190818152602001838380828437600081840152601f19601f820116905080830192505050505050509192919290505050612d78565b60405180821515815260200191505060405180910390f35b348015610b5c57600080fd5b50610ba960048036036040811015610b7357600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff169060200190929190803590602001909291905050506132b5565b6040518082815260200191505060405180910390f35b348015610bcb57600080fd5b50610d2660048036036060811015610be257600080fd5b810190808035906020019092919080359060200190640100000000811115610c0957600080fd5b820183602082011115610c1b57600080fd5b80359060200191846001830284011164010000000083111715610c3d57600080fd5b91908080601f016020809104026020016040519081016040528093929190818152602001838380828437600081840152601f19601f82011690508083019250505050505050919291929080359060200190640100000000811115610ca057600080fd5b820183602082011115610cb257600080fd5b80359060200191846001830284011164010000000083111715610cd457600080fd5b91908080601f016020809104026020016040519081016040528093929190818152602001838380828437600081840152601f19601f8201169050808301925050505050505091929192905050506132da565b005b348015610d3457600080fd5b50610d3d613369565b6040518080602001828103825283818151815260200191508051906020019060200280838360005b83811015610d80578082015181840152602081019050610d65565b505050509050019250505060405180910390f35b348015610da057600080fd5b50610da9613512565b6040518082815260200191505060405180910390f35b348015610dcb57600080fd5b50610ea560048036036040811015610de257600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff16906020019092919080359060200190640100000000811115610e1f57600080fd5b820183602082011115610e3157600080fd5b80359060200191846001830284011164010000000083111715610e5357600080fd5b91908080601f016020809104026020016040519081016040528093929190818152602001838380828437600081840152601f19601f820116905080830192505050505050509192919290505050613518565b005b348015610eb357600080fd5b506110156004803603610100811015610ecb57600080fd5b8101908080359060200190640100000000811115610ee857600080fd5b820183602082011115610efa57600080fd5b80359060200191846020830284011164010000000083111715610f1c57600080fd5b909192939192939080359060200190929190803573ffffffffffffffffffffffffffffffffffffffff16906020019092919080359060200190640100000000811115610f6757600080fd5b820183602082011115610f7957600080fd5b80359060200191846001830284011164010000000083111715610f9b57600080fd5b9091929391929390803573ffffffffffffffffffffffffffffffffffffffff169060200190929190803573ffffffffffffffffffffffffffffffffffffffff16906020019092919080359060200190929190803573ffffffffffffffffffffffffffffffffffffffff16906020019092919050505061353a565b005b34801561102357600080fd5b506110d26004803603608081101561103a57600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff169060200190929190803590602001909291908035906020019064010000000081111561108157600080fd5b82018360208201111561109357600080fd5b803590602001918460018302840111640100000000831117156110b557600080fd5b9091929391929390803560ff1690602001909291905050506136f8565b6040518082815260200191505060405180910390f35b3480156110f457600080fd5b506111416004803603604081101561110b57600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff16906020019092919080359060200190929190505050613820565b60405180806020018373ffffffffffffffffffffffffffffffffffffffff168152602001828103825284818151815260200191508051906020019060200280838360005b838110156111a0578082015181840152602081019050611185565b50505050905001935050505060405180910390f35b3480156111c157600080fd5b506111ee600480360360208110156111d857600080fd5b8101908080359060200190929190505050613a12565b005b3480156111fc57600080fd5b50611314600480360361014081101561121457600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff169060200190929190803590602001909291908035906020019064010000000081111561125b57600080fd5b82018360208201111561126d57600080fd5b8035906020019184600183028401116401000000008311171561128f57600080fd5b9091929391929390803560ff169060200190929190803590602001909291908035906020019092919080359060200190929190803573ffffffffffffffffffffffffffffffffffffffff169060200190929190803573ffffffffffffffffffffffffffffffffffffffff16906020019092919080359060200190929190505050613bb1565b6040518082815260200191505060405180910390f35b34801561133657600080fd5b506113996004803603604081101561134d57600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff169060200190929190803573ffffffffffffffffffffffffffffffffffffffff169060200190929190505050613bde565b005b3480156113a757600080fd5b506113ea600480360360208110156113be57600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff169060200190929190505050613f6f565b005b3480156113f857600080fd5b5061147b6004803603606081101561140f57600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff169060200190929190803573ffffffffffffffffffffffffffffffffffffffff169060200190929190803573ffffffffffffffffffffffffffffffffffffffff169060200190929190505050613ff3565b005b34801561148957600080fd5b50611492614665565b6040518082815260200191505060405180910390f35b3480156114b457600080fd5b506115cc60048036036101408110156114cc57600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff169060200190929190803590602001909291908035906020019064010000000081111561151357600080fd5b82018360208201111561152557600080fd5b8035906020019184600183028401116401000000008311171561154757600080fd5b9091929391929390803560ff169060200190929190803590602001909291908035906020019092919080359060200190929190803573ffffffffffffffffffffffffffffffffffffffff169060200190929190803573ffffffffffffffffffffffffffffffffffffffff1690602001909291908035906020019092919050505061466f565b6040518080602001828103825283818151815260200191508051906020019080838360005b8381101561160c5780820151818401526020810190506115f1565b50505050905090810190601f1680156116395780820380516001836020036101000a031916815260200191505b509250505060405180910390f35b34801561165357600080fd5b506116966004803603602081101561166a57600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff169060200190929190505050614817565b005b3480156116a457600080fd5b506116ad614878565b6040518082815260200191505060405180910390f35b3480156116cf57600080fd5b5061173c600480360360608110156116e657600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff169060200190929190803573ffffffffffffffffffffffffffffffffffffffff169060200190929190803590602001909291905050506148f6565b005b34801561174a57600080fd5b50611753614d29565b6040518080602001828103825283818151815260200191508051906020019080838360005b83811015611793578082015181840152602081019050611778565b50505050905090810190601f1680156117c05780820380516001836020036101000a031916815260200191505b509250505060405180910390f35b6117d6614d62565b600073ffffffffffffffffffffffffffffffffffffffff168273ffffffffffffffffffffffffffffffffffffffff16141580156118405750600173ffffffffffffffffffffffffffffffffffffffff168273ffffffffffffffffffffffffffffffffffffffff1614155b801561187857503073ffffffffffffffffffffffffffffffffffffffff168273ffffffffffffffffffffffffffffffffffffffff1614155b6118ea576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475332303300000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b600073ffffffffffffffffffffffffffffffffffffffff16600260008473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060009054906101000a900473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16146119eb576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475332303400000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b60026000600173ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060009054906101000a900473ffffffffffffffffffffffffffffffffffffffff16600260008473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060006101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055508160026000600173ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060006101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055506003600081548092919060010191905055507f9465fa0c962cc76958e6373a993326400c1c94f8be2fe3a952adfa7f60b2ea2682604051808273ffffffffffffffffffffffffffffffffffffffff16815260200191505060405180910390a18060045414611bba57611bb981612c3e565b5b5050565b611bd2604182614e0590919063ffffffff16565b82511015611c48576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475330323000000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b6000808060008060005b8681101561243457611c648882614e3f565b80945081955082965050505060008460ff16141561206d578260001c9450611c96604188614e0590919063ffffffff16565b8260001c1015611d0e576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475330323100000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b8751611d2760208460001c614e6e90919063ffffffff16565b1115611d9b576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475330323200000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b60006020838a01015190508851611dd182611dc360208760001c614e6e90919063ffffffff16565b614e6e90919063ffffffff16565b1115611e45576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475330323300000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b60606020848b010190506320c13b0b60e01b7bffffffffffffffffffffffffffffffffffffffffffffffffffffffff19168773ffffffffffffffffffffffffffffffffffffffff166320c13b0b8d846040518363ffffffff1660e01b8152600401808060200180602001838103835285818151815260200191508051906020019080838360005b83811015611ee7578082015181840152602081019050611ecc565b50505050905090810190601f168015611f145780820380516001836020036101000a031916815260200191505b50838103825284818151815260200191508051906020019080838360005b83811015611f4d578082015181840152602081019050611f32565b50505050905090810190601f168015611f7a5780820380516001836020036101000a031916815260200191505b5094505050505060206040518083038186803b158015611f9957600080fd5b505afa158015611fad573d6000803e3d6000fd5b505050506040513d6020811015611fc357600080fd5b81019080805190602001909291905050507bffffffffffffffffffffffffffffffffffffffffffffffffffffffff191614612066576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475330323400000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b50506122b2565b60018460ff161415612181578260001c94508473ffffffffffffffffffffffffffffffffffffffff163373ffffffffffffffffffffffffffffffffffffffff16148061210a57506000600860008773ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060008c81526020019081526020016000205414155b61217c576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475330323500000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b6122b1565b601e8460ff1611156122495760018a60405160200180807f19457468657265756d205369676e6564204d6573736167653a0a333200000000815250601c018281526020019150506040516020818303038152906040528051906020012060048603858560405160008152602001604052604051808581526020018460ff1681526020018381526020018281526020019450505050506020604051602081039080840390855afa158015612238573d6000803e3d6000fd5b5050506020604051035194506122b0565b60018a85858560405160008152602001604052604051808581526020018460ff1681526020018381526020018281526020019450505050506020604051602081039080840390855afa1580156122a3573d6000803e3d6000fd5b5050506020604051035194505b5b5b8573ffffffffffffffffffffffffffffffffffffffff168573ffffffffffffffffffffffffffffffffffffffff161180156123795750600073ffffffffffffffffffffffffffffffffffffffff16600260008773ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060009054906101000a900473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1614155b80156123b25750600173ffffffffffffffffffffffffffffffffffffffff168573ffffffffffffffffffffffffffffffffffffffff1614155b612424576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475330323600000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b8495508080600101915050611c52565b50505050505050505050565b60008173ffffffffffffffffffffffffffffffffffffffff16600173ffffffffffffffffffffffffffffffffffffffff161415801561250b5750600073ffffffffffffffffffffffffffffffffffffffff16600160008473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060009054906101000a900473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1614155b9050919050565b6000600173ffffffffffffffffffffffffffffffffffffffff168273ffffffffffffffffffffffffffffffffffffffff16141580156125dd5750600073ffffffffffffffffffffffffffffffffffffffff16600260008473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060009054906101000a900473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1614155b9050919050565b6000804690508091505090565b6000600173ffffffffffffffffffffffffffffffffffffffff163373ffffffffffffffffffffffffffffffffffffffff16141580156126bc5750600073ffffffffffffffffffffffffffffffffffffffff16600160003373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060009054906101000a900473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1614155b61272e576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475331303400000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b61273b858585855a614e8d565b9050801561278b573373ffffffffffffffffffffffffffffffffffffffff167f6895c13664aa4f67288b25d7a21d7aaa34916e355fb9b6fae0a139a9085becb860405160405180910390a26127cf565b3373ffffffffffffffffffffffffffffffffffffffff167facd2c8702804128fdb0db2bb49f6d127dd0181c13fd45dbfe16de0930e2bd37560405160405180910390a25b949350505050565b600060606127e7868686866125f1565b915060405160203d0181016040523d81523d6000602083013e8091505094509492505050565b606060006020830267ffffffffffffffff8111801561282b57600080fd5b506040519080825280601f01601f19166020018201604052801561285e5781602001600182028036833780820191505090505b50905060005b8381101561288957808501548060208302602085010152508080600101915050612864565b508091505092915050565b60076020528060005260406000206000915090505481565b6128b4614d62565b600073ffffffffffffffffffffffffffffffffffffffff168173ffffffffffffffffffffffffffffffffffffffff161415801561291e5750600173ffffffffffffffffffffffffffffffffffffffff168173ffffffffffffffffffffffffffffffffffffffff1614155b612990576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475331303100000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b600073ffffffffffffffffffffffffffffffffffffffff16600160008373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060009054906101000a900473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1614612a91576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475331303200000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b60016000600173ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060009054906101000a900473ffffffffffffffffffffffffffffffffffffffff16600160008373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060006101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055508060016000600173ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060006101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055507fecdf3a3effea5783a3c4c2140e677577666428d44ed9d474a0b3a4c9943f844081604051808273ffffffffffffffffffffffffffffffffffffffff16815260200191505060405180910390a150565b612c46614d62565b600354811115612cbe576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475332303100000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b6001811015612d35576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475332303200000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b806004819055507f610f7ff2b304ae8903c3de74c60c6ab1f7d6226b3f52c5161905bb5ad4039c936004546040518082815260200191505060405180910390a150565b6000806000612d928e8e8e8e8e8e8e8e8e8e60055461466f565b905060056000815480929190600101919050555080805190602001209150612dbb8282866132da565b506000612dc6614ed9565b9050600073ffffffffffffffffffffffffffffffffffffffff168173ffffffffffffffffffffffffffffffffffffffff1614612fac578073ffffffffffffffffffffffffffffffffffffffff166375f0bb528f8f8f8f8f8f8f8f8f8f8f336040518d63ffffffff1660e01b8152600401808d73ffffffffffffffffffffffffffffffffffffffff1681526020018c8152602001806020018a6001811115612e6957fe5b81526020018981526020018881526020018781526020018673ffffffffffffffffffffffffffffffffffffffff1681526020018573ffffffffffffffffffffffffffffffffffffffff168152602001806020018473ffffffffffffffffffffffffffffffffffffffff16815260200183810383528d8d82818152602001925080828437600081840152601f19601f820116905080830192505050838103825285818151815260200191508051906020019080838360005b83811015612f3b578082015181840152602081019050612f20565b50505050905090810190601f168015612f685780820380516001836020036101000a031916815260200191505b509e505050505050505050505050505050600060405180830381600087803b158015612f9357600080fd5b505af1158015612fa7573d6000803e3d6000fd5b505050505b6101f4612fd36109c48b01603f60408d0281612fc457fe5b04614f0a90919063ffffffff16565b015a1015613049576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475330313000000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b60005a90506130b28f8f8f8f8080601f016020809104026020016040519081016040528093929190818152602001838380828437600081840152601f19601f820116905080830192505050505050508e60008d146130a7578e6130ad565b6109c45a035b614e8d565b93506130c75a82614f2490919063ffffffff16565b905083806130d6575060008a14155b806130e2575060008814155b613154576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475330313300000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b60008089111561316e5761316b828b8b8b8b614f44565b90505b84156131b8577f442e715f626346e8c54381002da614f62bee8d27386535b2521ec8540898556e8482604051808381526020018281526020019250505060405180910390a16131f8565b7f23428b18acfb3ea64b08dc0c1d296ea9c09702c09083ca5272e64d115b687d238482604051808381526020018281526020019250505060405180910390a15b5050600073ffffffffffffffffffffffffffffffffffffffff168173ffffffffffffffffffffffffffffffffffffffff16146132a4578073ffffffffffffffffffffffffffffffffffffffff16639327136883856040518363ffffffff1660e01b815260040180838152602001821515815260200192505050600060405180830381600087803b15801561328b57600080fd5b505af115801561329f573d6000803e3d6000fd5b505050505b50509b9a5050505050505050505050565b6008602052816000526040600020602052806000526040600020600091509150505481565b6000600454905060008111613357576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475330303100000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b61336384848484611bbe565b50505050565b6060600060035467ffffffffffffffff8111801561338657600080fd5b506040519080825280602002602001820160405280156133b55781602001602082028036833780820191505090505b50905060008060026000600173ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060009054906101000a900473ffffffffffffffffffffffffffffffffffffffff1690505b600173ffffffffffffffffffffffffffffffffffffffff168173ffffffffffffffffffffffffffffffffffffffff1614613509578083838151811061346057fe5b602002602001019073ffffffffffffffffffffffffffffffffffffffff16908173ffffffffffffffffffffffffffffffffffffffff1681525050600260008273ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060009054906101000a900473ffffffffffffffffffffffffffffffffffffffff169050818060010192505061341f565b82935050505090565b60055481565b600080825160208401855af4806000523d6020523d600060403e60403d016000fd5b6135858a8a80806020026020016040519081016040528093929190818152602001838360200280828437600081840152601f19601f820116905080830192505050505050508961514a565b600073ffffffffffffffffffffffffffffffffffffffff168473ffffffffffffffffffffffffffffffffffffffff16146135c3576135c28461564a565b5b6136118787878080601f016020809104026020016040519081016040528093929190818152602001838380828437600081840152601f19601f82011690508083019250505050505050615679565b600082111561362b5761362982600060018685614f44565b505b3373ffffffffffffffffffffffffffffffffffffffff167f141df868a6331af528e38c83b7aa03edc19be66e37ae67f9285bf4f8e3c6a1a88b8b8b8b8960405180806020018581526020018473ffffffffffffffffffffffffffffffffffffffff1681526020018373ffffffffffffffffffffffffffffffffffffffff1681526020018281038252878782818152602001925060200280828437600081840152601f19601f820116905080830192505050965050505050505060405180910390a250505050505050505050565b6000805a905061374f878787878080601f016020809104026020016040519081016040528093929190818152602001838380828437600081840152601f19601f82011690508083019250505050505050865a614e8d565b61375857600080fd5b60005a8203905080604051602001808281526020019150506040516020818303038152906040526040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825283818151815260200191508051906020019080838360005b838110156137e55780820151818401526020810190506137ca565b50505050905090810190601f1680156138125780820380516001836020036101000a031916815260200191505b509250505060405180910390fd5b606060008267ffffffffffffffff8111801561383b57600080fd5b5060405190808252806020026020018201604052801561386a5781602001602082028036833780820191505090505b509150600080600160008773ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060009054906101000a900473ffffffffffffffffffffffffffffffffffffffff1690505b600073ffffffffffffffffffffffffffffffffffffffff168173ffffffffffffffffffffffffffffffffffffffff161415801561393d5750600173ffffffffffffffffffffffffffffffffffffffff168173ffffffffffffffffffffffffffffffffffffffff1614155b801561394857508482105b15613a03578084838151811061395a57fe5b602002602001019073ffffffffffffffffffffffffffffffffffffffff16908173ffffffffffffffffffffffffffffffffffffffff1681525050600160008273ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060009054906101000a900473ffffffffffffffffffffffffffffffffffffffff16905081806001019250506138d3565b80925081845250509250929050565b600073ffffffffffffffffffffffffffffffffffffffff16600260003373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060009054906101000a900473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff161415613b14576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475330333000000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b6001600860003373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020016000206000838152602001908152602001600020819055503373ffffffffffffffffffffffffffffffffffffffff16817ff2a0eb156472d1440255b0d7c1e19cc07115d1051fe605b0dce69acfec884d9c60405160405180910390a350565b6000613bc68c8c8c8c8c8c8c8c8c8c8c61466f565b8051906020012090509b9a5050505050505050505050565b613be6614d62565b600073ffffffffffffffffffffffffffffffffffffffff168173ffffffffffffffffffffffffffffffffffffffff1614158015613c505750600173ffffffffffffffffffffffffffffffffffffffff168173ffffffffffffffffffffffffffffffffffffffff1614155b613cc2576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475331303100000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b8073ffffffffffffffffffffffffffffffffffffffff16600160008473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060009054906101000a900473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1614613dc2576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475331303300000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b600160008273ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060009054906101000a900473ffffffffffffffffffffffffffffffffffffffff16600160008473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060006101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055506000600160008373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060006101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055507faab4fa2b463f581b2b32cb3b7e3b704b9ce37cc209b5fb4d77e593ace405427681604051808273ffffffffffffffffffffffffffffffffffffffff16815260200191505060405180910390a15050565b613f77614d62565b60007f4a204f620c8c5ccdca3fd54d003badd85ba500436a431f0cbda4f558c93c34c860001b90508181557f1151116914515bc0891ff9047a6cb32cf902546f83066499bcf8ba33d2353fa282604051808273ffffffffffffffffffffffffffffffffffffffff16815260200191505060405180910390a15050565b613ffb614d62565b600073ffffffffffffffffffffffffffffffffffffffff168173ffffffffffffffffffffffffffffffffffffffff16141580156140655750600173ffffffffffffffffffffffffffffffffffffffff168173ffffffffffffffffffffffffffffffffffffffff1614155b801561409d57503073ffffffffffffffffffffffffffffffffffffffff168173ffffffffffffffffffffffffffffffffffffffff1614155b61410f576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475332303300000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b600073ffffffffffffffffffffffffffffffffffffffff16600260008373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060009054906101000a900473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1614614210576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475332303400000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b600073ffffffffffffffffffffffffffffffffffffffff168273ffffffffffffffffffffffffffffffffffffffff161415801561427a5750600173ffffffffffffffffffffffffffffffffffffffff168273ffffffffffffffffffffffffffffffffffffffff1614155b6142ec576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475332303300000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b8173ffffffffffffffffffffffffffffffffffffffff16600260008573ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060009054906101000a900473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16146143ec576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475332303500000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b600260008373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060009054906101000a900473ffffffffffffffffffffffffffffffffffffffff16600260008373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060006101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff16021790555080600260008573ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060006101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055506000600260008473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060006101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055507ff8d49fc529812e9a7c5c50e69c20f0dccc0db8fa95c98bc58cc9a4f1c1299eaf82604051808273ffffffffffffffffffffffffffffffffffffffff16815260200191505060405180910390a17f9465fa0c962cc76958e6373a993326400c1c94f8be2fe3a952adfa7f60b2ea2681604051808273ffffffffffffffffffffffffffffffffffffffff16815260200191505060405180910390a1505050565b6000600454905090565b606060007fbb8310d486368db6bd6f849402fdd73ad53d316b5a4b2644ad6efe0f941286d860001b8d8d8d8d60405180838380828437808301925050509250505060405180910390208c8c8c8c8c8c8c604051602001808c81526020018b73ffffffffffffffffffffffffffffffffffffffff1681526020018a815260200189815260200188600181111561470057fe5b81526020018781526020018681526020018581526020018473ffffffffffffffffffffffffffffffffffffffff1681526020018373ffffffffffffffffffffffffffffffffffffffff1681526020018281526020019b505050505050505050505050604051602081830303815290604052805190602001209050601960f81b600160f81b61478c614878565b8360405160200180857effffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff19168152600101847effffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff191681526001018381526020018281526020019450505050506040516020818303038152906040529150509b9a5050505050505050505050565b61481f614d62565b6148288161564a565b7f5ac6c46c93c8d0e53714ba3b53db3e7c046da994313d7ed0d192028bc7c228b081604051808273ffffffffffffffffffffffffffffffffffffffff16815260200191505060405180910390a150565b60007f47e79534a245952e8b16893a336b85a3d9ea9fa8c573f3d803afb92a7946921860001b6148a66125e4565b30604051602001808481526020018381526020018273ffffffffffffffffffffffffffffffffffffffff168152602001935050505060405160208183030381529060405280519060200120905090565b6148fe614d62565b806001600354031015614979576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475332303100000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b600073ffffffffffffffffffffffffffffffffffffffff168273ffffffffffffffffffffffffffffffffffffffff16141580156149e35750600173ffffffffffffffffffffffffffffffffffffffff168273ffffffffffffffffffffffffffffffffffffffff1614155b614a55576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475332303300000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b8173ffffffffffffffffffffffffffffffffffffffff16600260008573ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060009054906101000a900473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1614614b55576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475332303500000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b600260008373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060009054906101000a900473ffffffffffffffffffffffffffffffffffffffff16600260008573ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060006101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055506000600260008473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060006101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff160217905550600360008154809291906001900391905055507ff8d49fc529812e9a7c5c50e69c20f0dccc0db8fa95c98bc58cc9a4f1c1299eaf82604051808273ffffffffffffffffffffffffffffffffffffffff16815260200191505060405180910390a18060045414614d2457614d2381612c3e565b5b505050565b6040518060400160405280600581526020017f312e332e3000000000000000000000000000000000000000000000000000000081525081565b3073ffffffffffffffffffffffffffffffffffffffff163373ffffffffffffffffffffffffffffffffffffffff1614614e03576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475330333100000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b565b600080831415614e185760009050614e39565b6000828402905082848281614e2957fe5b0414614e3457600080fd5b809150505b92915050565b60008060008360410260208101860151925060408101860151915060ff60418201870151169350509250925092565b600080828401905083811015614e8357600080fd5b8091505092915050565b6000600180811115614e9b57fe5b836001811115614ea757fe5b1415614ec0576000808551602087018986f49050614ed0565b600080855160208701888a87f190505b95945050505050565b6000807f4a204f620c8c5ccdca3fd54d003badd85ba500436a431f0cbda4f558c93c34c860001b9050805491505090565b600081831015614f1a5781614f1c565b825b905092915050565b600082821115614f3357600080fd5b600082840390508091505092915050565b600080600073ffffffffffffffffffffffffffffffffffffffff168373ffffffffffffffffffffffffffffffffffffffff1614614f815782614f83565b325b9050600073ffffffffffffffffffffffffffffffffffffffff168473ffffffffffffffffffffffffffffffffffffffff16141561509b57614fed3a8610614fca573a614fcc565b855b614fdf888a614e6e90919063ffffffff16565b614e0590919063ffffffff16565b91508073ffffffffffffffffffffffffffffffffffffffff166108fc839081150290604051600060405180830381858888f19350505050615096576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475330313100000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b615140565b6150c0856150b2888a614e6e90919063ffffffff16565b614e0590919063ffffffff16565b91506150cd8482846158b4565b61513f576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475330313200000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b5b5095945050505050565b6000600454146151c2576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475332303000000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b8151811115615239576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475332303100000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b60018110156152b0576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475332303200000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b60006001905060005b83518110156155b65760008482815181106152d057fe5b60200260200101519050600073ffffffffffffffffffffffffffffffffffffffff168173ffffffffffffffffffffffffffffffffffffffff16141580156153445750600173ffffffffffffffffffffffffffffffffffffffff168173ffffffffffffffffffffffffffffffffffffffff1614155b801561537c57503073ffffffffffffffffffffffffffffffffffffffff168173ffffffffffffffffffffffffffffffffffffffff1614155b80156153b457508073ffffffffffffffffffffffffffffffffffffffff168373ffffffffffffffffffffffffffffffffffffffff1614155b615426576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475332303300000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b600073ffffffffffffffffffffffffffffffffffffffff16600260008373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060009054906101000a900473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1614615527576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475332303400000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b80600260008573ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060006101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055508092505080806001019150506152b9565b506001600260008373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060006101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff160217905550825160038190555081600481905550505050565b60007f6c9a6c4a39284e37ed1cf53d337577d14212a4870fb976a4366c693b939918d560001b90508181555050565b600073ffffffffffffffffffffffffffffffffffffffff1660016000600173ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060009054906101000a900473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff161461577b576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475331303000000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b6001806000600173ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060006101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff160217905550600073ffffffffffffffffffffffffffffffffffffffff168273ffffffffffffffffffffffffffffffffffffffff16146158b05761583d8260008360015a614e8d565b6158af576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260058152602001807f475330303000000000000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b5b5050565b60008063a9059cbb8484604051602401808373ffffffffffffffffffffffffffffffffffffffff168152602001828152602001925050506040516020818303038152906040529060e01b6020820180517bffffffffffffffffffffffffffffffffffffffffffffffffffffffff83818316178352505050509050602060008251602084016000896127105a03f13d6000811461595b5760208114615963576000935061596e565b81935061596e565b600051158215171593505b505050939250505056fea26469706673582212203874bcf92e1722cc7bfa0cef1a0985cf0dc3485ba0663db3747ccdf1605df53464736f6c63430007060033
    /// ```
    #[rustfmt::skip]
    #[allow(clippy::all)]
    pub static BYTECODE: alloy_sol_types::private::Bytes = alloy_sol_types::private::Bytes::from_static(
        b"`\x80`@R4\x80\x15a\0\x10W`\0\x80\xFD[P`\x01`\x04\x81\x90UPaY\xAE\x80b\0\0)`\09`\0\xF3\xFE`\x80`@R`\x046\x10a\x01\xDCW`\x005`\xE0\x1C\x80c\xAF\xFE\xD0\xE0\x11a\x01\x02W\x80c\xE1\x9A\x9D\xD9\x11a\0\x95W\x80c\xF0\x8A\x03#\x11a\0dW\x80c\xF0\x8A\x03#\x14a\x16GW\x80c\xF6\x98\xDA%\x14a\x16\x98W\x80c\xF8\xDC]\xD9\x14a\x16\xC3W\x80c\xFF\xA1\xADt\x14a\x17>Wa\x021V[\x80c\xE1\x9A\x9D\xD9\x14a\x13\x9BW\x80c\xE3\x18\xB5+\x14a\x13\xECW\x80c\xE7R5\xB8\x14a\x14}W\x80c\xE8f7\xDB\x14a\x14\xA8Wa\x021V[\x80c\xCC/\x84R\x11a\0\xD1W\x80c\xCC/\x84R\x14a\x10\xE8W\x80c\xD4\xD9\xBD\xCD\x14a\x11\xB5W\x80c\xD8\xD1\x1Fx\x14a\x11\xF0W\x80c\xE0\t\xCF\xDE\x14a\x13*Wa\x021V[\x80c\xAF\xFE\xD0\xE0\x14a\r\x94W\x80c\xB4\xFA\xBA\t\x14a\r\xBFW\x80c\xB6>\x80\r\x14a\x0E\xA7W\x80c\xC4\xCA:\x9C\x14a\x10\x17Wa\x021V[\x80cV$\xB2[\x11a\x01zW\x80cjv\x12\x02\x11a\x01IW\x80cjv\x12\x02\x14a\t\x94W\x80c}\x83)t\x14a\x0BPW\x80c\x93O:\x11\x14a\x0B\xBFW\x80c\xA0\xE6~+\x14a\r(Wa\x021V[\x80cV$\xB2[\x14a\x07\xFBW\x80cZ\xE6\xBD7\x14a\x08\xB9W\x80ca\x0BY%\x14a\t\x08W\x80ciN\x80\xC3\x14a\tYWa\x021V[\x80c/T\xBFn\x11a\x01\xB6W\x80c/T\xBFn\x14a\x04\xD3W\x80c4\x08\xE4p\x14a\x05:W\x80cF\x87!\xA7\x14a\x05eW\x80cR)\x07?\x14a\x06zWa\x021V[\x80c\rX/\x13\x14a\x02\x9EW\x80c\x12\xFBh\xE0\x14a\x02\xF9W\x80c-\x9A\xD5=\x14a\x04lWa\x021V[6a\x021W3s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x7F=\x0C\xE9\xBF\xC3\xED}hb\xDB\xB2\x8B-\xEA\x94V\x1F\xE7\x14\xA1\xB4\xD0\x19\xAA\x8A\xF3\x970\xD1\xAD|=4`@Q\x80\x82\x81R` \x01\x91PP`@Q\x80\x91\x03\x90\xA2\0[4\x80\x15a\x02=W`\0\x80\xFD[P`\0\x7Fl\x9AlJ9(N7\xED\x1C\xF5=3uw\xD1B\x12\xA4\x87\x0F\xB9v\xA46li;\x93\x99\x18\xD5`\0\x1B\x90P\x80T\x80a\x02rW`\0\x80\xF3[6`\0\x8073``\x1B6R`\0\x80`\x146\x01`\0\x80\x85Z\xF1=`\0\x80>\x80a\x02\x99W=`\0\xFD[=`\0\xF3[4\x80\x15a\x02\xAAW`\0\x80\xFD[Pa\x02\xF7`\x04\x806\x03`@\x81\x10\x15a\x02\xC1W`\0\x80\xFD[\x81\x01\x90\x80\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90\x92\x91\x90PPPa\x17\xCEV[\0[4\x80\x15a\x03\x05W`\0\x80\xFD[Pa\x04j`\x04\x806\x03`\x80\x81\x10\x15a\x03\x1CW`\0\x80\xFD[\x81\x01\x90\x80\x805\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90d\x01\0\0\0\0\x81\x11\x15a\x03CW`\0\x80\xFD[\x82\x01\x83` \x82\x01\x11\x15a\x03UW`\0\x80\xFD[\x805\x90` \x01\x91\x84`\x01\x83\x02\x84\x01\x11d\x01\0\0\0\0\x83\x11\x17\x15a\x03wW`\0\x80\xFD[\x91\x90\x80\x80`\x1F\x01` \x80\x91\x04\x02` \x01`@Q\x90\x81\x01`@R\x80\x93\x92\x91\x90\x81\x81R` \x01\x83\x83\x80\x82\x847`\0\x81\x84\x01R`\x1F\x19`\x1F\x82\x01\x16\x90P\x80\x83\x01\x92PPPPPPP\x91\x92\x91\x92\x90\x805\x90` \x01\x90d\x01\0\0\0\0\x81\x11\x15a\x03\xDAW`\0\x80\xFD[\x82\x01\x83` \x82\x01\x11\x15a\x03\xECW`\0\x80\xFD[\x805\x90` \x01\x91\x84`\x01\x83\x02\x84\x01\x11d\x01\0\0\0\0\x83\x11\x17\x15a\x04\x0EW`\0\x80\xFD[\x91\x90\x80\x80`\x1F\x01` \x80\x91\x04\x02` \x01`@Q\x90\x81\x01`@R\x80\x93\x92\x91\x90\x81\x81R` \x01\x83\x83\x80\x82\x847`\0\x81\x84\x01R`\x1F\x19`\x1F\x82\x01\x16\x90P\x80\x83\x01\x92PPPPPPP\x91\x92\x91\x92\x90\x805\x90` \x01\x90\x92\x91\x90PPPa\x1B\xBEV[\0[4\x80\x15a\x04xW`\0\x80\xFD[Pa\x04\xBB`\x04\x806\x03` \x81\x10\x15a\x04\x8FW`\0\x80\xFD[\x81\x01\x90\x80\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90PPPa$@V[`@Q\x80\x82\x15\x15\x81R` \x01\x91PP`@Q\x80\x91\x03\x90\xF3[4\x80\x15a\x04\xDFW`\0\x80\xFD[Pa\x05\"`\x04\x806\x03` \x81\x10\x15a\x04\xF6W`\0\x80\xFD[\x81\x01\x90\x80\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90PPPa%\x12V[`@Q\x80\x82\x15\x15\x81R` \x01\x91PP`@Q\x80\x91\x03\x90\xF3[4\x80\x15a\x05FW`\0\x80\xFD[Pa\x05Oa%\xE4V[`@Q\x80\x82\x81R` \x01\x91PP`@Q\x80\x91\x03\x90\xF3[4\x80\x15a\x05qW`\0\x80\xFD[Pa\x06b`\x04\x806\x03`\x80\x81\x10\x15a\x05\x88W`\0\x80\xFD[\x81\x01\x90\x80\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90d\x01\0\0\0\0\x81\x11\x15a\x05\xCFW`\0\x80\xFD[\x82\x01\x83` \x82\x01\x11\x15a\x05\xE1W`\0\x80\xFD[\x805\x90` \x01\x91\x84`\x01\x83\x02\x84\x01\x11d\x01\0\0\0\0\x83\x11\x17\x15a\x06\x03W`\0\x80\xFD[\x91\x90\x80\x80`\x1F\x01` \x80\x91\x04\x02` \x01`@Q\x90\x81\x01`@R\x80\x93\x92\x91\x90\x81\x81R` \x01\x83\x83\x80\x82\x847`\0\x81\x84\x01R`\x1F\x19`\x1F\x82\x01\x16\x90P\x80\x83\x01\x92PPPPPPP\x91\x92\x91\x92\x90\x805`\xFF\x16\x90` \x01\x90\x92\x91\x90PPPa%\xF1V[`@Q\x80\x82\x15\x15\x81R` \x01\x91PP`@Q\x80\x91\x03\x90\xF3[4\x80\x15a\x06\x86W`\0\x80\xFD[Pa\x07w`\x04\x806\x03`\x80\x81\x10\x15a\x06\x9DW`\0\x80\xFD[\x81\x01\x90\x80\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90d\x01\0\0\0\0\x81\x11\x15a\x06\xE4W`\0\x80\xFD[\x82\x01\x83` \x82\x01\x11\x15a\x06\xF6W`\0\x80\xFD[\x805\x90` \x01\x91\x84`\x01\x83\x02\x84\x01\x11d\x01\0\0\0\0\x83\x11\x17\x15a\x07\x18W`\0\x80\xFD[\x91\x90\x80\x80`\x1F\x01` \x80\x91\x04\x02` \x01`@Q\x90\x81\x01`@R\x80\x93\x92\x91\x90\x81\x81R` \x01\x83\x83\x80\x82\x847`\0\x81\x84\x01R`\x1F\x19`\x1F\x82\x01\x16\x90P\x80\x83\x01\x92PPPPPPP\x91\x92\x91\x92\x90\x805`\xFF\x16\x90` \x01\x90\x92\x91\x90PPPa'\xD7V[`@Q\x80\x83\x15\x15\x81R` \x01\x80` \x01\x82\x81\x03\x82R\x83\x81\x81Q\x81R` \x01\x91P\x80Q\x90` \x01\x90\x80\x83\x83`\0[\x83\x81\x10\x15a\x07\xBFW\x80\x82\x01Q\x81\x84\x01R` \x81\x01\x90Pa\x07\xA4V[PPPP\x90P\x90\x81\x01\x90`\x1F\x16\x80\x15a\x07\xECW\x80\x82\x03\x80Q`\x01\x83` \x03a\x01\0\n\x03\x19\x16\x81R` \x01\x91P[P\x93PPPP`@Q\x80\x91\x03\x90\xF3[4\x80\x15a\x08\x07W`\0\x80\xFD[Pa\x08>`\x04\x806\x03`@\x81\x10\x15a\x08\x1EW`\0\x80\xFD[\x81\x01\x90\x80\x805\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90\x92\x91\x90PPPa(\rV[`@Q\x80\x80` \x01\x82\x81\x03\x82R\x83\x81\x81Q\x81R` \x01\x91P\x80Q\x90` \x01\x90\x80\x83\x83`\0[\x83\x81\x10\x15a\x08~W\x80\x82\x01Q\x81\x84\x01R` \x81\x01\x90Pa\x08cV[PPPP\x90P\x90\x81\x01\x90`\x1F\x16\x80\x15a\x08\xABW\x80\x82\x03\x80Q`\x01\x83` \x03a\x01\0\n\x03\x19\x16\x81R` \x01\x91P[P\x92PPP`@Q\x80\x91\x03\x90\xF3[4\x80\x15a\x08\xC5W`\0\x80\xFD[Pa\x08\xF2`\x04\x806\x03` \x81\x10\x15a\x08\xDCW`\0\x80\xFD[\x81\x01\x90\x80\x805\x90` \x01\x90\x92\x91\x90PPPa(\x94V[`@Q\x80\x82\x81R` \x01\x91PP`@Q\x80\x91\x03\x90\xF3[4\x80\x15a\t\x14W`\0\x80\xFD[Pa\tW`\x04\x806\x03` \x81\x10\x15a\t+W`\0\x80\xFD[\x81\x01\x90\x80\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90PPPa(\xACV[\0[4\x80\x15a\teW`\0\x80\xFD[Pa\t\x92`\x04\x806\x03` \x81\x10\x15a\t|W`\0\x80\xFD[\x81\x01\x90\x80\x805\x90` \x01\x90\x92\x91\x90PPPa,>V[\0[a\x0B8`\x04\x806\x03a\x01@\x81\x10\x15a\t\xABW`\0\x80\xFD[\x81\x01\x90\x80\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90d\x01\0\0\0\0\x81\x11\x15a\t\xF2W`\0\x80\xFD[\x82\x01\x83` \x82\x01\x11\x15a\n\x04W`\0\x80\xFD[\x805\x90` \x01\x91\x84`\x01\x83\x02\x84\x01\x11d\x01\0\0\0\0\x83\x11\x17\x15a\n&W`\0\x80\xFD[\x90\x91\x92\x93\x91\x92\x93\x90\x805`\xFF\x16\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90\x92\x91\x90\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90d\x01\0\0\0\0\x81\x11\x15a\n\xB2W`\0\x80\xFD[\x82\x01\x83` \x82\x01\x11\x15a\n\xC4W`\0\x80\xFD[\x805\x90` \x01\x91\x84`\x01\x83\x02\x84\x01\x11d\x01\0\0\0\0\x83\x11\x17\x15a\n\xE6W`\0\x80\xFD[\x91\x90\x80\x80`\x1F\x01` \x80\x91\x04\x02` \x01`@Q\x90\x81\x01`@R\x80\x93\x92\x91\x90\x81\x81R` \x01\x83\x83\x80\x82\x847`\0\x81\x84\x01R`\x1F\x19`\x1F\x82\x01\x16\x90P\x80\x83\x01\x92PPPPPPP\x91\x92\x91\x92\x90PPPa-xV[`@Q\x80\x82\x15\x15\x81R` \x01\x91PP`@Q\x80\x91\x03\x90\xF3[4\x80\x15a\x0B\\W`\0\x80\xFD[Pa\x0B\xA9`\x04\x806\x03`@\x81\x10\x15a\x0BsW`\0\x80\xFD[\x81\x01\x90\x80\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90\x92\x91\x90PPPa2\xB5V[`@Q\x80\x82\x81R` \x01\x91PP`@Q\x80\x91\x03\x90\xF3[4\x80\x15a\x0B\xCBW`\0\x80\xFD[Pa\r&`\x04\x806\x03``\x81\x10\x15a\x0B\xE2W`\0\x80\xFD[\x81\x01\x90\x80\x805\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90d\x01\0\0\0\0\x81\x11\x15a\x0C\tW`\0\x80\xFD[\x82\x01\x83` \x82\x01\x11\x15a\x0C\x1BW`\0\x80\xFD[\x805\x90` \x01\x91\x84`\x01\x83\x02\x84\x01\x11d\x01\0\0\0\0\x83\x11\x17\x15a\x0C=W`\0\x80\xFD[\x91\x90\x80\x80`\x1F\x01` \x80\x91\x04\x02` \x01`@Q\x90\x81\x01`@R\x80\x93\x92\x91\x90\x81\x81R` \x01\x83\x83\x80\x82\x847`\0\x81\x84\x01R`\x1F\x19`\x1F\x82\x01\x16\x90P\x80\x83\x01\x92PPPPPPP\x91\x92\x91\x92\x90\x805\x90` \x01\x90d\x01\0\0\0\0\x81\x11\x15a\x0C\xA0W`\0\x80\xFD[\x82\x01\x83` \x82\x01\x11\x15a\x0C\xB2W`\0\x80\xFD[\x805\x90` \x01\x91\x84`\x01\x83\x02\x84\x01\x11d\x01\0\0\0\0\x83\x11\x17\x15a\x0C\xD4W`\0\x80\xFD[\x91\x90\x80\x80`\x1F\x01` \x80\x91\x04\x02` \x01`@Q\x90\x81\x01`@R\x80\x93\x92\x91\x90\x81\x81R` \x01\x83\x83\x80\x82\x847`\0\x81\x84\x01R`\x1F\x19`\x1F\x82\x01\x16\x90P\x80\x83\x01\x92PPPPPPP\x91\x92\x91\x92\x90PPPa2\xDAV[\0[4\x80\x15a\r4W`\0\x80\xFD[Pa\r=a3iV[`@Q\x80\x80` \x01\x82\x81\x03\x82R\x83\x81\x81Q\x81R` \x01\x91P\x80Q\x90` \x01\x90` \x02\x80\x83\x83`\0[\x83\x81\x10\x15a\r\x80W\x80\x82\x01Q\x81\x84\x01R` \x81\x01\x90Pa\reV[PPPP\x90P\x01\x92PPP`@Q\x80\x91\x03\x90\xF3[4\x80\x15a\r\xA0W`\0\x80\xFD[Pa\r\xA9a5\x12V[`@Q\x80\x82\x81R` \x01\x91PP`@Q\x80\x91\x03\x90\xF3[4\x80\x15a\r\xCBW`\0\x80\xFD[Pa\x0E\xA5`\x04\x806\x03`@\x81\x10\x15a\r\xE2W`\0\x80\xFD[\x81\x01\x90\x80\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90d\x01\0\0\0\0\x81\x11\x15a\x0E\x1FW`\0\x80\xFD[\x82\x01\x83` \x82\x01\x11\x15a\x0E1W`\0\x80\xFD[\x805\x90` \x01\x91\x84`\x01\x83\x02\x84\x01\x11d\x01\0\0\0\0\x83\x11\x17\x15a\x0ESW`\0\x80\xFD[\x91\x90\x80\x80`\x1F\x01` \x80\x91\x04\x02` \x01`@Q\x90\x81\x01`@R\x80\x93\x92\x91\x90\x81\x81R` \x01\x83\x83\x80\x82\x847`\0\x81\x84\x01R`\x1F\x19`\x1F\x82\x01\x16\x90P\x80\x83\x01\x92PPPPPPP\x91\x92\x91\x92\x90PPPa5\x18V[\0[4\x80\x15a\x0E\xB3W`\0\x80\xFD[Pa\x10\x15`\x04\x806\x03a\x01\0\x81\x10\x15a\x0E\xCBW`\0\x80\xFD[\x81\x01\x90\x80\x805\x90` \x01\x90d\x01\0\0\0\0\x81\x11\x15a\x0E\xE8W`\0\x80\xFD[\x82\x01\x83` \x82\x01\x11\x15a\x0E\xFAW`\0\x80\xFD[\x805\x90` \x01\x91\x84` \x83\x02\x84\x01\x11d\x01\0\0\0\0\x83\x11\x17\x15a\x0F\x1CW`\0\x80\xFD[\x90\x91\x92\x93\x91\x92\x93\x90\x805\x90` \x01\x90\x92\x91\x90\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90d\x01\0\0\0\0\x81\x11\x15a\x0FgW`\0\x80\xFD[\x82\x01\x83` \x82\x01\x11\x15a\x0FyW`\0\x80\xFD[\x805\x90` \x01\x91\x84`\x01\x83\x02\x84\x01\x11d\x01\0\0\0\0\x83\x11\x17\x15a\x0F\x9BW`\0\x80\xFD[\x90\x91\x92\x93\x91\x92\x93\x90\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90\x92\x91\x90\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90PPPa5:V[\0[4\x80\x15a\x10#W`\0\x80\xFD[Pa\x10\xD2`\x04\x806\x03`\x80\x81\x10\x15a\x10:W`\0\x80\xFD[\x81\x01\x90\x80\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90d\x01\0\0\0\0\x81\x11\x15a\x10\x81W`\0\x80\xFD[\x82\x01\x83` \x82\x01\x11\x15a\x10\x93W`\0\x80\xFD[\x805\x90` \x01\x91\x84`\x01\x83\x02\x84\x01\x11d\x01\0\0\0\0\x83\x11\x17\x15a\x10\xB5W`\0\x80\xFD[\x90\x91\x92\x93\x91\x92\x93\x90\x805`\xFF\x16\x90` \x01\x90\x92\x91\x90PPPa6\xF8V[`@Q\x80\x82\x81R` \x01\x91PP`@Q\x80\x91\x03\x90\xF3[4\x80\x15a\x10\xF4W`\0\x80\xFD[Pa\x11A`\x04\x806\x03`@\x81\x10\x15a\x11\x0BW`\0\x80\xFD[\x81\x01\x90\x80\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90\x92\x91\x90PPPa8 V[`@Q\x80\x80` \x01\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x82\x81\x03\x82R\x84\x81\x81Q\x81R` \x01\x91P\x80Q\x90` \x01\x90` \x02\x80\x83\x83`\0[\x83\x81\x10\x15a\x11\xA0W\x80\x82\x01Q\x81\x84\x01R` \x81\x01\x90Pa\x11\x85V[PPPP\x90P\x01\x93PPPP`@Q\x80\x91\x03\x90\xF3[4\x80\x15a\x11\xC1W`\0\x80\xFD[Pa\x11\xEE`\x04\x806\x03` \x81\x10\x15a\x11\xD8W`\0\x80\xFD[\x81\x01\x90\x80\x805\x90` \x01\x90\x92\x91\x90PPPa:\x12V[\0[4\x80\x15a\x11\xFCW`\0\x80\xFD[Pa\x13\x14`\x04\x806\x03a\x01@\x81\x10\x15a\x12\x14W`\0\x80\xFD[\x81\x01\x90\x80\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90d\x01\0\0\0\0\x81\x11\x15a\x12[W`\0\x80\xFD[\x82\x01\x83` \x82\x01\x11\x15a\x12mW`\0\x80\xFD[\x805\x90` \x01\x91\x84`\x01\x83\x02\x84\x01\x11d\x01\0\0\0\0\x83\x11\x17\x15a\x12\x8FW`\0\x80\xFD[\x90\x91\x92\x93\x91\x92\x93\x90\x805`\xFF\x16\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90\x92\x91\x90\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90\x92\x91\x90PPPa;\xB1V[`@Q\x80\x82\x81R` \x01\x91PP`@Q\x80\x91\x03\x90\xF3[4\x80\x15a\x136W`\0\x80\xFD[Pa\x13\x99`\x04\x806\x03`@\x81\x10\x15a\x13MW`\0\x80\xFD[\x81\x01\x90\x80\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90PPPa;\xDEV[\0[4\x80\x15a\x13\xA7W`\0\x80\xFD[Pa\x13\xEA`\x04\x806\x03` \x81\x10\x15a\x13\xBEW`\0\x80\xFD[\x81\x01\x90\x80\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90PPPa?oV[\0[4\x80\x15a\x13\xF8W`\0\x80\xFD[Pa\x14{`\x04\x806\x03``\x81\x10\x15a\x14\x0FW`\0\x80\xFD[\x81\x01\x90\x80\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90PPPa?\xF3V[\0[4\x80\x15a\x14\x89W`\0\x80\xFD[Pa\x14\x92aFeV[`@Q\x80\x82\x81R` \x01\x91PP`@Q\x80\x91\x03\x90\xF3[4\x80\x15a\x14\xB4W`\0\x80\xFD[Pa\x15\xCC`\x04\x806\x03a\x01@\x81\x10\x15a\x14\xCCW`\0\x80\xFD[\x81\x01\x90\x80\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90d\x01\0\0\0\0\x81\x11\x15a\x15\x13W`\0\x80\xFD[\x82\x01\x83` \x82\x01\x11\x15a\x15%W`\0\x80\xFD[\x805\x90` \x01\x91\x84`\x01\x83\x02\x84\x01\x11d\x01\0\0\0\0\x83\x11\x17\x15a\x15GW`\0\x80\xFD[\x90\x91\x92\x93\x91\x92\x93\x90\x805`\xFF\x16\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90\x92\x91\x90\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90\x92\x91\x90PPPaFoV[`@Q\x80\x80` \x01\x82\x81\x03\x82R\x83\x81\x81Q\x81R` \x01\x91P\x80Q\x90` \x01\x90\x80\x83\x83`\0[\x83\x81\x10\x15a\x16\x0CW\x80\x82\x01Q\x81\x84\x01R` \x81\x01\x90Pa\x15\xF1V[PPPP\x90P\x90\x81\x01\x90`\x1F\x16\x80\x15a\x169W\x80\x82\x03\x80Q`\x01\x83` \x03a\x01\0\n\x03\x19\x16\x81R` \x01\x91P[P\x92PPP`@Q\x80\x91\x03\x90\xF3[4\x80\x15a\x16SW`\0\x80\xFD[Pa\x16\x96`\x04\x806\x03` \x81\x10\x15a\x16jW`\0\x80\xFD[\x81\x01\x90\x80\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90PPPaH\x17V[\0[4\x80\x15a\x16\xA4W`\0\x80\xFD[Pa\x16\xADaHxV[`@Q\x80\x82\x81R` \x01\x91PP`@Q\x80\x91\x03\x90\xF3[4\x80\x15a\x16\xCFW`\0\x80\xFD[Pa\x17<`\x04\x806\x03``\x81\x10\x15a\x16\xE6W`\0\x80\xFD[\x81\x01\x90\x80\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90\x805s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90` \x01\x90\x92\x91\x90\x805\x90` \x01\x90\x92\x91\x90PPPaH\xF6V[\0[4\x80\x15a\x17JW`\0\x80\xFD[Pa\x17SaM)V[`@Q\x80\x80` \x01\x82\x81\x03\x82R\x83\x81\x81Q\x81R` \x01\x91P\x80Q\x90` \x01\x90\x80\x83\x83`\0[\x83\x81\x10\x15a\x17\x93W\x80\x82\x01Q\x81\x84\x01R` \x81\x01\x90Pa\x17xV[PPPP\x90P\x90\x81\x01\x90`\x1F\x16\x80\x15a\x17\xC0W\x80\x82\x03\x80Q`\x01\x83` \x03a\x01\0\n\x03\x19\x16\x81R` \x01\x91P[P\x92PPP`@Q\x80\x91\x03\x90\xF3[a\x17\xD6aMbV[`\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15\x80\x15a\x18@WP`\x01s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15[\x80\x15a\x18xWP0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15[a\x18\xEAW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS203\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[`\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16`\x02`\0\x84s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0\x90T\x90a\x01\0\n\x90\x04s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14a\x19\xEBW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS204\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[`\x02`\0`\x01s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0\x90T\x90a\x01\0\n\x90\x04s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16`\x02`\0\x84s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0a\x01\0\n\x81T\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x02\x19\x16\x90\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x02\x17\x90UP\x81`\x02`\0`\x01s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0a\x01\0\n\x81T\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x02\x19\x16\x90\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x02\x17\x90UP`\x03`\0\x81T\x80\x92\x91\x90`\x01\x01\x91\x90PUP\x7F\x94e\xFA\x0C\x96,\xC7iX\xE67:\x993&@\x0C\x1C\x94\xF8\xBE/\xE3\xA9R\xAD\xFA\x7F`\xB2\xEA&\x82`@Q\x80\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x91PP`@Q\x80\x91\x03\x90\xA1\x80`\x04T\x14a\x1B\xBAWa\x1B\xB9\x81a,>V[[PPV[a\x1B\xD2`A\x82aN\x05\x90\x91\x90c\xFF\xFF\xFF\xFF\x16V[\x82Q\x10\x15a\x1CHW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS020\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[`\0\x80\x80`\0\x80`\0[\x86\x81\x10\x15a$4Wa\x1Cd\x88\x82aN?V[\x80\x94P\x81\x95P\x82\x96PPPP`\0\x84`\xFF\x16\x14\x15a mW\x82`\0\x1C\x94Pa\x1C\x96`A\x88aN\x05\x90\x91\x90c\xFF\xFF\xFF\xFF\x16V[\x82`\0\x1C\x10\x15a\x1D\x0EW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS021\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[\x87Qa\x1D'` \x84`\0\x1CaNn\x90\x91\x90c\xFF\xFF\xFF\xFF\x16V[\x11\x15a\x1D\x9BW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS022\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[`\0` \x83\x8A\x01\x01Q\x90P\x88Qa\x1D\xD1\x82a\x1D\xC3` \x87`\0\x1CaNn\x90\x91\x90c\xFF\xFF\xFF\xFF\x16V[aNn\x90\x91\x90c\xFF\xFF\xFF\xFF\x16V[\x11\x15a\x1EEW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS023\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[``` \x84\x8B\x01\x01\x90Pc \xC1;\x0B`\xE0\x1B{\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x19\x16\x87s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16c \xC1;\x0B\x8D\x84`@Q\x83c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01\x80\x80` \x01\x80` \x01\x83\x81\x03\x83R\x85\x81\x81Q\x81R` \x01\x91P\x80Q\x90` \x01\x90\x80\x83\x83`\0[\x83\x81\x10\x15a\x1E\xE7W\x80\x82\x01Q\x81\x84\x01R` \x81\x01\x90Pa\x1E\xCCV[PPPP\x90P\x90\x81\x01\x90`\x1F\x16\x80\x15a\x1F\x14W\x80\x82\x03\x80Q`\x01\x83` \x03a\x01\0\n\x03\x19\x16\x81R` \x01\x91P[P\x83\x81\x03\x82R\x84\x81\x81Q\x81R` \x01\x91P\x80Q\x90` \x01\x90\x80\x83\x83`\0[\x83\x81\x10\x15a\x1FMW\x80\x82\x01Q\x81\x84\x01R` \x81\x01\x90Pa\x1F2V[PPPP\x90P\x90\x81\x01\x90`\x1F\x16\x80\x15a\x1FzW\x80\x82\x03\x80Q`\x01\x83` \x03a\x01\0\n\x03\x19\x16\x81R` \x01\x91P[P\x94PPPPP` `@Q\x80\x83\x03\x81\x86\x80;\x15\x80\x15a\x1F\x99W`\0\x80\xFD[PZ\xFA\x15\x80\x15a\x1F\xADW=`\0\x80>=`\0\xFD[PPPP`@Q=` \x81\x10\x15a\x1F\xC3W`\0\x80\xFD[\x81\x01\x90\x80\x80Q\x90` \x01\x90\x92\x91\x90PPP{\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x19\x16\x14a fW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS024\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[PPa\"\xB2V[`\x01\x84`\xFF\x16\x14\x15a!\x81W\x82`\0\x1C\x94P\x84s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x163s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x80a!\nWP`\0`\x08`\0\x87s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0\x8C\x81R` \x01\x90\x81R` \x01`\0 T\x14\x15[a!|W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS025\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[a\"\xB1V[`\x1E\x84`\xFF\x16\x11\x15a\"IW`\x01\x8A`@Q` \x01\x80\x80\x7F\x19Ethereum Signed Message:\n32\0\0\0\0\x81RP`\x1C\x01\x82\x81R` \x01\x91PP`@Q` \x81\x83\x03\x03\x81R\x90`@R\x80Q\x90` \x01 `\x04\x86\x03\x85\x85`@Q`\0\x81R` \x01`@R`@Q\x80\x85\x81R` \x01\x84`\xFF\x16\x81R` \x01\x83\x81R` \x01\x82\x81R` \x01\x94PPPPP` `@Q` \x81\x03\x90\x80\x84\x03\x90\x85Z\xFA\x15\x80\x15a\"8W=`\0\x80>=`\0\xFD[PPP` `@Q\x03Q\x94Pa\"\xB0V[`\x01\x8A\x85\x85\x85`@Q`\0\x81R` \x01`@R`@Q\x80\x85\x81R` \x01\x84`\xFF\x16\x81R` \x01\x83\x81R` \x01\x82\x81R` \x01\x94PPPPP` `@Q` \x81\x03\x90\x80\x84\x03\x90\x85Z\xFA\x15\x80\x15a\"\xA3W=`\0\x80>=`\0\xFD[PPP` `@Q\x03Q\x94P[[[\x85s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x85s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x11\x80\x15a#yWP`\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16`\x02`\0\x87s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0\x90T\x90a\x01\0\n\x90\x04s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15[\x80\x15a#\xB2WP`\x01s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x85s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15[a$$W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS026\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[\x84\x95P\x80\x80`\x01\x01\x91PPa\x1CRV[PPPPPPPPPPV[`\0\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16`\x01s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15\x80\x15a%\x0BWP`\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16`\x01`\0\x84s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0\x90T\x90a\x01\0\n\x90\x04s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15[\x90P\x91\x90PV[`\0`\x01s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15\x80\x15a%\xDDWP`\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16`\x02`\0\x84s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0\x90T\x90a\x01\0\n\x90\x04s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15[\x90P\x91\x90PV[`\0\x80F\x90P\x80\x91PP\x90V[`\0`\x01s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x163s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15\x80\x15a&\xBCWP`\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16`\x01`\x003s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0\x90T\x90a\x01\0\n\x90\x04s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15[a'.W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS104\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[a';\x85\x85\x85\x85ZaN\x8DV[\x90P\x80\x15a'\x8BW3s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x7Fh\x95\xC16d\xAAOg(\x8B%\xD7\xA2\x1Dz\xAA4\x91n5_\xB9\xB6\xFA\xE0\xA19\xA9\x08[\xEC\xB8`@Q`@Q\x80\x91\x03\x90\xA2a'\xCFV[3s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x7F\xAC\xD2\xC8p(\x04\x12\x8F\xDB\r\xB2\xBBI\xF6\xD1'\xDD\x01\x81\xC1?\xD4]\xBF\xE1m\xE0\x93\x0E+\xD3u`@Q`@Q\x80\x91\x03\x90\xA2[\x94\x93PPPPV[`\0``a'\xE7\x86\x86\x86\x86a%\xF1V[\x91P`@Q` =\x01\x81\x01`@R=\x81R=`\0` \x83\x01>\x80\x91PP\x94P\x94\x92PPPV[```\0` \x83\x02g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x80\x15a(+W`\0\x80\xFD[P`@Q\x90\x80\x82R\x80`\x1F\x01`\x1F\x19\x16` \x01\x82\x01`@R\x80\x15a(^W\x81` \x01`\x01\x82\x02\x806\x837\x80\x82\x01\x91PP\x90P[P\x90P`\0[\x83\x81\x10\x15a(\x89W\x80\x85\x01T\x80` \x83\x02` \x85\x01\x01RP\x80\x80`\x01\x01\x91PPa(dV[P\x80\x91PP\x92\x91PPV[`\x07` R\x80`\0R`@`\0 `\0\x91P\x90PT\x81V[a(\xB4aMbV[`\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15\x80\x15a)\x1EWP`\x01s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15[a)\x90W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS101\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[`\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16`\x01`\0\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0\x90T\x90a\x01\0\n\x90\x04s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14a*\x91W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS102\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[`\x01`\0`\x01s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0\x90T\x90a\x01\0\n\x90\x04s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16`\x01`\0\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0a\x01\0\n\x81T\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x02\x19\x16\x90\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x02\x17\x90UP\x80`\x01`\0`\x01s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0a\x01\0\n\x81T\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x02\x19\x16\x90\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x02\x17\x90UP\x7F\xEC\xDF:>\xFF\xEAW\x83\xA3\xC4\xC2\x14\x0Eguwfd(\xD4N\xD9\xD4t\xA0\xB3\xA4\xC9\x94?\x84@\x81`@Q\x80\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x91PP`@Q\x80\x91\x03\x90\xA1PV[a,FaMbV[`\x03T\x81\x11\x15a,\xBEW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS201\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[`\x01\x81\x10\x15a-5W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS202\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[\x80`\x04\x81\x90UP\x7Fa\x0F\x7F\xF2\xB3\x04\xAE\x89\x03\xC3\xDEt\xC6\x0Cj\xB1\xF7\xD6\"k?R\xC5\x16\x19\x05\xBBZ\xD4\x03\x9C\x93`\x04T`@Q\x80\x82\x81R` \x01\x91PP`@Q\x80\x91\x03\x90\xA1PV[`\0\x80`\0a-\x92\x8E\x8E\x8E\x8E\x8E\x8E\x8E\x8E\x8E\x8E`\x05TaFoV[\x90P`\x05`\0\x81T\x80\x92\x91\x90`\x01\x01\x91\x90PUP\x80\x80Q\x90` \x01 \x91Pa-\xBB\x82\x82\x86a2\xDAV[P`\0a-\xC6aN\xD9V[\x90P`\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14a/\xACW\x80s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16cu\xF0\xBBR\x8F\x8F\x8F\x8F\x8F\x8F\x8F\x8F\x8F\x8F\x8F3`@Q\x8Dc\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01\x80\x8Ds\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x8C\x81R` \x01\x80` \x01\x8A`\x01\x81\x11\x15a.iW\xFE[\x81R` \x01\x89\x81R` \x01\x88\x81R` \x01\x87\x81R` \x01\x86s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x85s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x80` \x01\x84s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x83\x81\x03\x83R\x8D\x8D\x82\x81\x81R` \x01\x92P\x80\x82\x847`\0\x81\x84\x01R`\x1F\x19`\x1F\x82\x01\x16\x90P\x80\x83\x01\x92PPP\x83\x81\x03\x82R\x85\x81\x81Q\x81R` \x01\x91P\x80Q\x90` \x01\x90\x80\x83\x83`\0[\x83\x81\x10\x15a/;W\x80\x82\x01Q\x81\x84\x01R` \x81\x01\x90Pa/ V[PPPP\x90P\x90\x81\x01\x90`\x1F\x16\x80\x15a/hW\x80\x82\x03\x80Q`\x01\x83` \x03a\x01\0\n\x03\x19\x16\x81R` \x01\x91P[P\x9EPPPPPPPPPPPPPPP`\0`@Q\x80\x83\x03\x81`\0\x87\x80;\x15\x80\x15a/\x93W`\0\x80\xFD[PZ\xF1\x15\x80\x15a/\xA7W=`\0\x80>=`\0\xFD[PPPP[a\x01\xF4a/\xD3a\t\xC4\x8B\x01`?`@\x8D\x02\x81a/\xC4W\xFE[\x04aO\n\x90\x91\x90c\xFF\xFF\xFF\xFF\x16V[\x01Z\x10\x15a0IW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS010\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[`\0Z\x90Pa0\xB2\x8F\x8F\x8F\x8F\x80\x80`\x1F\x01` \x80\x91\x04\x02` \x01`@Q\x90\x81\x01`@R\x80\x93\x92\x91\x90\x81\x81R` \x01\x83\x83\x80\x82\x847`\0\x81\x84\x01R`\x1F\x19`\x1F\x82\x01\x16\x90P\x80\x83\x01\x92PPPPPPP\x8E`\0\x8D\x14a0\xA7W\x8Ea0\xADV[a\t\xC4Z\x03[aN\x8DV[\x93Pa0\xC7Z\x82aO$\x90\x91\x90c\xFF\xFF\xFF\xFF\x16V[\x90P\x83\x80a0\xD6WP`\0\x8A\x14\x15[\x80a0\xE2WP`\0\x88\x14\x15[a1TW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS013\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[`\0\x80\x89\x11\x15a1nWa1k\x82\x8B\x8B\x8B\x8BaODV[\x90P[\x84\x15a1\xB8W\x7FD.q_bcF\xE8\xC5C\x81\0-\xA6\x14\xF6+\xEE\x8D'8e5\xB2R\x1E\xC8T\x08\x98Un\x84\x82`@Q\x80\x83\x81R` \x01\x82\x81R` \x01\x92PPP`@Q\x80\x91\x03\x90\xA1a1\xF8V[\x7F#B\x8B\x18\xAC\xFB>\xA6K\x08\xDC\x0C\x1D)n\xA9\xC0\x97\x02\xC0\x90\x83\xCARr\xE6M\x11[h}#\x84\x82`@Q\x80\x83\x81R` \x01\x82\x81R` \x01\x92PPP`@Q\x80\x91\x03\x90\xA1[PP`\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14a2\xA4W\x80s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16c\x93'\x13h\x83\x85`@Q\x83c\xFF\xFF\xFF\xFF\x16`\xE0\x1B\x81R`\x04\x01\x80\x83\x81R` \x01\x82\x15\x15\x81R` \x01\x92PPP`\0`@Q\x80\x83\x03\x81`\0\x87\x80;\x15\x80\x15a2\x8BW`\0\x80\xFD[PZ\xF1\x15\x80\x15a2\x9FW=`\0\x80>=`\0\xFD[PPPP[PP\x9B\x9APPPPPPPPPPPV[`\x08` R\x81`\0R`@`\0 ` R\x80`\0R`@`\0 `\0\x91P\x91PPT\x81V[`\0`\x04T\x90P`\0\x81\x11a3WW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS001\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[a3c\x84\x84\x84\x84a\x1B\xBEV[PPPPV[```\0`\x03Tg\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x80\x15a3\x86W`\0\x80\xFD[P`@Q\x90\x80\x82R\x80` \x02` \x01\x82\x01`@R\x80\x15a3\xB5W\x81` \x01` \x82\x02\x806\x837\x80\x82\x01\x91PP\x90P[P\x90P`\0\x80`\x02`\0`\x01s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0\x90T\x90a\x01\0\n\x90\x04s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90P[`\x01s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14a5\tW\x80\x83\x83\x81Q\x81\x10a4`W\xFE[` \x02` \x01\x01\x90s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81RPP`\x02`\0\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0\x90T\x90a\x01\0\n\x90\x04s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90P\x81\x80`\x01\x01\x92PPa4\x1FV[\x82\x93PPPP\x90V[`\x05T\x81V[`\0\x80\x82Q` \x84\x01\x85Z\xF4\x80`\0R=` R=`\0`@>`@=\x01`\0\xFD[a5\x85\x8A\x8A\x80\x80` \x02` \x01`@Q\x90\x81\x01`@R\x80\x93\x92\x91\x90\x81\x81R` \x01\x83\x83` \x02\x80\x82\x847`\0\x81\x84\x01R`\x1F\x19`\x1F\x82\x01\x16\x90P\x80\x83\x01\x92PPPPPPP\x89aQJV[`\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x84s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14a5\xC3Wa5\xC2\x84aVJV[[a6\x11\x87\x87\x87\x80\x80`\x1F\x01` \x80\x91\x04\x02` \x01`@Q\x90\x81\x01`@R\x80\x93\x92\x91\x90\x81\x81R` \x01\x83\x83\x80\x82\x847`\0\x81\x84\x01R`\x1F\x19`\x1F\x82\x01\x16\x90P\x80\x83\x01\x92PPPPPPPaVyV[`\0\x82\x11\x15a6+Wa6)\x82`\0`\x01\x86\x85aODV[P[3s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x7F\x14\x1D\xF8h\xA63\x1A\xF5(\xE3\x8C\x83\xB7\xAA\x03\xED\xC1\x9B\xE6n7\xAEg\xF9([\xF4\xF8\xE3\xC6\xA1\xA8\x8B\x8B\x8B\x8B\x89`@Q\x80\x80` \x01\x85\x81R` \x01\x84s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x82\x81\x03\x82R\x87\x87\x82\x81\x81R` \x01\x92P` \x02\x80\x82\x847`\0\x81\x84\x01R`\x1F\x19`\x1F\x82\x01\x16\x90P\x80\x83\x01\x92PPP\x96PPPPPPP`@Q\x80\x91\x03\x90\xA2PPPPPPPPPPV[`\0\x80Z\x90Pa7O\x87\x87\x87\x87\x80\x80`\x1F\x01` \x80\x91\x04\x02` \x01`@Q\x90\x81\x01`@R\x80\x93\x92\x91\x90\x81\x81R` \x01\x83\x83\x80\x82\x847`\0\x81\x84\x01R`\x1F\x19`\x1F\x82\x01\x16\x90P\x80\x83\x01\x92PPPPPPP\x86ZaN\x8DV[a7XW`\0\x80\xFD[`\0Z\x82\x03\x90P\x80`@Q` \x01\x80\x82\x81R` \x01\x91PP`@Q` \x81\x83\x03\x03\x81R\x90`@R`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R\x83\x81\x81Q\x81R` \x01\x91P\x80Q\x90` \x01\x90\x80\x83\x83`\0[\x83\x81\x10\x15a7\xE5W\x80\x82\x01Q\x81\x84\x01R` \x81\x01\x90Pa7\xCAV[PPPP\x90P\x90\x81\x01\x90`\x1F\x16\x80\x15a8\x12W\x80\x82\x03\x80Q`\x01\x83` \x03a\x01\0\n\x03\x19\x16\x81R` \x01\x91P[P\x92PPP`@Q\x80\x91\x03\x90\xFD[```\0\x82g\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x81\x11\x80\x15a8;W`\0\x80\xFD[P`@Q\x90\x80\x82R\x80` \x02` \x01\x82\x01`@R\x80\x15a8jW\x81` \x01` \x82\x02\x806\x837\x80\x82\x01\x91PP\x90P[P\x91P`\0\x80`\x01`\0\x87s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0\x90T\x90a\x01\0\n\x90\x04s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90P[`\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15\x80\x15a9=WP`\x01s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15[\x80\x15a9HWP\x84\x82\x10[\x15a:\x03W\x80\x84\x83\x81Q\x81\x10a9ZW\xFE[` \x02` \x01\x01\x90s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81RPP`\x01`\0\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0\x90T\x90a\x01\0\n\x90\x04s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x90P\x81\x80`\x01\x01\x92PPa8\xD3V[\x80\x92P\x81\x84RPP\x92P\x92\x90PV[`\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16`\x02`\x003s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0\x90T\x90a\x01\0\n\x90\x04s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15a;\x14W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS030\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[`\x01`\x08`\x003s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0\x83\x81R` \x01\x90\x81R` \x01`\0 \x81\x90UP3s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81\x7F\xF2\xA0\xEB\x15dr\xD1D\x02U\xB0\xD7\xC1\xE1\x9C\xC0q\x15\xD1\x05\x1F\xE6\x05\xB0\xDC\xE6\x9A\xCF\xEC\x88M\x9C`@Q`@Q\x80\x91\x03\x90\xA3PV[`\0a;\xC6\x8C\x8C\x8C\x8C\x8C\x8C\x8C\x8C\x8C\x8C\x8CaFoV[\x80Q\x90` \x01 \x90P\x9B\x9APPPPPPPPPPPV[a;\xE6aMbV[`\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15\x80\x15a<PWP`\x01s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15[a<\xC2W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS101\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[\x80s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16`\x01`\0\x84s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0\x90T\x90a\x01\0\n\x90\x04s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14a=\xC2W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS103\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[`\x01`\0\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0\x90T\x90a\x01\0\n\x90\x04s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16`\x01`\0\x84s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0a\x01\0\n\x81T\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x02\x19\x16\x90\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x02\x17\x90UP`\0`\x01`\0\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0a\x01\0\n\x81T\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x02\x19\x16\x90\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x02\x17\x90UP\x7F\xAA\xB4\xFA+F?X\x1B+2\xCB;~;pK\x9C\xE3|\xC2\t\xB5\xFBMw\xE5\x93\xAC\xE4\x05Bv\x81`@Q\x80\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x91PP`@Q\x80\x91\x03\x90\xA1PPV[a?waMbV[`\0\x7FJ Ob\x0C\x8C\\\xCD\xCA?\xD5M\0;\xAD\xD8[\xA5\0CjC\x1F\x0C\xBD\xA4\xF5X\xC9<4\xC8`\0\x1B\x90P\x81\x81U\x7F\x11Q\x11i\x14Q[\xC0\x89\x1F\xF9\x04zl\xB3,\xF9\x02To\x83\x06d\x99\xBC\xF8\xBA3\xD25?\xA2\x82`@Q\x80\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x91PP`@Q\x80\x91\x03\x90\xA1PPV[a?\xFBaMbV[`\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15\x80\x15a@eWP`\x01s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15[\x80\x15a@\x9DWP0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15[aA\x0FW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS203\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[`\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16`\x02`\0\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0\x90T\x90a\x01\0\n\x90\x04s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14aB\x10W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS204\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[`\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15\x80\x15aBzWP`\x01s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15[aB\xECW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS203\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16`\x02`\0\x85s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0\x90T\x90a\x01\0\n\x90\x04s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14aC\xECW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS205\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[`\x02`\0\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0\x90T\x90a\x01\0\n\x90\x04s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16`\x02`\0\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0a\x01\0\n\x81T\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x02\x19\x16\x90\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x02\x17\x90UP\x80`\x02`\0\x85s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0a\x01\0\n\x81T\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x02\x19\x16\x90\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x02\x17\x90UP`\0`\x02`\0\x84s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0a\x01\0\n\x81T\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x02\x19\x16\x90\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x02\x17\x90UP\x7F\xF8\xD4\x9F\xC5)\x81.\x9A|\\P\xE6\x9C \xF0\xDC\xCC\r\xB8\xFA\x95\xC9\x8B\xC5\x8C\xC9\xA4\xF1\xC1)\x9E\xAF\x82`@Q\x80\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x91PP`@Q\x80\x91\x03\x90\xA1\x7F\x94e\xFA\x0C\x96,\xC7iX\xE67:\x993&@\x0C\x1C\x94\xF8\xBE/\xE3\xA9R\xAD\xFA\x7F`\xB2\xEA&\x81`@Q\x80\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x91PP`@Q\x80\x91\x03\x90\xA1PPPV[`\0`\x04T\x90P\x90V[```\0\x7F\xBB\x83\x10\xD4\x866\x8D\xB6\xBDo\x84\x94\x02\xFD\xD7:\xD5=1kZK&D\xADn\xFE\x0F\x94\x12\x86\xD8`\0\x1B\x8D\x8D\x8D\x8D`@Q\x80\x83\x83\x80\x82\x847\x80\x83\x01\x92PPP\x92PPP`@Q\x80\x91\x03\x90 \x8C\x8C\x8C\x8C\x8C\x8C\x8C`@Q` \x01\x80\x8C\x81R` \x01\x8Bs\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x8A\x81R` \x01\x89\x81R` \x01\x88`\x01\x81\x11\x15aG\0W\xFE[\x81R` \x01\x87\x81R` \x01\x86\x81R` \x01\x85\x81R` \x01\x84s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x82\x81R` \x01\x9BPPPPPPPPPPPP`@Q` \x81\x83\x03\x03\x81R\x90`@R\x80Q\x90` \x01 \x90P`\x19`\xF8\x1B`\x01`\xF8\x1BaG\x8CaHxV[\x83`@Q` \x01\x80\x85~\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x19\x16\x81R`\x01\x01\x84~\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x19\x16\x81R`\x01\x01\x83\x81R` \x01\x82\x81R` \x01\x94PPPPP`@Q` \x81\x83\x03\x03\x81R\x90`@R\x91PP\x9B\x9APPPPPPPPPPPV[aH\x1FaMbV[aH(\x81aVJV[\x7FZ\xC6\xC4l\x93\xC8\xD0\xE57\x14\xBA;S\xDB>|\x04m\xA9\x941=~\xD0\xD1\x92\x02\x8B\xC7\xC2(\xB0\x81`@Q\x80\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x91PP`@Q\x80\x91\x03\x90\xA1PV[`\0\x7FG\xE7\x954\xA2E\x95.\x8B\x16\x89:3k\x85\xA3\xD9\xEA\x9F\xA8\xC5s\xF3\xD8\x03\xAF\xB9*yF\x92\x18`\0\x1BaH\xA6a%\xE4V[0`@Q` \x01\x80\x84\x81R` \x01\x83\x81R` \x01\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x93PPPP`@Q` \x81\x83\x03\x03\x81R\x90`@R\x80Q\x90` \x01 \x90P\x90V[aH\xFEaMbV[\x80`\x01`\x03T\x03\x10\x15aIyW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS201\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[`\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15\x80\x15aI\xE3WP`\x01s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15[aJUW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS203\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16`\x02`\0\x85s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0\x90T\x90a\x01\0\n\x90\x04s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14aKUW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS205\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[`\x02`\0\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0\x90T\x90a\x01\0\n\x90\x04s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16`\x02`\0\x85s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0a\x01\0\n\x81T\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x02\x19\x16\x90\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x02\x17\x90UP`\0`\x02`\0\x84s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0a\x01\0\n\x81T\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x02\x19\x16\x90\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x02\x17\x90UP`\x03`\0\x81T\x80\x92\x91\x90`\x01\x90\x03\x91\x90PUP\x7F\xF8\xD4\x9F\xC5)\x81.\x9A|\\P\xE6\x9C \xF0\xDC\xCC\r\xB8\xFA\x95\xC9\x8B\xC5\x8C\xC9\xA4\xF1\xC1)\x9E\xAF\x82`@Q\x80\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x91PP`@Q\x80\x91\x03\x90\xA1\x80`\x04T\x14aM$WaM#\x81a,>V[[PPPV[`@Q\x80`@\x01`@R\x80`\x05\x81R` \x01\x7F1.3.0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP\x81V[0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x163s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14aN\x03W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS031\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[V[`\0\x80\x83\x14\x15aN\x18W`\0\x90PaN9V[`\0\x82\x84\x02\x90P\x82\x84\x82\x81aN)W\xFE[\x04\x14aN4W`\0\x80\xFD[\x80\x91PP[\x92\x91PPV[`\0\x80`\0\x83`A\x02` \x81\x01\x86\x01Q\x92P`@\x81\x01\x86\x01Q\x91P`\xFF`A\x82\x01\x87\x01Q\x16\x93PP\x92P\x92P\x92V[`\0\x80\x82\x84\x01\x90P\x83\x81\x10\x15aN\x83W`\0\x80\xFD[\x80\x91PP\x92\x91PPV[`\0`\x01\x80\x81\x11\x15aN\x9BW\xFE[\x83`\x01\x81\x11\x15aN\xA7W\xFE[\x14\x15aN\xC0W`\0\x80\x85Q` \x87\x01\x89\x86\xF4\x90PaN\xD0V[`\0\x80\x85Q` \x87\x01\x88\x8A\x87\xF1\x90P[\x95\x94PPPPPV[`\0\x80\x7FJ Ob\x0C\x8C\\\xCD\xCA?\xD5M\0;\xAD\xD8[\xA5\0CjC\x1F\x0C\xBD\xA4\xF5X\xC9<4\xC8`\0\x1B\x90P\x80T\x91PP\x90V[`\0\x81\x83\x10\x15aO\x1AW\x81aO\x1CV[\x82[\x90P\x92\x91PPV[`\0\x82\x82\x11\x15aO3W`\0\x80\xFD[`\0\x82\x84\x03\x90P\x80\x91PP\x92\x91PPV[`\0\x80`\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14aO\x81W\x82aO\x83V[2[\x90P`\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x84s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15aP\x9BWaO\xED:\x86\x10aO\xCAW:aO\xCCV[\x85[aO\xDF\x88\x8AaNn\x90\x91\x90c\xFF\xFF\xFF\xFF\x16V[aN\x05\x90\x91\x90c\xFF\xFF\xFF\xFF\x16V[\x91P\x80s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16a\x08\xFC\x83\x90\x81\x15\x02\x90`@Q`\0`@Q\x80\x83\x03\x81\x85\x88\x88\xF1\x93PPPPaP\x96W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS011\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[aQ@V[aP\xC0\x85aP\xB2\x88\x8AaNn\x90\x91\x90c\xFF\xFF\xFF\xFF\x16V[aN\x05\x90\x91\x90c\xFF\xFF\xFF\xFF\x16V[\x91PaP\xCD\x84\x82\x84aX\xB4V[aQ?W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS012\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[[P\x95\x94PPPPPV[`\0`\x04T\x14aQ\xC2W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS200\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[\x81Q\x81\x11\x15aR9W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS201\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[`\x01\x81\x10\x15aR\xB0W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS202\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[`\0`\x01\x90P`\0[\x83Q\x81\x10\x15aU\xB6W`\0\x84\x82\x81Q\x81\x10aR\xD0W\xFE[` \x02` \x01\x01Q\x90P`\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15\x80\x15aSDWP`\x01s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15[\x80\x15aS|WP0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15[\x80\x15aS\xB4WP\x80s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14\x15[aT&W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS203\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[`\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16`\x02`\0\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0\x90T\x90a\x01\0\n\x90\x04s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14aU'W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS204\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[\x80`\x02`\0\x85s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0a\x01\0\n\x81T\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x02\x19\x16\x90\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x02\x17\x90UP\x80\x92PP\x80\x80`\x01\x01\x91PPaR\xB9V[P`\x01`\x02`\0\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0a\x01\0\n\x81T\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x02\x19\x16\x90\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x02\x17\x90UP\x82Q`\x03\x81\x90UP\x81`\x04\x81\x90UPPPPV[`\0\x7Fl\x9AlJ9(N7\xED\x1C\xF5=3uw\xD1B\x12\xA4\x87\x0F\xB9v\xA46li;\x93\x99\x18\xD5`\0\x1B\x90P\x81\x81UPPV[`\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16`\x01`\0`\x01s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0\x90T\x90a\x01\0\n\x90\x04s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14aW{W`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS100\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[`\x01\x80`\0`\x01s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x90\x81R` \x01`\0 `\0a\x01\0\n\x81T\x81s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x02\x19\x16\x90\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x02\x17\x90UP`\0s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x82s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x14aX\xB0WaX=\x82`\0\x83`\x01ZaN\x8DV[aX\xAFW`@Q\x7F\x08\xC3y\xA0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81R`\x04\x01\x80\x80` \x01\x82\x81\x03\x82R`\x05\x81R` \x01\x80\x7FGS000\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x81RP` \x01\x91PP`@Q\x80\x91\x03\x90\xFD[[PPV[`\0\x80c\xA9\x05\x9C\xBB\x84\x84`@Q`$\x01\x80\x83s\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x16\x81R` \x01\x82\x81R` \x01\x92PPP`@Q` \x81\x83\x03\x03\x81R\x90`@R\x90`\xE0\x1B` \x82\x01\x80Q{\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x83\x81\x83\x16\x17\x83RPPPP\x90P` `\0\x82Q` \x84\x01`\0\x89a'\x10Z\x03\xF1=`\0\x81\x14aY[W` \x81\x14aYcW`\0\x93PaYnV[\x81\x93PaYnV[`\0Q\x15\x82\x15\x17\x15\x93P[PPP\x93\x92PPPV\xFE\xA2dipfsX\"\x12 8t\xBC\xF9.\x17\"\xCC{\xFA\x0C\xEF\x1A\t\x85\xCF\r\xC3H[\xA0f=\xB3t|\xCD\xF1`]\xF54dsolcC\0\x07\x06\x003",
    );
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `AddedOwner(address)` and selector `0x9465fa0c962cc76958e6373a993326400c1c94f8be2fe3a952adfa7f60b2ea26`.
    ```solidity
    event AddedOwner(address owner);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct AddedOwner {
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
        use alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::SolEvent for AddedOwner {
            type DataTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);
            const SIGNATURE: &'static str = "AddedOwner(address)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    148u8, 101u8, 250u8, 12u8, 150u8, 44u8, 199u8, 105u8, 88u8, 230u8, 55u8, 58u8,
                    153u8, 51u8, 38u8, 64u8, 12u8, 28u8, 148u8, 248u8, 190u8, 47u8, 227u8, 169u8,
                    82u8, 173u8, 250u8, 127u8, 96u8, 178u8, 234u8, 38u8,
                ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self { owner: data.0 }
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
                        &self.owner,
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
        impl alloy_sol_types::private::IntoLogData for AddedOwner {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&AddedOwner> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &AddedOwner) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `ApproveHash(bytes32,address)` and selector `0xf2a0eb156472d1440255b0d7c1e19cc07115d1051fe605b0dce69acfec884d9c`.
    ```solidity
    event ApproveHash(bytes32 indexed approvedHash, address indexed owner);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct ApproveHash {
        #[allow(missing_docs)]
        pub approvedHash: alloy_sol_types::private::FixedBytes<32>,
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
        use alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::SolEvent for ApproveHash {
            type DataTuple<'a> = ();
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "ApproveHash(bytes32,address)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    242u8, 160u8, 235u8, 21u8, 100u8, 114u8, 209u8, 68u8, 2u8, 85u8, 176u8, 215u8,
                    193u8, 225u8, 156u8, 192u8, 113u8, 21u8, 209u8, 5u8, 31u8, 230u8, 5u8, 176u8,
                    220u8, 230u8, 154u8, 207u8, 236u8, 136u8, 77u8, 156u8,
                ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    approvedHash: topics.1,
                    owner: topics.2,
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
                    self.approvedHash.clone(),
                    self.owner.clone(),
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
                out[1usize] = <alloy_sol_types::sol_data::FixedBytes<
                    32,
                > as alloy_sol_types::EventTopic>::encode_topic(&self.approvedHash);
                out[2usize] = <alloy_sol_types::sol_data::Address as alloy_sol_types::EventTopic>::encode_topic(
                    &self.owner,
                );
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for ApproveHash {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&ApproveHash> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &ApproveHash) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `ChangedFallbackHandler(address)` and selector `0x5ac6c46c93c8d0e53714ba3b53db3e7c046da994313d7ed0d192028bc7c228b0`.
    ```solidity
    event ChangedFallbackHandler(address handler);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct ChangedFallbackHandler {
        #[allow(missing_docs)]
        pub handler: alloy_sol_types::private::Address,
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
        impl alloy_sol_types::SolEvent for ChangedFallbackHandler {
            type DataTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);
            const SIGNATURE: &'static str = "ChangedFallbackHandler(address)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    90u8, 198u8, 196u8, 108u8, 147u8, 200u8, 208u8, 229u8, 55u8, 20u8, 186u8, 59u8,
                    83u8, 219u8, 62u8, 124u8, 4u8, 109u8, 169u8, 148u8, 49u8, 61u8, 126u8, 208u8,
                    209u8, 146u8, 2u8, 139u8, 199u8, 194u8, 40u8, 176u8,
                ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self { handler: data.0 }
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
                        &self.handler,
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
        impl alloy_sol_types::private::IntoLogData for ChangedFallbackHandler {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&ChangedFallbackHandler> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &ChangedFallbackHandler) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `ChangedGuard(address)` and selector `0x1151116914515bc0891ff9047a6cb32cf902546f83066499bcf8ba33d2353fa2`.
    ```solidity
    event ChangedGuard(address guard);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct ChangedGuard {
        #[allow(missing_docs)]
        pub guard: alloy_sol_types::private::Address,
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
        impl alloy_sol_types::SolEvent for ChangedGuard {
            type DataTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);
            const SIGNATURE: &'static str = "ChangedGuard(address)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    17u8, 81u8, 17u8, 105u8, 20u8, 81u8, 91u8, 192u8, 137u8, 31u8, 249u8, 4u8,
                    122u8, 108u8, 179u8, 44u8, 249u8, 2u8, 84u8, 111u8, 131u8, 6u8, 100u8, 153u8,
                    188u8, 248u8, 186u8, 51u8, 210u8, 53u8, 63u8, 162u8,
                ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self { guard: data.0 }
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
                        &self.guard,
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
        impl alloy_sol_types::private::IntoLogData for ChangedGuard {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&ChangedGuard> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &ChangedGuard) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `ChangedThreshold(uint256)` and selector `0x610f7ff2b304ae8903c3de74c60c6ab1f7d6226b3f52c5161905bb5ad4039c93`.
    ```solidity
    event ChangedThreshold(uint256 threshold);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct ChangedThreshold {
        #[allow(missing_docs)]
        pub threshold: alloy_sol_types::private::primitives::aliases::U256,
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
        impl alloy_sol_types::SolEvent for ChangedThreshold {
            type DataTuple<'a> = (alloy_sol_types::sol_data::Uint<256>,);
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);
            const SIGNATURE: &'static str = "ChangedThreshold(uint256)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    97u8, 15u8, 127u8, 242u8, 179u8, 4u8, 174u8, 137u8, 3u8, 195u8, 222u8, 116u8,
                    198u8, 12u8, 106u8, 177u8, 247u8, 214u8, 34u8, 107u8, 63u8, 82u8, 197u8, 22u8,
                    25u8, 5u8, 187u8, 90u8, 212u8, 3u8, 156u8, 147u8,
                ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self { threshold: data.0 }
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
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.threshold,
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
        impl alloy_sol_types::private::IntoLogData for ChangedThreshold {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&ChangedThreshold> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &ChangedThreshold) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `DisabledModule(address)` and selector `0xaab4fa2b463f581b2b32cb3b7e3b704b9ce37cc209b5fb4d77e593ace4054276`.
    ```solidity
    event DisabledModule(address module);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct DisabledModule {
        #[allow(missing_docs)]
        pub module: alloy_sol_types::private::Address,
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
        impl alloy_sol_types::SolEvent for DisabledModule {
            type DataTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);
            const SIGNATURE: &'static str = "DisabledModule(address)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    170u8, 180u8, 250u8, 43u8, 70u8, 63u8, 88u8, 27u8, 43u8, 50u8, 203u8, 59u8,
                    126u8, 59u8, 112u8, 75u8, 156u8, 227u8, 124u8, 194u8, 9u8, 181u8, 251u8, 77u8,
                    119u8, 229u8, 147u8, 172u8, 228u8, 5u8, 66u8, 118u8,
                ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self { module: data.0 }
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
                        &self.module,
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
        impl alloy_sol_types::private::IntoLogData for DisabledModule {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&DisabledModule> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &DisabledModule) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `EnabledModule(address)` and selector `0xecdf3a3effea5783a3c4c2140e677577666428d44ed9d474a0b3a4c9943f8440`.
    ```solidity
    event EnabledModule(address module);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct EnabledModule {
        #[allow(missing_docs)]
        pub module: alloy_sol_types::private::Address,
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
        impl alloy_sol_types::SolEvent for EnabledModule {
            type DataTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);
            const SIGNATURE: &'static str = "EnabledModule(address)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    236u8, 223u8, 58u8, 62u8, 255u8, 234u8, 87u8, 131u8, 163u8, 196u8, 194u8, 20u8,
                    14u8, 103u8, 117u8, 119u8, 102u8, 100u8, 40u8, 212u8, 78u8, 217u8, 212u8,
                    116u8, 160u8, 179u8, 164u8, 201u8, 148u8, 63u8, 132u8, 64u8,
                ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self { module: data.0 }
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
                        &self.module,
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
        impl alloy_sol_types::private::IntoLogData for EnabledModule {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&EnabledModule> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &EnabledModule) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `ExecutionFailure(bytes32,uint256)` and selector `0x23428b18acfb3ea64b08dc0c1d296ea9c09702c09083ca5272e64d115b687d23`.
    ```solidity
    event ExecutionFailure(bytes32 txHash, uint256 payment);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct ExecutionFailure {
        #[allow(missing_docs)]
        pub txHash: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub payment: alloy_sol_types::private::primitives::aliases::U256,
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
        impl alloy_sol_types::SolEvent for ExecutionFailure {
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);
            const SIGNATURE: &'static str = "ExecutionFailure(bytes32,uint256)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    35u8, 66u8, 139u8, 24u8, 172u8, 251u8, 62u8, 166u8, 75u8, 8u8, 220u8, 12u8,
                    29u8, 41u8, 110u8, 169u8, 192u8, 151u8, 2u8, 192u8, 144u8, 131u8, 202u8, 82u8,
                    114u8, 230u8, 77u8, 17u8, 91u8, 104u8, 125u8, 35u8,
                ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    txHash: data.0,
                    payment: data.1,
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
                    > as alloy_sol_types::SolType>::tokenize(&self.txHash),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.payment),
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
        impl alloy_sol_types::private::IntoLogData for ExecutionFailure {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&ExecutionFailure> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &ExecutionFailure) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `ExecutionFromModuleFailure(address)` and selector `0xacd2c8702804128fdb0db2bb49f6d127dd0181c13fd45dbfe16de0930e2bd375`.
    ```solidity
    event ExecutionFromModuleFailure(address indexed module);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct ExecutionFromModuleFailure {
        #[allow(missing_docs)]
        pub module: alloy_sol_types::private::Address,
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
        impl alloy_sol_types::SolEvent for ExecutionFromModuleFailure {
            type DataTuple<'a> = ();
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "ExecutionFromModuleFailure(address)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    172u8, 210u8, 200u8, 112u8, 40u8, 4u8, 18u8, 143u8, 219u8, 13u8, 178u8, 187u8,
                    73u8, 246u8, 209u8, 39u8, 221u8, 1u8, 129u8, 193u8, 63u8, 212u8, 93u8, 191u8,
                    225u8, 109u8, 224u8, 147u8, 14u8, 43u8, 211u8, 117u8,
                ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self { module: topics.1 }
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
                (Self::SIGNATURE_HASH.into(), self.module.clone())
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
                    &self.module,
                );
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for ExecutionFromModuleFailure {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&ExecutionFromModuleFailure> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &ExecutionFromModuleFailure) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `ExecutionFromModuleSuccess(address)` and selector `0x6895c13664aa4f67288b25d7a21d7aaa34916e355fb9b6fae0a139a9085becb8`.
    ```solidity
    event ExecutionFromModuleSuccess(address indexed module);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct ExecutionFromModuleSuccess {
        #[allow(missing_docs)]
        pub module: alloy_sol_types::private::Address,
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
        impl alloy_sol_types::SolEvent for ExecutionFromModuleSuccess {
            type DataTuple<'a> = ();
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "ExecutionFromModuleSuccess(address)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    104u8, 149u8, 193u8, 54u8, 100u8, 170u8, 79u8, 103u8, 40u8, 139u8, 37u8, 215u8,
                    162u8, 29u8, 122u8, 170u8, 52u8, 145u8, 110u8, 53u8, 95u8, 185u8, 182u8, 250u8,
                    224u8, 161u8, 57u8, 169u8, 8u8, 91u8, 236u8, 184u8,
                ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self { module: topics.1 }
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
                (Self::SIGNATURE_HASH.into(), self.module.clone())
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
                    &self.module,
                );
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for ExecutionFromModuleSuccess {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&ExecutionFromModuleSuccess> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &ExecutionFromModuleSuccess) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `ExecutionSuccess(bytes32,uint256)` and selector `0x442e715f626346e8c54381002da614f62bee8d27386535b2521ec8540898556e`.
    ```solidity
    event ExecutionSuccess(bytes32 txHash, uint256 payment);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct ExecutionSuccess {
        #[allow(missing_docs)]
        pub txHash: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub payment: alloy_sol_types::private::primitives::aliases::U256,
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
        impl alloy_sol_types::SolEvent for ExecutionSuccess {
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);
            const SIGNATURE: &'static str = "ExecutionSuccess(bytes32,uint256)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    68u8, 46u8, 113u8, 95u8, 98u8, 99u8, 70u8, 232u8, 197u8, 67u8, 129u8, 0u8,
                    45u8, 166u8, 20u8, 246u8, 43u8, 238u8, 141u8, 39u8, 56u8, 101u8, 53u8, 178u8,
                    82u8, 30u8, 200u8, 84u8, 8u8, 152u8, 85u8, 110u8,
                ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    txHash: data.0,
                    payment: data.1,
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
                    > as alloy_sol_types::SolType>::tokenize(&self.txHash),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.payment),
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
        impl alloy_sol_types::private::IntoLogData for ExecutionSuccess {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&ExecutionSuccess> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &ExecutionSuccess) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `RemovedOwner(address)` and selector `0xf8d49fc529812e9a7c5c50e69c20f0dccc0db8fa95c98bc58cc9a4f1c1299eaf`.
    ```solidity
    event RemovedOwner(address owner);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct RemovedOwner {
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
        use alloy_sol_types;
        #[automatically_derived]
        impl alloy_sol_types::SolEvent for RemovedOwner {
            type DataTuple<'a> = (alloy_sol_types::sol_data::Address,);
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (alloy_sol_types::sol_data::FixedBytes<32>,);
            const SIGNATURE: &'static str = "RemovedOwner(address)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    248u8, 212u8, 159u8, 197u8, 41u8, 129u8, 46u8, 154u8, 124u8, 92u8, 80u8, 230u8,
                    156u8, 32u8, 240u8, 220u8, 204u8, 13u8, 184u8, 250u8, 149u8, 201u8, 139u8,
                    197u8, 140u8, 201u8, 164u8, 241u8, 193u8, 41u8, 158u8, 175u8,
                ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self { owner: data.0 }
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
                        &self.owner,
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
        impl alloy_sol_types::private::IntoLogData for RemovedOwner {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&RemovedOwner> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &RemovedOwner) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `SafeReceived(address,uint256)` and selector `0x3d0ce9bfc3ed7d6862dbb28b2dea94561fe714a1b4d019aa8af39730d1ad7c3d`.
    ```solidity
    event SafeReceived(address indexed sender, uint256 value);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct SafeReceived {
        #[allow(missing_docs)]
        pub sender: alloy_sol_types::private::Address,
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
        #[automatically_derived]
        impl alloy_sol_types::SolEvent for SafeReceived {
            type DataTuple<'a> = (alloy_sol_types::sol_data::Uint<256>,);
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "SafeReceived(address,uint256)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    61u8, 12u8, 233u8, 191u8, 195u8, 237u8, 125u8, 104u8, 98u8, 219u8, 178u8,
                    139u8, 45u8, 234u8, 148u8, 86u8, 31u8, 231u8, 20u8, 161u8, 180u8, 208u8, 25u8,
                    170u8, 138u8, 243u8, 151u8, 48u8, 209u8, 173u8, 124u8, 61u8,
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
                    value: data.0,
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
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.value,
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
        impl alloy_sol_types::private::IntoLogData for SafeReceived {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&SafeReceived> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &SafeReceived) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `SafeSetup(address,address[],uint256,address,address)` and selector `0x141df868a6331af528e38c83b7aa03edc19be66e37ae67f9285bf4f8e3c6a1a8`.
    ```solidity
    event SafeSetup(address indexed initiator, address[] owners, uint256 threshold, address initializer, address fallbackHandler);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct SafeSetup {
        #[allow(missing_docs)]
        pub initiator: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub owners: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
        #[allow(missing_docs)]
        pub threshold: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub initializer: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub fallbackHandler: alloy_sol_types::private::Address,
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
        impl alloy_sol_types::SolEvent for SafeSetup {
            type DataTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
            );
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Address,
            );
            const SIGNATURE: &'static str = "SafeSetup(address,address[],uint256,address,address)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    20u8, 29u8, 248u8, 104u8, 166u8, 51u8, 26u8, 245u8, 40u8, 227u8, 140u8, 131u8,
                    183u8, 170u8, 3u8, 237u8, 193u8, 155u8, 230u8, 110u8, 55u8, 174u8, 103u8,
                    249u8, 40u8, 91u8, 244u8, 248u8, 227u8, 198u8, 161u8, 168u8,
                ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self {
                    initiator: topics.1,
                    owners: data.0,
                    threshold: data.1,
                    initializer: data.2,
                    fallbackHandler: data.3,
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
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Address,
                    > as alloy_sol_types::SolType>::tokenize(&self.owners),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.threshold),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.initializer,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.fallbackHandler,
                    ),
                )
            }
            #[inline]
            fn topics(&self) -> <Self::TopicList as alloy_sol_types::SolType>::RustType {
                (Self::SIGNATURE_HASH.into(), self.initiator.clone())
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
                    &self.initiator,
                );
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for SafeSetup {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&SafeSetup> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &SafeSetup) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Event with signature `SignMsg(bytes32)` and selector `0xe7f4675038f4f6034dfcbbb24c4dc08e4ebf10eb9d257d3d02c0f38d122ac6e4`.
    ```solidity
    event SignMsg(bytes32 indexed msgHash);
    ```*/
    #[allow(
        non_camel_case_types,
        non_snake_case,
        clippy::pub_underscore_fields,
        clippy::style
    )]
    #[derive(Clone)]
    pub struct SignMsg {
        #[allow(missing_docs)]
        pub msgHash: alloy_sol_types::private::FixedBytes<32>,
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
        impl alloy_sol_types::SolEvent for SignMsg {
            type DataTuple<'a> = ();
            type DataToken<'a> = <Self::DataTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            type TopicList = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::FixedBytes<32>,
            );
            const SIGNATURE: &'static str = "SignMsg(bytes32)";
            const SIGNATURE_HASH: alloy_sol_types::private::B256 =
                alloy_sol_types::private::B256::new([
                    231u8, 244u8, 103u8, 80u8, 56u8, 244u8, 246u8, 3u8, 77u8, 252u8, 187u8, 178u8,
                    76u8, 77u8, 192u8, 142u8, 78u8, 191u8, 16u8, 235u8, 157u8, 37u8, 125u8, 61u8,
                    2u8, 192u8, 243u8, 141u8, 18u8, 42u8, 198u8, 228u8,
                ]);
            const ANONYMOUS: bool = false;
            #[allow(unused_variables)]
            #[inline]
            fn new(
                topics: <Self::TopicList as alloy_sol_types::SolType>::RustType,
                data: <Self::DataTuple<'_> as alloy_sol_types::SolType>::RustType,
            ) -> Self {
                Self { msgHash: topics.1 }
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
                (Self::SIGNATURE_HASH.into(), self.msgHash.clone())
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
                > as alloy_sol_types::EventTopic>::encode_topic(&self.msgHash);
                Ok(())
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::private::IntoLogData for SignMsg {
            fn to_log_data(&self) -> alloy_sol_types::private::LogData {
                From::from(self)
            }
            fn into_log_data(self) -> alloy_sol_types::private::LogData {
                From::from(&self)
            }
        }
        #[automatically_derived]
        impl From<&SignMsg> for alloy_sol_types::private::LogData {
            #[inline]
            fn from(this: &SignMsg) -> alloy_sol_types::private::LogData {
                alloy_sol_types::SolEvent::encode_log_data(this)
            }
        }
    };
    /**Constructor`.
    ```solidity
    constructor();
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct constructorCall {}
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
            impl ::core::convert::From<constructorCall> for UnderlyingRustTuple<'_> {
                fn from(value: constructorCall) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for constructorCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolConstructor for constructorCall {
            type Parameters<'a> = ();
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
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
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `VERSION()` and selector `0xffa1ad74`.
    ```solidity
    function VERSION() external view returns (string memory);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct VERSIONCall;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`VERSION()`](VERSIONCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct VERSIONReturn {
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
            impl ::core::convert::From<VERSIONCall> for UnderlyingRustTuple<'_> {
                fn from(value: VERSIONCall) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for VERSIONCall {
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
            impl ::core::convert::From<VERSIONReturn> for UnderlyingRustTuple<'_> {
                fn from(value: VERSIONReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for VERSIONReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for VERSIONCall {
            type Parameters<'a> = ();
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::String;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::String,);
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "VERSION()";
            const SELECTOR: [u8; 4] = [255u8, 161u8, 173u8, 116u8];
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
                        let r: VERSIONReturn = r.into();
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
                    let r: VERSIONReturn = r.into();
                    r._0
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `addOwnerWithThreshold(address,uint256)` and selector `0x0d582f13`.
    ```solidity
    function addOwnerWithThreshold(address owner, uint256 _threshold) external;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct addOwnerWithThresholdCall {
        #[allow(missing_docs)]
        pub owner: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub _threshold: alloy_sol_types::private::primitives::aliases::U256,
    }
    ///Container type for the return parameters of the [`addOwnerWithThreshold(address,uint256)`](addOwnerWithThresholdCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct addOwnerWithThresholdReturn {}
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
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
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
            impl ::core::convert::From<addOwnerWithThresholdCall> for UnderlyingRustTuple<'_> {
                fn from(value: addOwnerWithThresholdCall) -> Self {
                    (value.owner, value._threshold)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for addOwnerWithThresholdCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        owner: tuple.0,
                        _threshold: tuple.1,
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
            impl ::core::convert::From<addOwnerWithThresholdReturn> for UnderlyingRustTuple<'_> {
                fn from(value: addOwnerWithThresholdReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for addOwnerWithThresholdReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl addOwnerWithThresholdReturn {
            fn _tokenize(
                &self,
            ) -> <addOwnerWithThresholdCall as alloy_sol_types::SolCall>::ReturnToken<'_>
            {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for addOwnerWithThresholdCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = addOwnerWithThresholdReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "addOwnerWithThreshold(address,uint256)";
            const SELECTOR: [u8; 4] = [13u8, 88u8, 47u8, 19u8];
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
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self._threshold,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                addOwnerWithThresholdReturn::_tokenize(ret)
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
    /**Function with signature `approveHash(bytes32)` and selector `0xd4d9bdcd`.
    ```solidity
    function approveHash(bytes32 hashToApprove) external;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct approveHashCall {
        #[allow(missing_docs)]
        pub hashToApprove: alloy_sol_types::private::FixedBytes<32>,
    }
    ///Container type for the return parameters of the [`approveHash(bytes32)`](approveHashCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct approveHashReturn {}
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
            impl ::core::convert::From<approveHashCall> for UnderlyingRustTuple<'_> {
                fn from(value: approveHashCall) -> Self {
                    (value.hashToApprove,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for approveHashCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        hashToApprove: tuple.0,
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
            impl ::core::convert::From<approveHashReturn> for UnderlyingRustTuple<'_> {
                fn from(value: approveHashReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for approveHashReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl approveHashReturn {
            fn _tokenize(&self) -> <approveHashCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for approveHashCall {
            type Parameters<'a> = (alloy_sol_types::sol_data::FixedBytes<32>,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = approveHashReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "approveHash(bytes32)";
            const SELECTOR: [u8; 4] = [212u8, 217u8, 189u8, 205u8];
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
                    > as alloy_sol_types::SolType>::tokenize(&self.hashToApprove),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                approveHashReturn::_tokenize(ret)
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
    /**Function with signature `approvedHashes(address,bytes32)` and selector `0x7d832974`.
    ```solidity
    function approvedHashes(address, bytes32) external view returns (uint256);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct approvedHashesCall {
        #[allow(missing_docs)]
        pub _0: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub _1: alloy_sol_types::private::FixedBytes<32>,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`approvedHashes(address,bytes32)`](approvedHashesCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct approvedHashesReturn {
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
        use alloy_sol_types;
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::FixedBytes<32>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
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
            impl ::core::convert::From<approvedHashesCall> for UnderlyingRustTuple<'_> {
                fn from(value: approvedHashesCall) -> Self {
                    (value._0, value._1)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for approvedHashesCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        _0: tuple.0,
                        _1: tuple.1,
                    }
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
            impl ::core::convert::From<approvedHashesReturn> for UnderlyingRustTuple<'_> {
                fn from(value: approvedHashesReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for approvedHashesReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for approvedHashesCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::FixedBytes<32>,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::primitives::aliases::U256;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Uint<256>,);
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "approvedHashes(address,bytes32)";
            const SELECTOR: [u8; 4] = [125u8, 131u8, 41u8, 116u8];
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
                        &self._0,
                    ),
                    <alloy_sol_types::sol_data::FixedBytes<
                        32,
                    > as alloy_sol_types::SolType>::tokenize(&self._1),
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
                        let r: approvedHashesReturn = r.into();
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
                    let r: approvedHashesReturn = r.into();
                    r._0
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `changeThreshold(uint256)` and selector `0x694e80c3`.
    ```solidity
    function changeThreshold(uint256 _threshold) external;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct changeThresholdCall {
        #[allow(missing_docs)]
        pub _threshold: alloy_sol_types::private::primitives::aliases::U256,
    }
    ///Container type for the return parameters of the [`changeThreshold(uint256)`](changeThresholdCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct changeThresholdReturn {}
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
            impl ::core::convert::From<changeThresholdCall> for UnderlyingRustTuple<'_> {
                fn from(value: changeThresholdCall) -> Self {
                    (value._threshold,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for changeThresholdCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        _threshold: tuple.0,
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
            impl ::core::convert::From<changeThresholdReturn> for UnderlyingRustTuple<'_> {
                fn from(value: changeThresholdReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for changeThresholdReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl changeThresholdReturn {
            fn _tokenize(
                &self,
            ) -> <changeThresholdCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for changeThresholdCall {
            type Parameters<'a> = (alloy_sol_types::sol_data::Uint<256>,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = changeThresholdReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "changeThreshold(uint256)";
            const SELECTOR: [u8; 4] = [105u8, 78u8, 128u8, 195u8];
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
                        &self._threshold,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                changeThresholdReturn::_tokenize(ret)
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
    /**Function with signature `checkNSignatures(bytes32,bytes,bytes,uint256)` and selector `0x12fb68e0`.
    ```solidity
    function checkNSignatures(bytes32 dataHash, bytes memory data, bytes memory signatures, uint256 requiredSignatures) external view;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct checkNSignaturesCall {
        #[allow(missing_docs)]
        pub dataHash: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub data: alloy_sol_types::private::Bytes,
        #[allow(missing_docs)]
        pub signatures: alloy_sol_types::private::Bytes,
        #[allow(missing_docs)]
        pub requiredSignatures: alloy_sol_types::private::primitives::aliases::U256,
    }
    ///Container type for the return parameters of the [`checkNSignatures(bytes32,bytes,bytes,uint256)`](checkNSignaturesCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct checkNSignaturesReturn {}
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
                alloy_sol_types::sol_data::Bytes,
                alloy_sol_types::sol_data::Uint<256>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::FixedBytes<32>,
                alloy_sol_types::private::Bytes,
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
            impl ::core::convert::From<checkNSignaturesCall> for UnderlyingRustTuple<'_> {
                fn from(value: checkNSignaturesCall) -> Self {
                    (
                        value.dataHash,
                        value.data,
                        value.signatures,
                        value.requiredSignatures,
                    )
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for checkNSignaturesCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        dataHash: tuple.0,
                        data: tuple.1,
                        signatures: tuple.2,
                        requiredSignatures: tuple.3,
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
            impl ::core::convert::From<checkNSignaturesReturn> for UnderlyingRustTuple<'_> {
                fn from(value: checkNSignaturesReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for checkNSignaturesReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl checkNSignaturesReturn {
            fn _tokenize(
                &self,
            ) -> <checkNSignaturesCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for checkNSignaturesCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Bytes,
                alloy_sol_types::sol_data::Bytes,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = checkNSignaturesReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "checkNSignatures(bytes32,bytes,bytes,uint256)";
            const SELECTOR: [u8; 4] = [18u8, 251u8, 104u8, 224u8];
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
                    > as alloy_sol_types::SolType>::tokenize(&self.dataHash),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.data,
                    ),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.signatures,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.requiredSignatures),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                checkNSignaturesReturn::_tokenize(ret)
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
    /**Function with signature `checkSignatures(bytes32,bytes,bytes)` and selector `0x934f3a11`.
    ```solidity
    function checkSignatures(bytes32 dataHash, bytes memory data, bytes memory signatures) external view;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct checkSignaturesCall {
        #[allow(missing_docs)]
        pub dataHash: alloy_sol_types::private::FixedBytes<32>,
        #[allow(missing_docs)]
        pub data: alloy_sol_types::private::Bytes,
        #[allow(missing_docs)]
        pub signatures: alloy_sol_types::private::Bytes,
    }
    ///Container type for the return parameters of the [`checkSignatures(bytes32,bytes,bytes)`](checkSignaturesCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct checkSignaturesReturn {}
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
                alloy_sol_types::sol_data::Bytes,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::FixedBytes<32>,
                alloy_sol_types::private::Bytes,
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
            impl ::core::convert::From<checkSignaturesCall> for UnderlyingRustTuple<'_> {
                fn from(value: checkSignaturesCall) -> Self {
                    (value.dataHash, value.data, value.signatures)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for checkSignaturesCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        dataHash: tuple.0,
                        data: tuple.1,
                        signatures: tuple.2,
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
            impl ::core::convert::From<checkSignaturesReturn> for UnderlyingRustTuple<'_> {
                fn from(value: checkSignaturesReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for checkSignaturesReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl checkSignaturesReturn {
            fn _tokenize(
                &self,
            ) -> <checkSignaturesCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for checkSignaturesCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::FixedBytes<32>,
                alloy_sol_types::sol_data::Bytes,
                alloy_sol_types::sol_data::Bytes,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = checkSignaturesReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "checkSignatures(bytes32,bytes,bytes)";
            const SELECTOR: [u8; 4] = [147u8, 79u8, 58u8, 17u8];
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
                    > as alloy_sol_types::SolType>::tokenize(&self.dataHash),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.data,
                    ),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.signatures,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                checkSignaturesReturn::_tokenize(ret)
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
    /**Function with signature `disableModule(address,address)` and selector `0xe009cfde`.
    ```solidity
    function disableModule(address prevModule, address module) external;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct disableModuleCall {
        #[allow(missing_docs)]
        pub prevModule: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub module: alloy_sol_types::private::Address,
    }
    ///Container type for the return parameters of the [`disableModule(address,address)`](disableModuleCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct disableModuleReturn {}
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
            impl ::core::convert::From<disableModuleCall> for UnderlyingRustTuple<'_> {
                fn from(value: disableModuleCall) -> Self {
                    (value.prevModule, value.module)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for disableModuleCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        prevModule: tuple.0,
                        module: tuple.1,
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
            impl ::core::convert::From<disableModuleReturn> for UnderlyingRustTuple<'_> {
                fn from(value: disableModuleReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for disableModuleReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl disableModuleReturn {
            fn _tokenize(
                &self,
            ) -> <disableModuleCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for disableModuleCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = disableModuleReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "disableModule(address,address)";
            const SELECTOR: [u8; 4] = [224u8, 9u8, 207u8, 222u8];
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
                        &self.prevModule,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.module,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                disableModuleReturn::_tokenize(ret)
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
            fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<domainSeparatorReturn> for UnderlyingRustTuple<'_> {
                fn from(value: domainSeparatorReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for domainSeparatorReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for domainSeparatorCall {
            type Parameters<'a> = ();
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::FixedBytes<32>;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::FixedBytes<32>,);
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
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
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence(data).map(
                    |r| {
                        let r: domainSeparatorReturn = r.into();
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
                    let r: domainSeparatorReturn = r.into();
                    r._0
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `enableModule(address)` and selector `0x610b5925`.
    ```solidity
    function enableModule(address module) external;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct enableModuleCall {
        #[allow(missing_docs)]
        pub module: alloy_sol_types::private::Address,
    }
    ///Container type for the return parameters of the [`enableModule(address)`](enableModuleCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct enableModuleReturn {}
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
            impl ::core::convert::From<enableModuleCall> for UnderlyingRustTuple<'_> {
                fn from(value: enableModuleCall) -> Self {
                    (value.module,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for enableModuleCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { module: tuple.0 }
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
            impl ::core::convert::From<enableModuleReturn> for UnderlyingRustTuple<'_> {
                fn from(value: enableModuleReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for enableModuleReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl enableModuleReturn {
            fn _tokenize(&self) -> <enableModuleCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for enableModuleCall {
            type Parameters<'a> = (alloy_sol_types::sol_data::Address,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = enableModuleReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "enableModule(address)";
            const SELECTOR: [u8; 4] = [97u8, 11u8, 89u8, 37u8];
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
                        &self.module,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                enableModuleReturn::_tokenize(ret)
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
    /**Function with signature `encodeTransactionData(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256)` and selector `0xe86637db`.
    ```solidity
    function encodeTransactionData(address to, uint256 value, bytes memory data, Enum.Operation operation, uint256 safeTxGas, uint256 baseGas, uint256 gasPrice, address gasToken, address refundReceiver, uint256 _nonce) external view returns (bytes memory);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct encodeTransactionDataCall {
        #[allow(missing_docs)]
        pub to: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub value: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub data: alloy_sol_types::private::Bytes,
        #[allow(missing_docs)]
        pub operation: <Enum::Operation as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub safeTxGas: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub baseGas: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub gasPrice: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub gasToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub refundReceiver: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub _nonce: alloy_sol_types::private::primitives::aliases::U256,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`encodeTransactionData(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256)`](encodeTransactionDataCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct encodeTransactionDataReturn {
        #[allow(missing_docs)]
        pub _0: alloy_sol_types::private::Bytes,
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
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Bytes,
                Enum::Operation,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::Bytes,
                <Enum::Operation as alloy_sol_types::SolType>::RustType,
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::Address,
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
            impl ::core::convert::From<encodeTransactionDataCall> for UnderlyingRustTuple<'_> {
                fn from(value: encodeTransactionDataCall) -> Self {
                    (
                        value.to,
                        value.value,
                        value.data,
                        value.operation,
                        value.safeTxGas,
                        value.baseGas,
                        value.gasPrice,
                        value.gasToken,
                        value.refundReceiver,
                        value._nonce,
                    )
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for encodeTransactionDataCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        to: tuple.0,
                        value: tuple.1,
                        data: tuple.2,
                        operation: tuple.3,
                        safeTxGas: tuple.4,
                        baseGas: tuple.5,
                        gasPrice: tuple.6,
                        gasToken: tuple.7,
                        refundReceiver: tuple.8,
                        _nonce: tuple.9,
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
            fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<encodeTransactionDataReturn> for UnderlyingRustTuple<'_> {
                fn from(value: encodeTransactionDataReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for encodeTransactionDataReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for encodeTransactionDataCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Bytes,
                Enum::Operation,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::Bytes;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Bytes,);
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "encodeTransactionData(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256)";
            const SELECTOR: [u8; 4] = [232u8, 102u8, 55u8, 219u8];
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
                        &self.to,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.value,
                    ),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.data,
                    ),
                    <Enum::Operation as alloy_sol_types::SolType>::tokenize(&self.operation),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.safeTxGas,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.baseGas,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.gasPrice,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.gasToken,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.refundReceiver,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self._nonce,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                (<alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(ret),)
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence(data).map(
                    |r| {
                        let r: encodeTransactionDataReturn = r.into();
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
                    let r: encodeTransactionDataReturn = r.into();
                    r._0
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `execTransaction(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,bytes)` and selector `0x6a761202`.
    ```solidity
    function execTransaction(address to, uint256 value, bytes memory data, Enum.Operation operation, uint256 safeTxGas, uint256 baseGas, uint256 gasPrice, address gasToken, address refundReceiver, bytes memory signatures) external payable returns (bool success);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct execTransactionCall {
        #[allow(missing_docs)]
        pub to: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub value: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub data: alloy_sol_types::private::Bytes,
        #[allow(missing_docs)]
        pub operation: <Enum::Operation as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub safeTxGas: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub baseGas: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub gasPrice: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub gasToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub refundReceiver: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub signatures: alloy_sol_types::private::Bytes,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`execTransaction(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,bytes)`](execTransactionCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct execTransactionReturn {
        #[allow(missing_docs)]
        pub success: bool,
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
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Bytes,
                Enum::Operation,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Bytes,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::Bytes,
                <Enum::Operation as alloy_sol_types::SolType>::RustType,
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::Address,
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
            impl ::core::convert::From<execTransactionCall> for UnderlyingRustTuple<'_> {
                fn from(value: execTransactionCall) -> Self {
                    (
                        value.to,
                        value.value,
                        value.data,
                        value.operation,
                        value.safeTxGas,
                        value.baseGas,
                        value.gasPrice,
                        value.gasToken,
                        value.refundReceiver,
                        value.signatures,
                    )
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for execTransactionCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        to: tuple.0,
                        value: tuple.1,
                        data: tuple.2,
                        operation: tuple.3,
                        safeTxGas: tuple.4,
                        baseGas: tuple.5,
                        gasPrice: tuple.6,
                        gasToken: tuple.7,
                        refundReceiver: tuple.8,
                        signatures: tuple.9,
                    }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (alloy_sol_types::sol_data::Bool,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (bool,);
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
            impl ::core::convert::From<execTransactionReturn> for UnderlyingRustTuple<'_> {
                fn from(value: execTransactionReturn) -> Self {
                    (value.success,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for execTransactionReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { success: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for execTransactionCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Bytes,
                Enum::Operation,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Bytes,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = bool;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Bool,);
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "execTransaction(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,bytes)";
            const SELECTOR: [u8; 4] = [106u8, 118u8, 18u8, 2u8];
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
                        &self.to,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.value,
                    ),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.data,
                    ),
                    <Enum::Operation as alloy_sol_types::SolType>::tokenize(&self.operation),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.safeTxGas,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.baseGas,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.gasPrice,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.gasToken,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.refundReceiver,
                    ),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.signatures,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                (<alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::tokenize(ret),)
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence(data).map(
                    |r| {
                        let r: execTransactionReturn = r.into();
                        r.success
                    },
                )
            }
            #[inline]
            fn abi_decode_returns_validate(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence_validate(
                    data,
                )
                .map(|r| {
                    let r: execTransactionReturn = r.into();
                    r.success
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `execTransactionFromModule(address,uint256,bytes,uint8)` and selector `0x468721a7`.
    ```solidity
    function execTransactionFromModule(address to, uint256 value, bytes memory data, Enum.Operation operation) external returns (bool success);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct execTransactionFromModuleCall {
        #[allow(missing_docs)]
        pub to: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub value: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub data: alloy_sol_types::private::Bytes,
        #[allow(missing_docs)]
        pub operation: <Enum::Operation as alloy_sol_types::SolType>::RustType,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`execTransactionFromModule(address,uint256,bytes,uint8)`](execTransactionFromModuleCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct execTransactionFromModuleReturn {
        #[allow(missing_docs)]
        pub success: bool,
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
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Bytes,
                Enum::Operation,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::Bytes,
                <Enum::Operation as alloy_sol_types::SolType>::RustType,
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
            impl ::core::convert::From<execTransactionFromModuleCall> for UnderlyingRustTuple<'_> {
                fn from(value: execTransactionFromModuleCall) -> Self {
                    (value.to, value.value, value.data, value.operation)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for execTransactionFromModuleCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        to: tuple.0,
                        value: tuple.1,
                        data: tuple.2,
                        operation: tuple.3,
                    }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (alloy_sol_types::sol_data::Bool,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (bool,);
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
            impl ::core::convert::From<execTransactionFromModuleReturn> for UnderlyingRustTuple<'_> {
                fn from(value: execTransactionFromModuleReturn) -> Self {
                    (value.success,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for execTransactionFromModuleReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { success: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for execTransactionFromModuleCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Bytes,
                Enum::Operation,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = bool;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Bool,);
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str =
                "execTransactionFromModule(address,uint256,bytes,uint8)";
            const SELECTOR: [u8; 4] = [70u8, 135u8, 33u8, 167u8];
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
                        &self.to,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.value,
                    ),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.data,
                    ),
                    <Enum::Operation as alloy_sol_types::SolType>::tokenize(&self.operation),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                (<alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::tokenize(ret),)
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence(data).map(
                    |r| {
                        let r: execTransactionFromModuleReturn = r.into();
                        r.success
                    },
                )
            }
            #[inline]
            fn abi_decode_returns_validate(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence_validate(
                    data,
                )
                .map(|r| {
                    let r: execTransactionFromModuleReturn = r.into();
                    r.success
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `execTransactionFromModuleReturnData(address,uint256,bytes,uint8)` and selector `0x5229073f`.
    ```solidity
    function execTransactionFromModuleReturnData(address to, uint256 value, bytes memory data, Enum.Operation operation) external returns (bool success, bytes memory returnData);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct execTransactionFromModuleReturnDataCall {
        #[allow(missing_docs)]
        pub to: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub value: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub data: alloy_sol_types::private::Bytes,
        #[allow(missing_docs)]
        pub operation: <Enum::Operation as alloy_sol_types::SolType>::RustType,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`execTransactionFromModuleReturnData(address,uint256,bytes,uint8)`](execTransactionFromModuleReturnDataCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct execTransactionFromModuleReturnDataReturn {
        #[allow(missing_docs)]
        pub success: bool,
        #[allow(missing_docs)]
        pub returnData: alloy_sol_types::private::Bytes,
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
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Bytes,
                Enum::Operation,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::Bytes,
                <Enum::Operation as alloy_sol_types::SolType>::RustType,
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
            impl ::core::convert::From<execTransactionFromModuleReturnDataCall> for UnderlyingRustTuple<'_> {
                fn from(value: execTransactionFromModuleReturnDataCall) -> Self {
                    (value.to, value.value, value.data, value.operation)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for execTransactionFromModuleReturnDataCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        to: tuple.0,
                        value: tuple.1,
                        data: tuple.2,
                        operation: tuple.3,
                    }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Bool,
                alloy_sol_types::sol_data::Bytes,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (bool, alloy_sol_types::private::Bytes);
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
            impl ::core::convert::From<execTransactionFromModuleReturnDataReturn> for UnderlyingRustTuple<'_> {
                fn from(value: execTransactionFromModuleReturnDataReturn) -> Self {
                    (value.success, value.returnData)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for execTransactionFromModuleReturnDataReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        success: tuple.0,
                        returnData: tuple.1,
                    }
                }
            }
        }
        impl execTransactionFromModuleReturnDataReturn {
            fn _tokenize(
                &self,
            ) -> <execTransactionFromModuleReturnDataCall as alloy_sol_types::SolCall>::ReturnToken<
                '_,
            >{
                (
                    <alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::tokenize(
                        &self.success,
                    ),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.returnData,
                    ),
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for execTransactionFromModuleReturnDataCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Bytes,
                Enum::Operation,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = execTransactionFromModuleReturnDataReturn;
            type ReturnTuple<'a> = (
                alloy_sol_types::sol_data::Bool,
                alloy_sol_types::sol_data::Bytes,
            );
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str =
                "execTransactionFromModuleReturnData(address,uint256,bytes,uint8)";
            const SELECTOR: [u8; 4] = [82u8, 41u8, 7u8, 63u8];
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
                        &self.to,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.value,
                    ),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.data,
                    ),
                    <Enum::Operation as alloy_sol_types::SolType>::tokenize(&self.operation),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                execTransactionFromModuleReturnDataReturn::_tokenize(ret)
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
    /**Function with signature `getChainId()` and selector `0x3408e470`.
    ```solidity
    function getChainId() external view returns (uint256);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct getChainIdCall;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`getChainId()`](getChainIdCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct getChainIdReturn {
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
            impl ::core::convert::From<getChainIdCall> for UnderlyingRustTuple<'_> {
                fn from(value: getChainIdCall) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for getChainIdCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self
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
            impl ::core::convert::From<getChainIdReturn> for UnderlyingRustTuple<'_> {
                fn from(value: getChainIdReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for getChainIdReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for getChainIdCall {
            type Parameters<'a> = ();
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::primitives::aliases::U256;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Uint<256>,);
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "getChainId()";
            const SELECTOR: [u8; 4] = [52u8, 8u8, 228u8, 112u8];
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
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        ret,
                    ),
                )
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence(data).map(
                    |r| {
                        let r: getChainIdReturn = r.into();
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
                    let r: getChainIdReturn = r.into();
                    r._0
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `getModulesPaginated(address,uint256)` and selector `0xcc2f8452`.
    ```solidity
    function getModulesPaginated(address start, uint256 pageSize) external view returns (address[] memory array, address next);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct getModulesPaginatedCall {
        #[allow(missing_docs)]
        pub start: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub pageSize: alloy_sol_types::private::primitives::aliases::U256,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`getModulesPaginated(address,uint256)`](getModulesPaginatedCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct getModulesPaginatedReturn {
        #[allow(missing_docs)]
        pub array: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
        #[allow(missing_docs)]
        pub next: alloy_sol_types::private::Address,
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
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
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
            impl ::core::convert::From<getModulesPaginatedCall> for UnderlyingRustTuple<'_> {
                fn from(value: getModulesPaginatedCall) -> Self {
                    (value.start, value.pageSize)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for getModulesPaginatedCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        start: tuple.0,
                        pageSize: tuple.1,
                    }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Address,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
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
            impl ::core::convert::From<getModulesPaginatedReturn> for UnderlyingRustTuple<'_> {
                fn from(value: getModulesPaginatedReturn) -> Self {
                    (value.array, value.next)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for getModulesPaginatedReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        array: tuple.0,
                        next: tuple.1,
                    }
                }
            }
        }
        impl getModulesPaginatedReturn {
            fn _tokenize(
                &self,
            ) -> <getModulesPaginatedCall as alloy_sol_types::SolCall>::ReturnToken<'_>
            {
                (
                    <alloy_sol_types::sol_data::Array<
                        alloy_sol_types::sol_data::Address,
                    > as alloy_sol_types::SolType>::tokenize(&self.array),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.next,
                    ),
                )
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for getModulesPaginatedCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = getModulesPaginatedReturn;
            type ReturnTuple<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Address,
            );
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "getModulesPaginated(address,uint256)";
            const SELECTOR: [u8; 4] = [204u8, 47u8, 132u8, 82u8];
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
                        &self.start,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.pageSize,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                getModulesPaginatedReturn::_tokenize(ret)
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
    /**Function with signature `getOwners()` and selector `0xa0e67e2b`.
    ```solidity
    function getOwners() external view returns (address[] memory);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct getOwnersCall;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`getOwners()`](getOwnersCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct getOwnersReturn {
        #[allow(missing_docs)]
        pub _0: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
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
            impl ::core::convert::From<getOwnersCall> for UnderlyingRustTuple<'_> {
                fn from(value: getOwnersCall) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for getOwnersCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> =
                (alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> =
                (alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,);
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
            impl ::core::convert::From<getOwnersReturn> for UnderlyingRustTuple<'_> {
                fn from(value: getOwnersReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for getOwnersReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for getOwnersCall {
            type Parameters<'a> = ();
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::Vec<alloy_sol_types::private::Address>;
            type ReturnTuple<'a> =
                (alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,);
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "getOwners()";
            const SELECTOR: [u8; 4] = [160u8, 230u8, 126u8, 43u8];
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
                (<alloy_sol_types::sol_data::Array<
                    alloy_sol_types::sol_data::Address,
                > as alloy_sol_types::SolType>::tokenize(ret),)
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence(data).map(
                    |r| {
                        let r: getOwnersReturn = r.into();
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
                    let r: getOwnersReturn = r.into();
                    r._0
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `getStorageAt(uint256,uint256)` and selector `0x5624b25b`.
    ```solidity
    function getStorageAt(uint256 offset, uint256 length) external view returns (bytes memory);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct getStorageAtCall {
        #[allow(missing_docs)]
        pub offset: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub length: alloy_sol_types::private::primitives::aliases::U256,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`getStorageAt(uint256,uint256)`](getStorageAtCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct getStorageAtReturn {
        #[allow(missing_docs)]
        pub _0: alloy_sol_types::private::Bytes,
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
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
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
            impl ::core::convert::From<getStorageAtCall> for UnderlyingRustTuple<'_> {
                fn from(value: getStorageAtCall) -> Self {
                    (value.offset, value.length)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for getStorageAtCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        offset: tuple.0,
                        length: tuple.1,
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
            fn _type_assertion(_t: alloy_sol_types::private::AssertTypeEq<UnderlyingRustTuple>) {
                match _t {
                    alloy_sol_types::private::AssertTypeEq::<
                        <UnderlyingSolTuple as alloy_sol_types::SolType>::RustType,
                    >(_) => {}
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<getStorageAtReturn> for UnderlyingRustTuple<'_> {
                fn from(value: getStorageAtReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for getStorageAtReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for getStorageAtCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::Bytes;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Bytes,);
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "getStorageAt(uint256,uint256)";
            const SELECTOR: [u8; 4] = [86u8, 36u8, 178u8, 91u8];
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
                        &self.offset,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.length,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                (<alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(ret),)
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence(data).map(
                    |r| {
                        let r: getStorageAtReturn = r.into();
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
                    let r: getStorageAtReturn = r.into();
                    r._0
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `getThreshold()` and selector `0xe75235b8`.
    ```solidity
    function getThreshold() external view returns (uint256);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct getThresholdCall;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`getThreshold()`](getThresholdCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct getThresholdReturn {
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
            impl ::core::convert::From<getThresholdCall> for UnderlyingRustTuple<'_> {
                fn from(value: getThresholdCall) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for getThresholdCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self
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
            impl ::core::convert::From<getThresholdReturn> for UnderlyingRustTuple<'_> {
                fn from(value: getThresholdReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for getThresholdReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for getThresholdCall {
            type Parameters<'a> = ();
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::primitives::aliases::U256;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Uint<256>,);
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "getThreshold()";
            const SELECTOR: [u8; 4] = [231u8, 82u8, 53u8, 184u8];
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
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        ret,
                    ),
                )
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence(data).map(
                    |r| {
                        let r: getThresholdReturn = r.into();
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
                    let r: getThresholdReturn = r.into();
                    r._0
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `getTransactionHash(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256)` and selector `0xd8d11f78`.
    ```solidity
    function getTransactionHash(address to, uint256 value, bytes memory data, Enum.Operation operation, uint256 safeTxGas, uint256 baseGas, uint256 gasPrice, address gasToken, address refundReceiver, uint256 _nonce) external view returns (bytes32);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct getTransactionHashCall {
        #[allow(missing_docs)]
        pub to: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub value: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub data: alloy_sol_types::private::Bytes,
        #[allow(missing_docs)]
        pub operation: <Enum::Operation as alloy_sol_types::SolType>::RustType,
        #[allow(missing_docs)]
        pub safeTxGas: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub baseGas: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub gasPrice: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub gasToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub refundReceiver: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub _nonce: alloy_sol_types::private::primitives::aliases::U256,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`getTransactionHash(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256)`](getTransactionHashCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct getTransactionHashReturn {
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
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Bytes,
                Enum::Operation,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::Bytes,
                <Enum::Operation as alloy_sol_types::SolType>::RustType,
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::Address,
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
            impl ::core::convert::From<getTransactionHashCall> for UnderlyingRustTuple<'_> {
                fn from(value: getTransactionHashCall) -> Self {
                    (
                        value.to,
                        value.value,
                        value.data,
                        value.operation,
                        value.safeTxGas,
                        value.baseGas,
                        value.gasPrice,
                        value.gasToken,
                        value.refundReceiver,
                        value._nonce,
                    )
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for getTransactionHashCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        to: tuple.0,
                        value: tuple.1,
                        data: tuple.2,
                        operation: tuple.3,
                        safeTxGas: tuple.4,
                        baseGas: tuple.5,
                        gasPrice: tuple.6,
                        gasToken: tuple.7,
                        refundReceiver: tuple.8,
                        _nonce: tuple.9,
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
            impl ::core::convert::From<getTransactionHashReturn> for UnderlyingRustTuple<'_> {
                fn from(value: getTransactionHashReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for getTransactionHashReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for getTransactionHashCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Bytes,
                Enum::Operation,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::FixedBytes<32>;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::FixedBytes<32>,);
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "getTransactionHash(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,uint256)";
            const SELECTOR: [u8; 4] = [216u8, 209u8, 31u8, 120u8];
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
                        &self.to,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.value,
                    ),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.data,
                    ),
                    <Enum::Operation as alloy_sol_types::SolType>::tokenize(&self.operation),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.safeTxGas,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.baseGas,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.gasPrice,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.gasToken,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.refundReceiver,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self._nonce,
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
                        let r: getTransactionHashReturn = r.into();
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
                    let r: getTransactionHashReturn = r.into();
                    r._0
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `isModuleEnabled(address)` and selector `0x2d9ad53d`.
    ```solidity
    function isModuleEnabled(address module) external view returns (bool);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct isModuleEnabledCall {
        #[allow(missing_docs)]
        pub module: alloy_sol_types::private::Address,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`isModuleEnabled(address)`](isModuleEnabledCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct isModuleEnabledReturn {
        #[allow(missing_docs)]
        pub _0: bool,
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
            impl ::core::convert::From<isModuleEnabledCall> for UnderlyingRustTuple<'_> {
                fn from(value: isModuleEnabledCall) -> Self {
                    (value.module,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for isModuleEnabledCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { module: tuple.0 }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (alloy_sol_types::sol_data::Bool,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (bool,);
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
            impl ::core::convert::From<isModuleEnabledReturn> for UnderlyingRustTuple<'_> {
                fn from(value: isModuleEnabledReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for isModuleEnabledReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for isModuleEnabledCall {
            type Parameters<'a> = (alloy_sol_types::sol_data::Address,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = bool;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Bool,);
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "isModuleEnabled(address)";
            const SELECTOR: [u8; 4] = [45u8, 154u8, 213u8, 61u8];
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
                        &self.module,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                (<alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::tokenize(ret),)
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence(data).map(
                    |r| {
                        let r: isModuleEnabledReturn = r.into();
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
                    let r: isModuleEnabledReturn = r.into();
                    r._0
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `isOwner(address)` and selector `0x2f54bf6e`.
    ```solidity
    function isOwner(address owner) external view returns (bool);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct isOwnerCall {
        #[allow(missing_docs)]
        pub owner: alloy_sol_types::private::Address,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`isOwner(address)`](isOwnerCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct isOwnerReturn {
        #[allow(missing_docs)]
        pub _0: bool,
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
            impl ::core::convert::From<isOwnerCall> for UnderlyingRustTuple<'_> {
                fn from(value: isOwnerCall) -> Self {
                    (value.owner,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for isOwnerCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { owner: tuple.0 }
                }
            }
        }
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (alloy_sol_types::sol_data::Bool,);
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (bool,);
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
            impl ::core::convert::From<isOwnerReturn> for UnderlyingRustTuple<'_> {
                fn from(value: isOwnerReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for isOwnerReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for isOwnerCall {
            type Parameters<'a> = (alloy_sol_types::sol_data::Address,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = bool;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Bool,);
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "isOwner(address)";
            const SELECTOR: [u8; 4] = [47u8, 84u8, 191u8, 110u8];
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
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                (<alloy_sol_types::sol_data::Bool as alloy_sol_types::SolType>::tokenize(ret),)
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence(data).map(
                    |r| {
                        let r: isOwnerReturn = r.into();
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
                    let r: isOwnerReturn = r.into();
                    r._0
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `nonce()` and selector `0xaffed0e0`.
    ```solidity
    function nonce() external view returns (uint256);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct nonceCall;
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`nonce()`](nonceCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct nonceReturn {
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
            impl ::core::convert::From<nonceCall> for UnderlyingRustTuple<'_> {
                fn from(value: nonceCall) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for nonceCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self
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
            impl ::core::convert::From<nonceReturn> for UnderlyingRustTuple<'_> {
                fn from(value: nonceReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for nonceReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for nonceCall {
            type Parameters<'a> = ();
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::primitives::aliases::U256;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Uint<256>,);
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "nonce()";
            const SELECTOR: [u8; 4] = [175u8, 254u8, 208u8, 224u8];
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
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        ret,
                    ),
                )
            }
            #[inline]
            fn abi_decode_returns(data: &[u8]) -> alloy_sol_types::Result<Self::Return> {
                <Self::ReturnTuple<'_> as alloy_sol_types::SolType>::abi_decode_sequence(data).map(
                    |r| {
                        let r: nonceReturn = r.into();
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
                    let r: nonceReturn = r.into();
                    r._0
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `removeOwner(address,address,uint256)` and selector `0xf8dc5dd9`.
    ```solidity
    function removeOwner(address prevOwner, address owner, uint256 _threshold) external;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct removeOwnerCall {
        #[allow(missing_docs)]
        pub prevOwner: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub owner: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub _threshold: alloy_sol_types::private::primitives::aliases::U256,
    }
    ///Container type for the return parameters of the [`removeOwner(address,address,uint256)`](removeOwnerCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct removeOwnerReturn {}
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
                alloy_sol_types::sol_data::Uint<256>,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
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
            impl ::core::convert::From<removeOwnerCall> for UnderlyingRustTuple<'_> {
                fn from(value: removeOwnerCall) -> Self {
                    (value.prevOwner, value.owner, value._threshold)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for removeOwnerCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        prevOwner: tuple.0,
                        owner: tuple.1,
                        _threshold: tuple.2,
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
            impl ::core::convert::From<removeOwnerReturn> for UnderlyingRustTuple<'_> {
                fn from(value: removeOwnerReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for removeOwnerReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl removeOwnerReturn {
            fn _tokenize(&self) -> <removeOwnerCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for removeOwnerCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = removeOwnerReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "removeOwner(address,address,uint256)";
            const SELECTOR: [u8; 4] = [248u8, 220u8, 93u8, 217u8];
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
                        &self.prevOwner,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.owner,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self._threshold,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                removeOwnerReturn::_tokenize(ret)
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
    /**Function with signature `requiredTxGas(address,uint256,bytes,uint8)` and selector `0xc4ca3a9c`.
    ```solidity
    function requiredTxGas(address to, uint256 value, bytes memory data, Enum.Operation operation) external returns (uint256);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct requiredTxGasCall {
        #[allow(missing_docs)]
        pub to: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub value: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub data: alloy_sol_types::private::Bytes,
        #[allow(missing_docs)]
        pub operation: <Enum::Operation as alloy_sol_types::SolType>::RustType,
    }
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`requiredTxGas(address,uint256,bytes,uint8)`](requiredTxGasCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct requiredTxGasReturn {
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
        use alloy_sol_types;
        {
            #[doc(hidden)]
            #[allow(dead_code)]
            type UnderlyingSolTuple<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Bytes,
                Enum::Operation,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Address,
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::Bytes,
                <Enum::Operation as alloy_sol_types::SolType>::RustType,
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
            impl ::core::convert::From<requiredTxGasCall> for UnderlyingRustTuple<'_> {
                fn from(value: requiredTxGasCall) -> Self {
                    (value.to, value.value, value.data, value.operation)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for requiredTxGasCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        to: tuple.0,
                        value: tuple.1,
                        data: tuple.2,
                        operation: tuple.3,
                    }
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
            impl ::core::convert::From<requiredTxGasReturn> for UnderlyingRustTuple<'_> {
                fn from(value: requiredTxGasReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for requiredTxGasReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for requiredTxGasCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Bytes,
                Enum::Operation,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::primitives::aliases::U256;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Uint<256>,);
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "requiredTxGas(address,uint256,bytes,uint8)";
            const SELECTOR: [u8; 4] = [196u8, 202u8, 58u8, 156u8];
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
                        &self.to,
                    ),
                    <alloy_sol_types::sol_data::Uint<256> as alloy_sol_types::SolType>::tokenize(
                        &self.value,
                    ),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.data,
                    ),
                    <Enum::Operation as alloy_sol_types::SolType>::tokenize(&self.operation),
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
                        let r: requiredTxGasReturn = r.into();
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
                    let r: requiredTxGasReturn = r.into();
                    r._0
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `setFallbackHandler(address)` and selector `0xf08a0323`.
    ```solidity
    function setFallbackHandler(address handler) external;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct setFallbackHandlerCall {
        #[allow(missing_docs)]
        pub handler: alloy_sol_types::private::Address,
    }
    ///Container type for the return parameters of the [`setFallbackHandler(address)`](setFallbackHandlerCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct setFallbackHandlerReturn {}
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
            impl ::core::convert::From<setFallbackHandlerCall> for UnderlyingRustTuple<'_> {
                fn from(value: setFallbackHandlerCall) -> Self {
                    (value.handler,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for setFallbackHandlerCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { handler: tuple.0 }
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
            impl ::core::convert::From<setFallbackHandlerReturn> for UnderlyingRustTuple<'_> {
                fn from(value: setFallbackHandlerReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for setFallbackHandlerReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl setFallbackHandlerReturn {
            fn _tokenize(
                &self,
            ) -> <setFallbackHandlerCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for setFallbackHandlerCall {
            type Parameters<'a> = (alloy_sol_types::sol_data::Address,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = setFallbackHandlerReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "setFallbackHandler(address)";
            const SELECTOR: [u8; 4] = [240u8, 138u8, 3u8, 35u8];
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
                        &self.handler,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                setFallbackHandlerReturn::_tokenize(ret)
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
    /**Function with signature `setGuard(address)` and selector `0xe19a9dd9`.
    ```solidity
    function setGuard(address guard) external;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct setGuardCall {
        #[allow(missing_docs)]
        pub guard: alloy_sol_types::private::Address,
    }
    ///Container type for the return parameters of the [`setGuard(address)`](setGuardCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct setGuardReturn {}
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
            impl ::core::convert::From<setGuardCall> for UnderlyingRustTuple<'_> {
                fn from(value: setGuardCall) -> Self {
                    (value.guard,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for setGuardCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { guard: tuple.0 }
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
            impl ::core::convert::From<setGuardReturn> for UnderlyingRustTuple<'_> {
                fn from(value: setGuardReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for setGuardReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl setGuardReturn {
            fn _tokenize(&self) -> <setGuardCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for setGuardCall {
            type Parameters<'a> = (alloy_sol_types::sol_data::Address,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = setGuardReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "setGuard(address)";
            const SELECTOR: [u8; 4] = [225u8, 154u8, 157u8, 217u8];
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
                        &self.guard,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                setGuardReturn::_tokenize(ret)
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
    /**Function with signature `setup(address[],uint256,address,bytes,address,address,uint256,address)` and selector `0xb63e800d`.
    ```solidity
    function setup(address[] memory _owners, uint256 _threshold, address to, bytes memory data, address fallbackHandler, address paymentToken, uint256 payment, address paymentReceiver) external;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct setupCall {
        #[allow(missing_docs)]
        pub _owners: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
        #[allow(missing_docs)]
        pub _threshold: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub to: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub data: alloy_sol_types::private::Bytes,
        #[allow(missing_docs)]
        pub fallbackHandler: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub paymentToken: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub payment: alloy_sol_types::private::primitives::aliases::U256,
        #[allow(missing_docs)]
        pub paymentReceiver: alloy_sol_types::private::Address,
    }
    ///Container type for the return parameters of the [`setup(address[],uint256,address,bytes,address,address,uint256,address)`](setupCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct setupReturn {}
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
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Bytes,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Address,
            );
            #[doc(hidden)]
            type UnderlyingRustTuple<'a> = (
                alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
                alloy_sol_types::private::primitives::aliases::U256,
                alloy_sol_types::private::Address,
                alloy_sol_types::private::Bytes,
                alloy_sol_types::private::Address,
                alloy_sol_types::private::Address,
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
            impl ::core::convert::From<setupCall> for UnderlyingRustTuple<'_> {
                fn from(value: setupCall) -> Self {
                    (
                        value._owners,
                        value._threshold,
                        value.to,
                        value.data,
                        value.fallbackHandler,
                        value.paymentToken,
                        value.payment,
                        value.paymentReceiver,
                    )
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for setupCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        _owners: tuple.0,
                        _threshold: tuple.1,
                        to: tuple.2,
                        data: tuple.3,
                        fallbackHandler: tuple.4,
                        paymentToken: tuple.5,
                        payment: tuple.6,
                        paymentReceiver: tuple.7,
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
            impl ::core::convert::From<setupReturn> for UnderlyingRustTuple<'_> {
                fn from(value: setupReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for setupReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl setupReturn {
            fn _tokenize(&self) -> <setupCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for setupCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Bytes,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Uint<256>,
                alloy_sol_types::sol_data::Address,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = setupReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str =
                "setup(address[],uint256,address,bytes,address,address,uint256,address)";
            const SELECTOR: [u8; 4] = [182u8, 62u8, 128u8, 13u8];
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
                    > as alloy_sol_types::SolType>::tokenize(&self._owners),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self._threshold),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.to,
                    ),
                    <alloy_sol_types::sol_data::Bytes as alloy_sol_types::SolType>::tokenize(
                        &self.data,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.fallbackHandler,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.paymentToken,
                    ),
                    <alloy_sol_types::sol_data::Uint<
                        256,
                    > as alloy_sol_types::SolType>::tokenize(&self.payment),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.paymentReceiver,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                setupReturn::_tokenize(ret)
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
    /**Function with signature `signedMessages(bytes32)` and selector `0x5ae6bd37`.
    ```solidity
    function signedMessages(bytes32) external view returns (uint256);
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct signedMessagesCall(pub alloy_sol_types::private::FixedBytes<32>);
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    ///Container type for the return parameters of the [`signedMessages(bytes32)`](signedMessagesCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct signedMessagesReturn {
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
            impl ::core::convert::From<signedMessagesCall> for UnderlyingRustTuple<'_> {
                fn from(value: signedMessagesCall) -> Self {
                    (value.0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for signedMessagesCall {
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
            impl ::core::convert::From<signedMessagesReturn> for UnderlyingRustTuple<'_> {
                fn from(value: signedMessagesReturn) -> Self {
                    (value._0,)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for signedMessagesReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self { _0: tuple.0 }
                }
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for signedMessagesCall {
            type Parameters<'a> = (alloy_sol_types::sol_data::FixedBytes<32>,);
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = alloy_sol_types::private::primitives::aliases::U256;
            type ReturnTuple<'a> = (alloy_sol_types::sol_data::Uint<256>,);
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "signedMessages(bytes32)";
            const SELECTOR: [u8; 4] = [90u8, 230u8, 189u8, 55u8];
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
                        let r: signedMessagesReturn = r.into();
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
                    let r: signedMessagesReturn = r.into();
                    r._0
                })
            }
        }
    };
    #[derive(Default, Debug, PartialEq, Eq, Hash)]
    /**Function with signature `simulateAndRevert(address,bytes)` and selector `0xb4faba09`.
    ```solidity
    function simulateAndRevert(address targetContract, bytes memory calldataPayload) external;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct simulateAndRevertCall {
        #[allow(missing_docs)]
        pub targetContract: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub calldataPayload: alloy_sol_types::private::Bytes,
    }
    ///Container type for the return parameters of the [`simulateAndRevert(address,bytes)`](simulateAndRevertCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct simulateAndRevertReturn {}
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
            impl ::core::convert::From<simulateAndRevertCall> for UnderlyingRustTuple<'_> {
                fn from(value: simulateAndRevertCall) -> Self {
                    (value.targetContract, value.calldataPayload)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for simulateAndRevertCall {
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
            impl ::core::convert::From<simulateAndRevertReturn> for UnderlyingRustTuple<'_> {
                fn from(value: simulateAndRevertReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for simulateAndRevertReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl simulateAndRevertReturn {
            fn _tokenize(
                &self,
            ) -> <simulateAndRevertCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for simulateAndRevertCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Bytes,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = simulateAndRevertReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "simulateAndRevert(address,bytes)";
            const SELECTOR: [u8; 4] = [180u8, 250u8, 186u8, 9u8];
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
                simulateAndRevertReturn::_tokenize(ret)
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
    /**Function with signature `swapOwner(address,address,address)` and selector `0xe318b52b`.
    ```solidity
    function swapOwner(address prevOwner, address oldOwner, address newOwner) external;
    ```*/
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct swapOwnerCall {
        #[allow(missing_docs)]
        pub prevOwner: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub oldOwner: alloy_sol_types::private::Address,
        #[allow(missing_docs)]
        pub newOwner: alloy_sol_types::private::Address,
    }
    ///Container type for the return parameters of the [`swapOwner(address,address,address)`](swapOwnerCall) function.
    #[allow(non_camel_case_types, non_snake_case, clippy::pub_underscore_fields)]
    #[derive(Clone)]
    pub struct swapOwnerReturn {}
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
            impl ::core::convert::From<swapOwnerCall> for UnderlyingRustTuple<'_> {
                fn from(value: swapOwnerCall) -> Self {
                    (value.prevOwner, value.oldOwner, value.newOwner)
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for swapOwnerCall {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {
                        prevOwner: tuple.0,
                        oldOwner: tuple.1,
                        newOwner: tuple.2,
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
            impl ::core::convert::From<swapOwnerReturn> for UnderlyingRustTuple<'_> {
                fn from(value: swapOwnerReturn) -> Self {
                    ()
                }
            }
            #[automatically_derived]
            #[doc(hidden)]
            impl ::core::convert::From<UnderlyingRustTuple<'_>> for swapOwnerReturn {
                fn from(tuple: UnderlyingRustTuple<'_>) -> Self {
                    Self {}
                }
            }
        }
        impl swapOwnerReturn {
            fn _tokenize(&self) -> <swapOwnerCall as alloy_sol_types::SolCall>::ReturnToken<'_> {
                ()
            }
        }
        #[automatically_derived]
        impl alloy_sol_types::SolCall for swapOwnerCall {
            type Parameters<'a> = (
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
                alloy_sol_types::sol_data::Address,
            );
            type Token<'a> = <Self::Parameters<'a> as alloy_sol_types::SolType>::Token<'a>;
            type Return = swapOwnerReturn;
            type ReturnTuple<'a> = ();
            type ReturnToken<'a> = <Self::ReturnTuple<'a> as alloy_sol_types::SolType>::Token<'a>;
            const SIGNATURE: &'static str = "swapOwner(address,address,address)";
            const SELECTOR: [u8; 4] = [227u8, 24u8, 181u8, 43u8];
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
                        &self.prevOwner,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.oldOwner,
                    ),
                    <alloy_sol_types::sol_data::Address as alloy_sol_types::SolType>::tokenize(
                        &self.newOwner,
                    ),
                )
            }
            #[inline]
            fn tokenize_returns(ret: &Self::Return) -> Self::ReturnToken<'_> {
                swapOwnerReturn::_tokenize(ret)
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
    ///Container for all the [`GnosisSafe`](self) function calls.
    #[derive(Clone)]
    pub enum GnosisSafeCalls {
        #[allow(missing_docs)]
        VERSION(VERSIONCall),
        #[allow(missing_docs)]
        addOwnerWithThreshold(addOwnerWithThresholdCall),
        #[allow(missing_docs)]
        approveHash(approveHashCall),
        #[allow(missing_docs)]
        approvedHashes(approvedHashesCall),
        #[allow(missing_docs)]
        changeThreshold(changeThresholdCall),
        #[allow(missing_docs)]
        checkNSignatures(checkNSignaturesCall),
        #[allow(missing_docs)]
        checkSignatures(checkSignaturesCall),
        #[allow(missing_docs)]
        disableModule(disableModuleCall),
        #[allow(missing_docs)]
        domainSeparator(domainSeparatorCall),
        #[allow(missing_docs)]
        enableModule(enableModuleCall),
        #[allow(missing_docs)]
        encodeTransactionData(encodeTransactionDataCall),
        #[allow(missing_docs)]
        execTransaction(execTransactionCall),
        #[allow(missing_docs)]
        execTransactionFromModule(execTransactionFromModuleCall),
        #[allow(missing_docs)]
        execTransactionFromModuleReturnData(execTransactionFromModuleReturnDataCall),
        #[allow(missing_docs)]
        getChainId(getChainIdCall),
        #[allow(missing_docs)]
        getModulesPaginated(getModulesPaginatedCall),
        #[allow(missing_docs)]
        getOwners(getOwnersCall),
        #[allow(missing_docs)]
        getStorageAt(getStorageAtCall),
        #[allow(missing_docs)]
        getThreshold(getThresholdCall),
        #[allow(missing_docs)]
        getTransactionHash(getTransactionHashCall),
        #[allow(missing_docs)]
        isModuleEnabled(isModuleEnabledCall),
        #[allow(missing_docs)]
        isOwner(isOwnerCall),
        #[allow(missing_docs)]
        nonce(nonceCall),
        #[allow(missing_docs)]
        removeOwner(removeOwnerCall),
        #[allow(missing_docs)]
        requiredTxGas(requiredTxGasCall),
        #[allow(missing_docs)]
        setFallbackHandler(setFallbackHandlerCall),
        #[allow(missing_docs)]
        setGuard(setGuardCall),
        #[allow(missing_docs)]
        setup(setupCall),
        #[allow(missing_docs)]
        signedMessages(signedMessagesCall),
        #[allow(missing_docs)]
        simulateAndRevert(simulateAndRevertCall),
        #[allow(missing_docs)]
        swapOwner(swapOwnerCall),
    }
    impl GnosisSafeCalls {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the variants.
        /// No guarantees are made about the order of the selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 4usize]] = &[
            [13u8, 88u8, 47u8, 19u8],
            [18u8, 251u8, 104u8, 224u8],
            [45u8, 154u8, 213u8, 61u8],
            [47u8, 84u8, 191u8, 110u8],
            [52u8, 8u8, 228u8, 112u8],
            [70u8, 135u8, 33u8, 167u8],
            [82u8, 41u8, 7u8, 63u8],
            [86u8, 36u8, 178u8, 91u8],
            [90u8, 230u8, 189u8, 55u8],
            [97u8, 11u8, 89u8, 37u8],
            [105u8, 78u8, 128u8, 195u8],
            [106u8, 118u8, 18u8, 2u8],
            [125u8, 131u8, 41u8, 116u8],
            [147u8, 79u8, 58u8, 17u8],
            [160u8, 230u8, 126u8, 43u8],
            [175u8, 254u8, 208u8, 224u8],
            [180u8, 250u8, 186u8, 9u8],
            [182u8, 62u8, 128u8, 13u8],
            [196u8, 202u8, 58u8, 156u8],
            [204u8, 47u8, 132u8, 82u8],
            [212u8, 217u8, 189u8, 205u8],
            [216u8, 209u8, 31u8, 120u8],
            [224u8, 9u8, 207u8, 222u8],
            [225u8, 154u8, 157u8, 217u8],
            [227u8, 24u8, 181u8, 43u8],
            [231u8, 82u8, 53u8, 184u8],
            [232u8, 102u8, 55u8, 219u8],
            [240u8, 138u8, 3u8, 35u8],
            [246u8, 152u8, 218u8, 37u8],
            [248u8, 220u8, 93u8, 217u8],
            [255u8, 161u8, 173u8, 116u8],
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(addOwnerWithThreshold),
            ::core::stringify!(checkNSignatures),
            ::core::stringify!(isModuleEnabled),
            ::core::stringify!(isOwner),
            ::core::stringify!(getChainId),
            ::core::stringify!(execTransactionFromModule),
            ::core::stringify!(execTransactionFromModuleReturnData),
            ::core::stringify!(getStorageAt),
            ::core::stringify!(signedMessages),
            ::core::stringify!(enableModule),
            ::core::stringify!(changeThreshold),
            ::core::stringify!(execTransaction),
            ::core::stringify!(approvedHashes),
            ::core::stringify!(checkSignatures),
            ::core::stringify!(getOwners),
            ::core::stringify!(nonce),
            ::core::stringify!(simulateAndRevert),
            ::core::stringify!(setup),
            ::core::stringify!(requiredTxGas),
            ::core::stringify!(getModulesPaginated),
            ::core::stringify!(approveHash),
            ::core::stringify!(getTransactionHash),
            ::core::stringify!(disableModule),
            ::core::stringify!(setGuard),
            ::core::stringify!(swapOwner),
            ::core::stringify!(getThreshold),
            ::core::stringify!(encodeTransactionData),
            ::core::stringify!(setFallbackHandler),
            ::core::stringify!(domainSeparator),
            ::core::stringify!(removeOwner),
            ::core::stringify!(VERSION),
        ];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <addOwnerWithThresholdCall as alloy_sol_types::SolCall>::SIGNATURE,
            <checkNSignaturesCall as alloy_sol_types::SolCall>::SIGNATURE,
            <isModuleEnabledCall as alloy_sol_types::SolCall>::SIGNATURE,
            <isOwnerCall as alloy_sol_types::SolCall>::SIGNATURE,
            <getChainIdCall as alloy_sol_types::SolCall>::SIGNATURE,
            <execTransactionFromModuleCall as alloy_sol_types::SolCall>::SIGNATURE,
            <execTransactionFromModuleReturnDataCall as alloy_sol_types::SolCall>::SIGNATURE,
            <getStorageAtCall as alloy_sol_types::SolCall>::SIGNATURE,
            <signedMessagesCall as alloy_sol_types::SolCall>::SIGNATURE,
            <enableModuleCall as alloy_sol_types::SolCall>::SIGNATURE,
            <changeThresholdCall as alloy_sol_types::SolCall>::SIGNATURE,
            <execTransactionCall as alloy_sol_types::SolCall>::SIGNATURE,
            <approvedHashesCall as alloy_sol_types::SolCall>::SIGNATURE,
            <checkSignaturesCall as alloy_sol_types::SolCall>::SIGNATURE,
            <getOwnersCall as alloy_sol_types::SolCall>::SIGNATURE,
            <nonceCall as alloy_sol_types::SolCall>::SIGNATURE,
            <simulateAndRevertCall as alloy_sol_types::SolCall>::SIGNATURE,
            <setupCall as alloy_sol_types::SolCall>::SIGNATURE,
            <requiredTxGasCall as alloy_sol_types::SolCall>::SIGNATURE,
            <getModulesPaginatedCall as alloy_sol_types::SolCall>::SIGNATURE,
            <approveHashCall as alloy_sol_types::SolCall>::SIGNATURE,
            <getTransactionHashCall as alloy_sol_types::SolCall>::SIGNATURE,
            <disableModuleCall as alloy_sol_types::SolCall>::SIGNATURE,
            <setGuardCall as alloy_sol_types::SolCall>::SIGNATURE,
            <swapOwnerCall as alloy_sol_types::SolCall>::SIGNATURE,
            <getThresholdCall as alloy_sol_types::SolCall>::SIGNATURE,
            <encodeTransactionDataCall as alloy_sol_types::SolCall>::SIGNATURE,
            <setFallbackHandlerCall as alloy_sol_types::SolCall>::SIGNATURE,
            <domainSeparatorCall as alloy_sol_types::SolCall>::SIGNATURE,
            <removeOwnerCall as alloy_sol_types::SolCall>::SIGNATURE,
            <VERSIONCall as alloy_sol_types::SolCall>::SIGNATURE,
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
    impl alloy_sol_types::SolInterface for GnosisSafeCalls {
        const NAME: &'static str = "GnosisSafeCalls";
        const MIN_DATA_LENGTH: usize = 0usize;
        const COUNT: usize = 31usize;
        #[inline]
        fn selector(&self) -> [u8; 4] {
            match self {
                Self::VERSION(_) => <VERSIONCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::addOwnerWithThreshold(_) => {
                    <addOwnerWithThresholdCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::approveHash(_) => <approveHashCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::approvedHashes(_) => {
                    <approvedHashesCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::changeThreshold(_) => {
                    <changeThresholdCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::checkNSignatures(_) => {
                    <checkNSignaturesCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::checkSignatures(_) => {
                    <checkSignaturesCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::disableModule(_) => <disableModuleCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::domainSeparator(_) => {
                    <domainSeparatorCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::enableModule(_) => <enableModuleCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::encodeTransactionData(_) => {
                    <encodeTransactionDataCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::execTransaction(_) => {
                    <execTransactionCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::execTransactionFromModule(_) => {
                    <execTransactionFromModuleCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::execTransactionFromModuleReturnData(_) => {
                    <execTransactionFromModuleReturnDataCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::getChainId(_) => <getChainIdCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::getModulesPaginated(_) => {
                    <getModulesPaginatedCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::getOwners(_) => <getOwnersCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::getStorageAt(_) => <getStorageAtCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::getThreshold(_) => <getThresholdCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::getTransactionHash(_) => {
                    <getTransactionHashCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::isModuleEnabled(_) => {
                    <isModuleEnabledCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::isOwner(_) => <isOwnerCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::nonce(_) => <nonceCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::removeOwner(_) => <removeOwnerCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::requiredTxGas(_) => <requiredTxGasCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::setFallbackHandler(_) => {
                    <setFallbackHandlerCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::setGuard(_) => <setGuardCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::setup(_) => <setupCall as alloy_sol_types::SolCall>::SELECTOR,
                Self::signedMessages(_) => {
                    <signedMessagesCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::simulateAndRevert(_) => {
                    <simulateAndRevertCall as alloy_sol_types::SolCall>::SELECTOR
                }
                Self::swapOwner(_) => <swapOwnerCall as alloy_sol_types::SolCall>::SELECTOR,
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
            static DECODE_SHIMS: &[fn(&[u8]) -> alloy_sol_types::Result<GnosisSafeCalls>] = &[
                {
                    fn addOwnerWithThreshold(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <addOwnerWithThresholdCall as alloy_sol_types::SolCall>::abi_decode_raw(
                            data,
                        )
                        .map(GnosisSafeCalls::addOwnerWithThreshold)
                    }
                    addOwnerWithThreshold
                },
                {
                    fn checkNSignatures(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <checkNSignaturesCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GnosisSafeCalls::checkNSignatures)
                    }
                    checkNSignatures
                },
                {
                    fn isModuleEnabled(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <isModuleEnabledCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GnosisSafeCalls::isModuleEnabled)
                    }
                    isModuleEnabled
                },
                {
                    fn isOwner(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <isOwnerCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GnosisSafeCalls::isOwner)
                    }
                    isOwner
                },
                {
                    fn getChainId(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <getChainIdCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GnosisSafeCalls::getChainId)
                    }
                    getChainId
                },
                {
                    fn execTransactionFromModule(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <execTransactionFromModuleCall as alloy_sol_types::SolCall>::abi_decode_raw(
                            data,
                        )
                        .map(GnosisSafeCalls::execTransactionFromModule)
                    }
                    execTransactionFromModule
                },
                {
                    fn execTransactionFromModuleReturnData(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <execTransactionFromModuleReturnDataCall as alloy_sol_types::SolCall>::abi_decode_raw(
                                data,
                            )
                            .map(GnosisSafeCalls::execTransactionFromModuleReturnData)
                    }
                    execTransactionFromModuleReturnData
                },
                {
                    fn getStorageAt(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <getStorageAtCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GnosisSafeCalls::getStorageAt)
                    }
                    getStorageAt
                },
                {
                    fn signedMessages(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <signedMessagesCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GnosisSafeCalls::signedMessages)
                    }
                    signedMessages
                },
                {
                    fn enableModule(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <enableModuleCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GnosisSafeCalls::enableModule)
                    }
                    enableModule
                },
                {
                    fn changeThreshold(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <changeThresholdCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GnosisSafeCalls::changeThreshold)
                    }
                    changeThreshold
                },
                {
                    fn execTransaction(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <execTransactionCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GnosisSafeCalls::execTransaction)
                    }
                    execTransaction
                },
                {
                    fn approvedHashes(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <approvedHashesCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GnosisSafeCalls::approvedHashes)
                    }
                    approvedHashes
                },
                {
                    fn checkSignatures(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <checkSignaturesCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GnosisSafeCalls::checkSignatures)
                    }
                    checkSignatures
                },
                {
                    fn getOwners(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <getOwnersCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GnosisSafeCalls::getOwners)
                    }
                    getOwners
                },
                {
                    fn nonce(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <nonceCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GnosisSafeCalls::nonce)
                    }
                    nonce
                },
                {
                    fn simulateAndRevert(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <simulateAndRevertCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GnosisSafeCalls::simulateAndRevert)
                    }
                    simulateAndRevert
                },
                {
                    fn setup(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <setupCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GnosisSafeCalls::setup)
                    }
                    setup
                },
                {
                    fn requiredTxGas(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <requiredTxGasCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GnosisSafeCalls::requiredTxGas)
                    }
                    requiredTxGas
                },
                {
                    fn getModulesPaginated(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <getModulesPaginatedCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GnosisSafeCalls::getModulesPaginated)
                    }
                    getModulesPaginated
                },
                {
                    fn approveHash(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <approveHashCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GnosisSafeCalls::approveHash)
                    }
                    approveHash
                },
                {
                    fn getTransactionHash(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <getTransactionHashCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GnosisSafeCalls::getTransactionHash)
                    }
                    getTransactionHash
                },
                {
                    fn disableModule(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <disableModuleCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GnosisSafeCalls::disableModule)
                    }
                    disableModule
                },
                {
                    fn setGuard(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <setGuardCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GnosisSafeCalls::setGuard)
                    }
                    setGuard
                },
                {
                    fn swapOwner(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <swapOwnerCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GnosisSafeCalls::swapOwner)
                    }
                    swapOwner
                },
                {
                    fn getThreshold(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <getThresholdCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GnosisSafeCalls::getThreshold)
                    }
                    getThreshold
                },
                {
                    fn encodeTransactionData(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <encodeTransactionDataCall as alloy_sol_types::SolCall>::abi_decode_raw(
                            data,
                        )
                        .map(GnosisSafeCalls::encodeTransactionData)
                    }
                    encodeTransactionData
                },
                {
                    fn setFallbackHandler(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <setFallbackHandlerCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GnosisSafeCalls::setFallbackHandler)
                    }
                    setFallbackHandler
                },
                {
                    fn domainSeparator(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <domainSeparatorCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GnosisSafeCalls::domainSeparator)
                    }
                    domainSeparator
                },
                {
                    fn removeOwner(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <removeOwnerCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GnosisSafeCalls::removeOwner)
                    }
                    removeOwner
                },
                {
                    fn VERSION(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <VERSIONCall as alloy_sol_types::SolCall>::abi_decode_raw(data)
                            .map(GnosisSafeCalls::VERSION)
                    }
                    VERSION
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
                -> alloy_sol_types::Result<GnosisSafeCalls>] = &[
                {
                    fn addOwnerWithThreshold(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <addOwnerWithThresholdCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(GnosisSafeCalls::addOwnerWithThreshold)
                    }
                    addOwnerWithThreshold
                },
                {
                    fn checkNSignatures(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <checkNSignaturesCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(GnosisSafeCalls::checkNSignatures)
                    }
                    checkNSignatures
                },
                {
                    fn isModuleEnabled(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <isModuleEnabledCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(GnosisSafeCalls::isModuleEnabled)
                    }
                    isModuleEnabled
                },
                {
                    fn isOwner(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <isOwnerCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(GnosisSafeCalls::isOwner)
                    }
                    isOwner
                },
                {
                    fn getChainId(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <getChainIdCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(GnosisSafeCalls::getChainId)
                    }
                    getChainId
                },
                {
                    fn execTransactionFromModule(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <execTransactionFromModuleCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(GnosisSafeCalls::execTransactionFromModule)
                    }
                    execTransactionFromModule
                },
                {
                    fn execTransactionFromModuleReturnData(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <execTransactionFromModuleReturnDataCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(GnosisSafeCalls::execTransactionFromModuleReturnData)
                    }
                    execTransactionFromModuleReturnData
                },
                {
                    fn getStorageAt(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <getStorageAtCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(GnosisSafeCalls::getStorageAt)
                    }
                    getStorageAt
                },
                {
                    fn signedMessages(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <signedMessagesCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(GnosisSafeCalls::signedMessages)
                    }
                    signedMessages
                },
                {
                    fn enableModule(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <enableModuleCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(GnosisSafeCalls::enableModule)
                    }
                    enableModule
                },
                {
                    fn changeThreshold(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <changeThresholdCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(GnosisSafeCalls::changeThreshold)
                    }
                    changeThreshold
                },
                {
                    fn execTransaction(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <execTransactionCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(GnosisSafeCalls::execTransaction)
                    }
                    execTransaction
                },
                {
                    fn approvedHashes(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <approvedHashesCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(GnosisSafeCalls::approvedHashes)
                    }
                    approvedHashes
                },
                {
                    fn checkSignatures(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <checkSignaturesCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(GnosisSafeCalls::checkSignatures)
                    }
                    checkSignatures
                },
                {
                    fn getOwners(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <getOwnersCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(GnosisSafeCalls::getOwners)
                    }
                    getOwners
                },
                {
                    fn nonce(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <nonceCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(GnosisSafeCalls::nonce)
                    }
                    nonce
                },
                {
                    fn simulateAndRevert(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <simulateAndRevertCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(GnosisSafeCalls::simulateAndRevert)
                    }
                    simulateAndRevert
                },
                {
                    fn setup(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <setupCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(GnosisSafeCalls::setup)
                    }
                    setup
                },
                {
                    fn requiredTxGas(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <requiredTxGasCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(GnosisSafeCalls::requiredTxGas)
                    }
                    requiredTxGas
                },
                {
                    fn getModulesPaginated(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <getModulesPaginatedCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(GnosisSafeCalls::getModulesPaginated)
                    }
                    getModulesPaginated
                },
                {
                    fn approveHash(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <approveHashCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(GnosisSafeCalls::approveHash)
                    }
                    approveHash
                },
                {
                    fn getTransactionHash(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <getTransactionHashCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(GnosisSafeCalls::getTransactionHash)
                    }
                    getTransactionHash
                },
                {
                    fn disableModule(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <disableModuleCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(GnosisSafeCalls::disableModule)
                    }
                    disableModule
                },
                {
                    fn setGuard(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <setGuardCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(GnosisSafeCalls::setGuard)
                    }
                    setGuard
                },
                {
                    fn swapOwner(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <swapOwnerCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(GnosisSafeCalls::swapOwner)
                    }
                    swapOwner
                },
                {
                    fn getThreshold(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <getThresholdCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(GnosisSafeCalls::getThreshold)
                    }
                    getThreshold
                },
                {
                    fn encodeTransactionData(
                        data: &[u8],
                    ) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <encodeTransactionDataCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(GnosisSafeCalls::encodeTransactionData)
                    }
                    encodeTransactionData
                },
                {
                    fn setFallbackHandler(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <setFallbackHandlerCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                                data,
                            )
                            .map(GnosisSafeCalls::setFallbackHandler)
                    }
                    setFallbackHandler
                },
                {
                    fn domainSeparator(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <domainSeparatorCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(
                            data,
                        )
                        .map(GnosisSafeCalls::domainSeparator)
                    }
                    domainSeparator
                },
                {
                    fn removeOwner(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <removeOwnerCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(GnosisSafeCalls::removeOwner)
                    }
                    removeOwner
                },
                {
                    fn VERSION(data: &[u8]) -> alloy_sol_types::Result<GnosisSafeCalls> {
                        <VERSIONCall as alloy_sol_types::SolCall>::abi_decode_raw_validate(data)
                            .map(GnosisSafeCalls::VERSION)
                    }
                    VERSION
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
                Self::VERSION(inner) => {
                    <VERSIONCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::addOwnerWithThreshold(inner) => {
                    <addOwnerWithThresholdCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::approveHash(inner) => {
                    <approveHashCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::approvedHashes(inner) => {
                    <approvedHashesCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::changeThreshold(inner) => {
                    <changeThresholdCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::checkNSignatures(inner) => {
                    <checkNSignaturesCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::checkSignatures(inner) => {
                    <checkSignaturesCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::disableModule(inner) => {
                    <disableModuleCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::domainSeparator(inner) => {
                    <domainSeparatorCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::enableModule(inner) => {
                    <enableModuleCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::encodeTransactionData(inner) => {
                    <encodeTransactionDataCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::execTransaction(inner) => {
                    <execTransactionCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::execTransactionFromModule(inner) => {
                    <execTransactionFromModuleCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::execTransactionFromModuleReturnData(inner) => {
                    <execTransactionFromModuleReturnDataCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::getChainId(inner) => {
                    <getChainIdCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::getModulesPaginated(inner) => {
                    <getModulesPaginatedCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::getOwners(inner) => {
                    <getOwnersCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::getStorageAt(inner) => {
                    <getStorageAtCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::getThreshold(inner) => {
                    <getThresholdCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::getTransactionHash(inner) => {
                    <getTransactionHashCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::isModuleEnabled(inner) => {
                    <isModuleEnabledCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::isOwner(inner) => {
                    <isOwnerCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::nonce(inner) => {
                    <nonceCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::removeOwner(inner) => {
                    <removeOwnerCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::requiredTxGas(inner) => {
                    <requiredTxGasCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::setFallbackHandler(inner) => {
                    <setFallbackHandlerCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::setGuard(inner) => {
                    <setGuardCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::setup(inner) => {
                    <setupCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
                Self::signedMessages(inner) => {
                    <signedMessagesCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::simulateAndRevert(inner) => {
                    <simulateAndRevertCall as alloy_sol_types::SolCall>::abi_encoded_size(
                        inner,
                    )
                }
                Self::swapOwner(inner) => {
                    <swapOwnerCall as alloy_sol_types::SolCall>::abi_encoded_size(inner)
                }
            }
        }
        #[inline]
        fn abi_encode_raw(&self, out: &mut alloy_sol_types::private::Vec<u8>) {
            match self {
                Self::VERSION(inner) => {
                    <VERSIONCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::addOwnerWithThreshold(inner) => {
                    <addOwnerWithThresholdCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::approveHash(inner) => {
                    <approveHashCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::approvedHashes(inner) => {
                    <approvedHashesCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::changeThreshold(inner) => {
                    <changeThresholdCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::checkNSignatures(inner) => {
                    <checkNSignaturesCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::checkSignatures(inner) => {
                    <checkSignaturesCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::disableModule(inner) => {
                    <disableModuleCall as alloy_sol_types::SolCall>::abi_encode_raw(
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
                Self::enableModule(inner) => {
                    <enableModuleCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::encodeTransactionData(inner) => {
                    <encodeTransactionDataCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::execTransaction(inner) => {
                    <execTransactionCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::execTransactionFromModule(inner) => {
                    <execTransactionFromModuleCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::execTransactionFromModuleReturnData(inner) => {
                    <execTransactionFromModuleReturnDataCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::getChainId(inner) => {
                    <getChainIdCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::getModulesPaginated(inner) => {
                    <getModulesPaginatedCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::getOwners(inner) => {
                    <getOwnersCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::getStorageAt(inner) => {
                    <getStorageAtCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::getThreshold(inner) => {
                    <getThresholdCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::getTransactionHash(inner) => {
                    <getTransactionHashCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::isModuleEnabled(inner) => {
                    <isModuleEnabledCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::isOwner(inner) => {
                    <isOwnerCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::nonce(inner) => {
                    <nonceCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::removeOwner(inner) => {
                    <removeOwnerCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::requiredTxGas(inner) => {
                    <requiredTxGasCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::setFallbackHandler(inner) => {
                    <setFallbackHandlerCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::setGuard(inner) => {
                    <setGuardCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::setup(inner) => {
                    <setupCall as alloy_sol_types::SolCall>::abi_encode_raw(inner, out)
                }
                Self::signedMessages(inner) => {
                    <signedMessagesCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::simulateAndRevert(inner) => {
                    <simulateAndRevertCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
                Self::swapOwner(inner) => {
                    <swapOwnerCall as alloy_sol_types::SolCall>::abi_encode_raw(
                        inner,
                        out,
                    )
                }
            }
        }
    }
    ///Container for all the [`GnosisSafe`](self) events.
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    pub enum GnosisSafeEvents {
        #[allow(missing_docs)]
        AddedOwner(AddedOwner),
        #[allow(missing_docs)]
        ApproveHash(ApproveHash),
        #[allow(missing_docs)]
        ChangedFallbackHandler(ChangedFallbackHandler),
        #[allow(missing_docs)]
        ChangedGuard(ChangedGuard),
        #[allow(missing_docs)]
        ChangedThreshold(ChangedThreshold),
        #[allow(missing_docs)]
        DisabledModule(DisabledModule),
        #[allow(missing_docs)]
        EnabledModule(EnabledModule),
        #[allow(missing_docs)]
        ExecutionFailure(ExecutionFailure),
        #[allow(missing_docs)]
        ExecutionFromModuleFailure(ExecutionFromModuleFailure),
        #[allow(missing_docs)]
        ExecutionFromModuleSuccess(ExecutionFromModuleSuccess),
        #[allow(missing_docs)]
        ExecutionSuccess(ExecutionSuccess),
        #[allow(missing_docs)]
        RemovedOwner(RemovedOwner),
        #[allow(missing_docs)]
        SafeReceived(SafeReceived),
        #[allow(missing_docs)]
        SafeSetup(SafeSetup),
        #[allow(missing_docs)]
        SignMsg(SignMsg),
    }
    impl GnosisSafeEvents {
        /// All the selectors of this enum.
        ///
        /// Note that the selectors might not be in the same order as the variants.
        /// No guarantees are made about the order of the selectors.
        ///
        /// Prefer using `SolInterface` methods instead.
        pub const SELECTORS: &'static [[u8; 32usize]] = &[
            [
                17u8, 81u8, 17u8, 105u8, 20u8, 81u8, 91u8, 192u8, 137u8, 31u8, 249u8, 4u8, 122u8,
                108u8, 179u8, 44u8, 249u8, 2u8, 84u8, 111u8, 131u8, 6u8, 100u8, 153u8, 188u8,
                248u8, 186u8, 51u8, 210u8, 53u8, 63u8, 162u8,
            ],
            [
                20u8, 29u8, 248u8, 104u8, 166u8, 51u8, 26u8, 245u8, 40u8, 227u8, 140u8, 131u8,
                183u8, 170u8, 3u8, 237u8, 193u8, 155u8, 230u8, 110u8, 55u8, 174u8, 103u8, 249u8,
                40u8, 91u8, 244u8, 248u8, 227u8, 198u8, 161u8, 168u8,
            ],
            [
                35u8, 66u8, 139u8, 24u8, 172u8, 251u8, 62u8, 166u8, 75u8, 8u8, 220u8, 12u8, 29u8,
                41u8, 110u8, 169u8, 192u8, 151u8, 2u8, 192u8, 144u8, 131u8, 202u8, 82u8, 114u8,
                230u8, 77u8, 17u8, 91u8, 104u8, 125u8, 35u8,
            ],
            [
                61u8, 12u8, 233u8, 191u8, 195u8, 237u8, 125u8, 104u8, 98u8, 219u8, 178u8, 139u8,
                45u8, 234u8, 148u8, 86u8, 31u8, 231u8, 20u8, 161u8, 180u8, 208u8, 25u8, 170u8,
                138u8, 243u8, 151u8, 48u8, 209u8, 173u8, 124u8, 61u8,
            ],
            [
                68u8, 46u8, 113u8, 95u8, 98u8, 99u8, 70u8, 232u8, 197u8, 67u8, 129u8, 0u8, 45u8,
                166u8, 20u8, 246u8, 43u8, 238u8, 141u8, 39u8, 56u8, 101u8, 53u8, 178u8, 82u8, 30u8,
                200u8, 84u8, 8u8, 152u8, 85u8, 110u8,
            ],
            [
                90u8, 198u8, 196u8, 108u8, 147u8, 200u8, 208u8, 229u8, 55u8, 20u8, 186u8, 59u8,
                83u8, 219u8, 62u8, 124u8, 4u8, 109u8, 169u8, 148u8, 49u8, 61u8, 126u8, 208u8,
                209u8, 146u8, 2u8, 139u8, 199u8, 194u8, 40u8, 176u8,
            ],
            [
                97u8, 15u8, 127u8, 242u8, 179u8, 4u8, 174u8, 137u8, 3u8, 195u8, 222u8, 116u8,
                198u8, 12u8, 106u8, 177u8, 247u8, 214u8, 34u8, 107u8, 63u8, 82u8, 197u8, 22u8,
                25u8, 5u8, 187u8, 90u8, 212u8, 3u8, 156u8, 147u8,
            ],
            [
                104u8, 149u8, 193u8, 54u8, 100u8, 170u8, 79u8, 103u8, 40u8, 139u8, 37u8, 215u8,
                162u8, 29u8, 122u8, 170u8, 52u8, 145u8, 110u8, 53u8, 95u8, 185u8, 182u8, 250u8,
                224u8, 161u8, 57u8, 169u8, 8u8, 91u8, 236u8, 184u8,
            ],
            [
                148u8, 101u8, 250u8, 12u8, 150u8, 44u8, 199u8, 105u8, 88u8, 230u8, 55u8, 58u8,
                153u8, 51u8, 38u8, 64u8, 12u8, 28u8, 148u8, 248u8, 190u8, 47u8, 227u8, 169u8, 82u8,
                173u8, 250u8, 127u8, 96u8, 178u8, 234u8, 38u8,
            ],
            [
                170u8, 180u8, 250u8, 43u8, 70u8, 63u8, 88u8, 27u8, 43u8, 50u8, 203u8, 59u8, 126u8,
                59u8, 112u8, 75u8, 156u8, 227u8, 124u8, 194u8, 9u8, 181u8, 251u8, 77u8, 119u8,
                229u8, 147u8, 172u8, 228u8, 5u8, 66u8, 118u8,
            ],
            [
                172u8, 210u8, 200u8, 112u8, 40u8, 4u8, 18u8, 143u8, 219u8, 13u8, 178u8, 187u8,
                73u8, 246u8, 209u8, 39u8, 221u8, 1u8, 129u8, 193u8, 63u8, 212u8, 93u8, 191u8,
                225u8, 109u8, 224u8, 147u8, 14u8, 43u8, 211u8, 117u8,
            ],
            [
                231u8, 244u8, 103u8, 80u8, 56u8, 244u8, 246u8, 3u8, 77u8, 252u8, 187u8, 178u8,
                76u8, 77u8, 192u8, 142u8, 78u8, 191u8, 16u8, 235u8, 157u8, 37u8, 125u8, 61u8, 2u8,
                192u8, 243u8, 141u8, 18u8, 42u8, 198u8, 228u8,
            ],
            [
                236u8, 223u8, 58u8, 62u8, 255u8, 234u8, 87u8, 131u8, 163u8, 196u8, 194u8, 20u8,
                14u8, 103u8, 117u8, 119u8, 102u8, 100u8, 40u8, 212u8, 78u8, 217u8, 212u8, 116u8,
                160u8, 179u8, 164u8, 201u8, 148u8, 63u8, 132u8, 64u8,
            ],
            [
                242u8, 160u8, 235u8, 21u8, 100u8, 114u8, 209u8, 68u8, 2u8, 85u8, 176u8, 215u8,
                193u8, 225u8, 156u8, 192u8, 113u8, 21u8, 209u8, 5u8, 31u8, 230u8, 5u8, 176u8,
                220u8, 230u8, 154u8, 207u8, 236u8, 136u8, 77u8, 156u8,
            ],
            [
                248u8, 212u8, 159u8, 197u8, 41u8, 129u8, 46u8, 154u8, 124u8, 92u8, 80u8, 230u8,
                156u8, 32u8, 240u8, 220u8, 204u8, 13u8, 184u8, 250u8, 149u8, 201u8, 139u8, 197u8,
                140u8, 201u8, 164u8, 241u8, 193u8, 41u8, 158u8, 175u8,
            ],
        ];
        /// The names of the variants in the same order as `SELECTORS`.
        pub const VARIANT_NAMES: &'static [&'static str] = &[
            ::core::stringify!(ChangedGuard),
            ::core::stringify!(SafeSetup),
            ::core::stringify!(ExecutionFailure),
            ::core::stringify!(SafeReceived),
            ::core::stringify!(ExecutionSuccess),
            ::core::stringify!(ChangedFallbackHandler),
            ::core::stringify!(ChangedThreshold),
            ::core::stringify!(ExecutionFromModuleSuccess),
            ::core::stringify!(AddedOwner),
            ::core::stringify!(DisabledModule),
            ::core::stringify!(ExecutionFromModuleFailure),
            ::core::stringify!(SignMsg),
            ::core::stringify!(EnabledModule),
            ::core::stringify!(ApproveHash),
            ::core::stringify!(RemovedOwner),
        ];
        /// The signatures in the same order as `SELECTORS`.
        pub const SIGNATURES: &'static [&'static str] = &[
            <ChangedGuard as alloy_sol_types::SolEvent>::SIGNATURE,
            <SafeSetup as alloy_sol_types::SolEvent>::SIGNATURE,
            <ExecutionFailure as alloy_sol_types::SolEvent>::SIGNATURE,
            <SafeReceived as alloy_sol_types::SolEvent>::SIGNATURE,
            <ExecutionSuccess as alloy_sol_types::SolEvent>::SIGNATURE,
            <ChangedFallbackHandler as alloy_sol_types::SolEvent>::SIGNATURE,
            <ChangedThreshold as alloy_sol_types::SolEvent>::SIGNATURE,
            <ExecutionFromModuleSuccess as alloy_sol_types::SolEvent>::SIGNATURE,
            <AddedOwner as alloy_sol_types::SolEvent>::SIGNATURE,
            <DisabledModule as alloy_sol_types::SolEvent>::SIGNATURE,
            <ExecutionFromModuleFailure as alloy_sol_types::SolEvent>::SIGNATURE,
            <SignMsg as alloy_sol_types::SolEvent>::SIGNATURE,
            <EnabledModule as alloy_sol_types::SolEvent>::SIGNATURE,
            <ApproveHash as alloy_sol_types::SolEvent>::SIGNATURE,
            <RemovedOwner as alloy_sol_types::SolEvent>::SIGNATURE,
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
    impl alloy_sol_types::SolEventInterface for GnosisSafeEvents {
        const NAME: &'static str = "GnosisSafeEvents";
        const COUNT: usize = 15usize;
        fn decode_raw_log(
            topics: &[alloy_sol_types::Word],
            data: &[u8],
        ) -> alloy_sol_types::Result<Self> {
            match topics.first().copied() {
                Some(<AddedOwner as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <AddedOwner as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::AddedOwner)
                }
                Some(<ApproveHash as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <ApproveHash as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::ApproveHash)
                }
                Some(<ChangedFallbackHandler as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <ChangedFallbackHandler as alloy_sol_types::SolEvent>::decode_raw_log(
                        topics, data,
                    )
                    .map(Self::ChangedFallbackHandler)
                }
                Some(<ChangedGuard as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <ChangedGuard as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::ChangedGuard)
                }
                Some(<ChangedThreshold as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <ChangedThreshold as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::ChangedThreshold)
                }
                Some(<DisabledModule as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <DisabledModule as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::DisabledModule)
                }
                Some(<EnabledModule as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <EnabledModule as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::EnabledModule)
                }
                Some(<ExecutionFailure as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <ExecutionFailure as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::ExecutionFailure)
                }
                Some(<ExecutionFromModuleFailure as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <ExecutionFromModuleFailure as alloy_sol_types::SolEvent>::decode_raw_log(
                        topics, data,
                    )
                    .map(Self::ExecutionFromModuleFailure)
                }
                Some(<ExecutionFromModuleSuccess as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <ExecutionFromModuleSuccess as alloy_sol_types::SolEvent>::decode_raw_log(
                        topics, data,
                    )
                    .map(Self::ExecutionFromModuleSuccess)
                }
                Some(<ExecutionSuccess as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <ExecutionSuccess as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::ExecutionSuccess)
                }
                Some(<RemovedOwner as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <RemovedOwner as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::RemovedOwner)
                }
                Some(<SafeReceived as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <SafeReceived as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::SafeReceived)
                }
                Some(<SafeSetup as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <SafeSetup as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::SafeSetup)
                }
                Some(<SignMsg as alloy_sol_types::SolEvent>::SIGNATURE_HASH) => {
                    <SignMsg as alloy_sol_types::SolEvent>::decode_raw_log(topics, data)
                        .map(Self::SignMsg)
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
    impl alloy_sol_types::private::IntoLogData for GnosisSafeEvents {
        fn to_log_data(&self) -> alloy_sol_types::private::LogData {
            match self {
                Self::AddedOwner(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::ApproveHash(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::ChangedFallbackHandler(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::ChangedGuard(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::ChangedThreshold(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::DisabledModule(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::EnabledModule(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::ExecutionFailure(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::ExecutionFromModuleFailure(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::ExecutionFromModuleSuccess(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::ExecutionSuccess(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::RemovedOwner(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::SafeReceived(inner) => {
                    alloy_sol_types::private::IntoLogData::to_log_data(inner)
                }
                Self::SafeSetup(inner) => alloy_sol_types::private::IntoLogData::to_log_data(inner),
                Self::SignMsg(inner) => alloy_sol_types::private::IntoLogData::to_log_data(inner),
            }
        }
        fn into_log_data(self) -> alloy_sol_types::private::LogData {
            match self {
                Self::AddedOwner(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::ApproveHash(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::ChangedFallbackHandler(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::ChangedGuard(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::ChangedThreshold(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::DisabledModule(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::EnabledModule(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::ExecutionFailure(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::ExecutionFromModuleFailure(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::ExecutionFromModuleSuccess(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::ExecutionSuccess(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::RemovedOwner(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::SafeReceived(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::SafeSetup(inner) => {
                    alloy_sol_types::private::IntoLogData::into_log_data(inner)
                }
                Self::SignMsg(inner) => alloy_sol_types::private::IntoLogData::into_log_data(inner),
            }
        }
    }
    use alloy_contract;
    /**Creates a new wrapper around an on-chain [`GnosisSafe`](self) contract instance.

    See the [wrapper's documentation](`GnosisSafeInstance`) for more details.*/
    #[inline]
    pub const fn new<
        P: alloy_contract::private::Provider<N>,
        N: alloy_contract::private::Network,
    >(
        address: alloy_sol_types::private::Address,
        __provider: P,
    ) -> GnosisSafeInstance<P, N> {
        GnosisSafeInstance::<P, N>::new(address, __provider)
    }
    /**Deploys this contract using the given `provider` and constructor arguments, if any.

    Returns a new instance of the contract, if the deployment was successful.

    For more fine-grained control over the deployment process, use [`deploy_builder`] instead.*/
    #[inline]
    pub fn deploy<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>(
        __provider: P,
    ) -> impl ::core::future::Future<Output = alloy_contract::Result<GnosisSafeInstance<P, N>>>
    {
        GnosisSafeInstance::<P, N>::deploy(__provider)
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
    ) -> alloy_contract::RawCallBuilder<P, N> {
        GnosisSafeInstance::<P, N>::deploy_builder(__provider)
    }
    /**A [`GnosisSafe`](self) instance.

    Contains type-safe methods for interacting with an on-chain instance of the
    [`GnosisSafe`](self) contract located at a given `address`, using a given
    provider `P`.

    If the contract bytecode is available (see the [`sol!`](alloy_sol_types::sol!)
    documentation on how to provide it), the `deploy` and `deploy_builder` methods can
    be used to deploy a new instance of the contract.

    See the [module-level documentation](self) for all the available methods.*/
    #[derive(Clone)]
    pub struct GnosisSafeInstance<P, N = alloy_contract::private::Ethereum> {
        address: alloy_sol_types::private::Address,
        provider: P,
        _network: ::core::marker::PhantomData<N>,
    }
    #[automatically_derived]
    impl<P, N> ::core::fmt::Debug for GnosisSafeInstance<P, N> {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            f.debug_tuple("GnosisSafeInstance")
                .field(&self.address)
                .finish()
        }
    }
    /// Instantiation and getters/setters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        GnosisSafeInstance<P, N>
    {
        /**Creates a new wrapper around an on-chain [`GnosisSafe`](self) contract instance.

        See the [wrapper's documentation](`GnosisSafeInstance`) for more details.*/
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
        pub async fn deploy(__provider: P) -> alloy_contract::Result<GnosisSafeInstance<P, N>> {
            let call_builder = Self::deploy_builder(__provider);
            let contract_address = call_builder.deploy().await?;
            Ok(Self::new(contract_address, call_builder.provider))
        }
        /**Creates a `RawCallBuilder` for deploying this contract using the given `provider`
        and constructor arguments, if any.

        This is a simple wrapper around creating a `RawCallBuilder` with the data set to
        the bytecode concatenated with the constructor's ABI-encoded arguments.*/
        #[inline]
        pub fn deploy_builder(__provider: P) -> alloy_contract::RawCallBuilder<P, N> {
            alloy_contract::RawCallBuilder::new_raw_deploy(
                __provider,
                ::core::clone::Clone::clone(&BYTECODE),
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
    impl<P: ::core::clone::Clone, N> GnosisSafeInstance<&P, N> {
        /// Clones the provider and returns a new instance with the cloned provider.
        #[inline]
        pub fn with_cloned_provider(self) -> GnosisSafeInstance<P, N> {
            GnosisSafeInstance {
                address: self.address,
                provider: ::core::clone::Clone::clone(&self.provider),
                _network: ::core::marker::PhantomData,
            }
        }
    }
    /// Function calls.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        GnosisSafeInstance<P, N>
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
        ///Creates a new call builder for the [`VERSION`] function.
        pub fn VERSION(&self) -> alloy_contract::SolCallBuilder<&P, VERSIONCall, N> {
            self.call_builder(&VERSIONCall)
        }
        ///Creates a new call builder for the [`addOwnerWithThreshold`] function.
        pub fn addOwnerWithThreshold(
            &self,
            owner: alloy_sol_types::private::Address,
            _threshold: alloy_sol_types::private::primitives::aliases::U256,
        ) -> alloy_contract::SolCallBuilder<&P, addOwnerWithThresholdCall, N> {
            self.call_builder(&addOwnerWithThresholdCall { owner, _threshold })
        }
        ///Creates a new call builder for the [`approveHash`] function.
        pub fn approveHash(
            &self,
            hashToApprove: alloy_sol_types::private::FixedBytes<32>,
        ) -> alloy_contract::SolCallBuilder<&P, approveHashCall, N> {
            self.call_builder(&approveHashCall { hashToApprove })
        }
        ///Creates a new call builder for the [`approvedHashes`] function.
        pub fn approvedHashes(
            &self,
            _0: alloy_sol_types::private::Address,
            _1: alloy_sol_types::private::FixedBytes<32>,
        ) -> alloy_contract::SolCallBuilder<&P, approvedHashesCall, N> {
            self.call_builder(&approvedHashesCall { _0, _1 })
        }
        ///Creates a new call builder for the [`changeThreshold`] function.
        pub fn changeThreshold(
            &self,
            _threshold: alloy_sol_types::private::primitives::aliases::U256,
        ) -> alloy_contract::SolCallBuilder<&P, changeThresholdCall, N> {
            self.call_builder(&changeThresholdCall { _threshold })
        }
        ///Creates a new call builder for the [`checkNSignatures`] function.
        pub fn checkNSignatures(
            &self,
            dataHash: alloy_sol_types::private::FixedBytes<32>,
            data: alloy_sol_types::private::Bytes,
            signatures: alloy_sol_types::private::Bytes,
            requiredSignatures: alloy_sol_types::private::primitives::aliases::U256,
        ) -> alloy_contract::SolCallBuilder<&P, checkNSignaturesCall, N> {
            self.call_builder(&checkNSignaturesCall {
                dataHash,
                data,
                signatures,
                requiredSignatures,
            })
        }
        ///Creates a new call builder for the [`checkSignatures`] function.
        pub fn checkSignatures(
            &self,
            dataHash: alloy_sol_types::private::FixedBytes<32>,
            data: alloy_sol_types::private::Bytes,
            signatures: alloy_sol_types::private::Bytes,
        ) -> alloy_contract::SolCallBuilder<&P, checkSignaturesCall, N> {
            self.call_builder(&checkSignaturesCall {
                dataHash,
                data,
                signatures,
            })
        }
        ///Creates a new call builder for the [`disableModule`] function.
        pub fn disableModule(
            &self,
            prevModule: alloy_sol_types::private::Address,
            module: alloy_sol_types::private::Address,
        ) -> alloy_contract::SolCallBuilder<&P, disableModuleCall, N> {
            self.call_builder(&disableModuleCall { prevModule, module })
        }
        ///Creates a new call builder for the [`domainSeparator`] function.
        pub fn domainSeparator(
            &self,
        ) -> alloy_contract::SolCallBuilder<&P, domainSeparatorCall, N> {
            self.call_builder(&domainSeparatorCall)
        }
        ///Creates a new call builder for the [`enableModule`] function.
        pub fn enableModule(
            &self,
            module: alloy_sol_types::private::Address,
        ) -> alloy_contract::SolCallBuilder<&P, enableModuleCall, N> {
            self.call_builder(&enableModuleCall { module })
        }
        ///Creates a new call builder for the [`encodeTransactionData`] function.
        pub fn encodeTransactionData(
            &self,
            to: alloy_sol_types::private::Address,
            value: alloy_sol_types::private::primitives::aliases::U256,
            data: alloy_sol_types::private::Bytes,
            operation: <Enum::Operation as alloy_sol_types::SolType>::RustType,
            safeTxGas: alloy_sol_types::private::primitives::aliases::U256,
            baseGas: alloy_sol_types::private::primitives::aliases::U256,
            gasPrice: alloy_sol_types::private::primitives::aliases::U256,
            gasToken: alloy_sol_types::private::Address,
            refundReceiver: alloy_sol_types::private::Address,
            _nonce: alloy_sol_types::private::primitives::aliases::U256,
        ) -> alloy_contract::SolCallBuilder<&P, encodeTransactionDataCall, N> {
            self.call_builder(&encodeTransactionDataCall {
                to,
                value,
                data,
                operation,
                safeTxGas,
                baseGas,
                gasPrice,
                gasToken,
                refundReceiver,
                _nonce,
            })
        }
        ///Creates a new call builder for the [`execTransaction`] function.
        pub fn execTransaction(
            &self,
            to: alloy_sol_types::private::Address,
            value: alloy_sol_types::private::primitives::aliases::U256,
            data: alloy_sol_types::private::Bytes,
            operation: <Enum::Operation as alloy_sol_types::SolType>::RustType,
            safeTxGas: alloy_sol_types::private::primitives::aliases::U256,
            baseGas: alloy_sol_types::private::primitives::aliases::U256,
            gasPrice: alloy_sol_types::private::primitives::aliases::U256,
            gasToken: alloy_sol_types::private::Address,
            refundReceiver: alloy_sol_types::private::Address,
            signatures: alloy_sol_types::private::Bytes,
        ) -> alloy_contract::SolCallBuilder<&P, execTransactionCall, N> {
            self.call_builder(&execTransactionCall {
                to,
                value,
                data,
                operation,
                safeTxGas,
                baseGas,
                gasPrice,
                gasToken,
                refundReceiver,
                signatures,
            })
        }
        ///Creates a new call builder for the [`execTransactionFromModule`] function.
        pub fn execTransactionFromModule(
            &self,
            to: alloy_sol_types::private::Address,
            value: alloy_sol_types::private::primitives::aliases::U256,
            data: alloy_sol_types::private::Bytes,
            operation: <Enum::Operation as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, execTransactionFromModuleCall, N> {
            self.call_builder(&execTransactionFromModuleCall {
                to,
                value,
                data,
                operation,
            })
        }
        ///Creates a new call builder for the [`execTransactionFromModuleReturnData`] function.
        pub fn execTransactionFromModuleReturnData(
            &self,
            to: alloy_sol_types::private::Address,
            value: alloy_sol_types::private::primitives::aliases::U256,
            data: alloy_sol_types::private::Bytes,
            operation: <Enum::Operation as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, execTransactionFromModuleReturnDataCall, N>
        {
            self.call_builder(&execTransactionFromModuleReturnDataCall {
                to,
                value,
                data,
                operation,
            })
        }
        ///Creates a new call builder for the [`getChainId`] function.
        pub fn getChainId(&self) -> alloy_contract::SolCallBuilder<&P, getChainIdCall, N> {
            self.call_builder(&getChainIdCall)
        }
        ///Creates a new call builder for the [`getModulesPaginated`] function.
        pub fn getModulesPaginated(
            &self,
            start: alloy_sol_types::private::Address,
            pageSize: alloy_sol_types::private::primitives::aliases::U256,
        ) -> alloy_contract::SolCallBuilder<&P, getModulesPaginatedCall, N> {
            self.call_builder(&getModulesPaginatedCall { start, pageSize })
        }
        ///Creates a new call builder for the [`getOwners`] function.
        pub fn getOwners(&self) -> alloy_contract::SolCallBuilder<&P, getOwnersCall, N> {
            self.call_builder(&getOwnersCall)
        }
        ///Creates a new call builder for the [`getStorageAt`] function.
        pub fn getStorageAt(
            &self,
            offset: alloy_sol_types::private::primitives::aliases::U256,
            length: alloy_sol_types::private::primitives::aliases::U256,
        ) -> alloy_contract::SolCallBuilder<&P, getStorageAtCall, N> {
            self.call_builder(&getStorageAtCall { offset, length })
        }
        ///Creates a new call builder for the [`getThreshold`] function.
        pub fn getThreshold(&self) -> alloy_contract::SolCallBuilder<&P, getThresholdCall, N> {
            self.call_builder(&getThresholdCall)
        }
        ///Creates a new call builder for the [`getTransactionHash`] function.
        pub fn getTransactionHash(
            &self,
            to: alloy_sol_types::private::Address,
            value: alloy_sol_types::private::primitives::aliases::U256,
            data: alloy_sol_types::private::Bytes,
            operation: <Enum::Operation as alloy_sol_types::SolType>::RustType,
            safeTxGas: alloy_sol_types::private::primitives::aliases::U256,
            baseGas: alloy_sol_types::private::primitives::aliases::U256,
            gasPrice: alloy_sol_types::private::primitives::aliases::U256,
            gasToken: alloy_sol_types::private::Address,
            refundReceiver: alloy_sol_types::private::Address,
            _nonce: alloy_sol_types::private::primitives::aliases::U256,
        ) -> alloy_contract::SolCallBuilder<&P, getTransactionHashCall, N> {
            self.call_builder(&getTransactionHashCall {
                to,
                value,
                data,
                operation,
                safeTxGas,
                baseGas,
                gasPrice,
                gasToken,
                refundReceiver,
                _nonce,
            })
        }
        ///Creates a new call builder for the [`isModuleEnabled`] function.
        pub fn isModuleEnabled(
            &self,
            module: alloy_sol_types::private::Address,
        ) -> alloy_contract::SolCallBuilder<&P, isModuleEnabledCall, N> {
            self.call_builder(&isModuleEnabledCall { module })
        }
        ///Creates a new call builder for the [`isOwner`] function.
        pub fn isOwner(
            &self,
            owner: alloy_sol_types::private::Address,
        ) -> alloy_contract::SolCallBuilder<&P, isOwnerCall, N> {
            self.call_builder(&isOwnerCall { owner })
        }
        ///Creates a new call builder for the [`nonce`] function.
        pub fn nonce(&self) -> alloy_contract::SolCallBuilder<&P, nonceCall, N> {
            self.call_builder(&nonceCall)
        }
        ///Creates a new call builder for the [`removeOwner`] function.
        pub fn removeOwner(
            &self,
            prevOwner: alloy_sol_types::private::Address,
            owner: alloy_sol_types::private::Address,
            _threshold: alloy_sol_types::private::primitives::aliases::U256,
        ) -> alloy_contract::SolCallBuilder<&P, removeOwnerCall, N> {
            self.call_builder(&removeOwnerCall {
                prevOwner,
                owner,
                _threshold,
            })
        }
        ///Creates a new call builder for the [`requiredTxGas`] function.
        pub fn requiredTxGas(
            &self,
            to: alloy_sol_types::private::Address,
            value: alloy_sol_types::private::primitives::aliases::U256,
            data: alloy_sol_types::private::Bytes,
            operation: <Enum::Operation as alloy_sol_types::SolType>::RustType,
        ) -> alloy_contract::SolCallBuilder<&P, requiredTxGasCall, N> {
            self.call_builder(&requiredTxGasCall {
                to,
                value,
                data,
                operation,
            })
        }
        ///Creates a new call builder for the [`setFallbackHandler`] function.
        pub fn setFallbackHandler(
            &self,
            handler: alloy_sol_types::private::Address,
        ) -> alloy_contract::SolCallBuilder<&P, setFallbackHandlerCall, N> {
            self.call_builder(&setFallbackHandlerCall { handler })
        }
        ///Creates a new call builder for the [`setGuard`] function.
        pub fn setGuard(
            &self,
            guard: alloy_sol_types::private::Address,
        ) -> alloy_contract::SolCallBuilder<&P, setGuardCall, N> {
            self.call_builder(&setGuardCall { guard })
        }
        ///Creates a new call builder for the [`setup`] function.
        pub fn setup(
            &self,
            _owners: alloy_sol_types::private::Vec<alloy_sol_types::private::Address>,
            _threshold: alloy_sol_types::private::primitives::aliases::U256,
            to: alloy_sol_types::private::Address,
            data: alloy_sol_types::private::Bytes,
            fallbackHandler: alloy_sol_types::private::Address,
            paymentToken: alloy_sol_types::private::Address,
            payment: alloy_sol_types::private::primitives::aliases::U256,
            paymentReceiver: alloy_sol_types::private::Address,
        ) -> alloy_contract::SolCallBuilder<&P, setupCall, N> {
            self.call_builder(&setupCall {
                _owners,
                _threshold,
                to,
                data,
                fallbackHandler,
                paymentToken,
                payment,
                paymentReceiver,
            })
        }
        ///Creates a new call builder for the [`signedMessages`] function.
        pub fn signedMessages(
            &self,
            _0: alloy_sol_types::private::FixedBytes<32>,
        ) -> alloy_contract::SolCallBuilder<&P, signedMessagesCall, N> {
            self.call_builder(&signedMessagesCall(_0))
        }
        ///Creates a new call builder for the [`simulateAndRevert`] function.
        pub fn simulateAndRevert(
            &self,
            targetContract: alloy_sol_types::private::Address,
            calldataPayload: alloy_sol_types::private::Bytes,
        ) -> alloy_contract::SolCallBuilder<&P, simulateAndRevertCall, N> {
            self.call_builder(&simulateAndRevertCall {
                targetContract,
                calldataPayload,
            })
        }
        ///Creates a new call builder for the [`swapOwner`] function.
        pub fn swapOwner(
            &self,
            prevOwner: alloy_sol_types::private::Address,
            oldOwner: alloy_sol_types::private::Address,
            newOwner: alloy_sol_types::private::Address,
        ) -> alloy_contract::SolCallBuilder<&P, swapOwnerCall, N> {
            self.call_builder(&swapOwnerCall {
                prevOwner,
                oldOwner,
                newOwner,
            })
        }
    }
    /// Event filters.
    impl<P: alloy_contract::private::Provider<N>, N: alloy_contract::private::Network>
        GnosisSafeInstance<P, N>
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
        ///Creates a new event filter for the [`AddedOwner`] event.
        pub fn AddedOwner_filter(&self) -> alloy_contract::Event<&P, AddedOwner, N> {
            self.event_filter::<AddedOwner>()
        }
        ///Creates a new event filter for the [`ApproveHash`] event.
        pub fn ApproveHash_filter(&self) -> alloy_contract::Event<&P, ApproveHash, N> {
            self.event_filter::<ApproveHash>()
        }
        ///Creates a new event filter for the [`ChangedFallbackHandler`] event.
        pub fn ChangedFallbackHandler_filter(
            &self,
        ) -> alloy_contract::Event<&P, ChangedFallbackHandler, N> {
            self.event_filter::<ChangedFallbackHandler>()
        }
        ///Creates a new event filter for the [`ChangedGuard`] event.
        pub fn ChangedGuard_filter(&self) -> alloy_contract::Event<&P, ChangedGuard, N> {
            self.event_filter::<ChangedGuard>()
        }
        ///Creates a new event filter for the [`ChangedThreshold`] event.
        pub fn ChangedThreshold_filter(&self) -> alloy_contract::Event<&P, ChangedThreshold, N> {
            self.event_filter::<ChangedThreshold>()
        }
        ///Creates a new event filter for the [`DisabledModule`] event.
        pub fn DisabledModule_filter(&self) -> alloy_contract::Event<&P, DisabledModule, N> {
            self.event_filter::<DisabledModule>()
        }
        ///Creates a new event filter for the [`EnabledModule`] event.
        pub fn EnabledModule_filter(&self) -> alloy_contract::Event<&P, EnabledModule, N> {
            self.event_filter::<EnabledModule>()
        }
        ///Creates a new event filter for the [`ExecutionFailure`] event.
        pub fn ExecutionFailure_filter(&self) -> alloy_contract::Event<&P, ExecutionFailure, N> {
            self.event_filter::<ExecutionFailure>()
        }
        ///Creates a new event filter for the [`ExecutionFromModuleFailure`] event.
        pub fn ExecutionFromModuleFailure_filter(
            &self,
        ) -> alloy_contract::Event<&P, ExecutionFromModuleFailure, N> {
            self.event_filter::<ExecutionFromModuleFailure>()
        }
        ///Creates a new event filter for the [`ExecutionFromModuleSuccess`] event.
        pub fn ExecutionFromModuleSuccess_filter(
            &self,
        ) -> alloy_contract::Event<&P, ExecutionFromModuleSuccess, N> {
            self.event_filter::<ExecutionFromModuleSuccess>()
        }
        ///Creates a new event filter for the [`ExecutionSuccess`] event.
        pub fn ExecutionSuccess_filter(&self) -> alloy_contract::Event<&P, ExecutionSuccess, N> {
            self.event_filter::<ExecutionSuccess>()
        }
        ///Creates a new event filter for the [`RemovedOwner`] event.
        pub fn RemovedOwner_filter(&self) -> alloy_contract::Event<&P, RemovedOwner, N> {
            self.event_filter::<RemovedOwner>()
        }
        ///Creates a new event filter for the [`SafeReceived`] event.
        pub fn SafeReceived_filter(&self) -> alloy_contract::Event<&P, SafeReceived, N> {
            self.event_filter::<SafeReceived>()
        }
        ///Creates a new event filter for the [`SafeSetup`] event.
        pub fn SafeSetup_filter(&self) -> alloy_contract::Event<&P, SafeSetup, N> {
            self.event_filter::<SafeSetup>()
        }
        ///Creates a new event filter for the [`SignMsg`] event.
        pub fn SignMsg_filter(&self) -> alloy_contract::Event<&P, SignMsg, N> {
            self.event_filter::<SignMsg>()
        }
    }
}
pub type Instance = GnosisSafe::GnosisSafeInstance<::alloy_provider::DynProvider>;
