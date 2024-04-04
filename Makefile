ERROR := \x1b[0;91m
INFO := \x1b[0;94m
NC := \x1b[0m

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

.PHONY: coverage-pct
coverage-pct: ## Ensure code coverage == 100%
	@python -m coverage run -m unittest > /dev/null 2>&1 || :
	@python -m coverage json -q -o /dev/stdout | python -c \
		'import decimal, json, sys; \
		coverage = json.loads(input(), parse_float=decimal.Decimal); \
		percent_covered = coverage["totals"]["percent_covered"]; \
		print(percent_covered); \
		sys.exit(0 if percent_covered == 100 else 1);'

.PHONY: b
b: build
.PHONY: build
build: ## Build CronRunner
	python -m build

.PHONY: install
install: ## Install CronRunner
	install -d $(PREFIX)/bin/
	install ./cronrunner/cronrunner.py $(PREFIX)/bin/cronrunner

%:
	@$(call show_error_message,Unknown command '$@')
	@$(show_help_message)
