# Clipsy

Clipboard history manager for Linux (Rust + GTK4/libadwaita). The full plan and all decisions are in `docs/PLAN.md`. Read it before starting work.

- The user is a beginner. Explain what you built and give exact commands to run and test.
- Dev machine: Ubuntu 24.04, GNOME 46, X11 session (GTK 4.14, libadwaita 1.5).
- Work phase by phase (PLAN.md §11). Don't start the next phase until the user confirms the acceptance checks.
- Before claiming a phase is done, run `cargo clippy -- -D warnings` and `cargo test`.
