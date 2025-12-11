use crate::types::Feature;

pub struct FeatureSelector {
    enabled_features: Vec<String>,
}

impl FeatureSelector {
    pub fn new(enabled_features: Vec<String>) -> Self {
        Self { enabled_features }
    }

    pub fn is_enabled(&self, feature: &Feature) -> bool {
        self.enabled_features.contains(&feature.name)
    }

    pub fn select(&self, features: &[Feature]) -> Vec<Feature> {
        features
            .iter()
            .filter(|f| self.is_enabled(f))
            .cloned()
            .collect()
    }
}

impl Default for FeatureSelector {
    fn default() -> Self {
        Self {
            enabled_features: vec![],
        }
    }
}
