.DEFAULT_GOAL := help

.PHONY: help build run demo-build demo media stills install dist fmt lint deny test nvim sanitize brand landing landing-dev check

# Homebrew's llvm is keg-only: its clang tools are not on PATH by default.
LLVM := /opt/homebrew/opt/llvm/bin
CLANG_FORMAT ?= $(or $(shell command -v clang-format),$(LLVM)/clang-format)
CLANG_TIDY ?= $(or $(shell command -v clang-tidy),$(LLVM)/clang-tidy)
TARGET ?= $(shell rustc -vV | sed -n 's/^host: //p')
DIST := postal-$(TARGET)

help: ## list every target
	@grep -E '^[a-z-]+:.*## ' $(MAKEFILE_LIST) | awk -F':.*## ' '{printf "  %-11s %s\n", $$1, $$2}'

build: ## build the release binary
	cargo build --release

run: ## run the release build
	cargo run --release

demo-build:
	cargo build --release --bins --example demo

demo: demo-build ## run postal over the demo mailbox, touching nothing of your own
	target/release/examples/demo target/release/postal

media: demo-build ## record the landing page's videos from demo/tapes with VHS
	for tape in demo/tapes/*.tape; do \
		[ "$$tape" = demo/tapes/setup.tape ] || vhs "$$tape" || exit 1; \
	done

stills: demo-build ## shoot the landing page's screenshots in your Ghostty (macOS)
	demo/stills.sh

install: ## install a release postal in ~/.cargo/bin
	cargo install --path . --locked --force
	@echo "postal: $$(command -v postal)"

# install.sh reads the binary from $(DIST)/postal in this archive.
dist: ## pack the release archive of postal for TARGET
	cargo build --release --locked --target $(TARGET)
	rm -rf dist/$(DIST) && mkdir -p dist/$(DIST)
	cp target/$(TARGET)/release/postal LICENSE THIRD_PARTY_LICENSES.md README.md dist/$(DIST)/
	tar -czf dist/$(DIST).tar.gz -C dist $(DIST)
	cd dist && shasum -a 256 $(DIST).tar.gz > $(DIST).tar.gz.sha256

fmt: ## format the Rust, the C++ and the Lua
	cargo fmt
	$(CLANG_FORMAT) -i cpp/*
	$(MAKE) -C nvim fmt

lint: ## fail on unformatted code, a clippy lint or a clang-tidy check
	cargo fmt --check
	cargo clippy --all-targets -- -D warnings
	$(CLANG_FORMAT) --dry-run --Werror cpp/*
	$(CLANG_TIDY) --quiet cpp/*.cpp

deny: ## check the dependencies' advisories, licenses and sources
	cargo deny check

test: ## run every Rust test
	cargo test

nvim: ## format, lint and smoke-test the Neovim plugin
	$(MAKE) -C nvim check

sanitize: ## lay out every cached HTML mail under ASan and UBSan
	scripts/sanitize.sh

brand: ## write the design tokens and draw the logo files in assets/brand
	uv run scripts/tokens.py
	uv run scripts/brand.py

landing: ## lint, type-check and build the landing page
	cd landing && pnpm install --frozen-lockfile && pnpm lint && pnpm check && pnpm build

landing-dev: ## serve the landing page at http://localhost:4321, reloading on edits
	cd landing && pnpm install --frozen-lockfile && pnpm dev

check: lint deny test nvim ## the gate before a commit
