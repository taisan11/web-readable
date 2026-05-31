use std::collections::HashMap;

use markup5ever_rcdom::Handle;

use crate::dom::{
    NodeMetrics, attr_value, build_metrics_map, child_handles, collect_element_contexts,
    element_name, node_key,
};
use crate::error::{ExtractError, Result};
use crate::model::ExtractOptions;

pub(crate) struct ScoringResult {
    pub(crate) top_score: f64,
    pub(crate) selected_nodes: Vec<Handle>,
    pub(crate) metrics: HashMap<usize, NodeMetrics>,
}

pub(crate) fn score_content(root: &Handle, options: &ExtractOptions) -> Result<ScoringResult> {
    let contexts = collect_element_contexts(root);
    let metrics = build_metrics_map(root);

    let mut parent_of: HashMap<usize, Option<usize>> = HashMap::new();
    let mut handle_of: HashMap<usize, Handle> = HashMap::new();
    let mut tag_of: HashMap<usize, String> = HashMap::new();
    let mut own_scores: HashMap<usize, f64> = HashMap::new();

    for ctx in contexts {
        parent_of.insert(ctx.key, ctx.parent);
        handle_of.insert(ctx.key, ctx.handle.clone());
        tag_of.insert(ctx.key, ctx.tag.clone());
        let _ = ctx.grandparent;

        let m = metrics.get(&ctx.key).cloned().unwrap_or_default();
        if !is_candidate_node(&ctx.tag, &ctx.handle, &m, options) {
            continue;
        }
        let score = base_score(&ctx.tag, &ctx.handle, &m);
        if score > 0.0 {
            own_scores.insert(ctx.key, score);
        }
    }

    if own_scores.is_empty() {
        return Err(ExtractError::NoCandidate);
    }

    let mut total_scores = own_scores.clone();
    for (key, own_score) in &own_scores {
        if let Some(Some(parent)) = parent_of.get(key)
            && own_scores.contains_key(parent)
        {
            *total_scores.entry(*parent).or_insert(0.0) += own_score * 0.35;
            if let Some(Some(grandparent)) = parent_of.get(parent)
                && own_scores.contains_key(grandparent)
            {
                *total_scores.entry(*grandparent).or_insert(0.0) += own_score * 0.15;
            }
        }
    }

    for (key, score) in &mut total_scores {
        let m = metrics.get(key).cloned().unwrap_or_default();
        let link_penalty = (m.link_density() * 65.0).min(50.0);
        let list_total = m.list_item_count + m.paragraph_count;
        let list_density = if list_total == 0 {
            0.0
        } else {
            m.list_item_count as f64 / list_total as f64
        };
        let list_penalty = if list_density > 0.58 {
            (list_density - 0.58) * 42.0
        } else {
            0.0
        };
        let heading_penalty = if m.paragraph_count == 0 && m.heading_count > 0 {
            16.0
        } else if m.heading_count > m.paragraph_count.saturating_add(2) {
            8.0
        } else {
            0.0
        };
        *score -= link_penalty + list_penalty + heading_penalty;
    }

    let (top_key, top_score) = total_scores
        .iter()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map(|(k, v)| (*k, *v))
        .ok_or(ExtractError::NoCandidate)?;

    if top_score <= 0.0 {
        return Err(ExtractError::NoCandidate);
    }

    let selected_nodes = select_siblings(
        top_key,
        top_score,
        options,
        &total_scores,
        &metrics,
        &parent_of,
        &handle_of,
        &tag_of,
    );

    Ok(ScoringResult {
        top_score,
        selected_nodes,
        metrics,
    })
}

fn select_siblings(
    top_key: usize,
    top_score: f64,
    options: &ExtractOptions,
    total_scores: &HashMap<usize, f64>,
    metrics: &HashMap<usize, NodeMetrics>,
    parent_of: &HashMap<usize, Option<usize>>,
    handle_of: &HashMap<usize, Handle>,
    _tag_of: &HashMap<usize, String>,
) -> Vec<Handle> {
    let Some(top_handle) = handle_of.get(&top_key).cloned() else {
        return Vec::new();
    };

    let Some(Some(parent_key)) = parent_of.get(&top_key).copied() else {
        return vec![top_handle];
    };

    let Some(parent_handle) = handle_of.get(&parent_key) else {
        return vec![top_handle];
    };

    let mut selected = Vec::new();
    for child in child_handles(parent_handle) {
        let Some(tag) = element_name(&child) else {
            continue;
        };
        let child_key = node_key(&child);

        if child_key == top_key {
            selected.push(child);
            continue;
        }

        let sibling_score = total_scores.get(&child_key).copied().unwrap_or(0.0);
        let m = metrics.get(&child_key).cloned().unwrap_or_default();

        let mut include = sibling_score >= top_score * options.sibling_score_ratio;
        if !include && tag == "p" && m.text_len >= 80 && m.link_density() < 0.35 {
            include = true;
        }
        if !include && m.text_len >= 220 && m.link_density() < 0.22 {
            include = true;
        }

        if include {
            selected.push(child);
        }
    }

    if selected.is_empty() {
        vec![top_handle]
    } else {
        selected
    }
}

fn is_candidate_node(
    tag: &str,
    handle: &Handle,
    metrics: &NodeMetrics,
    options: &ExtractOptions,
) -> bool {
    if metrics.text_len < options.min_candidate_text {
        return false;
    }
    if metrics.paragraph_count == 0 && metrics.heading_count > 0 && metrics.text_len < 500 {
        return false;
    }
    if matches!(
        tag,
        "script"
            | "style"
            | "noscript"
            | "template"
            | "nav"
            | "aside"
            | "footer"
            | "header"
            | "button"
            | "input"
            | "select"
            | "textarea"
            | "form"
    ) {
        return false;
    }

    if matches!(tag, "article" | "main" | "section" | "div" | "p" | "body") {
        return true;
    }

    attr_value(handle, "role")
        .map(|role| {
            matches!(
                role.trim().to_ascii_lowercase().as_str(),
                "main" | "article"
            )
        })
        .unwrap_or(false)
}

fn base_score(tag: &str, handle: &Handle, metrics: &NodeMetrics) -> f64 {
    let semantic = tag_weight(tag) + role_weight(handle) + class_id_weight(handle);
    let text_score = (metrics.text_len as f64).sqrt() * 1.8;
    let paragraph_bonus = (metrics.paragraph_count as f64 * 7.0).min(45.0);
    let punctuation_bonus = (metrics.punctuation_count as f64 * 0.7).min(22.0);
    let media_adjustment = if metrics.media_count > 0 && metrics.text_len < 180 {
        -12.0
    } else if metrics.media_count > 0 {
        4.0
    } else {
        0.0
    };
    let early_link_penalty = (metrics.link_density() * 30.0).min(25.0);

    semantic + text_score + paragraph_bonus + punctuation_bonus + media_adjustment
        - early_link_penalty
}

fn tag_weight(tag: &str) -> f64 {
    match tag {
        "main" => 38.0,
        "article" => 34.0,
        "section" => 16.0,
        "div" => 8.0,
        "p" => 10.0,
        "body" => 6.0,
        "figure" => -6.0,
        "header" | "footer" | "aside" | "nav" => -42.0,
        "ul" | "ol" => -18.0,
        "form" => -36.0,
        _ => 0.0,
    }
}

fn role_weight(handle: &Handle) -> f64 {
    let Some(role) = attr_value(handle, "role").map(|v| v.trim().to_ascii_lowercase()) else {
        return 0.0;
    };
    match role.as_str() {
        "main" | "article" => 20.0,
        "navigation" | "complementary" | "banner" | "contentinfo" | "menu" => -28.0,
        _ => 0.0,
    }
}

fn class_id_weight(handle: &Handle) -> f64 {
    let class = attr_value(handle, "class").unwrap_or_default();
    let id = attr_value(handle, "id").unwrap_or_default();
    let combined = format!("{} {}", class, id).to_ascii_lowercase();

    let positive = [
        "article", "content", "main", "post", "entry", "story", "prose", "markdown", "read", "body",
    ];
    let negative = [
        "comment",
        "footer",
        "header",
        "nav",
        "menu",
        "sidebar",
        "share",
        "related",
        "advert",
        "ads",
        "promo",
        "breadcrumb",
        "social",
        "pagination",
        "widget",
    ];

    let mut score: f64 = 0.0;
    for token in positive {
        if combined.contains(token) {
            score += 4.5;
        }
    }
    for token in negative {
        if combined.contains(token) {
            score -= 7.0;
        }
    }

    score.clamp(-30.0, 26.0)
}
