# .forge

State and configuration for Forge, a software factory (https://github.com/maxwellmattryan/forge).

| Path | Tracked | What |
|---|---|---|
| `config.yml` | yes | Forge configuration (`/forge init` writes it; docs/CONFIG.md in the plugin explains every key) |
| `backlog.md` | yes | Local backlog when `tracker.kind: local` |
| `notes/learnings.md` | yes | Lessons every agent reads; maintained by `/forge retro` |
| `notes/<role>.md` | yes | Optional per-role addenda (e.g. `builder.md`) |
| `reviewers/<name>.md` | yes | Repo-specific reviewer checklists (override or extend the plugin catalog) |
| `evals/` | yes | Eval tasks and run results |
| `trackers/<name>.mjs` or `.ts` | yes | Custom tracker adapters |
| `secrets-allow.txt` | yes | Path globs and `sha256:` fingerprints the secret scanner allows (docs/CONFIG.md "Secret scanning"); read from the main checkout only, and created by hand when a false positive needs it |
| `jobs/`, `locks/`, `runtime/` | no | Runtime state |

Dashboard: `forge dash` (or `/forge dash`).
