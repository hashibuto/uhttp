RUST_IMAGE := rust:1.83-alpine3.21
PWD := $(shell pwd)
USER_ID = $(shell id -u)
GROUP_ID = $(shell id -g)
RUN_CARGO := docker run --rm -w /build -v $(PWD):/build -u $(USER_ID):$(GROUP_ID) $(RUST_IMAGE) cargo

phony: test
test:
	$(RUN_CARGO) test
