import { definePlugin } from "@oxlint/plugins";
import { noServiceConstructorImportsRule } from "./rules/no-service-constructor-imports.js";
export default definePlugin({
    meta: { name: "project-effect" },
    rules: { "no-service-constructor-imports": noServiceConstructorImportsRule },
});
