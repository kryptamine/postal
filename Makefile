.DEFAULT_GOAL := help

.PHONY: help build run install fmt lint test nvim sanitize brand check

# Homebrew's llvm is keg-only: its clang tools are not on PATH by default.
LLVM := /opt/homebrew/opt/llvm/bin
CLANG_FORMAT ?= $(or $(shell command -v clang-format),$(LLVM)/clang-format)
CLANG_TIDY ?= $(or $(shell command -v clang-tidy),$(LLVM)/clang-tidy)
PLUGIN := $(or $(XDG_DATA_HOME),$(HOME)/.local/share)/postal/nvim

help: ## list every target
	@grep -E '^[a-z-]+:.*## ' $(MAKEFILE_LIST) | awk -F':.*## ' '{printf "  %-9s %s\n", $$1, $$2}'

build: ## build the release binary
	cargo build --release

run: ## run the release build
	cargo run --release

# The plugin is copied whole, then swapped in, so no Neovim loads half of one.
install: ## install a release postal in ~/.cargo/bin and a copy of the plugin
	cargo install --path . --locked --force
	rm -rf "$(PLUGIN).new" && mkdir -p "$(PLUGIN).new"
	cp -R nvim/lua "$(PLUGIN).new/"
	rm -rf "$(PLUGIN)" && mv "$(PLUGIN).new" "$(PLUGIN)"
	@echo "postal: $$(command -v postal)"
	@echo "plugin: $(PLUGIN)"

fmt: ## format the Rust, the C++ and the Lua
	cargo fmt
	$(CLANG_FORMAT) -i cpp/*
	$(MAKE) -C nvim fmt

lint: ## fail on unformatted code, a clippy lint or a clang-tidy check
	cargo fmt --check
	cargo clippy --all-targets -- -D warnings
	$(CLANG_FORMAT) --dry-run --Werror cpp/*
	$(CLANG_TIDY) --quiet cpp/*.cpp

test: ## run every Rust test
	cargo test

nvim: ## format, lint and smoke-test the Neovim plugin
	$(MAKE) -C nvim check

sanitize: ## lay out every cached HTML mail under ASan and UBSan
	scripts/sanitize.sh

brand: ## write the design tokens and draw the logo files in assets/brand
	uv run scripts/tokens.py
	uv run scripts/brand.py

check: lint test nvim ## the gate before a commit
