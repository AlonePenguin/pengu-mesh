use serde_json::{Value, json};

const STYLE_MARKER: &str = "no-animations";

/// CSS snippet that force-disables all animations and transitions.
const SUPPRESS_CSS: &str = "*, *::before, *::after { \
    animation: none !important; \
    animation-duration: 0s !important; \
    transition: none !important; \
    transition-duration: 0s !important; \
    scroll-behavior: auto !important; \
}";

/// Returns the CSS text used to suppress all animations and transitions.
pub(crate) fn suppress_animations_css() -> &'static str {
    SUPPRESS_CSS
}

/// Builds a CDP `Emulation.setEmulatedMedia` message that sets
/// `prefers-reduced-motion: reduce`.
pub(crate) fn reduced_motion_media_override() -> Value {
    json!({
        "method": "Emulation.setEmulatedMedia",
        "params": {
            "features": [
                { "name": "prefers-reduced-motion", "value": "reduce" }
            ]
        }
    })
}

/// Builds a CDP `Emulation.setEmulatedMedia` message that clears any previously
/// configured media overrides.
pub(crate) fn clear_reduced_motion_media_override() -> Value {
    json!({
        "method": "Emulation.setEmulatedMedia",
        "params": {
            "media": "",
            "features": []
        }
    })
}

/// Builds a CDP `Page.addScriptToEvaluateOnNewDocument` message that injects a
/// script to insert a `<style>` element disabling CSS animations on every
/// document load.
pub(crate) fn inject_animation_suppression() -> Value {
    json!({
        "method": "Page.addScriptToEvaluateOnNewDocument",
        "params": {
            "source": suppression_script_source()
        }
    })
}

/// Builds a CDP `Page.removeScriptToEvaluateOnNewDocument` message that removes
/// a previously registered animation-suppression script.
pub(crate) fn remove_injected_animation_suppression(identifier: &str) -> Value {
    json!({
        "method": "Page.removeScriptToEvaluateOnNewDocument",
        "params": {
            "identifier": identifier
        }
    })
}

/// Builds a CDP `Runtime.evaluate` message that applies animation suppression
/// to the current document immediately.
pub(crate) fn apply_animation_suppression() -> Value {
    json!({
        "method": "Runtime.evaluate",
        "params": {
            "expression": suppression_script_source(),
            "returnByValue": true,
            "awaitPromise": true
        }
    })
}

/// Builds a CDP `Runtime.evaluate` message that removes any animation
/// suppression styles from the current document.
pub(crate) fn clear_animation_suppression() -> Value {
    json!({
        "method": "Runtime.evaluate",
        "params": {
            "expression": clear_suppression_script_source(),
            "returnByValue": true,
            "awaitPromise": true
        }
    })
}

fn suppression_script_source() -> String {
    let marker = serde_json::to_string(STYLE_MARKER).expect("serialize style marker");
    let css = serde_json::to_string(suppress_animations_css()).expect("serialize suppression CSS");
    format!(
        r#"(function() {{
  const marker = {marker};
  const css = {css};
  const selector = 'style[data-pengu-mesh="' + marker + '"]';
  const install = () => {{
    if (document.querySelector(selector)) {{
      return 'already-applied';
    }}
    const root = document.head || document.documentElement;
    if (!root) {{
      return 'pending';
    }}
    const style = document.createElement('style');
    style.setAttribute('data-pengu-mesh', marker);
    style.textContent = css;
    root.appendChild(style);
    return 'applied';
  }};
  const result = install();
  if (result !== 'pending') {{
    return result;
  }}
  const observer = new MutationObserver(() => {{
    if (install() !== 'pending') {{
      observer.disconnect();
    }}
  }});
  observer.observe(document, {{ childList: true, subtree: true }});
  return result;
}})()"#
    )
}

fn clear_suppression_script_source() -> String {
    let marker = serde_json::to_string(STYLE_MARKER).expect("serialize style marker");
    format!(
        r#"(function() {{
  const selector = 'style[data-pengu-mesh="' + {marker} + '"]';
  let removed = 0;
  document.querySelectorAll(selector).forEach((style) => {{
    style.remove();
    removed += 1;
  }});
  return removed;
}})()"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_snippet_disables_animations_and_transitions() {
        let css = suppress_animations_css();
        assert!(css.contains("animation: none !important"));
        assert!(css.contains("transition: none !important"));
        assert!(css.contains("*::before"));
        assert!(css.contains("*::after"));
    }

    #[test]
    fn reduced_motion_message_has_correct_structure() {
        let msg = reduced_motion_media_override();
        assert_eq!(msg["method"], "Emulation.setEmulatedMedia");
        let features = msg["params"]["features"].as_array().unwrap();
        assert_eq!(features.len(), 1);
        assert_eq!(features[0]["name"], "prefers-reduced-motion");
        assert_eq!(features[0]["value"], "reduce");
    }

    #[test]
    fn clear_reduced_motion_message_has_correct_structure() {
        let msg = clear_reduced_motion_media_override();
        assert_eq!(msg["method"], "Emulation.setEmulatedMedia");
        assert_eq!(msg["params"]["media"], "");
        assert_eq!(msg["params"]["features"], json!([]));
    }

    #[test]
    fn inject_suppression_message_has_correct_structure() {
        let msg = inject_animation_suppression();
        assert_eq!(msg["method"], "Page.addScriptToEvaluateOnNewDocument");
        let source = msg["params"]["source"].as_str().unwrap();
        assert!(source.contains("animation: none !important"));
        assert!(source.contains("data-pengu-mesh"));
        assert!(source.contains("MutationObserver"));
    }

    #[test]
    fn remove_injected_suppression_message_has_correct_structure() {
        let msg = remove_injected_animation_suppression("script-123");
        assert_eq!(msg["method"], "Page.removeScriptToEvaluateOnNewDocument");
        assert_eq!(msg["params"]["identifier"], "script-123");
    }

    #[test]
    fn apply_suppression_message_has_correct_structure() {
        let msg = apply_animation_suppression();
        assert_eq!(msg["method"], "Runtime.evaluate");
        assert_eq!(msg["params"]["returnByValue"], true);
        assert_eq!(msg["params"]["awaitPromise"], true);
        let expression = msg["params"]["expression"].as_str().unwrap();
        assert!(expression.contains("style.setAttribute('data-pengu-mesh'"));
        assert!(expression.contains("return 'applied'"));
    }

    #[test]
    fn clear_suppression_message_has_correct_structure() {
        let msg = clear_animation_suppression();
        assert_eq!(msg["method"], "Runtime.evaluate");
        let expression = msg["params"]["expression"].as_str().unwrap();
        assert!(expression.contains("document.querySelectorAll"));
        assert!(expression.contains("style.remove()"));
        assert!(expression.contains("return removed"));
    }
}
