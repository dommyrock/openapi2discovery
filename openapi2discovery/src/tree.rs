#[derive(Debug, Clone)]
pub enum Segment {
    Static(String),
    Param(String),
}

/// Parse `/users/{userId}/files/{fileId}` into typed segments.
pub fn parse_segments(path: &str) -> Vec<Segment> {
    path.split('/')
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.strip_prefix('{')
                .and_then(|s| s.strip_suffix('}'))
                .map(|name| Segment::Param(name.to_owned()))
                .unwrap_or_else(|| Segment::Static(s.to_owned()))
        })
        .collect()
}

/// Extract the static resource names forming the nesting hierarchy.
pub fn resource_chain(segments: &[Segment]) -> Vec<String> {
    segments
        .iter()
        .filter_map(|s| match s {
            Segment::Static(name) => Some(name.clone()),
            Segment::Param(_) => None,
        })
        .collect()
}

/// True if the path targets a specific item (ends with `{param}`).
pub fn ends_with_param(segments: &[Segment]) -> bool {
    matches!(segments.last(), Some(Segment::Param(_)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_path() {
        let segs = parse_segments("/users/{userId}/files/{fileId}");
        assert_eq!(segs.len(), 4);
        assert!(matches!(&segs[0], Segment::Static(s) if s == "users"));
        assert!(matches!(&segs[1], Segment::Param(s) if s == "userId"));
        assert!(matches!(&segs[2], Segment::Static(s) if s == "files"));
        assert!(matches!(&segs[3], Segment::Param(s) if s == "fileId"));
        assert_eq!(resource_chain(&segs), ["users", "files"]);
        assert!(ends_with_param(&segs));
    }

    #[test]
    fn collection_path() {
        let segs = parse_segments("/pets");
        assert_eq!(resource_chain(&segs), ["pets"]);
        assert!(!ends_with_param(&segs));
    }
}
