# Security

Report vulnerabilities privately through GitHub's
[private vulnerability reporting](../../security/advisories/new), not in a
public issue. Please don't include your household data, database files or
passwords in a report.

Vorrat has no accounts by default and listens only on `127.0.0.1`. Anyone who
can reach it can read and change the data, unless you set `VORRAT_PASSWORD`
or put a proxy with its own sign-in in front. See
[docs/SELF-HOSTING.md](docs/SELF-HOSTING.md).
