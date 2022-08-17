## x ▢ ▢
## ▢ ▢ o
## ▢ ▢ ▢
## Cheddar TIC-TAC-TOE Game


#### setup environment
```sh
export TICTACTOE=tictactoe.cheddar.testnet
export USER_ID=rmlsnk.testnet
export USER_ID_1=participant_1.testnet
export USER_ID_2=participant_2.testnet
export ONE_TOKEN=1000000000000000000000000
export ONE_TOKEN_DEPOSIT=1000000000000000000000001
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
near call $TICTACTOE set_max_duration '{"max_duration": 900}' --accountId $TICTACTOE
near call $TICTACTOE whitelist_token '{
    "token_id" : "token-v3.cheddar.testnet",
    "min_deposit": "'$ONE_TOKEN_DEPOSIT'"
}' --accountId $TICTACTOE
near call token-v3.cheddar.testnet storage_deposit '' --accountId $TICTACTOE --amount 0.0125
near view $TICTACTOE get_whitelisted_tokens ''
```

#### make available (no referrer)
NEAR
```rust
near call $TICTACTOE make_available '{
    "config": {
        "deposit": "'$ONE_TOKEN_DEPOSIT'",
    }
}' --accountId $USER_ID_1 --depositYocto=$ONE_NEAR --gas=300000000000000
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
        "deposit": "'$ONE_TOKEN_DEPOSIT'",
        "referrer_id": "'$USER_ID_1'"
    }
}' --accountId $USER_ID --depositYocto=$ONE_NEAR --gas=300000000000000
```
FT
```rust
near call token-v3.cheddar.testnet ft_transfer_call '{
    "receiver_id":"'$TICTACTOE'",
    "amount":"'$ONE_TOKEN_DEPOSIT'",
    "msg": "{\"referrer_id\":\"'$USER_ID_1'\"}"
}' --accountId $USER_ID_2 --depositYocto 1 --gas=300000000000000
```

```rust
near view $TICTACTOE get_available_players ''
```

#### start game
```rust
near call $TICTACTOE start_game '{"player_2_id": "'$USER_ID'"}' --accountId $USER_ID_1
```

#### play
```rust
/// view order for players to move
near view $TICTACTOE get_contract_params ''
near call $TICTACTOE make_move '{"game_id": 2, "row": 0, "col": 1}' --accountId $USER_ID 
near call $TICTACTOE make_move '{"game_id": 2, "row": 0, "col": 0}' --accountId $USER_ID_1
near call $TICTACTOE make_move '{"game_id": 2, "row": 1, "col": 1}' --accountId $USER_ID 
near call $TICTACTOE make_move '{"game_id": 2, "row": 2, "col": 2}' --accountId $USER_ID_1
near call $TICTACTOE make_move '{"game_id": 2, "row": 0, "col": 2}' --accountId $USER_ID 
near call $TICTACTOE make_move '{"game_id": 2, "row": 2, "col": 0}' --accountId $USER_ID_1
near call $TICTACTOE make_move '{"game_id": 2, "row": 2, "col": 1}' --accountId $USER_ID 
near call $TICTACTOE make_move '{"game_id": 2, "row": 1, "col": 0}' --accountId $USER_ID_1

near view $TICTACTOE get_stats '{"account_id": "'$USER_ID'"}'
near view $TICTACTOE get_stats '{"account_id": "'$USER_ID_1'"}'
near view $TICTACTOE get_stats '{"account_id": "'$USER_ID_2'"}'
```
#### give-up
```rust
near call $TICTACTOE give_up '{"game_id": 2}' --accountId $USER_ID --depositYocto 1 --gas=300000000000000
```