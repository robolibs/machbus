SHELL := /bin/bash

ROOT := ../..
PYTHON ?= $(PYO3_PYTHON)
ifeq ($(PYTHON),)
PYTHON := python3
endif
CARGO_BIN ?= $(MACHBUS_CARGO)
RUSTC_BIN ?= $(MACHBUS_RUSTC)
MATURIN ?= maturin
ifeq ($(CARGO_BIN),)
CARGO_BIN := cargo
endif
ifeq ($(RUSTC_BIN),)
RUSTC_BIN := rustc
endif
VENV := .venv
VENV_PYTHON := $(CURDIR)/$(VENV)/bin/python

.PHONY: all develop basic regression wheel-smoke test clean venv

all: test

venv:
	@if [ -x "$(VENV_PYTHON)" ] && ! $(VENV_PYTHON) -m pip --version >/dev/null 2>&1 && ! $(VENV_PYTHON) -m ensurepip --version >/dev/null 2>&1; then \
		echo "removing stale Python venv without pip/ensurepip"; \
		rm -rf $(VENV); \
	fi
	@if [ ! -x "$(VENV_PYTHON)" ]; then $(PYTHON) -m venv $(VENV); fi
	@$(VENV_PYTHON) -m pip --version >/dev/null 2>&1 || $(VENV_PYTHON) -m ensurepip --upgrade

develop: venv
	@command -v $(MATURIN) >/dev/null 2>&1 || { echo "maturin is not installed; enter nix develop or set MATURIN=/path/to/maturin"; exit 1; }
	@cd $(ROOT) && \
	  VIRTUAL_ENV=$(CURDIR)/$(VENV) \
	  PATH=$(CURDIR)/$(VENV)/bin:$$PATH \
	  PYO3_PYTHON=$(VENV_PYTHON) \
	  CARGO=$(CARGO_BIN) RUSTC=$(RUSTC_BIN) \
	  $(MATURIN) develop --features pyo3/extension-module

basic: develop
	@$(VENV_PYTHON) basic.py

regression: develop
	@$(VENV_PYTHON) regression.py

wheel-smoke: venv
	@command -v $(MATURIN) >/dev/null 2>&1 || { echo "maturin is not installed; enter nix develop or set MATURIN=/path/to/maturin"; exit 1; }
	@tmp="$$(mktemp -d)"; \
	trap 'rm -rf "$$tmp"' EXIT; \
	cd $(ROOT) && \
	  PYO3_PYTHON=$(VENV_PYTHON) \
	  CARGO=$(CARGO_BIN) RUSTC=$(RUSTC_BIN) \
	  $(MATURIN) build --features pyo3/extension-module --out "$$tmp/dist"; \
	wheels=( "$$tmp"/dist/machbus-*.whl ); \
	if [ ! -f "$${wheels[0]}" ]; then \
		echo "no machbus wheel built under $$tmp/dist"; \
		exit 1; \
	fi; \
	$(PYTHON) -m venv "$$tmp/wheel-venv"; \
	"$$tmp/wheel-venv/bin/python" -m pip --version >/dev/null 2>&1 || \
		"$$tmp/wheel-venv/bin/python" -m ensurepip --upgrade; \
	"$$tmp/wheel-venv/bin/python" -m pip --disable-pip-version-check install --no-deps --force-reinstall "$${wheels[0]}"; \
	cd $(CURDIR) && "$$tmp/wheel-venv/bin/python" -c 'import machbus, importlib.metadata as metadata; assert metadata.version("machbus"); assert hasattr(machbus, "Session"); assert not hasattr(machbus, "Machbus")'; \
	cd $(CURDIR) && "$$tmp/wheel-venv/bin/python" regression.py

test: basic regression wheel-smoke

clean:
	@rm -rf $(VENV)
