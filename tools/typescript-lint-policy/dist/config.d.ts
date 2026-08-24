export declare const strictestConfig: {
    categories: {
        correctness: "error";
        perf: "error";
        suspicious: "error";
    };
    plugins: ("eslint" | "react" | "unicorn" | "typescript" | "oxc" | "jsx-a11y")[];
    jsPlugins: {
        name: string;
        specifier: string;
    }[];
    options: {
        typeAware: false;
        typeCheck: false;
    };
    rules: {
        "eslint/no-await-in-loop": "off";
        "eslint/no-shadow": "off";
        "eslint/no-underscore-dangle": "off";
        "oxc/no-map-spread": "off";
        "react-in-jsx-scope": "off";
        "typescript/consistent-return": "off";
        "typescript/no-unnecessary-boolean-literal-compare": "off";
        "typescript/no-unnecessary-type-arguments": "off";
        "typescript/no-unnecessary-type-assertion": "off";
        "typescript/no-unnecessary-type-conversion": "off";
        "typescript/no-unnecessary-type-parameters": "off";
        "typescript/no-unsafe-type-assertion": "off";
        "unicorn/consistent-function-scoping": "off";
        "unicorn/no-array-sort": "off";
        "react/exhaustive-deps": "error";
        "react/rules-of-hooks": "error";
        "typescript/no-explicit-any": "error";
        "typescript/no-non-null-assertion": "error";
        "project/namespace-node-imports": "error";
        "project/no-chained-type-assertions": "error";
        "project/no-conditional-empty-object-spread": "error";
        "project/no-global-process-runtime": "error";
        "project/no-inline-schema-compile": "error";
        "project/no-known-value-widening": "error";
        "project/no-manual-effect-runtime-in-tests": "error";
        "project/no-module-mocking": "error";
        "project/no-object-parameters": "error";
        "project/no-reflect-apply": "error";
        "project/no-reflect-get": "error";
        "project/no-runtime-typeof": "error";
        "project/no-shape-in-symbol-names": "error";
        "project/no-unknown-parameters": "error";
        "project/no-unknown-returns": "error";
        "project/no-unknown-type-aliases": "error";
        "project/no-unsafe-dictionary-type": "error";
        "project/no-widen-then-assert": "error";
        "project/require-safety-comment-for-type-assertion": "error";
        "eslint/max-lines": ["error", {
            max: number;
            skipBlankLines: boolean;
            skipComments: boolean;
        }];
        "eslint/max-lines-per-function": ["error", {
            IIFEs: boolean;
            max: number;
            skipBlankLines: boolean;
            skipComments: boolean;
        }];
    };
};
export default strictestConfig;
