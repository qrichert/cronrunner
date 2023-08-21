ERROR := \033[0;91m
INFO := \033[0;94m
NC := \033[0m

define show_help_message
	echo "Usage: make TARGET"
	echo ""
	echo "Commands:"
	grep -hE '^[A-Za-z0-9_ \-]*?:.*##.*$$' $(MAKEFILE_LIST) | \
	    awk 'BEGIN {FS = ":.*?## "}; {printf "  $(INFO)%-12s$(NC) %s\n", $$1, $$2}'
endef

define show_error_message
	echo "$(ERROR)[Error] $(1)$(NC)"
endef

PREFIX ?= /usr/local

SOURCE_DIRS := cronrunner tests

.PHONY: all
all: help

.PHONY: help
help: ## Show this help message
	@$(show_help_message)

.PHONY: l
l: lint
.PHONY: lint
lint: ## Run various linting tools
	@pre-commit run --all-files

.PHONY: t
t: test
.PHONY: test
test: ## Run unit tests
	@python -m unittest

.PHONY: c
c: coverage
.PHONY: coverage
coverage: ## Unit tests coverage report
	@python -m coverage run -m unittest
	@python -m coverage html -d var/htmlcov
	@open var/htmlcov/index.html || xdg-open var/htmlcov/index.html || :

.PHONY: b
b: build
.PHONY: build
build: ## Build cronrunner
	python -m build

.PHONY: install
install: ## Install cronrunner
	install -d $(PREFIX)/bin/
	install ./cronrunner/cronrunner.py $(PREFIX)/bin/cronrunner

%:
	@$(call show_error_message,Unknown command '$@')
	@$(show_help_message)
