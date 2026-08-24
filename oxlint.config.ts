import policy from "@yazanabuashour/oxlint-config"
import { defineConfig } from "oxlint"

export default defineConfig({
  extends: [policy],
  ignorePatterns: ["tools/typescript-lint-policy", "target", "apps/web/dist"],
})
