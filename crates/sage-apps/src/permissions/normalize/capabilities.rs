use std::collections::BTreeSet;
use anyhow::{anyhow, Result};
use crate::permissions::get_user_capability_definition;
use crate::types::SageRequestedCapabilities;

pub fn normalize_capabilities(
    capabilities: &SageRequestedCapabilities,
) -> Result<SageRequestedCapabilities> {
    let mut required = BTreeSet::new();
    let mut optional = BTreeSet::new();

    for capability in &capabilities.required {
        let definition = get_user_capability_definition(*capability)
            .ok_or_else(|| anyhow!("unknown capability: {}", capability.key()))?;

        if !definition.flags.requestable_by_app {
            return Err(anyhow!(
                "capability is not requestable by apps: {}",
                capability.key()
            ));
        }

        required.insert(*capability);
    }

    for capability in &capabilities.optional {
        let definition = get_user_capability_definition(*capability)
            .ok_or_else(|| anyhow!("unknown capability: {}", capability.key()))?;

        if !definition.flags.requestable_by_app {
            return Err(anyhow!(
                "capability is not requestable by apps: {}",
                capability.key()
            ));
        }

        if !required.contains(capability) {
            optional.insert(*capability);
        }
    }

    Ok(SageRequestedCapabilities {
        required: required.into_iter().collect(),
        optional: optional.into_iter().collect(),
    })
}
