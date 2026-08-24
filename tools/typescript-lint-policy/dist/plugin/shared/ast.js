function isExpressionWrapper(node) {
    return (node.type === "ChainExpression" ||
        node.type === "ParenthesizedExpression" ||
        node.type === "TSNonNullExpression" ||
        node.type === "TSAsExpression" ||
        node.type === "TSTypeAssertion");
}
export function unwrapExpression(node) {
    let current = node ?? null;
    while (current !== null && isExpressionWrapper(current)) {
        current = current.expression;
    }
    return current;
}
export function getPropertyName(expression) {
    if (expression === null || expression === undefined)
        return null;
    if (expression.type === "Identifier" ||
        expression.type === "PrivateIdentifier") {
        return expression.name;
    }
    if (expression.type === "Literal" && typeof expression.value === "string") {
        return expression.value;
    }
    return null;
}
export function isIdentifier(node, name) {
    return (node?.type === "Identifier" && (name === undefined || node.name === name));
}
export function literalStringValue(expression) {
    return expression?.type === "Literal" && typeof expression.value === "string"
        ? expression.value
        : null;
}
