use std::collections::HashMap;

use egui::Color32;
use nucleo::{
    Config, Matcher,
    pattern::{CaseMatching, Normalization, Pattern},
};
use parking_lot::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KnowledgeProvider {
    Udc,
    Schema,
    Unknown,
}

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

pub struct KnowledgeRegistry {
    definitions: HashMap<String, String>,
    matcher: Mutex<Matcher>,
    search_items: Vec<UdcEntry>,
}

impl Default for KnowledgeRegistry {
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

impl KnowledgeRegistry {
    fn provider_for_tag(tag: &str) -> KnowledgeProvider {
        let trimmed = tag.trim();
        if trimmed.starts_with("udc:") {
            return KnowledgeProvider::Udc;
        }
        if trimmed.starts_with("schema:") {
            return KnowledgeProvider::Schema;
        }
        if trimmed
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == ':')
        {
            return KnowledgeProvider::Udc;
        }
        KnowledgeProvider::Unknown
    }

    fn parse_udc_code(code_str: &str) -> Option<CompactCode> {
        let bytes: Vec<u8> = code_str
            .chars()
            .filter(|c| c.is_ascii_digit())
            .map(|c| c as u8 - b'0')
            .collect();
        if bytes.is_empty() {
            return None;
        }
        Some(CompactCode(bytes))
    }

    fn seed_defaults(&mut self) {
        let defaults = [
            ("0", "Generalities. Knowledge. Organization"),
            ("00", "Prolegomena. Fundamentals"),
            ("001", "Knowledge. Epistemology"),
            ("002", "Documentation. Information science"),
            ("003", "Systems theory. Cybernetics"),
            ("004", "Computer science & technology"),
            ("004.3", "Computer hardware"),
            ("004.4", "Software"),
            ("004.5", "Human-computer interaction"),
            ("004.6", "Data processing"),
            ("004.7", "Computer networks"),
            ("004.8", "Artificial intelligence"),
            ("005", "Management"),
            ("01", "Bibliography"),
            ("02", "Library science"),
            ("03", "Reference works. Encyclopedias"),
            ("06", "Organizations. Museums"),
            ("07", "Journalism. Newspapers"),
            ("08", "Polygraphies. Collected works"),
            ("1", "Philosophy. Psychology"),
            ("11", "Metaphysics"),
            ("13", "Philosophy of mind. Philosophical anthropology"),
            ("14", "Philosophical systems"),
            ("159.9", "Psychology"),
            ("16", "Logic. Epistemology. Theory of knowledge"),
            ("17", "Ethics. Moral philosophy"),
            ("2", "Religion. Theology"),
            ("24", "Buddhism"),
            ("26", "Judaism"),
            ("27", "Christianity"),
            ("28", "Islam"),
            ("3", "Social sciences"),
            ("30", "Theories & methods in social sciences"),
            ("31", "Statistics. Demography"),
            ("32", "Politics"),
            ("33", "Economics"),
            ("34", "Law. Jurisprudence"),
            ("35", "Public administration"),
            ("36", "Social problems & welfare"),
            ("37", "Education"),
            ("39", "Ethnology. Ethnography. Customs"),
            ("5", "Mathematics & Natural Sciences"),
            ("50", "Environmental sciences"),
            ("51", "Mathematics"),
            ("510", "Fundamental mathematics"),
            ("511", "Number theory"),
            ("512", "Algebra"),
            ("514", "Geometry"),
            ("515", "Mathematical analysis"),
            ("517", "Calculus"),
            ("519", "Computational mathematics. Numerical analysis"),
            ("519.2", "Probability. Mathematical statistics"),
            ("519.6", "Computational mathematics"),
            ("519.7", "Mathematical cybernetics"),
            ("519.8", "Operations research"),
            ("52", "Astronomy. Astrophysics"),
            ("53", "Physics"),
            ("531", "Classical mechanics. Dynamics"),
            ("534", "Acoustics"),
            ("535", "Optics"),
            ("536", "Heat. Thermodynamics"),
            ("537", "Electricity. Electromagnetism"),
            ("538", "Magnetism"),
            ("539", "Modern physics. Quantum mechanics"),
            ("54", "Chemistry"),
            ("541", "Physical chemistry. Chemical physics"),
            ("542", "Laboratory techniques"),
            ("543", "Analytical chemistry"),
            ("544", "Quantum chemistry"),
            ("546", "Inorganic chemistry"),
            ("547", "Organic chemistry"),
            ("548", "Crystallography"),
            ("549", "Mineralogy"),
            ("55", "Earth sciences. Geology"),
            ("551", "Geology"),
            ("551.5", "Meteorology"),
            ("56", "Paleontology"),
            ("57", "Biological sciences"),
            ("571", "Cell biology"),
            ("572", "Biochemistry. Molecular biology"),
            ("573", "General biology"),
            ("575", "Genetics. Evolution"),
            ("576", "Microbiology"),
            ("577", "Ecology"),
            ("58", "Botany"),
            ("59", "Zoology"),
            ("6", "Applied sciences. Technology"),
            ("60", "Biotechnology"),
            ("61", "Medicine. Health"),
            ("611", "Anatomy"),
            ("612", "Physiology"),
            ("613", "Hygiene. Public health"),
            ("615", "Pharmacology. Therapeutics"),
            ("616", "Pathology. Clinical medicine"),
            ("617", "Surgery. Orthopedics"),
            ("62", "Engineering. Technology"),
            ("621", "Mechanical engineering"),
            ("621.3", "Electrical engineering. Electronics"),
            ("621.38", "Electronics"),
            ("621.39", "Telecommunications"),
            ("622", "Mining engineering"),
            ("624", "Civil engineering. Structural engineering"),
            ("625", "Transport engineering"),
            ("626", "Hydraulic engineering"),
            ("627", "Water resources engineering"),
            ("628", "Environmental engineering"),
            ("629", "Aerospace & vehicle engineering"),
            ("63", "Agriculture. Forestry"),
            ("64", "Home economics. Domestic science"),
            ("65", "Business. Management"),
            ("656", "Transport services"),
            ("657", "Accounting"),
            ("658", "Business management"),
            ("66", "Chemical technology"),
            ("67", "Manufacturing industries"),
            ("68", "Industries & trades"),
            ("69", "Building construction"),
            ("7", "The Arts. Recreation. Entertainment"),
            ("71", "Physical planning. Architecture"),
            ("72", "Architecture"),
            ("73", "Sculpture. Plastic arts"),
            ("74", "Drawing. Design"),
            ("75", "Painting"),
            ("76", "Graphic arts. Printmaking"),
            ("77", "Photography"),
            ("78", "Music"),
            ("79", "Entertainment. Games. Sports"),
            ("791", "Film. Cinema"),
            ("792", "Theatre. Drama"),
            ("793", "Social games. Dancing"),
            ("794", "Board games. Chess"),
            ("796", "Sports & games"),
            ("797", "Water sports"),
            ("8", "Language. Linguistics. Literature"),
            ("80", "General linguistics"),
            ("801", "Theory of language"),
            ("802", "Practical linguistics"),
            ("81", "Linguistics & languages"),
            ("811", "Languages"),
            ("82", "Literature"),
            ("820", "English literature"),
            ("830", "German literature"),
            ("840", "French literature"),
            ("850", "Italian literature"),
            ("860", "Spanish literature"),
            ("87", "Classical languages & literatures"),
            ("88", "Classical Greek literature"),
            ("89", "Other literatures"),
            ("9", "Geography. Biography. History"),
            ("90", "Archaeology"),
            ("91", "Geography. Travel"),
            ("92", "Biography"),
            ("93", "History"),
            ("930", "Ancient history"),
            ("94", "General history of Europe"),
            ("95", "General history of Asia"),
            ("96", "General history of Africa"),
            ("97", "General history of North America"),
            ("98", "General history of South America"),
            ("99", "General history of other regions"),
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
        let trimmed = tag.trim();
        match Self::provider_for_tag(trimmed) {
            KnowledgeProvider::Udc => {
                let code = trimmed.strip_prefix("udc:").unwrap_or(trimmed);
                Self::parse_udc_code(code)
            }
            KnowledgeProvider::Schema | KnowledgeProvider::Unknown => None,
        }
    }

    pub fn get_label(&self, code: &str) -> Option<&str> {
        self.definitions.get(code).map(|s| s.as_str())
    }

    pub fn validate(&self, tag: &str) -> bool {
        match Self::provider_for_tag(tag) {
            KnowledgeProvider::Udc => self.parse(tag).is_some(),
            KnowledgeProvider::Schema | KnowledgeProvider::Unknown => false,
        }
    }

    pub fn distance(&self, a: &CompactCode, b: &CompactCode) -> f32 {
        a.distance(b)
    }

    pub fn get_color_hint(&self, code: &CompactCode) -> Option<Color32> {
        code.0.first().map(|&class| match class {
            0 => Color32::from_rgb(150, 150, 150),
            1 => Color32::from_rgb(180, 100, 200),
            2 => Color32::from_rgb(255, 140, 0),
            3 => Color32::from_rgb(100, 150, 250),
            5 => Color32::from_rgb(50, 200, 100),
            6 => Color32::from_rgb(0, 200, 200),
            7 => Color32::from_rgb(250, 100, 100),
            8 => Color32::from_rgb(250, 250, 100),
            9 => Color32::from_rgb(160, 100, 50),
            _ => Color32::GRAY,
        })
    }

    pub fn search(&self, query: &str) -> Vec<UdcEntry> {
        if query.trim().starts_with("schema:") {
            return Vec::new();
        }

        let normalized_query = query.trim().strip_prefix("udc:").unwrap_or(query.trim());
        let mut matcher = self.matcher.lock();
        let pattern = Pattern::parse(normalized_query, CaseMatching::Ignore, Normalization::Smart);
        let matches = pattern.match_list(&self.search_items, &mut matcher);

        matches
            .into_iter()
            .take(10)
            .map(|(item, _score)| item.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_recognizes_udc_tags() {
        let registry = KnowledgeRegistry::default();
        let code = registry.parse("udc:519.6").expect("udc tag should parse");
        assert_eq!(code.0, vec![5, 1, 9, 6]);
    }

    #[test]
    fn parse_accepts_plain_udc_codes() {
        let registry = KnowledgeRegistry::default();
        let code = registry
            .parse("519.6")
            .expect("plain UDC code should parse");
        assert_eq!(code.0, vec![5, 1, 9, 6]);
    }

    #[test]
    fn provider_routing_rejects_non_udc_prefixes_for_parse() {
        let registry = KnowledgeRegistry::default();
        assert!(registry.parse("schema:CreativeWork").is_none());
        assert!(!registry.validate("schema:CreativeWork"));
    }

    #[test]
    fn search_finds_label_first_query() {
        let registry = KnowledgeRegistry::default();
        let hits = registry.search("math");
        assert!(!hits.is_empty(), "math query should return UDC matches");
        assert!(
            hits.iter().any(|entry| entry.code == "51"),
            "mathematics code should be included in top matches"
        );
    }

    #[test]
    fn expanded_dataset_hierarchical_relationships() {
        let registry = KnowledgeRegistry::default();

        let math = registry.parse("udc:51").unwrap();
        let comp_math = registry.parse("udc:519").unwrap();
        let numerical = registry.parse("udc:519.6").unwrap();
        let probability = registry.parse("udc:519.2").unwrap();

        let dist_comp_to_numerical = registry.distance(&comp_math, &numerical);
        let dist_math_to_numerical = registry.distance(&math, &numerical);
        assert!(dist_comp_to_numerical < dist_math_to_numerical);

        let dist_comp_subfields = registry.distance(&numerical, &probability);
        let dist_to_parent = registry.distance(&numerical, &math);
        assert!(dist_comp_subfields < dist_to_parent);
    }

    #[test]
    fn search_finds_diverse_subjects() {
        let registry = KnowledgeRegistry::default();

        let cs_results = registry.search("computer");
        assert!(cs_results.iter().any(|entry| entry.code.starts_with("004")));

        let physics_results = registry.search("quantum");
        assert!(
            physics_results
                .iter()
                .any(|entry| entry.code.starts_with("539"))
        );

        let medicine_results = registry.search("surgery");
        assert!(
            medicine_results
                .iter()
                .any(|entry| entry.code.starts_with("617"))
        );

        let lit_results = registry.search("literature");
        assert!(lit_results.iter().any(|entry| entry.code.starts_with("82")));
    }

    #[test]
    fn distance_calculation_cross_domain() {
        let registry = KnowledgeRegistry::default();

        let math = registry.parse("udc:51").unwrap();
        let physics = registry.parse("udc:53").unwrap();
        let music = registry.parse("udc:78").unwrap();

        let math_to_physics = registry.distance(&math, &physics);
        let math_to_music = registry.distance(&math, &music);
        assert!(math_to_physics < math_to_music);
    }
}
