# @imagesquid/ui

The ImageSquid distribution of FileSquid: photo-album views, design tokens, label table and brand, with no backend code. Requirements: [SRS](./resources/srs.md). Plan: [plan](./resources/plan.md).

## Develop and test

From `apps/imagesquid`, on any OS with Node.js 22.12 or later and npm 11 or later, after `npm ci` in `apps/filesquid/ui` (this package links `@filesquid/ui` by relative path):

```sh
npm ci
npm run lint           # ESLint
npm run check          # tsc --noEmit
npm test               # Vitest unit and jsdom UI tests; Allure and JUnit in tests/allure-results/
```

The other scripts (`dev` on port 5174, `test:e2e` for Playwright, `report` for the Allure report) are listed in [`package.json`](./package.json).

During the FileSquid MVP the FileSquid repository's pre-commit hook does not run these scripts (owner decision 2026-10-23, recorded in the FileSquid rule `.agents/rules/local-verification.md`, section "ImageSquid during the MVP"). Run them by hand when you change ImageSquid. The hook still checks this package in two ways, with no ImageSquid install:

- `prettier --check .` here, with `@filesquid/ui`'s Prettier and this package's `.prettierrc.json` and `.prettierignore`.
- `@filesquid/ui`'s `tests/unit/distribution-tables.test.ts` reads [`src/theme/labels.en.json`](./src/theme/labels.en.json) and [`src/theme/tokens.css`](./src/theme/tokens.css) and fails when either lacks a key or token of FileSquid's defaults, in light or dark, or when a colour pair misses WCAG 2.2 AA (no exemption). A change confined to this directory, its top-level `package.json` and `package-lock.json` included, selects the hook scope `npm:apps/filesquid/ui`, which runs that suite and no Cargo step. A nested `package.json` or `package-lock.json` here, a change to the hook's own scripts or to the FileSquid Cargo manifest or lockfile, or any doubt in scope detection selects the full suite instead (the scope table in the FileSquid rule `.agents/rules/local-verification.md`).

ImageSquid's own vocabulary, brand and Arkrim primary checks stay in [`tests/unit/theme.test.ts`](./tests/unit/theme.test.ts) and [`tests/ui/distribution.test.ts`](./tests/ui/distribution.test.ts). When ImageSquid work starts, this package's full lint, typecheck, Vitest and Playwright suite returns to its own repository's gates, or to the FileSquid hook if ImageSquid is still in that repository.
