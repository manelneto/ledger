all: build

build:
	cargo build

clean:
	cargo clean

bootstrap:
	cargo run -- $(PORT) $(PORT)

run:
	cargo run -- $(PORT) $(BOOTSTRAP)

shutdown:
	cargo run --bin shutdown -- $(PORTS)
