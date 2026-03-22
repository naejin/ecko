;; Match all import specs in Go (name captures alias or blank identifier if present)
(import_spec name: (_)? @name path: (interpreted_string_literal) @path) @match
