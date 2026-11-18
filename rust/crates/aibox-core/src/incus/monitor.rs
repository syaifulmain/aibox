use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleEvent {
    pub action: String,
    pub instance_name: Option<String>,
    pub is_image_event: bool,
}

impl LifecycleEvent {
    /// Returns true when this lifecycle event reflects an instance or snapshot
    /// state change that should trigger an immediate environment list refresh.
    pub fn should_trigger_refresh(&self) -> bool {
        self.action.starts_with("instance-") || self.is_image_event
    }

    /// Returns true when this lifecycle event may have changed the local image catalog.
    pub fn invalidates_catalog(&self) -> bool {
        self.is_image_event
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawMonitorLine {
    #[serde(rename = "type")]
    event_type: String,
    metadata: RawLifecycleMetadata,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawLifecycleMetadata {
    action: String,
    source: String,
}

/// Parses a single JSON line from `incus monitor --type=lifecycle --format=json`.
/// Unknown fields are ignored. Returns `None` for non-lifecycle or unparseable lines.
pub fn parse_lifecycle_event(line: &str) -> Option<LifecycleEvent> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let raw: RawMonitorLine = serde_json::from_str(trimmed).ok()?;
    if !raw.event_type.is_empty() && raw.event_type != "lifecycle" {
        return None;
    }
    if raw.metadata.action.is_empty() {
        return None;
    }
    let instance_name = extract_instance_name(&raw.metadata.source);
    let is_image_event = raw.metadata.action.starts_with("image-")
        || raw.metadata.source.starts_with("/1.0/images/");
    Some(LifecycleEvent {
        action: raw.metadata.action,
        instance_name,
        is_image_event,
    })
}

fn extract_instance_name(source: &str) -> Option<String> {
    let rest = source.strip_prefix("/1.0/instances/")?;
    let name = rest.split('/').next().unwrap_or("").trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_instance_lifecycle_event_and_ignores_unknown_fields() {
        let line = r#"{"timestamp":"2026-11-18T10:00:00Z","type":"lifecycle","project":"default","metadata":{"action":"instance-started","source":"/1.0/instances/mybox","requestor":{"username":"saep"}}}"#;
        let ev = parse_lifecycle_event(line).expect("parse lifecycle event");
        assert_eq!(ev.action, "instance-started");
        assert_eq!(ev.instance_name.as_deref(), Some("mybox"));
        assert!(!ev.is_image_event);
        assert!(ev.should_trigger_refresh());
        assert!(!ev.invalidates_catalog());
    }

    #[test]
    fn parses_image_lifecycle_event_and_invalidates_catalog() {
        let line = r#"{"type":"lifecycle","metadata":{"action":"image-created","source":"/1.0/images/abc123"}}"#;
        let ev = parse_lifecycle_event(line).expect("parse image event");
        assert_eq!(ev.action, "image-created");
        assert_eq!(ev.instance_name, None);
        assert!(ev.is_image_event);
        assert!(ev.should_trigger_refresh());
        assert!(ev.invalidates_catalog());
    }

    #[test]
    fn ignores_non_lifecycle_or_invalid_lines() {
        assert!(parse_lifecycle_event("").is_none());
        assert!(parse_lifecycle_event("not json").is_none());
        assert!(
            parse_lifecycle_event(r#"{"type":"logging","metadata":{"action":"foo"}}"#).is_none()
        );
    }
}
