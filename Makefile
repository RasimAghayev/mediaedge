.PHONY: up down build test lint fmt fmt-check health logs reset clean ci

COMPOSE_DEV := infrastructure/docker/compose/compose.dev.yaml
COMPOSE_TEST := infrastructure/docker/compose/compose.test.yaml

up: ## Start the full dev stack
	docker compose -f $(COMPOSE_DEV) up -d

down: ## Stop the dev stack
	docker compose -f $(COMPOSE_DEV) down

build: ## Build all Docker images
	docker compose -f $(COMPOSE_DEV) build

test: ## Run the full test suite
	cargo nextest run --workspace --profile ci

test-integration: ## Run integration tests
	cargo nextest run --workspace --profile integration --test-threads=4

lint: ## Run clippy with warnings as errors
	cargo clippy --all-targets --all-features -D warnings

fmt: ## Format code
	cargo fmt --all

fmt-check: ## Check formatting
	cargo fmt --all --check

typecheck: ## Typecheck all targets
	cargo check --all-targets --all-features

health: ## Check service health
	docker compose -f $(COMPOSE_DEV) ps

logs: ## Follow service logs
	docker compose -f $(COMPOSE_DEV) logs -f

reset: ## Reset the dev stack (destroy volumes)
	docker compose -f $(COMPOSE_DEV) down -v
	docker compose -f $(COMPOSE_DEV) up -d

clean: ## Clean build artifacts
	cargo clean

ci: fmt-check lint typecheck test test-integration

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'
