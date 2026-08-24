const BUILT_INS = new Set([
    "Record",
    "Readonly",
    "Partial",
    "Required",
    "Pick",
    "Omit",
    "PropertyKey",
    "NonNullable",
]);
export const TRANSPARENT_WRAPPERS = new Set([
    "Readonly",
    "Partial",
    "Required",
    "NonNullable",
]);
function declaredStatement(statement) {
    return statement.type === "ExportNamedDeclaration" ||
        statement.type === "ExportDefaultDeclaration"
        ? (statement.declaration ?? null)
        : statement;
}
export function createTypeEnvironment(program) {
    const aliases = new Map();
    const interfaces = new Map();
    const shadowedBuiltIns = new Set();
    for (const statement of program.body) {
        const declaration = declaredStatement(statement);
        if (declaration?.type === "ImportDeclaration") {
            for (const specifier of declaration.specifiers) {
                if (BUILT_INS.has(specifier.local.name))
                    shadowedBuiltIns.add(specifier.local.name);
            }
            continue;
        }
        if (declaration?.type === "TSTypeAliasDeclaration") {
            const existing = aliases.get(declaration.id.name);
            if (existing === undefined)
                aliases.set(declaration.id.name, declaration);
            else
                shadowedBuiltIns.add(declaration.id.name);
            if (BUILT_INS.has(declaration.id.name))
                shadowedBuiltIns.add(declaration.id.name);
            continue;
        }
        if (declaration?.type === "TSInterfaceDeclaration") {
            const declarations = interfaces.get(declaration.id.name) ?? [];
            declarations.push(declaration);
            interfaces.set(declaration.id.name, declarations);
            if (BUILT_INS.has(declaration.id.name))
                shadowedBuiltIns.add(declaration.id.name);
            continue;
        }
        if (declaration?.type === "TSEnumDeclaration") {
            if (BUILT_INS.has(declaration.id.name))
                shadowedBuiltIns.add(declaration.id.name);
            continue;
        }
        if ((declaration?.type === "ClassDeclaration" ||
            declaration?.type === "FunctionDeclaration") &&
            declaration.id !== null &&
            BUILT_INS.has(declaration.id.name)) {
            shadowedBuiltIns.add(declaration.id.name);
        }
    }
    return { aliases, interfaces, shadowedBuiltIns };
}
export function typeReferenceName(type) {
    return type.typeName.type === "Identifier" ? type.typeName.name : null;
}
export function isBuiltIn(name, environment) {
    return BUILT_INS.has(name) && !environment.shadowedBuiltIns.has(name);
}
export function unwrapTransparentType(type) {
    let current = type;
    while (current.type === "TSParenthesizedType" ||
        (current.type === "TSTypeOperator" && current.operator === "readonly")) {
        current = current.typeAnnotation;
    }
    return current;
}
export function isUnappliedReferenceTo(type, name) {
    const unwrapped = unwrapTransparentType(type);
    return (unwrapped.type === "TSTypeReference" &&
        typeReferenceName(unwrapped) === name &&
        (unwrapped.typeArguments === null ||
            unwrapped.typeArguments === undefined ||
            unwrapped.typeArguments.params.length === 0));
}
function resolvedSubstitutionArgument(type, base, resolving = new Set()) {
    const unwrapped = unwrapTransparentType(type);
    if (unwrapped.type !== "TSTypeReference")
        return type;
    const name = typeReferenceName(unwrapped);
    if (name === null || resolving.has(name))
        return type;
    const substitution = base.get(name);
    if (substitution === undefined)
        return type;
    const nextResolving = new Set(resolving);
    nextResolving.add(name);
    return resolvedSubstitutionArgument(substitution, base, nextResolving);
}
export function aliasSubstitution(alias, type, base) {
    const parameters = alias.typeParameters?.params ?? [];
    const typeArguments = type.typeArguments?.params ?? [];
    const next = new Map(base);
    for (const [index, parameter] of parameters.entries()) {
        const argument = typeArguments[index] ?? parameter.default;
        if (argument === null || argument === undefined)
            return null;
        next.set(parameter.name.name, resolvedSubstitutionArgument(argument, next));
    }
    return next;
}
