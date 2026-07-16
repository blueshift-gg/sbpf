SHELL := /bin/bash

build:
	make node; make bundler; make web
node:
	wasm-pack build crates/assembler --release --no-pack --out-dir ../../npm/dist/node --target nodejs
	rm npm/dist/node/.gitignore
bundler:
	wasm-pack build crates/assembler --release --no-pack --out-dir ../../npm/dist/bundler --target bundler
	rm npm/dist/bundler/.gitignore
	printf '{"type": "module"}' > npm/dist/bundler/package.json
web:
	wasm-pack build crates/assembler --release --no-pack --out-dir ../../npm/dist/web --target web
	rm npm/dist/web/.gitignore
	printf '{"type": "module"}' > npm/dist/web/package.json

.PHONY: test-examples
test-examples:
	@set -e; \
	cargo build; \
	for d in examples/*; do \
		if [ -d "$$d" ]; then \
			echo "=== Building and testing $$d ==="; \
			( cd "$$d" && cargo run --manifest-path ../../Cargo.toml --bin sbpf -- build || exit 1 ); \
			( cd "$$d" && cargo run --manifest-path ../../Cargo.toml --bin sbpf -- test || exit 1 ); \
		fi; \
	done

release:
	@set -o pipefail; \
	for pkg in sbpf-syscall-map sbpf-common sbpf-ir sbpf-analyze sbpf-vm sbpf-assembler sbpf-disassembler sbpf-runtime sbpf-debugger sbpf; do \
		echo "Publishing $$pkg..."; \
		if cargo publish --package=$$pkg 2>&1 | tee /tmp/publish-$$pkg.log; then \
			: ; \
		elif grep -qiE "already (uploaded|exists)" /tmp/publish-$$pkg.log; then \
			echo "$$pkg: already published, skipping"; \
		else \
			echo "$$pkg: publish failed"; \
			exit 1; \
		fi; \
	done
release-npm:
	cd npm && npm publish --access public