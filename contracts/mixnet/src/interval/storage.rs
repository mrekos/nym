// Copyright 2022 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: Apache-2.0

use cosmwasm_std::{StdResult, Storage};
use cw_storage_plus::{Item, Map};
use mixnet_contract_common::{IdentityKey, Interval, RewardedSetNodeStatus};

// type aliases for better reasoning for storage keys
// (I found it helpful)
type BlockHeight = u64;
type IntervalId = u32;

// TODO: those values need to be verified
pub(crate) const REWARDED_NODE_DEFAULT_PAGE_LIMIT: u32 = 1000;
pub(crate) const REWARDED_NODE_MAX_PAGE_LIMIT: u32 = 1500;

pub(crate) const CURRENT_INTERVAL: Item<Interval> = Item::new("cep");
pub(crate) const CURRENT_REWARDED_SET_HEIGHT: Item<BlockHeight> = Item::new("crh");

// I've changed the `()` data to an `u8` as after serializing `()` is represented as "null",
// taking more space than a single digit u8. If we don't care about what's there, why not go with more efficient approach? : )
pub(crate) const REWARDED_SET_HEIGHTS_FOR_INTERVAL: Map<(IntervalId, BlockHeight), u8> =
    Map::new("rsh");

// pub(crate) const REWARDED_SET: Map<(u64, IdentityKey), NodeStatus> = Map::new("rs");
pub(crate) const REWARDED_SET: Map<(BlockHeight, IdentityKey), RewardedSetNodeStatus> =
    Map::new("rs");

pub(crate) fn save_rewarded_set(
    storage: &mut dyn Storage,
    height: BlockHeight,
    active_set_size: u32,
    entries: Vec<IdentityKey>,
) -> StdResult<()> {
    for (i, identity) in entries.into_iter().enumerate() {
        // first k nodes are active
        let set_status = if i < active_set_size as usize {
            RewardedSetNodeStatus::Active
        } else {
            RewardedSetNodeStatus::Standby
        };

        REWARDED_SET.save(storage, (height, identity), &set_status)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::support::tests::test_helpers;

    #[test]
    fn saving_rewarded_set() {
        let mut deps = test_helpers::init_contract();

        let active_set_size = 100;
        let mut nodes = Vec::new();
        for i in 0..1000 {
            nodes.push(format!("identity{:04}", i))
        }

        save_rewarded_set(deps.as_mut().storage, 1234, active_set_size, nodes).unwrap();

        // first k nodes MUST BE active
        for i in 0..1000 {
            let identity = format!("identity{:04}", i);
            if i < active_set_size {
                assert_eq!(
                    RewardedSetNodeStatus::Active,
                    REWARDED_SET
                        .load(deps.as_ref().storage, (1234, identity))
                        .unwrap()
                )
            } else {
                assert_eq!(
                    RewardedSetNodeStatus::Standby,
                    REWARDED_SET
                        .load(deps.as_ref().storage, (1234, identity))
                        .unwrap()
                )
            }
        }
    }
}
