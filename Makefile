CARGO ?= cargo
OBJCOPY ?= llvm-objcopy-20
BUILD_DIR ?= target
HELLO_DIR = $(BUILD_DIR)/hello-world/APP/HelloWorld

.PHONY: hello-world
hello-world:
	$(CARGO) rustc -p brain-hello-world --bin brain-hello-world --features device --release --target armv7a-none-eabi --target-dir $(BUILD_DIR) -- -C panic=abort -C target-cpu=cortex-a7 -C relocation-model=static -C link-arg=-Tcrates/brain-hello-world/link.x
	mkdir -p $(HELLO_DIR)
	$(OBJCOPY) -O binary $(BUILD_DIR)/armv7a-none-eabi/release/brain-hello-world $(HELLO_DIR)/AppMain.bin
	touch $(HELLO_DIR)/index.din

DOOM_DIR = $(BUILD_DIR)/doom/APP/Doom

.PHONY: doom-fetch doom doom-preview
doom-fetch:
	ruby scripts/fetch-doom.rb $(BUILD_DIR)/doom-inputs

doom:
	DOOM_INPUTS=$(abspath $(BUILD_DIR)/doom-inputs) $(CARGO) rustc -p brain-doom --bin brain-doom --features device --release --target armv7a-none-eabi --target-dir $(BUILD_DIR) -- -C panic=abort -C target-cpu=cortex-a7 -C relocation-model=static -C link-arg=-Tcrates/brain-doom/link.x
	mkdir -p $(DOOM_DIR)
	$(OBJCOPY) -O binary $(BUILD_DIR)/armv7a-none-eabi/release/brain-doom $(DOOM_DIR)/AppMain.bin
	touch $(DOOM_DIR)/index.din

doom-preview:
	DOOM_INPUTS=$(abspath $(BUILD_DIR)/doom-inputs) $(CARGO) run -p brain-doom --example preview --features engine --release --target-dir $(BUILD_DIR) -- $(BUILD_DIR)/doom-preview.ppm

INSPECTOR_DIR = $(BUILD_DIR)/inspector/APP/Inspector

.PHONY: inspector
inspector:
	$(CARGO) rustc -p brain-inspector --bin brain-inspector --features device --release --target armv7a-none-eabi --target-dir $(BUILD_DIR) -- -C panic=abort -C target-cpu=cortex-a7 -C relocation-model=static -C link-arg=-Tcrates/brain-inspector/link.x
	mkdir -p $(INSPECTOR_DIR)
	$(OBJCOPY) -O binary $(BUILD_DIR)/armv7a-none-eabi/release/brain-inspector $(INSPECTOR_DIR)/AppMain.bin
	touch $(INSPECTOR_DIR)/index.din
