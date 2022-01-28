use cosmwasm_std::Uint128;
use cw721::Cw721ReceiveMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub monkeez_nft: String,
    pub kongz_nft: String,
    pub reward_nft: String, //NFT token contract
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Receive(Cw721ReceiveMsg),
    Unstake {
        token_kind: u64, // 0: monkeez, 1: kongz
        token_id: String,
    },
    ClaimReward {},
    Update {
        owner: Option<String>,
        monkeez_nft: Option<String>,
        kongz_nft: Option<String>,
        reward_token: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw721HookMsg {
    Stake {},
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    Reward { staker: String },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct ConfigResponse {
    pub owner: String,
    pub monkeez_nft: String,
    pub kongz_nft: String,
    pub reward_nft: String,
}
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct RewardResponse {
    pub reward_amount: Uint128,
}
