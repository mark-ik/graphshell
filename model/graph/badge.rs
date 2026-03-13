use std::collections::HashSet;

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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum Badge {
    Crashed,
    WorkspaceCount(usize),
    Pinned,
    Starred,
    Unread,
    Tag { label: String, icon: BadgeIcon },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum BadgeIcon {
    Emoji(String),
    Lucide(String),
    None,
}

pub(crate) fn badges_for_tags(
    tags: &HashSet<String>,
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

    let mut remaining = tags.iter().cloned().collect::<Vec<_>>();
    remaining.sort();
    for tag in remaining {
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
            icon: default_icon_for_tag(&tag),
        });
    }

    badges
}

pub(crate) fn compact_badge_token(badge: &Badge) -> String {
    match badge {
        Badge::Crashed => "⚠".to_string(),
        Badge::WorkspaceCount(count) => count.to_string(),
        Badge::Pinned => "📌".to_string(),
        Badge::Starred => "⭐".to_string(),
        Badge::Unread => "●".to_string(),
        Badge::Tag { label, icon } => match icon {
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
        Badge::Tag {
            icon: BadgeIcon::Emoji(value),
            ..
        } => Some(value.clone()),
        Badge::Tag { .. } | Badge::WorkspaceCount(_) => None,
    }
}

pub(crate) fn is_archived_tag(tag: &str) -> bool {
    tag == TAG_ARCHIVE
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
}
