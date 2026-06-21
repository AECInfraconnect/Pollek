pub use pollen_contract::PollenBundleEnvelopeV1;
pub use pollen_contract::PollenBundleSignatureV1 as BundleSignature;
pub use pollen_contract::PollenPolicyBundleManifestV2;
pub use pollen_contract::PollenPolicyBundleManifestV2ArtifactsItem as BundleArtifactV2;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub enum ActivationStrategy {
    AtomicAllOrNothing,
    AdapterByAdapterWithRollback,
    ShadowOnly,
}
