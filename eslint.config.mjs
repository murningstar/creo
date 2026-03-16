// @ts-check
import withNuxt from './.nuxt/eslint.config.mjs';

export default withNuxt({
    rules: {
        'no-restricted-imports': [
            'error',
            {
                patterns: [
                    {
                        group: ['**/server/**'],
                        allowTypeImports: true,
                        message:
                            'It\'s only allowed to import types from "server" folder, for example `import type { ... }` or `import { type ... } `',
                    },
                ],
            },
        ],
    },
});
