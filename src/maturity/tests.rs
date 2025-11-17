//! Tests for maturity-based routing infrastructure

#[cfg(test)]
mod tests {
    use super::super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_maturity_stage_parsing() {
        assert_eq!(
            MaturityStage::from_str("prototype").unwrap(),
            MaturityStage::Prototype
        );
        assert_eq!(
            MaturityStage::from_str("PRODUCTION").unwrap(),
            MaturityStage::Production
        );
        assert_eq!(
            MaturityStage::from_str("Transitioning").unwrap(),
            MaturityStage::Transitioning
        );
        assert!(MaturityStage::from_str("invalid").is_err());
    }

    #[test]
    fn test_language_parsing() {
        assert_eq!(Language::from_str("python").unwrap(), Language::Python);
        assert_eq!(Language::from_str("RUST").unwrap(), Language::Rust);
        assert_eq!(Language::from_str("ts").unwrap(), Language::Typescript);
        assert!(Language::from_str("invalid").is_err());
    }

    #[test]
    fn test_implementation_route_from_toml() {
        let toml_content = r#"
            [meta]
            implementation_stage = "prototype"
            implementation_language = "python"
            implementation_path = "/usr/bin/python3"
            validated_at = "2025-11-15"
            production_language = "rust"
            production_path = "/path/to/binary"
            production_ready = false
            formalization_deadline = "2025-12-31"
        "#;

        let value: toml::Value = toml::from_str(toml_content).unwrap();
        let meta = value.get("meta").unwrap();

        let route = ImplementationRoute::from_toml_meta(meta).unwrap();

        assert_eq!(route.stage, MaturityStage::Prototype);
        assert_eq!(route.language, Language::Python);
        assert_eq!(route.validated_at, Some("2025-11-15".to_string()));

        let prod_route = route.production_route.unwrap();
        assert_eq!(prod_route.language, Language::Rust);
        assert!(!prod_route.ready);
        assert_eq!(prod_route.deadline, Some("2025-12-31".to_string()));
    }

    #[test]
    fn test_implementation_route_minimal() {
        let toml_content = r#"
            [meta]
            implementation_stage = "production"
            implementation_language = "rust"
            implementation_path = "/path/to/binary"
            production_language = "rust"
            production_path = "/path/to/binary"
            production_ready = true
        "#;

        let value: toml::Value = toml::from_str(toml_content).unwrap();
        let meta = value.get("meta").unwrap();

        let route = ImplementationRoute::from_toml_meta(meta).unwrap();

        assert_eq!(route.stage, MaturityStage::Production);
        assert_eq!(route.validated_at, None);
        assert!(route.production_route.unwrap().ready);
    }

    #[test]
    fn test_path_expansion() {
        let home = dirs::home_dir().unwrap();
        let expanded = expand_path("~/.config/test").unwrap();

        assert!(expanded.starts_with(&home));
        assert!(expanded.to_string_lossy().contains(".config/test"));
        assert!(!expanded.to_string_lossy().contains('~'));
    }

    #[test]
    fn test_maturity_stage_metadata() {
        assert_eq!(MaturityStage::Prototype.status_icon(), "🧪");
        assert_eq!(MaturityStage::Production.status_icon(), "🏭");
        assert_eq!(MaturityStage::Transitioning.status_icon(), "🔄");
    }

    #[test]
    fn test_language_executor() {
        assert_eq!(Language::Python.executor(), "python3");
        assert_eq!(Language::Rust.executor(), "cargo");
        assert_eq!(Language::Typescript.executor(), "tsx");
        assert_eq!(Language::Shell.executor(), "bash");
    }

    #[test]
    fn test_missing_required_fields() {
        let toml_content = r#"
            [meta]
            implementation_stage = "prototype"
        "#;

        let value: toml::Value = toml::from_str(toml_content).unwrap();
        let meta = value.get("meta").unwrap();

        let result = ImplementationRoute::from_toml_meta(meta);
        assert!(result.is_err());
    }
}
