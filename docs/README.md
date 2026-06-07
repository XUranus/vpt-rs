# vpt-rs Documentation

This site is built using [Docusaurus](https://docusaurus.io/).

## Local Development

```bash
cd docs
npm install
npm start
```

Opens a local dev server at `http://localhost:3000/vpt-rs/`.

## Build

```bash
cd docs
npm run build
```

Generates static files in `build/` for both English and Chinese locales.

## Deployment

The site is automatically deployed to **https://xuranus.github.io/vpt-rs** via GitHub Actions when changes are pushed to `docs/` on the `master` branch.

The workflow is defined in `.github/workflows/deploy-docs.yml`.

### Prerequisites

GitHub Pages must be enabled in repository settings:
1. Go to **Settings → Pages**
2. Under **Source**, select **GitHub Actions**
3. Save

## Languages

- English (default)
- 简体中文 (zh-Hans)

Switch languages using the locale dropdown in the navbar.
