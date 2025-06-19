use eyre::Result;
use rusqlite::Connection;

/// Apply tweaks to the SQLite database that we expect to be better w/r/t performance
///
///
/// journal_mode = WAL significantly slows down our bulk-inserts
/// locking_mode = EXCLUSIVE has no significant impact on performance, as we use big transactions anyhow
/// temp_store = MEMORY has no significant impact on performance
pub(crate) fn before_bulk_insert(conn: &mut Connection) -> Result<()> {
    let page_size = 4096;
    let cache_size = page_size * 2usize.pow(15); // 4096 * 2^16 => 256 MiB

    info!("applying performance tweaks");
    conn.pragma_update(None, "cache_size", cache_size)?; // non-persistent
    conn.pragma_update(None, "page_size", page_size)?;

    #[allow(clippy::single_element_loop)]
    for (name, value) in [
        // ("journal_mode", "WAL"),
        // ("locking_mode", "EXCLUSIVE"),
        ("synchronous", "OFF"),
        // ("temp_store", "MEMORY"),
    ] {
        conn.pragma_update(None, name, value)?;
    }

    Ok(())
}

pub(crate) fn after_bulk_insert(conn: &mut Connection, vacuum: bool) -> Result<()> {
    info!("resetting performance tweaks");

    #[allow(clippy::single_element_loop)]
    for (name, value) in [
        // ("journal_mode", "DELETE"),
        // ("locking_mode", "NORMAL"),
        ("synchronous", "NORMAL"),
        // ("temp_store", "DEFAULT"),
    ] {
        conn.pragma_update(None, name, value)?;
    }

    for op in if vacuum {
        &["VACUUM", "ANALYZE"][..]
    } else {
        &["ANALYZE"][..]
    } {
        let now = std::time::Instant::now();
        info!("executing {op:?} in db");
        conn.execute_batch(op)?;
        info!("that took {:?}", now.elapsed());
    }

    Ok(())
}
