# Implement Real `encode()` Logic for CircleUbiTransitiveInteraction

## Overview
This PR implements the real `encode()` logic for `CircleUbiTransitiveInteraction` using the `transferThrough` functionality provided by the Hub contract. The implementation enables the solver to match two CRC orders by generating the appropriate calldata for transitive transfers.

## Changes
- Added `new` constructor for `CircleUbiTransitiveInteraction`
- Implemented `build_transfer_through_params` to construct arrays for transitive transfers
- Fixed `encode()` method to properly generate calldata using the Hub contract's `transferThrough` method
- Added comprehensive test coverage for the implementation

## Implementation Details
The implementation follows these key aspects:
1. **Token Path Construction**: Properly handles both direct transfers and transfers through intermediaries
2. **Parameter Generation**: Creates the required arrays (`tokenOwners`, `srcs`, `dests`, `wads`) for the `transferThrough` call
3. **Type Safety**: Ensures proper handling of Ethereum types and byte conversions

## Testing
Added tests to verify:
- Interaction creation with proper parameters
- Transfer through parameter construction
- Calldata generation for the Hub contract

## Technical Notes
- Uses the existing Hub contract ABI without modifications
- Maintains compatibility with the existing solver infrastructure
- Handles proper type conversions between `web3` and `ethcontract` types

## Next Steps
- [ ] Review test coverage
- [ ] Integration testing with actual CRC orders
- [ ] Performance testing with complex transfer paths 