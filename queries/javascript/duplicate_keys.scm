;; Match object literals (pair nodes with keys).
;; Post-filter in Rust: collect keys and report duplicates.
(object
  (pair
    key: [
      (property_identifier)
      (string)
    ] @key
  )
) @object
