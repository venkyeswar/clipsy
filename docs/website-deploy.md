# How the website is deployed

The website lives in `website/` (React + Vite). It is served by the Cloudflare Worker named `clipsy` at https://clipsy.thalvik.in.

Cloudflare builds and deploys it automatically when something is pushed to the `main` branch on GitHub. This is called Cloudflare Workers Builds.

## Cloudflare settings (Workers & Pages > clipsy > Settings > Builds)

| Setting | Value |
|---|---|
| Production branch | `main` |
| Root directory | `/website` |
| Build command | `npm run build` |
| Deploy command | `npx wrangler deploy` |
| Build watch paths | None. Every push to `main` builds. Can be narrowed to `website/*` later. |
| Non-production branch builds | Off. Or use root `/website` and deploy command `npx wrangler versions upload`. Preview builds of other branches fail with the wrong root otherwise. |

## Good to know

- The site shows the newest release by reading GitHub releases in the visitor's browser. `website/src/data/releases.snapshot.json` is only an offline fallback. The `prebuild` step refreshes it with `node scripts/generate-snapshot.mjs`.
- `website/wrangler.jsonc` names the Worker (`clipsy`) and the build folder (`./dist`).
- Push to `main` only for website changes. Pushing the same commit to two branches at once confused Cloudflare earlier. It built the other branch with the wrong root.
- Manual fallback if automatic builds stop: run `cd website && npm run build`. Then, in the Cloudflare dashboard, open the worker, go to Deployments, click New deployment, and upload the `website/dist` folder.
- There is also a GitHub Actions workflow, `.github/workflows/deploy-website.yml`, that can deploy with wrangler. It does nothing unless the repository secrets `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` exist.
