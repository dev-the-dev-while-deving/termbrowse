# termbrowse — Questions to Answer

**Purpose:** Make the project clear to **you** and to anyone building with you (including AI partners).  
**Bias:** This product is **highly performance- and focus-based**. Every answer should pass: *Does this make the core loop faster, clearer, or smaller?*

**How to use**

1. Answer in order (product → loop → non-goals → performance → focus → shipping).  
2. Prefer **one sentence** or a **single choice**. “TBD” is fine once; revisit before v1 freeze.  
3. When an answer changes, update this file and `DESIGN-SYSTEM-AND-FUNCTIONALITY.md` so they stay aligned.  
4. Default recommendations from prior discussion are in *italics* — override freely.

---

## 0. North star (answer first)

### 0.1 In one sentence, what is termbrowse?

> _________________________________________________________________

*Hint: not “minimal Chrome.” Something like: interactive web session in the terminal for search/read/navigate, structure-first, no browser engine.*

### 0.2 In one sentence, what is termbrowse *not*?

> _________________________________________________________________

### 0.3 If you could only keep **one** superpower, what is it?

Pick one:

- [ ] Fast structured reading of docs/articles  
- [ ] Search → open result → read (session)  
- [ ] Start Page (favorites + reading list) as daily home  
- [ ] Agent-accessible page model (`snapshot` + refs)  
- [ ] Other: _______________  

**Your pick:** _______________

### 0.4 Why “performance and focus” matter for *this* product (your words)

> _________________________________________________________________

---

## 1. Clarity — who and why

### 1.1 Who is the day-1 user? (pick one primary)

- [ ] Only me  
- [ ] Developers who live in terminal/SSH/tmux  
- [ ] People using coding agents (Cursor/Claude/Codex)  
- [ ] Privacy-conscious users avoiding full browsers  
- [ ] Other: _______________  

### 1.2 What job are they hiring termbrowse for?

Complete: *When I _______________, I want to _______________, so I can _______________.*

### 1.3 What do they use today instead?

- [ ] GUI browser  
- [ ] lynx / w3m / links  
- [ ] curl + less / bat  
- [ ] Browser automation (Playwright, etc.)  
- [ ] Scrapers / Firecrawl  
- [ ] Other: _______________  

### 1.4 Why will they switch? (one reason)

> _________________________________________________________________

### 1.5 What would make them uninstall after a week?

> _________________________________________________________________

---

## 2. Category and messaging

### 2.1 What do we call it in public? (pick one)

- [ ] Terminal browser  
- [ ] Minimal browser (for the terminal)  
- [ ] Structure browser  
- [ ] Interactive terminal web session  
- [ ] Agent-friendly web CLI  
- [ ] Other: _______________  

### 2.2 Homepage one-liner (max 15 words)

> _________________________________________________________________

### 2.3 The three bullets on a README hero (must be true)

1. _________________________________________________________________  
2. _________________________________________________________________  
3. _________________________________________________________________  

### 2.4 Main selling point (rank 1 = primary pitch)

| Rank | Candidate |
|------|-----------|
| __ | Stay in the terminal for the useful web (workflow) |
| __ | Structure-first = fast, no Chromium |
| __ | Same model for humans + agents |
| __ | Safari-like start page (favorites + reading list) |
| __ | DuckDuckGo-only opinionated search |
| __ | Grok-density / modern TUI feel |
| __ | Other: _______________ |

### 2.5 What must we **never** promise?

> _________________________________________________________________  
*e.g. “works on every site,” “replaces Chrome,” “bypasses CAPTCHA”*

---

## 3. Core loop (focus)

### 3.1 What is the **primary** daily loop? (pick one)

- [ ] Open app → Start Page → open favorite → read  
- [ ] Open app → search (DDG) → open result → read  
- [ ] Open URL from CLI → read → save  
- [ ] Agent: snapshot → act on refs  
- [ ] Other: _______________  

### 3.2 Walk the primary loop in 5 steps max

1. _______________  
2. _______________  
3. _______________  
4. _______________  
5. _______________  

### 3.3 Which secondary loops are in v1? (check only if yes)

- [ ] Favorites CRUD  
- [ ] Reading list save/open  
- [ ] History back/forward  
- [ ] Agent snapshot  
- [ ] PrivSearch CLI (`termbrowse search`)  
- [ ] Other: _______________  

### 3.4 What is explicitly **out** of the core loop for v1?

> _________________________________________________________________

### 3.5 If we cut half the features tomorrow, what remains?

> _________________________________________________________________

---

## 4. Performance (non-negotiable)

### 4.1 What does “fast” mean for v1? (define measurable)

| Metric | Target | Notes |
|--------|--------|-------|
| Time to Start Page interactive | ___ ms | local only |
| Time to first content (static docs, e.g. example.com / Rust book) | ___ ms | cold network |
| Time to first content (DDG results, when not blocked) | ___ ms | cold network |
| Max memory while browsing one page | ___ MB | rough |
| Binary size (release) | ___ MB | optional |

*Suggested defaults if unsure: Start Page &lt; 50ms; static docs p50 &lt; 500ms; memory &lt; 50MB; no browser process.*

### 4.2 What are we willing to **refuse** to stay fast?

- [ ] No JS engine  
- [ ] No headless Chrome  
- [ ] No pixel/screenshot primary UI  
- [ ] No multi-engine search  
- [ ] No heavy image loading  
- [ ] No cookies/login in v1  
- [ ] Other: _______________  

### 4.3 Performance budget — when do we say no to a feature?

Complete: *If a feature adds more than ___ of complexity/latency without helping the primary loop, we defer it.*

### 4.4 Network policy

| Question | Answer |
|----------|--------|
| Fetch timeout | ___ s *(default idea: 30)* |
| Retries | ___ *(default idea: 0–1)* |
| Concurrent fetches | ___ *(default idea: 1 for browse)* |
| User-Agent policy | browser-like / identify as termbrowse / _______________ |
| Respect robots.txt? | yes / no / later |
| Max response body size | ___ MB |

### 4.5 Parse/layout budget

| Question | Answer |
|----------|--------|
| Max HTML size we fully parse | ___ MB |
| Max blocks to layout | ___ |
| Max links to keep | ___ |
| Giant page strategy | truncate / sample / fail |

### 4.6 What must **never** run on the hot path?

> _________________________________________________________________  
*e.g. launching Chrome, full-page screenshots, multi-provider search fanout*

### 4.7 How do we prove performance in CI or release?

- [ ] Fixture HTML parse benchmarks  
- [ ] Timed `snapshot` on fixed URLs  
- [ ] Manual checklist only  
- [ ] Other: _______________  

---

## 5. Focus (scope discipline)

### 5.1 v1 is successful if **only** these work (list ≤ 5)

1. _______________  
2. _______________  
3. _______________  
4. _______________  
5. _______________  

### 5.2 Hard non-goals for v1 (check all that apply)

- [ ] Full CSS  
- [ ] JavaScript execution  
- [ ] Pixel-perfect pages  
- [ ] Google/Bing as engines  
- [ ] Solving CAPTCHAs  
- [ ] Video/audio  
- [ ] Multi-account / sync  
- [ ] Plugin marketplace  
- [ ] Monetization  
- [ ] Other: _______________  

### 5.3 Feature request filter

A feature ships in v1 only if:

- [ ] It serves the primary loop (3.1), **and**  
- [ ] It does not violate performance refusals (4.2), **and**  
- [ ] It can be explained in one sentence to a new user  

**Your extra filter (optional):** _______________

### 5.4 When tempted to add Chrome/engine back, what must be true?

> _________________________________________________________________  
*Recommended: never for v1; only if primary loop is dead without it and no HTML path exists.*

### 5.5 Site support tiers (define)

| Tier | Meaning | Examples you care about |
|------|---------|-------------------------|
| A — must work well | | |
| B — best effort | | |
| C — designed failure OK | CAPTCHA, empty SPA | |

**List 10 Tier A URLs/sites:**

1. ___  
2. ___  
3. ___  
4. ___  
5. ___  
6. ___  
7. ___  
8. ___  
9. ___  
10. ___  

---

## 6. Rendering & interpretation

### 6.1 Fidelity target for v1 (pick one)

- [ ] **Reader fidelity** — main content clear; chrome dropped  
- [ ] **Structure fidelity** — roles preserved (headings, lists, tables, borders where bordered)  
- [ ] **Visual approximation** — closer to site look (still no full CSS)  
- [ ] Other: _______________  

### 6.2 Role map: anything missing or wrong for v1?

| Role | Keep? | Change? |
|------|-------|---------|
| Heading | Y/N | |
| Paragraph | Y/N | |
| Link + refs | Y/N | |
| List | Y/N | |
| Pre (boxed) | Y/N | |
| Quote | Y/N | |
| Table | Y/N | |
| Frame/card | Y/N | |
| Image → alt only | Y/N | |
| HR | Y/N | |

### 6.3 Is “borders only when bordered” still the rule?

- [ ] Yes  
- [ ] No — new rule: _______________  

### 6.4 Readability (main-content extract) for v1?

- [ ] Must have  
- [ ] Nice if easy  
- [ ] Explicitly later  

### 6.5 SERP (search results) as a dedicated list UI for v1?

- [ ] Must have  
- [ ] Content scroll is enough  
- [ ] Later  

---

## 7. Search

### 7.1 Confirm: DuckDuckGo HTML is the **only** search engine?

- [ ] Yes, locked  
- [ ] No — change to: _______________  

### 7.2 What happens if DDG CAPTCHAs the user?

> _________________________________________________________________  
*e.g. designed error + suggest favorites/docs; no engine fallback*

### 7.3 Is `termbrowse search` (PrivSearch CLI) part of v1 product story?

- [ ] Core  
- [ ] Hidden/power user  
- [ ] Separate project / parked  

### 7.4 Search UX default

- [ ] Centered box on DDG home only  
- [ ] `/` always focuses DDG search from anywhere  
- [ ] Other: _______________  

---

## 8. Start Page & library

### 8.1 Is Start Page the default launch (no URL)?

- [ ] Yes  
- [ ] No  

### 8.2 Favorites: required for v1?

- [ ] Yes  
- [ ] Optional  

### 8.3 Reading list: required for v1?

- [ ] Yes  
- [ ] Optional  

### 8.4 Seed favorites final list?

> _________________________________________________________________

### 8.5 Sync across machines?

- [ ] No (local JSON only)  
- [ ] Later  
- [ ] v1 requirement (how?): _______________  

---

## 9. Agents (optional track — still answer)

### 9.1 Are agents a v1 audience?

- [ ] Primary  
- [ ] Secondary  
- [ ] Ignore until v1.x  

### 9.2 If agents matter, what is the contract?

| Question | Answer |
|----------|--------|
| Command or protocol? | snapshot CLI / MCP / both / _______________ |
| How do agents click? | ref `eN` / URL only / _______________ |
| Stability guarantee? | best effort / golden fixtures / _______________ |

### 9.3 What agent problem are we solving that Playwright doesn’t?

> _________________________________________________________________

---

## 10. UX / design system

### 10.1 Visual language locked as Grok-density (dark + magenta)?

- [ ] Yes  
- [ ] No — describe: _______________  

### 10.2 Must-feel qualities (pick top 3)

- [ ] Fast  
- [ ] Dense  
- [ ] Calm  
- [ ] Keyboard-first  
- [ ] Opinionated  
- [ ] Friendly errors  
- [ ] Beautiful  
- [ ] Invisible/minimal chrome  

### 10.3 Mouse support in v1?

- [ ] No  
- [ ] Nice  
- [ ] Required  

### 10.4 Configurability in v1?

- [ ] Almost none (opinionated)  
- [ ] Theme + key basics  
- [ ] Heavy config  

---

## 11. Security & trust

### 11.1 What origins may we fetch?

- [ ] Any http(s)  
- [ ] Block private/link-local (SSRF safety)  
- [ ] Other: _______________  

### 11.2 Max trust model for HTML

- [ ] Hostile HTML assumed; strip scripts always  
- [ ] Other: _______________  

### 11.3 Do we ever execute page JS in v1?

- [ ] Never  
- [ ] Only under flag (describe): _______________  

### 11.4 Privacy promise in one sentence

> _________________________________________________________________

---

## 12. Shipping & quality

### 12.1 Install story for v1

- [ ] `cargo install` / build from source only  
- [ ] GitHub releases binary  
- [ ] brew / other later  

### 12.2 Definition of done for a “good” page

> _________________________________________________________________

### 12.3 Automated tests required for v1?

- [ ] Unit parse/layout only  
- [ ] Fixtures for Tier A sites  
- [ ] Snapshot golden files  
- [ ] Other: _______________  

### 12.4 60-second demo script (write it)

1. _______________  
2. _______________  
3. _______________  
4. _______________  

---

## 13. Performance + focus contract (sign-off)

Fill this until it feels true:

```text
termbrowse v1 is for _______________ who need to _______________.

We optimize for _______________ (speed metric) and refuse _______________.

The primary loop is: _______________.

We will not build _______________ until that loop is excellent.

If a feature doesn’t help _______________, it waits.
```

**Your filled contract:**

```text




```

---

## 14. Answer summary table (fill last)

| Area | Your answer in ≤10 words |
|------|---------------------------|
| What it is | |
| Who for | |
| Primary loop | |
| Main sell | |
| Performance target | |
| Hard refuses | |
| Search | |
| Home | |
| Agents | |
| v1 cut line | |

---

## 15. For AI / collab partners (how to help you)

When you paste or point at this file:

1. Treat **§0 + §3 + §4 + §5 + §13** as binding.  
2. Prefer changes that improve **primary loop** and **measured speed**.  
3. Reject or park ideas that reintroduce **JS engines, Chrome, multi-engine search, or pixel-primary UI** unless §5.4 is explicitly updated.  
4. If a request conflicts with answered questions, **stop and quote the conflict**.  
5. Unanswered questions default to *italics recommendations* in this doc or `DESIGN-SYSTEM-AND-FUNCTIONALITY.md` — mark as assumption when used.

---

## 16. Related docs

| Doc | Role |
|-----|------|
| `docs/v1/DESIGN-SYSTEM-AND-FUNCTIONALITY.md` | Spec + role map + freeze checklist |
| `docs/PRIVSEARCH.md` | Parked search-product track |
| This file | **Questions you must answer for clarity** |

---

*Fill this in before major refactors. Clarity compounds; unfinished answers become accidental product.*
