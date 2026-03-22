;; Match try-catch statements.
;; Post-filter in Rust: flag if catch body only re-throws the caught variable.
(try_statement
  handler: (catch_clause
    parameter: (identifier) @param
    body: (statement_block) @body
  )
) @match
