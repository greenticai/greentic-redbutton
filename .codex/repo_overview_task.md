Repo Overview Maintenance routine:

1. Scan the project structure and identify the main modules, packages, services, and config/build files.
2. Infer current implemented behavior from entrypoints, public APIs, tests, examples, and docs.
3. Search for unfinished work markers such as `TODO`, `FIXME`, `XXX`, `HACK`, `todo!`, and `unimplemented!`.
4. Run the repo’s standard non-destructive build/test/package checks when they are clearly indicated.
5. Refresh `.codex/repo_overview.md` completely so it reflects the repo as it exists now, including:
   - high-level purpose
   - main components and functionality
   - work in progress, TODOs, and stubs
   - broken, failing, or conflicting areas
   - notes for future work
