use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;
use std::process::Command;

pub(crate) use crate::architecture_graph::{Edge, Graph, Layer, Package};

const SURFACE_PACKAGES: &[&str] = &["soma-api", "soma-cli", "soma-mcp"];
const APPLICATION_PORT_PACKAGES: &[&str] = &["soma-application", "soma-service"];
const CONCRETE_SHARED_ENGINES: &[&str] = &[
    "soma-codemode",
    "soma-gateway",
    "soma-http-server",
    "soma-openapi",
    "soma-provider-adapters",
];

const TEMPORARY_EXCEPTIONS: &[ArchitectureException] = &[
    ArchitectureException {
        from: "soma-application",
        to: "soma-service",
        owner: "architecture-refactor",
        reason: "strangler migration keeps legacy service behind the new application facade",
        removal_pr: "PR 12: Split soma-service",
        expiration_milestone: "before declaring the application boundary stable",
    },
    ArchitectureException {
        from: "soma-application",
        to: "soma-contracts",
        owner: "architecture-refactor",
        reason: "strangler migration keeps legacy action/config contracts behind the new application facade",
        removal_pr: "PR 13: Split soma-contracts",
        expiration_milestone: "before declaring the application boundary stable",
    },
];

#[derive(Debug)]
struct ArchitectureException {
    from: &'static str,
    to: &'static str,
    owner: &'static str,
    reason: &'static str,
    removal_pr: &'static str,
    expiration_milestone: &'static str,
}

pub fn check(root: &Path) -> Result<()> {
    validate_exceptions()?;
    let metadata = cargo_metadata(root)?;
    let graph = Graph::from_metadata(root, &metadata)?;
    let failures = check_graph(&graph);

    if failures.is_empty() {
        println!(
            "Architecture check passed ({} workspace packages, {} internal edges).",
            graph.packages.len(),
            graph.edges.len()
        );
        return Ok(());
    }

    eprintln!("Architecture check failed:");
    for failure in &failures {
        eprintln!("\n{failure}");
    }
    bail!("architecture boundary check failed")
}

fn cargo_metadata(root: &Path) -> Result<Value> {
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--locked",
            "--all-features",
            "--format-version",
            "1",
        ])
        .current_dir(root)
        .output()
        .context("failed to run cargo metadata")?;
    if !output.status.success() {
        bail!(
            "cargo metadata failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    serde_json::from_slice(&output.stdout).context("cargo metadata emitted invalid JSON")
}

fn check_graph(graph: &Graph) -> Vec<String> {
    let mut failures = Vec::new();
    failures.extend(check_metadata_layers(graph));
    failures.extend(check_direct_edges(graph));
    failures.extend(check_shared_transitive_graphs(graph));
    failures.extend(check_internal_cycles(graph));
    failures
}

fn check_metadata_layers(graph: &Graph) -> Vec<String> {
    graph
        .packages
        .values()
        .filter_map(|package| match package.metadata_layer {
            None => Some(format!(
                "{} ({}) is missing [package.metadata.soma-architecture] layer = {:?}",
                package.name,
                package.rel_path,
                package.layer.as_str()
            )),
            Some(layer) if layer != package.layer => Some(format!(
                "{} ({}) declares architecture layer {:?}, but its path requires {:?}",
                package.name,
                package.rel_path,
                layer.as_str(),
                package.layer.as_str()
            )),
            _ => None,
        })
        .collect()
}

fn check_direct_edges(graph: &Graph) -> Vec<String> {
    let mut failures = Vec::new();
    for edge in graph.edges.iter().filter(|edge| edge.from != edge.to) {
        if is_exception(graph, edge) {
            continue;
        }
        let from = graph.package(&edge.from);
        let to = graph.package(&edge.to);
        if from.layer == Layer::Shared && to.layer != Layer::Shared {
            failures.push(format!(
                "shared package {} ({}) depends on non-shared package {} ({})\n  edge: {}",
                from.name,
                from.rel_path,
                to.name,
                to.rel_path,
                graph.edge_label(edge)
            ));
        }
        failures.extend(check_layer_edge(graph, edge, from, to));
    }
    failures.extend(check_mixed_application_and_engine_edges(graph));
    failures
}

fn check_layer_edge(graph: &Graph, edge: &Edge, from: &Package, to: &Package) -> Vec<String> {
    let mut failures = Vec::new();
    if from.name == "soma-domain"
        && matches!(
            to.layer,
            Layer::App
                | Layer::ProductApplication
                | Layer::ProductIntegration
                | Layer::ProductRuntime
                | Layer::ProductSurface
        )
    {
        failures.push(format!(
            "soma-domain must not depend outward to {}\n  edge: {}",
            to.layer.as_str(),
            graph.edge_label(edge)
        ));
    }

    if from.name == "soma-application"
        && (matches!(
            to.layer,
            Layer::App | Layer::ProductRuntime | Layer::ProductSurface
        ) || CONCRETE_SHARED_ENGINES.contains(&to.name.as_str()))
    {
        failures.push(format!(
            "soma-application must not depend on runtime/surface/app/concrete engines\n  edge: {}",
            graph.edge_label(edge)
        ));
    }

    if SURFACE_PACKAGES.contains(&from.name.as_str())
        && SURFACE_PACKAGES.contains(&to.name.as_str())
    {
        failures.push(format!(
            "surface packages soma-api, soma-cli, and soma-mcp must not depend on one another\n  edge: {}",
            graph.edge_label(edge)
        ));
    }
    failures
}

fn check_mixed_application_and_engine_edges(graph: &Graph) -> Vec<String> {
    graph
        .packages
        .values()
        .filter_map(|package| {
            let deps = graph.direct_dependency_names(&package.id);
            let has_application_port = deps
                .iter()
                .any(|name| APPLICATION_PORT_PACKAGES.contains(&name.as_str()));
            let has_concrete_engine = deps
                .iter()
                .any(|name| CONCRETE_SHARED_ENGINES.contains(&name.as_str()));
            (has_application_port
                && has_concrete_engine
                && !matches!(package.layer, Layer::App | Layer::ProductIntegration))
            .then(|| {
                format!(
                    "{} ({}) depends on both product application ports and concrete shared engines; move that bridge to apps/soma or crates/soma/integrations",
                    package.name, package.rel_path
                )
            })
        })
        .collect()
}

fn check_shared_transitive_graphs(graph: &Graph) -> Vec<String> {
    graph
        .packages
        .values()
        .filter(|package| package.layer == Layer::Shared)
        .filter_map(|package| {
            shortest_path(graph, &package.id, |p| p.layer != Layer::Shared).map(|path| {
                format!(
                    "shared all-features graph for {} reaches a non-shared package\n  path: {}",
                    package.name,
                    graph.path_label(&path)
                )
            })
        })
        .collect()
}

fn check_internal_cycles(graph: &Graph) -> Vec<String> {
    find_cycle(graph)
        .map(|cycle| {
            vec![format!(
                "internal dependency cycle detected: {}",
                cycle.join(" -> ")
            )]
        })
        .unwrap_or_default()
}

fn shortest_path(
    graph: &Graph,
    start: &str,
    predicate: impl Fn(&Package) -> bool,
) -> Option<Vec<usize>> {
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([(start.to_owned(), Vec::<usize>::new())]);
    seen.insert(start.to_owned());
    while let Some((id, path)) = queue.pop_front() {
        for edge_index in graph.edges_by_from.get(&id).into_iter().flatten() {
            let edge = &graph.edges[*edge_index];
            if edge.from == edge.to || is_exception(graph, edge) || !seen.insert(edge.to.clone()) {
                continue;
            }
            let mut next_path = path.clone();
            next_path.push(*edge_index);
            if predicate(graph.package(&edge.to)) {
                return Some(next_path);
            }
            queue.push_back((edge.to.clone(), next_path));
        }
    }
    None
}

fn find_cycle(graph: &Graph) -> Option<Vec<String>> {
    let mut states = BTreeMap::new();
    let mut stack = Vec::new();
    for id in graph.packages.keys() {
        if states.get(id).copied().unwrap_or(0) == 0 {
            if let Some(cycle) = visit_cycle(graph, id, &mut states, &mut stack) {
                return Some(cycle);
            }
        }
    }
    None
}

fn visit_cycle(
    graph: &Graph,
    id: &str,
    states: &mut BTreeMap<String, u8>,
    stack: &mut Vec<String>,
) -> Option<Vec<String>> {
    states.insert(id.to_owned(), 1);
    stack.push(id.to_owned());
    for edge_index in graph.edges_by_from.get(id).into_iter().flatten() {
        let edge = &graph.edges[*edge_index];
        if edge.from == edge.to {
            continue;
        }
        match states.get(&edge.to).copied().unwrap_or(0) {
            0 => {
                if let Some(cycle) = visit_cycle(graph, &edge.to, states, stack) {
                    return Some(cycle);
                }
            }
            1 => return Some(cycle_names(graph, stack, &edge.to)),
            _ => {}
        }
    }
    stack.pop();
    states.insert(id.to_owned(), 2);
    None
}

fn cycle_names(graph: &Graph, stack: &[String], repeated: &str) -> Vec<String> {
    let start = stack.iter().position(|node| node == repeated).unwrap_or(0);
    let mut cycle: Vec<String> = stack[start..]
        .iter()
        .map(|node| graph.package(node).name.clone())
        .collect();
    cycle.push(graph.package(repeated).name.clone());
    cycle
}

fn validate_exceptions() -> Result<()> {
    for exception in TEMPORARY_EXCEPTIONS {
        if exception.owner.is_empty()
            || exception.reason.is_empty()
            || exception.removal_pr.is_empty()
            || exception.expiration_milestone.is_empty()
        {
            bail!(
                "architecture exception {} -> {} is missing owner, reason, removal PR, or expiration milestone",
                exception.from,
                exception.to
            );
        }
    }
    Ok(())
}

fn is_exception(graph: &Graph, edge: &Edge) -> bool {
    let from = &graph.package(&edge.from).name;
    let to = &graph.package(&edge.to).name;
    TEMPORARY_EXCEPTIONS
        .iter()
        .any(|exception| exception.from == from && exception.to == to)
}

#[cfg(test)]
#[path = "architecture_tests.rs"]
mod tests;
