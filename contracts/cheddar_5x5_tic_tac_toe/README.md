```rust
X ▢ ▢ ▢ ▢
▢ ▢ O ▢ ▢
▢ ▢ ▢ ▢ ▢
▢ X ▢ ▢ ▢
▢ O ▢ O ▢
```

## Cheddar 5X5 TIC-TAC-TOE Game


#### setup environment
```sh
export TICTACTOE=
export PLAYER1=first.near
export PLAYER2=second.near
export ONE_TOKEN_DEPOSIT=1000000000000000000000000
export ONE_NEAR=1000000000000000000000000
```
#### #[init]
```rust
"config" : {
    /// Service fee in BASIS_P E.g 10% => 1000; 2% => 200
    pub service_fee_percentage: u32,
    /// Referrer ratio to fees distribution from `service_fee_percentage`
    /// in BASIS_P. E.g if `service_fee_percentage` = 1000 (10%)
    /// `referrer_ratio` = 5000 means that 5% from total game reward
    /// comes to protocol and 5% to referrer
    pub referrer_ratio: u32,
    /// `max_game_duration_sec` in seconds (0..3600) is required 
    pub max_game_duration_sec: u32
    /// max number of stored games into contract
    pub max_stored_games: u8
}
```
```rust
/// 100 minutes - max game duration
/// 2% - service fee
/// 50% from service fee (1%) goes to winner's refferers
near call $TICTACTOE new '{
    "config": {
        "service_fee_percentage": 200,
        "referrer_ratio": 5000,
        "max_game_duration_sec": 6000
    }
}' --accountId $TICTACTOE
```

#### whitelist token(private) and register contract into token
```rust
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
```rust
near call $TICTACTOE make_available '{}' --accountId $PLAYER1 --amount 1 --gas=300000000000000
```
FT
```rust
near call token-v3.cheddar.testnet ft_transfer_call '{
    "receiver_id":"'$TICTACTOE'",
    "amount":"'$ONE_TOKEN_DEPOSIT'",
    "msg": ""
}' --accountId $PLAYER2 --gas=300000000000000 --depositYocto 1
```

#### make available (with referrer)
NEAR
```rust
near call $TICTACTOE make_available '{
    "game_config": {
        "referrer_id": "'$PLAYER1'"
    }
}' --accountId participant_1.testnet --depositYocto=$ONE_NEAR --gas=300000000000000
```
FT
```rust
near call token-v3.cheddar.testnet ft_transfer_call '{
    "receiver_id":"'$TICTACTOE'",
    "amount":"'$ONE_TOKEN_DEPOSIT'",
    "msg": "{\"referrer_id\":\"'$PLAYER1'\"}"
}' --accountId $PLAYER2 --depositYocto 1 --gas=300000000000000
```
#### make unavailable
```rust
near call $TICTACTOE make_unavailable '' --accountId $PLAYER1 --depositYocto=1 --gas=300000000000000
```

```rust
near view $TICTACTOE get_available_players ''
```

#### start game
```rust
near call $TICTACTOE start_game '{"player_2_id": "'$PLAYER1'"}' --accountId $PLAYER2
near view $TICTACTOE get_active_games ''
near view $TICTACTOE get_last_games ''

```

#### play
```rust
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
```rust
near call $TICTACTOE give_up '{"game_id": 0}' --accountId $USER_ID --depositYocto 1 --gas=300000000000000
near call $TICTACTOE stop_game '{"game_id": 0}' --accountId $USER_ID --gas=300000000000000
```

#### more views
```rust
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