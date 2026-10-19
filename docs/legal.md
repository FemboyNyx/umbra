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
- **This file does not cover:** trademark clearance for “Umbra,” patents, GDPR/CCPA product design in general, or non-U.S. crypto import rules. Community-server hosting (including GDPR for host metadata) is in section 4.

---

## 4. Community servers and blind hosts

**Scope.** This covers people who run a **blind host** for a community server (see [servers.md](servers.md)): an always-on node that stores and forwards Megolm ciphertext, encrypted file chunks (up to 100 MiB per file, proposed) and queued Olm envelopes for an invite-only server, keeps them up to 30 days (proposed), and never gets session keys. **US and EU only.** Every other country, and every EU Member State's national law, still applies on top. **Not legal advice.** Hosts are usually private individuals. Anyone hosting for strangers, at scale, or for money should talk to a lawyer where they live.

Umbra the project runs no hosts, index or relay nodes ([servers.md](servers.md), "Nothing depends on a node we run"). The person who runs a host is that host's operator. Writing and publishing the software is a separate question that this section doesn't cover.

**What a blind host can and can't see.** This drives everything below. It **can't** read messages, files, channel names, server names or rosters. It **does** hold member PeerIds (from its `HostGrant`), connecting IP addresses (unless the member uses Tor), timestamps, envelope and blob sizes, which PeerId pushes and fetches what, who sends Olm relays to whom, `ServerId`s, and `BlobId`s (SHA-256 of each ciphertext chunk). A **member host** is different: it reads everything it holds keys for, so for legal purposes it should be treated like any other member who can see the content.

### 4.1 Host operator liability

**US: 47 U.S.C. § 230.** Under § 230(c)(1), a provider or user of an "interactive computer service" is not treated as the publisher or speaker of information provided by someone else. A blind host storing members' ciphertext fits the usual reading well. Its limits, from § 230(e), are what matter:
- **No shield against federal criminal law** (§ 230(e)(1)). That explicitly includes chapter 110 of title 18 (child sexual exploitation) and obscenity law.
- **No effect on intellectual property law** (§ 230(e)(2)). Copyright goes through the DMCA (4.2).
- **No effect on ECPA / communications privacy law** (§ 230(e)(4)).
- **No shield against sex-trafficking claims** under 18 U.S.C. §§ 1591/1595 or related state charges (§ 230(e)(5), added by FOSTA, Pub. L. 115-164, 2018).

**EU: Digital Services Act, Regulation (EU) 2022/2065 (applies from 17 Feb 2024, Art. 93).**
- **Does the DSA reach a hobby host at all?** It applies to "intermediary services", which have to be *information society services*: services "normally provided for remuneration" (Art. 3(a), pointing to Directive (EU) 2015/1535 Art. 1(1)(b)). Whether a free, non-commercial host run by one person for friends counts is **unclear, and I couldn't verify a definitive answer**. Assume it might, and follow the light-touch rules below. They're cheap to follow anyway.
- **Which role?** Live relaying looks like **mere conduit** (Art. 4: provider doesn't initiate the transmission, select the receiver, or select or modify the content). Holding envelopes and chunks for up to 30 days is **storage at a recipient's request**, which makes the host a **hosting service** (Art. 3(g)(iii), Art. 6). Plan for the hosting rules.
- **Hosting safe harbour (Art. 6(1)).** The host is not liable for stored information if it has no **actual knowledge** of illegal content (for damages claims, no awareness of facts making it apparent), **or** if, once it knows, it **acts expeditiously** to remove or disable access. The safe harbour doesn't apply if the uploader acts "under the authority or the control of the provider" (Art. 6(2)). A host that is also a member, admin or owner of the server it hosts should keep that in mind.
- **No general monitoring (Art. 8).** No obligation to monitor stored data or to actively look for illegal content may be imposed. A blind host isn't expected to scan, and can't.
- **Probably not an "online platform".** An online platform stores *and disseminates to the public*, meaning to "a potentially unlimited number of third parties" (Art. 3(i), (k)). Recital 14 says services used for interpersonal communication between a finite number of persons determined by the sender, "such as emails or private messaging services", fall outside the online-platform definition, but **public groups or open channels** may fall inside it. An **invite-only** server with a member cap (250, proposed) fits the first description best. A **publicly listed server anyone can join** (phase 2) moves toward the second, so revisit this before public servers ship.
- **Duties that apply to every hosting service, small or not:** a notice-and-action mechanism (Art. 16), a statement of reasons when you restrict someone (Art. 17), notifying police of suspected criminal offences involving a **threat to life or safety** (Art. 18), a single point of contact for authorities (Art. 11) and for users (Art. 12), clear terms and conditions (Art. 14), and, for providers established outside the EU that offer services in it, a **legal representative** in the EU (Art. 13).
- **Small-provider relief that does exist:** micro and small enterprises (Recommendation 2003/361/EC) are exempt from annual transparency reports (Art. 15(2)) and from the extra online-platform duties (Art. 19). There is **no** small-enterprise exemption from Arts. 16, 17 or 18.

### 4.2 Copyright: DMCA and EU notice-and-action

**US: 17 U.S.C. § 512.**
- **Which safe harbour?** § 512(a) covers transmission with only "intermediate and transient" storage. Store-and-forward for up to 30 days is better treated as **§ 512(c)**, storage "at the direction of a user". Whether § 512(a) also covers offline-delivery queues is **not settled here. I didn't verify any case law on it.**
- **§ 512(c) conditions:** no actual knowledge or red-flag awareness, no financial benefit directly attributable to infringement the host can control, and **expeditious removal** on knowledge or on a valid notice (§ 512(c)(1)).
- **Designated agent (§ 512(c)(2)).** The safe harbour only applies if the host has registered an agent with the **U.S. Copyright Office** and shows the same details publicly. Under 37 CFR § 201.38: register in the online system; give your **full legal name and a physical street address** (a P.O. box only with a written waiver for personal-safety reasons, § 201.38(b)(1)(ii)); **renew every 3 years** or the designation lapses (§ 201.38(c)(4)). The **fee is $6** per designation, amendment or resubmission (37 CFR § 201.3(c)(25)). Publishing a home address is a real privacy cost for individuals, so think about it before you register.
- **Repeat infringers (§ 512(i)(1)(A)).** All the safe harbours require a policy, adopted, reasonably implemented and told to users, for terminating repeat infringers "in appropriate circumstances". For a host that means refusing a PeerId locally after repeated valid notices (see the host denylist under 4.5).
- **No duty to monitor (§ 512(m)).** Safe harbour can't be made conditional on monitoring or looking for infringement.
- **Encrypted content.** A valid notice must identify the material and give information "reasonably sufficient to permit the service provider to locate the material" (§ 512(c)(3)(A)(iii)). A blind host can't see a filename or a URL. In practice a workable notice has to come from someone who **can** see the content (a member) and give the **`ServerId` plus `BlobId`s or envelope identifiers**. The host can then drop those exact blobs without decrypting anything. A notice the host can't map to anything it stores should get a reply asking for those identifiers (compare § 512(c)(3)(B)(ii)). Counter-notices are covered in § 512(g).
- **DMCA subpoenas (§ 512(h))** can compel identifying information "to the extent such information is available to the service provider". For a blind host that's at most PeerIds, IPs and timestamps it still has. Short log retention limits what exists.

**EU.**
- Copyright notices go through the same **DSA Art. 16** mechanism. A notice that is "sufficiently precise and adequately substantiated" creates actual knowledge for Art. 6 (Art. 16(3)). The required "exact electronic location" (Art. 16(2)(b)) is again the `ServerId` plus `BlobId`.
- **DSM Directive (EU) 2019/790, Art. 17 most likely does not apply.** It covers "online content-sharing service providers": services whose main purpose is to give **the public** access to a large amount of copyrighted works, which they organise and promote **for profit** (Art. 2(6)). The same definition excludes providers of electronic communications services under Directive (EU) 2018/1972 and "cloud services that allow users to upload content for their own use". An invite-only, non-profit, E2EE store-and-forward host doesn't fit the definition.

### 4.3 CSAM when the host can't see content

**US: 18 U.S.C. § 2258A.**
- **Who it covers:** a "provider", meaning an electronic communication service or remote computing service provider (§ 2258E). An ECS is "any service which provides to users thereof the ability to send or receive" electronic communications (18 U.S.C. § 2510(15)), with no "to the public" requirement. Assume a blind host is covered.
- **When the duty applies:** only on **actual knowledge** of facts or circumstances showing an apparent violation of the listed child-pornography offences (§ 2258A(a)(1)(A)(i), (a)(2)(A)). Since the **REPORT Act** (Pub. L. 118-59, 7 May 2024), the duty also covers child sex trafficking (§ 1591, where a minor is involved) and enticement (§ 2422(b)). The report goes to NCMEC's **CyberTipline**.
- **No duty to scan:** § 2258A(f) says nothing in the section requires a provider to monitor users or the content of communications, or to affirmatively search for or screen content.
- **REPORT Act changes (verified in the current code):** a completed CyberTipline report counts as a request to **preserve the reported contents for 1 year** (up from 90 days) (§ 2258A(h)(1)), and providers may voluntarily keep them longer (§ 2258A(h)(5)). Fines for knowingly and willfully failing to report are now up to **$600,000** for a first failure and **$850,000** for later ones for providers with fewer than 100 million monthly active users (§ 2258A(e)). Larger providers face $850,000 and $1,000,000.
- **What that means for a blind host.** Ciphertext alone almost never gives you actual knowledge. You can get it, though, from **a member report**, a **law-enforcement notice**, or a **notice that names specific `BlobId`s** as CSAM. If that happens: **don't decrypt, open or forward the material, and don't ask for a copy**. Preserve the identified blobs and the related metadata, take them out of service, and file a CyberTipline report (https://report.cybertip.org/) with what you have (ServerId, BlobIds, PeerIds, IPs, timestamps). Sharing contents with NCMEC in a report is a permitted disclosure under 18 U.S.C. § 2702(b)(6). The 1-year preservation rule **overrides the 30-day auto-delete** for anything you reported.

**EU.**
- **No EU law in force (as of 19 Oct 2026) requires a messaging or hosting service to scan private messages.** The old interim derogation, **Regulation (EU) 2021/1232** (extended to 3 April 2026 by Reg. (EU) 2024/1307), **expired on 3 April 2026**. It was replaced by **Regulation (EU) 2026/1881** (OJ 28 Jul 2026, in force 31 Jul 2026, **applies until 3 April 2028**). That regulation only *permits* voluntary detection by number-independent interpersonal communications services, and **does not apply to communications that are end-to-end encrypted** (Art. 1(3)). It neither requires nor allows anything for Umbra's blind hosts.
- **The proposed CSA Regulation** (COM(2022) 209, procedure 2022/0155(COD)) **has not been adopted**. Per press and tracker reports on the 29 Sep 2026 sixth trilogue, negotiators agreed on the EU Centre's tasks and, reportedly, in principle on detection orders for *public* content, but **not** on scanning private messages. No next trilogue date was confirmed. Recheck this before every release. If a final text requires detection or removal orders against private or E2EE services, this section needs rewriting.
- **What does apply now:** DSA Art. 6 (act expeditiously once you know), Art. 16 (accept notices, including anonymous ones about the child-abuse offences in Directive 2011/93/EU, Art. 16(2)(c)), Art. 18 (tell the police about threats to life or safety), and **national criminal law**, which may add reporting duties of its own. I didn't check any Member State's law.

### 4.4 Data retention and law-enforcement requests

**US: Stored Communications Act, 18 U.S.C. §§ 2701–2713.**
- **Content:** for content in electronic storage **180 days or less**, the government needs a **warrant** (§ 2703(a)). With a 30-day retention window, everything a host stores is in that bracket. What the warrant gets is **ciphertext**.
- **Records:** basic subscriber records, including "records of session times and durations" and "any temporarily assigned network address" (meaning IPs), can be obtained by **subpoena** (§ 2703(c)(2)). Other non-content records need a § 2703(d) court order or a warrant.
- **Preservation (§ 2703(f)):** when the government asks, you **must** preserve the records and evidence you already have **for 90 days, extended by another 90 days on renewal**. That suspends auto-delete for whatever the request covers. Preservation doesn't mean disclosure: you still hand nothing over without legal process.
- **Voluntary disclosure (§ 2702):** providers "to the public" mustn't divulge content or give records to government except under exceptions, including: to an addressee, with consent, to NCMEC under § 2258A (b)(6), and in a good-faith **emergency involving danger of death or serious physical injury** (b)(8). Whether an invite-only host is "to the public" is **unclear (not verified)**. Follow § 2702 anyway as good practice.
- **CLOUD Act (18 U.S.C. § 2713, Pub. L. 115-141, 2018):** a US provider has to preserve or disclose data in its possession, custody or control **wherever it's stored**. Hosting abroad doesn't take a US operator outside US process.

**EU.**
- **GDPR (Regulation (EU) 2016/679) applies to the metadata.** PeerIds, IP addresses and timestamps tied to a person are personal data. The household exemption (Art. 2(2)(c)) covers "purely personal or household activity", but Recital 18 says the GDPR still applies to those who "provide the means for processing" for such activities. Hosting for people you don't know is unlikely to be purely household. Main duties: data minimisation and storage limitation (Art. 5(1)(c), (e)), a short privacy notice, and dealing with data-subject requests for whatever you hold.
- **No EU-wide data-retention mandate.** The Data Retention Directive 2006/24/EC was struck down in **Digital Rights Ireland** (Joined Cases C-293/12 and C-594/12, 8 Apr 2014). The CJEU has since ruled out general and indiscriminate retention of traffic and location data in **Tele2 Sverige / Watson** (Joined Cases C-203/15 and C-698/15, 21 Dec 2016) and **La Quadrature du Net** (Joined Cases C-511/18, C-512/18 and C-520/18, 6 Oct 2020), with narrow exceptions (e.g. serious threats to national security). National retention laws exist and vary. As of 19 Oct 2026 the Commission has run an impact assessment on new EU data-retention rules, but **I found no tabled legislative proposal**. Recheck.
- **e-Evidence Regulation (EU) 2023/1543 applies from 18 Aug 2026** (Art. 34). Judicial authorities can send **European Production Orders** and **European Preservation Orders** to service providers offering services in the Union (Art. 2(1)). "Service provider" includes electronic communications services and information society services that let users communicate or store data (Art. 3(3)). The companion **Directive (EU) 2023/1544** (legal representatives) had a transposition deadline of 18 Feb 2026. Whether a private hobby host is in scope runs into the same "remuneration" question as the DSA. **Not verified.**

**Practical upshot.** Whatever process a host receives, it can hand over **ciphertext, PeerIds, IPs, timestamps, sizes and who-sent-to-whom for Olm relays**, and nothing more. Collect less and there's less to hand over:
- Keep **no IP or connection logs** beyond what's needed to run (or rotate them in hours, not weeks). Encourage Tor and onion hosting.
- Keep the **30-day hard delete** (shorter is fine), and actually delete, not just hide.
- Don't log `Fetch` and `Push` patterns or Olm relay sender/recipient pairs after delivery.
- Write down how you'll handle a preservation request (snapshot only the named records, suspend deletion for them only) and when you'll lift it (90 days unless renewed).
- Only hand anything over in response to valid legal process, and tell the affected member when you're allowed to.

### 4.5 What the host program should show (warnings and terms)

**Before hosting starts (one-time onboarding, has to be accepted):**
> **You are about to host encrypted data for other people.** You can't read it. You can still receive legal notices, preservation requests, subpoenas, warrants or court orders about it, and the law where you live applies to you as the operator. You'll hold members' PeerIds, IP addresses (unless they use Tor), timestamps and data sizes for up to [N] days. Only host for people you trust. If you're told about illegal material (for example child sexual abuse material), you may be legally required to act and to report it. Don't open or decrypt it. This program doesn't give legal advice. You must be **18 or older** (or the age of majority where you live) to run a host.

**Settings the host program should have** (requests to Coding League):
- **Retention:** default 30 days, adjustable down, hard delete.
- **Logs:** IP and connection logging off by default, with a short rotation window if it's turned on.
- **Contact and notice address** (an email or form) that the host can publish, and that members can see in the join preview.
- **Blob denylist:** "Remove these `BlobId`s / envelopes", given by ID, with no decryption. Removed IDs get `Rejected` on future `Put`/`Get` so the same chunk can't be re-uploaded.
- **Peer denylist:** refuse a PeerId locally even if it's in the owner's `HostGrant`. That's what makes a repeat-infringer policy possible.
- **Preservation hold:** freeze specific records past auto-delete, with an expiry date.
- An **"I am a member host (I can read everything)"** mode with a stronger warning. Member hosts can get actual knowledge directly.

**Acceptable-use policy (short, shown to members in the join preview):**
> This server's host stores encrypted messages and files it can't read. Don't use it for: child sexual abuse material or exploitation of minors; content that infringes copyright; threats, harassment or incitement to violence; malware; or anything illegal where you or the host live. The host may remove specific files or messages it's notified about and may refuse service to any account. Report problems to: [host contact].

**Notice template (for the host's contact page):**
> **Notices to [host name / PeerId]**
> This host stores end-to-end encrypted data and can't see its content. To report illegal content or claimed copyright infringement, send: (1) the **ServerId** and the **BlobId(s)** or message identifiers concerned (visible to members in the Umbra app); (2) why you believe it's illegal, or the copyrighted work concerned; (3) your name and email address (you may omit these for child sexual abuse reports); (4) a statement that the notice is accurate and made in good faith, and for copyright claims, the statements and signature required by 17 U.S.C. § 512(c)(3). Contact: [email]. DMCA designated agent: [if registered, the name and address as filed with the U.S. Copyright Office].

**Takedown handling (operator checklist):**
1. Log the notice (date, sender, IDs).
2. Map it to the `BlobId`s or envelopes you hold. If you can't, ask for the IDs.
3. If it maps, **disable access to those exact items expeditiously**, add them to the blob denylist, and tell the server owner and the affected member why (DSA Art. 17 statement of reasons).
4. For CSAM: **preserve, don't view, report to NCMEC** (US) or the police (EU), and keep for the period required (1 year after a CyberTipline report).
5. For copyright: process counter-notices under § 512(g). Refuse the PeerId after repeated valid notices.
6. Keep a record of what you did. Don't keep the content itself longer than needed.

**Jurisdiction caveat (show in onboarding and docs):** These notes cover only US federal law and EU-level law as of October 2026. State law, EU Member State law and the law of any other country where you or your members live can add duties (reporting, retention, age checks, encryption rules). Running a host is your own legal responsibility, not the Umbra project's.

**Age note.** The 18+ host requirement above is a project recommendation, not a quote from any statute. Hosts take on legal duties (DMCA registration, reports, responding to legal process) that minors may not be able to take on themselves. For members, the GDPR's default age for consent-based processing of children's data in information society services is 16, and Member States can lower it to no less than 13 (Art. 8(1)). In the US, check COPPA if the app is aimed at children. **Not analysed here.**

### 4.6 Couldn't verify (open items)

- Whether a free, non-commercial host run by an individual is an "information society service" (DSA, e-Evidence), and so whether the DSA applies to it at all.
- Whether § 512(a) (transit) rather than § 512(c) covers store-and-forward queues for offline members.
- Whether an invite-only blind host is a provider "to the public" for 18 U.S.C. § 2702.
- National CSAM-reporting or data-retention duties in individual EU Member States.
- The final shape of the EU CSA Regulation (still in trilogue) and any future EU data-retention proposal.

---

## 5. Sources (verify before release)

- 15 CFR § 742.15 — https://www.ecfr.gov/current/title-15/subtitle-B/chapter-VII/subchapter-C/part-742/section-742.15  
- 15 CFR § 734.17 — https://www.ecfr.gov/current/title-15/subtitle-B/chapter-VII/subchapter-C/part-734/section-734.17  
- 15 CFR § 734.7 — https://www.ecfr.gov/current/title-15/subtitle-B/chapter-VII/subchapter-C/part-734/section-734.7  
- 15 CFR § 740.17 — https://www.ecfr.gov/current/title-15/subtitle-B/chapter-VII/subchapter-C/part-740/section-740.17  
- 15 CFR § 740.13 — https://www.ecfr.gov/current/title-15/subtitle-B/chapter-VII/subchapter-C/part-740/section-740.13  
- 15 CFR § 772.1 — https://www.ecfr.gov/current/title-15/subtitle-B/chapter-VII/subchapter-C/part-772/section-772.1  
- BIS: Encryption items not subject to the EAR — https://www.bis.gov/learn-support/encryption-controls/encryption-items-not-subject-to-ear  
- GNU AGPL-3.0 — https://www.gnu.org/licenses/agpl-3.0.html  
- 47 U.S.C. § 230 — https://www.law.cornell.edu/uscode/text/47/230  
- 17 U.S.C. § 512 — https://www.law.cornell.edu/uscode/text/17/512  
- 37 CFR § 201.38 (designated agent) — https://www.law.cornell.edu/cfr/text/37/201.38  
- 37 CFR § 201.3 (Copyright Office fees; (c)(25) agent designation $6) — https://www.law.cornell.edu/cfr/text/37/201.3  
- U.S. Copyright Office DMCA Designated Agent Directory — https://www.copyright.gov/dmca-directory/  
- 18 U.S.C. § 2258A — https://www.law.cornell.edu/uscode/text/18/2258A  
- 18 U.S.C. § 2258E — https://www.law.cornell.edu/uscode/text/18/2258E  
- REPORT Act, Pub. L. 118-59 (7 May 2024) — https://www.govinfo.gov/app/details/PLAW-118publ59  
- NCMEC CyberTipline — https://report.cybertip.org/  
- 18 U.S.C. § 2510 (definitions) — https://www.law.cornell.edu/uscode/text/18/2510  
- 18 U.S.C. § 2702 — https://www.law.cornell.edu/uscode/text/18/2702  
- 18 U.S.C. § 2703 — https://www.law.cornell.edu/uscode/text/18/2703  
- 18 U.S.C. § 2711 — https://www.law.cornell.edu/uscode/text/18/2711  
- 18 U.S.C. § 2713 (CLOUD Act) — https://www.law.cornell.edu/uscode/text/18/2713  
- Regulation (EU) 2022/2065 (Digital Services Act) — https://eur-lex.europa.eu/eli/reg/2022/2065/oj  
- Directive (EU) 2015/1535, Art. 1(1)(b) — https://eur-lex.europa.eu/eli/dir/2015/1535/oj  
- Directive (EU) 2019/790 (DSM), Arts. 2(6), 17 — https://eur-lex.europa.eu/eli/dir/2019/790/oj  
- Regulation (EU) 2021/1232 — https://eur-lex.europa.eu/eli/reg/2021/1232/oj  
- Regulation (EU) 2024/1307 — https://eur-lex.europa.eu/eli/reg/2024/1307/oj  
- Regulation (EU) 2026/1881 — https://eur-lex.europa.eu/eli/reg/2026/1881/oj  
- Council press release, 23 Jul 2026 (interim measure reinstated) — https://www.consilium.europa.eu/en/press/press-releases/2026/07/23/fighting-child-sexual-abuse-online-interim-measure-protecting-children-now-reinstated/  
- CSA Regulation trilogue, 29 Sep 2026 (press reports) — https://agenceurope.eu/en/bulletin/article/13949/2/csam-regulation-co-legislators-agree-on-detection-of-public-content-and-tasks-of-eu-centre-for-child-protection ; https://netzpolitik.org/2026/chatkontrolle-verhandlungen-eu-staaten-und-parlament-weiter-uneins/  
- Regulation (EU) 2016/679 (GDPR) — https://eur-lex.europa.eu/eli/reg/2016/679/oj  
- CJEU C-293/12 & C-594/12 Digital Rights Ireland — https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:62012CJ0293  
- CJEU C-203/15 & C-698/15 Tele2 Sverige — https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:62015CJ0203  
- CJEU C-511/18, C-512/18 & C-520/18 La Quadrature du Net — https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:62018CJ0511  
- EPRS briefing, Renewed debate on a future EU data retention framework (2026) — https://www.europarl.europa.eu/RegData/etudes/BRIE/2026/789334/EPRS_BRI%282026%29789334_EN.pdf  
- European Commission, data retention — https://home-affairs.ec.europa.eu/policies/internal-security/lawful-access-data/data-retention_en  
- Regulation (EU) 2023/1543 (e-Evidence) — https://eur-lex.europa.eu/eli/reg/2023/1543/oj  
- Directive (EU) 2023/1544 (legal representatives) — https://eur-lex.europa.eu/eli/dir/2023/1544/oj  

*Maintainer draft. Update citations if BIS amends the EAR or the laws in section 4 change (US/EU status checked 19 Oct 2026). Not legal advice.*
