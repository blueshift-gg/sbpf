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
