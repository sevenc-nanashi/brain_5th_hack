CARGO ?= cargo
OBJCOPY ?= llvm-objcopy-20
BUILD_DIR ?= target
REFLEX = $(CARGO) run --quiet -p reflex-cli -- --target-dir "$(BUILD_DIR)" --objcopy "$(OBJCOPY)"

.PHONY: hello-world doom inspector calculator explorer doom-fetch doom-preview calculator-preview
hello-world doom inspector calculator explorer:
	$(REFLEX) build $@

doom-fetch:
	ruby scripts/fetch-doom.rb "$(BUILD_DIR)/doom-inputs"

doom-preview:
	DOOM_INPUTS=$(abspath $(BUILD_DIR)/doom-inputs) $(CARGO) run -p brain-doom --example preview --features engine --release --target-dir $(BUILD_DIR) -- $(BUILD_DIR)/doom-preview.ppm

calculator-preview:
	$(CARGO) run -p brain-calculator --example preview --target-dir $(BUILD_DIR) -- $(BUILD_DIR)/calculator-preview
