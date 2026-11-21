import { defineConfig } from "oxlint"

import policy from "./tools/typescript-lint-policy/dist/config.js"

export default defineConfig({
  extends: [policy],
  ignorePatterns: ["tools/typescript-lint-policy", "target"],
})
