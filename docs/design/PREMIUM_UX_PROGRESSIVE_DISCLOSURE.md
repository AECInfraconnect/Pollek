# Premium UX: Plain-Language First, Technical Depth on Demand

## Problem

The Local Dashboard had grown into a *feature catalog*: pages opened with walls
of stat tiles and technical vocabulary (`capture_quality`,
`dashboard-timeline-fallback`, "Observe coverage matrix", "Exact usage /
Estimated usage", scan ids) spread across the primary surface. A non-technical
user landing on the AI Activity page met fourteen number tiles and several
jargon blocks before reaching the one thing they wanted: *what did my AI apps
do, and was anything blocked?*

Premium consumer/prosumer security apps (1Password, Cloudflare One, Vanta,
Tailscale, Apple Screen Time) share a pattern: **lead with one plain-language
answer and a few friendly numbers; keep raw/technical detail one deliberate
click away.** Nothing technical is removed — it is *demoted* behind progressive
disclosure so the default surface stays calm and legible.

## Principles

1. **Plain-language first.** The top of every page answers "what is this and
   what happened" in one sentence a non-expert understands. No field names, no
   internal nouns.
2. **A few headline numbers, not a wall.** At most ~4 friendly stats on the
   primary surface. Everything else is detail.
3. **Technical depth on demand.** Raw fields, coverage matrices, capture-quality
   notes, data-source internals, and scan ids live behind a single, calm
   "Technical details" disclosure — the same control everywhere, so users learn
   it once.
4. **One consistent chrome.** Every page uses the same header shape and the same
   disclosure, so the product reads as one system, not forty bespoke screens.
5. **Respect the persona modes.** In `desktop_simple` the technical panels stay
   collapsed by default; in `desktop_advanced` they open by default. The
   information is identical — only the default disclosure state differs.

## Shared primitives

- **`components/ui/TechnicalDetails.tsx`** — the single progressive-disclosure
  surface. A calm, muted toggle (slider icon, "Technical details" label,
  optional count/hint) that reveals a subdued panel. Use it instead of bespoke
  collapsibles so depth is disclosed the same way across the product. Its
  `defaultOpen` is wired to `isAdvanceMode(mode)` on each page.
- **`components/layout/PageHeader.tsx`** — the single page header: icon + plain
  title + one plain-language sentence + right-aligned actions. It deliberately
  has no slot for technical sub-labels; those go into the page body behind
  `TechnicalDetails`.
- **Global content frame** — `DashboardLayout` now centers page content in a
  `max-w-[1600px]` column with generous padding, so wide screens feel composed
  rather than stretched.

## Reference implementation: AI Activity

The AI Activity page is the pattern's first full application:

- **Header** → `PageHeader` ("AI Activity" + one plain sentence + Observe/Export
  actions).
- **Hero** → `ActivityHeadline`: one sentence ("Pollek watched your AI apps and
  recorded N recent activities. Nothing was blocked.") plus four friendly
  headline numbers (Activities, Blocked, Safety checks, Cost). The `Blocked`
  stat turns red only when non-zero.
- **Technical details** → one `TechnicalDetails` panel now holds everything that
  used to crowd the surface: the per-category breakdown tiles, the latest
  observe-refresh breakdown and scan id, capture quality, "what may need setup",
  the data-source indicator, and the observe-coverage matrix.
- The plain-language activity timeline and the Prompt Guard safety card remain
  the main content.

Net effect: the first screen went from ~14 tiles + 3 jargon blocks to one
sentence, four numbers, and a single "Technical details" toggle — with zero loss
of information for advanced users.

## Rollout

Every page adopts the same two primitives:

1. Replace the bespoke `text-2xl font-bold` header with `PageHeader` (plain
   title + one plain sentence + actions).
2. Move any raw/technical sub-content (field dumps, coverage matrices, internal
   status vocabulary) into a `TechnicalDetails` panel, leaving a plain-language
   summary on the surface.

Pages are migrated incrementally; the primitives and the AI Activity reference
implementation define the target for the rest of the surface (Overview, Find AI
Apps, My AI Apps, Data & Apps, Cost, Detection Coverage, Setup, Policies, Tools
& Resources, History, and the remaining screens).
