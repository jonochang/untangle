use crate::analysis_context::resolve_project_root;
use crate::analysis_report::build_analysis_snapshot;
use crate::config::ResolvedConfig;
use crate::errors::Result;
use crate::output::json::build_hotspots;
use crate::quality::{UntangleHotspot, UntangleMetricSummary};
use std::path::Path;

pub fn compute_untangle_summary(
    root: &Path,
    config: &ResolvedConfig,
    hotspot_limit: usize,
) -> Result<UntangleMetricSummary> {
    let project_root = resolve_project_root(root, config.lang);
    let snapshot = build_analysis_snapshot(root, &project_root, config, true)?;
    let hotspots = build_hotspots(&snapshot.graph, &snapshot.sccs, Some(hotspot_limit))
        .into_iter()
        .map(|hotspot| UntangleHotspot {
            path: snapshot
                .graph
                .node_indices()
                .find(|&idx| snapshot.graph[idx].name == hotspot.node)
                .map(|idx| snapshot.graph[idx].path.clone())
                .unwrap_or_default(),
            module: hotspot.node,
            fanout: hotspot.fanout,
            fanin: hotspot.fanin,
            scc: hotspot.scc_id,
        })
        .collect();

    Ok(UntangleMetricSummary {
        nodes: snapshot.metadata.node_count,
        edges: snapshot.metadata.edge_count,
        edge_density: snapshot.metadata.edge_density,
        files_parsed: snapshot.metadata.files_parsed,
        summary: snapshot.summary,
        hotspots,
    })
}
