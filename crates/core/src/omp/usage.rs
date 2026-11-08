//! The usage limits of the providers omp is connected to (`requirements/tech#usage`): the reading
//! `omp usage --json` and the parsing of its output into what the Usage dialog shows.
//!
//! Only what the dialog needs is read: per report the `provider`, `fetchedAt` and each limit's
//! `label`, `window.resetsAt`, `amount` and `status`; from `accountsWithoutUsage` and
//! `disabledCredentials` the `provider`. Everything else (the email, account and organisation of
//! `metadata`, the plan, `capacity`, …) has no field here, so it is skipped by the parser and never
//! kept or sent to the interface. Pure code apart from [`read_usage`], which goes through [`Exec`].

use std::path::Path;

use serde::{Deserialize, Deserializer, Serialize};
use ts_rs::TS;

use super::probe::OmpModel;
use super::providers::{Provider, PROVIDERS};
use super::run::{limits, Exec};

/// The reason of a reading whose output is not the JSON Weaver reads.
pub const INVALID_OUTPUT: &str = "the agent did not return valid usage";

/// From this used fraction on, a limit without a status of omp counts as almost used up.
pub const ALMOST_USED_UP_FROM: f64 = 0.8;

/// How used up a limit is: omp's `status` (`ok`, `warning`, `exhausted`), or, without one, the
/// used fraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename = "UsageLimitStatus")]
pub enum LimitStatus {
    Ok,
    /// `warning` in omp.
    AlmostUsedUp,
    /// `exhausted` in omp.
    UsedUp,
}

/// Why a connected provider has no limit to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename = "UsageNoLimits")]
pub enum NoLimits {
    /// `No usage data`: omp reports the account without usage.
    NoUsageData,
    /// `Login expired`: omp turned the credential off.
    LoginExpired,
    /// `Not reported by the agent`: omp says nothing about the provider.
    NotReported,
}

/// One limit of a provider, as omp gives it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UsageLimit {
    /// The name as omp gives it (`Claude 5 Hour`, `Premium Requests`).
    pub label: String,
    /// When the limit resets, in milliseconds since the Unix epoch; `None` when omp gives none.
    pub resets_at: Option<f64>,
    pub used: Option<f64>,
    pub limit: Option<f64>,
    /// `percent`, `requests`, `tokens`, `credits`, `minutes`, `usd` or whatever omp says, in
    /// lower case.
    pub unit: Option<String>,
    /// The used fraction clamped to 0–1; `None` when it cannot be known.
    pub used_fraction: Option<f64>,
    pub status: LimitStatus,
}

/// A connected provider of the fixed list with its limits, or why it has none.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProviderUsage {
    pub id: String,
    /// The short name of the fixed list.
    pub name: String,
    /// The limits of every account of the provider, one after the other.
    pub limits: Vec<UsageLimit>,
    /// Set exactly when `limits` is empty.
    pub no_limits: Option<NoLimits>,
}

/// What `omp usage --json` said about the connected providers.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    /// The oldest `fetchedAt` of the providers shown, in milliseconds since the Unix epoch.
    pub fetched_at: Option<f64>,
    /// The connected providers, in the order of the fixed list.
    pub providers: Vec<ProviderUsage>,
}

impl Usage {
    /// Whether no provider has a limit to show.
    pub fn is_empty(&self) -> bool {
        self.providers.iter().all(|p| p.limits.is_empty())
    }
}

/// The limit closest to being used up among the shown providers: what the summary in the sidebar
/// shows (`requirements/tech#usage`).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummary {
    /// The limit's name as omp gives it.
    pub label: String,
    /// The used fraction clamped to 0–1; `None` when no limit has a known one (then the summary
    /// shows neither a percentage nor a bar).
    pub used_fraction: Option<f64>,
    pub status: LimitStatus,
    /// When the limit resets, in milliseconds since the Unix epoch; `None` when omp gives none.
    pub resets_at: Option<f64>,
}

impl LimitStatus {
    /// 0 for `Ok`, 1 for almost used up, 2 for used up: the worse, the higher.
    fn severity(self) -> u8 {
        match self {
            Self::Ok => 0,
            Self::AlmostUsedUp => 1,
            Self::UsedUp => 2,
        }
    }
}

impl Usage {
    /// The limit closest to being used up: the worst status (used up, then almost used up, then
    /// the others) and, within it, the highest used fraction; ties go to the earliest reset
    /// (a limit without one comes last), then to the order of the providers and of their limits.
    /// Limits with an unknown used fraction count only when no other limit exists. `None` when no
    /// shown provider has a limit.
    pub fn summary(&self) -> Option<UsageSummary> {
        let limits = || self.providers.iter().flat_map(|p| p.limits.iter());
        let known = limits().any(|l| l.used_fraction.is_some());
        limits()
            .filter(|l| !known || l.used_fraction.is_some())
            .min_by(|a, b| {
                b.status
                    .severity()
                    .cmp(&a.status.severity())
                    .then_with(|| {
                        let a = a.used_fraction.unwrap_or(0.0);
                        let b = b.used_fraction.unwrap_or(0.0);
                        b.total_cmp(&a)
                    })
                    .then_with(|| match (a.resets_at, b.resets_at) {
                        (Some(a), Some(b)) => a.total_cmp(&b),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => std::cmp::Ordering::Equal,
                    })
            })
            .map(|l| UsageSummary {
                label: l.label.clone(),
                used_fraction: l.used_fraction,
                status: l.status,
                resets_at: l.resets_at,
            })
    }
}

/// The providers of the fixed list that have a model in `models` (the connected ones), in the
/// order of the list.
pub fn connected_providers(models: &[OmpModel]) -> Vec<&'static Provider> {
    PROVIDERS
        .iter()
        .filter(|p| models.iter().any(|m| m.provider == p.id))
        .collect()
}

// --- the shape of the output ----------------------------------------------------------------------

fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Raw {
    reports: Vec<RawReport>,
    #[serde(default, deserialize_with = "null_as_default")]
    accounts_without_usage: Vec<RawProviderRef>,
    #[serde(default, deserialize_with = "null_as_default")]
    disabled_credentials: Vec<RawProviderRef>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawReport {
    provider: Option<String>,
    fetched_at: Option<f64>,
    #[serde(default, deserialize_with = "null_as_default")]
    limits: Vec<RawLimit>,
}

#[derive(Deserialize)]
struct RawLimit {
    label: Option<String>,
    window: Option<RawWindow>,
    amount: Option<RawAmount>,
    status: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawWindow {
    resets_at: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAmount {
    used: Option<f64>,
    limit: Option<f64>,
    used_fraction: Option<f64>,
    remaining_fraction: Option<f64>,
    unit: Option<String>,
}

/// An entry of `accountsWithoutUsage` or `disabledCredentials`: an object with a `provider`
/// (a bare id is accepted too); only that is read.
#[derive(Deserialize)]
#[serde(untagged)]
enum RawProviderRef {
    Id(String),
    Entry { provider: Option<String> },
}

impl RawProviderRef {
    fn provider(&self) -> Option<&str> {
        match self {
            Self::Id(id) => Some(id),
            Self::Entry { provider } => provider.as_deref(),
        }
    }
}

// --- the computations ------------------------------------------------------------------------------

fn finite(value: Option<f64>) -> Option<f64> {
    value.filter(|v| v.is_finite())
}

/// The used fraction of an amount, clamped to 0–1: `usedFraction`, else `used / limit` when `limit`
/// is above 0, else `used / 100` when the unit is `percent`, else `1 − remainingFraction`, else
/// unknown.
pub fn used_fraction(
    used: Option<f64>,
    limit: Option<f64>,
    used_fraction: Option<f64>,
    remaining_fraction: Option<f64>,
    unit: Option<&str>,
) -> Option<f64> {
    let (used, limit) = (finite(used), finite(limit));
    let fraction = finite(used_fraction)
        .or_else(|| {
            used.zip(limit)
                .filter(|(_, l)| *l > 0.0)
                .map(|(u, l)| u / l)
        })
        .or_else(|| {
            used.filter(|_| unit.is_some_and(|u| u.eq_ignore_ascii_case("percent")))
                .map(|u| u / 100.0)
        })
        .or_else(|| finite(remaining_fraction).map(|r| 1.0 - r));
    fraction.map(|f| f.clamp(0.0, 1.0))
}

/// The status of a limit: omp's `status` when it is one of `ok`, `warning` and `exhausted`;
/// without it, almost used up from [`ALMOST_USED_UP_FROM`] and used up from 1 of the used fraction.
pub fn limit_status(status: Option<&str>, used_fraction: Option<f64>) -> LimitStatus {
    match status.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        Some("ok") => LimitStatus::Ok,
        Some("warning") => LimitStatus::AlmostUsedUp,
        Some("exhausted") => LimitStatus::UsedUp,
        _ => match used_fraction {
            Some(f) if f >= 1.0 => LimitStatus::UsedUp,
            Some(f) if f >= ALMOST_USED_UP_FROM => LimitStatus::AlmostUsedUp,
            _ => LimitStatus::Ok,
        },
    }
}

fn limit_of(raw: RawLimit) -> UsageLimit {
    let amount = raw.amount.unwrap_or(RawAmount {
        used: None,
        limit: None,
        used_fraction: None,
        remaining_fraction: None,
        unit: None,
    });
    let unit = amount
        .unit
        .map(|u| u.trim().to_ascii_lowercase())
        .filter(|u| !u.is_empty());
    let fraction = used_fraction(
        amount.used,
        amount.limit,
        amount.used_fraction,
        amount.remaining_fraction,
        unit.as_deref(),
    );
    UsageLimit {
        label: raw
            .label
            .map(|l| l.trim().to_owned())
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| "Usage limit".to_owned()),
        resets_at: raw.window.and_then(|w| finite(w.resets_at)),
        used: finite(amount.used),
        limit: finite(amount.limit),
        unit,
        used_fraction: fraction,
        status: limit_status(raw.status.as_deref(), fraction),
    }
}

/// Parses the output of `omp usage --json` for the `connected` providers. Providers that are not
/// connected, or not in the fixed list, are dropped. `None` when the text is not JSON of the shape
/// `{"reports": [...]}`.
pub fn parse_usage(stdout: &str, connected: &[&Provider]) -> Option<Usage> {
    let raw: Raw = serde_json::from_str(stdout.trim()).ok()?;
    let mentions =
        |refs: &[RawProviderRef], id: &str| refs.iter().any(|r| r.provider() == Some(id));
    let mut fetched_at: Option<f64> = None;
    let mut providers = Vec::new();
    // `connected` is in the order of the fixed list; the list decides the order, not omp.
    let mut reports: Vec<RawReport> = raw.reports;
    for provider in PROVIDERS
        .iter()
        .filter(|p| connected.iter().any(|c| c.id == p.id))
    {
        let mut limits = Vec::new();
        let mut rest = Vec::with_capacity(reports.len());
        for report in reports {
            if report.provider.as_deref() == Some(provider.id) {
                if let Some(at) = finite(report.fetched_at) {
                    fetched_at = Some(fetched_at.map_or(at, |oldest| oldest.min(at)));
                }
                limits.extend(report.limits.into_iter().map(limit_of));
            } else {
                rest.push(report);
            }
        }
        reports = rest;
        let no_limits = limits.is_empty().then(|| {
            if mentions(&raw.accounts_without_usage, provider.id) {
                NoLimits::NoUsageData
            } else if mentions(&raw.disabled_credentials, provider.id) {
                NoLimits::LoginExpired
            } else {
                NoLimits::NotReported
            }
        });
        providers.push(ProviderUsage {
            id: provider.id.to_owned(),
            name: provider.name.to_owned(),
            limits,
            no_limits,
        });
    }
    Some(Usage {
        fetched_at,
        providers,
    })
}

/// `omp usage --json` (30 s) for the `connected` providers. On failure, its reason: the explanation
/// of a proxy problem, else the last line of stderr (`requirements/tech#omp`, "Failure reasons"),
/// chosen with the setting as it is now; [`INVALID_OUTPUT`] when the output is not understood.
/// Never `invalidate`, `reset` or `--history`: omp's own cache is relied on.
pub fn read_usage(omp: &Path, connected: &[&Provider], exec: &dyn Exec) -> Result<Usage, String> {
    let output = exec
        .run(omp, &["usage", "--json"], limits::USAGE)
        .map_err(|failure| failure.reason_for(exec.system_certificates_on()))?;
    parse_usage(&output.stdout, connected).ok_or_else(|| INVALID_OUTPUT.to_owned())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::omp::probe::fake::FakeOmp;
    use crate::omp::providers::provider;
    use crate::process::timed::Failure;

    /// The shape of the output of omp 18.16.0, with made-up identities.
    const SAMPLE: &str = r#"{
      "generatedAt": 1793655088513,
      "reports": [
        {
          "provider": "anthropic",
          "fetchedAt": 1793655087331,
          "limits": [
            {
              "id": "anthropic:5h",
              "label": "Claude 5 Hour",
              "scope": {"provider": "anthropic", "windowId": "5h", "shared": true},
              "window": {"id": "5h", "label": "5 Hour", "durationMs": 18000000, "resetsAt": 1793672400229},
              "amount": {"used": 17, "limit": 100, "remaining": 83, "usedFraction": 0.17, "remainingFraction": 0.83, "unit": "percent"},
              "status": "ok"
            },
            {
              "id": "anthropic:7d",
              "label": "Claude 7 Day",
              "window": {"id": "7d", "resetsAt": 1794106800229},
              "amount": {"used": 85, "limit": 100, "unit": "percent"}
            }
          ],
          "metadata": {
            "endpoint": "https://api.anthropic.com/api/oauth/usage",
            "accountId": "acct-123",
            "email": "someone@example.com",
            "orgId": "org-456",
            "orgName": "Someone's Organization"
          },
          "resetCredits": {"availableCount": 0, "credits": []}
        },
        {
          "provider": "github-copilot",
          "fetchedAt": 1793655088512,
          "limits": [
            {
              "id": "copilot:premium",
              "label": "Premium Requests",
              "scope": {"provider": "github-copilot", "accountId": "ACCOUNT-9", "tier": "enterprise"},
              "window": {"id": "monthly", "label": "Monthly", "resetsAt": 1796083200000},
              "amount": {"used": 3437, "limit": 100000, "remaining": 96563, "usedFraction": 0.03437, "remainingFraction": 0.96563, "unit": "requests"},
              "status": "ok"
            }
          ],
          "metadata": {"accountId": "ACCOUNT-9", "plan": "enterprise", "quotaResetDate": "2026-12-01"}
        },
        {
          "provider": "cursor",
          "fetchedAt": 1793655087026,
          "limits": [
            {
              "id": "cursor:usd:individual-overall",
              "label": "Personal Usage",
              "window": {"id": "monthly", "label": "Monthly", "resetsAt": 1796083200000},
              "amount": {"used": 0.5, "limit": 1000, "remaining": 999.5, "usedFraction": 0.0005, "remainingFraction": 0.9995, "unit": "usd"},
              "status": "ok"
            }
          ],
          "metadata": {"email": "other@example.com"}
        },
        {
          "provider": "some-other-provider",
          "fetchedAt": 1000,
          "limits": [{"label": "Not ours", "amount": {"used": 1, "limit": 2, "unit": "tokens"}}]
        }
      ],
      "accountsWithoutUsage": [{"provider": "google-gemini-cli", "email": "gemini@example.com"}],
      "disabledCredentials": [{"provider": "xai-oauth", "accountId": "xai-1"}],
      "capacity": {"anthropic": [{"window": "5h", "accounts": 1}]}
    }"#;

    fn providers(ids: &[&str]) -> Vec<&'static Provider> {
        ids.iter().map(|id| provider(id).unwrap()).collect()
    }

    fn model(provider: &str) -> OmpModel {
        OmpModel {
            provider: provider.into(),
            selector: format!("{provider}/m"),
            name: None,
            thinking_levels: vec![],
        }
    }

    fn limit_of_amount(amount: &str, status: Option<&str>) -> UsageLimit {
        let status = status.map_or(String::new(), |s| format!(r#","status":"{s}""#));
        let text = format!(
            r#"{{"reports":[{{"provider":"anthropic","limits":[{{"label":"L","amount":{amount}{status}}}]}}]}}"#
        );
        parse_usage(&text, &providers(&["anthropic"]))
            .unwrap()
            .providers
            .remove(0)
            .limits
            .remove(0)
    }

    #[test]
    fn parses_the_limits_of_the_connected_providers_in_the_order_of_the_list() {
        let connected = providers(&["cursor", "anthropic", "github-copilot"]);
        let usage = parse_usage(SAMPLE, &connected).unwrap();

        let ids: Vec<&str> = usage.providers.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["anthropic", "github-copilot", "cursor"]);
        assert_eq!(usage.providers[0].name, "Anthropic");

        let anthropic = &usage.providers[0].limits;
        assert_eq!(anthropic.len(), 2);
        assert_eq!(anthropic[0].label, "Claude 5 Hour");
        assert_eq!(anthropic[0].used, Some(17.0));
        assert_eq!(anthropic[0].limit, Some(100.0));
        assert_eq!(anthropic[0].unit.as_deref(), Some("percent"));
        assert_eq!(anthropic[0].used_fraction, Some(0.17));
        assert_eq!(anthropic[0].resets_at, Some(1793672400229.0));
        assert_eq!(anthropic[0].status, LimitStatus::Ok);
        // No `usedFraction` and no status: 85/100, so almost used up.
        assert_eq!(anthropic[1].label, "Claude 7 Day");
        assert_eq!(anthropic[1].used_fraction, Some(0.85));
        assert_eq!(anthropic[1].status, LimitStatus::AlmostUsedUp);

        let copilot = &usage.providers[1].limits[0];
        assert_eq!(copilot.label, "Premium Requests");
        assert_eq!(copilot.used, Some(3437.0));
        assert_eq!(copilot.limit, Some(100000.0));
        assert_eq!(copilot.unit.as_deref(), Some("requests"));

        let cursor = &usage.providers[2].limits[0];
        assert_eq!(cursor.label, "Personal Usage");
        assert_eq!(cursor.used, Some(0.5));
        assert_eq!(cursor.unit.as_deref(), Some("usd"));
    }

    #[test]
    fn the_time_of_the_data_is_the_oldest_fetch_of_the_shown_providers() {
        let usage = parse_usage(SAMPLE, &providers(&["anthropic", "cursor"])).unwrap();
        assert_eq!(usage.fetched_at, Some(1793655087026.0));
        let only = parse_usage(SAMPLE, &providers(&["github-copilot"])).unwrap();
        assert_eq!(only.fetched_at, Some(1793655088512.0));
        // A provider of another list is dropped: its old fetch never counts.
        assert!(parse_usage(SAMPLE, &providers(&["anthropic"]))
            .unwrap()
            .fetched_at
            .is_some_and(|at| at > 1_000_000.0));
    }

    #[test]
    fn a_connected_provider_without_limits_says_why() {
        let connected = providers(&[
            "anthropic",
            "google-gemini-cli",
            "xai-oauth",
            "openai-codex",
        ]);
        let usage = parse_usage(SAMPLE, &connected).unwrap();
        let by_id = |id: &str| usage.providers.iter().find(|p| p.id == id).unwrap();
        assert_eq!(by_id("anthropic").no_limits, None);
        assert_eq!(
            by_id("google-gemini-cli").no_limits,
            Some(NoLimits::NoUsageData)
        );
        assert_eq!(by_id("xai-oauth").no_limits, Some(NoLimits::LoginExpired));
        assert_eq!(by_id("openai-codex").no_limits, Some(NoLimits::NotReported));
        assert!(!usage.is_empty());
    }

    #[test]
    fn no_provider_with_limits_is_an_empty_usage() {
        let usage = parse_usage(SAMPLE, &providers(&["openai-codex"])).unwrap();
        assert!(usage.is_empty());
        assert_eq!(usage.fetched_at, None);
        assert!(parse_usage(SAMPLE, &[]).unwrap().providers.is_empty());
    }

    #[test]
    fn a_report_of_a_provider_that_is_not_connected_or_not_listed_is_dropped() {
        let usage = parse_usage(SAMPLE, &providers(&["anthropic"])).unwrap();
        assert_eq!(usage.providers.len(), 1);
        let text = serde_json::to_string(&usage.providers).unwrap();
        assert!(!text.contains("Not ours"));
        assert!(!text.contains("Premium Requests"));
    }

    #[test]
    fn several_accounts_of_a_provider_follow_one_another_in_one_block() {
        let text = r#"{"reports":[
            {"provider":"anthropic","fetchedAt":20,"limits":[{"label":"Claude 5 Hour","amount":{"used":1,"limit":100,"unit":"percent"}}]},
            {"provider":"cursor","fetchedAt":5,"limits":[{"label":"Personal Usage","amount":{"used":1,"limit":10,"unit":"usd"}}]},
            {"provider":"anthropic","fetchedAt":10,"limits":[{"label":"Claude 5 Hour","amount":{"used":50,"limit":100,"unit":"percent"}}]}
        ]}"#;
        let usage = parse_usage(text, &providers(&["anthropic", "cursor"])).unwrap();
        assert_eq!(usage.providers.len(), 2);
        let anthropic = &usage.providers[0].limits;
        assert_eq!(anthropic.len(), 2);
        assert_eq!(anthropic[0].used, Some(1.0));
        assert_eq!(anthropic[1].used, Some(50.0));
        assert_eq!(usage.fetched_at, Some(5.0));
    }

    #[test]
    fn nothing_that_identifies_an_account_is_kept() {
        let usage = parse_usage(
            SAMPLE,
            &providers(&["anthropic", "github-copilot", "cursor"]),
        )
        .unwrap();
        let text = format!("{usage:?}{}", serde_json::to_string(&usage).unwrap());
        for identity in [
            "someone@example.com",
            "other@example.com",
            "gemini@example.com",
            "acct-123",
            "org-456",
            "Organization",
            "ACCOUNT-9",
            "enterprise",
            "xai-1",
        ] {
            assert!(!text.contains(identity), "{identity} leaked");
        }
    }

    #[test]
    fn the_used_fraction_follows_its_precedence() {
        let f = used_fraction;
        // `usedFraction` first.
        assert_eq!(
            f(Some(1.0), Some(10.0), Some(0.5), Some(0.0), Some("usd")),
            Some(0.5)
        );
        // Then used / limit, only for a limit above 0.
        assert_eq!(f(Some(1.0), Some(4.0), None, None, None), Some(0.25));
        assert_eq!(f(Some(1.0), Some(0.0), None, None, Some("requests")), None);
        // Then used / 100 for percent.
        assert_eq!(
            f(Some(17.0), Some(0.0), None, None, Some("percent")),
            Some(0.17)
        );
        assert_eq!(f(Some(17.0), None, None, None, Some("PERCENT")), Some(0.17));
        // Then 1 - remainingFraction.
        assert!(f(None, None, None, Some(0.75), Some("requests"))
            .is_some_and(|v| (v - 0.25).abs() < 1e-9));
        // Else unknown.
        assert_eq!(f(Some(3.0), None, None, None, Some("requests")), None);
        assert_eq!(f(None, None, None, None, None), None);
    }

    #[test]
    fn the_used_fraction_is_clamped_to_zero_and_one() {
        assert_eq!(
            used_fraction(Some(150.0), Some(100.0), None, None, None),
            Some(1.0)
        );
        assert_eq!(used_fraction(None, None, Some(-0.2), None, None), Some(0.0));
        assert_eq!(used_fraction(None, None, None, Some(1.5), None), Some(0.0));
        assert_eq!(used_fraction(None, None, None, Some(-1.0), None), Some(1.0));
    }

    #[test]
    fn a_fraction_of_a_limit_is_taken_from_its_amount() {
        let l = limit_of_amount(r#"{"used":2,"limit":8,"unit":"tokens"}"#, None);
        assert_eq!(l.used_fraction, Some(0.25));
        let l = limit_of_amount(r#"{"used":2,"unit":"requests"}"#, None);
        assert_eq!(l.used_fraction, None);
        assert_eq!(l.status, LimitStatus::Ok);
    }

    #[test]
    fn the_status_is_omps_or_comes_from_the_fraction() {
        assert_eq!(limit_status(Some("ok"), Some(0.99)), LimitStatus::Ok);
        assert_eq!(
            limit_status(Some("warning"), Some(0.1)),
            LimitStatus::AlmostUsedUp
        );
        assert_eq!(
            limit_status(Some("exhausted"), Some(0.1)),
            LimitStatus::UsedUp
        );
        assert_eq!(limit_status(None, Some(0.79)), LimitStatus::Ok);
        assert_eq!(limit_status(None, Some(0.8)), LimitStatus::AlmostUsedUp);
        assert_eq!(limit_status(None, Some(0.99)), LimitStatus::AlmostUsedUp);
        assert_eq!(limit_status(None, Some(1.0)), LimitStatus::UsedUp);
        assert_eq!(limit_status(None, None), LimitStatus::Ok);
        // A status omp does not use is as good as none.
        assert_eq!(limit_status(Some("???"), Some(1.0)), LimitStatus::UsedUp);
        let l = limit_of_amount(
            r#"{"used":100,"limit":100,"unit":"percent"}"#,
            Some("warning"),
        );
        assert_eq!(l.status, LimitStatus::AlmostUsedUp);
    }

    #[test]
    fn output_that_is_not_the_expected_json_is_refused() {
        let connected = providers(&["anthropic"]);
        for text in [
            "",
            "usage unavailable",
            "[]",
            "{}",
            r#"{"reports":"x"}"#,
            r#"{"reports":null}"#,
        ] {
            assert_eq!(parse_usage(text, &connected), None, "{text:?}");
        }
        assert!(parse_usage(r#"{"reports":[]}"#, &connected).is_some());
        // Unknown fields and a report without limits are fine.
        let usage = parse_usage(
            r#"{"reports":[{"provider":"anthropic","limits":null,"future":1}],"extra":[]}"#,
            &connected,
        )
        .unwrap();
        assert_eq!(usage.providers[0].no_limits, Some(NoLimits::NotReported));
    }

    #[test]
    fn the_connected_providers_are_those_with_a_model_in_the_order_of_the_list() {
        let models = [
            model("cursor"),
            model("anthropic"),
            model("not-listed"),
            model("anthropic"),
        ];
        let ids: Vec<&str> = connected_providers(&models).iter().map(|p| p.id).collect();
        assert_eq!(ids, ["anthropic", "cursor"]);
        assert!(connected_providers(&[]).is_empty());
    }

    fn limit(
        label: &str,
        fraction: Option<f64>,
        status: LimitStatus,
        resets: Option<f64>,
    ) -> UsageLimit {
        UsageLimit {
            label: label.into(),
            resets_at: resets,
            used: None,
            limit: None,
            unit: None,
            used_fraction: fraction,
            status,
        }
    }

    fn usage_of(providers: Vec<Vec<UsageLimit>>) -> Usage {
        Usage {
            fetched_at: None,
            providers: providers
                .into_iter()
                .enumerate()
                .map(|(i, limits)| ProviderUsage {
                    id: format!("p{i}"),
                    name: format!("P{i}"),
                    no_limits: limits.is_empty().then_some(NoLimits::NotReported),
                    limits,
                })
                .collect(),
        }
    }

    fn summary_label(usage: &Usage) -> Option<String> {
        usage.summary().map(|s| s.label)
    }

    #[test]
    fn the_summary_is_none_without_any_limit() {
        assert_eq!(usage_of(vec![]).summary(), None);
        assert_eq!(usage_of(vec![vec![], vec![]]).summary(), None);
    }

    #[test]
    fn the_summary_carries_the_limit_with_its_fraction_status_and_reset() {
        let usage = usage_of(vec![vec![limit(
            "Claude 5 Hour",
            Some(0.17),
            LimitStatus::Ok,
            Some(5.0),
        )]]);
        assert_eq!(
            usage.summary(),
            Some(UsageSummary {
                label: "Claude 5 Hour".into(),
                used_fraction: Some(0.17),
                status: LimitStatus::Ok,
                resets_at: Some(5.0),
            })
        );
    }

    #[test]
    fn the_worst_status_wins_before_the_fraction() {
        let usage = usage_of(vec![vec![
            limit("high but ok", Some(0.79), LimitStatus::Ok, None),
            limit("warning", Some(0.5), LimitStatus::AlmostUsedUp, None),
        ]]);
        assert_eq!(summary_label(&usage).as_deref(), Some("warning"));
        let usage = usage_of(vec![vec![
            limit("warning", Some(0.99), LimitStatus::AlmostUsedUp, None),
            limit("exhausted", Some(0.2), LimitStatus::UsedUp, None),
        ]]);
        assert_eq!(summary_label(&usage).as_deref(), Some("exhausted"));
    }

    #[test]
    fn within_a_status_the_highest_fraction_wins() {
        let usage = usage_of(vec![
            vec![limit("a", Some(0.2), LimitStatus::Ok, None)],
            vec![limit("b", Some(0.6), LimitStatus::Ok, None)],
        ]);
        assert_eq!(summary_label(&usage).as_deref(), Some("b"));
    }

    #[test]
    fn ties_go_to_the_earliest_reset_then_to_the_order_of_providers_and_limits() {
        let usage = usage_of(vec![
            vec![limit("late", Some(0.5), LimitStatus::Ok, Some(20.0))],
            vec![limit("early", Some(0.5), LimitStatus::Ok, Some(10.0))],
        ]);
        assert_eq!(summary_label(&usage).as_deref(), Some("early"));
        // A limit without a reset comes after one with a reset.
        let usage = usage_of(vec![vec![
            limit("none", Some(0.5), LimitStatus::Ok, None),
            limit("some", Some(0.5), LimitStatus::Ok, Some(10.0)),
        ]]);
        assert_eq!(summary_label(&usage).as_deref(), Some("some"));
        // Everything equal: the first limit of the first provider.
        let usage = usage_of(vec![
            vec![
                limit("first", Some(0.5), LimitStatus::Ok, Some(10.0)),
                limit("second", Some(0.5), LimitStatus::Ok, Some(10.0)),
            ],
            vec![limit("third", Some(0.5), LimitStatus::Ok, Some(10.0))],
        ]);
        assert_eq!(summary_label(&usage).as_deref(), Some("first"));
    }

    #[test]
    fn limits_with_an_unknown_fraction_count_only_when_no_other_limit_exists() {
        let usage = usage_of(vec![
            vec![limit("unknown", None, LimitStatus::UsedUp, Some(1.0))],
            vec![limit("known", Some(0.1), LimitStatus::Ok, None)],
        ]);
        assert_eq!(summary_label(&usage).as_deref(), Some("known"));

        let usage = usage_of(vec![vec![
            limit("first", None, LimitStatus::Ok, Some(30.0)),
            limit("second", None, LimitStatus::Ok, Some(20.0)),
        ]]);
        let summary = usage.summary().unwrap();
        assert_eq!(summary.label, "second");
        assert_eq!(summary.used_fraction, None);
    }

    #[test]
    fn reading_runs_only_omp_usage_json_with_a_limit_of_30_seconds() {
        let exec = FakeOmp::new().on(&["usage", "--json"], FakeOmp::ok(SAMPLE));
        let usage = read_usage(Path::new("/opt/omp"), &providers(&["anthropic"]), &exec).unwrap();
        assert_eq!(usage.providers[0].id, "anthropic");
        let calls = exec.calls.lock().unwrap();
        assert_eq!(
            *calls,
            [(
                vec!["usage".to_owned(), "--json".to_owned()],
                Duration::from_secs(30)
            )]
        );
    }

    #[test]
    fn reading_failures_say_why() {
        let connected = providers(&["anthropic"]);
        let omp = Path::new("/opt/omp");
        let run = |answer| {
            let exec = FakeOmp::new().on(&["usage", "--json"], answer);
            read_usage(omp, &connected, &exec)
        };
        assert_eq!(
            run(Err(Failure::Exit {
                code: Some(1),
                stderr: "boom\nnetwork down\n".into()
            })),
            Err("network down".to_owned())
        );
        assert_eq!(
            run(Err(Failure::Exit {
                code: Some(3),
                stderr: String::new()
            })),
            Err("exit code 3".to_owned())
        );
        assert_eq!(
            run(Err(Failure::NotStarted)),
            Err("could not start the agent".to_owned())
        );
        assert_eq!(
            run(Err(Failure::TimedOut {
                limit: Duration::from_secs(30)
            })),
            Err("timed out".to_owned())
        );
        assert_eq!(run(FakeOmp::ok("not json")), Err(INVALID_OUTPUT.to_owned()));
    }

    #[test]
    fn a_certificate_error_is_replaced_by_its_explanation() {
        let failure = || {
            Err(Failure::Exit {
                code: Some(1),
                stderr: "fetch failed: UNABLE_TO_GET_ISSUER_CERT_LOCALLY\n".into(),
            })
        };
        let connected = providers(&["anthropic"]);
        let omp = Path::new("/opt/omp");
        let off = read_usage(
            omp,
            &connected,
            &FakeOmp::new().on(&["usage", "--json"], failure()),
        )
        .unwrap_err();
        assert!(off.starts_with("the agent could not verify a server's certificate"));
        assert!(off.contains("turning on System Certificates"));
        let on = read_usage(
            omp,
            &connected,
            &FakeOmp::new()
                .certificates_on()
                .on(&["usage", "--json"], failure()),
        )
        .unwrap_err();
        assert!(on.contains("even with System Certificates on"));
    }
}
