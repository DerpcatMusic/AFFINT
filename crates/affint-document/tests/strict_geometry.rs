//! Native-schema geometry oracles using an independently authored archive fixture.
//!
//! Valid controls and targeted nested mutations check strict field acceptance,
//! original wire shapes, and retained geometry validation through the public seam.
//! Native storage owns the separate correctly rounded JSON number parsing oracle.

use affint_document::{
    Archive, Color, Content, Error, Limits, LiveOperation, Session, VectorContent,
};
use kurbo::{BezPath, PathEl, Point, Rect};
use serde_json::{Value, json};

// Independently written schema-1 fixture, rather than a document self-round-trip.
// All scalar values are exact binary fractions so codec rounding is a separate
// native-storage oracle, not an accidental confounder for these shape checks.
const SUPPORTED_ARCHIVE: &str = r#"{
  "schema":1,
  "next_id":3,
  "pages":[{"id":1,"name":"Page","size":[16.0,12.0],"roots":[2]}],
  "assets":[],
  "layers":[{
    "id":2,"name":"Geometry","parent":{"Page":1},"children":[],
    "content":{"Vector":{
      "path":[
        {"MoveTo":{"x":0.0,"y":0.0}},
        {"LineTo":{"x":1.0,"y":0.5}},
        {"QuadTo":[{"x":2.0,"y":0.25},{"x":3.0,"y":1.5}]},
        {"CurveTo":[{"x":4.0,"y":2.5},{"x":5.0,"y":3.25},{"x":6.0,"y":4.0}]},
        "ClosePath"
      ],
      "fill":[0.25,0.5,0.75,1.0]
    }},
    "transform":[1.0,0.0,0.0,1.0,0.0,0.0],
    "opacity":1.0,"visible":true,"mask":null,
    "operations":[{"Crop":{"x0":0.25,"y0":0.5,"x1":8.0,"y1":10.0}},{"Exposure":0.75}]
  }]
}"#;

fn control() -> Session {
    let archive = serde_json::from_str::<Archive>(SUPPORTED_ARCHIVE).unwrap();
    Session::from_archive(archive, Limits::default()).unwrap()
}

fn fixture() -> Value {
    // A valid published control proves mutations below target nested members,
    // rather than relying on an already-invalid fixture to report some error.
    control();
    serde_json::from_str(SUPPORTED_ARCHIVE).unwrap()
}

#[test]
fn strict_geometry_accepts_the_preexisting_supported_native_schema() {
    let session = control();
    let snapshot = session.snapshot();
    let layer = snapshot.layers().next().unwrap().1;
    let Content::Vector(vector) = layer.content() else {
        panic!("fixture lost vector content")
    };
    assert_eq!(
        vector.path().elements(),
        &[
            PathEl::MoveTo(Point::new(0.0, 0.0)),
            PathEl::LineTo(Point::new(1.0, 0.5)),
            PathEl::QuadTo(Point::new(2.0, 0.25), Point::new(3.0, 1.5)),
            PathEl::CurveTo(
                Point::new(4.0, 2.5),
                Point::new(5.0, 3.25),
                Point::new(6.0, 4.0)
            ),
            PathEl::ClosePath,
        ]
    );
    assert_eq!(
        layer.operations(),
        &[
            LiveOperation::Crop(Rect::new(0.25, 0.5, 8.0, 10.0)),
            LiveOperation::Exposure(0.75),
        ]
    );
    assert_eq!(
        serde_json::to_value(snapshot.archive().unwrap()).unwrap(),
        fixture()
    );
}

#[test]
fn crop_payload_refuses_a_nested_unknown_member_in_an_otherwise_valid_archive() {
    let mut archive = fixture();
    archive["layers"][0]["operations"][0]["Crop"]
        .as_object_mut()
        .unwrap()
        .insert(
            "future_crop_rule".to_owned(),
            json!("retain rather than silently discard"),
        );
    let error = serde_json::from_value::<Archive>(archive).unwrap_err();
    assert!(error.to_string().contains("future_crop_rule"));
}

#[test]
fn every_path_point_payload_refuses_nested_unknown_members() {
    // Probe every point slot, including both quadratic and all three cubic points.
    for (element, variant, point) in [
        (0, "MoveTo", None),
        (1, "LineTo", None),
        (2, "QuadTo", Some(0)),
        (2, "QuadTo", Some(1)),
        (3, "CurveTo", Some(0)),
        (3, "CurveTo", Some(1)),
        (3, "CurveTo", Some(2)),
    ] {
        let mut archive = fixture();
        let point_payload =
            &mut archive["layers"][0]["content"]["Vector"]["path"][element][variant];
        let point_payload = if let Some(index) = point {
            &mut point_payload[index]
        } else {
            point_payload
        };
        point_payload
            .as_object_mut()
            .unwrap()
            .insert("future_point_rule".to_owned(), json!({"retained":true}));
        let error = serde_json::from_value::<Archive>(archive).unwrap_err();
        assert!(
            error.to_string().contains("future_point_rule"),
            "{variant} slot {point:?}: {error}"
        );
    }
}

#[test]
fn geometry_serialization_matches_pinned_kurbo_and_independent_literal_shapes() {
    let session = control();
    let snapshot = session.snapshot();
    let layer = snapshot.layers().next().unwrap().1;
    let Content::Vector(vector) = layer.content() else {
        panic!("missing vector")
    };
    let encoded = serde_json::to_value(vector).unwrap();
    assert_eq!(
        encoded["path"],
        serde_json::to_value(vector.path()).unwrap()
    );
    assert_eq!(
        encoded["path"],
        json!([
            {"MoveTo":{"x":0.0,"y":0.0}},
            {"LineTo":{"x":1.0,"y":0.5}},
            {"QuadTo":[{"x":2.0,"y":0.25},{"x":3.0,"y":1.5}]},
            {"CurveTo":[{"x":4.0,"y":2.5},{"x":5.0,"y":3.25},{"x":6.0,"y":4.0}]},
            "ClosePath"
        ])
    );
    assert_eq!(
        serde_json::to_string(&layer.operations()[0]).unwrap(),
        r#"{"Crop":{"x0":0.25,"y0":0.5,"x1":8.0,"y1":10.0}}"#
    );
    assert_eq!(
        serde_json::to_string(vector).unwrap(),
        r#"{"path":[{"MoveTo":{"x":0.0,"y":0.0}},{"LineTo":{"x":1.0,"y":0.5}},{"QuadTo":[{"x":2.0,"y":0.25},{"x":3.0,"y":1.5}]},{"CurveTo":[{"x":4.0,"y":2.5},{"x":5.0,"y":3.25},{"x":6.0,"y":4.0}]},"ClosePath"],"fill":[0.25,0.5,0.75,1.0]}"#
    );
}

#[test]
fn strict_point_and_rect_adapters_refuse_duplicate_fields() {
    control();
    let duplicate_point = SUPPORTED_ARCHIVE.replacen("\"x\":0.0", "\"x\":0.0,\"x\":1.0", 1);
    let duplicate_rect = SUPPORTED_ARCHIVE.replacen("\"x0\":0.25", "\"x0\":0.25,\"x0\":1.0", 1);
    for input in [duplicate_point, duplicate_rect] {
        let error = serde_json::from_str::<Archive>(&input).unwrap_err();
        assert!(error.to_string().contains("duplicate field"));
    }
}

#[test]
fn strict_geometry_keeps_existing_subpath_and_live_crop_validation() {
    let mut invalid_path = fixture();
    invalid_path["layers"][0]["content"]["Vector"]["path"][0] = json!({"LineTo":{"x":0.0,"y":0.0}});
    let error = serde_json::from_value::<Archive>(invalid_path).unwrap_err();
    assert!(error.to_string().contains("vector subpath"));

    let mut invalid_crop = fixture();
    invalid_crop["layers"][0]["operations"][0]["Crop"]["x1"] = json!(-1.0);
    let archive = serde_json::from_value::<Archive>(invalid_crop).unwrap();
    assert_eq!(
        Session::from_archive(archive, Limits::default()).unwrap_err(),
        Error::InvalidValue("live crop rectangle")
    );
}

#[test]
fn malformed_starts_and_closed_subpath_continuations_return_decode_errors() {
    for path in [
        json!([{"LineTo":{"x":0.0,"y":0.0}}]),
        json!([{"QuadTo":[{"x":0.0,"y":0.0},{"x":1.0,"y":1.0}]}]),
        json!([{"CurveTo":[{"x":0.0,"y":0.0},{"x":1.0,"y":1.0},{"x":2.0,"y":2.0}]}]),
        json!(["ClosePath"]),
        json!([{"MoveTo":{"x":0.0,"y":0.0}},"ClosePath",{"LineTo":{"x":1.0,"y":1.0}}]),
        json!([{"MoveTo":{"x":0.0,"y":0.0}},"ClosePath","ClosePath"]),
    ] {
        let mut archive = fixture();
        archive["layers"][0]["content"]["Vector"]["path"] = path;
        let error = serde_json::from_value::<Archive>(archive).unwrap_err();
        assert!(error.to_string().contains("vector subpath"), "{error}");
    }
}

#[test]
fn typed_vector_constructor_validates_borrowed_geometry_before_canonicalization() {
    let fill = Color::new(0.25, 0.5, 0.75, 1.0).unwrap();
    // Kurbo exposes editable elements. A caller can therefore supply a malformed
    // path without using its asserting constructors; our public seam must refuse it.
    let mut malformed = BezPath::new();
    malformed.move_to((0.0, 0.0));
    malformed.elements_mut()[0] = PathEl::LineTo(Point::new(0.0, 0.0));
    assert_eq!(
        VectorContent::new(malformed, fill).unwrap_err(),
        Error::InvalidValue("vector subpath")
    );

    let mut nonfinite = BezPath::new();
    nonfinite.move_to((0.0, 0.0));
    nonfinite.curve_to((f64::NAN, 0.0), (1.0, 1.0), (2.0, 2.0));
    assert_eq!(
        VectorContent::new(nonfinite, fill).unwrap_err(),
        Error::InvalidValue("vector path coordinates")
    );
}

#[test]
fn empty_and_explicitly_restarted_subpaths_keep_supported_behavior() {
    for (path, length) in [
        (json!([]), 0),
        (
            json!([
                {"MoveTo":{"x":0.0,"y":0.0}},
                {"LineTo":{"x":1.0,"y":1.0}},
                "ClosePath",
                {"MoveTo":{"x":2.0,"y":2.0}},
                {"QuadTo":[{"x":3.0,"y":2.0},{"x":4.0,"y":4.0}]}
            ]),
            5,
        ),
    ] {
        let mut archive = fixture();
        archive["layers"][0]["content"]["Vector"]["path"] = path;
        let archive = serde_json::from_value::<Archive>(archive).unwrap();
        let session = Session::from_archive(archive, Limits::default()).unwrap();
        let snapshot = session.snapshot();
        let Content::Vector(vector) = snapshot.layers().next().unwrap().1.content() else {
            panic!("lost vector")
        };
        assert_eq!(vector.path().elements().len(), length);
    }
}
