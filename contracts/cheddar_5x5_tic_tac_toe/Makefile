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
MIN_DEPOSIT=20
deploy-testnet:
	@NEAR_ENV=testnet near deploy ${CTR_T} ../../res/cheddar_big_tic_tac_toe.wasm \
	  "new" '{"cheddar": "'${CHEDDAR}'"}'

deploy-dev:
#rm neardev/tic_tac_toe/*
	@near dev-deploy --wasmFile ../../res/cheddar_big_tic_tac_toe.wasm  \
                --initFunction "new" \
                --projectKeyDirectory ./neardev/ \
                --initArgs '{"cheddar": "'${CHEDDAR}'", "min_deposit": ${MIN_DEPOSIT}}'

show-dev-account:
	cat neardev/tic_tac_toe/dev-account
