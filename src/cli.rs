//! Command Line Interface (CLI) of this software
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub(crate) struct Cli {
    /// Increase verbosity (i.e. debug or trace level logging)
    ///
    /// Repeat to increase the verbosity further
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// SQLite db to operate on
    ///
    /// Creates a new file on demand
    pub db_file: PathBuf,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Import data from JSON file to the db
    ///
    /// This operation is idempotent, i.e. importing the same JSON file multiple times is equivalent
    /// to only importing it once and will not throw an error. This operation is atomic, i.e. if any
    /// error is found while importing, the db remains unchanged.
    ///
    /// Elements already present in the database will remain. However, if one element is both
    /// present in the database but also in the imported JSON file, the data in the JSON file will
    /// prevail. This means, that only those relations etc. found in the element's version from the
    /// JSON file will remain, all previously existing relations originating from that element which
    /// do not exist in the JSON file will be removed.
    ImportJson {
        /// File to import from
        ///
        ///
        file: PathBuf,

        /// Run vacuum after the import
        ///
        /// This makes the import significantly slower and is not required, however especially
        /// after huge imports, vacuum can both reduce the on-disk size of the database and
        /// positively affect the performance of later database operations. Vacuum is similar to
        /// defragemention, hence its biggest impact is when ran in a database which had many rows
        /// removed since the last vacuum/initial database creation.
        #[arg(short, long, action)]
        vacuum: bool,
    },

    /// Initialize a db, creating all missing tables to the db
    ///
    /// This operation is idempotent, i.e. one db can be initialized multiple times over without harm.
    /// However, this operation does not handle schema migrations.
    InitDb,

    /// Parse a JSON schema and generate a suitable SQL schema from it
    ///
    /// This command does not work with arbitrary JSON schemata, but is meant to work with the
    /// schema provided by the OMG SysML-v2 effort. You can download a recent version for example from here:
    ///
    /// https://raw.githubusercontent.com/Systems-Modeling/SysML-v2-API-Services/refs/heads/master/conf/json/schema/api/schemas.json
    JsonSchemaToSqlSchema {
        /// File to read JSON schema from
        file: PathBuf,

        /// SQL File to write derived SQL schema to
        #[arg(short, long, action)]
        dump_sql: Option<PathBuf>,

        // Do not run the generated SQL in DB
        #[arg(short, long, action)]
        no_init: bool,
    },

    /// Fetch from the API to a JSON file
    ///
    /// This operation fetches data from an SysML v2 API server, and stores in a JSON file. The same
    /// JSON file can than later be used for an import from JSON.
    ///
    /// HTTP basic auth is supported. Simply set the environment variables SYSML_USERNAME and
    /// SYSML_PASSWORD accordingly. If setting environment variables is complicated, you can also
    /// put them in a .env file.
    Fetch {
        // URL to the SysML v2 API server, without trailing `/`
        base_url: String,

        /// The project ID
        #[command(subcommand)]
        project: ProjectSelector,

        /// JSON File to write output to
        #[arg(short, long, action)]
        dump_json: Option<PathBuf>,

        /// Page size to request from SysML v2 API server
        #[arg(short, long)]
        page_size: Option<u32>,

        /// Whether to prettify the JSON dump
        #[arg(short = 'y', long, action)]
        pretty: bool,

        /// Do not import the fetched data into the DB
        #[arg(short, long, action)]
        no_import: bool,
    },
}

#[derive(Subcommand)]
pub enum ProjectSelector {
    /// Select an identified project
    ProjectId {
        /// The project ID
        project_id: String,

        #[command(subcommand)]
        commit: CommitSelector,
    },
    /// Select a named project
    ProjectName {
        /// The project name
        project_name: String,

        #[command(subcommand)]
        commit: CommitSelector,
    },
}

#[derive(Subcommand)]
pub enum CommitSelector {
    /// Select an identified commit
    CommitId { commit_id: String },

    /// Select the latest commit from an identified branch
    BranchId { branch_id: String },

    /// Select the latest commit from a named branch
    BranchName { branch_name: String },

    /// Select the latest commit from the default branch
    DefaultBranch,
}
