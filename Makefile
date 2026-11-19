# Prefer the project-local toolchain when present. A normal Rust installation also works.
ifneq ($(wildcard $(CURDIR)/.toolchain/cargo/bin/cargo),)
export RUSTUP_HOME := $(CURDIR)/.toolchain/rustup
export CARGO_HOME := $(CURDIR)/.toolchain/cargo
export PATH := $(CARGO_HOME)/bin:$(PATH)
CARGO := $(CARGO_HOME)/bin/cargo
else
CARGO := cargo
endif
PYTHON ?= .venv/bin/python
export PYTHONPATH := $(CURDIR)/python

.PHONY: release check episode
release:
	$(CARGO) build --locked --offline --release

check: release
	$(CARGO) test --locked --offline
	$(PYTHON) -m unittest discover -s python/tests -v

episode: release
	$(PYTHON) examples/episodes.py --seed 42

.PHONY: play
play: release
	$(PYTHON) python/arena_play.py $(PLAY_ARGS)

.PHONY: web
web: release
	$(PYTHON) python/arena_web.py $(WEB_ARGS)
