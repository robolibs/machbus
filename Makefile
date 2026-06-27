SHELL := /bin/bash

PROJECT_NAME := $(shell if [ -f PROJECT ]; then sed -n '/^[[:space:]]*[^#\[[:space:]]/p' PROJECT | head -1 | tr -d '[:space:]'; else sed -n 's/^[[:space:]]*name[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' Cargo.toml | head -1; fi)
PROJECT_VERSION := $(shell if [ -f PROJECT ]; then sed -n '/^[[:space:]]*[^#\[[:space:]]/p' PROJECT | sed -n '2p' | tr -d '[:space:]'; else sed -n 's/^[[:space:]]*version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' Cargo.toml | head -1; fi)
ifeq ($(PROJECT_NAME),)
    $(error Error: PROJECT file not found or invalid)
endif

TOP_DIR := $(CURDIR)
CARGO := cargo
CBINDGEN_INPUT := $(TOP_DIR)/src/ffi.rs
GENERATE_C_ABI_PUBLIC_HEADER := bash $(TOP_DIR)/tools/generate_c_abi_public_header.sh
EXAMPLE ?= main
NO_STD_TARGET ?= thumbv7em-none-eabihf

HAS_REL := $(shell command -v git-rel 2>/dev/null)

$(info ------------------------------------------)
$(info Project: $(PROJECT_NAME) v$(PROJECT_VERSION))
$(info ------------------------------------------)

.PHONY: build b compile c run r test t check no-std-check no-std-target-check no-std-surface-check embedded-examples-check check-all test-all clippy rustdoc fmt clean bind bind-c bind-c-check bind-py c-demo c-full-demo python-demo trace-replay-demo vt-evidence-smoke fuzz-smoke wirebit-examples-check standard-suite-check semantic-split-name-check whitespace-check book verify release help h

build:
	@$(CARGO) build --lib

b: build

compile:
	@$(CARGO) clean
	@$(MAKE) build

c: compile

run:
	@$(CARGO) run --example $(EXAMPLE)

r: run

test:
	@$(CARGO) test --all-targets

t: test

check:
	@$(CARGO) check --all-targets


no-std-check:
	@$(CARGO) rustc --lib --no-default-features --features embedded --crate-type rlib

no-std-target-check:
	@rustup target list --installed | grep -qx '$(NO_STD_TARGET)' || { \
		echo "Rust target $(NO_STD_TARGET) is not installed; run: rustup target add $(NO_STD_TARGET)"; \
		exit 1; \
	}
	@RUSTC="$$(rustup which rustc)" $(CARGO) rustc --lib --no-default-features --features embedded --target $(NO_STD_TARGET) --crate-type rlib

no-std-surface-check:
	@$(CARGO) check --no-default-features --features embedded --test no_std_surface

embedded-examples-check:
	@$(CARGO) check --no-default-features --features embedded --example embedded_session_loop
	@$(CARGO) check --no-default-features --features embedded --example embedded_hal_adapter
	@$(CARGO) check --no-default-features --features embedded --example embedded_fixed_queue

check-all:
	@$(CARGO) check --all-targets --features async

fmt:
	@$(CARGO) fmt --all

clippy:
	@$(CARGO) clippy --all-targets --features async -- -D warnings

rustdoc:
	@RUSTDOCFLAGS="-Dwarnings" $(CARGO) doc --features async --no-deps

test-all:
	@$(CARGO) test --all-targets --features async

clean:
	@$(CARGO) clean

bind: bind-c bind-py

bind-c:
	@$(CARGO) build --lib
	@tmp="$$(mktemp)"; \
	RUSTC_BOOTSTRAP=1 cbindgen --config cbindgen.toml --crate $(PROJECT_NAME) --output "$$tmp"; \
	$(GENERATE_C_ABI_PUBLIC_HEADER) "$$tmp" include/$(PROJECT_NAME).h; \
	rm -f "$$tmp"

bind-c-check:
	@tmpdir="$$(mktemp -d)"; \
	tmp="$$tmpdir/generated.h"; \
	RUSTC_BOOTSTRAP=1 cbindgen --config cbindgen.toml --crate $(PROJECT_NAME) --output "$$tmp"; \
	mkdir -p "$$tmpdir/generated-header"; \
	$(GENERATE_C_ABI_PUBLIC_HEADER) "$$tmp" "$$tmpdir/generated-header/$(PROJECT_NAME).h"; \
	diff -ru include/$(PROJECT_NAME).h "$$tmpdir/generated-header/$(PROJECT_NAME).h" >/dev/null && \
	diff -ru include/$(PROJECT_NAME) "$$tmpdir/generated-header/$(PROJECT_NAME)" >/dev/null || { \
		echo "include/$(PROJECT_NAME).h is stale; run make bind-c"; \
		diff -ru include/$(PROJECT_NAME).h "$$tmpdir/generated-header/$(PROJECT_NAME).h" || true; \
		diff -ru include/$(PROJECT_NAME) "$$tmpdir/generated-header/$(PROJECT_NAME)" || true; \
		rm -rf "$$tmpdir"; \
		exit 1; \
	}; \
	rm -rf "$$tmpdir"

bind-py:
	@maturin build --features pyo3/extension-module

c-demo:
	@$(MAKE) -C examples/c_abi run

c-full-demo:
	@$(MAKE) -C examples/c_abi run-full

python-demo:
	@$(MAKE) -C examples/python_binding test

trace-replay-demo:
	@$(CARGO) run --quiet --example candump_replay -- tests/fixtures/traces/time_date_agisostack.candump
	@$(CARGO) run --quiet --example candump_replay -- tests/fixtures/traces/bracketed_time_date.candump
	@$(CARGO) run --quiet --example candump_replay -- tests/fixtures/traces/standard_id_rejection.candump
	@$(CARGO) run --quiet --example candump_replay -- tests/fixtures/traces/malformed_candump.candump

vt-evidence-smoke:
	@$(CARGO) run --quiet --example iop_inspect -- --strict --physical-soft-keys 10 --navigation-soft-keys 2 --write-report-json /tmp/machbus-iop-inspect-report.json --write-rgb888 /tmp/machbus-iop-inspect.rgb --write-rgb565-be /tmp/machbus-iop-inspect-be.rgb565 --write-rgb565-le /tmp/machbus-iop-inspect-le.rgb565 --expect-unsupported-records 0 --expect-placeholder-pixels 0 --expect-rgb888-fnv64 0x527FEA44D2914422 --expect-rgb565-be-fnv64 0xC0F7DB231D7BC71F --expect-rgb565-le-fnv64 0xA27374387E955487
	@$(CARGO) run --quiet --example vt_trace_inspect -- --strict --physical-soft-keys 10 --navigation-soft-keys 2 --write-report-json /tmp/machbus-vt-trace-report.json --write-initial-rgb888 /tmp/machbus-vt-trace-initial.rgb --write-final-rgb888 /tmp/machbus-vt-trace-final.rgb --write-initial-rgb565-be /tmp/machbus-vt-trace-initial-be.rgb565 --write-initial-rgb565-le /tmp/machbus-vt-trace-initial-le.rgb565 --write-final-rgb565-be /tmp/machbus-vt-trace-final-be.rgb565 --write-final-rgb565-le /tmp/machbus-vt-trace-final-le.rgb565 --expect-accepted-effects 3 --expect-initial-placeholder-pixels 0 --expect-final-placeholder-pixels 0 --expect-rgb888-fnv64 0xF7299A9637CEE405 --expect-rgb565-be-fnv64 0x54DC62D0E04D5405 --expect-rgb565-le-fnv64 0x54DC62D0E04D5405

fuzz-smoke:
	@$(CARGO) test --test fuzz_targets -- --nocapture

wirebit-examples-check:
	@$(CARGO) check --features wirebit --examples

standard-suite-check:
	@$(CARGO) test --test standard -- --nocapture

semantic-split-name-check:
	@bad="$$(find . \
		\( -path './.git' \
		-o -path './target' \
		-o -path './book/book' \
		-o -path './examples/python_binding/.venv' \
		-o -path './examples/python_binding/.maturin' \) -prune -o \
		\( -type d -name '*_parts' \
		-o -type d -name '*_chunks' \
		-o -type d -name '*_sections' \
		-o -type d -name '*_slices' \
		-o -type d -name '*_pieces' \
		-o -type f -name 'part_[0-9]*.*' \
		-o -type f -name 'part-[0-9]*.*' \
		-o -type f -name 'chunk_[0-9]*.*' \
		-o -type f -name 'chunk-[0-9]*.*' \
		-o -type f -name 'section_[0-9]*.*' \
		-o -type f -name 'section-[0-9]*.*' \
		-o -type f -name 'slice_[0-9]*.*' \
		-o -type f -name 'slice-[0-9]*.*' \
		-o -type f -name 'piece_[0-9]*.*' \
		-o -type f -name 'piece-[0-9]*.*' \
		-o -type f -name 'split_[0-9]*.*' \
		-o -type f -name 'split-[0-9]*.*' \
		-o -type f -name 'parts-at-a-glance.*' \) -print)"; \
	if [ -n "$$bad" ]; then \
		echo "non-semantic generated/split/doc file names found; name files after their contents instead:"; \
		printf '%s\n' "$$bad"; \
		exit 1; \
	fi
	@if git grep --untracked -n -E 'include!\("[^"]*_(parts|chunks|sections|slices|pieces)/[a-z]+_[0-9]+\.rs"\)|#include "[^"]*_(parts|chunks|sections|slices|pieces)/[a-z]+_[0-9]+\.h"' -- src tests include tools >/tmp/machbus-semantic-split-name-grep.txt; then \
		echo "non-semantic split references found:"; \
		cat /tmp/machbus-semantic-split-name-grep.txt; \
		rm -f /tmp/machbus-semantic-split-name-grep.txt; \
		exit 1; \
	fi; \
	rm -f /tmp/machbus-semantic-split-name-grep.txt
	@if git grep --untracked -n -E '\\]\([^)]*standards/(part-[0-9]+|parts-at-a-glance)[^)]*\\)|standards/(part-[0-9]+|parts-at-a-glance)\\.md' -- book/src >/tmp/machbus-semantic-doc-name-grep.txt; then \
		echo "non-semantic standard document references found:"; \
		cat /tmp/machbus-semantic-doc-name-grep.txt; \
		rm -f /tmp/machbus-semantic-doc-name-grep.txt; \
		exit 1; \
	fi; \
	rm -f /tmp/machbus-semantic-doc-name-grep.txt

whitespace-check:
	@git diff --check

book:
	@command -v mdbook >/dev/null 2>&1 || { echo "mdbook is not installed. Please install it first."; exit 1; }
	@test -f $(TOP_DIR)/book/book.toml || { echo "book/book.toml does not exist"; exit 1; }
	@mdbook build $(TOP_DIR)/book

verify: check test check-all test-all clippy rustdoc bind-c-check c-demo c-full-demo python-demo trace-replay-demo fuzz-smoke wirebit-examples-check standard-suite-check semantic-split-name-check whitespace-check

release:
	@if [ -z "$(HAS_REL)" ]; then \
		echo "git-rel is not installed. Please install it first."; \
		exit 1; \
	fi
	@if [ -z "$(TYPE)" ]; then \
		echo "Release type not specified. Use 'make release TYPE=[patch|minor|major|m.m.p]'"; \
		exit 1; \
	fi
	@git rel $(TYPE)

help:
	@echo
	@echo "Usage: make [target]"
	@echo
	@echo "Available targets:"
	@echo "  build        Build the library"
	@echo "  compile      Clean and rebuild"
	@echo "  run          Run a development example (if examples exist)"
	@echo "  test         Run all tests"
	@echo "  bind         Generate both C and Python bindings"
	@echo "  bind-c-check Verify generated C header is up to date"
	@echo "  c-demo       Build and run the basic C ABI demo"
	@echo "  c-full-demo  Build and run the full C ABI demo"
	@echo "  python-demo  Build and run the Python binding smokes plus wheel install"
	@echo "  trace-replay-demo Replay compact/bracketed/malformed candump fixtures"
	@echo "  vt-evidence-smoke Run VT object-pool and command-trace evidence smokes"
	@echo "  fuzz-smoke   Run the arbitrary-input decoder fuzz-smoke test"
	@echo "  wirebit-examples-check Typecheck SocketCAN/vcan examples"
	@echo "  standard-suite-check Run the standard-derived ISO 11783/AEF/NMEA test suite"
	@echo "  semantic-split-name-check Verify split source/header/doc names are content-based"
	@echo "  whitespace-check Verify git diff whitespace"
	@echo "  verify       Run the full local hardening gate"
	@echo "  check        Run cargo check on all targets"
	@echo "  Check std without default hosted adapter deps"
	@echo "  no-std-check Check the transitional no_std + alloc embedded surface"
	@echo "  no-std-target-check Check no_std on NO_STD_TARGET (default thumbv7em-none-eabihf)"
	@echo "  no-std-surface-check Check embedded public imports in a dedicated test"
	@echo "  embedded-examples-check Check embedded-shaped examples"
	@echo "  check-all    Run cargo check on all targets/all features"
	@echo "  test-all     Run cargo test on all targets/all features"
	@echo "  clippy       Run clippy with warnings denied"
	@echo "  rustdoc      Build docs with warnings denied"
	@echo "  fmt          Format the workspace"
	@echo "  clean        Remove Cargo build artifacts"
	@echo "  book         Build the documentation (mdBook in ./book)"
	@echo "  release      Release a new version"
	@echo
	@echo "Examples:"
	@echo "  make run"
	@echo "  make run EXAMPLE=main"
	@echo

h: help
