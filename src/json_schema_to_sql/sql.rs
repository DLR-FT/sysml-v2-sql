use std::collections::BTreeMap;

use eyre::{bail, ensure, Result};

use crate::{
    config::{ELEMENTS_TABLE, ELEMENT_PK_COL, EXTENDED_TABLE, POLYMORPHIC_PROPS, RELATIONS_TABLE},
    util::{escape_sql_ident, escape_sql_str_lit},
};

use super::{CompositeType, ConcreteType, Type};

/// Enum that describes how something from the JSON-Schema will be represented in our SQL schema
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum SqlRepresentation {
    /// Represent it as a column in the main table containing all elements
    Column {
        unique: bool,
        null: bool,
        id_foreign_key_constraint: bool,
        ty: String,
    },

    /// Represent it via the table containing relations
    ///
    /// This is relevant whenever one element references multiple other elements via one property.
    RelationsTable,

    /// TODO find out when we need this
    ExtendedPropertiesTable,
}

impl SqlRepresentation {
    /// Try to fuse two SQL representations, promoting the less general one to the more general one
    ///
    /// Specific semantics are up for debate.
    ///
    /// # Returns
    ///
    /// - true if something was done,
    /// - false if nothing was done, i.e. if `self` and `other` where identical
    // TODO add a string with checks, whenever `null` differs
    pub(super) fn fuse(&mut self, other: &Self, column_name: &str) -> Result<bool> {
        let s = self.clone();
        let o = other.clone();

        use SqlRepresentation::*;
        match (self, other) {
            // fuse of identical representations is a nop
            (s, o) if s == o => {
                return Ok(false);
            }

            // fuse of representations is fine, but:
            // - null allowed dominates
            // - id_foreign_key_constraint must be equal, except for the `value` column
            //   `value` is the only column we allow to be truly polymorphic, being eithere a
            //   reference to another element or a literal value itself
            // TODO revisit the excemption for `value`
            (
                Column {
                    unique: self_uniq,
                    null: self_null,
                    id_foreign_key_constraint: self_fkc,
                    ty: self_ty,
                },
                Column {
                    unique: other_uniq,
                    null: other_null,
                    id_foreign_key_constraint: other_fkc,
                    ty: other_ty,
                },
            ) => {
                ensure!(
                    self_fkc == other_fkc || POLYMORPHIC_PROPS.contains(&column_name),
                    "Fusing two SqlRepresentations with differing id_foreign_key_constraint values for column {column_name:?}: {s:?}, {o:?}, prop = {column_name:?}"
                );

                // allow relaxation of varying TEXT types
                if !(self_ty.starts_with("TEXT") && other_ty.starts_with("TEXT")) {
                    ensure!(
                    self_ty == other_ty,
                    "Fusing two SqlRepresentations with differing type: {self_ty} vs. {other_ty}"
                );
                } else if self_ty.starts_with("TEXT") && other_ty.starts_with("TEXT") {
                    *self_ty = "TEXT".to_string();
                }

                ensure!(self_uniq == other_uniq, "Uniqueness must not differ");

                if self_null != other_null {
                    debug!("Propagating that null is allowed for column {column_name:?}");
                    *self_null |= other_null;
                }
            }

            // a column is dominated by a relation table if the columns is reference to another
            // element
            (
                s @ Column {
                    id_foreign_key_constraint: true,
                    ..
                },
                RelationsTable,
            ) => {
                // TODO check type
                // ensure!(
                //     ty == "TEXT",
                //     "If foreign_key_constraint is true, the type must be TEXT"
                // );
                debug!("Upgrading column {column_name:?} from {s:?} to {other:?}");
                *s = other.clone();
            }

            // relation table prevails over column, if that column is a reference to another element
            (
                s @ RelationsTable,
                o @ Column {
                    id_foreign_key_constraint: true,
                    ty,
                    ..
                },
            ) => {
                ensure!(
                    ty == "TEXT",
                    "If foreign_key_constraint is true, the type must be TEXT"
                );
                debug!(
                    "Not doing anything on SqlRepresentation of column {column_name:?}, as {s:?} is more general than {o:?}"
                )
            }

            // other cases are treated as error
            (s, o) => {
                bail!(
                    "Fusing column {column_name:?} representation {s:?} with {o:?} is impossible"
                );
            }
        }

        Ok(true)
    }
}

/// WIP function to generate SQL statements
///
/// # Remaining issues
///
pub(super) fn to_create_table(columns: &BTreeMap<String, SqlRepresentation>) -> Result<String> {
    let create_table = |table_name, inner| {
        format!(
            "CREATE TABLE {} (\n{inner}\n) STRICT;\n",
            escape_sql_ident(table_name)
        )
    };

    let mut column_defs = vec![];

    for (name, repr) in columns {
        match repr {
            SqlRepresentation::Column {
                unique,
                null,
                id_foreign_key_constraint,
                ty,
            } => {
                let mut column_def = vec![];

                // column-name
                column_def.push(escape_sql_ident(name));

                // type-name
                column_def.push(ty.to_owned());

                // column-constraint
                if name == ELEMENT_PK_COL {
                    column_def.push("PRIMARY KEY".to_string());
                }

                if !null {
                    // TODO figure out NOT NULL stuff via trigger
                    //     column_def.push("NOT NULL".to_string());
                }

                if *unique {
                    column_def.push("UNIQUE".to_string());
                }

                // foreign-key-clause
                if *id_foreign_key_constraint {
                    column_def.push("REFERENCES".to_string());
                    column_def.push(escape_sql_ident(ELEMENTS_TABLE));
                    column_def.push(format!("({})", escape_sql_ident(ELEMENT_PK_COL)));
                }

                column_defs.push(column_def.join(" "));
            }

            // ignore representations about other tables
            SqlRepresentation::RelationsTable | SqlRepresentation::ExtendedPropertiesTable => {}
        }
    }

    let mut stmt = create_table(
        ELEMENTS_TABLE,
        column_defs
            .iter()
            .map(|cd| format!("\t{cd}"))
            .collect::<Vec<_>>()
            .join(",\n"),
    );
    column_defs.clear();
    stmt += "\n\n";

    //
    // this concludes the elements table, now the relations table
    //

    let main_table_escaped = escape_sql_ident(ELEMENTS_TABLE);
    let pk_column_escaped = escape_sql_ident(ELEMENT_PK_COL);
    let allowed_relation_names = columns
        .iter()
        .filter_map(|(n, c)| match c {
            SqlRepresentation::RelationsTable => Some(n.to_owned()),
            _ => None,
        })
        .chain(POLYMORPHIC_PROPS.into_iter().map(str::to_string))
        .chain(std::iter::once("analysisAction".to_owned())) // TODO remove hot-fix
        .map(escape_sql_str_lit)
        .collect::<Vec<_>>()
        .join(",\n\t\t");

    // TODO rename 'name' to 'property'
    stmt.push_str(&create_table(
        RELATIONS_TABLE,
        format!(
            r#"    "name" TEXT NOT NULL CHECK("name" IN ({allowed_relation_names})),
	"origin_id" TEXT NOT NULL,
	"target_id" TEXT NOT NULL,
	FOREIGN KEY("origin_id") REFERENCES {main_table_escaped}({pk_column_escaped}) DEFERRABLE INITIALLY DEFERRED,
	FOREIGN KEY("target_id") REFERENCES {main_table_escaped}({pk_column_escaped}) DEFERRABLE INITIALLY DEFERRED,
	PRIMARY KEY("name","origin_id","target_id")"#
        ),
    ));
    stmt += "\n\n";

    //
    // this concludes the relations table, now the extended_properties table
    //

    column_defs.push(format!(
        "{} TEXT NOT NULL",
        escape_sql_ident(ELEMENT_PK_COL)
    ));
    for (name, repr) in columns {
        match repr {
            SqlRepresentation::ExtendedPropertiesTable => {
                // TODO maybe type the extended properties properly?
                let column_def = [
                    // column-name
                    escape_sql_ident(name),
                    // type-name
                    "TEXT".to_string(),
                ];

                column_defs.push(column_def.join(" "));
            }

            // ignore other representations
            SqlRepresentation::Column { .. } | SqlRepresentation::RelationsTable => {}
        }
    }
    column_defs.push(format!(
        "FOREIGN KEY({pk_column_escaped}) REFERENCES {main_table_escaped}({pk_column_escaped}) DEFERRABLE INITIALLY DEFERRED"
    ));

    stmt.push_str(&create_table(
        EXTENDED_TABLE,
        column_defs
            .iter()
            .map(|cd| format!("\t{cd}"))
            .collect::<Vec<_>>()
            .join(",\n"),
    ));
    column_defs.clear();

    stmt += "\n\n";

    // and finally, add indexes for quicker lookups
    stmt.push_str(&create_index());

    Ok(stmt)
}

impl SqlRepresentation {
    /// Tries to convert a [`Type`] into a [`SqlRepresentation`]
    // TODO maybe emit SQL Check constraints as side-effect of transformation
    pub(super) fn try_from_json_schema_ty(prop_name: &str, prop: &Type) -> Result<Self> {
        let null = ConcreteType::Null;

        let sql_repr = match prop {
            // array of identifieables
            Type::Concrete(ConcreteType::Array { items }) if identified_ref(items) => {
                SqlRepresentation::RelationsTable
            }

            // array of strings
            Type::Concrete(ConcreteType::Array { items })
                if items.as_ref()
                    == &Type::Concrete(ConcreteType::String {
                        enumeration: None,
                        format: None,
                        constant: None,
                    }) =>
            {
                SqlRepresentation::ExtendedPropertiesTable
            }

            // string which must be unique and adhere to a specific format
            // TODO set column type to string
            // TODO trigger to check values matches the UUID format
            Type::Concrete(ConcreteType::String {
                enumeration: None,
                format: Some(format),
                constant: None,
            }) if format == "uuid" => SqlRepresentation::Column {
                null: false,
                id_foreign_key_constraint: false,
                unique: true,
                ty: "TEXT".to_string(),
            },

            // a string
            // TODO add trigger that fails if the wrong data was set?
            ty @ Type::Concrete(ConcreteType::String { .. }) => SqlRepresentation::Column {
                null: false,
                id_foreign_key_constraint: false,
                unique: false,
                ty: json_schema_type_to_sql_type(ty, prop_name)?,
            },

            // reference to exactly one other element
            Type::Composite(CompositeType::Ref { reference }) if identified_str(reference) => {
                SqlRepresentation::RelationsTable

                // TODO this is not clever, it violates design principle 2
                // SqlRepresentation::Column {
                //     null: false,
                //     id_foreign_key_constraint: true,
                //     unique: false,
                //     ty: "TEXT".to_string(),
                // }
            }

            // either a reference to one other element or null
            Type::Composite(CompositeType::OneOf { one_of })
                if one_of.len() == 2
                    && one_of.contains(&null.clone().into())
                    && one_of.iter().any(identified_ref) =>
            {
                SqlRepresentation::RelationsTable

                // TODO this is not clever, it violates design principle 2
                // SqlRepresentation::Column {
                //     null: true,
                //     id_foreign_key_constraint: true,
                //     unique: false,
                //     ty: "TEXT".to_string(),
                // }
            }

            // either something or null
            Type::Composite(CompositeType::OneOf { one_of })
                if one_of.len() == 2 && one_of.contains(&null.clone().into()) =>
            {
                // get the other type (other than `ConcreteType::Null`), derive its best fitting
                // SQL counterpart
                let other_json_type = one_of.iter().find(|x| *x != &null).unwrap();

                SqlRepresentation::Column {
                    null: true,
                    id_foreign_key_constraint: false,
                    unique: false,
                    ty: json_schema_type_to_sql_type(other_json_type, prop_name)?,
                }
            }

            x => bail!("Unsure how to represent {x:#?}"),
        };

        Ok(sql_repr)
    }
}

//
// Helper functions
//

/// Convert a JSON-Schema type to a SQLite type, assuming the JSON-Schema type to be a
/// [`Type::Concrete`]
///
/// See <https://www.sqlite.org/datatype3.html> for more information.
// TODO add emitation of check/constraints?
fn json_schema_type_to_sql_type(json_ty: &Type, column_name: &str) -> Result<String> {
    let column_name_escaped = escape_sql_ident(column_name);

    let ty = match json_ty {
        Type::Concrete(ConcreteType::String {
            enumeration: Some(variants),
            format: None,
            constant: None,
        }) => {
            let legal_variants = variants
                .iter()
                .map(escape_sql_str_lit)
                .collect::<Vec<_>>()
                .join(", ");
            format!("TEXT CHECK({column_name_escaped} IN ({legal_variants}))")
        }

        Type::Concrete(ConcreteType::String {
            enumeration: None,
            format: Some(format),
            constant: None,
        }) => match format.as_str() {
            // see <https://json-schema.org/understanding-json-schema/reference/string>
            // and <https://datatracker.ietf.org/doc/html/rfc4122>
            "uuid" => {
                // TODO this is weak
                let uuid_like_pattern = "________-____-____-____-____________";
                let uuid_like_pattern_escaped = escape_sql_str_lit(uuid_like_pattern);
                format!("TEXT CHECK({column_name_escaped} LIKE ({uuid_like_pattern_escaped}))")
            }
            _ => {
                bail!("There is no SQLite type for format {format:?} defined");
            }
        },

        Type::Concrete(ConcreteType::String {
            enumeration: None,
            format: None,
            constant: Some(legal_value),
        }) => {
            let column_name_escaped = escape_sql_ident(column_name);
            let legal_value_escaped = escape_sql_str_lit(legal_value);
            format!("TEXT CHECK({column_name_escaped} = ({legal_value_escaped}))")
        }

        Type::Concrete(ConcreteType::String { .. }) => "TEXT".to_string(),
        Type::Concrete(ConcreteType::Integer) | Type::Concrete(ConcreteType::Boolean) => {
            "INTEGER".to_string()
        }
        Type::Concrete(ConcreteType::Number) => "REAL".to_string(),
        _ => bail!("There is no suitable SQLite counterpart type for {json_ty:#?} defined"),
    };

    Ok(ty)
}

/// Check if a string ends with the definition id of Identified
fn identified_str<S: AsRef<str>>(str_to_check: S) -> bool {
    str_to_check.as_ref().ends_with("/Identified")
}

/// Check if a Type is a reference to an element
fn identified_ref(t_to_check: &Type) -> bool {
    matches!(t_to_check, Type::Composite(CompositeType::Ref{reference}) if identified_str(reference))
}

// Function to create indexes on relevant columns
fn create_index() -> String {
    let create_index = |table, column| {
        let index_name_escaped = escape_sql_ident(format!("{table}.{column}"));
        let table_name_escaped = escape_sql_ident(table);
        let column_name_escaped = escape_sql_ident(column);
        format!(
            "DROP INDEX IF EXISTS {index_name_escaped};\n\
            CREATE INDEX {index_name_escaped} ON {table_name_escaped}\
            ({column_name_escaped});\n\n"
        )
    };

    let idxs = [
        (
            "elements",
            &[
                // "@id, // is already contained, its the primary key
                "@type",
                "declaredName",
                "declaredShortName",
                "isLibraryElement",
                "name",
                "qualifiedName",
                "value",
            ][..], // make Rust treat this as slice, not ref to fixed size array
        ),
        (
            "relations",
            &[
                //"name", // as all three columns are the primary index, name is not required
                "origin_id",
                "target_id",
            ],
        ),
    ];

    let mut result = String::new();
    for (table, columns) in idxs {
        for column in columns {
            result = result + &create_index(table, column);
        }
    }
    result
}
