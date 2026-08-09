# PrivSearch — Full Product Brief (Parked)

**Status:** PARKED — do not expand until **termbrowse is completely functional**.  
**Created:** 2026-08-06 · **Updated:** 2026-08-10  
**Branch for product work:** `master`  
**Working name:** PrivSearch (brand TBD)  
**Client foundation:** `termbrowse` (Rust TUI structure browser)

> **Come back here later.** This file is the single source of truth for the search-engine product: vision, decisions, architecture, monetization, roadmap, and what already exists in code. Finish termbrowse first; then resume from **§12 Resume checklist**.

Related earlier draft: [`superpowers/specs/2026-08-06-privsearch-design.md`](superpowers/specs/2026-08-06-privsearch-design.md) (same ideas; **this file supersedes it** as the full brief).

---

## 0. Priority order (do not reorder casually)

1. **termbrowse fully functional** — fetch, parse, layout, session, TUI, images, links, forms, reliability  
2. **Then** PrivSearch product (this doc)  
3. **Then** web app / Pro billing / own index horizons  

---

## 1. Origin story (why this product)

### Ambition

- Build a **custom search engine** class of product (think Google / DuckDuckGo), not a site-search widget.  
- **No ads** — ever. Best-in-class at being a search engine: efficient, fast, smooth, accurate, highly useful.  
- Long-term disruptor energy toward big tech; **short-term beachhead only** (see §3).

### Hard truths we accepted

| Fantasy | Useful version |
|---------|----------------|
| Destroy Google/Microsoft/Apple/Meta at everything | Wound **one surface** (search for privacy power users) so hard people switch |
| Beat Google on full open-web day one | Partner index + **our ranking/UX**; own crawl later |
| Free forever + no ads + huge infra | **Paid Pro** (and free limited tier) funds quality |
| AI chat replaces search entirely | Free AI is good for casual Q&A; people **pay** when wrong/unverified answers are expensive |

### Why people will pay (when free AI “does search”)

Free AI optimizes for *sounding helpful*, not *being accountable*.

People pay when the product is better than **ChatGPT + Google** at a job where being wrong is costly:

1. **Cleaner results** — less SEO spam; quality-first ranking  
2. **Grounded answers + sources** — AI only from retrieved docs; always citable  
3. **Speed + craft UX** — keyboard-first, zero clutter  
4. **Privacy** — no profiles, no ads, minimal/no query logging  

**Pitch line:**  
> Free AI guesses. We **find, rank, and prove** — for queries where guessing is expensive.

Casual trivia users will not pay. Pros and privacy power users might.

---

## 2. Product thesis

An **ad-free, privacy-first search product** for power users who want results that feel clean, fast, and trustworthy — better than polluted SEO search and more accountable than free AI chat.

**One-liner:** Privacy-respecting search that ranks for quality, not ads — TUI first, web next.

We do **not** try to destroy Google at everything in v1. We build a daily-driver search habit for privacy power users, starting in the terminal, then web.

---

## 3. Locked product decisions

| Topic | Choice | Notes |
|-------|--------|--------|
| **Audience (beachhead)** | Privacy power users | Hardest monetization; closest to “fight Google”; chosen deliberately |
| **Monetization** | Free tier (limits) + **Paid Pro subscription** | No ads ever |
| **#1 paid reason** | All of: clean results, grounded answers, craft UX, privacy — **ranked in §1** | Privacy = table stakes; quality + answers = wallet |
| **Result source (v1)** | **Partner index** + our ranking / UX / answers | Launch in months, not years |
| **Not v1 source** | Full open-web crawl ourselves | Multi-year arms race; wrong for first ship |
| **Client v1** | **TUI** (`termbrowse`) | Then web on same API |
| **Day-one surface** | TUI now, web soon after | Not extension-first |
| **Approach** | **A** — Ranked privacy search (TUI → API → web) | See §5 |

### Approaches considered (for the record)

| Approach | Summary | Verdict |
|----------|---------|---------|
| **A — Partner + our rank/UX** | Stable path; shippable | **Chosen** |
| **B — Curated clean web + partner fallback** | Stronger brand later; two pipelines | Later (v2) |
| **C — Local-first privacy max** | Strong privacy; hard quality loop & billing | Not v1 |

### Surfaces explicitly deferred

- Browser extension  
- Full Google replacement for every query on Earth  
- Ad network / affiliate spam results  
- Training public models on user queries  
- Full JS browser engine (termbrowse stays structure-based)  
- Building web + extension + API + TUI all on day one  

---

## 4. Competitive map (how we win a slice)

| Player | They optimize for | Our opening |
|--------|-------------------|-------------|
| Google | Coverage + habit + ads | Cleaner, no ads, better sources for power users |
| DuckDuckGo / Brave | Privacy + good-enough general | **Quality ranking + grounded answers + craft TUI** |
| ChatGPT / Perplexity | Synthesis UX | **Trust, citations, no hallucination-as-product** |
| Meta / Apple / Microsoft | Platform lock-in | Not the v1 fight — search beachhead only |

**Success is displacement, not vibes:** people change their default for a class of queries, or pay for Pro.

---

## 5. Architecture (target)

```
User query
  → Query understanding (normalize, language)
  → Partner retrieval (API or HTML partner)
  → Filter + re-rank (quality, spam, diversity)
  → Optional grounded answer (only from top docs)
  → Structured response (JSON / TUI / later web)
```

### Components

| Unit | Responsibility |
|------|----------------|
| `SearchProvider` | Fetch raw candidate hits for a query |
| `rank` | Score, filter, diversify hits |
| `answer` (later) | Summarize only from top hits + citations |
| TUI / CLI | Present hits; open URLs; privacy defaults |
| Billing (later) | Free limits + Pro |
| HTTP API (later) | Same core for web + third parties |

### Data flow

```
CLI/TUI ──query──► search::run
                      │
                      ├─► provider.fetch(query)  → Vec<RawHit>
                      ├─► rank::apply(hits)      → Vec<Hit>
                      └─► SearchResponse { hits, meta, answer? }
```

### Repo intent (on master)

```
termbrowse/                 # crate host
  src/
    search/                 # PrivSearch core (started; park expansion)
      mod.rs
      types.rs
      provider.rs
      ddg.rs
      rank.rs
      answer.rs             # NOT YET
    ... fetch / parse / layout / tui_session ...
  docs/
    PRIVSEARCH.md           # THIS FILE
```

Later (not required to split yet):

- `privsearch-api` — HTTP service  
- Web frontend — same API  

---

## 6. Privacy model (non-negotiable)

| Rule | Behavior |
|------|----------|
| Ads | **Never** |
| User profile / ad graph | **Never** |
| Query logging | Off by default; optional history is **local-only** in TUI |
| Partner queries | Sent to partner without account linkage when possible |
| Personalization | None (global quality ranking, not per-user ad targeting) |
| Telemetry | Opt-in only; no query text in analytics by default |

`SearchResponse.privacy` already encodes: `ads=false`, `profiling=false`, `query_logged=false`.

---

## 7. Free vs Pro (product gates)

| Feature | Free | Pro |
|---------|------|-----|
| Queries / day | Limited (e.g. ~30) | High / unlimited fair use |
| Clean re-ranked results | Yes | Yes |
| Advanced filters (site, date, language) | Basic | Full |
| Grounded answer mode | Limited or off | On |
| API access | No | Yes (later) |
| Priority / lower latency | No | Yes (later) |

Exact numbers TBD at billing time. Architecture must support a `Quota` check before provider call.

---

## 8. Ranking pipeline

Partner scores are a starting point, not truth.

### Heuristic signals (v1 — ML later)

- Title / snippet term match quality  
- Domain quality allow/deny (boost docs, reference, `.edu`/`.gov`; demote farms)  
- URL shape penalties (spam paths, tracking params)  
- Soft boosts for technical/primary sources  
- Diversity: at most ~2 hits per registrable host in top set  
- Drop extreme junk when decent hits exist  

### Success metric

Weekly human eval set of **50–100 hard queries**. Track **“was top result useful?”** — not vanity DAU.

---

## 9. Grounded answers (v1.5 — after core search UX)

1. Retrieve top N hits  
2. Fetch/parse top pages (**reuse termbrowse** fetch/parse — reason termbrowse must be solid first)  
3. LLM or extractive summary **restricted to that text**  
4. Every claim maps to a citation hit id  
5. If evidence is weak → say so; **never invent sources**  

This is the answer to “why not just ChatGPT?” — **retrieval + proof**, not vibes.

---

## 10. Partner strategy

| Phase | Provider | Notes |
|-------|----------|--------|
| **v0 (started)** | DuckDuckGo HTML | Privacy-aligned; fragile; CAPTCHA risk |
| **v1** | Brave Search API or Bing (env key) | Stable JSON; cost per query; needed for Pro quality |
| **v2** | Hybrid: curated crawl + partner | Clean-web brand |
| **Later** | Larger own index | Independence |

### Env (implemented / planned)

| Variable | Values | Default |
|----------|--------|---------|
| `PRIVSEARCH_PROVIDER` | `ddg`, `mock` (later: `brave`, …) | `ddg` |
| `PRIVSEARCH_BRAVE_API_KEY` | secret | — |
| `PRIVSEARCH_NO_LOG` | `1` | default on |

**DDG HTML notes:** Prefer **POST** to `https://html.duckduckgo.com/html/` (GET often returns lite shell). Detect `anomaly-modal` / challenge pages and fail clearly. Rate limits and bot challenges are real — **API partner is the Pro path**.

---

## 11. What already exists in code (as of 2026-08-10)

Landed on **`master`** during early PrivSearch spike (may be uncommitted or partially committed — check `git status`):

### Implemented

- `src/search/types.rs` — `Query`, `RawHit`, `Hit`, `SearchResponse`, `PrivacyMeta`  
- `src/search/provider.rs` — `SearchProvider` trait, `from_env()`, `MockProvider`  
- `src/search/ddg.rs` — DDG HTML partner, parse, redirect unwrap, block detection  
- `src/search/rank.rs` — quality boosts, spam demotion, host diversity  
- `src/search/mod.rs` — `run`, `run_with`, `format_text`  
- CLI: `termbrowse search <query> [-n limit] [--json]`  
- Tests: DDG parse, rank spam demotion, uddg unwrap  
- README snippets for PrivSearch  

### Try (when resuming)

```bash
cargo run --release -- search "rust async"
cargo run --release -- search -n 5 --json "privacy search engine"
PRIVSEARCH_PROVIDER=mock cargo run --release -- search "demo"
```

### Explicitly NOT done yet

- [ ] Native results TUI (home = PrivSearch, not Google HTML)  
- [ ] Open hit into termbrowse session from results list  
- [ ] Brave/Bing official API provider  
- [ ] Free quota tracking  
- [ ] Local query history  
- [ ] Grounded answers  
- [ ] Accounts / Pro billing  
- [ ] HTTP API  
- [ ] Web UI  
- [ ] Own crawl / curated index  
- [ ] Browser extension  

---

## 12. Roadmap (resume after termbrowse)

### Before PrivSearch: termbrowse complete

Definition of “completely functional” is owned by termbrowse work — roughly:

- Reliable HTTPS fetch  
- Solid HTML → structure parse  
- Layout + Grok-density TUI  
- Links, forms, history, scroll  
- Images / media as designed  
- Search-form UX on third-party pages still works as browser feature  
- No critical crashes on common sites  
- Documented keys and known limits  

**Only when that bar is met, open this file and continue.**

### M0 — Search core ✅ (started)

- [x] Types, provider trait, DDG + mock, rank, CLI, tests  
- [ ] Stabilize when resuming (CAPTCHA handling, API provider)

### M1 — Native results TUI

- [ ] No-URL launch → PrivSearch home (centered prompt)  
- [ ] Results list: `j/k`, Enter open, status (latency, provider, privacy)  
- [ ] Open hit in existing session browser  
- [ ] Keep structure-browse of arbitrary URLs working  

### M2 — Quality + privacy productization

- [ ] Domain quality lists (expand)  
- [ ] Local query history (optional, file-based)  
- [ ] Free quota tracking (local)  
- [ ] Brave or Bing provider behind API key  

### M3 — Grounded answers

- [ ] Fetch top docs via termbrowse stack  
- [ ] Extractive or LLM answer with citations only  

### M4 — Pro + web

- [ ] Accounts + subscription  
- [ ] HTTP API  
- [ ] Minimal web UI  

### M5+ — Moat

- [ ] Curated clean-web crawl (Approach B)  
- [ ] Larger own index  
- [ ] Eval harness + weekly ranking reviews  

---

## 13. TUI UX (when we build M1)

1. `termbrowse` with no URL → **PrivSearch home** (centered prompt, Grok-density)  
2. Submit query → **native results view** (not raw Google/DDG HTML as the product surface)  
3. Keys (proposed): `j/k` move, `Enter` open, `a` toggle answer (later), `/` new query, `q` quit  
4. Status line: latency, partner, privacy mode, remaining free quota  
5. Opening a hit: prefer **in-app** structure browser (session stack); optional system browser later  

termbrowse’s existing “type on search homes / Google CAPTCHA / DDG HTML” path is a **browser feature**, not the final product UX.

---

## 14. Risks

| Risk | Mitigation |
|------|------------|
| Partner ToS / HTML breakage / CAPTCHA | Abstract `SearchProvider`; Pro uses official API |
| Free users never convert | Make Pro answers + filters clearly better |
| “Just use ChatGPT” | Win on sources, freshness, no-ads, terminal workflow |
| Scope creep to full crawl | Keep partner-backed through M2–M3 |
| termbrowse unfinished → search on sand | **This park:** finish browser first |
| “Destroy big tech” dilutes focus | One beachhead; measure displacement |

---

## 15. Success criteria (first 90 days *after* un-parking)

1. Privacy power user can use termbrowse/PrivSearch as daily search for real queries without Google.  
2. Blind comparison on a 50-query set: we beat raw DDG HTML on usefulness.  
3. At least one person outside the author would pay for Pro if billing existed (intent signal).  

---

## 16. Non-goals (keep visible)

- Beating Google on every head query in year one  
- Ads or engagement-max ranking  
- Chat product that invents facts  
- Platform for everyone before one audience loves us  
- Parallel build of every client surface  

---

## 17. Resume checklist (print this when you return)

```
[ ] termbrowse is completely functional (your bar, not this doc’s)
[ ] Read this entire PRIVSEARCH.md once
[ ] git checkout master; review src/search/*
[ ] cargo test; mock search; live ddg search
[ ] Decide: finish M0 stabilization vs start M1 TUI
[ ] If Pro path soon: pick Brave vs Bing API and get a key
[ ] Re-lock beachhead if market/learning changed (update §3)
[ ] Implement next milestone only — no M4 while M1 open
```

---

## 18. Open questions (resolve on resume)

1. Final product name / domain?  
2. Brave Search API vs Bing as primary paid partner?  
3. Free tier daily limit number?  
4. Local-only history path on disk (`~/.config/privsearch/` vs project)?  
5. Should PrivSearch stay inside the `termbrowse` binary forever, or split crates?  
6. Grounded answers: which LLM provider / local model policy for privacy?  

---

## 19. Conversation anchors (strategy one-liners)

- Search engine = **crawl → parse → index → retrieve → rank → serve** (+ spam + UX).  
- Custom “Google-class” general search is a multi-year factory, not a weekend.  
- **Wedge, then expand.** Conquerors expand after; disruptors pick a throat.  
- Ad-free requires a **business model** (we chose Pro subscription).  
- AI didn’t kill search; it raised the bar to **correct, current, sourced, actionable**.  
- termbrowse is the **reader + session layer**; PrivSearch is the **discovery + ranking layer**. Together: find → read in one privacy-respecting terminal workflow.

---

## 20. Document control

| Field | Value |
|-------|--------|
| Canonical path | `docs/PRIVSEARCH.md` |
| Owner | Project author |
| Status | **PARKED** pending termbrowse completeness |
| Next action | None on PrivSearch until termbrowse done; then §12 M0/M1 |

When un-parking, update **Status** at the top and the date, then execute §17.
