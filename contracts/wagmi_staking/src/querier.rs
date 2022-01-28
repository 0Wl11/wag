use cosmwasm_std::{to_binary, CanonicalAddr, Deps, QueryRequest, StdResult, WasmQuery};
use cw721::{Cw721QueryMsg, OwnerOfResponse};

pub fn query_token_owner(
    deps: Deps,
    contract_addr: &CanonicalAddr,
    token_id: &String,
) -> StdResult<String> {
    let owner: OwnerOfResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: deps.api.addr_humanize(contract_addr).unwrap().to_string(),
        msg: to_binary(&Cw721QueryMsg::OwnerOf {
            token_id: token_id.to_string(),
            include_expired: None,
        })?,
    }))?;
    Ok(owner.owner)
}
