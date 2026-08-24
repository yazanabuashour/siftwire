import type { ESTree } from "@oxlint/plugins";
export declare const TRANSPARENT_WRAPPERS: Set<string>;
export type TypeAliasEnvironment = ReadonlyMap<string, ESTree.TSType>;
export type TypeEnvironment = {
    readonly aliases: ReadonlyMap<string, ESTree.TSTypeAliasDeclaration>;
    readonly interfaces: ReadonlyMap<string, readonly ESTree.TSInterfaceDeclaration[]>;
    readonly shadowedBuiltIns: ReadonlySet<string>;
};
export declare function createTypeEnvironment(program: ESTree.Program): TypeEnvironment;
export declare function typeReferenceName(type: ESTree.TSTypeReference): string | null;
export declare function isBuiltIn(name: string, environment: TypeEnvironment): boolean;
export declare function unwrapTransparentType(type: ESTree.TSType): ESTree.TSType;
export declare function isUnappliedReferenceTo(type: ESTree.TSType, name: string): boolean;
export declare function aliasSubstitution(alias: ESTree.TSTypeAliasDeclaration, type: ESTree.TSTypeReference, base: TypeAliasEnvironment): TypeAliasEnvironment | null;
