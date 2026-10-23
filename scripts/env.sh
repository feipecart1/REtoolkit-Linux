# Source from repository root when using the project-local Rust toolchain.
if [ -x "$PWD/.build/cargo/bin/rustup" ]; then
    export CARGO_HOME="$PWD/.build/cargo"
    export RUSTUP_HOME="$PWD/.build/rustup"
    export PATH="$CARGO_HOME/bin:$PATH"
fi
