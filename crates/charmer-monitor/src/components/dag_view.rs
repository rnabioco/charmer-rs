//! DAG (Directed Acyclic Graph) visualization component using Canvas widget.
//!
//! Builds a real dependency graph from job inputs/outputs and renders it
//! using a layered layout algorithm.

use charmer_state::{JobStatus, PipelineState};
use ratatui::{
    layout::Rect,
    style::Color,
    symbols,
    widgets::{
        canvas::{Canvas, Circle, Line as CanvasLine},
        Block, Borders,
    },
    Frame,
};
use std::collections::{HashMap, HashSet};

pub struct DagView;

/// Node in the DAG representing a rule
#[derive(Debug)]
struct RuleNode {
    name: String,
    layer: usize,
    completed: usize,
    running: usize,
    failed: usize,
    pending: usize,
    total: usize,
}

impl DagView {
    /// Build dependency edges between rules based on job inputs/outputs.
    fn build_rule_dependencies(state: &PipelineState) -> HashMap<String, HashSet<String>> {
        // Map output files to the rule that produces them
        let mut output_to_rule: HashMap<&str, &str> = HashMap::new();
        for job in state.jobs.values() {
            if job.is_target {
                continue;
            }
            for output in &job.outputs {
                output_to_rule.insert(output.as_str(), job.rule.as_str());
            }
        }

        // For each rule, find which rules it depends on (via inputs)
        let mut rule_deps: HashMap<String, HashSet<String>> = HashMap::new();
        for job in state.jobs.values() {
            if job.is_target {
                continue;
            }
            let deps = rule_deps.entry(job.rule.clone()).or_default();
            for input in &job.inputs {
                if let Some(&upstream_rule) = output_to_rule.get(input.as_str()) {
                    if upstream_rule != job.rule {
                        deps.insert(upstream_rule.to_string());
                    }
                }
            }
        }

        rule_deps
    }

    /// Assign layers to rules using topological sort (Kahn's algorithm).
    /// Rules with no dependencies are layer 0, their dependents are layer 1, etc.
    fn assign_layers(
        rules: &[String],
        deps: &HashMap<String, HashSet<String>>,
    ) -> HashMap<String, usize> {
        let mut layers: HashMap<String, usize> = HashMap::new();
        let mut remaining: HashSet<&str> = rules.iter().map(|s| s.as_str()).collect();

        // Build reverse dependency map (rule -> rules that depend on it)
        let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();
        for rule in rules {
            dependents.entry(rule.as_str()).or_default();
            if let Some(rule_deps) = deps.get(rule) {
                for dep in rule_deps {
                    dependents.entry(dep.as_str()).or_default().push(rule.as_str());
                }
            }
        }

        let mut current_layer = 0;
        while !remaining.is_empty() {
            // Find all rules whose dependencies are all assigned
            let ready: Vec<&str> = remaining
                .iter()
                .filter(|&&rule| {
                    deps.get(rule)
                        .map(|d| d.iter().all(|dep| layers.contains_key(dep)))
                        .unwrap_or(true)
                })
                .copied()
                .collect();

            if ready.is_empty() {
                // Cycle detected or remaining rules have unmet deps - assign to current layer
                for rule in remaining.iter() {
                    layers.insert(rule.to_string(), current_layer);
                }
                break;
            }

            for rule in ready {
                layers.insert(rule.to_string(), current_layer);
                remaining.remove(rule);
            }
            current_layer += 1;
        }

        layers
    }

    /// Render the DAG visualization showing rule dependencies.
    pub fn render(frame: &mut Frame, area: Rect, state: &PipelineState) {
        // Group jobs by rule and collect stats
        let mut rule_stats: HashMap<String, (usize, usize, usize, usize)> = HashMap::new();

        for job in state.jobs.values() {
            if job.is_target {
                continue;
            }
            let entry = rule_stats.entry(job.rule.clone()).or_default();
            match job.status {
                JobStatus::Completed => entry.0 += 1,
                JobStatus::Running => entry.1 += 1,
                JobStatus::Failed => entry.2 += 1,
                JobStatus::Pending | JobStatus::Queued => entry.3 += 1,
                _ => {}
            }
        }

        let rules: Vec<String> = rule_stats.keys().cloned().collect();
        let num_rules = rules.len();

        if num_rules == 0 {
            let empty = Canvas::default()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" DAG View - No jobs yet "),
                )
                .x_bounds([0.0, 100.0])
                .y_bounds([0.0, 100.0])
                .marker(symbols::Marker::Braille)
                .paint(|_ctx| {});
            frame.render_widget(empty, area);
            return;
        }

        // Build dependency graph and assign layers
        let deps = Self::build_rule_dependencies(state);
        let layers = Self::assign_layers(&rules, &deps);

        // Build nodes with positions
        let max_layer = layers.values().copied().max().unwrap_or(0);
        let mut nodes: Vec<RuleNode> = Vec::new();

        for rule in &rules {
            let (completed, running, failed, pending) = rule_stats.get(rule).copied().unwrap_or_default();
            nodes.push(RuleNode {
                name: rule.clone(),
                layer: *layers.get(rule).unwrap_or(&0),
                completed,
                running,
                failed,
                pending,
                total: completed + running + failed + pending,
            });
        }

        // Sort nodes by layer, then by name for consistent positioning
        nodes.sort_by(|a, b| a.layer.cmp(&b.layer).then(a.name.cmp(&b.name)));

        // Count nodes per layer for positioning
        let mut layer_counts: HashMap<usize, usize> = HashMap::new();
        let mut layer_indices: HashMap<usize, usize> = HashMap::new();
        for node in &nodes {
            *layer_counts.entry(node.layer).or_default() += 1;
        }

        // Calculate positions
        let mut node_positions: HashMap<String, (f64, f64)> = HashMap::new();
        let padding = 8.0;
        let usable_width = 100.0 - 2.0 * padding;
        let usable_height = 100.0 - 2.0 * padding;

        for node in &nodes {
            let layer_count = layer_counts[&node.layer];
            let idx = layer_indices.entry(node.layer).or_default();

            // X position based on layer (left to right)
            let x = if max_layer == 0 {
                50.0
            } else {
                padding + (node.layer as f64 / max_layer as f64) * usable_width
            };

            // Y position based on index within layer (top to bottom, centered)
            let y = if layer_count == 1 {
                50.0
            } else {
                let spacing = usable_height / (layer_count as f64 + 1.0);
                padding + spacing * (*idx as f64 + 1.0)
            };

            node_positions.insert(node.name.clone(), (x, y));
            *idx += 1;
        }

        // Render the canvas
        let title = format!(
            " DAG View - {} rules, {} layers ",
            num_rules,
            max_layer + 1
        );

        let canvas = Canvas::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .title_bottom(" Press 'd' to return │ ← upstream │ → downstream "),
            )
            .x_bounds([0.0, 100.0])
            .y_bounds([0.0, 100.0])
            .marker(symbols::Marker::Braille)
            .paint(move |ctx| {
                // Draw edges first (so nodes are on top)
                for (rule, rule_deps) in &deps {
                    if let Some(&(x2, y2)) = node_positions.get(rule) {
                        for dep in rule_deps {
                            if let Some(&(x1, y1)) = node_positions.get(dep) {
                                ctx.draw(&CanvasLine {
                                    x1,
                                    y1,
                                    x2,
                                    y2,
                                    color: Color::DarkGray,
                                });
                            }
                        }
                    }
                }

                // Draw nodes
                for node in &nodes {
                    if let Some(&(x, y)) = node_positions.get(&node.name) {
                        // Choose color based on status
                        let color = if node.failed > 0 {
                            Color::Red
                        } else if node.running > 0 {
                            Color::Yellow
                        } else if node.completed == node.total && node.total > 0 {
                            Color::Green
                        } else {
                            Color::Blue
                        };

                        // Node size based on job count (clamped)
                        let radius = 2.0 + (node.total as f64).sqrt().min(3.0);

                        ctx.draw(&Circle {
                            x,
                            y,
                            radius,
                            color,
                        });

                        // Draw rule name label
                        let label = if node.name.len() > 12 {
                            format!("{}…", &node.name[..11])
                        } else {
                            node.name.clone()
                        };
                        ctx.print(x, y - radius - 2.0, label);
                    }
                }
            });

        frame.render_widget(canvas, area);
    }
}
