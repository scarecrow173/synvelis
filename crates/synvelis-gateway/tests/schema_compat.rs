//! Schema compatibility and `ValueSchema` validation tests.

use std::collections::BTreeMap;

use serde_json::json;
use synvelis_gateway::*;
use synvelis_project::{ClipId, SourceId};

#[test]
fn schema_ref_compatibility_rule() {
    let declared = SchemaRef::new("x", 1, 2);
    // Same name, same major, minor at or below declared → compatible.
    assert!(SchemaRef::new("x", 1, 0).is_compatible_with(&declared));
    assert!(SchemaRef::new("x", 1, 2).is_compatible_with(&declared));
    // Newer minor → incompatible (the server does not know it).
    assert!(!SchemaRef::new("x", 1, 3).is_compatible_with(&declared));
    // Different major → incompatible.
    assert!(!SchemaRef::new("x", 2, 0).is_compatible_with(&declared));
    // Different name → incompatible.
    assert!(!SchemaRef::new("y", 1, 0).is_compatible_with(&declared));
}

#[test]
fn schema_registry_resolution() {
    let mut reg = SchemaRegistry::new();
    reg.register("s", SchemaVersion::new(1, 0), ValueSchema::Bool)
        .unwrap();
    reg.register("s", SchemaVersion::new(1, 1), ValueSchema::Bool)
        .unwrap();

    assert!(reg.resolve(&SchemaRef::new("s", 1, 0)).is_ok());
    assert!(reg.resolve(&SchemaRef::new("s", 1, 1)).is_ok());
    assert_eq!(
        reg.resolve(&SchemaRef::new("s", 2, 0)).unwrap_err(),
        GatewayError::UnsupportedSchemaVersion {
            name: "s".to_owned(),
            requested: SchemaVersion::new(2, 0),
            supported: vec![SchemaVersion::new(1, 0), SchemaVersion::new(1, 1)],
        }
    );
    assert_eq!(
        reg.resolve(&SchemaRef::new("missing", 1, 0)).unwrap_err(),
        GatewayError::UnknownSchema {
            name: "missing".to_owned()
        }
    );

    // Re-registering the same version is a conflict — a published
    // schema version is immutable.
    assert_eq!(
        reg.register("s", SchemaVersion::new(1, 0), ValueSchema::Number),
        Err(GatewayError::SchemaConflict {
            name: "s".to_owned(),
            version: SchemaVersion::new(1, 0),
        })
    );
}

#[test]
fn value_schema_primitives() {
    let int = ValueSchema::Integer {
        min: Some(1),
        max: Some(127),
    };
    assert!(int.validate("v", &json!(64)).is_ok());
    assert!(matches!(
        int.validate("v", &json!(0)),
        Err(GatewayError::SchemaViolation { .. })
    ));
    assert!(matches!(
        int.validate("v", &json!(128)),
        Err(GatewayError::SchemaViolation { .. })
    ));
    assert!(matches!(
        int.validate("v", &json!("64")),
        Err(GatewayError::SchemaViolation { .. })
    ));

    let e = ValueSchema::Enum {
        values: vec!["a".to_owned(), "b".to_owned()],
    };
    assert!(e.validate("v", &json!("a")).is_ok());
    assert!(matches!(
        e.validate("v", &json!("c")),
        Err(GatewayError::SchemaViolation { .. })
    ));
}

#[test]
fn value_schema_id_kind() {
    let id = ValueSchema::Id {
        kind: ResourceKind::Source,
    };
    assert!(
        id.validate("v", &json!(SourceId::new(3).to_tagged()))
            .is_ok()
    );
    // A clip tag where a source is required → typed kind error.
    assert_eq!(
        id.validate("v", &json!(ClipId::new(3).to_tagged())),
        Err(GatewayError::WrongResourceKind {
            expected: ResourceKind::Source,
            actual: ResourceKind::Clip,
        })
    );
    // Malformed id text.
    assert!(matches!(
        id.validate("v", &json!("nonsense")),
        Err(GatewayError::InvalidId { .. })
    ));
}

#[test]
fn value_schema_time_range() {
    let range = ValueSchema::TimeRange {
        domain: TimeDomain::SourceNative,
    };
    assert!(
        range
            .validate(
                "v",
                &json!({"domain": "source-native", "start": 0, "len": 480})
            )
            .is_ok()
    );
    assert_eq!(
        range.validate(
            "v",
            &json!({"domain": "project-musical", "start": 0, "len": 480})
        ),
        Err(GatewayError::WrongTimeDomain {
            expected: TimeDomain::SourceNative,
            actual: TimeDomain::ProjectMusical,
        })
    );
    assert!(matches!(
        range.validate(
            "v",
            &json!({"domain": "source-native", "start": 0, "len": 0})
        ),
        Err(GatewayError::SchemaViolation { .. })
    ));
    assert!(matches!(
        range.validate("v", &json!({"domain": "wat", "start": 0, "len": 480})),
        Err(GatewayError::SchemaViolation { .. })
    ));
}

#[test]
fn value_schema_object() {
    let mut fields = BTreeMap::new();
    fields.insert("req".to_owned(), FieldSchema::required(ValueSchema::Bool));
    fields.insert("opt".to_owned(), FieldSchema::optional(ValueSchema::Number));
    let obj = ValueSchema::Object {
        fields,
        deny_unknown: true,
    };
    assert!(obj.validate("v", &json!({"req": true})).is_ok());
    assert!(obj.validate("v", &json!({"req": true, "opt": 1.5})).is_ok());
    // Missing required field.
    assert!(matches!(
        obj.validate("v", &json!({})),
        Err(GatewayError::SchemaViolation { .. })
    ));
    // Unknown field denied.
    assert!(matches!(
        obj.validate("v", &json!({"req": true, "extra": 1})),
        Err(GatewayError::SchemaViolation { .. })
    ));
    // Permissive object accepts the extra field.
    let mut fields = BTreeMap::new();
    fields.insert("req".to_owned(), FieldSchema::required(ValueSchema::Bool));
    let open = ValueSchema::Object {
        fields,
        deny_unknown: false,
    };
    assert!(
        open.validate("v", &json!({"req": true, "extra": 1}))
            .is_ok()
    );
}

#[test]
fn value_schema_nested_paths() {
    let mut inner = BTreeMap::new();
    inner.insert(
        "len".to_owned(),
        FieldSchema::required(ValueSchema::Integer {
            min: Some(1),
            max: None,
        }),
    );
    let arr = ValueSchema::Array {
        item: Box::new(ValueSchema::Object {
            fields: inner,
            deny_unknown: true,
        }),
        max_len: Some(2),
    };
    assert!(arr.validate("v", &json!([{"len": 1}, {"len": 2}])).is_ok());
    // Nested violation reports the element path.
    match arr.validate("v", &json!([{"len": 1}, {"len": 0}])) {
        Err(GatewayError::SchemaViolation { path, .. }) => {
            assert_eq!(path, "v[1].len");
        }
        other => panic!("expected nested SchemaViolation, got {other:?}"),
    }
    assert!(matches!(
        arr.validate("v", &json!([{"len": 1}, {"len": 1}, {"len": 1}])),
        Err(GatewayError::SchemaViolation { .. })
    ));
}
