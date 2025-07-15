# About

This tool allows interacting with SysML v2 models via SQL.
It can:

- Fetch a model from a SysML v2 API server to a JSON file
- Import a model in that JSON format into a SQLite database
- Generate a SQLite compatible-SQL Schema based on the JSON-Schema for the SysML v2 API

# Usage

- Initialize a new database
  - `sysml-v2-sql sysml-v2.db init-db`
- Import a SysML-V2 API JSON dump
  - `sysml-v2-sql cargo run --release -- sysml-v2.db import-json sysml-v2-api-dump.json`

# Development

- Re-Generating the assets/schema.sql:

  - Download the most up-to-date JSON schema, for example from
    https://raw.githubusercontent.com/Systems-Modeling/SysML-v2-API-Services/refs/heads/master/conf/json/schema/api/schemas.json
  - ```bash
    # preferred source of trut, but possibly outdated
    curl \
      https://www.omg.org/spec/KerML/20240201/KerML.json > assets/KerML-schema.json
    cargo run --release -- /dev/null json-schema-to-sql-schema --no-init --dump-sql assets/schema.sql assets/KerML-schema.json

    # OR

    # more up-to-date, but possibly not (yet) standardized
    curl \
      https://raw.githubusercontent.com/Systems-Modeling/SysML-v2-API-Services/refs/heads/master/conf/json/schema/api/schemas.json \
      > assets/SysML-schema.json
    cargo run --release -- /dev/null json-schema-to-sql-schema --no-init --dump-sql assets/schema.sql assets/SysML-schema.json
    ```

  - Releasing new version:
    1. Check that the new version can be release: `cargo release minor`
    2. If you are happy, do the release: `cargo release minor --execute --no-publish`

# License and Copyright

For this library the copyright belongs to the German Aerospace Center / Deutsches Zentrum für Luft- und Raumfahrt e.V. (DLR):

Copyright (c) 2025 Deutsches Zentrum für Luft- und Raumfahrt e.V. (DLR)

Licensed under MIT + Apache 2.0. That means, as a downstream consumer of this software you may
choose to either use it under MIT or under Apache 2.0 license, at your discretion. All contributions
from upstream must be licensed under both MIT and Apache 2.0; if you contribute code to this project
you agree to license your code under both the MIT and the Apache 2.0 license.
