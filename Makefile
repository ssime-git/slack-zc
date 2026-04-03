.PHONY: all check-zeroclaw run test

all: run

check-zeroclaw:
	@command -v zeroclaw >/dev/null 2>&1 || { \
		echo "Error: zeroclaw is not installed or not in PATH."; \
		echo "Install/configure it first, then retry."; \
		echo "Hint: zeroclaw onboard --interactive"; \
		exit 1; \
	}
	@echo "zeroclaw found: $$(command -v zeroclaw)"

run: check-zeroclaw
	cargo run -p slack-zc

test:
	cargo test -q
