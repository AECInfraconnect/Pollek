pub use pollek_contract::PollekBundleEnvelopeV1;
pub use pollek_contract::PollekBundleSignatureV1 as BundleSignature;
pub use pollek_contract::PollekPolicyBundleManifestV2;
pub use pollek_contract::PollekPolicyBundleManifestV2ArtifactsItem as BundleArtifactV2;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub enum ActivationStrategy {
    AtomicAllOrNothing,
    AdapterByAdapterWithRollback,
    ShadowOnly,
}
