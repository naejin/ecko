;; Match catch clauses with a body.
;; Post-filter in Rust: flag if body has no named children (empty) and no comments.
(catch_clause
  body: (statement_block) @body
) @match
