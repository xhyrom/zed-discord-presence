use super::update::UpdateFromJson;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum RulesMode {
    Whitelist,
    Blacklist,
}

impl Default for RulesMode {
    fn default() -> Self {
        Self::Blacklist
    }
}

#[derive(Debug, Clone)]
pub struct Rules {
    pub mode: RulesMode,
    pub paths: Vec<String>,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            mode: RulesMode::default(),
            paths: Vec::new(),
        }
    }
}

impl Rules {
    pub fn suitable(&self, path: &str) -> bool {
        let contains = self.paths.contains(&path.to_string());
        match self.mode {
            RulesMode::Blacklist => !contains,
            RulesMode::Whitelist => contains,
        }
    }
}

impl UpdateFromJson for Rules {
    fn update_from_json(&mut self, json: &Value) -> Result<(), super::ConfigError> {
        if let Some(mode) = json.get("mode").and_then(Value::as_str) {
            self.mode = match mode {
                "whitelist" => RulesMode::Whitelist,
                "blacklist" => RulesMode::Blacklist,
                _ => RulesMode::default(),
            };
        }

        if let Some(paths) = json.get("paths").and_then(Value::as_array) {
            self.paths = paths
                .iter()
                .filter_map(|p| p.as_str().map(String::from))
                .collect();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_rules() {
        let rules = Rules::default();
        assert_eq!(rules.mode, RulesMode::Blacklist);
        assert!(rules.paths.is_empty());
    }

    #[test]
    fn test_suitable() {
        let mut rules = Rules::default();
        rules.paths = vec!["/test/path".to_string()];

        // Test blacklist mode
        assert!(!rules.suitable("/test/path"));
        assert!(rules.suitable("/other/path"));

        // Test whitelist mode
        rules.mode = RulesMode::Whitelist;
        assert!(rules.suitable("/test/path"));
        assert!(!rules.suitable("/other/path"));
    }

    #[test]
    fn test_update_from_json() {
        let mut rules = Rules::default();
        let json = serde_json::json!({
            "mode": "whitelist",
            "paths": ["/path1", "/path2"]
        });

        rules.update_from_json(&json).unwrap();

        assert_eq!(rules.mode, RulesMode::Whitelist);
        assert_eq!(rules.paths, vec!["/path1", "/path2"]);
    }
}
