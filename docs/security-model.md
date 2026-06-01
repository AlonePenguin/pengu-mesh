# Security Model

- bind locally by default
- keep attach restricted until explicitly enabled
- treat lease holders as trusted-local coordination identities, not authenticated principals
- surface capability policy in health, doctor, and the dashboard: safe
  capabilities are allowed by default, elevated capabilities are denied by
  default, and dangerous capabilities require explicit grants
- keep control-surface and host-access permissions visible through doctor, health, and audit outputs
- avoid widening host permissions without a proof artifact and rollback path
- prefer background-safe channels first and report every escalation to app or global takeover explicitly
- keep machine setup flows auditable: before and after posture, requested services, and settings deeplinks should be captured under ignored local report paths
- commit only curated public-safe summaries or artifacts from proof runs
