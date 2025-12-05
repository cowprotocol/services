JQ     := jq

SOLC     := solc
SOLFLAGS := --overwrite --abi --bin --bin-runtime --metadata-hash none --optimize --optimize-runs 1000000 --evm-version shanghai

TARGETDIR   := ../../../target/solidity
ARTIFACTDIR := ../artifacts

CONTRACTS := \
	AnyoneAuthenticator.sol \
	Balances.sol \
	Signatures.sol \
	Solver.sol \
	Spardose.sol \
	Swapper.sol \
	Trader.sol
ARTIFACTS := $(patsubst %.sol,$(ARTIFACTDIR)/%.json,$(CONTRACTS))

TEST_CONTRACTS := Counter.sol GasHog.sol NonStandardERC20Balances.sol
TEST_ARTIFACTS := $(patsubst %.sol,$(ARTIFACTDIR)/%.json,$(TEST_CONTRACTS))

.PHONY: artifacts
artifacts: $(ARTIFACTS) $(TEST_ARTIFACTS)

define ARTIFACTTEMPLATE
{
  "abi": [],
  "bytecode": "0x",
  "deployedBytecode": "0x",
  "devdoc": {
    "methods": {}
  },
  "userdoc": {
    "methods": {}
  }
}
endef
export ARTIFACTTEMPLATE
$(ARTIFACTDIR)/%.json: $(TARGETDIR)/%.abi
	@echo jq "'...'" '>' $@
	@echo "$$ARTIFACTTEMPLATE" \
		| jq -c \
			--argjson ABI   '$(shell cat $(TARGETDIR)/$*.abi)' \
			--arg     BIN   '0x$(shell cat $(TARGETDIR)/$*.bin | head -n1)' \
			--arg     BINRT '0x$(shell cat $(TARGETDIR)/$*.bin-runtime | head -n1)' \
			'.abi=$$ABI | .bytecode=$$BIN | .deployedBytecode=$$BINRT' \
		> $@

$(TARGETDIR)/%.abi: %.sol
	@mkdir -p $(TARGETDIR)
	@echo solc $(SOLFLAGS) -o $(TARGETDIR) $<
	@$(SOLC) $(SOLFLAGS) -o $(TARGETDIR) $<

$(TARGETDIR)/%.abi: tests/%.sol
	@mkdir -p $(TARGETDIR)
	@echo solc $(SOLFLAGS) -o $(TARGETDIR) $<
	@cd tests && $(SOLC) $(SOLFLAGS) -o $(abspath $(TARGETDIR)) $(notdir $<)
