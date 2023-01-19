build:
	@RUSTFLAGS='-C link-arg=-s' cargo build --target wasm32-unknown-unknown --release
	mkdir -p ../../res
	@cp ../../target/wasm32-unknown-unknown/release/*.wasm ../../res/

# run all tests
# to test specific test run: cargo test <test name>
test:
	@cargo test

# doesn't capture stdout and prints all "println"
test-debug:
# "--" allows to pass extra arguments
	@RUST_BACKTRACE=1 cargo test  -- --nocapture

test-unit:
	@cargo test --lib
