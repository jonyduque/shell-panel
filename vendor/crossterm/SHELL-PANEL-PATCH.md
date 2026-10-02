Upstream: crossterm 0.28.1 (unchanged except for the patch below).
Patched: src/event/sys/windows/parse.rs, function handle_key_event, the WindowsKeyEvent::Surrogate arm.
Reason: the console sends a key down and a key up for each half of a surrogate pair; pairing halves regardless of direction joined two high (or two low) halves and lost the character, so surrogates from key-up records are now ignored (except the Alt-code release).
Cargo.toml: added [lints.rust] with unexpected_cfgs = "allow" and dead_code = "allow", so the vendored code stops printing build warnings in this workspace (no source change).
