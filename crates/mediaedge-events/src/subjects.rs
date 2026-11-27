//! NATS JetStream subject definitions.

/// Event subject prefix.
pub const SUBJECT_PREFIX: &str = "mediaedge";

/// Domain identifiers.
pub mod domain {
    pub const STREAM: &str = "stream";
    pub const CLUSTER: &str = "cluster";
    pub const TENANT: &str = "tenant";
    pub const MONETIZATION: &str = "monetization";
    pub const STORAGE: &str = "storage";
    pub const REPLICATION: &str = "replication";
}

/// Action identifiers.
pub mod action {
    pub const CREATED: &str = "created";
    pub const UPDATED: &str = "updated";
    pub const DELETED: &str = "deleted";
    pub const STARTED: &str = "started";
    pub const STOPPED: &str = "stopped";
    pub const ERROR: &str = "error";
    pub const METRICS: &str = "metrics";
}

/// Build a subject string.
pub fn build_subject(domain: &str, action: &str, entity: &str, id: &str) -> String {
    format!("{}.{}.{}.{}.{}", SUBJECT_PREFIX, domain, action, entity, id)
}

/// Parse a subject string.
pub fn parse_subject(subject: &str) -> Option<(String, String, String, String)> {
    let parts: Vec<&str> = subject.split('.').collect();
    if parts.len() == 5 && parts[0] == SUBJECT_PREFIX {
        Some((
            parts[1].to_string(),
            parts[2].to_string(),
            parts[3].to_string(),
            parts[4].to_string(),
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_subject() {
        let s = build_subject(domain::STREAM, action::CREATED, "stream", "123");
        assert_eq!(s, "mediaedge.stream.created.stream.123");
    }

    #[test]
    fn test_parse_subject() {
        let parsed = parse_subject("mediaedge.stream.created.stream.123");
        assert_eq!(
            parsed,
            Some((
                "stream".into(),
                "created".into(),
                "stream".into(),
                "123".into()
            ))
        );
    }
}
