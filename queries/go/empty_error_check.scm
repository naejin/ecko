;; Match if err != nil with empty block
(if_statement
  condition: (binary_expression
    left: (identifier) @err_var
    right: (nil))
  consequence: (block) @body) @match
