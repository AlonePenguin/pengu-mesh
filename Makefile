SHELL := /bin/zsh
CARGO ?= cargo

.PHONY: fmt check test build doctor bench-discover bench-run local-gate

fmt:
	$(CARGO) fmt --all

check:
	$(CARGO) check --workspace

test:
	$(CARGO) test --workspace

build:
	$(CARGO) build --workspace

doctor:
	$(CARGO) run -p pengu-mesh-doctor -- --json

bench-discover:
	./scripts/bench/discover.sh

bench-run:
	./scripts/bench/run.sh

local-gate:
	./scripts/release/local-gate.sh
