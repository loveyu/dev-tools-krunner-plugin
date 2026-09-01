export type JsonPrimitive = string | number | boolean | null;
// Record 别名无法表达递归 JSON 对象，因此这里保留显式索引签名。
export type JsonObject = { readonly [key: string]: JsonValue };
export type JsonValue = JsonPrimitive | readonly JsonValue[] | JsonObject;

export type JsonTreeNode = {
  readonly id: string;
  readonly key: string;
  readonly path: string;
  readonly type: 'array' | 'object' | 'primitive';
  readonly preview: string;
  readonly children: readonly JsonTreeNode[];
};

export function parseJson(raw: string): JsonValue {
  const value: unknown = JSON.parse(raw);
  if (!isJsonValue(value)) {
    throw new TypeError('convert.errors.invalidJsonValue');
  }
  return value;
}

export function formatJson(value: JsonValue): string {
  return JSON.stringify(value, null, 2);
}

export function minifyJson(value: JsonValue): string {
  return JSON.stringify(value);
}

export function buildJsonTree(value: JsonValue): JsonTreeNode {
  return buildNode('root', '$', value);
}

export function filterJsonTree(node: JsonTreeNode, query: string): JsonTreeNode | null {
  const normalizedQuery = query.trim().toLocaleLowerCase('zh-CN');
  if (normalizedQuery.length === 0 || nodeMatches(node, normalizedQuery)) {
    return node;
  }

  const children = node.children
    .map((child) => filterJsonTree(child, normalizedQuery))
    .filter((child): child is JsonTreeNode => child !== null);

  return children.length === 0 ? null : { ...node, children };
}

export function countMatches(node: JsonTreeNode, query: string): number {
  const normalizedQuery = query.trim().toLocaleLowerCase('zh-CN');
  if (normalizedQuery.length === 0) {
    return 0;
  }

  const ownMatch = nodeMatches(node, normalizedQuery) ? 1 : 0;
  return ownMatch + node.children.reduce((total, child) => total + countMatches(child, query), 0);
}

function buildNode(key: string, path: string, value: JsonValue): JsonTreeNode {
  if (isJsonArray(value)) {
    return {
      id: path,
      key,
      path,
      type: 'array',
      preview: `[${String(value.length)}]`,
      children: value.map((child, index) =>
        buildNode(String(index), `${path}[${String(index)}]`, child),
      ),
    };
  }

  if (isJsonObject(value)) {
    const entries = Object.entries(value);
    return {
      id: path,
      key,
      path,
      type: 'object',
      preview: `{${String(entries.length)}}`,
      children: entries.map(([childKey, child]) =>
        buildNode(childKey, childPath(path, childKey), child),
      ),
    };
  }

  return {
    id: path,
    key,
    path,
    type: 'primitive',
    preview: JSON.stringify(value),
    children: [],
  };
}

function isJsonValue(value: unknown): value is JsonValue {
  if (
    value === null ||
    typeof value === 'string' ||
    typeof value === 'boolean' ||
    (typeof value === 'number' && Number.isFinite(value))
  ) {
    return true;
  }

  if (Array.isArray(value)) {
    return value.every((item: unknown) => isJsonValue(item));
  }

  return isJsonObject(value) && Object.values(value).every((item) => isJsonValue(item));
}

function isJsonObject(value: unknown): value is JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isJsonArray(value: JsonValue): value is readonly JsonValue[] {
  return Array.isArray(value);
}

function nodeMatches(node: JsonTreeNode, normalizedQuery: string): boolean {
  return [node.key, node.path, node.preview].some((candidate) =>
    candidate.toLocaleLowerCase('zh-CN').includes(normalizedQuery),
  );
}

function childPath(parent: string, segment: string): string {
  return /^[A-Za-z_$][\w$]*$/u.test(segment)
    ? `${parent}.${segment}`
    : `${parent}[${JSON.stringify(segment)}]`;
}
