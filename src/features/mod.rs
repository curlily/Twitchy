pub mod auto_category;

use auto_category::AutoCategory;
use crate::CONFIG;

/// Feature trait
pub trait Feature: Send {
    fn name(&self) -> &str;
    fn is_enabled(&self) -> bool;
    fn start(&mut self);
    fn stop(&mut self);
}

/// Initialize all features from the config
pub fn init_features() -> Vec<Box<dyn Feature>> {
    let mut features: Vec<Box<dyn Feature>> = Vec::new();

    // AutoCategory
    let auto_enabled = CONFIG.features.get("auto_category").cloned().unwrap_or(false);
    let auto_category = AutoCategory::new(auto_enabled);
    features.push(Box::new(auto_category));

    // Add other features here similarly, e.g.:
    // let another_enabled = config.features.get("another_feature").cloned().unwrap_or(false);
    // let another_feature = AnotherFeature::new(another_enabled);
    // features.push(Box::new(another_feature));

    features
}
