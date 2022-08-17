#!/usr/bin/env bash
set -e

rm neardev/tic_tac_toe/*


OWNER=participant_20.testnet #TODO
USER1=rmlsnk.testnet #TODO
USER2=participant_22.testnet #TODO
USER3=participant_21.testnet #TODO
USER4=participant_23.testnet #TODO

TOKEN=guacharo.testnet
ONE_TOKEN=1000000000000000000000000

function has_substring() {
   [[ "$1" != "${2/$1/}" ]]
}

################################################################################

near dev-deploy --wasmFile ./res/cheddar_big_tic_tac_toe.wasm  \
		--initFunction "new" \
		--projectKeyDirectory ./neardev/tic_tac_toe/ \
		--initArgs '{
            "config": {
                "service_fee_percentage": 200,
                "referrer_ratio": 5000,
                "max_game_duration_sec": 3600,
                "max_stored_games": 50
            }
		}'

TICTACTOE=$(cat neardev/tic_tac_toe/dev-account)

echo set_max_duration
near call $TICTACTOE set_max_duration '{"max_duration": 3600}' --accountId $TICTACTOE 

echo whitelist
near call $TICTACTOE whitelist_token '{
    "token_id" : "'$TOKEN'",
    "min_deposit": "'$ONE_TOKEN'"
}' --accountId $TICTACTOE

near call $TOKEN storage_deposit '' --accountId $TICTACTOE --amount 0.0125
near view $TICTACTOE get_whitelisted_tokens ''

echo make_users_available
near call $TICTACTOE make_available '{}' --accountId $USER1 --amount 1 --gas=300000000000000
near call $TICTACTOE make_available '{}' --accountId $USER2 --amount 1 --gas=300000000000000

echo make_available_ft
near call $TOKEN ft_transfer_call '{
    "receiver_id":"'$TICTACTOE'",
    "amount":"'$ONE_TOKEN'",
    "msg": ""
}' --accountId $USER3 --gas=300000000000000 --depositYocto 1

near call $TOKEN ft_transfer_call '{
    "receiver_id":"'$TICTACTOE'",
    "amount":"'$ONE_TOKEN'",
    "msg": "{\"referrer_id\":\"'$USER1'\"}"
}' --accountId $USER4 --gas=300000000000000 --depositYocto 1

near view $TICTACTOE get_available_players ''


echo start_game_0
near call $TICTACTOE start_game '{"player_2_id": "'$USER1'"}' --accountId $USER2
near view $TICTACTOE get_active_games ''
near view $TICTACTOE get_last_games ''
sleep 20
GAME_EXPIRATION=$(date -v +100S +%s)

echo start_game_1
near call $TICTACTOE start_game '{"player_2_id": "'$USER3'"}' --accountId $USER4
near view $TICTACTOE get_active_games ''
near view $TICTACTOE get_last_games ''

echo play_first_game

####
PLAYER_VIEW=$(near view $TICTACTOE get_current_player '{"game_id":0}')
array=( $PLAYER_VIEW )
SUB='testnet'

i=0

while [ $i -lt ${#array[@]} ];
do
    if has_substring "${array[$i]}" "testnet"
    then
        ACCOUNT1=${array[$i]}
    fi
    let i++
done
####
PLAYER_VIEW=$(near view $TICTACTOE get_next_player '{"game_id":0}')
array=( $PLAYER_VIEW )
SUB='testnet'

i=0

while [ $i -lt ${#array[@]} ];
do
    if has_substring "${array[$i]}" "testnet"
    then
        ACCOUNT2=${array[$i]}
    fi
    let i++
done
####
echo players
PLAYER1=$( echo "${ACCOUNT1}" | sed "s/\'//g" | tr -s ')')
PLAYER2=$( echo "${ACCOUNT2}" | sed "s/\'//g" | tr -s ')')

echo game_started
near call $TICTACTOE make_move '{"game_id": 0, "row": 0, "col": 1}' --accountId $PLAYER1 --gas 300000000000000
near call $TICTACTOE make_move '{"game_id": 0, "row": 0, "col": 4}' --accountId $PLAYER2 --gas 300000000000000
near call $TICTACTOE make_move '{"game_id": 0, "row": 1, "col": 1}' --accountId $PLAYER1 --gas 300000000000000
near call $TICTACTOE make_move '{"game_id": 0, "row": 1, "col": 3}' --accountId $PLAYER2 --gas 300000000000000
near call $TICTACTOE make_move '{"game_id": 0, "row": 0, "col": 2}' --accountId $PLAYER1 --gas 300000000000000
near call $TICTACTOE make_move '{"game_id": 0, "row": 2, "col": 2}' --accountId $PLAYER2 --gas 300000000000000
near call $TICTACTOE make_move '{"game_id": 0, "row": 2, "col": 1}' --accountId $PLAYER1 --gas 300000000000000
near call $TICTACTOE make_move '{"game_id": 0, "row": 3, "col": 1}' --accountId $PLAYER2 --gas 300000000000000
near call $TICTACTOE make_move '{"game_id": 0, "row": 3, "col": 3}' --accountId $PLAYER1 --gas 300000000000000

# date0=$(date +%s)
# i=0

# while [ "$date0" != "$GAME_EXPIRATION" ]
# do
#     echo LHS $date0
#     echo RHS $START
#     date0=$(date -v +1S +%s)
#     sleep 1
#     ((i=i+1))
# done

near call $TICTACTOE make_move '{"game_id": 1, "row": 4, "col": 0}' --accountId $USER2 --gas 300000000000000

near view $TICTACTOE get_stats '{"account_id": "'$USER1'"}'
near view $TICTACTOE get_stats '{"account_id": "'$USER2'"}'
near view $TICTACTOE get_stats '{"account_id": "'$USER3'"}'
near view $TICTACTOE get_stats '{"account_id": "'$USER4'"}'

near view $TICTACTOE get_total_stats_num '' 
near view $TICTACTOE get_accounts_played ''
near view $TICTACTOE get_user_penalties '{"account_id":"'$USER1'"}'
near view $TICTACTOE get_user_penalties '{"account_id":"'$USER2'"}'
near view $TICTACTOE get_penalty_users ''
near view $TICTACTOE get_game '{"game_id":0}'

echo TIC_TAC_TOE is ${TIC_TAC_TOE}
