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


CTR_T=tic-tac-toe-5-v1.cheddar.testnet
CHEDDAR=token-v3.cheddar.testnet
deploy-testnet:
	@NEAR_ENV=testnet near deploy ${CTR_T} ../../res/cheddar_big_tic_tac_toe.wasm \
	  "new" '{"cheddar": "'${CHEDDAR}'"}'

deploy-dev:
#rm neardev/tic_tac_toe/*
	@near dev-deploy ../../cheddar_big_tic_tac_toe.wasm \
		--projectKeyDirectory ./neardev/tic_tac_toe/ \
		"new" '{"cheddar": "'${CHEDDAR}'"}'

show-dev-account:
	cat neardev/tic_tac_toe/dev-account
