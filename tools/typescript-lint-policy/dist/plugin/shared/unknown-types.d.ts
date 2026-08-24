import type { ESTree } from "@oxlint/plugins";
export type TypeAliases = ReadonlyMap<string, ESTree.TSTypeAliasDeclaration>;
export declare function collectTypeAliases(program: ESTree.Program): TypeAliases;
export declare function resolvesToUnknown(type: ESTree.TSType, aliases: TypeAliases, visited?: ReadonlySet<string>): boolean;
