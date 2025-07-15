// TODO Track element ids of thos eelements imported in the current operation, remove all relations of these

use color_eyre::Section;
use eyre::{bail, Result};
use rusqlite::{Connection, Statement, ToSql};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::HashSet;

use crate::{
    config::{ELEMENT_PK_COL, POLYMORPHIC_PROPS, TIME_BETWEEN_STATUS_REPORTS},
    maybe_time_report,
    util::escape_sql_ident,
};

/// JSON representation of an Element in the SysML-v2 API
#[derive(Debug, Clone, Hash, PartialEq, serde::Deserialize, serde::Serialize)]
pub(crate) struct Element {
    #[serde(rename = "@id")]
    pub(crate) id: String,

    #[serde(flatten)]
    pub(crate) rest: Map<String, Value>,
}

/// This function is passed a slice of [`Element`]s, and then calls [`import_from_iter`]
pub(crate) fn import_from_slice(
    elements: &[Element],
    conn: &mut Connection,
    vacuum: bool,
) -> Result<()> {
    let maybe_elements_iter = elements
        .iter()
        .map(|e| -> Result<_, std::convert::Infallible> { Ok(e.to_owned()) });
    import_from_iter(maybe_elements_iter, conn, vacuum)
}

/// # Overview
///
/// This function is passed a cloneable iterator of [`Element`]s. Said iterator is iterated twice.
/// In the first round, all elements are inserted. This round only considers those attributes of
/// each element from the JSON for which an equally named column exists in the `"elements"` table.
///
/// The second round considers each attribute of each element from the JSON. It does however only
/// use those elements which refer to another element (some variation of `{"@id": "..."}`). For
/// each of these a relation is insert into the `"relations"` table. Attributes which are seen are
/// memorized (but not stored in the database!) to warn on irregularities, such as attributes from
/// the JSON which were not used at all in the database.
pub(crate) fn import_from_iter<E: Send + Sync + std::error::Error + 'static>(
    elements: impl Clone + Iterator<Item = Result<Element, E>>,
    conn: &mut Connection,
    vacuum: bool,
) -> Result<()> {
    let import_t0 = std::time::Instant::now();

    crate::tweaks::before_bulk_insert(conn)?;

    debug!("enabling foreign key constraint support");
    conn.pragma_update(None, "foreign_keys", "ON")?;

    debug!("starting db transaction for import");
    let db_ta = conn.transaction()?;

    let elements_table_columns = get_table_columns(&db_ta, "elements")?;
    let extended_properties_table_columns = get_table_columns(&db_ta, "extended_properties")?;

    //
    // Damage tracking
    //

    // Create a temporary table to track which elements where (re-)created by the current import
    db_ta.execute_batch(r#"CREATE TEMPORARY TABLE "inserted_elements"("@id")"#)?;

    //
    // Prepare SQL statements
    //

    // Statement to insert into the elements table
    let statement = format!(
        r#"INSERT OR REPLACE INTO "elements" VALUES ({})"#,
        std::iter::repeat_n("?", elements_table_columns.len())
            .collect::<Vec<_>>()
            .join(", ")
    );
    debug!("prepared the following statement:\n{statement}"); // debug, since statement is actually generated as opposed to being hardcoded.
    let mut e_insert_stmt = db_ta.prepare(&statement)?;

    // Statement to insert into the relations table
    // TODO why do we fail with primary key unique failure with `INSERT INTO`?
    let statement =
        r#"INSERT OR REPLACE INTO "relations"("name", "origin_id", "target_id") VALUES (?, ?, ?)"#;
    trace!("prepared the following statement:\n{statement}");
    let mut r_insert_stmt = db_ta.prepare(statement)?;

    // One statements for each column in the extended_properties table
    let maybe_e_p_insert_stmts: Result<Vec<_>, rusqlite::Error> = extended_properties_table_columns
        .iter()
        .filter(|(col_name, _)| col_name != ELEMENT_PK_COL) // filter out an insert for the first column, the "@id@ primary key
        .map(|(col_name, _)| {
            format!(
                r#"INSERT INTO "extended_properties"("@id", {}) VALUES (?, ?)"#,
                escape_sql_ident(col_name)
            )
        })
        .inspect(|statement| debug!("prepared the following statement:\n{statement}"))
        .map(|statement| db_ta.prepare(&statement))
        .collect();
    let mut e_p_insert_stmts = maybe_e_p_insert_stmts?;
    assert_eq!(
        extended_properties_table_columns.len(),
        e_p_insert_stmts.len() + 1,
        r#"extended_properties_table columns must have exactly one element more than maybe_e_p_insert statements, because there is an insert statement for each column except for the primary key column 0 with the name "@id""#
    );

    // Statement to track those elements inserted during this import for relations/
    // extended_properties damage tracking
    let statement = r#"INSERT INTO "inserted_elements" VALUES (?)"#;
    trace!("prepared the following statement:\n{statement}");
    let mut e_tracking_insert_stmt = db_ta.prepare(statement)?;

    // Statement to remove relations and extended_properties originating from the recently inserted
    // elements
    let statement = r#"
        DELETE FROM "relations" WHERE "origin_id" IN (SELECT "@id" FROM "inserted_elements");
        DELETE FROM "extended_properties" WHERE "@id" IN (SELECT "@id" FROM "inserted_elements");
    "#;
    trace!("prepared the following statement:\n{statement}");
    let mut obsolete_delete_stmt = db_ta.prepare(statement)?;

    //
    // Track unused or misunderstood JSON properties and database columns
    //

    // Explanation
    //
    // The following section declares various data structures to track what kind of attributes in the JSON where imported how into the database.
    //
    // A primitive attribute is one which has a primitive value. These are inserted either into the
    // elements table, or into the extended_properties table.
    //
    // A complex attribute is one which itself is a JSON Object, for example the `{ "@id": "..." }`
    // observed for relations between elements. These will be imported into the relations table.
    //
    // Very few (currently only one, tracked in POLYMORPHIC_PROPS) elements are know to be either
    // primitive or complex. These get special treatment, they either might be inserted into a
    // corresponding column in elements, or into the relations table.

    // tracks all columns in the elements table, which never occured in the JSON
    let mut unused_db_columns: HashSet<_> = elements_table_columns
        .iter()
        .map(|(name, _)| name)
        .cloned()
        .collect();

    // all attributes ever observed in JSON
    let mut observed_json_attrs = HashSet::new();

    // all attributes which at least once occured with a primitive value other than null
    let mut observed_primitive_attrs = HashSet::new();

    // all attributes which where both observed as primitive and as not-primitive and not part of KNOWN_POLYMORPH_FIELDS
    let mut observed_unexpected_polymorph_attrs = HashSet::new();

    // all attributes which where observed at least once as relation (both 1:1 and 1:*)
    let mut observed_relational_attrs = HashSet::new();

    // all attributes which where observed at least once as not a relation but complex
    let mut observed_unexpected_complex_attrs = HashSet::new();

    //
    // Insert elements
    //

    info!("inserting elements");
    let elements_t0 = std::time::Instant::now();
    let mut report_td = TIME_BETWEEN_STATUS_REPORTS;
    let mut elements_inserted = 0;
    for maybe_element in elements.clone() {
        let element = maybe_element?;

        // sporadically report on timing
        maybe_time_report!("element", elements_t0, report_td, elements_inserted);
        elements_inserted += 1;

        let mut db_row_values: Vec<_> = Vec::with_capacity(elements_table_columns.len());

        for (column_name, column_type) in &elements_table_columns {
            // special case: the @id is not in the Element::rest, but in Element::id
            if column_name == ELEMENT_PK_COL {
                db_row_values.push(RusValue::Text(element.id.clone()));
                unused_db_columns.remove(column_name);
                continue;
            }

            let maybe_json_value = element.rest.get(column_name);
            if maybe_json_value.is_some() {
                unused_db_columns.remove(column_name);
            }

            use rusqlite::types::Value as RusValue;
            let db_value = match maybe_json_value {
                None => {
                    trace!(
                        "setting {column_name:?} to NULL, its not present in this element's JSON"
                    );
                    RusValue::Null
                }
                Some(Value::Null) => RusValue::Null,
                Some(Value::Bool(b)) => RusValue::Integer(if *b { 1 } else { 0 }),
                Some(Value::String(s))
                    if column_name.starts_with("is")
                        && column_name
                            .chars()
                            .nth(2)
                            .map(char::is_uppercase)
                            .unwrap_or(false) =>
                {
                    RusValue::Integer(if s.parse()? { 1 } else { 0 })
                }
                Some(Value::Number(n)) if n.is_f64() => {
                    RusValue::Real(n.as_f64().expect("floating point number"))
                }
                Some(Value::Number(n)) => RusValue::Integer(n.as_i64().expect("integer number")),
                Some(Value::String(s)) => RusValue::Text(s.to_string()),
                Some(v @ Value::Array(_)) | Some(v @ Value::Object(_)) => {
                    if POLYMORPHIC_PROPS.iter().any(|kpf| kpf == column_name) {
                        trace!("the {column_name:?} column is known to be polymorph, setting it to NULL");
                    } else {
                        warn!("db expects column {column_name:?} of type {column_type}, but JSON is {v:?}");
                        warn!("skipping this entry, setting it to NULL instead");
                    }
                    RusValue::Null
                }
            };

            db_row_values.push(db_value);
        }

        // TODO remove this ugly vtable hack
        let mut ref_vec = Vec::with_capacity(db_row_values.len());
        for v in &db_row_values {
            ref_vec.push(v as &dyn ToSql);
        }
        assert_eq!(elements_table_columns.len(), db_row_values.len());

        trace!("inserting row for element");
        e_insert_stmt.execute(ref_vec.as_slice())?;

        // retain the information that this element was (re-) inserted by the current import run
        e_tracking_insert_stmt.execute([&element.id])?;
    }

    // finalize all prepared statements which are not used later
    e_insert_stmt.finalize()?;
    e_tracking_insert_stmt.finalize()?;

    // Each relation associated with each element imported during this import run needs to be
    // deleted, to have only those relations from the current import, without remnants from the
    // past.
    debug!(
        "removing relations and extended_properties originating from recently inserted elements"
    );
    obsolete_delete_stmt.execute(())?;
    obsolete_delete_stmt.finalize()?;
    db_ta.execute(r#"DROP TABLE "inserted_elements""#, ())?;

    maybe_time_report!("element", elements_t0, elements_inserted);

    //
    // Insert relations & extended properties
    //

    info!("inserting relations & extended_properties");

    let mut relations_inserted = 0;

    let relations_t0 = std::time::Instant::now();
    report_td = std::time::Duration::from_secs(0);
    for maybe_element in elements {
        let element = maybe_element?;

        // sporadically report on timing
        maybe_time_report!("relation", relations_t0, report_td, relations_inserted);

        // go through all JSON attributes, and try to stuff them into our db
        for (json_attr_name, json_attr_value) in &element.rest {
            observed_json_attrs.insert(json_attr_name.to_owned());

            // check for unknown polymorph fields
            match json_attr_value {
                // an empty attribute is irrelevant for us
                Value::Null => continue,

                // primitive values are just tracked but irrelevant in this import phase
                Value::Bool(_) | Value::Number(_) | Value::String(_) => {
                    observed_primitive_attrs.insert(json_attr_name.to_owned());
                    continue;
                }

                // this is a 1:1 relation (i.e. `{"@id": "..."}` in the JSON)
                o @ Value::Object(json_object) if is_relation_object(json_object) => {
                    let target_element = Element::deserialize(o).unwrap();
                                    trace!("found 1:1 relation of type {json_attr_name}");

                    observed_relational_attrs.insert(json_attr_name.to_owned());
                    relations_inserted += 1;

                    insert_relation(
                        &mut r_insert_stmt,
                        json_attr_name,
                        &element.id,
                        &target_element.id,
                    )?;
                }

                // this is a 1:* relation (i.e. `[{"@id": "..."}]` in the JSON)
                a @ Value::Array(array_elements)
                    if array_elements.iter().all(|v| matches!(v, Value::Object(json_object) if is_relation_object(json_object))) =>
                {
                    // try to parse this as a 1:* relation (i.e. `[{"@id": "..."}]` in the JSON)
                    let target_elements: Vec<Element> = Vec::deserialize(a).unwrap();

                    trace!("found a 1:* relation of type {json_attr_name}");
                    observed_relational_attrs.insert(json_attr_name.to_owned());
                    relations_inserted += target_elements.len();

                    for target_element in target_elements {
                        insert_relation(
                            &mut r_insert_stmt,
                            json_attr_name,
                            &element.id,
                            &target_element.id,
                        )?;
                    }
                }

                // add extended_properties found in the element
                Value::Array(_) if extended_properties_table_columns.iter().any(|(n, _)| n == json_attr_name)  =>{
                    for (column_idx, (column_name, column_type)) in
                        extended_properties_table_columns.iter().enumerate()
                    {
                        let Some(json_value) = element.rest.get(column_name) else {
                            continue;
                        };

                        match column_type {
                            rusqlite::types::Type::Text => {
                                let text_values: Vec<String> = serde_json::from_value(json_value.to_owned())?;
                                for text_value in text_values {
                                    trace!("inserting row for extended_properties");
                                    e_p_insert_stmts[column_idx - 1].execute([&element.id, &text_value])?;
                                }
                            }
                            rusqlite::types::Type::Null
                            | rusqlite::types::Type::Integer
                            | rusqlite::types::Type::Real
                            | rusqlite::types::Type::Blob => {
                                bail!("found unexpected SQLite type in the extended_properties table")
                            }
                        }
                    }
                }

                // This property is complex, but believed to be primitive and is not known to be
                // polymorph.
                // Occurences of this indicate a bug in our business logic
                v @ Value::Array(_) | v @ Value::Object(_)
                    if observed_primitive_attrs.contains(json_attr_name)
                        && POLYMORPHIC_PROPS.iter().all(|kpf| kpf != json_attr_name) =>
                {
                    observed_unexpected_polymorph_attrs.insert(json_attr_name.to_owned());
                    error!("the JSON attribute {json_attr_name} is believed to be literal, but was found with the following value:\n{v:#?}");
                }

                // This property is complex, but neither a know polymorph field nor a relation nor
                // an extended property know to our schema
                // Occurences of this indicate a bug in our business logic
                v @ Value::Array(_) | v @ Value::Object(_) => {
                    observed_unexpected_complex_attrs.insert(json_attr_name.to_owned());
                    error!("the JSON attribute {json_attr_name} is a complex JSON property but it is neither a relation nor an known extended property:\n{v:#?}");
                }
            }
        }
    }
    r_insert_stmt.finalize()?;

    for stmt in e_p_insert_stmts {
        stmt.finalize()?;
    }

    maybe_time_report!("relations", relations_t0, relations_inserted);

    info!("committing changes to db");
    db_ta.commit()?;

    trace!("observed JSON attributes:\n{observed_json_attrs:#?}");
    trace!("observed non-relation JSON attributes:\n{observed_primitive_attrs:#?}");

    if !unused_db_columns.is_empty() {
        debug!("the following db columns occured not at all in the JSON:\n{unused_db_columns:?}");
    }

    if !observed_unexpected_complex_attrs.is_empty() {
        debug!("the following complex attributes where observed and ignored at least once:\n{observed_unexpected_complex_attrs:#?}");
    }

    let known_db_column_set: HashSet<_> = elements_table_columns
        .iter()
        .map(|(n, _)| n)
        .cloned()
        .collect();

    let always_valid_relational_attributes: HashSet<_> = observed_relational_attrs
        .difference(&observed_unexpected_complex_attrs)
        .cloned()
        .collect();
    let always_valid_attributes: HashSet<_> = always_valid_relational_attributes
        .union(&known_db_column_set)
        .cloned()
        .collect();

    let problematic_attributes: HashSet<_> = observed_json_attrs
        .difference(&always_valid_attributes)
        .cloned()
        .collect();

    if !problematic_attributes.is_empty() {
        warn!("the following attributes were not always understood:\n{problematic_attributes:#?}");
    }

    crate::tweaks::after_bulk_insert(conn, vacuum)?;

    info!("import took {:?}", import_t0.elapsed());
    Ok(())
}

/// Gets a [`Vec`] with column name, column type tuples for a given table
///
/// Returns a Vec, so that the order as returned by the DB is maintained
fn get_table_columns(
    conn: &Connection,
    table_name: &str,
) -> Result<Vec<(String, rusqlite::types::Type)>> {
    // contains the type as `String`
    let mut columns_str = Vec::new();

    conn.pragma(None, "table_info", table_name, |row| {
        let idx: usize = row.get_unwrap(0);
        let name: String = row.get_unwrap(1);
        let r#type: String = row.get_unwrap(2);

        assert_eq!(idx, columns_str.len(), "index of a column must be equal to the length of the columns Vec before insertion of that column");

        columns_str.push((name, r#type));
        Ok(())
    })?;

    // contains the type as `rusqlite::types::Type`
    let mut columns_typed = Vec::new();
    for (column_name, column_type_str) in columns_str {
        let parsed_ty = match column_type_str.as_ref() {
            "INTEGER" => rusqlite::types::Type::Integer,
            "REAL" => rusqlite::types::Type::Real,
            "TEXT" => rusqlite::types::Type::Text,
            "BLOB" => rusqlite::types::Type::Blob,
            "ANY" => rusqlite::types::Type::Text, // TODO revisit this hack
            x => bail!(
                "unexpected SQLite data type {x:?} encountered in schema of {table_name} table"
            ),
        };
        columns_typed.push((column_name, parsed_ty));
    }

    trace!(
        "found the following {table_name} table columns, in total {}:\n{columns_typed:#?}",
        columns_typed.len()
    );

    Ok(columns_typed)
}

/// Insert a relation into the `relations` table
fn insert_relation(
    prepared_statement: &mut Statement,
    relation_kind: &str,
    origin_id: &str,
    target_id: &str,
) -> Result<()> {
    prepared_statement.execute((relation_kind, origin_id, target_id))
        .with_warning(|| format!("failed to insert relation ({relation_kind}, {origin_id}, {target_id})"))
        .note("a cause for this could be an incomplete JSON file, that does not contain all elements of the model")
        .note("are both element ids present in the imported JSON?")?;
    Ok(())
}

/// Checks whether an object is a relation object
///
/// It is assumed, that relation objects are JSON objects with single attribute, which must be named "@id" and of type string.
fn is_relation_object(json_object: &serde_json::Map<String, Value>) -> bool {
    let maybe_id_attribute = json_object.get(ELEMENT_PK_COL);
    matches!(maybe_id_attribute, Some(Value::String(_))) && json_object.len() == 1
}
