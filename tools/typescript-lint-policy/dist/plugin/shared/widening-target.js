import { aliasSubstitution, isBuiltIn, isUnappliedReferenceTo, TRANSPARENT_WRAPPERS, typeReferenceName, unwrapTransparentType, } from "./type-environment.js";
import { dictionaryValueTypes } from "./unsafe-dictionary.js";
function resolvesToDictionary(type, environment, substitutions, resolvingAliases) {
    return (dictionaryValueTypes(type, environment, substitutions, resolvingAliases)
        .length > 0);
}
export function classifyWideningTarget(type, environment) {
    const unwrapped = unwrapTransparentType(type);
    if (unwrapped.type === "TSUnknownKeyword")
        return { kind: "unknown" };
    if (unwrapped.type === "TSObjectKeyword")
        return { kind: "object" };
    if (unwrapped.type === "TSTypeLiteral") {
        return unwrapped.members.some((member) => member.type === "TSIndexSignature")
            ? { kind: "open dictionary" }
            : unwrapped.members.length > 0
                ? { kind: "anonymous object" }
                : null;
    }
    if (unwrapped.type === "TSMappedType")
        return { kind: "open dictionary" };
    if (unwrapped.type !== "TSTypeReference")
        return null;
    const name = typeReferenceName(unwrapped);
    if (name === null)
        return null;
    if (TRANSPARENT_WRAPPERS.has(name) && isBuiltIn(name, environment)) {
        const wrapped = unwrapped.typeArguments?.params[0];
        return wrapped === undefined
            ? null
            : classifyWideningTarget(wrapped, environment);
    }
    if (name === "Record" && isBuiltIn(name, environment))
        return { kind: "open dictionary" };
    const alias = environment.aliases.get(name);
    if (alias === undefined)
        return null;
    if ((alias.typeParameters?.params.length ?? 0) > 0) {
        const substitutions = aliasSubstitution(alias, unwrapped, new Map());
        return substitutions !== null &&
            resolvesToDictionary(alias.typeAnnotation, environment, substitutions, new Set([name]))
            ? { kind: "generic container" }
            : null;
    }
    const substitutions = aliasSubstitution(alias, unwrapped, new Map());
    if (substitutions === null)
        return null;
    return classifyAliasBroadTarget(alias.typeAnnotation, environment, substitutions, new Set([name]));
}
function isBroadMappedKey(type, environment, substitutions) {
    const unwrapped = unwrapTransparentType(type);
    if (unwrapped.type === "TSStringKeyword" ||
        unwrapped.type === "TSNumberKeyword" ||
        unwrapped.type === "TSSymbolKeyword") {
        return true;
    }
    if (unwrapped.type === "TSUnionType") {
        return unwrapped.types.every((member) => isBroadMappedKey(member, environment, substitutions));
    }
    if (unwrapped.type !== "TSTypeReference")
        return false;
    const name = typeReferenceName(unwrapped);
    if (name === null)
        return false;
    const substitution = substitutions.get(name);
    if (substitution !== undefined &&
        !isUnappliedReferenceTo(substitution, name)) {
        return isBroadMappedKey(substitution, environment, substitutions);
    }
    return name === "PropertyKey" && isBuiltIn(name, environment);
}
function classifyAliasBroadTarget(type, environment, substitutions, resolvingAliases) {
    const unwrapped = unwrapTransparentType(type);
    if (unwrapped.type === "TSUnknownKeyword")
        return { kind: "unknown" };
    if (unwrapped.type === "TSObjectKeyword")
        return { kind: "object" };
    if (unwrapped.type === "TSTypeLiteral") {
        return unwrapped.members.some((member) => member.type === "TSIndexSignature")
            ? { kind: "open dictionary" }
            : null;
    }
    if (unwrapped.type === "TSMappedType") {
        return isBroadMappedKey(unwrapped.constraint, environment, substitutions)
            ? { kind: "open dictionary" }
            : null;
    }
    if (unwrapped.type !== "TSTypeReference")
        return null;
    const name = typeReferenceName(unwrapped);
    if (name === null)
        return null;
    const substitution = substitutions.get(name);
    if (substitution !== undefined) {
        return isUnappliedReferenceTo(substitution, name)
            ? null
            : classifyAliasBroadTarget(substitution, environment, substitutions, resolvingAliases);
    }
    if (TRANSPARENT_WRAPPERS.has(name) && isBuiltIn(name, environment)) {
        const wrapped = unwrapped.typeArguments?.params[0];
        return wrapped === undefined
            ? null
            : classifyAliasBroadTarget(wrapped, environment, substitutions, resolvingAliases);
    }
    if (name === "Record" && isBuiltIn(name, environment))
        return { kind: "open dictionary" };
    const alias = environment.aliases.get(name);
    if (alias === undefined || resolvingAliases.has(name))
        return null;
    const nextSubstitutions = aliasSubstitution(alias, unwrapped, substitutions);
    if (nextSubstitutions === null)
        return null;
    const nextResolving = new Set(resolvingAliases);
    nextResolving.add(name);
    return classifyAliasBroadTarget(alias.typeAnnotation, environment, nextSubstitutions, nextResolving);
}
