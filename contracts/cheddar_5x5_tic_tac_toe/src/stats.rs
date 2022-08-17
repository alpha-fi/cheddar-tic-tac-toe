use crate::*;

#[derive(PartialEq)]
pub enum UpdateStatsAction {
    AddPlayedGame,
    AddReferral,
    AddAffiliate,
    AddWonGame,
    AddTotalReward,
    AddAffiliateReward,
    AddPenaltyGame,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Stats {
    pub referrer_id: Option<AccountId>,
    pub affiliates: UnorderedSet<AffiliateId>,
    pub games_num: u64,
    pub victories_num: u64,
    pub penalties_num: u64,
    pub total_reward: UnorderedMap<TokenContractId, Balance>,
    pub total_affiliate_reward: UnorderedMap<TokenContractId, Balance>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct UserPenalties {
    pub penalties_num: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub struct StatsView {
    pub referrer_id: Option<AccountId>,
    pub games_played: u64,
    pub victories_num: u64,
    pub penalties_num: u64,
    pub total_reward: Vec<(TokenContractId, Balance)>,
    pub total_affiliate_reward: Vec<(AffiliateId, Balance)>,
}
#[near_bindgen]
impl Contract {
    pub fn get_stats(&self, account_id: &AccountId) -> StatsView {
        let stats = self.internal_get_stats(account_id);
        StatsView { 
            referrer_id: stats.referrer_id, 
            games_played: stats.games_num, 
            victories_num: stats.victories_num, 
            penalties_num: stats.penalties_num, 
            total_reward: stats.total_reward.to_vec(), 
            total_affiliate_reward: stats.total_affiliate_reward.to_vec() 
        }
    }
    pub fn get_user_penalties(&self, account_id: &AccountId) -> UserPenalties {
        let stats = self.internal_get_stats(account_id);
        UserPenalties { penalties_num: stats.penalties_num }
    }
    pub fn get_total_stats_num(&self) -> u32 {
        self.stats.len() as _
    }
    /// return vector of accounts played game even once upon a time
    pub fn get_accounts_played(&self) -> Vec<AccountId> {
        self.stats
            .keys()
            .collect()
    }
}

impl Stats {
    pub fn new(account_id: &AccountId) -> Stats {
        Stats {
            referrer_id: None,
            affiliates: UnorderedSet::new(StorageKey::Affiliates { account_id: account_id.clone() }),
            games_num: 0,
            victories_num: 0,
            penalties_num: 0,
            total_reward: UnorderedMap::new(StorageKey::TotalRewards { account_id: account_id.clone() }),
            total_affiliate_reward: UnorderedMap::new(StorageKey::TotalAffiliateRewards { account_id: account_id.clone() }),
        }
    }
}

impl Contract {
    pub(crate) fn internal_get_stats(&self, account_id: &AccountId) -> Stats {
        if let Some(stats) = self.stats.get(account_id) {
            stats.into()
        } else {
            Stats::new(&account_id)
        }
    }
    pub(crate) fn internal_update_stats(&mut self,
        token_id: Option<&AccountId>,
        account_id: &AccountId,
        action: UpdateStatsAction,
        additional_account_id: Option<AccountId>,
        balance: Option<Balance>
    ) {
            let mut stats = self.internal_get_stats(account_id);
            match action {
                UpdateStatsAction::AddPlayedGame => {
                    stats.games_num += 1;
                },
                UpdateStatsAction::AddReferral => if additional_account_id.is_some() {
                    stats.referrer_id = additional_account_id;
                },
                UpdateStatsAction::AddAffiliate => {
                    if let Some(affiliate_id) = additional_account_id {
                        stats.affiliates.insert(&affiliate_id);
                    }
                },
                UpdateStatsAction::AddWonGame => {
                    stats.victories_num += 1;
                },
                UpdateStatsAction::AddTotalReward => {
                    let token_id = match token_id {
                        Some(id) => id,
                        None => panic!("TokenId for update stats isn't set"),
                    };
                    if let Some(added_balance) = balance {
                        let cur_balance = stats.total_reward.get(token_id).unwrap_or(0);
                        stats.total_reward.insert(token_id, &(cur_balance + added_balance));
                    }
                },
                UpdateStatsAction::AddAffiliateReward => {
                    let token_id = match token_id {
                        Some(id) => id,
                        None => panic!("TokenId for update stats isn't set"),
                    };
                    if let Some(added_balance) = balance {
                        let cur_balance = stats.total_affiliate_reward.get(token_id).unwrap_or(0);
                        stats.total_affiliate_reward.insert(token_id, &(cur_balance + added_balance));
                    }
                },
                UpdateStatsAction::AddPenaltyGame => {
                    stats.penalties_num += 1;
                },
            }
            self.stats.insert(account_id, &stats);
    }
}