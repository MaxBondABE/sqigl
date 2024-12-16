#!/usr/bin/env just --justfile

default: check check-docs

check:
    cargo check
    cargo fmt --check --all
    cargo clippy --all

check-docs:
    zola --root docs/ check

build-docs:
    zola --root docs/ build

clean:
    cargo clean
    rm -rf docs/public
