use crate::gateway::manager::GatewayManager;
use crate::gateway::protected_routes::{project_route, ProtectedRouteProjection};
use crate::upstream::UpstreamHealth;

impl GatewayManager {
    pub fn protected_route_projections(&self) -> Vec<ProtectedRouteProjection> {
        let config = self.config.read().expect("gateway config poisoned");
        let snapshots = self.discover().unwrap_or_default();
        config
            .protected_mcp_routes
            .iter()
            .map(|route| {
                let upstream_connected = route.upstream.as_deref().is_some_and(|name| {
                    snapshots.iter().any(|snapshot| {
                        snapshot.name == name && snapshot.health == UpstreamHealth::Connected
                    })
                });
                project_route(route, upstream_connected)
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "protected_routes_tests.rs"]
mod tests;
