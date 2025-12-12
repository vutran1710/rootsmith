// Feature selection module - placeholder for future feature flags
pub struct FeatureSelector {
    enabled_features: Vec<String>,
}

impl FeatureSelector {
    pub fn new(enabled_features: Vec<String>) -> Self {
        Self { enabled_features }
    }

    pub fn is_enabled(&self, feature_name: &str) -> bool {
        self.enabled_features.contains(&feature_name.to_string())
    }
}

impl Default for FeatureSelector {
    fn default() -> Self {
        Self {
            enabled_features: vec![],
        }
    }
}
