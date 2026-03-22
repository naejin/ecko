;; Match throw new Error("...") patterns.
;; Post-filter in Rust: check if the error message is a placeholder
;; (case-insensitive: "not implemented", "todo", etc.).
(throw_statement
  (new_expression
    constructor: (identifier) @constructor
    arguments: (arguments
      (string) @message
    )
  )
) @match
