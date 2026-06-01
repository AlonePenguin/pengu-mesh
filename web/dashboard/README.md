# Operator Console Scaffold

`web/dashboard/` is a read-only Vite and React scaffold over the local
`/health` control-plane contract.

It is not the operator truth surface and it does not mutate runtime state. The
CLI, `pengu-mesh-doctor`, and the HTTP daemon remain authoritative.

## Local development

```bash
npm ci
npm run dev
```

By default the dev server proxies supported control-plane routes to
`http://127.0.0.1:43127`. Override that origin with
`PENGU_MESH_DASHBOARD_API_ORIGIN` when needed.

## Verification

```bash
npm ci
npm run build
```
