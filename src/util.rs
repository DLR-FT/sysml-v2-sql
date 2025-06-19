use eyre::Result;
use serde::de::DeserializeOwned;
use std::{fs::File, io::BufReader, path::Path};

mod stream_json;

/// Reads a JSON file, parsing it into a type `T`
///
/// Measures time taken to parse
pub(crate) fn read_json_file<T: DeserializeOwned>(file: &Path) -> Result<T> {
    let now = std::time::Instant::now();

    info!("importing from JSON document {file:?}");
    trace!("opening JSON document");
    let f = File::open(file)?;
    let reader = BufReader::new(f);

    trace!("parsing JSON document");
    let parsed = serde_json::from_reader(reader)?;

    debug!("file parsing took {:?}", now.elapsed());
    Ok(parsed)
}

/// Escapes string in the way prescribed by the SQL standard, but generic over the quotation symbol
/// in use
pub(crate) fn escape_sql<const DELIM: char, S: AsRef<str>>(str_to_escape: S) -> String {
    let escaped = str_to_escape
        .as_ref()
        .replace(DELIM, String::from_iter([DELIM, DELIM]).as_str());
    format!("{DELIM}{escaped}{DELIM}")
}

/// Escape a string to be used as SQLite string literal
///
/// The SQL Standard requires single quotes around string literals (see
/// <https://sqlite.org/lang_keywords.html>).
///
/// Single quotes within a string literal can be encoded by prefixing them with a second single
/// quote (see <https://www.sqlite.org/lang_expr.html>):
///
/// > A single quote within the string can be encoded by putting two single quotes in a row - as
/// > in Pascal.
#[allow(unused)]
pub(crate) fn escape_sql_str_lit<S: AsRef<str>>(str_to_escape: S) -> String {
    escape_sql::<'\'', S>(str_to_escape)
}

/// Escape a string to be used as SQLite identifier
///
/// The SQL Standard requires double quotes around string literals (see
/// <https://sqlite.org/lang_keywords.html>).
pub(crate) fn escape_sql_ident<S: AsRef<str>>(str_to_escape: S) -> String {
    escape_sql::<'"', S>(str_to_escape)
}

/// This type is a wrapper arround the streaming JSON iterator provided in [`stream_json`]
///
/// Open a JSON file, assuming it to be an array of elements of type `T`. Streams the file to
/// It assumes files as source for the JSON, provides buffered reading, and implements [`Clone`].
pub(crate) struct CloneableJsonArrayStreamIterator<T> {
    /// File to stream JSON from
    file: std::path::PathBuf,

    /// Internal iterator
    iter: Box<dyn Iterator<Item = Result<T, std::io::Error>>>,
}

impl<T: 'static + DeserializeOwned> CloneableJsonArrayStreamIterator<T> {
    /// Create a new streaming JSON iterator from a [`Path`]
    pub fn new<P: AsRef<Path>>(file_path: P) -> Result<Self, std::io::Error> {
        let path_buf = file_path.as_ref().into();
        info!("streaming from JSON document {path_buf:?}");

        trace!("opening JSON document");
        let f = File::open(&path_buf)?;
        let reader = BufReader::new(f);

        trace!("initializing JSON stream");
        Ok(Self {
            file: path_buf,
            iter: Box::new(stream_json::iter_json_array(reader)),
        })
    }
}

impl<T: 'static + DeserializeOwned> Clone for CloneableJsonArrayStreamIterator<T> {
    fn clone(&self) -> Self {
        Self::new(&self.file).unwrap()
    }
}

impl<T> Iterator for CloneableJsonArrayStreamIterator<T> {
    type Item = Result<T, std::io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// If enough time passed, create a status report
#[macro_export]
macro_rules! maybe_time_report {
    ($row_kind:expr, $timer:expr, $duration_since_last_update:expr, $rows_inserted:expr) => {
        if $timer.elapsed() > $duration_since_last_update && $rows_inserted != 0 {
            let elapsed_since_start = $timer.elapsed();
            info!(
                "inserted {rows_inserted} {row_kind}s over {total_time_passed:?}, averaging {time_per_insertion:?}/{row_kind} ↔ {insertions_per_second:.0} {row_kind}s/s",
                row_kind = $row_kind,
                rows_inserted = $rows_inserted,
                total_time_passed = elapsed_since_start,
                time_per_insertion = elapsed_since_start.div_f64($rows_inserted as f64),
                insertions_per_second = $rows_inserted as f64 / elapsed_since_start.as_secs_f64()
            );
            $duration_since_last_update += $crate::config::TIME_BETWEEN_STATUS_REPORTS;
        }
    };

    ($row_kind:expr, $timer:expr, $rows_inserted:expr) => {
        if $rows_inserted != 0 {
            let elapsed_since_start = $timer.elapsed();
            info!(
                "inserted {rows_inserted} {row_kind}s over {total_time_passed:?}, averaging {time_per_insertion:?}/{row_kind} ↔ {insertions_per_second:.0} {row_kind}s/s",
                row_kind = $row_kind,
                rows_inserted = $rows_inserted,
                total_time_passed = elapsed_since_start,
                time_per_insertion = elapsed_since_start.div_f64($rows_inserted as f64),
                insertions_per_second = $rows_inserted as f64 / elapsed_since_start.as_secs_f64()
            );
        }
    };
}
