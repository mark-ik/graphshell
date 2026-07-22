//! Traits application adapters implement beside their own source truth.

use graphshell_protocol::{
    IntentInvocation, IntentResult, ProjectionRequest, ProjectionSnapshot, ResourceRequest,
    ResourceResponse,
};

/// The read boundary. Implementations authorize selection before they disclose
/// a score or scene and retain ownership of native source data.
pub trait ProjectionSource {
    type Error;

    fn snapshot(&mut self, request: ProjectionRequest) -> Result<ProjectionSnapshot, Self::Error>;
}

/// The presentation-resource boundary. Resource authorization remains
/// endpoint-side and is evaluated independently of scene disclosure.
pub trait PresentationSource {
    type Error;

    fn resource(&mut self, request: ResourceRequest) -> Result<ResourceResponse, Self::Error>;
}

/// The write boundary. Implementations validate revision and authority before
/// lowering an intent into a product-specific action.
pub trait IntentSink {
    type Error;

    fn invoke(&mut self, intent: IntentInvocation) -> Result<IntentResult, Self::Error>;
}
