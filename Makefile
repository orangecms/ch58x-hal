WCHISP := wchisp
EXAMPLE := serial

build:
	cargo build --release --example $(EXAMPLE)

bin:
	cargo objcopy --release --example $(EXAMPLE) -- -O ihex $(EXAMPLE).hex

flash: build
	$(WCHISP) flash $(EXAMPLE).hex
