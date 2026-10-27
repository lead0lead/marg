# CLAUDE.md

marg: keyboard-driven PDF reader and annotator (Rust + MuPDF + egui).
**`concept.md` is the source of truth.** Read it first, especially §0 (current
state), §10 (How We Work), §11 (build phases) and §14 (session log).

## Rules (summary of concept.md §10)

- Work on the next unfinished phase only, on a feature branch, one commit per
  step. Stop at the end of each phase and hand the build to the user.
- Do not pass the decision milestone (end of Phase 2) without the user's go-ahead.
- Before every commit: `cargo build`, `cargo test`,
  `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
- Never change the commit author identity; report anything unexpected.
- No history rewriting. Merge to `main` only after the user tried the phase.
- Ask before adding any library not listed in concept.md §9.
- Workspace: `marg-core` (pure logic, no MuPDF/egui, unit-tested),
  `marg-pdf` (MuPDF adapter + document thread), `marg` (eframe app).
- The UI thread never calls MuPDF. Core uses PDF points; pixels only in the app.
- All tuning values live in `marg-core` config defaults.
- Never test saving on the user's real PDFs; use fixtures or copies.
- Performance is judged on release builds only. Feel/taste is the user's call.
- Three failed fix attempts → stop and report. Never weaken checks to pass.
- Keep concept.md current: §0 after each step; session log, as-implemented
  sections and open questions at the end of each phase/session.
