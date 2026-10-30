CARGO ?= cargo
OBJCOPY ?= llvm-objcopy-20
BUILD_DIR ?= target
REFLEX = $(CARGO) run --quiet -p reflex-cli -- --target-dir "$(BUILD_DIR)" --objcopy "$(OBJCOPY)"

.PHONY: hello-world doom inspector calculator explorer browser doom-fetch browser-fetch doom-preview calculator-preview browser-preview
hello-world doom inspector calculator explorer browser:
	$(REFLEX) build $@

doom-fetch:
	ruby scripts/fetch-doom.rb "$(BUILD_DIR)/doom-inputs"

browser-fetch:
	ruby scripts/fetch-netsurf.rb "$(BUILD_DIR)/netsurf-inputs"

browser: browser-fetch

browser-preview: browser-fetch
	ruby scripts/build-netsurf.rb "$(BUILD_DIR)/netsurf-inputs" host
	ruby scripts/browser-preview.rb "$(BUILD_DIR)/netsurf-inputs" "$(BUILD_DIR)/browser-preview.ppm"

doom-preview:
	DOOM_INPUTS=$(abspath $(BUILD_DIR)/doom-inputs) $(CARGO) run -p brain-doom --example preview --features engine --release --target-dir $(BUILD_DIR) -- $(BUILD_DIR)/doom-preview.ppm

calculator-preview:
	$(CARGO) run -p brain-calculator --example preview --target-dir $(BUILD_DIR) -- $(BUILD_DIR)/calculator-preview
