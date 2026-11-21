.DEFAULT_GOAL := help

.PHONY: help fmt lint smoke check

help: ## list every target
	@grep -E '^[a-z-]+:.*## ' $(MAKEFILE_LIST) | awk -F':.*## ' '{printf "  %-8s %s\n", $$1, $$2}'

fmt: ## format every Lua file with stylua
	stylua .

lint: ## fail if a Lua file is not formatted
	stylua --check .

# POSTAL_BIN runs a real postal; without it, scripts/postal-stub answers.
smoke: ## complete a Draft's recipients and chat with a stub agent, headless
	nvim --clean --headless --cmd "set rtp^=." -c "lua vim.schedule(function() dofile('scripts/smoke.lua') end)"

check: fmt lint smoke ## the gate before any commit
