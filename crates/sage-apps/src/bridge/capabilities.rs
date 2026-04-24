use serde::{Deserialize, Serialize};
use specta::Type;

macro_rules! define_bridge_capabilities {
    (
        $visibility:vis enum $name:ident {
            $(
                $variant:ident => $key:expr
            ),* $(,)?
        }
    ) => {
        #[derive(
            Debug,
            Clone,
            Copy,
            Serialize,
            Deserialize,
            Type,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
        )]
        $visibility enum $name {
            $(
                #[serde(rename = $key)]
                #[specta(rename = $key)]
                $variant,
            )*
        }

        impl $name {
            pub const ALL: &'static [Self] = &[
                $(Self::$variant),*
            ];

            pub fn key(self) -> &'static str {
                match self {
                    $(Self::$variant => $key),*
                }
            }

            pub fn from_key(key: &str) -> Option<Self> {
                match key {
                    $($key => Some(Self::$variant),)*
                    _ => None,
                }
            }
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BridgeCapability {
    User(UserBridgeCapability),
    System(SystemBridgeCapability),
}

define_bridge_capabilities! {
    pub enum UserBridgeCapability {
        PersistentStorage => "persistent_storage",
        BridgeSend => "bridge.send",
        AppGetCapabilities => "app.get_capabilities",
        AppGetInfo => "app.get_info",
        AppLifecycleReadyToStop => "app.lifecycle.ready_to_stop",
        AppLifecycleSetBeforeStopListener => "app.lifecycle.set_before_stop_listener",
        AppRequestCapabilityGrant => "app.request_capability_grant",
        AppRequestNetworkWhitelistGrant => "app.request_network_whitelist_grant",
        WalletSendXch => "wallet.send_xch",
        WalletSendXchAutoSubmit => "wallet.send_xch_auto_submit",
    }
}

define_bridge_capabilities! {
    pub enum SystemBridgeCapability {
        RuntimeManagerListRuntimes => "runtime_manager.list_runtimes",
        RuntimeManagerFocusRuntime => "runtime_manager.focus_runtime",
        RuntimeManagerHideRuntime => "runtime_manager.hide_runtime",
        RuntimeManagerKillRuntime => "runtime_manager.kill_runtime",
        RuntimeManagerListenRuntimesChanged => "runtime_manager.listen_runtimes_changed",
    }
}

impl BridgeCapability {
    pub fn key(self) -> &'static str {
        match self {
            Self::User(capability) => capability.key(),
            Self::System(capability) => capability.key(),
        }
    }
}
