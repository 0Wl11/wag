use cosmwasm_bignumber::Decimal256;
use cosmwasm_std::{
    attr, entry_point, from_binary, to_binary, Binary, CosmosMsg, Decimal, Deps, DepsMut, Env,
    MessageInfo, Response, StdError, StdResult, Uint128, WasmMsg,
};
use cw721::Cw721ReceiveMsg;
use cw721_base::{ExecuteMsg as Cw721BaseExecuteMsg, MintMsg};

use crate::querier::query_token_owner;
use crate::state::{read_holder, store_holder, Config, Holder, TokenInfo, CONFIG, NEW_TOKEN_ID};
use wagmi_protocol::staking::ExecuteMsg::Receive;
use wagmi_protocol::staking::{
    ConfigResponse, Cw721HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, RewardResponse,
};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let config = Config {
        owner: deps.api.addr_canonicalize(info.sender.as_str())?,
        monkeez_nft: deps.api.addr_canonicalize(&msg.monkeez_nft)?,
        kongz_nft: deps.api.addr_canonicalize(&msg.kongz_nft)?,
        reward_nft: deps.api.addr_canonicalize(&msg.reward_nft)?,
    };
    NEW_TOKEN_ID.save(deps.storage, &0u64)?;

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attributes(vec![
        attr("action", "instantiate"),
        attr("owner", info.sender),
        attr("monkeez_nft", &msg.monkeez_nft),
        attr("kongz_nft", &msg.kongz_nft),
        attr("reward_nft", &msg.reward_nft),
    ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        Receive(msg) => receive_cw721(deps, env, info, msg),
        ExecuteMsg::Unstake {
            token_kind,
            token_id,
        } => execute_unstake(deps, env, info, token_kind, token_id),
        ExecuteMsg::ClaimReward {} => execute_claim_reward(deps, env, info),
        ExecuteMsg::Update {
            owner,
            monkeez_nft,
            kongz_nft,
            reward_token,
        } => execute_update(deps, env, info, owner, monkeez_nft, kongz_nft, reward_token),
    }
}

fn receive_cw721(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw721_msg: Cw721ReceiveMsg,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    match from_binary(&cw721_msg.msg) {
        Ok(Cw721HookMsg::Stake {}) => {
            //check staked_nft
            if deps.api.addr_canonicalize(info.sender.as_str())? == config.monkeez_nft {
                return execute_stake(deps, env, info, cw721_msg.sender, cw721_msg.token_id, 0u64);
            } else if deps.api.addr_canonicalize(info.sender.as_str())? == config.kongz_nft {
                return execute_stake(deps, env, info, cw721_msg.sender, cw721_msg.token_id, 1u64);
            } else {
                return Err(StdError::generic_err("unauthorized"));
            }
        }
        _ => Err(StdError::generic_err("missing stake hook")),
    }
}

fn execute_stake(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    sender: String,
    token_id: String,
    nft_kind: u64,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let sender_raw = deps.api.addr_canonicalize(&sender)?;
    let mut holder = read_holder(deps.storage, &sender_raw)?;

    if holder.last_reward_time == 0 {
        holder.last_reward_time = env.block.time.seconds();
    }

    let token_owner = query_token_owner(
        deps.as_ref(),
        config.staked_nft_addr(nft_kind).unwrap(),
        &token_id,
    )?;
    if token_owner != sender {
        return Err(StdError::generic_err("unauthorized"));
    }

    update_reward(&mut holder, env.clone());

    holder.token_ids.push(TokenInfo {
        token_kind: nft_kind,
        token_id: token_id.clone(),
    });

    store_holder(deps.storage, &sender_raw, &holder)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "stake"),
        attr("token_kind", nft_kind.to_string()),
        attr("token_id", token_id.clone()),
    ]))
}

fn execute_unstake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token_kind: u64,
    token_id: String,
) -> StdResult<Response> {
    if token_kind >= 2 {
        return Err(StdError::generic_err("token_kind has only 0 or 1"));
    }
    let sender_raw = deps.api.addr_canonicalize(info.sender.as_str())?;
    let mut holder = read_holder(deps.storage, &sender_raw)?;

    let staked_nft_option = holder
        .token_ids
        .iter()
        .find(|&x| x.is_match(token_kind, &token_id));

    if staked_nft_option.is_none() {
        return Err(StdError::generic_err("Sender must have staked tokenID"));
    }

    update_reward(&mut holder, env);

    holder
        .token_ids
        .retain(|x| !x.is_match(token_kind, &token_id));
    store_holder(deps.storage, &sender_raw, &holder)?;
    Ok(Response::new())
}

fn execute_claim_reward(deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let sender_raw = deps.api.addr_canonicalize(info.sender.as_str())?;
    let mut holder = read_holder(deps.storage, &sender_raw)?;
    update_reward(&mut holder, env);
    let release_reward = holder.last_reward_earned - holder.last_reward_release;
    let mint_num = Uint128::from(1u128) * release_reward;

    let mut msgs = vec![];
    if mint_num > Uint128::zero() {
        holder.last_reward_release =
            holder.last_reward_release + Decimal::from_ratio(mint_num, Uint128::from(1u128));
        // mint
        // config.reward_nft
        let mut new_token_id = NEW_TOKEN_ID.load(deps.storage)?;
        for _ in 0..mint_num.u128() {
            new_token_id += 1;
            msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.addr_humanize(&config.reward_nft)?.to_string(),
                msg: to_binary(&Cw721BaseExecuteMsg::Mint(MintMsg {
                    token_id: format!("baybe_ape_{}", new_token_id).to_string(),
                    owner: info.sender.to_string(),
                    token_uri: None,
                    extension: (),
                }))?,
                funds: vec![],
            }));
        }
        NEW_TOKEN_ID.save(deps.storage, &new_token_id)?;
    }
    store_holder(deps.storage, &sender_raw, &holder)?;
    Ok(Response::new().add_messages(msgs).add_attributes(vec![
        attr("action", "claim_reward"),
        attr("reward_num", mint_num),
    ]))
}

fn execute_update(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    owner: Option<String>,
    monkeez_nft: Option<String>,
    kongz_nft: Option<String>,
    reward_token: Option<String>,
) -> StdResult<Response> {
    let mut config = CONFIG.load(deps.storage)?;
    if config.owner != deps.api.addr_canonicalize(info.sender.as_str())? {
        return Err(StdError::generic_err("unauthorized"));
    }
    let mut attr_vec = vec![];
    attr_vec.push(attr("action", "update"));
    if owner.is_some() {
        config.owner = deps
            .api
            .addr_canonicalize(owner.clone().unwrap().as_str())?;
        attr_vec.push(attr("owner", owner.clone().unwrap()));
    }
    if monkeez_nft.is_some() {
        config.monkeez_nft = deps
            .api
            .addr_canonicalize(monkeez_nft.clone().unwrap().as_str())?;
        attr_vec.push(attr("monkeez_nft", monkeez_nft.clone().unwrap()));
    }
    if kongz_nft.is_some() {
        config.kongz_nft = deps
            .api
            .addr_canonicalize(kongz_nft.clone().unwrap().as_str())?;
        attr_vec.push(attr("kongz_nft", kongz_nft.clone().unwrap()));
    }
    if reward_token.is_some() {
        config.reward_nft = deps
            .api
            .addr_canonicalize(reward_token.clone().unwrap().as_str())?;
        attr_vec.push(attr("reward_token", reward_token.clone().unwrap()));
    }
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attributes(attr_vec))
}

fn update_reward(holder: &mut Holder, env: Env) {
    let secs_need_reward = staking_time(&holder.token_ids).unwrap();
    let diff_sec = env.block.time.seconds() - holder.last_reward_time;
    let reward = Decimal::from_ratio(Uint128::from(diff_sec), Uint128::from(secs_need_reward));
    holder.last_reward_time = env.block.time.seconds();
    holder.last_reward_earned = holder.last_reward_earned + reward;
}

fn staking_time(token_list: &Vec<TokenInfo>) -> Option<u64> {
    let one_for_monkeez: u64 = 84 * 86400; //84 days
    let one_for_kongz: u64 = 126 * 86400; // 126 days
    let staked_count: u64 = token_list.len() as u64;
    if staked_count == 1 {
        return match token_list[0].token_kind {
            0 => Some(one_for_monkeez),
            1 => Some(one_for_kongz),
            _ => None,
        };
    }
    let monkeez_count: u64 = token_list.iter().filter(|&x| x.token_kind == 0).count() as u64;
    let kongz_count: u64 = token_list.iter().filter(|&x| x.token_kind == 1).count() as u64;

    // ((84 * X/staked_num+ 126 * Y/staked_num)/staked_num) * (1 - 0.1 *(staked_num -1))

    // t1 = 84*X/staked_num + 126*Y/staked_num
    let t1 = Decimal::from_ratio(
        Uint128::from(one_for_monkeez * monkeez_count),
        Uint128::from(staked_count),
    ) + Decimal::from_ratio(
        Uint128::from(one_for_kongz * kongz_count),
        Uint128::from(staked_count),
    );

    // t2 = t1 / staked_num
    let t2: Decimal256 = Decimal256::from(t1)
        / Decimal256::from(Decimal::from_ratio(
            Uint128::from(staked_count),
            Uint128::from(1u128),
        ));

    //1 - (staked_num -1) /10
    let k = Decimal::one()
        - Decimal::from_ratio(Uint128::from(staked_count - 1), Uint128::from(10u128));
    if k.is_zero() {
        return Some(86400);
    }
    let t3 = t2 * Decimal256::from(k);
    let t4 = Decimal::from(t3) * Uint128::from(1u128);
    Some(t4.u128() as u64)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::Reward { staker } => to_binary(&query_reward(deps, env, staker)?),
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;

    Ok(ConfigResponse {
        owner: deps.api.addr_humanize(&config.owner)?.to_string(),
        monkeez_nft: deps.api.addr_humanize(&config.monkeez_nft)?.to_string(),
        kongz_nft: deps.api.addr_humanize(&config.kongz_nft)?.to_string(),
        reward_nft: deps.api.addr_humanize(&config.reward_nft)?.to_string(),
    })
}

fn query_reward(deps: Deps, env: Env, staker: String) -> StdResult<RewardResponse> {
    let staker_raw = deps.api.addr_canonicalize(staker.as_str())?;
    let mut holder = read_holder(deps.storage, &staker_raw)?;
    update_reward(&mut holder, env);
    Ok(RewardResponse {
        reward_amount: Uint128::from(1u128)
            * (holder.last_reward_release - holder.last_reward_earned),
    })
}
