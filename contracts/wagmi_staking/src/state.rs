use cosmwasm_std::{CanonicalAddr, Decimal, StdResult, Storage};
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONFIG: Item<Config> = Item::new("config");
pub const HOLDERS: Map<&[u8], Holder> = Map::new("holders");
pub const NEW_TOKEN_ID: Item<u64> = Item::new("new_token_id");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub monkeez_nft: CanonicalAddr,
    pub kongz_nft: CanonicalAddr,
    pub reward_nft: CanonicalAddr,
}
impl Config {
    pub fn staked_nft_addr(&self, selector: u64) -> Option<&CanonicalAddr> {
        match selector {
            0 => Some(&self.monkeez_nft),
            1 => Some(&self.kongz_nft),
            _ => None,
        }
    }
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TokenInfo {
    pub token_kind: u64,
    pub token_id: String,
}

impl TokenInfo {
    pub fn is_match(&self, token_kind: u64, token_id: &String) -> bool {
        self.token_kind == token_kind && self.token_id.as_str() == token_id.as_str()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Holder {
    pub token_ids: Vec<TokenInfo>,
    pub last_reward_time: u64,
    pub last_reward_earned: Decimal,
    pub last_reward_release: Decimal,
}

pub fn store_holder(
    storage: &mut dyn Storage,
    holder_address: &CanonicalAddr,
    holder: &Holder,
) -> StdResult<()> {
    HOLDERS.save(storage, holder_address.as_slice(), holder)
}

pub fn read_holder(storage: &dyn Storage, holder_address: &CanonicalAddr) -> StdResult<Holder> {
    let res = HOLDERS.may_load(storage, holder_address.as_slice())?;
    match res {
        Some(holder) => Ok(holder),
        None => Ok(Holder {
            token_ids: vec![],
            last_reward_time: 0u64,
            last_reward_earned: Decimal::zero(),
            last_reward_release: Decimal::zero(),
        }),
    }
}
