export RUSTUP_HOME=/opt/rust/rustup
export CARGO_HOME=/opt/rust/cargo
case ":$PATH:" in *:/opt/rust/cargo/bin:*) ;; *) export PATH="/opt/rust/cargo/bin:$PATH" ;; esac
