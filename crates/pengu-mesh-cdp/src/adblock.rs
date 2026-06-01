use serde_json::{Value, json};
use std::collections::HashSet;

const REQUEST_STAGE_REQUEST: &str = "Request";
const BLOCKED_BY_CLIENT_REASON: &str = "BlockedByClient";

/// Default ad/tracker domain patterns based on PinchTab's upstream block list.
pub const DEFAULT_BLOCK_PATTERNS: &[&str] = &[
    // Analytics & tracking
    "*google-analytics.com/*",
    "*googletagmanager.com/*",
    "*googletagservices.com/*",
    "*googlesyndication.com/*",
    "*googleadservices.com/*",
    "*doubleclick.net/*",
    "*facebook.com/tr/*",
    "*facebook.com/plugins/*",
    "*connect.facebook.net/*",
    "*fbcdn.net/*/fbevents.js",
    "*twitter.com/i/adsct",
    "*analytics.twitter.com/*",
    "*static.ads-twitter.com/*",
    "*amazon-adsystem.com/*",
    "*amazontrust.com/*",
    "*adsafeprotected.com/*",
    "*segment.io/*",
    "*segment.com/*",
    "*mixpanel.com/*",
    "*amplitude.com/*",
    "*mxpnl.com/*",
    "*kissmetrics.com/*",
    "*hotjar.com/*",
    "*fullstory.com/*",
    "*heapanalytics.com/*",
    "*mouseflow.com/*",
    "*luckyorange.com/*",
    "*crazyegg.com/*",
    "*pingdom.net/*",
    "*newrelic.com/*",
    "*nr-data.net/*",
    // Ad networks
    "*adnxs.com/*",
    "*adsymptotic.com/*",
    "*openx.net/*",
    "*pubmatic.com/*",
    "*rubiconproject.com/*",
    "*adsrvr.org/*",
    "*media.net/*",
    "*adtech.de/*",
    "*adzerk.net/*",
    "*criteo.com/*",
    "*criteo.net/*",
    "*casalemedia.com/*",
    "*33across.com/*",
    "*taboola.com/*",
    "*outbrain.com/*",
    "*revcontent.com/*",
    "*zemanta.com/*",
    "*disqus.com/ads/*",
    // Marketing automation
    "*marketo.com/*",
    "*marketo.net/*",
    "*hubspot.com/analytics/*",
    "*pardot.com/*",
    "*leadfeeder.com/*",
    "*clickcease.com/*",
    "*leadforensics.com/*",
    // Social media widgets
    "*platform.twitter.com/widgets/*",
    "*platform.instagram.com/widgets/*",
    "*platform.linkedin.com/widgets/*",
    "*pinterest.com/js/pinit.js",
    // Common trackers
    "*scorecardresearch.com/*",
    "*quantserve.com/*",
    "*quantcount.com/*",
    "*parsely.com/*",
    "*chartbeat.com/*",
    "*omtrdc.net/*",
    "*optimizely.com/*",
    "*visualwebsiteoptimizer.com/*",
    "*demdex.net/*",
    "*bluekai.com/*",
    "*addthis.com/*",
    "*sharethis.com/*",
    // Cookie consent overlays
    "*cookielaw.org/*",
    "*cookiebot.com/*",
    "*onetrust.com/*",
    "*trustarc.com/*",
    "*usercentrics.com/*",
    // Misc tracking pixels
    "*pixel.gif*",
    "*tracking.gif*",
    "*analytics.gif*",
    "*/tr?*",
    "*/pixel?*",
    "*/collect?*",
];

/// Returns the default PinchTab-derived ad-block patterns.
pub fn default_block_patterns() -> &'static [&'static str] {
    DEFAULT_BLOCK_PATTERNS
}

/// Combines multiple block-pattern lists while preserving first-seen order and
/// removing duplicates.
pub fn combine_block_patterns<I, J, S>(pattern_lists: I) -> Vec<String>
where
    I: IntoIterator<Item = J>,
    J: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut combined = Vec::new();
    let mut seen = HashSet::new();

    for pattern_list in pattern_lists {
        for pattern in pattern_list {
            let pattern = pattern.as_ref();
            if seen.insert(pattern.to_owned()) {
                combined.push(pattern.to_owned());
            }
        }
    }

    combined
}

/// A set of URL patterns used to block ad and tracker network requests via CDP
/// `Fetch.enable`.
#[derive(Debug, Clone)]
pub struct BlockList {
    patterns: Vec<String>,
}

impl Default for BlockList {
    fn default() -> Self {
        Self {
            patterns: combine_block_patterns([DEFAULT_BLOCK_PATTERNS]),
        }
    }
}

impl BlockList {
    /// Create a block list from custom URL patterns.
    pub fn with_patterns(patterns: Vec<String>) -> Self {
        Self {
            patterns: combine_block_patterns([patterns.iter().map(String::as_str)]),
        }
    }

    /// Returns the currently configured URL patterns.
    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }

    /// Returns `true` if `url` matches any pattern in the block list.
    ///
    /// Patterns use simple glob matching where `*` matches zero or more
    /// characters.
    pub fn matches(&self, url: &str) -> bool {
        self.patterns.iter().any(|pat| glob_match(pat, url))
    }

    /// Build CDP `Fetch.requestPattern` objects suitable for use in a
    /// `Fetch.enable` call.
    pub fn to_fetch_patterns(&self) -> Vec<Value> {
        self.patterns
            .iter()
            .map(|pat| {
                json!({
                    "urlPattern": pat,
                    "requestStage": REQUEST_STAGE_REQUEST,
                })
            })
            .collect()
    }

    /// Builds a raw CDP `Fetch.enable` message using the current block list.
    pub fn fetch_enable_message(&self) -> Value {
        json!({
            "method": "Fetch.enable",
            "params": {
                "patterns": self.to_fetch_patterns(),
            }
        })
    }
}

/// Minimal glob matcher: `*` matches zero or more arbitrary characters.
fn glob_match(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == text;
    }

    let mut pos = 0usize;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        match text[pos..].find(part) {
            Some(offset) => {
                // The first segment must anchor at the start if the pattern
                // doesn't begin with `*`.
                if i == 0 && offset != 0 {
                    return false;
                }
                pos += offset + part.len();
            }
            None => return false,
        }
    }

    // If the pattern doesn't end with `*`, the text must end exactly after the
    // last segment.
    if !pattern.ends_with('*') {
        return pos == text.len();
    }

    true
}

/// Builds a raw CDP `Fetch.continueRequest` message for an intercepted request.
pub fn continue_request_message(request_id: &str) -> Value {
    json!({
        "method": "Fetch.continueRequest",
        "params": {
            "requestId": request_id,
        }
    })
}

/// Builds a raw CDP `Fetch.failRequest` message that blocks an intercepted
/// request as client-side filtering.
pub fn fail_request_message(request_id: &str) -> Value {
    json!({
        "method": "Fetch.failRequest",
        "params": {
            "requestId": request_id,
            "errorReason": BLOCKED_BY_CLIENT_REASON,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_block_list_ports_upstream_pattern_coverage() {
        let bl = BlockList::default();
        assert!(default_block_patterns().len() >= 70);
        assert_eq!(bl.patterns().len(), default_block_patterns().len());
        for essential in [
            "*google-analytics.com/*",
            "*doubleclick.net/*",
            "*segment.io/*",
            "*trustarc.com/*",
            "*/collect?*",
        ] {
            assert!(bl.patterns().iter().any(|pattern| pattern == essential));
        }
    }

    #[test]
    fn matches_ad_urls() {
        let bl = BlockList::default();
        assert!(bl.matches("https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js"));
        assert!(bl.matches("https://www.googleadservices.com/pagead/conversion/12345"));
        assert!(bl.matches("https://ad.doubleclick.net/ddm/track"));
        assert!(bl.matches("https://www.facebook.com/tr/?ev=PageView"));
        assert!(bl.matches("https://www.google-analytics.com/analytics.js"));
        assert!(bl.matches("https://cdn.onetrust.com/scripttemplates/otSDKStub.js"));
        assert!(bl.matches("https://cdn.trustarc.com/notice?domain=example.com"));
        assert!(bl.matches("https://cdn.example.com/pixel.gif?campaign=123"));
    }

    #[test]
    fn does_not_match_normal_urls() {
        let bl = BlockList::default();
        assert!(!bl.matches("https://example.com/index.html"));
        assert!(!bl.matches("https://www.google.com/search?q=rust"));
        assert!(!bl.matches("https://docs.rs/serde_json"));
    }

    #[test]
    fn custom_patterns() {
        let bl = BlockList::with_patterns(vec!["*evil.example.com*".to_owned()]);
        assert!(bl.matches("https://evil.example.com/tracker.js"));
        assert!(!bl.matches("https://good.example.com/page"));
    }

    #[test]
    fn custom_patterns_are_deduplicated() {
        let bl = BlockList::with_patterns(vec![
            "*evil.example.com*".to_owned(),
            "*evil.example.com*".to_owned(),
            "*tracking.example.com*".to_owned(),
        ]);
        assert_eq!(
            bl.patterns(),
            &[
                "*evil.example.com*".to_owned(),
                "*tracking.example.com*".to_owned(),
            ]
        );
    }

    #[test]
    fn combine_block_patterns_preserves_first_seen_order() {
        let combined = combine_block_patterns([
            &["*.jpg", "*.png", "*.gif"][..],
            &["*.mp4", "*.png", "*.webm"][..],
            &["*.pdf", "*.gif"][..],
        ]);
        assert_eq!(
            combined,
            vec![
                "*.jpg".to_owned(),
                "*.png".to_owned(),
                "*.gif".to_owned(),
                "*.mp4".to_owned(),
                "*.webm".to_owned(),
                "*.pdf".to_owned(),
            ]
        );
    }

    #[test]
    fn glob_match_edge_cases() {
        assert!(glob_match("*", "anything"));
        assert!(glob_match("*foo*", "xfoox"));
        assert!(!glob_match("foo", "foobar"));
        assert!(glob_match("foo*", "foobar"));
        assert!(!glob_match("*foo", "foobar"));
        assert!(glob_match("*foo", "barfoo"));
        assert!(glob_match("*/collect?*", "https://example.com/collect?x=1"));
        assert!(!glob_match("*/collect?*", "https://example.com/collector"));
    }

    #[test]
    fn to_fetch_patterns_format() {
        let bl = BlockList::with_patterns(vec!["*ads.example.com*".to_owned()]);
        let patterns = bl.to_fetch_patterns();
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0]["urlPattern"], "*ads.example.com*");
        assert_eq!(patterns[0]["requestStage"], REQUEST_STAGE_REQUEST);
    }

    #[test]
    fn fetch_enable_message_has_expected_shape() {
        let bl = BlockList::with_patterns(vec!["*ads.example.com*".to_owned()]);
        let message = bl.fetch_enable_message();
        assert_eq!(message["method"], "Fetch.enable");
        assert_eq!(
            message["params"]["patterns"][0]["urlPattern"],
            "*ads.example.com*"
        );
        assert_eq!(
            message["params"]["patterns"][0]["requestStage"],
            REQUEST_STAGE_REQUEST
        );
    }

    #[test]
    fn request_control_messages_match_cdp_contract() {
        let continue_message = continue_request_message("request-1");
        assert_eq!(continue_message["method"], "Fetch.continueRequest");
        assert_eq!(continue_message["params"]["requestId"], "request-1");

        let fail_message = fail_request_message("request-2");
        assert_eq!(fail_message["method"], "Fetch.failRequest");
        assert_eq!(fail_message["params"]["requestId"], "request-2");
        assert_eq!(
            fail_message["params"]["errorReason"],
            BLOCKED_BY_CLIENT_REASON
        );
    }
}
