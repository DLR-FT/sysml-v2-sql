//! This module contains serde deriving Rust data types to deserialize JSON Schema

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Deserialize, Serialize, PartialOrd, Ord, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Root {
    #[serde(rename = "$schema")]
    pub(super) schema: String,

    #[serde(rename = "$defs")]
    pub(super) defs: BTreeMap<String, Definition>,
}

#[derive(Debug, Deserialize, Serialize, PartialOrd, Ord, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct Definition {
    #[serde(rename = "$id")]
    pub(super) id: String,

    pub(super) title: Option<String>,

    #[serde(flatten)]
    pub(super) ty: Type,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialOrd, Ord, PartialEq, Eq)]
#[serde(rename_all_fields = "camelCase", untagged)]
pub(super) enum Type {
    Concrete(ConcreteType),
    Composite(CompositeType),
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialOrd, Ord, PartialEq, Eq)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "type"
)]
pub(super) enum ConcreteType {
    Array {
        items: Box<Type>,
    },
    Object {
        properties: BTreeMap<String, Box<Type>>,
        #[serde(default)]
        required: Vec<String>,
        #[serde(default)]
        additional_properties: bool,
    },
    String {
        // // TODO this field is not always present, how to distinguish Null and string?
        #[serde(default, rename = "enum")]
        enumeration: Option<Vec<String>>,

        #[serde(default)]
        format: Option<String>,

        #[serde(default, rename = "const")]
        constant: Option<String>,
    },
    Null,
    Boolean,
    Number,
    Integer,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialOrd, Ord, PartialEq, Eq)]
#[serde(rename_all_fields = "camelCase", untagged)]
pub(super) enum CompositeType {
    AnyOf {
        any_of: Vec<Type>,
    },
    OneOf {
        one_of: Vec<Type>,
    },
    Ref {
        #[serde(rename = "$ref")]
        reference: String,
    },
}

impl From<ConcreteType> for Type {
    fn from(value: ConcreteType) -> Self {
        Self::Concrete(value)
    }
}

impl From<CompositeType> for Type {
    fn from(value: CompositeType) -> Self {
        Self::Composite(value)
    }
}

impl std::cmp::PartialEq<ConcreteType> for Type {
    fn eq(&self, other: &ConcreteType) -> bool {
        self.eq(&Type::Concrete(other.to_owned()))
    }
}
