/*!
An algorithm to find a SQL Schema capable of representing a given JSON-Schema

The algorithm is meant to be used with the JSON-Schema published for the SysML-v2 API, so it is
not intended as a generic JSON-Schema to SQL Schema translator. It is also made for SQLite in
particular, and may be incompatible with other databases.

The following design principles were established to guide the conversion process:

1. **One table for all elements**. The primary idea is, that it should be easy (both in terms of
   writing the query and in computational complexity) to query for one element. Having one table to
   house all the elements satisfies this, i.e. `SELECT * FROM "elements" WHERE "@id" = '..'`.
2. **One table for all relations**. This allows querying to be simple: a relation between two
   elements exists exactly if (and only if) there is at least one row in the relations table
   containing the ids of both elements.
3. **One table for all properties of a one-to-many cardinality**. If one property is of the type
   *array of string*, it shall become a part of an external. TODO revisit this choice.
4. **UUIDs are stored as TEXT**. This is less efficient, but simplifies most queries tremendously.
   TODO revisit this choice.
*/

use eyre::{Result, bail, ensure};
use rusqlite::Connection;
use std::collections::{BTreeMap, BTreeSet};

mod json_schema;
mod sql;

use json_schema::*;
use sql::*;

use crate::config::{ELEMENT_PK_COL, POLYMORPHIC_PROPS};

pub(crate) fn consume_json_schema(
    schema: &Root,
    maybe_conn: Option<&mut Connection>,
) -> Result<String> {
    let now = std::time::Instant::now();

    let Root { defs, schema: _ } = schema;

    debug!("found {} definitions", defs.len());

    let mut columns: BTreeMap<_, BTreeSet<_>> = BTreeMap::new();

    let mut problematic_cases = BTreeSet::new();

    // iterate through all definitions
    for (def_name, def) in defs {
        trace!("Processing definition {def_name:?}");
        match &def.ty {
            // Case: the definition is an object containing attributes
            Type::Concrete(ConcreteType::Object { properties, .. }) => {
                handle_properties(properties.iter(), &mut columns, &mut problematic_cases)?;
            }

            // Case: the definition is a string
            s @ Type::Concrete(ConcreteType::String { .. }) => {
                // TODO how to represent this
                problematic_cases.insert(s.clone());
            }
            Type::Composite(CompositeType::AnyOf { any_of }) => {
                for ty in any_of {
                    match ty {
                        Type::Concrete(ConcreteType::Object { properties, .. }) => {
                            handle_properties(
                                properties.iter(),
                                &mut columns,
                                &mut problematic_cases,
                            )?;
                        }

                        Type::Composite(_r @ CompositeType::Ref { .. }) => {
                            // TODO do we have to do anything with references?
                        }

                        unexpected_def => {
                            bail!(
                                "Found a definition of unexpected type inside of composite inside {def_name}:\n{unexpected_def:#?}"
                            )
                        }
                    }
                }
            }

            // TODO check this
            Type::Composite(CompositeType::OneOf { one_of }) => {
                for ty in one_of {
                    match ty {
                        Type::Concrete(ConcreteType::Object { properties, .. }) => {
                            handle_properties(
                                properties.iter(),
                                &mut columns,
                                &mut problematic_cases,
                            )?;
                        }

                        Type::Composite(_r @ CompositeType::Ref { .. }) => {
                            // TODO do we have to do anything with references?
                        }

                        unexpected_def => {
                            bail!(
                                "Found a definition of unexpected type inside of composite inside {def_name}:\n{unexpected_def:#?}"
                            )
                        }
                    }
                }
            }

            unexpected_def => bail!("Found a definition of unexpected type: {unexpected_def:#?}"),
        }
    }

    for (name, repr) in columns.iter().filter(|(_, v)| v.len() > 1) {
        use std::fmt::Write;

        let definitions_formattted: String = repr.iter().fold(String::new(), |mut output, r| {
            let _ = write!(output, "\n\t{r:?}");
            output
        });
        debug!("conflicting definitions for column {name:?}:{definitions_formattted}");
    }

    info!("fusing polymorphic SQL representations");
    let mut fused_columns = BTreeMap::new();
    for (name, reprs) in &columns {
        if POLYMORPHIC_PROPS.contains(&name.as_str()) {
            // TODO handle the existence of value both in the relations and the main table
            continue;
        }

        let mut reprs = reprs.iter();
        let first = reprs.next().unwrap().clone();
        let final_repr = reprs.try_fold(first, |mut acc, other_repr| {
            let result = acc.fuse(other_repr, name);
            result.map(|_| acc)
        });
        fused_columns.insert(name.to_string(), final_repr?);
    }

    for name in POLYMORPHIC_PROPS {
        if let Some(x) = fused_columns.insert(
            name.to_string(),
            SqlRepresentation::Column {
                unique: false,
                null: true,
                id_foreign_key_constraint: false,
                ty: "ANY".to_owned(),
            },
        ) {
            bail!(
                "there was already a column present for the known polymorphic property {name}:\n{x:#?}"
            );
        }
    }

    debug!("Pathologic cases:\n{problematic_cases:#?}");

    let create_table = sql::to_create_table(&fused_columns)?;
    debug!("schema conversion took {:?}", now.elapsed());

    trace!("The following SQL schema was generated:\n{create_table}");

    if let Some(conn) = maybe_conn {
        info!("running CREATE TABLE statements in db");
        conn.execute_batch(&create_table)?;
    }

    Ok(create_table)
}

/// Handles a JSON schema property, collecting its [`SqlRepresentation`]s
///
/// # Arguments
///
/// - `properties`: Iterator over `(property name, property)` tuples
/// - `columns`: Set of [`SqlRepresentation`]s to represent a given property
/// - `problems`: Set of properties that have no [`SqlRepresentation`]
fn handle_properties<I: Iterator<Item = (U, T)>, U: AsRef<str>, T: AsRef<Type>>(
    properties: I,
    columns: &mut BTreeMap<String, BTreeSet<SqlRepresentation>>,
    problems: &mut BTreeSet<Type>,
) -> Result<()> {
    for (prop_name, prop) in properties {
        let prop_name = prop_name.as_ref();
        let Ok(new_repr): Result<_, _> =
            SqlRepresentation::try_from_json_schema_ty(prop_name, prop.as_ref())
        else {
            problems.insert(prop.as_ref().clone());
            continue;
        };

        // special check for the primary key id property
        if prop_name == ELEMENT_PK_COL {
            if let SqlRepresentation::Column {
                null,
                id_foreign_key_constraint,
                unique,
                ..
            } = new_repr
            {
                ensure!(
                    unique,
                    "the {ELEMENT_PK_COL:?} field is expected to be unique"
                );
                ensure!(
                    !id_foreign_key_constraint,
                    "foreign_key_constraint is expected to be false for the {ELEMENT_PK_COL:?} property"
                );
                ensure!(!null, "the {ELEMENT_PK_COL:?} must not allow null values");
            } else {
                bail!("the {ELEMENT_PK_COL:?} property must be resolved to a column!");
            }
        }

        let curr_reprs = columns.entry(prop_name.to_string()).or_default();
        curr_reprs.insert(new_repr);
    }
    Ok(())
}
