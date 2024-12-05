DOCKER := docker
JQ     := jq

SOLC     := ethereum/solc:0.8.17
SOLFLAGS := --overwrite --abi --bin --bin-runtime --metadata-hash none --optimize --optimize-runs 1000000

TARGETDIR   := ../../../target/solidity
ARTIFACTDIR := ../artifacts

CONTRACTS := \
	AnyoneAuthenticator.sol \
	Balances.sol \
	Multicall.sol \
	Signatures.sol \
	SimulateCode.sol \
	Solver.sol \
	Spardose.sol \
	Swapper.sol \
	Trader.sol
ARTIFACTS := $(patsubst %.sol,$(ARTIFACTDIR)/%.json,$(CONTRACTS))

TEST_CONTRACTS := Counter.sol GasHog.sol
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
	@echo solc $(SOLFLAGS) -o /target $<
	@$(DOCKER) run -it --rm \
		-v "$(abspath .):/contracts" -w "/contracts" \
		-v "$(abspath $(TARGETDIR)):/target" \
		$(SOLC) \
		$(SOLFLAGS) -o /target $<

$(TARGETDIR)/%.abi: tests/%.sol
	@mkdir -p $(TARGETDIR)
	@echo solc $(SOLFLAGS) -o /target $(notdir $<)
	@$(DOCKER) run -it --rm \
		-v "$(abspath .)/tests:/contracts" -w "/contracts" \
		-v "$(abspath $(TARGETDIR)):/target" \
		$(SOLC) \
		$(SOLFLAGS) -o /target $(notdir $<)
