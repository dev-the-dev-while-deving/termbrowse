# termbrowse — Business Strategy

**Status:** Operating strategy (follow this; update deliberately when strategy changes)  
**Owner:** Project lead  
**Horizon:** v1 ship → first users → optional India/privacy expansion  
**Bias:** Performance, focus, privacy. No “we’ll see” on the core.

**Related docs**

| Doc | Role |
|-----|------|
| `QUESTIONS-TO-ANSWER.md` | Personal clarity questionnaire |
| `DESIGN-SYSTEM-AND-FUNCTIONALITY.md` | Product/tech spec |
| `CONSTRUCTIVE-CRITIQUE-AND-OPPORTUNITY.md` | Critique + opportunity raw material |
| **This file** | **What we sell, to whom, how we win, what we refuse** |

---

## 1. Mission

Build the **fastest, most private way to use the useful web without leaving the terminal** — a focused session client for people (and later agents) who refuse full-browser bloat and surveillance-by-default stacks.

We do **not** build “India’s Google” or “Chrome with fewer buttons.”  
We build a **terminal-native web session** that is measurable on speed and honest on trust.

---

## 2. Vision (3–5 years, directional)

| Horizon | Picture |
|---------|---------|
| **Now → v1** | Best-in-class terminal session: home, search, read, navigate, save — structure-first, DDG-only, no browser engine |
| **v1.x** | Advanced CLI density (reader quality, results UI); agent snapshot as a real protocol; “looks great in termbrowse” author guide |
| **Later** | Optional private/search *providers* behind the same UI; campus and builder distribution in India; sites/docs optimized for structure clients |

Vision check: if a milestone requires running arbitrary website JavaScript as the default, it is **out of strategy** unless this document is formally revised.

---

## 3. Strategic principles

1. **Performance is product.** No full browser engine on the hot path. Budgets are public and real.  
2. **Focus is product.** One primary loop; one search engine; refuse feature zoo.  
3. **Privacy is technical, not marketing.** No page JS execution; local-first data; no telemetry by default.  
4. **Honesty is brand.** We don’t promise the whole interactive web or CAPTCHA bypass.  
5. **Complement Chrome.** GUI browser for apps; termbrowse for reading, search, session, deep work.  
6. **Ship the 80% core solid.** The 20% is polish — never half the product.  
7. **Ecosystem after value.** Sites designed for us is phase 2; hooks in v1, dependency never.  
8. **India is a channel and a story, not a nation-state product in year one.**

---

## 4. Category and positioning

### 4.1 Category name (use this)

**Primary:** Terminal web session / structure browser  
**Avoid as lead:** “Minimal browser” (implies thin Chrome)  
**Acceptable casual:** Terminal browser — always with “structure-first, no engine”

### 4.2 Positioning statement

> **For developers and terminal-native people who need to search, open, and read the web without context-switching,**  
> **termbrowse is a private, ultra-fast terminal web session**  
> **that delivers structured pages, DuckDuckGo search, and a local Start Page**  
> **unlike full browsers (heavy, JS-everything) or scrapers (no session)**  
> **because we never run a browser engine — only what you need to stay in flow.**

### 4.3 Spine of the sell (priority order)

| Order | Message |
|-------|---------|
| **1** | Ultra-performance — no Chromium tax; fast first content |
| **2** | Privacy — no arbitrary site JS; local bookmarks/reading list |
| **3** | Focus — one engine (DDG), keyboard session, deep work |
| **4** | Builder / India channel — modest hardware, open, local-first |
| **5** | Agents — same structure for humans and tools (secondary until polished) |

### 4.4 One-liners by audience

| Audience | Line |
|----------|------|
| Global devs | Stay in the terminal for the useful web — fast, private, no browser bloat. |
| India builders / students | A lightweight, private web client that runs well on real hardware. |
| Agent users | Structured web access without driving a full browser. |
| Privacy-conscious | Read and search without executing random website JavaScript. |

### 4.5 Never promise

- Replaces Chrome/Safari for everything  
- Works on every website  
- Bypasses or solves CAPTCHAs  
- National search index / “India’s Google” at v1  
- Pixel-perfect rendering  

### 4.6 Competitive stance

| Alternative | When they win | When we win |
|-------------|---------------|-------------|
| Chrome / Edge | Web apps, media, logins, JS SPAs | Reading, search session, low overhead |
| lynx / w3m | Extreme minimalism, nostalgia | Modern UX, home, search, agents |
| Scrapers / Firecrawl | One-shot data extract | Interactive session + human TUI |
| Playwright / browser automation | Full automation fidelity | Lighter, auditable structure; no engine tax |
| Browsh / Carbonyl | Need real engine paint in TTY | Privacy/performance/no-engine purity |

**Win theme:** *Workflow + trust + speed*, not feature parity.

---

## 5. Customer and jobs-to-be-done

### 5.1 Primary customer (v1)

**Developers and builders who live in the terminal** (including India and similar markets): need docs, search, articles, public HTML — want speed, keyboard flow, less distraction and less big-tech surface.

### 5.2 Secondary (v1.x)

- Coding-agent users/authors who want structured fetch + click-by-ref  
- Students / campus labs on modest machines  
- Privacy-minded power users  

### 5.3 Jobs-to-be-done

| Job | Outcome |
|-----|---------|
| Search without leaving shell | DDG → results → open page |
| Read docs deeply | Fast structure reader, links, history |
| Come back tomorrow | Start Page favorites + reading list |
| Stay private/light | No page JS; local data; small footprint |
| (Later) Agent browse | Same document model via CLI |

### 5.4 Anti-customer (do not optimize for)

- Consumers who want Netflix/YouTube/Gmail full UI  
- Users who need Google as primary search  
- Enterprises needing SSO/compliance day one (unless separate track)  

---

## 6. Product strategy

### 6.1 Primary loop (the business of the product)

```text
Launch → Start Page → (Favorite | Search DDG | Open URL)
       → Read / follow links
       → Save (favorite or reading list)
       → Return next day
```

**Everything funds this loop.** Features that don’t strengthen it wait.

### 6.2 The 80% core (must be excellent at ship)

| # | Capability |
|---|------------|
| 1 | Start Page: favorites + reading list, CRUD, persistence |
| 2 | Open any http(s) URL: structure render, working links, history |
| 3 | DuckDuckGo-only search → result → real destination page |
| 4 | Tier A sites readable (docs, blogs, articles) |
| 5 | Designed failures: CAPTCHA, sparse SPA, network error |
| 6 | Performance: no browser engine; budgets met on Tier A |
| 7 | Privacy baseline documented and true in code |
| 8 | One install/run path + &lt;60s demo |

### 6.3 The 20% (kinks / later — not blockers for “v1 good”)

- Perfect tables / every HTML edge case  
- Mouse, multi-theme, animations  
- Cookies, login, POST forms  
- Full agent MCP  
- Indic i18n  
- Pluggable search providers  
- National-scale distribution deals  

### 6.4 Product laws (code + marketing)

| Law | Implementation |
|-----|----------------|
| No browser engine on hot path | No headless Chrome default |
| Search = DuckDuckGo HTML only | Rewrite Google/Bing paths; no multi-engine UI |
| Structure not pixels | Role → layout → ratatui |
| Local-first library | JSON under user config dir |
| Honest failure | CAPTCHA/sparse screens, not silent junk |

### 6.5 Tier A content strategy

Maintain a list of **10–20 must-work sites/URLs** (docs, FOSS, Wikipedia, HN, DDG HTML, etc.).  
Release quality is gated on Tier A, not on “the whole web.”

---

## 7. Privacy & security strategy

### 7.1 Brand promise

> We help you use the useful web **without running arbitrary website JavaScript**, and we keep your session data **on your machine** by default.

### 7.2 Technical guarantees (v1)

| Guarantee | Status required |
|-----------|-----------------|
| Do not execute page JS | Always |
| Bookmarks / reading list local only | Always |
| No telemetry by default | Always |
| HTTPS fetch only for browsing | Always |
| No selling user data | Always |

### 7.3 Trust content (ship with v1)

Short public page/section:

- What we fetch  
- What we store  
- What we never run  
- What we can’t do (CAPTCHA, full web apps)  

### 7.4 Security roadmap (post-v1, don’t block ship)

- Block private/link-local IPs (SSRF) for agent mode  
- Response size caps  
- Hostile HTML fuzz tests  

---

## 8. Performance strategy

### 8.1 Performance is a brand pillar

Publish and defend:

| Metric | Target (lock or revise here) |
|--------|------------------------------|
| Start Page interactive | &lt; 50 ms after process start |
| Static docs first useful content | p50 &lt; 500 ms (reasonable network) |
| Memory (one page) | &lt; 50 MB RSS |
| Side processes | **Zero** browser engines |
| Architecture | Single binary |

### 8.2 How we stay fast

- Refuse JS engine and Chrome  
- Cap parse/layout work on huge pages  
- Prefer main content paths  
- One concurrent browse fetch by default  
- No multi-provider search fanout in v1  

### 8.3 How we prove it

- Document budgets in README  
- Fixture HTML parse tests  
- Manual timed checks on Tier A list  
- Optional later: `termbrowse bench`  

---

## 9. Go-to-market strategy

### 9.1 Phase 0 — Dogfood (now)

| Goal | Tactic |
|------|--------|
| You use it daily | Primary loop only |
| 60s demo works cold | Script: home → search or favorite → read → save |
| Spec locked | This doc + design docs |

### 9.2 Phase 1 — Builder launch (v1 public)

| Channel | Action |
|---------|--------|
| GitHub | Clear README: position, non-promises, install, demo |
| Dev Twitter/X, Reddit (r/rust, r/commandline), HN | Show demo GIF/asciinema + privacy/speed angle |
| FOSS India / college clubs | Lightweight client for labs and students |
| Agent communities | Snapshot CLI for structured browse |

**Launch message:** performance + privacy + focus — not “India’s browser.”

### 9.3 Phase 2 — Niche depth

| Niche | Play |
|-------|------|
| Campus / education | Single binary install; low RAM story |
| Indie docs / SSG themes | “Looks great in termbrowse” checklist |
| Agent tooling | Stable refs + snapshot protocol |

### 9.4 Phase 3 — Ecosystem (only after users)

| Play | Note |
|------|------|
| Structure profile / `.well-known` | Sites opt in; you don’t wait for them to ship v1 |
| Markdown alternate for publishers | Zero-ambiguity render |
| Pluggable private search backends | Sovereignty narrative without building Google |

### 9.5 Distribution principles

- Open source client first (trust + India FOSS culture)  
- No growth hack that violates privacy promises  
- Prefer depth in builder communities over mass consumer ads  

---

## 10. India strategy (specific)

### 10.1 What we are not doing in v1

- Competing with Google Search or Chrome for national default  
- Building a crawl index  
- Government dependency as a requirement to ship  

### 10.2 What we are doing

| Pillar | Action |
|--------|--------|
| **Builders first** | Devs, students, FOSS — English-first docs OK at v1 |
| **Hardware reality** | Market low memory / no Chrome sidekick |
| **Trust** | Local data, open source, no JS execution |
| **Search** | DDG (or later community meta-search) — not “we invented Google” |
| **Long narrative** | Private digital tooling made here / for builders here — earned, not claimed day one |

### 10.3 India channels (ordered)

1. Personal network + GitHub  
2. Engineering college FOSS clubs / Discord  
3. Indian tech Twitter and communities  
4. Workshops: “terminal deep work + private reading”  
5. Only later: institutions, education boards, formal partnerships  

### 10.4 Language

- v1: English UI  
- Later: Indic language content *rendering* and UI strings as adoption demands  

---

## 11. Ecosystem & platform opportunities

### 11.1 Plant in v1 (hooks, not dependencies)

| Hook | Purpose |
|------|---------|
| Clean semantic HTML support | Sites that write good HTML look good |
| Documented role map | Authors know what we honor |
| “Looks great in termbrowse” checklist | Social proof for indie sites |
| Snapshot JSON schema | Agents and tools integrate |

### 11.2 Phase 2 mechanisms

| Mechanism | Description |
|-----------|-------------|
| `/.well-known/termbrowse.json` | Title, content selectors, nav, theme hints |
| `rel=alternate` markdown | Perfect reader path |
| Badge for static site themes | Hugo/Zola/etc. |
| Partner docs template | FOSS/docs default |

### 11.3 Strategic rule

**Never delay v1 waiting for publisher adoption.**  
Client value first → ecosystem second.

---

## 12. Agent strategy

### 12.1 Role in the business

| Stage | Role |
|-------|------|
| v1 | Secondary: `snapshot` works; not the hero pitch |
| v1.1+ | Co-equal: humans + agents same session model |
| Later | Possible B2B: hosted or team tooling (only if demand is real) |

### 12.2 Differentiation vs Playwright

- Smaller, faster, no full browser  
- Text/structure output by design  
- Lower accidental complexity for “read and follow links” jobs  
- Tradeoff: not for full web apps  

### 12.3 Guardrails

- Don’t become a botnet frontend (rate limits, politeness later)  
- SSRF protections when agents drive fetches  
- Stability of refs best-effort until fixtures exist  

---

## 13. Business model (honest sequence)

### 13.1 v1

**Revenue: none required.**  
Goal: daily use, clarity, reputation, open-source traction.

### 13.2 Optional future models (only after pull)

| Model | Fit | Risk |
|-------|-----|------|
| Donations / GitHub Sponsors | OSS culture | Low revenue |
| Paid support for teams/campus | Education/IT labs | Sales effort |
| Hosted agent snapshot API | If agents explode | Ops + abuse |
| Premium themes/sync | Weak; conflicts with local-first unless careful | Scope creep |
| Search partnership rev share | Unlikely early; careful with privacy brand | Brand risk |

### 13.3 Rule

**No monetization that requires tracking users or running more third-party JS.**  
If money conflicts with privacy/performance, privacy/performance win.

---

## 14. Brand and narrative

### 14.1 Brand attributes

| Be | Don’t be |
|----|----------|
| Fast | Bloated |
| Honest | Hype-driven |
| Focused | “Everything browser” |
| Calm / dense UI | Clownish cyberpunk noise |
| Builder-native | Consumer mass-ad energy |

### 14.2 Signature lines (approved directions)

- *Stay in the terminal for the useful web.*  
- *No full browser engine. No arbitrary site JS.*  
- *Chrome for apps. termbrowse for focused reading and search.*  
- *Private by architecture, not by settings maze.*  

### 14.3 Demo narrative (always the same)

1. Start Page  
2. Search (DDG) or open a favorite  
3. Open a result / article  
4. Follow a link  
5. Save to reading list or favorites  
6. (Optional) `snapshot` for agents  

---

## 15. Roadmap aligned to strategy

### Phase A — Foundation (v1 ship)

- 80% core loop excellent  
- Privacy page + performance numbers  
- Tier A quality bar  
- Public GitHub + demo  

**Exit criteria:** You use it daily; cold demo &lt; 60s; Tier A list passes.

### Phase B — Density (v1.1)

- Readability main-content extract  
- SERP list UI  
- Stronger agent snapshot docs  
- Author checklist “looks great in termbrowse”  

**Exit criteria:** Feels like advanced CLI, not lynx-with-theme.

### Phase C — Distribution

- Campus / FOSS India pushes  
- Editor/agent “open in termbrowse”  
- Benchmarks public  

### Phase D — Ecosystem (only with users)

- well-known structure profile  
- Optional search providers (privacy-preserving)  
- Partnerships that don’t force Chrome back  

---

## 16. Risks and mitigations

| Risk | Mitigation |
|------|------------|
| Users expect full web | Positioning + Tier A honesty |
| CAPTCHA / bot walls | DDG-only; designed screens; no Google path |
| Scope creep to Chromium | Product laws; this strategy doc |
| “India’s browser” hype trap | Builders-first narrative; no national claims early |
| Ecosystem chicken-egg | Ship client value first |
| Agent abuse | Later: rate limits, SSRF, auth |
| Burnout / unfocused build | 80% checklist; refuse 20% until core shines |

---

## 17. Decision rights

| Decision type | Rule |
|---------------|------|
| Breaks no-engine / DDG-only / local-first | Requires **written update** to this strategy |
| New feature | Must help primary loop + pass performance refusal list |
| Marketing claim | Must be true in code today |
| India national-scale pitch | Not before Phase C traction |

---

## 18. Operating cadence

| Cadence | Action |
|---------|--------|
| Weekly | Dogfood primary loop; note friction |
| Per release | Re-check 80% checklist + Tier A |
| Monthly | Re-read §3 principles; cut one distraction |
| When lost | Re-run 60s demo; if it fails, stop feature work |

---

## 19. Success metrics

### 19.1 v1 (qualitative + light quantitative)

| Signal | Target |
|--------|--------|
| Daily use by founder | Yes |
| 60s demo reliability | Cold machine works |
| Tier A pass rate | All green |
| External installs | Even small (tens) of real users &gt; vanity stars alone |
| Issue quality | Real usage bugs, not “add Chrome” as only feedback |

### 19.2 Later

| Signal | Meaning |
|--------|---------|
| Return usage | Start Page / reading list stickiness |
| Agent integrations | Others shell out to snapshot |
| Structure-profile early adopters | Ecosystem ignition |
| Campus installs | India channel working |

---

## 20. Strategy one-pager (memorize)

```text
WHO:     Terminal-native builders (global + India)
WHAT:    Private, ultra-fast web SESSION in the terminal
HOW:     Structure pipeline, no browser engine, DDG-only search, local Start Page
WHY US:  Speed + privacy by architecture + focus + modern CLI UX
NOT:     Chrome replacement, national search, CAPTCHA wars
WIN:     Daily loop excellent → niche distribution → ecosystem hooks
MONEY:   None required at v1; never sell out privacy for revenue
```

---

## 21. Commitment

By following this strategy we agree:

1. The **primary loop** is sacred.  
2. The **80% core** ships solid; the **20%** waits.  
3. **Performance and privacy** are proven, not slogans.  
4. **India** is earned through builders and product truth, not slogans.  
5. **Ecosystem** is optional upside after real use.  
6. We update this file when strategy changes — we don’t silently drift back to “full browser.”

---

*End of business strategy. Execute against this document; debate changes here before changing code direction.*
