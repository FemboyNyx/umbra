# Umbra — Legal & compliance notes

**Status:** Draft for project maintainers. **Not legal advice.** Laws change; confirm with counsel or BIS before relying on this for exports or commercial distribution.

**Repo:** https://github.com/FemboyNyx/umbra (public)

---

## 1. License: AGPL-3.0 is a good fit

**Recommendation:** Use **GNU Affero General Public License v3.0** (`AGPL-3.0`) for Umbra’s project-authored code.

### Why it fits

- Umbra is a **P2P end-to-end encrypted messenger** that may include relays, signaling, or other network services. AGPL-3.0’s **remote network interaction** rule (section 13) requires that if someone modifies a covered version that supports remote network interaction, they must offer Corresponding Source to users who interact with it over the network.
- That closes the classic “ASP loophole” in GPLv3 for hosted or relayed services—important for a privacy-focused messenger.
- Peer projects use AGPL for similar goals (e.g. Signal Desktop is AGPL-3.0).

### Tradeoffs (intentional)

- Strong copyleft: derivative works and combined works must stay under compatible terms; many companies avoid AGPL for proprietary products.
- Does **not** by itself stop surveillance forks from compiling the code—it only keeps modified network deployments under source-sharing obligations when section 13 applies.
- Docs, logos, fonts, and third-party deps need their **own** license inventory; do not assume the repo license covers artwork or vendored libraries.

### Setup checklist

1. Add a root `LICENSE` file with the full AGPL-3.0 text (https://www.gnu.org/licenses/agpl-3.0.html).
2. SPDX identifier in source headers where appropriate: `AGPL-3.0-only` or `AGPL-3.0-or-later` (pick one and stay consistent).
3. Track third-party licenses separately (NOTICE or `docs/third-party.md`).

---

## 2. U.S. export controls (encryption / EAR)

Umbra includes **cryptographic software**. Under U.S. law, posting encryption software on the public internet can be an **export**. Primary rules live in the **Export Administration Regulations (EAR)**, administered by the Bureau of Industry and Security (BIS), U.S. Department of Commerce.

### Key citations

| Topic | Citation | Summary |
|--------|----------|---------|
| Encryption items (policy & public source code) | **15 CFR § 742.15** | Controls on encryption items; **§ 742.15(b)** addresses publicly available encryption source code |
| What counts as an encryption software “export” | **15 CFR § 734.17** | Includes making encryption source/object code available for transfer outside the U.S. (e.g. public web/GitHub) unless adequate precautions apply |
| “Published” / publicly available software | **15 CFR §§ 734.3(b)(3), 734.7** | Unrestricted public posting (including the internet) can make software “published”; encryption software has extra rules |
| License Exception ENC (when items remain subject to the EAR) | **15 CFR § 740.17** | Authorization path for many encryption commodities/software that are still subject to the EAR (e.g. mass-market / classified items) |
| License Exception TSU (historical note) | **15 CFR § 740.13** | Older path for some unrestricted tech/software; **encryption publicly available source-code notifications moved to § 742.15(b)**. BIS: notifications made before Sept 2016 under TSU remain valid under § 742.15 |
| “Non-standard cryptography” definition | **15 CFR § 772.1** | Proprietary/unpublished crypto or algorithms/protocols **not** adopted/approved by a recognized standards body (e.g. IETF, IEEE, ISO) and not otherwise published |

Official BIS overview:  
https://www.bis.gov/learn-support/encryption-controls/encryption-items-not-subject-to-ear

### Public open-source encryption source code (§ 742.15(b))

Per **15 CFR § 742.15(b)(1)** and BIS guidance:

- Publicly available encryption **source code** classified under **ECCN 5D002** is **not subject to the EAR**, subject to the notification rules in **§ 742.15(b)(2)**.
- “Publicly available” includes free open-source code posted on the public internet (see **§§ 734.3(b)(3), 734.7**).

**Notification for “non-standard cryptography” (§ 742.15(b)(2)):**

- If the publicly available 5D002 source code provides or performs **“non-standard cryptography”** (as defined in **§ 772.1**), you must notify:
  - BIS: **crypt@bis.doc.gov**
  - ENC Encryption Request Coordinator: **enc@nsa.gov**
- Notify of the internet location (URL) **or** provide a copy of the source.
- Re-notify when cryptographic functionality is updated/modified, or when the posted URL changes (updates at an already-notified URL do not require a new notice of each code change—follow the regulation’s exact wording).

If Umbra uses only **standard, published** algorithms/protocols (e.g. well-known IETF TLS/Noise-style constructions adopted by standards bodies), the **§ 742.15(b)(2)** email may **not** be required—but maintainers should still document the crypto inventory and re-check § 772.1 before assuming that.

**Corresponding object code:** Publicly available encryption **object code** is not subject to the EAR when the corresponding source is publicly available **and** has been notified as specified under **§ 742.15(b)** (BIS item 3 on the page above).

### Important BIS caveats

1. **Incorporating open-source crypto ≠ automatic public availability of a new product.** A new binary/app that embeds or calls publicly available encryption source is a **new item** and must be evaluated under the EAR as a whole (BIS Note 2).
2. **Mass-market free apps** may follow a different path: classify under mass-market rules (**§ 740.17(b)(1)** or **(b)(3)** → often **ECCN 5D992.c**), then if made publicly available free of charge, may become not subject to the EAR (BIS item 1). That is **separate** from the open-source source-code path.
3. **Country Group E:1 / E:2** and government end-user restrictions still matter for items that **remain** subject to the EAR and for License Exception ENC (**§ 740.17**). Do not assume “open source” alone authorizes every destination or end user for controlled binaries.

### Practical checklist for Umbra maintainers

1. Keep **all** encryption-related source in the public GitHub repo (no private crypto forks that undermine “publicly available”).
2. Document algorithms/protocols used (and whether each is standards-body adopted / published).
3. If any “non-standard cryptography” is present (or unsure): send the **§ 742.15(b)** notice to `crypt@bis.doc.gov` and `enc@nsa.gov` with the repo URL before or upon public release of that code.
4. For **store / packaged binaries** (Play Store, installers): evaluate mass-market / ENC classification separately; do not rely only on the source-code exception.
5. Add a short **Cryptography Notice** in the README (see template below). Import laws in other countries still apply to users—U.S. EAR does not clear foreign import restrictions.

### README cryptography notice (template)

> This distribution may include cryptographic software. The country in which you reside may restrict import, possession, use, and/or re-export of encryption software. Check your local laws before use.
>
> Under U.S. law, encryption source code that is publicly available may be treated as not subject to the EAR when the conditions of **15 CFR § 742.15(b)** (and related published/publicly available rules in **15 CFR part 734**) are met. Maintainers should complete any required BIS/ENC notifications. This notice is informational only and is not an official classification.

---

## 3. Other compliance (short)

- **Privacy / wiretap:** Building an E2EE messenger does not by itself create a duty to build government backdoors under U.S. law, but operators of **services** (if any) may face separate lawful-process obligations. Prefer true P2P with minimal retained metadata.
- **App stores:** Google Play / Microsoft Store have their own cryptography and export questionnaires—answer consistently with the ECCN/path you actually use.
- **This file does not cover:** trademark clearance for “Umbra,” patents, GDPR/CCPA product design, or non-U.S. crypto import rules.

---

## 4. Sources (verify before release)

- 15 CFR § 742.15 — https://www.ecfr.gov/current/title-15/subtitle-B/chapter-VII/subchapter-C/part-742/section-742.15  
- 15 CFR § 734.17 — https://www.ecfr.gov/current/title-15/subtitle-B/chapter-VII/subchapter-C/part-734/section-734.17  
- 15 CFR § 734.7 — https://www.ecfr.gov/current/title-15/subtitle-B/chapter-VII/subchapter-C/part-734/section-734.7  
- 15 CFR § 740.17 — https://www.ecfr.gov/current/title-15/subtitle-B/chapter-VII/subchapter-C/part-740/section-740.17  
- 15 CFR § 740.13 — https://www.ecfr.gov/current/title-15/subtitle-B/chapter-VII/subchapter-C/part-740/section-740.13  
- 15 CFR § 772.1 — https://www.ecfr.gov/current/title-15/subtitle-B/chapter-VII/subchapter-C/part-772/section-772.1  
- BIS: Encryption items not subject to the EAR — https://www.bis.gov/learn-support/encryption-controls/encryption-items-not-subject-to-ear  
- GNU AGPL-3.0 — https://www.gnu.org/licenses/agpl-3.0.html  

*Maintainer draft. Update citations if BIS amends the EAR. Not legal advice.*
