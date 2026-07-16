use soma_domain::{AuthorizationMode, Confirmation, Principal, RequestId, Surface, TraceContext};

#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub principal: Option<Principal>,
    pub authorization_mode: AuthorizationMode,
    pub surface: Surface,
    pub trace: Option<TraceContext>,
    pub destructive_confirmation: Confirmation,
    pub response_limit: Option<usize>,
    pub request_id: RequestId,
}

impl ExecutionContext {
    pub fn loopback(surface: Surface, request_id: RequestId) -> Self {
        Self {
            principal: None,
            authorization_mode: AuthorizationMode::LoopbackDev,
            surface,
            trace: None,
            destructive_confirmation: Confirmation::Missing,
            response_limit: None,
            request_id,
        }
    }
}
