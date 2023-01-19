#!/usr/bin/env bash
set -e

# REQUIRED! - ./neardev/tic_tac_toe/dev-account empty file for deployment

rm neardev/tic_tac_toe/*

################################################################################
near dev-deploy --wasmFile ./res/cheddar_big_tic_tac_toe.wasm  \
	--projectKeyDirectory ./neardev/tic_tac_toe/ \
	"new"
	'{
	"config": {
	    "service_fee_percentage": 200,
	    "referrer_ratio": 5000,
	    "max_game_duration_sec": 3600,
	    "max_stored_games": 50
	}
	}'

TICTACTOE=$(cat neardev/tic_tac_toe/dev-account)
