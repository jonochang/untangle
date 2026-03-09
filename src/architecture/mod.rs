use crate::errors::Result;
use crate::graph::ir::{DepGraph, GraphNode};
use petgraph::visit::EdgeRef;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::path::{Component, Path, PathBuf};

const EXACT_FEEDBACK_MAX_NODES: usize = 8;
const EXACT_FEEDBACK_MAX_EDGES: usize = 12;
const ROOT_FALLBACK: &str = "root";

#[derive(Debug, Clone, Serialize)]
pub struct ArchitectureOutput {
    pub level: usize,
    pub metadata: ArchitectureMetadata,
    pub nodes: Vec<ArchitectureNode>,
    pub edges: Vec<ArchitectureEdge>,
    pub feedback_edges: Vec<ArchitectureEdgeRef>,
    pub layers: Vec<ArchitectureLayer>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchitectureMetadata {
    pub root: PathBuf,
    pub source_node_count: usize,
    pub source_edge_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchitectureNode {
    pub id: String,
    pub label: String,
    pub layer: usize,
    pub module_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchitectureEdge {
    pub from: String,
    pub to: String,
    pub count: usize,
    pub source_location_count: usize,
    pub feedback: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ArchitectureEdgeRef {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchitectureLayer {
    pub index: usize,
    pub nodes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct EdgeKey {
    from: String,
    to: String,
}

#[derive(Debug, Clone, Default)]
struct AggregatedEdge {
    count: usize,
    source_location_count: usize,
}

pub fn project_architecture(graph: &DepGraph, root: &Path, level: usize) -> ArchitectureOutput {
    let level = level.max(1);
    let mut module_to_component = BTreeMap::new();
    let mut component_module_counts: BTreeMap<String, usize> = BTreeMap::new();

    for idx in graph.node_indices() {
        let node = &graph[idx];
        let component_id = project_node(node, level);
        module_to_component.insert(idx, component_id.clone());
        *component_module_counts.entry(component_id).or_default() += 1;
    }

    let mut aggregated_edges: BTreeMap<EdgeKey, AggregatedEdge> = BTreeMap::new();
    for edge in graph.edge_references() {
        let from = module_to_component.get(&edge.source()).cloned();
        let to = module_to_component.get(&edge.target()).cloned();
        if let (Some(from), Some(to)) = (from, to) {
            if from == to {
                continue;
            }
            let key = EdgeKey { from, to };
            let aggregated = aggregated_edges.entry(key).or_default();
            aggregated.count += 1;
            aggregated.source_location_count += edge.weight().source_locations.len();
        }
    }

    let nodes: BTreeSet<String> = component_module_counts.keys().cloned().collect();
    let projected_edges: BTreeSet<EdgeKey> = aggregated_edges.keys().cloned().collect();
    let feedback_edges = feedback_edge_set(&nodes, &projected_edges);
    let acyclic_edges: BTreeSet<EdgeKey> = projected_edges
        .difference(&feedback_edges)
        .cloned()
        .collect();
    let layer_map = topological_layers(&nodes, &acyclic_edges);

    let nodes: Vec<ArchitectureNode> = component_module_counts
        .into_iter()
        .map(|(id, module_count)| ArchitectureNode {
            label: component_label(&id),
            layer: *layer_map.get(&id).unwrap_or(&0),
            id,
            module_count,
        })
        .collect();

    let edges: Vec<ArchitectureEdge> = aggregated_edges
        .into_iter()
        .map(|(key, aggregated)| ArchitectureEdge {
            feedback: feedback_edges.contains(&key),
            from: key.from,
            to: key.to,
            count: aggregated.count,
            source_location_count: aggregated.source_location_count,
        })
        .collect();

    let feedback_edges: Vec<ArchitectureEdgeRef> = feedback_edges
        .into_iter()
        .map(|edge| ArchitectureEdgeRef {
            from: edge.from,
            to: edge.to,
        })
        .collect();

    let layers = layers_from_map(&layer_map);

    ArchitectureOutput {
        level,
        metadata: ArchitectureMetadata {
            root: root.to_path_buf(),
            source_node_count: graph.node_count(),
            source_edge_count: graph.edge_count(),
        },
        nodes,
        edges,
        feedback_edges,
        layers,
    }
}

pub fn write_dot<W: std::io::Write>(
    writer: &mut W,
    architecture: &ArchitectureOutput,
) -> Result<()> {
    writeln!(writer, "digraph architecture {{")?;
    writeln!(writer, "    rankdir=TB;")?;
    writeln!(
        writer,
        "    node [shape=box, style=\"rounded,filled\", fillcolor=linen, color=gray35];"
    )?;
    writeln!(writer, "    edge [color=gray45];")?;
    writeln!(writer)?;

    for layer in &architecture.layers {
        writeln!(writer, "    {{ rank=same;")?;
        for node_id in &layer.nodes {
            let node = architecture
                .nodes
                .iter()
                .find(|node| &node.id == node_id)
                .expect("layer node should exist");
            writeln!(
                writer,
                "        \"{}\" [label=\"{}\\n{} modules\"];",
                node.id, node.label, node.module_count
            )?;
        }
        writeln!(writer, "    }}")?;
    }
    writeln!(writer)?;

    for edge in &architecture.edges {
        if edge.feedback {
            writeln!(
                writer,
                "    \"{}\" -> \"{}\" [label=\"{}\", color=firebrick, style=dashed];",
                edge.from, edge.to, edge.count
            )?;
        } else if edge.count > 1 {
            writeln!(
                writer,
                "    \"{}\" -> \"{}\" [label=\"{}\"];",
                edge.from, edge.to, edge.count
            )?;
        } else {
            writeln!(writer, "    \"{}\" -> \"{}\";", edge.from, edge.to)?;
        }
    }

    writeln!(writer, "}}")?;
    Ok(())
}

pub fn layer_map(output: &ArchitectureOutput) -> BTreeMap<String, usize> {
    output
        .nodes
        .iter()
        .map(|node| (node.id.clone(), node.layer))
        .collect()
}

fn project_node(node: &GraphNode, level: usize) -> String {
    let mut segments = module_segments(node);
    if segments.is_empty() {
        return ROOT_FALLBACK.to_string();
    }
    if segments.len() > level {
        segments.truncate(level);
    }
    segments.join(".")
}

fn module_segments(node: &GraphNode) -> Vec<String> {
    let mut segments = path_segments(&node.path);
    trim_leading_boilerplate(&mut segments);
    trim_terminal_module_marker(&mut segments);
    if segments.is_empty() {
        segments = name_segments(&node.name);
        trim_leading_boilerplate(&mut segments);
        trim_terminal_module_marker(&mut segments);
    }
    if segments.is_empty() {
        vec![ROOT_FALLBACK.to_string()]
    } else {
        segments
    }
}

fn path_segments(path: &Path) -> Vec<String> {
    let mut segments = Vec::new();
    for component in path.components() {
        if let Component::Normal(value) = component {
            let value = value.to_string_lossy().trim().to_string();
            if value.is_empty() {
                continue;
            }
            segments.push(value);
        }
    }
    if let Some(last) = segments.last_mut() {
        if let Some((stem, _)) = last.rsplit_once('.') {
            *last = stem.to_string();
        }
    }
    segments
}

fn name_segments(name: &str) -> Vec<String> {
    name.split('.')
        .filter(|segment| !segment.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn trim_leading_boilerplate(segments: &mut Vec<String>) {
    while segments.len() > 1 && is_boilerplate_root(segments.first().map(String::as_str)) {
        segments.remove(0);
    }
}

fn trim_terminal_module_marker(segments: &mut Vec<String>) {
    if segments.len() > 1
        && matches!(
            segments.last().map(String::as_str),
            Some("__init__" | "mod" | "index")
        )
    {
        segments.pop();
    }
}

fn is_boilerplate_root(segment: Option<&str>) -> bool {
    matches!(segment, Some("src" | "lib" | "app" | "pkg"))
}

fn component_label(id: &str) -> String {
    id.rsplit('.').next().unwrap_or(id).to_string()
}

fn layers_from_map(layer_map: &BTreeMap<String, usize>) -> Vec<ArchitectureLayer> {
    let mut grouped: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for (node, layer) in layer_map {
        grouped.entry(*layer).or_default().push(node.clone());
    }
    grouped
        .into_iter()
        .map(|(index, mut nodes)| {
            nodes.sort();
            ArchitectureLayer { index, nodes }
        })
        .collect()
}

fn feedback_edge_set(nodes: &BTreeSet<String>, edges: &BTreeSet<EdgeKey>) -> BTreeSet<EdgeKey> {
    strongly_connected_components(nodes, edges)
        .into_iter()
        .flat_map(|component| component_feedback_edges(&component, edges))
        .collect()
}

fn component_feedback_edges(
    component: &BTreeSet<String>,
    edges: &BTreeSet<EdgeKey>,
) -> BTreeSet<EdgeKey> {
    let internal = component_internal_edges(component, edges);
    if !cyclic_component(component, &internal) {
        return BTreeSet::new();
    }
    if component.len() <= EXACT_FEEDBACK_MAX_NODES && internal.len() <= EXACT_FEEDBACK_MAX_EDGES {
        exact_feedback_edges(component, &internal).unwrap_or_default()
    } else {
        heuristic_feedback_edges(component, &internal)
    }
}

fn component_internal_edges(
    component: &BTreeSet<String>,
    edges: &BTreeSet<EdgeKey>,
) -> BTreeSet<EdgeKey> {
    edges
        .iter()
        .filter(|edge| component.contains(&edge.from) && component.contains(&edge.to))
        .cloned()
        .collect()
}

fn cyclic_component(component: &BTreeSet<String>, edges: &BTreeSet<EdgeKey>) -> bool {
    component.len() > 1 || edges.iter().any(|edge| edge.from == edge.to)
}

fn exact_feedback_edges(
    nodes: &BTreeSet<String>,
    edges: &BTreeSet<EdgeKey>,
) -> Option<BTreeSet<EdgeKey>> {
    let edge_vec: Vec<_> = edges.iter().cloned().collect();
    for remove_count in 0..=edge_vec.len() {
        let mut current = Vec::new();
        if let Some(found) = choose_feedback_subset(nodes, &edge_vec, remove_count, 0, &mut current)
        {
            return Some(found.into_iter().collect());
        }
    }
    None
}

fn choose_feedback_subset(
    nodes: &BTreeSet<String>,
    edges: &[EdgeKey],
    remove_count: usize,
    start: usize,
    current: &mut Vec<EdgeKey>,
) -> Option<Vec<EdgeKey>> {
    if current.len() == remove_count {
        let removed: HashSet<_> = current.iter().cloned().collect();
        let remaining: BTreeSet<_> = edges
            .iter()
            .filter(|edge| !removed.contains(*edge))
            .cloned()
            .collect();
        if is_dag(nodes, &remaining) {
            return Some(current.clone());
        }
        return None;
    }

    for idx in start..edges.len() {
        current.push(edges[idx].clone());
        if let Some(found) = choose_feedback_subset(nodes, edges, remove_count, idx + 1, current) {
            return Some(found);
        }
        current.pop();
    }

    None
}

fn heuristic_feedback_edges(
    nodes: &BTreeSet<String>,
    edges: &BTreeSet<EdgeKey>,
) -> BTreeSet<EdgeKey> {
    let order = greedy_order(nodes, edges);
    let index: BTreeMap<_, _> = order
        .iter()
        .enumerate()
        .map(|(i, node)| (node.clone(), i))
        .collect();
    edges
        .iter()
        .filter(|edge| {
            index.get(&edge.from).unwrap_or(&usize::MAX)
                >= index.get(&edge.to).unwrap_or(&usize::MAX)
        })
        .cloned()
        .collect()
}

fn greedy_order(nodes: &BTreeSet<String>, edges: &BTreeSet<EdgeKey>) -> Vec<String> {
    let mut remaining = nodes.clone();
    let mut active_edges = edges.clone();
    let mut left = Vec::new();
    let mut right = Vec::new();

    while !remaining.is_empty() {
        let incoming = incoming_map(&remaining, &active_edges);
        let outgoing = outgoing_map(&remaining, &active_edges);
        let choice = choose_next_node(&remaining, &incoming, &outgoing);
        match choice.1 {
            Side::Left => left.push(choice.0.clone()),
            Side::Right => right.push(choice.0.clone()),
        }
        remaining.remove(&choice.0);
        active_edges = active_edges
            .into_iter()
            .filter(|edge| edge.from != choice.0 && edge.to != choice.0)
            .collect();
    }

    right.reverse();
    left.extend(right);
    left
}

#[derive(Clone, Copy)]
enum Side {
    Left,
    Right,
}

fn choose_next_node(
    remaining: &BTreeSet<String>,
    incoming: &BTreeMap<String, BTreeSet<String>>,
    outgoing: &BTreeMap<String, BTreeSet<String>>,
) -> (String, Side) {
    if let Some(node) = remaining
        .iter()
        .find(|node| incoming.get(*node).is_none_or(BTreeSet::is_empty))
    {
        return (node.clone(), Side::Left);
    }
    if let Some(node) = remaining
        .iter()
        .find(|node| outgoing.get(*node).is_none_or(BTreeSet::is_empty))
    {
        return (node.clone(), Side::Right);
    }

    remaining
        .iter()
        .max_by_key(|node| {
            let out = outgoing.get(*node).map(BTreeSet::len).unwrap_or(0) as isize;
            let incoming = incoming.get(*node).map(BTreeSet::len).unwrap_or(0) as isize;
            (out - incoming, *node)
        })
        .map(|node| (node.clone(), Side::Left))
        .expect("remaining nodes should not be empty")
}

fn topological_layers(
    nodes: &BTreeSet<String>,
    edges: &BTreeSet<EdgeKey>,
) -> BTreeMap<String, usize> {
    let incoming = incoming_map(nodes, edges);
    let outgoing = outgoing_map(nodes, edges);
    let mut indegree: BTreeMap<String, usize> =
        nodes.iter().map(|node| (node.clone(), 0)).collect();
    for edge in edges {
        *indegree.entry(edge.to.clone()).or_default() += 1;
    }

    let mut queue: VecDeque<String> = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(node, _)| node.clone())
        .collect();
    let mut queue_items: Vec<_> = queue.iter().cloned().collect();
    queue_items.sort();
    queue = queue_items.into();

    let mut layers: BTreeMap<String, usize> = nodes.iter().map(|node| (node.clone(), 0)).collect();

    while let Some(node) = queue.pop_front() {
        let node_layer = *layers.get(&node).unwrap_or(&0);
        if let Some(targets) = outgoing.get(&node) {
            for target in targets {
                let next_layer = node_layer + 1;
                layers
                    .entry(target.clone())
                    .and_modify(|layer| *layer = (*layer).max(next_layer))
                    .or_insert(next_layer);
                if let Some(in_degree) = indegree.get_mut(target) {
                    *in_degree -= 1;
                    if *in_degree == 0 {
                        let position = queue.iter().position(|queued| queued > target);
                        if let Some(position) = position {
                            queue.insert(position, target.clone());
                        } else {
                            queue.push_back(target.clone());
                        }
                    }
                }
            }
        }
        if let Some(roots) = incoming.get(&node) {
            let implied = roots
                .iter()
                .map(|root| layers.get(root).copied().unwrap_or(0) + 1)
                .max()
                .unwrap_or(node_layer);
            if let Some(layer) = layers.get_mut(&node) {
                *layer = (*layer).max(implied);
            }
        }
    }

    layers
}

fn is_dag(nodes: &BTreeSet<String>, edges: &BTreeSet<EdgeKey>) -> bool {
    topological_sort(nodes, edges).len() == nodes.len()
}

fn topological_sort(nodes: &BTreeSet<String>, edges: &BTreeSet<EdgeKey>) -> Vec<String> {
    let outgoing = outgoing_map(nodes, edges);
    let mut indegree: BTreeMap<String, usize> =
        nodes.iter().map(|node| (node.clone(), 0)).collect();
    for edge in edges {
        *indegree.entry(edge.to.clone()).or_default() += 1;
    }

    let mut queue: Vec<_> = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(node, _)| node.clone())
        .collect();
    queue.sort();

    let mut ordered = Vec::new();
    while let Some(node) = queue.first().cloned() {
        queue.remove(0);
        ordered.push(node.clone());
        if let Some(targets) = outgoing.get(&node) {
            for target in targets {
                if let Some(degree) = indegree.get_mut(target) {
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push(target.clone());
                        queue.sort();
                    }
                }
            }
        }
    }

    ordered
}

fn strongly_connected_components(
    nodes: &BTreeSet<String>,
    edges: &BTreeSet<EdgeKey>,
) -> Vec<BTreeSet<String>> {
    struct Tarjan<'a> {
        adjacency: BTreeMap<String, BTreeSet<String>>,
        index: usize,
        stack: Vec<String>,
        on_stack: BTreeSet<String>,
        indices: BTreeMap<String, usize>,
        lowlink: BTreeMap<String, usize>,
        components: Vec<BTreeSet<String>>,
        nodes: &'a BTreeSet<String>,
    }

    impl<'a> Tarjan<'a> {
        fn strong_connect(&mut self, node: &str) {
            self.indices.insert(node.to_string(), self.index);
            self.lowlink.insert(node.to_string(), self.index);
            self.index += 1;
            self.stack.push(node.to_string());
            self.on_stack.insert(node.to_string());

            if let Some(neighbors) = self.adjacency.get(node).cloned() {
                for neighbor in &neighbors {
                    if !self.indices.contains_key(neighbor) {
                        self.strong_connect(neighbor);
                        let low = self.lowlink[node].min(self.lowlink[neighbor]);
                        self.lowlink.insert(node.to_string(), low);
                    } else if self.on_stack.contains(neighbor) {
                        let low = self.lowlink[node].min(self.indices[neighbor]);
                        self.lowlink.insert(node.to_string(), low);
                    }
                }
            }

            if self.lowlink[node] == self.indices[node] {
                let mut component = BTreeSet::new();
                while let Some(stack_node) = self.stack.pop() {
                    self.on_stack.remove(&stack_node);
                    component.insert(stack_node.clone());
                    if stack_node == node {
                        break;
                    }
                }
                self.components.push(component);
            }
        }
    }

    let mut tarjan = Tarjan {
        adjacency: outgoing_map(nodes, edges),
        index: 0,
        stack: Vec::new(),
        on_stack: BTreeSet::new(),
        indices: BTreeMap::new(),
        lowlink: BTreeMap::new(),
        components: Vec::new(),
        nodes,
    };

    for node in tarjan.nodes {
        if !tarjan.indices.contains_key(node) {
            tarjan.strong_connect(node);
        }
    }

    tarjan.components
}

fn outgoing_map(
    nodes: &BTreeSet<String>,
    edges: &BTreeSet<EdgeKey>,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut map: BTreeMap<String, BTreeSet<String>> = nodes
        .iter()
        .map(|node| (node.clone(), BTreeSet::new()))
        .collect();
    for edge in edges {
        map.entry(edge.from.clone())
            .or_default()
            .insert(edge.to.clone());
    }
    map
}

fn incoming_map(
    nodes: &BTreeSet<String>,
    edges: &BTreeSet<EdgeKey>,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut map: BTreeMap<String, BTreeSet<String>> = nodes
        .iter()
        .map(|node| (node.clone(), BTreeSet::new()))
        .collect();
    for edge in edges {
        map.entry(edge.to.clone())
            .or_default()
            .insert(edge.from.clone());
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::ir::{EdgeKind, GraphEdge, NodeKind};
    use std::path::PathBuf;

    fn add_node(graph: &mut DepGraph, name: &str, path: &str) -> petgraph::graph::NodeIndex {
        graph.add_node(GraphNode {
            kind: NodeKind::Module,
            path: PathBuf::from(path),
            name: name.to_string(),
            span: None,
            language: None,
        })
    }

    #[test]
    fn projects_modules_into_architecture_nodes() {
        let mut graph = DepGraph::new();
        let api_handler = add_node(&mut graph, "src.api.handler", "src/api/handler.py");
        let api_client = add_node(&mut graph, "src.api.client", "src/api/client.py");
        let db = add_node(&mut graph, "src.db.__init__", "src/db/__init__.py");
        let utils = add_node(&mut graph, "src.utils.__init__", "src/utils/__init__.py");

        graph.add_edge(
            api_handler,
            db,
            GraphEdge {
                kind: EdgeKind::Import,
                source_locations: vec![],
                weight: 1,
            },
        );
        graph.add_edge(
            api_client,
            db,
            GraphEdge {
                kind: EdgeKind::Import,
                source_locations: vec![],
                weight: 1,
            },
        );
        graph.add_edge(
            api_handler,
            utils,
            GraphEdge {
                kind: EdgeKind::Import,
                source_locations: vec![],
                weight: 1,
            },
        );

        let output = project_architecture(&graph, Path::new("."), 1);
        let node_ids: Vec<_> = output.nodes.iter().map(|node| node.id.as_str()).collect();
        assert_eq!(node_ids, vec!["api", "db", "utils"]);

        let counts: Vec<_> = output
            .edges
            .iter()
            .map(|edge| (edge.from.as_str(), edge.to.as_str(), edge.count))
            .collect();
        assert_eq!(counts, vec![("api", "db", 2), ("api", "utils", 1)]);
    }

    #[test]
    fn marks_feedback_edges_and_assigns_layers() {
        let mut graph = DepGraph::new();
        let api = add_node(&mut graph, "src.api.handler", "src/api/handler.py");
        let db = add_node(&mut graph, "src.db.connection", "src/db/connection.py");
        let ui = add_node(&mut graph, "src.ui.page", "src/ui/page.py");

        for (from, to) in [(api, db), (db, api), (ui, api)] {
            graph.add_edge(
                from,
                to,
                GraphEdge {
                    kind: EdgeKind::Import,
                    source_locations: vec![],
                    weight: 1,
                },
            );
        }

        let output = project_architecture(&graph, Path::new("."), 1);
        let layers = layer_map(&output);
        assert_eq!(layers.get("ui"), Some(&0));
        assert_ne!(layers.get("api"), layers.get("db"));

        assert_eq!(output.edges.iter().filter(|edge| edge.feedback).count(), 1);
        assert!(output.edges.iter().any(|edge| edge.feedback
            && ((edge.from == "api" && edge.to == "db")
                || (edge.from == "db" && edge.to == "api"))));
    }

    #[test]
    fn normalizes_root_containers_for_multiple_languages() {
        let python = GraphNode {
            kind: NodeKind::Module,
            path: PathBuf::from("src/api/handler.py"),
            name: "src.api.handler".to_string(),
            span: None,
            language: None,
        };
        let ruby = GraphNode {
            kind: NodeKind::Module,
            path: PathBuf::from("lib/service.rb"),
            name: "lib.service".to_string(),
            span: None,
            language: None,
        };
        let go = GraphNode {
            kind: NodeKind::Module,
            path: PathBuf::from("pkg/foo"),
            name: "pkg.foo".to_string(),
            span: None,
            language: None,
        };

        assert_eq!(module_segments(&python), vec!["api", "handler"]);
        assert_eq!(module_segments(&ruby), vec!["service"]);
        assert_eq!(module_segments(&go), vec!["foo"]);
    }
}
