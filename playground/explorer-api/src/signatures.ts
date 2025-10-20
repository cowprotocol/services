// Minimal offline signature registry for common functions/events used in Playground
// Extendable in future milestones or replaced by Sourcify/ABI store
export const functionSigs: Record<string, string> = {
  // ERC20
  '0xa9059cbb': 'transfer(address,uint256)',
  '0x095ea7b3': 'approve(address,uint256)',
  '0x23b872dd': 'transferFrom(address,address,uint256)',
  '0x70a08231': 'balanceOf(address)',
  '0xdd62ed3e': 'allowance(address,address)',
  '0x313ce567': 'decimals()',
  '0x06fdde03': 'name()',
  '0x95d89b41': 'symbol()',
  // ERC20 Permit
  '0xd505accf': 'permit(address,address,uint256,uint256,uint8,bytes32,bytes32)',
  // ERC721
  '0xa22cb465': 'setApprovalForAll(address,bool)',
  // '0x095ea7b3': 'approve(address,uint256)', // Duplicate - same as ERC20
  // Multicall
  '0x5ae401dc': 'multicall(uint256,bytes[])',
  '0xac9650d8': 'multicall(bytes[])',
  // UniswapV2/V3-ish
  '0x38ed1739': 'swapExactTokensForTokens(uint256,uint256,address[],address,uint256)',
  '0x18cbafe5': 'swapExactTokensForETH(uint256,uint256,address[],address,uint256)',
  '0x7ff36ab5': 'swapExactETHForTokens(uint256,address[],address,uint256)'
};

export const eventSigs: Record<string, string> = {
  // keccak256("Transfer(address,address,uint256)")
  '0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef': 'Transfer(address,address,uint256)',
  // keccak256("Approval(address,address,uint256)")
  '0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925': 'Approval(address,address,uint256)',
  // keccak256("ApprovalForAll(address,address,bool)")
  '0x17307eab39ab6107e8899845ad3d59bd9653f200f220920489ca2b5937696c31': 'ApprovalForAll(address,address,bool)'
};

