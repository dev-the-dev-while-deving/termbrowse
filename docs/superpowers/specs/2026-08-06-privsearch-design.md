# PrivSearch — Design Spec

**Date:** 2026-08-06  
**Branch:** `master`  
**Status:** SUPERSEDED / PARKED — full brief is [`docs/PRIVSEARCH.md`](../../PRIVSEARCH.md). Finish termbrowse first.  
**Working name:** PrivSearch (brand TBD)  
**Client foundation:** `termbrowse` (existing Rust TUI structure browser)

---

## 1. Product thesis

An **ad-free, privacy-first search product** for power users who want results that feel clean, fast, and trustworthy — better than polluted SEO search and more accountable than free AI chat.

**One-liner:** Free AI guesses. We find, rank, and prove — with sources, no ads, no profiles.

We do **not** try to destroy Google at everything in v1. We build a daily-driver search habit for privacy power users, starting in the terminal, then web.

---

## 2. Decisions (locked)

| Topic | Choice |
|-------|--------|
| Audience | Privacy power users |
| Monetization | Free tier (limits) + **Paid Pro subscription** |
| Result source (v1) | **Partner index** + our ranking / UX / grounded answers |
| Client v1 | **TUI** (`termbrowse`) first |
| Client v1.1 | Web app on the same API |
| Not v1 | Full open-web crawl, ads, chat-only product, browser extension |

### Why people pay (priority order)

1. **Cleaner results** — less SEO spam; quality-first ranking  
2. **Grounded answers + sources** — optional AI summary only from retrieved docs  
3. **Speed + craft UX** — keyboard-first TUI, zero clutter  
4. **Privacy** — no ad IDs, no personalization graph, minimal logs  

Privacy is table stakes; ranking quality + grounded answers are the paid hook.

---

## 3. Approach

**Approach A — Ranked privacy search (TUI → API → web)**

```
User query
  → Query understanding (normalize, language)
  → Partner retrieval (API or HTML partner)
  → Filter + re-rank (quality, spam, diversity)
  → Optional grounded answer (only from top docs)
  → Structured response (JSON / TUI / later web)
```

Own full-web crawl is a later horizon, not a launch blocker.

---

## 4. Architecture

### 4.1 Repo layout (master)

```
termbrowse/                 # existing crate, product host
  src/
    search/                 # NEW — product search core
      mod.rs
      types.rs              # Query, Hit, SearchResponse
      provider.rs           # SearchProvider trait
      ddg.rs                # DuckDuckGo HTML partner (default v0)
      rank.rs               # re-rank / spam heuristics
      answer.rs             # grounded answer (later)
    ...existing fetch/parse/tui...
  docs/superpowers/specs/   # this design
```

Later (not v1 monorepo split required):

- `privsearch-api` — HTTP service wrapping the same search core  
- Web frontend — same API  

### 4.2 Components

| Unit | Responsibility | Depends on |
|------|----------------|------------|
| `SearchProvider` | Fetch raw candidate hits for a query | Network |
| `rank` | Score, filter, diversify hits | Provider output |
| `answer` (later) | Summarize only from top hits + citations | rank + LLM |
| `tui_session` / search mode | Present hits; open URLs; privacy defaults | rank |
| Billing (later) | Free limits + Pro | Auth + Stripe-class |

### 4.3 Data flow

```
CLI/TUI ──query──► search::run
                      │
                      ├─► provider.fetch(query)  → Vec<RawHit>
                      ├─► rank::apply(hits)      → Vec<Hit>
                      └─► SearchResponse { hits, meta, answer? }
```

### 4.4 Privacy model

| Rule | v1 behavior |
|------|-------------|
| Ads | Never |
| User profile / ad graph | Never |
| Query logging | Off by default; Pro “history” is **local-only** in TUI |
| Partner queries | Sent to partner without account linkage when possible |
| Personalization | None (quality ranking is global, not per-user) |
| Telemetry | Opt-in only; no query text in analytics by default |

### 4.5 Free vs Pro (product gates)

| Feature | Free | Pro |
|---------|------|-----|
| Queries / day | Limited (e.g. 30) | High / unlimited fair use |
| Clean re-ranked results | Yes | Yes |
| Advanced filters (site, date, language) | Basic | Full |
| Grounded answer mode | Limited or off | On |
| API access | No | Yes (later) |
| Priority / lower latency path | No | Yes (later) |

Exact numbers TBD at billing implementation; architecture must support a `Quota` check before provider call.

---

## 5. Ranking pipeline (v1)

Partner scores are a starting point, not truth.

**Signals (heuristic first, ML later):**

- Title / snippet term match quality  
- Domain quality allow/deny lists (block known content farms)  
- URL shape penalties (spammy paths, excessive tracking params)  
- Soft boosts for docs, reference, primary sources when query looks technical  
- Diversity: avoid 5 near-duplicate domains in top 10  

**Success metric:** weekly human eval set of 50–100 hard queries; track “was top result useful?” not vanity DAU.

---

## 6. Grounded answers (v1.5, not day-one blocker)

- Retrieve top N hits  
- Fetch/parse top pages (reuse termbrowse fetch/parse)  
- LLM (or extractive summary) **restricted to that text**  
- Every sentence or claim maps to a citation hit id  
- If evidence is weak → say so; never invent sources  

---

## 7. TUI UX (v1)

Extend termbrowse, do not throw it away.

1. `termbrowse` with no URL → **PrivSearch home** (centered prompt, Grok-density)  
2. Submit query → native results view (not raw Google HTML scrape as the product surface)  
3. Keys: `j/k` move, `Enter` open, `a` toggle answer (when available), `q` quit  
4. Status line: latency, partner, privacy mode, remaining free quota  

Opening a hit can use existing session navigation (structure browser) or system open — prefer in-app load via current session stack.

---

## 8. Partner strategy

| Phase | Provider | Notes |
|-------|----------|-------|
| v0 (now) | DuckDuckGo HTML | Aligns with privacy story; already partially used; fragile HTML |
| v1 | Brave Search API or Bing (via env key) | Stable JSON; cost per query; required for Pro quality |
| v2 | Hybrid: curated crawl + partner | Clean-web corpus for quality brand |
| Later | Larger own index | Independence |

Config via env:

- `PRIVSEARCH_PROVIDER=ddg|brave|mock`  
- `PRIVSEARCH_BRAVE_API_KEY=...`  
- `PRIVSEARCH_NO_LOG=1` (default)

---

## 9. Non-goals (explicit)

- Beating Google on every head query in year one  
- Ad network or affiliate spam results  
- Training public models on user queries  
- Full JS browser engine  
- Building web + extension + API + TUI all on day one  

---

## 10. Milestones

### M0 — Search core on master (current)
- [x] Design doc  
- [x] `search` module: types, provider trait, DDG provider, rank  
- [x] CLI: `termbrowse search "query"`  
- [x] Unit tests for rank + parse  
- [ ] Native results TUI (M1)  

### M1 — Native results TUI
- [ ] Homescreen is PrivSearch, not Google  
- [ ] Results list view with keyboard nav  
- [ ] Open hit in existing session browser  

### M2 — Quality + privacy productization
- [ ] Domain quality lists  
- [ ] Local query history (optional, file-based)  
- [ ] Free quota tracking (local)  
- [ ] Brave/Bing provider behind API key  

### M3 — Grounded answers
- [ ] Fetch top docs → extractive or LLM answer with citations  

### M4 — Pro + web
- [ ] Accounts + subscription  
- [ ] HTTP API  
- [ ] Minimal web UI  

---

## 11. Risks

| Risk | Mitigation |
|------|------------|
| Partner ToS / HTML breakage | Abstract `SearchProvider`; prefer official API for Pro |
| Free users never convert | Make Pro answer mode + filters clearly better |
| “Just use ChatGPT” | Win on sources, freshness, no-ads, terminal workflow |
| Scope creep to full crawl | Keep M0–M2 partner-backed |

---

## 12. Success criteria (first 90 days)

1. A privacy power user can set `termbrowse` as daily search and complete real queries without Google.  
2. Blind comparison: on a 50-query set in our niche of “clean web,” we beat raw DDG HTML on usefulness.  
3. At least one person outside the author would pay for Pro if billing existed (intent signal).  

---

## 13. Implementation notes for master

- All product work lands on **`master`** (no long-lived feature branch required for this effort).  
- Prefer small commits: search core → CLI → TUI → providers.  
- Keep Chrome-free; stay on custom fetch/parse stack.  
- Do not regress existing `open` / `snapshot` / structure browsing.  
