# Umbra — costs & sustainability (hands-off)

Goal: keep call infra free or near-free and fund any overage with donations, with almost no monthly ops.

## 1. Recommended stack (lowest cost + lowest upkeep)

| Layer | Pick | Cost | Why |
| --- | --- | --- | --- |
| STUN | `stun.cloudflare.com:3478` | Free, unlimited | Official Cloudflare STUN; no account required for STUN alone |
| TURN (prod) | Cloudflare Realtime TURN (`turn.cloudflare.com`) | **1,000 GB/mo free**, then **$0.05/GB** outbound | Cheapest managed TURN; global anycast; set credentials once |
| TURN (dev / tiny tests) | Metered Open Relay | **20 GB/mo free** (API key) | Fine for local/dev; not ideal as sole production depend |
| Avoid for now | Twilio Network Traversal | STUN free; TURN **$0.40–$0.80/GB** | 8–16× Cloudflare after free tier |
| Avoid for now | Self-host coturn on a VPS | ~$4–12/mo + patching | Breaks “hands-off” |

**Default for Umbra:** Cloudflare STUN + Cloudflare TURN. Most P2P sessions never need TURN; free 1 TB/mo covers a long runway for an early open-source project.

### Rough usage (order of magnitude)

- Voice-ish ~50 kbps/direction ≈ ~0.4 GB/hour per participant pair if fully relayed
- 1080p video can be several GB/hour when TURN is forced
- With good P2P, only a minority of minutes hit TURN → free tier lasts longer than naive math

### One-time setup

1. Cloudflare account → enable Realtime TURN → create TURN credentials (short TTL recommended)
2. Put TURN URIs + credential minting in Umbra config / backend (creds should not be hard-coded permanently)
3. Ship clients with STUN always on; TURN as fallback
4. Optional budget alert in Cloudflare when approaching free-tier usage

**Monthly upkeep:** ~0 hours if alerts are set. No servers to patch.

### Sources

- Cloudflare TURN: https://developers.cloudflare.com/realtime/turn/
- Cloudflare TURN FAQ (pricing / free tier): https://developers.cloudflare.com/realtime/turn/faq/
- Metered Open Relay: https://www.metered.ca/tools/openrelay/
- Twilio STUN/TURN pricing: https://www.twilio.com/en-us/stun-turn/pricing

## 2. Funding — GitHub Sponsors (primary)

Hands-off donation path that matches a public AGPL repo.

| Item | Detail |
| --- | --- |
| Where | GitHub Sponsors on the project maintainer account, or an org later |
| Fees | **0%** on sponsorships from personal accounts (GitHub). Org sponsors: up to **6%** (3% card + 3% GitHub) |
| Setup once | Join Sponsors → profile + tiers → bank or fiscal host → tax forms → enable 2FA → wait for GitHub approval |
| Constraints | Must live in a [supported region](https://docs.github.com/en/sponsors/getting-started-with-github-sponsors/about-github-sponsors); bank region must match if using Stripe Connect |
| Suggested tiers (low maintenance) | `$3` “thanks” · `$5` “covers a bit of TURN” · `$10` “README shout-out” — no weekly newsletters or private Discord promises |
| Repo wiring | Add Sponsors button + FUNDING.yml pointing at GitHub Sponsors (optional second: Ko-fi link later) |

### Docs

- Setup (personal): https://docs.github.com/en/sponsors/receiving-sponsorships-through-github-sponsors/setting-up-github-sponsors-for-your-personal-account
- About fees: https://docs.github.com/en/sponsors/getting-started-with-github-sponsors/about-github-sponsors

## 3. What not to do (for this experiment)

- Don’t run your own TURN VPS “to save money” — labor > $0.05/GB
- Don’t rely only on public shared TURN servers with no SLA for production
- Don’t promise sponsor perks that create weekly work

## 4. Suggested next steps

1. Wire Cloudflare STUN/TURN behind short-lived credentials in the client/config
2. When ready, enable GitHub Sponsors and add a `FUNDING.yml` / Sponsors button
3. Mention Sponsors in the README once the funding page is live
