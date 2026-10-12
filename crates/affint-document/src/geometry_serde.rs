//! Strict persistence adapters for the pinned Kurbo geometry wire representations.
//!
//! Math stays in Kurbo. These definitions only control field/variant acceptance;
//! vector finite/subpath validation and operation validation remain at their
//! existing document seams. Serialization visits borrowed path elements directly.

use std::fmt;
use std::sync::Arc;

use kurbo::{BezPath, PathEl, Point, Rect};
use serde::de::{SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Serialize, Deserialize)]
#[serde(remote = "Point", rename = "Point", deny_unknown_fields)]
struct PointFields {
    x: f64,
    y: f64,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "PathEl", rename = "PathEl")]
enum PathElement {
    MoveTo(#[serde(with = "PointFields")] Point),
    LineTo(#[serde(with = "PointFields")] Point),
    QuadTo(
        #[serde(with = "PointFields")] Point,
        #[serde(with = "PointFields")] Point,
    ),
    CurveTo(
        #[serde(with = "PointFields")] Point,
        #[serde(with = "PointFields")] Point,
        #[serde(with = "PointFields")] Point,
    ),
    ClosePath,
}

/// The crop remains a Kurbo Rect publicly; only its decoding policy changes.
pub(crate) mod rect {
    use super::*;

    #[derive(Serialize, Deserialize)]
    #[serde(remote = "Rect", rename = "Rect", deny_unknown_fields)]
    struct RectFields {
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
    }

    pub(crate) fn serialize<S>(rect: &Rect, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RectFields::serialize(rect, serializer)
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Rect, D::Error>
    where
        D: Deserializer<'de>,
    {
        RectFields::deserialize(deserializer)
    }
}

/// BezPath's existing newtype sequence, with strict points for every PathEl variant.
pub(crate) mod path {
    use super::*;

    struct BorrowedElement<'a>(&'a PathEl);

    impl Serialize for BorrowedElement<'_> {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            PathElement::serialize(self.0, serializer)
        }
    }

    struct BorrowedElements<'a>(&'a [PathEl]);

    impl Serialize for BorrowedElements<'_> {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
            for element in self.0 {
                sequence.serialize_element(&BorrowedElement(element))?;
            }
            sequence.end()
        }
    }

    #[derive(Deserialize)]
    #[serde(transparent)]
    struct OwnedElement(#[serde(with = "PathElement")] PathEl);

    struct ElementsVisitor;

    impl<'de> Visitor<'de> for ElementsVisitor {
        type Value = Arc<BezPath>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a sequence of supported Bézier path elements")
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            // Do not trust a deserializer's allocation hint. The storage decoder
            // separately bounds input bytes before this private geometry seam.
            let mut elements = Vec::new();
            while let Some(OwnedElement(element)) = sequence.next_element()? {
                elements.push(element);
            }
            // Kurbo's constructor asserts this programmer precondition. Refuse
            // malformed input before invoking it; the VectorInput conversion then
            // performs full finite/subpath validation before canonicalization.
            if !elements.is_empty() && !matches!(elements.first(), Some(PathEl::MoveTo(_))) {
                return Err(serde::de::Error::custom(crate::Error::InvalidValue(
                    "vector subpath",
                )));
            }
            // The existing validated VectorInput conversion canonicalizes ownership
            // once; no second intermediate vector of surrogate path elements exists.
            Ok(Arc::new(BezPath::from_vec(elements)))
        }
    }

    struct PathVisitor;

    impl<'de> Visitor<'de> for PathVisitor {
        type Value = Arc<BezPath>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a Bézier path")
        }

        fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_seq(ElementsVisitor)
        }
    }

    pub(crate) fn serialize<S>(path: &Arc<BezPath>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_newtype_struct("BezPath", &BorrowedElements(path.elements()))
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Arc<BezPath>, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_newtype_struct("BezPath", PathVisitor)
    }
}
