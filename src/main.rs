//! This tool allows to interact with SysML v2 models via SQL
//!
//! It does so by importing model data expressed in the canonical JSON schema described in the
//! [`schemas.json`](https://raw.githubusercontent.com/Systems-Modeling/SysML-v2-API-Services/refs/heads/master/conf/json/schema/api/schemas.json)
//! file into a SQLite database.
//!
//! There is a strong coupling between the model expressivity described in said `schemas.json` and
//! the database schema. To avoid erroneous manual labor, this tool can also generate a SQLite
//! compatible SQL Schema from aforementioned JSON schema.

#![deny(missing_docs)]
#![deny(unsafe_code)]
#![deny(clippy::all)]

use std::io::Write;

use clap::Parser;
use eyre::Result;

use crate::cli::Commands;

#[macro_use]
extern crate log;

mod cli;
mod config;
mod fetch;
mod import;
mod init_db;
mod json_schema_to_sql;
mod tweaks;
mod util;

fn main() -> Result<()> {
    dotenv::dotenv().ok();

    // parse the CLI arguments
    let args = cli::Cli::parse();

    // intialize logger
    let rust_log_var = "RUST_LOG";
    if std::env::var(rust_log_var).is_err() && args.verbose != 0 {
        let level = match args.verbose {
            1 => "debug",
            _ => "trace",
        };
        std::env::set_var(rust_log_var, level);
    }
    colog::init();
    color_eyre::install()?;

    trace!("parsed args");

    info!("opening database {:?}", args.db_file);
    let mut conn = rusqlite::Connection::open(args.db_file)?;

    match args.command {
        Commands::InitDb => init_db::init_db(&mut conn)?,
        Commands::ImportJson { file, vacuum } => {
            let elements_stream = crate::util::CloneableJsonArrayStreamIterator::new(&file)?;
            import::import_from_iter(elements_stream, &mut conn, vacuum)?;
        }
        Commands::JsonSchemaToSqlSchema {
            file,
            dump_sql,
            no_init,
        } => {
            let schema = crate::util::read_json_file(&file)?;

            let maybe_conn = (!no_init).then_some(&mut conn);

            let schema = json_schema_to_sql::consume_json_schema(&schema, maybe_conn)?;

            if let Some(path) = dump_sql {
                info!("writing the fetched data to {path:?}");
                let mut f = std::fs::File::create(path)?;
                f.write_all(schema.as_bytes())?;
            }
        }
        Commands::Fetch {
            base_url,
            dump_json,
            page_size,
            pretty,
            no_import,
            project,
        } => {
            if dump_json.is_none() && pretty {
                warn!("the -p/--pretty flag has no effect if FILE is not set");
            }

            let base_url = reqwest::Url::parse(&base_url)?;
            let sysml_browser = fetch::SysmlV2ApiBrowser::new(base_url)?;

            // start an async runtime
            let rt = tokio::runtime::Runtime::new().unwrap();

            // Spawn a future onto the runtime
            let result: Result<()> = rt.block_on(async {
                let (project_id, commit_id) =
                    fetch::interprete_cli(&sysml_browser, &project).await?;

                let url_path = fetch::build_url_path(&project_id, &commit_id, page_size);
                let maybe_conn = (!no_import).then_some(&mut conn);
                fetch::fetch_from_url_to_file(
                    sysml_browser,
                    &url_path,
                    &dump_json,
                    maybe_conn,
                    pretty,
                )
                .await?;

                Ok(())
            });
            result?;
        }
    }

    Ok(())
}
