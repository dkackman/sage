use crate::types::SageApp;

use super::{
    AppLaunchGateResult, SandboxCapability, SandboxCapabilityStatus, SandboxState,
};

fn capability_status(
    state: &SandboxState,
    capability: SandboxCapability,
) -> SandboxCapabilityStatus {
    match capability {
        SandboxCapability::StorageIsolationFromSage => {
            state.storage_isolation_from_sage.status
        }
        SandboxCapability::StoragePersistenceNormal => {
            state.storage_persistence_normal.status
        }
        SandboxCapability::StorageNonPersistenceIncognito => {
            state.storage_non_persistence_incognito.status
        }
        SandboxCapability::StorageClearCycle => state.storage_clear_cycle.status,
        SandboxCapability::NetworkAllowlistEnforced => {
            state.network_allowlist_enforced.status
        }
    }
}

fn app_requires_sandbox_gate(app: &SageApp) -> bool {
    !app.id().starts_with("__sage_test_")
}

pub fn evaluate_app_launch_gate(
    app: &SageApp,
    effective: &SandboxState,
) -> AppLaunchGateResult {
    if !app_requires_sandbox_gate(app) {
        return AppLaunchGateResult {
            allowed: true,
            kind: "allowed".into(),
            capability: None,
            message: None,
        };
    }

    let critical_caps = [
        SandboxCapability::StorageIsolationFromSage,
        SandboxCapability::StorageClearCycle,
        SandboxCapability::NetworkAllowlistEnforced,
    ];

    for capability in critical_caps {
        match capability_status(effective, capability) {
            SandboxCapabilityStatus::Passed => {}
            SandboxCapabilityStatus::Pending | SandboxCapabilityStatus::Running => {
                return AppLaunchGateResult {
                    allowed: false,
                    kind: "sandboxPending".into(),
                    capability: Some(capability),
                    message: Some(
                        "Apps are allowed to launch only when all required sandbox capabilities have passed."
                            .into(),
                    ),
                };
            }
            SandboxCapabilityStatus::Failed => {
                return AppLaunchGateResult {
                    allowed: false,
                    kind: "sandboxFailed".into(),
                    capability: Some(capability),
                    message: Some(
                        "Apps are blocked because a required sandbox capability failed."
                            .into(),
                    ),
                };
            }
        }
    }

    AppLaunchGateResult {
        allowed: true,
        kind: "allowed".into(),
        capability: None,
        message: None,
    }
}
