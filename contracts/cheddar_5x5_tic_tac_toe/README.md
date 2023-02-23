```rust
X ▢ ▢ ▢ ▢
▢ ▢ O ▢ ▢
▢ ▢ ▢ ▢ ▢ ...
▢ X ▢ ▢ ▢
▢ O ▢ O ▢
    .
    .
    .
```

## Cheddar 25x25 TIC-TAC-TOE Game

### Who starts th game?

The starting player is selected at the beginning of the game using NEAR random mechanism. The player that stards will always have `O` piece and the other `X`.

#### setup environment

```sh
export TICTACTOE=<account-where-we-deploy>
export PLAYER1=first.near
export PLAYER2=second.near
export ONE_TOKEN_DEPOSIT=1000000000000000000000000
export ONE_NEAR=1000000000000000000000000
```

#### Setup

See Makefile `deploy-testnet` job and `config.rs` for available config options.

#### whitelist token(private) and register contract into token

```sh
near call $TICTACTOE set_max_duration '{"max_duration": 3600}' --accountId $TICTACTOE
near call $TICTACTOE whitelist_token '{
    "token_id" : "token-v3.cheddar.testnet",
    "min_deposit": "'$ONE_TOKEN_DEPOSIT'"
}' --accountId $TICTACTOE
near call token-v3.cheddar.testnet storage_deposit '' --accountId $TICTACTOE --amount 0.0125
near view $TICTACTOE get_whitelisted_tokens ''
```

#### make available (no referrer, no opponent)

NEAR

```sh
near call $TICTACTOE make_available '{}' --accountId $PLAYER1 --amount 1 --gas=300000000000000
```

FT

```sh
near call token-v3.cheddar.testnet ft_transfer_call '{
    "receiver_id":"'$TICTACTOE'",
    "amount":"'$ONE_TOKEN_DEPOSIT'",
    "msg": ""
}' --accountId $PLAYER2 --gas=300000000000000 --depositYocto 1
```

#### make available (with referrer)

NEAR

```sh
near call $TICTACTOE make_available '{
    "game_config": {
        "referrer_id": "'$PLAYER1'"
    }
}' --accountId participant_1.testnet --depositYocto=$ONE_NEAR --gas=300000000000000
```

FT

```sh
near call token-v3.cheddar.testnet ft_transfer_call '{
    "receiver_id":"'$TICTACTOE'",
    "amount":"'$ONE_TOKEN_DEPOSIT'",
    "msg": "{\"referrer_id\":\"'$PLAYER1'\"}"
}' --accountId $PLAYER2 --depositYocto 1 --gas=300000000000000
```

#### make unavailable

```sh
near call $TICTACTOE make_unavailable '' --accountId $PLAYER1 --depositYocto=1 --gas=300000000000000
```

```sh
near view $TICTACTOE get_available_players ''
```

#### start game

```sh
near call $TICTACTOE start_game '{"player_2_id": "'$PLAYER1'"}' --accountId $PLAYER2
near view $TICTACTOE get_active_games ''
near view $TICTACTOE get_last_games ''

```

#### play

```sh
/// view order for players to move
near view $TICTACTOE get_contract_params ''

near call $TICTACTOE make_move '{"game_id": 0, "row": 0, "col": 1}' --accountId $PLAYER1 --gas 300000000000000
near call $TICTACTOE make_move '{"game_id": 0, "row": 0, "col": 4}' --accountId $PLAYER2 --gas 300000000000000
near call $TICTACTOE make_move '{"game_id": 0, "row": 1, "col": 1}' --accountId $PLAYER1 --gas 300000000000000
near call $TICTACTOE make_move '{"game_id": 0, "row": 1, "col": 3}' --accountId $PLAYER2 --gas 300000000000000
near call $TICTACTOE make_move '{"game_id": 0, "row": 0, "col": 2}' --accountId $PLAYER1 --gas 300000000000000
near call $TICTACTOE make_move '{"game_id": 0, "row": 2, "col": 2}' --accountId $PLAYER2 --gas 300000000000000
near call $TICTACTOE make_move '{"game_id": 0, "row": 2, "col": 1}' --accountId $PLAYER1 --gas 300000000000000
near call $TICTACTOE make_move '{"game_id": 0, "row": 3, "col": 1}' --accountId $PLAYER2 --gas 300000000000000
near call $TICTACTOE make_move '{"game_id": 0, "row": 3, "col": 3}' --accountId $PLAYER1 --gas 300000000000000


near view $TICTACTOE get_stats '{"account_id": "'$PLAYER1'"}'
near view $TICTACTOE get_stats '{"account_id": "'$PLAYER2'"}'
```

#### give-up

```sh
near call $TICTACTOE give_up '{"game_id": 0}' --accountId $USER_ID --depositYocto 1 --gas=300000000000000
near call $TICTACTOE stop_game '{"game_id": 0}' --accountId $USER_ID --gas=300000000000000
```

#### Claim timeout win

When your opponent doesnt to respond for 5 or more minutes you are able to claim a timeout win. The win reward will be transferred to you and the game will end

```sh
near call $TICTACTOE claim_timeout_win '{"game_id": 4}' --accountId $USER_ID
```

#### more views

```sh
// total players across all played games history (num)
near view $TICTACTOE get_total_stats_num ''
// total players across all played games history (accounts)
near view $TICTACTOE get_accounts_played ''
// penalty games num for given player account_id
near view $TICTACTOE get_user_penalties '{"account_id":"'$USER_ID'"}'
// all user penalties (non-zeroed)
near view $TICTACTOE get_penalty_users ''
// stored games
near view $TICTACTOE get_game '{"game_id": 0}'
```
