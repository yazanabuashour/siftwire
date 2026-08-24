import { defineRule } from "@oxlint/plugins";
import { collectTypeAliases, resolvesToUnknown, } from "../shared/unknown-types.js";
/** Ban named aliases that merely conceal TypeScript's unknown top type. */
export const noUnknownTypeAliasesRule = defineRule({
    meta: {
        type: "problem",
        docs: {
            description: "Disallow type aliases whose resolved type is unknown; unknown must remain visible at an allowed boundary.",
        },
        messages: {
            unknownAlias: "Type alias `{{alias}}` hides `unknown`. Keep `unknown` explicit at the parsing boundary or on an allowed `cause` field; otherwise use the parsed owner type.",
        },
    },
    create(context) {
        return {
            Program(node) {
                const aliases = collectTypeAliases(node);
                for (const alias of aliases.values()) {
                    if (!resolvesToUnknown(alias.typeAnnotation, aliases, new Set([alias.id.name])))
                        continue;
                    context.report({
                        node: alias.id,
                        messageId: "unknownAlias",
                        data: { alias: alias.id.name },
                    });
                }
            },
        };
    },
});
