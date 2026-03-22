;; Match import statements with import clauses.
;; Post-filter in Rust: collect imported names, scan for usages.
(import_statement
  (import_clause) @clause
) @match
