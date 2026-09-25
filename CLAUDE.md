# CLAUDE.md

Imitation de Total Annihilation en Rust + Bevy : 3D, solo et en ligne, natif et navigateur.

- Appliquer aussi `~/.claude/CLAUDE.md` (code minimal, hypothèses explicites, vérifier).
- Début de session : lire `docs/README.md`, puis `docs/architecture.md` et `docs/roadmap.md`.
  Fin de session : mettre `docs/` à jour dans le même commit que le code.
- `ta-sim` doit rester déterministe : voir les règles dans `docs/architecture.md`.
- Avant de commiter : `cargo test --workspace` et `cargo clippy --workspace --all-targets`
  (commandes dans `docs/dev.md`). Le web (wasm) est reporté : voir `docs/roadmap.md`.
- Commits : Claude en auteur (`--author="Claude <modèle> <noreply@anthropic.com>"`),
  trailer `Directed-by: Primo <contact@primo-ideas.xyz>`. Primo fait les push : ne jamais
  pousser.
