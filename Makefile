all: build

build:
	cargo build

clean:
	cargo clean

boostrap:
	cargo run --bin main -- $(PORT) $(PORT)

run:
	cargo run --bin main -- $(PORT) $(BOOTSTRAP)

shutdown:
	cargo run --bin shutdown -- $(PORTS)
