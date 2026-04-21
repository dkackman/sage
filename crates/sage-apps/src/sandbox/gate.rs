use crate::types::InstalledSageApp;

use super::types::{
    AppLaunchGateResult, SandboxCapability, SandboxCapabilityStatus, SandboxState,
    cap_result,
};

fn cap_label(cap: SandboxCapability) -> &'static str {
    match cap {
        SandboxCapability::StorageIsolationFromSage => "storage isolation from Sage",
        SandboxCapability::StoragePersistenceNormal => "persistent storage behavior",
        SandboxCapability::StorageNonPersistenceIncognito => "incognito storage behavior",
        SandboxCapability::StorageClearCycle => "storage clear cycle behavior",
        SandboxCapability::NetworkAllowlistEnforced => "network allowlist enforcement",
    }
}

pub fn get_required_sandbox_capabilities(
    app: &InstalledSageApp,
) -> Vec<SandboxCapability> {
    let mut required = vec![SandboxCapability::StorageIsolationFromSage];

    if app
        .granted_permissions
        .capabilities
        .iter()
        .any(|cap| cap == "persistent_storage")
    {
        required.push(SandboxCapability::StoragePersistenceNormal);
    } else {
        required.push(SandboxCapability::StorageNonPersistenceIncognito);
    }

    if !app.granted_permissions.network.whitelist.is_empty() {
        required.push(SandboxCapability::NetworkAllowlistEnforced);
    }

    required
}

pub fn evaluate_app_launch_gate(
    app: &InstalledSageApp,
    sandbox: &SandboxState,
) -> AppLaunchGateResult {
    let isolation = &sandbox.storage_isolation_from_sage;

    if matches!(
        isolation.status,
        SandboxCapabilityStatus::Pending | SandboxCapabilityStatus::Running
    ) {
        return AppLaunchGateResult {
            allowed: false,
            kind: "running".into(),
            capability: Some(SandboxCapability::StorageIsolationFromSage),
            message: Some(format!(
                "Sandbox tests are still running for {}.",
                cap_label(SandboxCapability::StorageIsolationFromSage)
            )),
        };
    }

    if isolation.status == SandboxCapabilityStatus::Failed {
        return AppLaunchGateResult {
            allowed: false,
            kind: "failed".into(),
            capability: Some(SandboxCapability::StorageIsolationFromSage),
            message: isolation.details.clone().or_else(|| {
                Some(format!(
                    "Sandbox test failed for {}.",
                    cap_label(SandboxCapability::StorageIsolationFromSage)
                ))
            }),
        };
    }

    for cap in get_required_sandbox_capabilities(app) {
        let result = cap_result(sandbox, cap);

        if matches!(
            result.status,
            SandboxCapabilityStatus::Pending | SandboxCapabilityStatus::Running
        ) {
            return AppLaunchGateResult {
                allowed: false,
                kind: "running".into(),
                capability: Some(cap),
                message: Some(format!(
                    "Sandbox tests are still running for {}.",
                    cap_label(cap)
                )),
            };
        }

        if result.status == SandboxCapabilityStatus::Failed {
            return AppLaunchGateResult {
                allowed: false,
                kind: "failed".into(),
                capability: Some(cap),
                message: result.details.clone().or_else(|| {
                    Some(format!("Sandbox test failed for {}.", cap_label(cap)))
                }),
            };
        }
    }

    AppLaunchGateResult {
        allowed: true,
        kind: "allowed".into(),
        capability: None,
        message: None,
    }
}
