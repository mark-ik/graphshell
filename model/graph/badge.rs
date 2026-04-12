use std::collections::HashSet;

use super::Node;

pub(crate) const TAG_PIN: &str = "#pin";
pub(crate) const TAG_STARRED: &str = "#starred";
pub(crate) const TAG_ARCHIVE: &str = "#archive";
pub(crate) const TAG_RESIDENT: &str = "#resident";
pub(crate) const TAG_PRIVATE: &str = "#private";
pub(crate) const TAG_NOHISTORY: &str = "#nohistory";
pub(crate) const TAG_MONITOR: &str = "#monitor";
pub(crate) const TAG_UNREAD: &str = "#unread";
pub(crate) const TAG_FOCUS: &str = "#focus";
pub(crate) const TAG_CLIP: &str = "#clip";

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(derive(Debug, PartialEq, Eq))]
pub(crate) enum Badge {
    Crashed,
    WorkspaceCount(usize),
    Pinned,
    Starred,
    Unread,
    /// Content-type badge derived from `mime_hint` (e.g. PDF, Image, Audio, Directory).
    ContentType { label: String, icon: BadgeIcon },
    Tag { label: String, icon: BadgeIcon },
}

// Canonical definitions live in `graphshell_core::types`.
pub use graphshell_core::types::{BadgeIcon, NodeTagPresentationState};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct BadgeVisual {
    pub(crate) token: String,
    pub(crate) label: String,
}

pub(crate) fn badges_for_node(node: &Node, workspace_count: usize, is_crashed: bool) -> Vec<Badge> {
    let mut badges = badges_for_tags_with_presentation(
        &node.tags,
        Some(&node.tag_presentation),
        workspace_count,
        is_crashed,
    );

    // Prepend the primary accepted/verified classification as a semantic badge
    // (spec §Badge Policy: semantic badges outrank backend badges in graph view).
    // Only show if not already present via the tag path to avoid duplication.
    let mut has_classification = false;
    if let Some(primary) = node.classifications.iter().find(|c| {
        c.primary
            && matches!(
                c.status,
                super::ClassificationStatus::Accepted | super::ClassificationStatus::Verified
            )
    }) {
        let label = if let Some(l) = &primary.label {
            l.clone()
        } else {
            primary.value.clone()
        };
        // Insert before any Tag badges so it leads the badge row
        let insert_pos = badges
            .iter()
            .position(|b| matches!(b, Badge::Tag { .. }))
            .unwrap_or(badges.len());
        badges.insert(
            insert_pos,
            Badge::Tag {
                label,
                icon: BadgeIcon::None,
            },
        );
        has_classification = true;
    }

    // Content-type badge derived from mime_hint (placed after classification
    // badges but before user-tag badges).
    if let Some(badge) = content_type_badge_for_node(node) {
        let insert_pos = badges
            .iter()
            .position(|b| matches!(b, Badge::Tag { .. }))
            .map(|p| if has_classification { p + 1 } else { p })
            .unwrap_or(badges.len());
        badges.insert(insert_pos, badge);
    }

    badges
}

/// Derive a content-type badge from a node's `mime_hint` and address kind.
fn content_type_badge_for_node(node: &Node) -> Option<Badge> {
    use super::AddressKind;

    if matches!(node.address.address_kind(), AddressKind::Directory) {
        return Some(Badge::ContentType {
            label: "Directory".to_string(),
            icon: BadgeIcon::Emoji("📁".to_string()),
        });
    }

    let mime = node.mime_hint.as_deref()?;
    let (label, icon) = if mime == "application/pdf" {
        ("PDF", "📄")
    } else if mime.starts_with("image/") {
        ("Image", "🖼")
    } else if mime.starts_with("audio/") {
        ("Audio", "🔊")
    } else if mime.starts_with("video/") {
        ("Video", "🎬")
    } else if mime.starts_with("text/") || is_structured_text_mime(mime) {
        ("Text", "📝")
    } else {
        return None;
    };

    Some(Badge::ContentType {
        label: label.to_string(),
        icon: BadgeIcon::Emoji(icon.to_string()),
    })
}

fn is_structured_text_mime(mime: &str) -> bool {
    matches!(
        mime,
        "application/json"
            | "application/toml"
            | "application/yaml"
            | "application/xml"
            | "application/javascript"
            | "application/typescript"
    )
}

pub(crate) fn badges_for_tags(
    tags: &HashSet<String>,
    workspace_count: usize,
    is_crashed: bool,
) -> Vec<Badge> {
    badges_for_tags_with_presentation(tags, None, workspace_count, is_crashed)
}

pub(crate) fn badges_for_tags_with_presentation(
    tags: &HashSet<String>,
    presentation: Option<&NodeTagPresentationState>,
    workspace_count: usize,
    is_crashed: bool,
) -> Vec<Badge> {
    let mut badges = Vec::new();

    if is_crashed {
        badges.push(Badge::Crashed);
    }
    if workspace_count >= 2 {
        badges.push(Badge::WorkspaceCount(workspace_count));
    }
    if tags.contains(TAG_PIN) {
        badges.push(Badge::Pinned);
    }
    if tags.contains(TAG_STARRED) {
        badges.push(Badge::Starred);
    }
    if tags.contains(TAG_UNREAD) {
        badges.push(Badge::Unread);
    }

    for tag in ordered_tags(tags, presentation) {
        if matches!(tag.as_str(), TAG_PIN | TAG_STARRED | TAG_UNREAD) {
            continue;
        }

        if let Some(code) = tag.strip_prefix("udc:") {
            badges.push(Badge::Tag {
                label: code.to_string(),
                icon: BadgeIcon::None,
            });
            continue;
        }

        badges.push(Badge::Tag {
            label: tag.clone(),
            icon: icon_for_tag(&tag, presentation),
        });
    }

    badges
}

pub(crate) fn badge_visuals(badges: &[Badge]) -> Vec<BadgeVisual> {
    badges
        .iter()
        .map(|badge| BadgeVisual {
            token: compact_badge_token(badge),
            label: badge_label(badge),
        })
        .collect()
}

pub(crate) fn compact_badge_token(badge: &Badge) -> String {
    match badge {
        Badge::Crashed => "⚠".to_string(),
        Badge::WorkspaceCount(count) => count.to_string(),
        Badge::Pinned => "📌".to_string(),
        Badge::Starred => "⭐".to_string(),
        Badge::Unread => "●".to_string(),
        Badge::ContentType { label, icon } | Badge::Tag { label, icon } => match icon {
            BadgeIcon::Emoji(value) => value.clone(),
            BadgeIcon::Lucide(_) => first_grapheme_fallback(label),
            BadgeIcon::None => first_grapheme_fallback(label),
        },
    }
}

pub(crate) fn tab_badge_token(badge: &Badge) -> Option<String> {
    match badge {
        Badge::Crashed => Some("●".to_string()),
        Badge::Pinned => Some("📌".to_string()),
        Badge::Starred => Some("⭐".to_string()),
        Badge::Unread => Some("●".to_string()),
        Badge::ContentType {
            icon: BadgeIcon::Emoji(value),
            ..
        }
        | Badge::Tag {
            icon: BadgeIcon::Emoji(value),
            ..
        } => Some(value.clone()),
        Badge::ContentType { .. } | Badge::Tag { .. } | Badge::WorkspaceCount(_) => None,
    }
}

pub(crate) fn is_archived_tag(tag: &str) -> bool {
    tag == TAG_ARCHIVE
}

pub(crate) fn is_clip_tag(tag: &str) -> bool {
    tag == TAG_CLIP
}

fn icon_for_tag(tag: &str, presentation: Option<&NodeTagPresentationState>) -> BadgeIcon {
    if !tag.starts_with('#')
        && !tag.starts_with("udc:")
        && let Some(icon) = presentation.and_then(|state| state.icon_overrides.get(tag))
    {
        return icon.clone();
    }
    default_icon_for_tag(tag)
}

fn default_icon_for_tag(tag: &str) -> BadgeIcon {
    match tag {
        TAG_ARCHIVE => BadgeIcon::Emoji("🗄".to_string()),
        TAG_RESIDENT => BadgeIcon::Emoji("🏠".to_string()),
        TAG_PRIVATE => BadgeIcon::Emoji("🔒".to_string()),
        TAG_NOHISTORY => BadgeIcon::Emoji("🚫".to_string()),
        TAG_MONITOR => BadgeIcon::Emoji("👁".to_string()),
        TAG_FOCUS => BadgeIcon::Emoji("🎯".to_string()),
        TAG_CLIP => BadgeIcon::Emoji("✂".to_string()),
        _ => BadgeIcon::None,
    }
}

fn badge_label(badge: &Badge) -> String {
    match badge {
        Badge::Crashed => "Crashed".to_string(),
        Badge::WorkspaceCount(count) => format!("{count} workspaces"),
        Badge::Pinned => TAG_PIN.to_string(),
        Badge::Starred => TAG_STARRED.to_string(),
        Badge::Unread => TAG_UNREAD.to_string(),
        Badge::ContentType { label, .. } | Badge::Tag { label, .. } => label.clone(),
    }
}

fn ordered_tags(
    tags: &HashSet<String>,
    presentation: Option<&NodeTagPresentationState>,
) -> Vec<String> {
    let mut ordered = Vec::new();
    if let Some(presentation) = presentation {
        for tag in &presentation.ordered_tags {
            if tags.contains(tag) {
                ordered.push(tag.clone());
            }
        }
    }

    let mut remaining = tags
        .iter()
        .filter(|tag| !ordered.contains(tag))
        .cloned()
        .collect::<Vec<_>>();
    remaining.sort();
    ordered.extend(remaining);
    ordered
}

fn first_grapheme_fallback(label: &str) -> String {
    label
        .chars()
        .next()
        .map(|ch| ch.to_uppercase().collect::<String>())
        .unwrap_or_else(|| "?".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badges_for_pinned_node_prioritizes_pin() {
        let tags = [TAG_PIN.to_string()].into_iter().collect();
        assert_eq!(badges_for_tags(&tags, 1, false), vec![Badge::Pinned]);
    }

    #[test]
    fn badges_for_starred_node_prioritizes_star() {
        let tags = [TAG_STARRED.to_string()].into_iter().collect();
        assert_eq!(badges_for_tags(&tags, 1, false), vec![Badge::Starred]);
    }

    #[test]
    fn badges_priority_order_places_crash_and_workspace_first() {
        let tags = [
            TAG_PIN.to_string(),
            TAG_STARRED.to_string(),
            "work".to_string(),
        ]
        .into_iter()
        .collect();
        assert_eq!(
            badges_for_tags(&tags, 2, true),
            vec![
                Badge::Crashed,
                Badge::WorkspaceCount(2),
                Badge::Pinned,
                Badge::Starred,
                Badge::Tag {
                    label: "work".to_string(),
                    icon: BadgeIcon::None
                }
            ]
        );
    }

    #[test]
    fn compact_badge_token_uses_emoji_or_initial() {
        assert_eq!(compact_badge_token(&Badge::Pinned), "📌");
        assert_eq!(
            compact_badge_token(&Badge::Tag {
                label: "research".to_string(),
                icon: BadgeIcon::None,
            }),
            "R"
        );
    }

    #[test]
    fn badges_for_node_prefers_presentation_order_and_icon_override() {
        let mut node = Node::test_stub("https://example.com");
        node.tags.insert("research".to_string());
        node.tags.insert("work".to_string());
        node.tag_presentation.ordered_tags = vec!["work".to_string(), "research".to_string()];
        node.tag_presentation
            .icon_overrides
            .insert("work".to_string(), BadgeIcon::Emoji("🔬".to_string()));

        let badges = badges_for_node(&node, 1, false);
        assert_eq!(
            badges,
            vec![
                Badge::Tag {
                    label: "work".to_string(),
                    icon: BadgeIcon::Emoji("🔬".to_string()),
                },
                Badge::Tag {
                    label: "research".to_string(),
                    icon: BadgeIcon::None,
                },
            ]
        );
    }

    #[test]
    fn ordered_tags_fall_back_to_sorted_membership_for_missing_entries() {
        let mut node = Node::test_stub("https://example.com");
        node.tags.insert("research".to_string());
        node.tags.insert("alpha".to_string());
        node.tag_presentation.ordered_tags = vec!["missing".to_string(), "research".to_string()];

        let badges = badges_for_node(&node, 1, false);
        assert_eq!(
            badges,
            vec![
                Badge::Tag {
                    label: "research".to_string(),
                    icon: BadgeIcon::None,
                },
                Badge::Tag {
                    label: "alpha".to_string(),
                    icon: BadgeIcon::None,
                },
            ]
        );
    }
}
