//! DAG (Directed Acyclic Graph) visualization component.
//!
//! Renders a clean ASCII box diagram of rule dependencies using Unicode
//! box-drawing characters.

use charmer_state::{JobStatus, PipelineState};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use std::collections::{HashMap, HashSet};

pub struct DagView;

/// Node in the DAG representing a rule
struct RuleNode {
    name: String,
    layer: usize,
    completed: usize,
    running: usize,
    failed: usize,
    pending: usize,
}

impl RuleNode {
    fn total(&self) -> usize {
        self.completed + self.running + self.failed + self.pending
    }

    fn status_color(&self) -> Color {
        if self.failed > 0 {
            Color::Red
        } else if self.running > 0 {
            Color::Yellow
        } else if self.completed == self.total() && self.total() > 0 {
            Color::Green
        } else {
            Color::Blue
        }
    }

    fn status_char(&self) -> &'static str {
        if self.failed > 0 {
            "✗"
        } else if self.running > 0 {
            "▶"
        } else if self.completed == self.total() && self.total() > 0 {
            "✓"
        } else {
            "○"
        }
    }
}

impl DagView {
    /// Build dependency edges between rules based on job inputs/outputs.
    fn build_rule_dependencies(state: &PipelineState) -> HashMap<String, HashSet<String>> {
        let mut output_to_rule: HashMap<&str, &str> = HashMap::new();
        for job in state.jobs.values() {
            if job.is_target {
                continue;
            }
            for output in &job.outputs {
                output_to_rule.insert(output.as_str(), job.rule.as_str());
            }
        }

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

    /// Assign layers using topological sort.
    fn assign_layers(
        rules: &[String],
        deps: &HashMap<String, HashSet<String>>,
    ) -> HashMap<String, usize> {
        let mut layers: HashMap<String, usize> = HashMap::new();
        let mut remaining: HashSet<&str> = rules.iter().map(|s| s.as_str()).collect();

        let mut current_layer = 0;
        while !remaining.is_empty() {
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

    /// Generate Mermaid-style flowchart as ASCII.
    fn generate_ascii_dag(
        nodes: &[RuleNode],
        deps: &HashMap<String, HashSet<String>>,
        max_layer: usize,
        width: usize,
    ) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();

        // Group nodes by layer
        let mut layers: Vec<Vec<&RuleNode>> = vec![Vec::new(); max_layer + 1];
        for node in nodes {
            layers[node.layer].push(node);
        }

        // Calculate box width based on longest rule name
        let max_name_len = nodes.iter().map(|n| n.name.len()).max().unwrap_or(8);
        let box_width = (max_name_len + 4).min(20); // padding + borders, max 20

        // Build reverse deps for drawing arrows
        let mut reverse_deps: HashMap<&str, Vec<&str>> = HashMap::new();
        for (rule, rule_deps) in deps {
            for dep in rule_deps {
                reverse_deps
                    .entry(dep.as_str())
                    .or_default()
                    .push(rule.as_str());
            }
        }

        // Render each layer
        for (layer_idx, layer_nodes) in layers.iter().enumerate() {
            if layer_nodes.is_empty() {
                continue;
            }

            // Sort nodes in layer by name for consistency
            let mut sorted_nodes: Vec<&RuleNode> = layer_nodes.clone();
            sorted_nodes.sort_by(|a, b| a.name.cmp(&b.name));

            // Calculate spacing
            let num_nodes = sorted_nodes.len();
            let total_box_width = box_width * num_nodes + (num_nodes.saturating_sub(1)) * 3;
            let left_pad = if total_box_width < width {
                (width - total_box_width) / 2
            } else {
                1
            };

            // Top border line
            let mut top_spans: Vec<Span> = vec![Span::raw(" ".repeat(left_pad))];
            for (i, node) in sorted_nodes.iter().enumerate() {
                if i > 0 {
                    top_spans.push(Span::raw("   ")); // spacing between boxes
                }
                top_spans.push(Span::styled(
                    format!("┌{}┐", "─".repeat(box_width - 2)),
                    Style::default().fg(node.status_color()),
                ));
            }
            lines.push(Line::from(top_spans));

            // Content line with rule name
            let mut content_spans: Vec<Span> = vec![Span::raw(" ".repeat(left_pad))];
            for (i, node) in sorted_nodes.iter().enumerate() {
                if i > 0 {
                    content_spans.push(Span::raw("───")); // horizontal connector
                }
                let name = if node.name.len() > box_width - 4 {
                    format!("{}…", &node.name[..box_width - 5])
                } else {
                    node.name.clone()
                };
                let padding = box_width - 4 - name.len();
                let left = padding / 2;
                let right = padding - left;
                content_spans.push(Span::styled(
                    format!(
                        "│{}{}{}{}│",
                        node.status_char(),
                        " ".repeat(left),
                        name,
                        " ".repeat(right)
                    ),
                    Style::default()
                        .fg(node.status_color())
                        .add_modifier(Modifier::BOLD),
                ));
            }
            lines.push(Line::from(content_spans));

            // Stats line
            let mut stats_spans: Vec<Span> = vec![Span::raw(" ".repeat(left_pad))];
            for (i, node) in sorted_nodes.iter().enumerate() {
                if i > 0 {
                    stats_spans.push(Span::raw("   "));
                }
                let stats = format!(
                    "{}/{}",
                    node.completed + node.running,
                    node.total()
                );
                let padding = box_width - 2 - stats.len();
                let left = padding / 2;
                let right = padding - left;
                stats_spans.push(Span::styled(
                    format!("│{}{}{}│", " ".repeat(left), stats, " ".repeat(right)),
                    Style::default().fg(node.status_color()),
                ));
            }
            lines.push(Line::from(stats_spans));

            // Bottom border line
            let mut bottom_spans: Vec<Span> = vec![Span::raw(" ".repeat(left_pad))];
            for (i, node) in sorted_nodes.iter().enumerate() {
                if i > 0 {
                    bottom_spans.push(Span::raw("   "));
                }
                bottom_spans.push(Span::styled(
                    format!("└{}┘", "─".repeat(box_width - 2)),
                    Style::default().fg(node.status_color()),
                ));
            }
            lines.push(Line::from(bottom_spans));

            // Draw arrows to next layer if not last
            if layer_idx < layers.len() - 1 && !layers[layer_idx + 1].is_empty() {
                // Find which nodes connect to next layer
                let mut arrow_positions: Vec<(usize, bool)> = Vec::new();
                for (i, node) in sorted_nodes.iter().enumerate() {
                    let has_downstream = reverse_deps
                        .get(node.name.as_str())
                        .map(|d| !d.is_empty())
                        .unwrap_or(false);
                    arrow_positions.push((i, has_downstream));
                }

                // Arrow line
                let mut arrow_spans: Vec<Span> = vec![Span::raw(" ".repeat(left_pad))];
                for (i, (_, has_arrow)) in arrow_positions.iter().enumerate() {
                    if i > 0 {
                        arrow_spans.push(Span::raw("   "));
                    }
                    let center_pad = (box_width - 1) / 2;
                    if *has_arrow {
                        arrow_spans.push(Span::styled(
                            format!("{}│{}", " ".repeat(center_pad), " ".repeat(box_width - 1 - center_pad)),
                            Style::default().fg(Color::DarkGray),
                        ));
                    } else {
                        arrow_spans.push(Span::raw(" ".repeat(box_width)));
                    }
                }
                lines.push(Line::from(arrow_spans));

                // Arrow head line
                let mut head_spans: Vec<Span> = vec![Span::raw(" ".repeat(left_pad))];
                for (i, (_, has_arrow)) in arrow_positions.iter().enumerate() {
                    if i > 0 {
                        head_spans.push(Span::raw("   "));
                    }
                    let center_pad = (box_width - 1) / 2;
                    if *has_arrow {
                        head_spans.push(Span::styled(
                            format!("{}▼{}", " ".repeat(center_pad), " ".repeat(box_width - 1 - center_pad)),
                            Style::default().fg(Color::DarkGray),
                        ));
                    } else {
                        head_spans.push(Span::raw(" ".repeat(box_width)));
                    }
                }
                lines.push(Line::from(head_spans));

                lines.push(Line::from("")); // blank line between layers
            }
        }

        // Add legend
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(" ✓ ", Style::default().fg(Color::Green)),
            Span::raw("done  "),
            Span::styled(" ▶ ", Style::default().fg(Color::Yellow)),
            Span::raw("running  "),
            Span::styled(" ✗ ", Style::default().fg(Color::Red)),
            Span::raw("failed  "),
            Span::styled(" ○ ", Style::default().fg(Color::Blue)),
            Span::raw("pending"),
        ]));

        lines
    }

    /// Render the DAG visualization.
    pub fn render(frame: &mut Frame, area: Rect, state: &PipelineState) {
        // Collect rule stats
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
            let empty = Paragraph::new("No jobs yet")
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" DAG View "),
                );
            frame.render_widget(empty, area);
            return;
        }

        // Build graph
        let deps = Self::build_rule_dependencies(state);
        let layers = Self::assign_layers(&rules, &deps);
        let max_layer = layers.values().copied().max().unwrap_or(0);

        // Build nodes
        let mut nodes: Vec<RuleNode> = rules
            .iter()
            .map(|rule| {
                let (completed, running, failed, pending) =
                    rule_stats.get(rule).copied().unwrap_or_default();
                RuleNode {
                    name: rule.clone(),
                    layer: *layers.get(rule).unwrap_or(&0),
                    completed,
                    running,
                    failed,
                    pending,
                }
            })
            .collect();
        nodes.sort_by(|a, b| a.layer.cmp(&b.layer).then(a.name.cmp(&b.name)));

        // Generate ASCII
        let content_width = area.width.saturating_sub(2) as usize;
        let ascii_lines = Self::generate_ascii_dag(&nodes, &deps, max_layer, content_width);

        let paragraph = Paragraph::new(ascii_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" DAG View - {} rules ", num_rules))
                    .title_bottom(" Press 'd' to return "),
            )
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, area);
    }
}
