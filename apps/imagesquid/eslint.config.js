// ESLint flat config: TypeScript rules for sources, tests and configs.
import js from "@eslint/js";
import globals from "globals";
import tseslint from "typescript-eslint";

export default tseslint.config(
  {
    ignores: [
      "dist",
      "node_modules",
      "tests/allure-results",
      "tests/allure-report",
      "tests/test-results",
      "tests/playwright-report",
      "",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.strict,
  {
    languageOptions: { globals: { ...globals.browser, ...globals.node } },
  },
);
