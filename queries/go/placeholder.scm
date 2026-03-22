;; Match panic("not implemented") and panic("TODO") calls
(call_expression
  function: (identifier) @fn_name
  arguments: (argument_list
    (interpreted_string_literal) @arg)) @match
