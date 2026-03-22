;; Match call expressions for fixed-wait detection (time.sleep, asyncio.sleep).
;; Post-filter in Rust: only flag time.sleep/asyncio.sleep/wait_for_timeout.
(call
  function: (attribute
    object: (identifier) @obj
    attribute: (identifier) @method)
  arguments: (argument_list) @args) @match
