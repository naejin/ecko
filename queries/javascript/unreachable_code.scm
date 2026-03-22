;; Match control-flow terminating statements inside block bodies.
;; Post-filter in Rust: check if next sibling statement exists.
[
  (return_statement)
  (throw_statement)
  (break_statement)
  (continue_statement)
] @match
