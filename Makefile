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
