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
	for d in examples/*; do \
		if [ -d "$$d" ]; then \
			echo "=== Building and testing $$d ==="; \
			( cd "$$d" && sbpf build || exit 1 ); \
			( cd "$$d" && sbpf test || exit 1 ); \
		fi; \
	done
