import type { ESTree } from "@oxlint/plugins";
export type AstNode = ESTree.Node;
export declare function unwrapExpression(node: AstNode | null | undefined): AstNode | null;
export declare function getPropertyName(expression: AstNode | null | undefined): string | null;
export declare function isIdentifier(node: AstNode | null, name?: string): boolean;
export declare function literalStringValue(expression: AstNode | null | undefined): string | null;
