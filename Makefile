RUST_IMAGE := rust:1.83-alpine3.21
PWD := $(shell pwd)
USER_ID = $(shell id -u)
GROUP_ID = $(shell id -g)
RUN := docker run --rm -w /build -v $(PWD):/build -u $(USER_ID):$(GROUP_ID) $(RUST_IMAGE)
RUN_CARGO := $(RUN) cargo

.PHONY: test
test:
	$(RUN_CARGO) test

.PHONY: check-format
check-format:
	$(RUN) rustup component add rustfmt && cargo fmt --check
