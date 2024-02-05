-- Populates app data table with a set of legacy (ie not well formed) but known to be legitimate app data values. 
-- We are assuming no specific metadata semantics (ie empty app data) for these values.
INSERT INTO app_data 
  (contract_app_data, full_app_data) 
VALUES
  -- Legit user 0x634b41c246f9afce16de397424704130b588139f
  ('\x1ba2c7f5680dd17a4d852b9c590afa0969893c2b1052a7f553542697f5668171', decode('{}', 'escape')),
  -- Legit use case (lots of users)
  ('\x8906d5e6f69e3d8133f70c0451990044978ad5ed54be76f6f618b6c5784526c5', decode('{}', 'escape')),
  -- Fee withdrawals
  ('\x2947be33ebfa25686ec204857135dd1c676f35d6c252eb066fffaf9b493a01b4', decode('{}', 'escape')),
  -- Example python trading script
  ('\x0000000000000000000000000000000000000000000000000000000000000ccc', decode('{}', 'escape')),
  -- Legit user 0xfcd2f5f382e4b3cd3b67a4e399ada0edf56d0383
  ('\xd19e76e4a302bc4e0018de6210f5fde3c55e3618a23a261fa94e4d19ceeb039d', decode('{}', 'escape')),
  -- Limit order tutorial
  ('\xf785fae7a7c5abc49f3cd6a61f6df1ff26433392b066ee9ff2240ff1eb7ab6e4', decode('{}', 'escape')),
  -- DCA Order (e.g. https://etherscan.io/address/0x224d04a92583936b9dd86c9ee8dd450290eded66#code)
  ('\x9b5c6dfa0fa4be89e17700f05bee8775b281aa6d2dac7dfbf3945e0f9642d777', decode('{}', 'escape'));
