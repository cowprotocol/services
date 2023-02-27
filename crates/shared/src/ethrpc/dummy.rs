pub fn web3() -> web3::Web3<contracts::web3::DummyTransport> {
    contracts::web3::dummy()
}
