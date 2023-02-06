```rust
X ▢ ▢
▢ ▢ O 
▢ ▢ ▢ 
```
## Cheddar TIC-TAC-TOE Game


#### setup environment
```sh
export TICTACTOE=tictactoe.cheddar.testnet
export USER_ID=rmlsnk.testnet
export USER_ID_1=guacharo.testnet
export USER_ID_2=second.testnet
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
}
```
```rust
/// 10 minutes - max game duration
/// 2% - service fee
/// 50% from service fee (1%) goes to winner's refferers
near call $TICTACTOE new '{
    "config": {
        "service_fee_percentage": 200,
        "referrer_ratio": 5000,
        "max_game_duration_sec": 600
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
near call $TICTACTOE make_available '{}' --accountId $USER_ID_1 --amount 1 --gas=300000000000000
```
FT
```rust
near call token-v3.cheddar.testnet ft_transfer_call '{
    "receiver_id":"'$TICTACTOE'",
    "amount":"'$ONE_TOKEN_DEPOSIT'",
    "msg": ""
}' --accountId $USER_ID --gas=300000000000000 --depositYocto 1
```

#### make available (with referrer)
NEAR
```rust
near call $TICTACTOE make_available '{
    "game_config": {
        "referrer_id": "'$USER_ID'"
    }
}' --accountId participant_1.testnet --depositYocto=$ONE_NEAR --gas=300000000000000
```
FT
```rust
near call token-v3.cheddar.testnet ft_transfer_call '{
    "receiver_id":"'$TICTACTOE'",
    "amount":"'$ONE_TOKEN_DEPOSIT'",
    "msg": "{\"referrer_id\":\"'$USER_ID_1'\"}"
}' --accountId $USER_ID_2 --depositYocto 1 --gas=300000000000000
```
#### make unavailable
```rust
near call $TICTACTOE make_unavailable '' --accountId $USER_ID --depositYocto=1 --gas=300000000000000
```

```rust
near view $TICTACTOE get_available_players ''
```

#### start game
```rust
near call $TICTACTOE start_game '{"player_2_id": "'$USER_ID_1'"}' --accountId $USER_ID
near view $TICTACTOE get_active_games ''
near view $TICTACTOE get_last_games ''

```

#### play
```rust
/// view order for players to move
near view $TICTACTOE get_contract_params ''

near call $TICTACTOE make_move '{"game_id": 5, "row": 0, "col": 0}' --accountId participant_1.testnet
near call $TICTACTOE make_move '{"game_id": 3, "row": 0, "col": 1}' --accountId $USER_ID_1
near call $TICTACTOE make_move '{"game_id": 3, "row": 0, "col": 2}' --accountId $USER_ID
near call $TICTACTOE make_move '{"game_id": 3, "row": 2, "col": 0}' --accountId $USER_ID_1
near call $TICTACTOE make_move '{"game_id": 3, "row": 2, "col": 1}' --accountId $USER_ID
near call $TICTACTOE make_move '{"game_id": 3, "row": 2, "col": 2}' --accountId $USER_ID_1 
near call $TICTACTOE make_move '{"game_id": 3, "row": 1, "col": 0}' --accountId $USER_ID
near call $TICTACTOE make_move '{"game_id": 3, "row": 1, "col": 2}' --accountId $USER_ID_1
near call $TICTACTOE make_move '{"game_id": 3, "row": 1, "col": 1}' --accountId $USER_ID --gas 300000000000000

near view $TICTACTOE get_stats '{"account_id": "'$USER_ID'"}'
near view $TICTACTOE get_stats '{"account_id": "'$USER_ID_1'"}'
near view $TICTACTOE get_stats '{"account_id": "'$USER_ID_2'"}'
```
#### give-up
```rust
near call $TICTACTOE give_up '{"game_id": 2}' --accountId $USER_ID --depositYocto 1 --gas=300000000000000
near call $TICTACTOE stop_game '{"game_id": 4}' --accountId $USER_ID --gas=300000000000000
```

#### Claim timeout win
When your opponent doesnt to respond for 5 or more minutes you are able to claim a timeout win. The win reward will be transferred to you and the game will end
```rust
near call $TICTACTOE claim_timeout_win '{"game_id": 4}' --accountId $USER_ID
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
```