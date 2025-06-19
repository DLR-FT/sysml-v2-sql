use color_eyre::Section;
use eyre::Result;
use rusqlite::Connection;

/// Initializes a db with the schema and views from `schema.sql`
pub(crate) fn init_db(conn: &mut Connection) -> Result<()> {
    info!("creating tables");
    conn.execute_batch(include_str!("../assets/schema.sql"))
        .note("are there pre-existing tables/views in the db?")?;
    info!("done");

    Ok(())
}
