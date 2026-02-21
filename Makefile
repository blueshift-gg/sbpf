build:
	make node; make bundler; make web
node:
	wasm-pack build crates/assembler --release --no-pack --out-dir ../../dist/node --target nodejs
	rm dist/node/.gitignore
bundler:
	wasm-pack build crates/assembler --release --no-pack --out-dir ../../dist/bundler --target bundler
	rm dist/bundler/.gitignore
web:
	wasm-pack build crates/assembler --release --no-pack --out-dir ../../dist/web --target web
	rm dist/web/.gitignore

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
	@for pkg in sbpf-syscall-map sbpf-common sbpf-vm sbpf-assembler sbpf-disassembler; do \
		echo "Publishing $$pkg..."; \
		cargo publish --package=$$pkg 2>&1 | tee /tmp/publish-$$pkg.log || \
		if grep -q "already uploaded" /tmp/publish-$$pkg.log; then \
			echo "$$pkg: already published, skipping"; \
		else \
			exit 1; \
		fi; \
	done