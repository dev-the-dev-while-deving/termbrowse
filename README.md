# termbrowse (from-scratch)

**Branch purpose:** Rebuild from a clean slate using the business strategy and product clarity docs.

**History / old implementation:** see branch `master` and tag `legacy/structure-browser-v0`.

## Strategy docs (start here)

| Doc | Role |
|-----|------|
| [docs/v1/BUSINESS-STRATEGY.md](docs/v1/BUSINESS-STRATEGY.md) | What we sell, to whom, how we win |
| [docs/v1/QUESTIONS-TO-ANSWER.md](docs/v1/QUESTIONS-TO-ANSWER.md) | Questions to lock clarity |
| [docs/v1/DESIGN-SYSTEM-AND-FUNCTIONALITY.md](docs/v1/DESIGN-SYSTEM-AND-FUNCTIONALITY.md) | Product/tech spec |
| [docs/v1/CONSTRUCTIVE-CRITIQUE-AND-OPPORTUNITY.md](docs/v1/CONSTRUCTIVE-CRITIQUE-AND-OPPORTUNITY.md) | Critique + opportunities |

## Branch rules

- `master` — frozen reference of the prior implementation (do not treat as the active product line).
- `from-scratch` — **all new work** lives here.
- Prefer the 80% core loop over experiments that reintroduce a full browser engine.

## Next

Implement v1 against the business strategy, not by copying the old tree wholesale.
