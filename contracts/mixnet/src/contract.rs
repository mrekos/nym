// Copyright 2021 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: Apache-2.0

use crate::constants::{
    ACTIVE_SET_WORK_FACTOR, INTERVAL_REWARD_PERCENT, REWARDING_INTERVAL_LENGTH,
    SYBIL_RESISTANCE_PERCENT,
};
use crate::delegations::queries::query_all_network_delegations_paged;
use crate::delegations::queries::query_delegator_delegations_paged;
use crate::delegations::queries::query_mixnode_delegation;
use crate::delegations::queries::query_mixnode_delegations_paged;
use crate::error::ContractError;
use crate::gateways::queries::query_gateways_paged;
use crate::gateways::queries::query_owns_gateway;
use crate::interval::queries::{
    query_current_interval, query_current_rewarded_set_height, query_rewarded_set,
    query_rewarded_set_heights_for_interval, query_rewarded_set_refresh_minimum_blocks,
    query_rewarded_set_update_details,
};
use crate::interval::storage as interval_storage;
use crate::mixnet_contract_settings::models::ContractState;
use crate::mixnet_contract_settings::queries::{
    query_contract_settings_params, query_contract_version,
};
use crate::mixnet_contract_settings::storage as mixnet_params_storage;
use crate::mixnodes::bonding_queries as mixnode_queries;
use crate::mixnodes::bonding_queries::query_mixnodes_paged;
use crate::mixnodes::layer_queries::query_layer_distribution;
use crate::rewards::queries::{
    query_circulating_supply, query_reward_pool, query_rewarding_status,
};
use crate::rewards::storage as rewards_storage;
use cosmwasm_std::{
    entry_point, to_binary, Addr, Deps, DepsMut, Env, MessageInfo, QueryResponse, Response, Uint128,
};
use mixnet_contract_common::{
    ContractStateParams, ExecuteMsg, InstantiateMsg, Interval, MigrateMsg, QueryMsg,
};
use time::OffsetDateTime;

/// Constant specifying minimum of coin required to bond a gateway
pub const INITIAL_GATEWAY_PLEDGE: Uint128 = Uint128::new(100_000_000);

/// Constant specifying minimum of coin required to bond a mixnode
pub const INITIAL_MIXNODE_PLEDGE: Uint128 = Uint128::new(100_000_000);

pub const INITIAL_MIXNODE_REWARDED_SET_SIZE: u32 = 200;
pub const INITIAL_MIXNODE_ACTIVE_SET_SIZE: u32 = 100;

pub const INITIAL_REWARD_POOL: u128 = 250_000_000_000_000;
pub const INITIAL_ACTIVE_SET_WORK_FACTOR: u8 = 10;

pub const DEFAULT_FIRST_INTERVAL_START: OffsetDateTime =
    time::macros::datetime!(2022-01-01 12:00 UTC);

fn default_initial_state(owner: Addr, rewarding_validator_address: Addr) -> ContractState {
    ContractState {
        owner,
        rewarding_validator_address,
        params: ContractStateParams {
            minimum_mixnode_pledge: INITIAL_MIXNODE_PLEDGE,
            minimum_gateway_pledge: INITIAL_GATEWAY_PLEDGE,
            mixnode_rewarded_set_size: INITIAL_MIXNODE_REWARDED_SET_SIZE,
            mixnode_active_set_size: INITIAL_MIXNODE_ACTIVE_SET_SIZE,
        },
    }
}

/// Instantiate the contract.
///
/// `deps` contains Storage, API and Querier
/// `env` contains block, message and contract info
/// `msg` is the contract initialization message, sort of like a constructor call.
#[entry_point]
pub fn instantiate(
    deps: DepsMut<'_>,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let rewarding_validator_address = deps.api.addr_validate(&msg.rewarding_validator_address)?;
    let state = default_initial_state(info.sender, rewarding_validator_address);
    let rewarding_interval =
        Interval::new(0, DEFAULT_FIRST_INTERVAL_START, REWARDING_INTERVAL_LENGTH);

    mixnet_params_storage::CONTRACT_STATE.save(deps.storage, &state)?;
    mixnet_params_storage::LAYERS.save(deps.storage, &Default::default())?;
    rewards_storage::REWARD_POOL.save(deps.storage, &Uint128::new(INITIAL_REWARD_POOL))?;
    interval_storage::CURRENT_INTERVAL.save(deps.storage, &rewarding_interval)?;
    interval_storage::CURRENT_REWARDED_SET_HEIGHT.save(deps.storage, &env.block.height)?;

    Ok(Response::default())
}

/// Handle an incoming message
#[entry_point]
pub fn execute(
    deps: DepsMut<'_>,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::BondMixnode {
            mix_node,
            owner_signature,
        } => crate::mixnodes::transactions::try_add_mixnode(
            deps,
            env,
            info,
            mix_node,
            owner_signature,
        ),
        ExecuteMsg::UnbondMixnode {} => {
            crate::mixnodes::transactions::try_remove_mixnode(deps, info)
        }
        ExecuteMsg::UpdateMixnodeConfig {
            profit_margin_percent,
        } => crate::mixnodes::transactions::try_update_mixnode_config(
            deps,
            env,
            info,
            profit_margin_percent,
        ),
        ExecuteMsg::UpdateMixnodeConfigOnBehalf {
            profit_margin_percent,
            owner,
        } => crate::mixnodes::transactions::try_update_mixnode_config_on_behalf(
            deps,
            env,
            info,
            profit_margin_percent,
            owner,
        ),
        ExecuteMsg::BondGateway {
            gateway,
            owner_signature,
        } => crate::gateways::transactions::try_add_gateway(
            deps,
            env,
            info,
            gateway,
            owner_signature,
        ),
        ExecuteMsg::UnbondGateway {} => {
            crate::gateways::transactions::try_remove_gateway(deps, info)
        }
        ExecuteMsg::UpdateContractStateParams(params) => {
            crate::mixnet_contract_settings::transactions::try_update_contract_settings(
                deps, info, params,
            )
        }
        ExecuteMsg::RewardMixnode {
            identity,
            params,
            interval_id,
        } => crate::rewards::transactions::try_reward_mixnode(
            deps,
            env,
            info,
            identity,
            params,
            interval_id,
        ),
        ExecuteMsg::DelegateToMixnode { mix_identity } => {
            crate::delegations::transactions::try_delegate_to_mixnode(deps, env, info, mix_identity)
        }
        ExecuteMsg::UndelegateFromMixnode { mix_identity } => {
            crate::delegations::transactions::try_remove_delegation_from_mixnode(
                deps,
                info,
                mix_identity,
            )
        }
        ExecuteMsg::RewardNextMixDelegators {
            mix_identity,
            interval_id,
        } => crate::rewards::transactions::try_reward_next_mixnode_delegators(
            deps,
            info,
            mix_identity,
            interval_id,
        ),
        ExecuteMsg::DelegateToMixnodeOnBehalf {
            mix_identity,
            delegate,
        } => crate::delegations::transactions::try_delegate_to_mixnode_on_behalf(
            deps,
            env,
            info,
            mix_identity,
            delegate,
        ),
        ExecuteMsg::UndelegateFromMixnodeOnBehalf {
            mix_identity,
            delegate,
        } => crate::delegations::transactions::try_remove_delegation_from_mixnode_on_behalf(
            deps,
            info,
            mix_identity,
            delegate,
        ),
        ExecuteMsg::BondMixnodeOnBehalf {
            mix_node,
            owner,
            owner_signature,
        } => crate::mixnodes::transactions::try_add_mixnode_on_behalf(
            deps,
            env,
            info,
            mix_node,
            owner,
            owner_signature,
        ),
        ExecuteMsg::UnbondMixnodeOnBehalf { owner } => {
            crate::mixnodes::transactions::try_remove_mixnode_on_behalf(deps, info, owner)
        }
        ExecuteMsg::BondGatewayOnBehalf {
            gateway,
            owner,
            owner_signature,
        } => crate::gateways::transactions::try_add_gateway_on_behalf(
            deps,
            env,
            info,
            gateway,
            owner,
            owner_signature,
        ),
        ExecuteMsg::UnbondGatewayOnBehalf { owner } => {
            crate::gateways::transactions::try_remove_gateway_on_behalf(deps, info, owner)
        }
        ExecuteMsg::WriteRewardedSet {
            rewarded_set,
            expected_active_set_size,
        } => crate::interval::transactions::try_write_rewarded_set(
            deps,
            env,
            info,
            rewarded_set,
            expected_active_set_size,
        ),
        ExecuteMsg::AdvanceCurrentInterval {} => {
            crate::interval::transactions::try_advance_interval(env, deps.storage)
        }
    }
}

#[entry_point]
pub fn query(deps: Deps<'_>, env: Env, msg: QueryMsg) -> Result<QueryResponse, ContractError> {
    let query_res = match msg {
        QueryMsg::GetContractVersion {} => to_binary(&query_contract_version()),
        QueryMsg::GetMixNodes { start_after, limit } => {
            to_binary(&query_mixnodes_paged(deps, start_after, limit)?)
        }
        QueryMsg::GetGateways { limit, start_after } => {
            to_binary(&query_gateways_paged(deps, start_after, limit)?)
        }
        QueryMsg::OwnsMixnode { address } => {
            to_binary(&mixnode_queries::query_owns_mixnode(deps, address)?)
        }
        QueryMsg::OwnsGateway { address } => to_binary(&query_owns_gateway(deps, address)?),
        QueryMsg::StateParams {} => to_binary(&query_contract_settings_params(deps)?),
        QueryMsg::LayerDistribution {} => to_binary(&query_layer_distribution(deps)?),
        QueryMsg::GetMixnodeDelegations {
            mix_identity,
            start_after,
            limit,
        } => to_binary(&query_mixnode_delegations_paged(
            deps,
            mix_identity,
            start_after,
            limit,
        )?),
        QueryMsg::GetAllNetworkDelegations { start_after, limit } => to_binary(
            &query_all_network_delegations_paged(deps, start_after, limit)?,
        ),
        QueryMsg::GetDelegatorDelegations {
            delegator: delegation_owner,
            start_after,
            limit,
        } => to_binary(&query_delegator_delegations_paged(
            deps,
            delegation_owner,
            start_after,
            limit,
        )?),
        QueryMsg::GetDelegationDetails {
            mix_identity,
            delegator,
        } => to_binary(&query_mixnode_delegation(deps, mix_identity, delegator)?),
        QueryMsg::GetRewardPool {} => to_binary(&query_reward_pool(deps)?),
        QueryMsg::GetCirculatingSupply {} => to_binary(&query_circulating_supply(deps)?),
        QueryMsg::GetIntervalRewardPercent {} => to_binary(&INTERVAL_REWARD_PERCENT),
        QueryMsg::GetSybilResistancePercent {} => to_binary(&SYBIL_RESISTANCE_PERCENT),
        QueryMsg::GetActiveSetWorkFactor {} => to_binary(&ACTIVE_SET_WORK_FACTOR),
        QueryMsg::GetRewardingStatus {
            mix_identity,
            interval_id,
        } => to_binary(&query_rewarding_status(deps, mix_identity, interval_id)?),
        QueryMsg::GetRewardedSet {
            height,
            start_after,
            limit,
        } => to_binary(&query_rewarded_set(
            deps.storage,
            height,
            start_after,
            limit,
        )?),
        QueryMsg::GetRewardedSetHeightsForInterval { interval_id } => to_binary(
            &query_rewarded_set_heights_for_interval(deps.storage, interval_id)?,
        ),
        QueryMsg::GetRewardedSetUpdateDetails {} => {
            to_binary(&query_rewarded_set_update_details(env, deps.storage)?)
        }
        QueryMsg::GetCurrentRewardedSetHeight {} => {
            to_binary(&query_current_rewarded_set_height(deps.storage)?)
        }
        QueryMsg::GetCurrentInterval {} => to_binary(&query_current_interval(deps.storage)?),
        QueryMsg::GetRewardedSetRefreshBlocks {} => {
            to_binary(&query_rewarded_set_refresh_minimum_blocks())
        }
    };

    Ok(query_res?)
}
#[entry_point]
pub fn migrate(deps: DepsMut<'_>, env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    use cw_storage_plus::Item;
    use serde::{Deserialize, Serialize};

    // needed migration:
    /*
       1. removal of rewarding_interval_starting_block field from ContractState
       2. removal of latest_rewarding_interval_nonce field from ContractState
       3. removal of rewarding_in_progress field from ContractState
       4. interval_storage::CURRENT_INTERVAL.save(deps.storage, &Interval::default())?;
       5. interval_storage::CURRENT_REWARDED_SET_HEIGHT.save(deps.storage, &env.block.height)?;
       6. removal of active_set_work_factor fields from ContractStateParams
    */

    #[derive(Serialize, Deserialize)]
    pub struct OldContractStateParams {
        pub minimum_mixnode_pledge: Uint128,
        pub minimum_gateway_pledge: Uint128,
        pub mixnode_rewarded_set_size: u32,
        pub mixnode_active_set_size: u32,
        pub active_set_work_factor: u8,
    }

    #[derive(Serialize, Deserialize)]
    struct OldContractState {
        pub owner: Addr, // only the owner account can update state
        pub rewarding_validator_address: Addr,
        pub params: OldContractStateParams,
        pub rewarding_interval_starting_block: u64,
        pub latest_rewarding_interval_nonce: u32,
        pub rewarding_in_progress: bool,
    }

    let old_contract_state: Item<'_, OldContractState> = Item::new("config");

    let old_state = old_contract_state.load(deps.storage)?;

    let new_params = mixnet_contract_common::ContractStateParams {
        minimum_mixnode_pledge: old_state.params.minimum_mixnode_pledge,
        minimum_gateway_pledge: old_state.params.minimum_mixnode_pledge,
        mixnode_rewarded_set_size: old_state.params.mixnode_rewarded_set_size,
        mixnode_active_set_size: old_state.params.mixnode_active_set_size,
    };

    let new_state = crate::mixnet_contract_settings::models::ContractState {
        owner: old_state.owner,
        rewarding_validator_address: old_state.rewarding_validator_address,
        params: new_params,
    };
    let rewarding_interval =
        Interval::new(0, DEFAULT_FIRST_INTERVAL_START, REWARDING_INTERVAL_LENGTH);

    mixnet_params_storage::CONTRACT_STATE.save(deps.storage, &new_state)?;

    interval_storage::CURRENT_INTERVAL.save(deps.storage, &rewarding_interval)?;
    interval_storage::CURRENT_REWARDED_SET_HEIGHT.save(deps.storage, &env.block.height)?;

    Ok(Default::default())
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::support::tests;
    use config::defaults::DENOM;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};
    use mixnet_contract_common::PagedMixnodeResponse;

    #[test]
    fn initialize_contract() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let msg = InstantiateMsg {
            rewarding_validator_address: config::defaults::DEFAULT_REWARDING_VALIDATOR.to_string(),
        };
        let info = mock_info("creator", &[]);

        let res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // mix_node_bonds should be empty after initialization
        let res = query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::GetMixNodes {
                start_after: None,
                limit: Option::from(2),
            },
        )
        .unwrap();
        let page: PagedMixnodeResponse = from_binary(&res).unwrap();
        assert_eq!(0, page.nodes.len()); // there are no mixnodes in the list when it's just been initialized

        // Contract balance should match what we initialized it as
        assert_eq!(
            coins(0, DENOM),
            tests::queries::query_contract_balance(env.contract.address, deps)
        );
    }
}
