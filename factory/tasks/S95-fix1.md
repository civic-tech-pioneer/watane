Subsystem: app
Effort: low
Depends on: S95-2, S95-4
Slice: S95 — The agent layer in watane's contracts (agentic-harness TECHNICAL 15, N3). Docs only.

## Goal
app/TECHNICAL.md section 19 and app/FUNCTIONAL.md use the names that host 19 (validated in S95-2)
gives for what S95-4 asked of host, so the app's agent layer and the host's agree word for word.

## Acceptance
- AC1 app 15's table "asked of host, not in host 19 yet" is gone; each item is a row of what host
  gives, with host's name: `Confirm::consent(&self, ConsentRequest) -> Pin<Box<dyn Future<Output =
  ConsentAnswer> + Send>>` (one method for the three consents, replacing the app's `Confirm::agent`);
  `ConsentRequest { origin, via, consent, profile: Option<String>, functions }`; `ConsentAnswer
  { Granted, Refused }`; `Consent { Workspace { folder }, Account { account }, Toolset { toolset } }`
  with its `what` text (`workspace:<folder>`, `account:openai`, `toolset:<name>`), which is
  `ConsentLine.key`; `Host::withdraw_consent(&LocalName, &Consent)`; `Confirm::accounts(&self) ->
  Option<Arc<dyn AccountSource>>`; `AccountError { NoAccount, Unavailable(String) }`; `Account
  { OpenAi, Judge { provider } }` and `Host::account_changed(&self, &Account)` (called for sign-out,
  judge key removed or replaced, and uninstall); `watane_host::judge_origin(text) ->
  Result<String, HostError>`.
- AC2 Every body text of app 19 (19.1 to 19.8), section 3, 6, 7 and the test rows that cited
  `Confirm::agent`, "asked of host, section 15", `unavailable` or a judge-key-removed call now cite
  the host name and section (host 19.4, 19.7, 12.1 as host has them).
- AC3 The judge origin rule of 19.4 and S63 match host 19.4: a lone trailing `/` is accepted, any
  other path, a query or a fragment is refused; the app uses `watane_host::judge_origin` for what
  it shows and stores.
- AC4 The workspace consent words fill {profile} from `ConsentRequest.profile` and {provider} from
  `via`; say what the words are when `profile` is absent.

## Spec
host/TECHNICAL.md 2.1, 12.1, 19.4, 19.7 (read them); app/TECHNICAL.md 3, 6, 7, 15, 19; app/FUNCTIONAL.md.

## Interface
As in AC1; host owns every name there.

## Files
app/TECHNICAL.md, app/FUNCTIONAL.md.

## Changed tests
none (no code).

## Out of scope
Code; host, pkg, dev, spec documents.
