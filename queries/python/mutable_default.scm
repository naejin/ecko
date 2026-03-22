;; Match default parameters with mutable literal values: list [], dict {}, set {x}.
;; Covers both default_parameter and typed_default_parameter.
;; Note: set() / dict() / list() calls are handled via post-filter in Rust.
(default_parameter value: (list) @match)
(default_parameter value: (dictionary) @match)
(default_parameter value: (set) @match)
(typed_default_parameter value: (list) @match)
(typed_default_parameter value: (dictionary) @match)
(typed_default_parameter value: (set) @match)
(default_parameter value: (call) @call_match)
(typed_default_parameter value: (call) @call_match)
