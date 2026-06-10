use std::time::Instant;

#[derive(Debug)]
pub struct HostState {
    pub tenant_id: String,
    pub plugin_id: String,
    pub version: String,
    pub request_id: String,
    pub deadline: Instant,
    pub dirty: bool,
}
