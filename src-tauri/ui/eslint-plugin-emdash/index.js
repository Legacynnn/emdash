/**
 * eslint-plugin-emdash
 *
 * Local-only ESLint plugin enforcing emdash-dev's bridge discipline.
 *
 * Rule: `no-tauri-event-bus`
 * -------------------------
 * Renderer state sync flows through one `Channel<UiMutationEvent>` opened
 * by `useUiMutations`. Ad-hoc `listen(...)` from `@tauri-apps/api/event`
 * or host-side `app.emit(...)` calls fragment the cache-invalidation
 * policy. This rule errors on either pattern outside the sanctioned
 * `ui-sync/` module.
 *
 * **Exemption pragma**: prefix a line with
 *   // emdash-disable-next-line no-tauri-event-bus -- <reason>
 * to bypass the rule for genuine exceptions. ESLint's standard
 * `eslint-disable-*` comments work too, but the named pragma keeps the
 * audit trail searchable.
 */

const ALLOWED_DIR_FRAGMENT = '/ui-sync/';
const PRAGMA = 'emdash-disable-next-line no-tauri-event-bus';

function isExempted(context, node) {
  const sourceCode = context.sourceCode ?? context.getSourceCode();
  const before = sourceCode.getCommentsBefore(node);
  for (const c of before) {
    if (c.value.trim().startsWith(PRAGMA)) return true;
  }
  return false;
}

function inUiSyncDir(filename) {
  return filename.replace(/\\/g, '/').includes(ALLOWED_DIR_FRAGMENT);
}

/**
 * Imports of `listen` / `once` from `@tauri-apps/api/event` and direct
 * calls to those bindings outside `ui-sync/`.
 */
const noTauriEventBus = {
  meta: {
    type: 'problem',
    docs: {
      description:
        'Disallow ad-hoc Tauri event-bus subscriptions/emissions outside the UiMutationEvent bridge.',
    },
    schema: [],
    messages: {
      forbiddenImport:
        '`{{name}}` from `@tauri-apps/api/event` is reserved for the UiMutationEvent bridge in `ui-sync/`. Use `useUiMutations` instead.',
      forbiddenCall:
        '`{{name}}(...)` bypasses the UiMutationEvent bridge. Route renderer state sync through `useUiMutations` (see ADR-0004).',
      forbiddenEmit:
        '`app.emit(...)` (or Tauri-host equivalent) is forbidden — broadcasts must go through `UiSyncManager::broadcast`.',
    },
  },
  create(context) {
    const filename = context.filename ?? context.getFilename();
    if (inUiSyncDir(filename)) return {};

    const eventBusImports = new Set();

    return {
      ImportDeclaration(node) {
        if (node.source.value !== '@tauri-apps/api/event') return;
        for (const spec of node.specifiers) {
          if (spec.type !== 'ImportSpecifier') continue;
          const imported = spec.imported.name;
          if (imported !== 'listen' && imported !== 'once') continue;
          if (isExempted(context, node)) continue;
          context.report({
            node: spec,
            messageId: 'forbiddenImport',
            data: { name: imported },
          });
          eventBusImports.add(spec.local.name);
        }
      },
      CallExpression(node) {
        if (isExempted(context, node)) return;

        // Bare-imported listen/once
        if (node.callee.type === 'Identifier' && eventBusImports.has(node.callee.name)) {
          context.report({
            node,
            messageId: 'forbiddenCall',
            data: { name: node.callee.name },
          });
          return;
        }

        // Member-form: <anything>.emit(...) where the receiver is plausibly the
        // Tauri AppHandle. Match defensively on the property name.
        if (
          node.callee.type === 'MemberExpression' &&
          !node.callee.computed &&
          node.callee.property.type === 'Identifier'
        ) {
          const prop = node.callee.property.name;
          if (prop === 'emit' || prop === 'emit_all' || prop === 'emit_to') {
            // Skip method calls on `Channel` instances (they have .send, not .emit,
            // but harmless to be precise) and on obvious DOM event-target
            // identifiers; conservative match keeps false positives down.
            const objectName =
              node.callee.object.type === 'Identifier' ? node.callee.object.name : null;
            const isLikelyDom =
              objectName !== null &&
              ['window', 'document', 'element', 'event', 'target'].includes(
                objectName.toLowerCase(),
              );
            if (isLikelyDom) return;
            context.report({ node, messageId: 'forbiddenEmit' });
          }
        }
      },
    };
  },
};

export default {
  rules: {
    'no-tauri-event-bus': noTauriEventBus,
  },
};
