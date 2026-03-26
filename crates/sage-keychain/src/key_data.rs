use serde::{Deserialize, Serialize};
use serde_with::{Bytes, serde_as};

use crate::encrypt::Encrypted;

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(u8)]
pub enum KeyData {
    Public {
        #[serde_as(as = "Bytes")]
        master_pk: [u8; 48],
    },
    Secret {
        #[serde_as(as = "Bytes")]
        master_pk: [u8; 48],
        entropy: bool,
        encrypted: Encrypted,
        password_protected: bool,
    },
    PasskeyProtected {
        #[serde_as(as = "Bytes")]
        master_pk: [u8; 48],
        entropy: bool,
        encrypted: Encrypted,
        #[serde_as(as = "Bytes")]
        credential_id: Vec<u8>,
        #[serde_as(as = "Bytes")]
        prf_salt: [u8; 32],
    },
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretKeyData(#[serde_as(as = "Bytes")] pub Vec<u8>);
