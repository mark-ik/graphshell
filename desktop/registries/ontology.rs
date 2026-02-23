use std::collections::HashMap;
use parking_lot::Mutex;

use crate::app::GraphBrowserApp;
use nucleo::{Config, Matcher, pattern::{CaseMatching, Normalization, Pattern}};
use egui::Color32;

/// A compact representation of a UDC code for fast distance calculations.
/// For MVP, we treat UDC codes as hierarchical paths of bytes.
/// e.g. "519.6" -> [5, 1, 9, 6]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CompactCode(pub Vec<u8>);

impl CompactCode {
    pub fn distance(&self, other: &CompactCode) -> f32 {
        let len = self.0.len().min(other.0.len());
        let mut shared_prefix = 0;
        for i in 0..len {
            if self.0[i] == other.0[i] {
                shared_prefix += 1;
            } else {
                break;
            }
        }
        
        // Simple similarity: ratio of shared prefix to max length.
        // Distance is 1.0 - similarity.
        let max_len = self.0.len().max(other.0.len());
        if max_len == 0 {
            return 1.0;
        }
        
        1.0 - (shared_prefix as f32 / max_len as f32)
    }
}

#[derive(Debug, Clone)]
pub struct UdcEntry {
    pub code: String,
    pub label: String,
    search_text: String,
}

impl AsRef<str> for UdcEntry {
    fn as_ref(&self) -> &str {
        &self.search_text
    }
}

pub struct OntologyRegistry {
    /// Map of "51" -> "Mathematics"
    definitions: HashMap<String, String>,
    /// Fuzzy matcher for search
    matcher: Mutex<Matcher>,
    /// Cache of items for the matcher
    search_items: Vec<UdcEntry>,
}

impl Default for OntologyRegistry {
    fn default() -> Self {
        let mut registry = Self {
            definitions: HashMap::new(),
            matcher: Mutex::new(Matcher::new(Config::DEFAULT)),
            search_items: Vec::new(),
        };
        registry.seed_defaults();
        registry
    }
}

impl OntologyRegistry {
    fn seed_defaults(&mut self) {
        // Minimal MVP dataset
        let defaults = [
            ("0", "Generalities"),
            ("004", "Computer science"),
            ("1", "Philosophy"),
            ("3", "Social Sciences"),
            ("5", "Mathematics & Natural Sciences"),
            ("51", "Mathematics"),
            ("519", "Computational mathematics"),
            ("53", "Physics"),
            ("7", "The Arts"),
            ("8", "Language & Literature"),
            ("9", "Geography & History"),
        ];

        for (code, label) in defaults {
            self.definitions.insert(code.to_string(), label.to_string());
            self.search_items.push(UdcEntry {
                code: code.to_string(),
                label: label.to_string(),
                search_text: format!("{} udc:{} {}", label, code, code),
            });
        }
    }

    pub fn parse(&self, tag: &str) -> Option<CompactCode> {
        if let Some(code_str) = tag.strip_prefix("udc:") {
            // Naive parsing: remove dots, treat each char as a digit
            // Real UDC is more complex, but this suffices for "519.6" -> [5,1,9,6]
            let bytes: Vec<u8> = code_str
                .chars()
                .filter(|c| c.is_ascii_digit())
                .map(|c| c as u8 - b'0')
                .collect();
            if !bytes.is_empty() {
                return Some(CompactCode(bytes));
            }
        }
        None
    }

    pub fn get_label(&self, code: &str) -> Option<&str> {
        self.definitions.get(code).map(|s| s.as_str())
    }

    pub fn validate(&self, tag: &str) -> bool {
        self.parse(tag).is_some()
    }

    pub fn distance(&self, a: &CompactCode, b: &CompactCode) -> f32 {
        a.distance(b)
    }

    pub fn get_color_hint(&self, code: &CompactCode) -> Option<Color32> {
        // Simple MVP coloring based on top-level UDC class (first byte)
        // Colors chosen to be distinct but muted enough for UI hints
        code.0.first().map(|&class| match class {
            0 => Color32::from_rgb(150, 150, 150), // Generalities (Grey)
            1 => Color32::from_rgb(180, 100, 200), // Philosophy (Purple)
            2 => Color32::from_rgb(255, 140, 0),   // Religion (Orange)
            3 => Color32::from_rgb(100, 150, 250), // Social Sciences (Blue)
            // 4 is vacant in UDC
            5 => Color32::from_rgb(50, 200, 100),  // Science (Green)
            6 => Color32::from_rgb(0, 200, 200),   // Technology (Cyan)
            7 => Color32::from_rgb(250, 100, 100), // Arts (Red)
            8 => Color32::from_rgb(250, 250, 100), // Lang/Lit (Yellow)
            9 => Color32::from_rgb(160, 100, 50),  // Geog/Hist (Brown)
            _ => Color32::GRAY,
        })
    }

    pub fn search(&self, query: &str) -> Vec<UdcEntry> {
        let mut matcher = self.matcher.lock();
        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
        let matches = pattern.match_list(&self.search_items, &mut matcher);
        
        matches
            .into_iter()
            .take(10)
            .map(|(item, _score)| item.clone())
            .collect()
    }
}

/// Reconciliation function: updates the app's semantic index based on node tags.
/// This respects the "Data vs System" split: App owns Data, Registry owns Logic.
pub fn reconcile_semantics(app: &mut GraphBrowserApp, registry: &OntologyRegistry) {
    if !app.semantic_index_dirty {
        return;
    }

    app.semantic_tags
        .retain(|key, _| app.graph.get_node(*key).is_some());

    app.semantic_index.clear();
    for (&key, tags) in &app.semantic_tags {
        for tag in tags {
            if let Some(code) = registry.parse(tag) {
                app.semantic_index.insert(key, code);
                // For MVP, we only support one UDC code per node for physics simplicity
                break; 
            }
        }
    }
    app.semantic_index_dirty = false;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::NodeKey;

    #[test]
    fn parse_recognizes_udc_tags() {
        let registry = OntologyRegistry::default();
        let code = registry.parse("udc:519.6").expect("udc tag should parse");
        assert_eq!(code.0, vec![5, 1, 9, 6]);
    }

    #[test]
    fn search_finds_label_first_query() {
        let registry = OntologyRegistry::default();
        let hits = registry.search("math");
        assert!(!hits.is_empty(), "math query should return UDC matches");
        assert!(
            hits.iter().any(|entry| entry.code == "51"),
            "mathematics code should be included in top matches"
        );
    }

    #[test]
    fn reconcile_updates_semantic_index_and_clears_dirty_flag() {
        let registry = OntologyRegistry::default();
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app.add_node_and_sync(
            "https://example.com".to_string(),
            euclid::default::Point2D::new(10.0, 10.0),
        );
        app.semantic_tags
            .insert(key, ["udc:51".to_string()].into_iter().collect());
        app.semantic_index_dirty = true;

        reconcile_semantics(&mut app, &registry);

        assert!(!app.semantic_index_dirty);
        assert_eq!(app.semantic_index.get(&key), Some(&CompactCode(vec![5, 1])));

        let stale = NodeKey::new(999_999);
        app.semantic_tags
            .insert(stale, ["udc:7".to_string()].into_iter().collect());
        app.semantic_index_dirty = true;
        reconcile_semantics(&mut app, &registry);
        assert!(!app.semantic_tags.contains_key(&stale));
    }
}