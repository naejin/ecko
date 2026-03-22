;; Match all import statements and from-import statements for unused-imports check.
;; Post-filter in Rust: collect imported names, scan for usages.
[
  (import_statement)
  (import_from_statement)
] @match
