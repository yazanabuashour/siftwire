function referencedAliasName(type) {
    if (type.type !== "TSTypeReference" || type.typeName.type !== "Identifier")
        return;
    if ((type.typeArguments?.params.length ?? 0) > 0)
        return;
    return type.typeName.name;
}
export function collectTypeAliases(program) {
    const aliases = new Map();
    for (const statement of program.body) {
        const declaration = statement.type === "ExportNamedDeclaration"
            ? statement.declaration
            : statement;
        if (declaration?.type === "TSTypeAliasDeclaration" &&
            (declaration.typeParameters === null ||
                declaration.typeParameters === undefined)) {
            aliases.set(declaration.id.name, declaration);
        }
    }
    return aliases;
}
export function resolvesToUnknown(type, aliases, visited = new Set()) {
    if (type.type === "TSUnknownKeyword")
        return true;
    if (type.type === "TSParenthesizedType") {
        return resolvesToUnknown(type.typeAnnotation, aliases, visited);
    }
    if (type.type === "TSUnionType") {
        return type.types.some((member) => resolvesToUnknown(member, aliases, visited));
    }
    const name = referencedAliasName(type);
    if (name === undefined || visited.has(name))
        return false;
    const alias = aliases.get(name);
    if (alias === undefined)
        return false;
    const nextVisited = new Set(visited);
    nextVisited.add(name);
    return resolvesToUnknown(alias.typeAnnotation, aliases, nextVisited);
}
