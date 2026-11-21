# nie-sql

Shared bounded SQL boundary for NIE host tools. It supports read-only SQLite file access, a conservative SQL read policy, parameterized PostgreSQL reads over TLS, and a SQLite/PostgreSQL migration registry. No workspace crate depends on it yet. PostgreSQL connections set the server session read-only; development-only plaintext PostgreSQL access is explicit.

The crate keeps its upstream license (`LICENSE.upstream`). It is native host code and is not used by the portable format parsers.
