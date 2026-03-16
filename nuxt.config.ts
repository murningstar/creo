import type { NuxtConfig } from 'nuxt/schema';
import { globSync } from 'node:fs';
import { resolve } from 'node:path';
import { defu } from 'defu';
import tailwindcss from '@tailwindcss/vite';

//#region readme
/*
    - `defu` just merges them back into single NuxtConfig
    - NuxtConfig is split into named parts just for convenience
*/
// #endregion;

export default defineNuxtConfig(
    defu<NuxtConfig, NuxtConfig[]>(
        {
            // #region Nuxt, Vite base cfg
            // Base nuxt configuration - nuxt related, modules, plugins and etc
            compatibilityDate: '2025-07-15',
            devtools: { enabled: true },

            modules: ['@nuxt/eslint', '@nuxt/ui', '@pinia/nuxt', '@vueuse/nuxt'],

            vite: {
                plugins: [tailwindcss()],
            },

            pinia: { storesDirs: [] }, // TODO

            css: ['./src/app/styles/main.css'],

            ssr: false, // Disable SSR for Tauri compatibility

            serverDir: './server',

            typescript: {
                strict: true,
                typeCheck: true,
            },

            telemetry: false,
            // #endregion
        },
        {
            // #region Tauri && Vite cfg
            devServer: {
                host: '0.0.0.0',
                port: 4730,
            },

            vite: {
                // Better support for Tauri CLI output
                clearScreen: false,
                // Enable environment variables
                // Additional environment variables can be found at
                // https://v2.tauri.app/reference/environment-variables/
                envPrefix: ['VITE_', 'TAURI_'],
                server: {
                    strictPort: true, // Tauri requires a consistent port

                    watch: {
                        usePolling: true, // Fix file watcher issues on some systems
                    },
                },
            },

            ignore: ['**/src-tauri/**'], // Prevent watching Tauri files (causes infinite loops)
            // #endregion
        },
        {
            // #region Feature Sliced Design
            // FSD architecture configuration
            srcDir: './src', // Используем /src, а не /app, т.к. коллизия имён - в FSD тоже есть папка /app

            vite: {
                resolve: {
                    alias: {
                        '@': './src',
                        '@app': './src/app',
                        '@pages': './src/pages',
                        '@widgets': './src/widgets',
                        '@features': './src/features',
                        '@entities': './src/entities',
                        '@shared': './src/shared',
                    },
                },
            },

            alias: {
                '@': './src',
                '@app': './src/app',
                '@pages': './src/pages',
                '@widgets': './src/widgets',
                '@features': './src/features',
                '@entities': './src/entities',
                '@shared': './src/shared',
            },

            hooks: {
                'app:resolve': app => {
                    app.mainComponent = '@/app/entrypoint/app.vue'; // Override the default app.vue location to FSD's app/entrypoint
                },
            },

            dir: {
                layouts: 'shared/ui/layouts',
                plugins: 'app/plugins',
                middleware: 'app/middleware',
            },

            /* Auto-imports for components from `/shared` */
            components: {
                dirs: [
                    {
                        path: '.',
                        pattern: '{shared}/**/ui/*/*.vue',
                        prefix: 'c',
                        pathPrefix: false,
                    },
                ],
            },
            // #endregion
        },
        {
            // #region `/server` as FSD segment
            // experimental nuxt config for `/server` folder as FSD segment. So there could be a server code directly in entity or feature or whatever.
            nitro: {
                scanDirs: [
                    ...globSync('src/app/**/server', { cwd: import.meta.dirname }).map(path => resolve(path)),
                    ...globSync('src/pages/**/server', { cwd: import.meta.dirname }).map(path => resolve(path)),
                    ...globSync('src/widgets/**/server', { cwd: import.meta.dirname }).map(path => resolve(path)),
                    ...globSync('src/features/**/server', { cwd: import.meta.dirname }).map(path => resolve(path)),
                    ...globSync('src/entities/**/server', { cwd: import.meta.dirname }).map(path => resolve(path)),
                    ...globSync('src/shared/**/server', { cwd: import.meta.dirname }).map(path => resolve(path)),
                ],
            },
            // #endregion
        }
    )
);
