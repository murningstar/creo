<template>
    <div class="flex h-screen w-screen items-center justify-center">
        <div class="indicator" :class="modeClass">
            <!-- Standby: breathing glow -->
            <span v-if="isStandby" class="standby-dot" />

            <!-- Dictation: waveform bars -->
            <div v-else-if="isDictation" class="waveform">
                <span
                    v-for="i in 5"
                    :key="i"
                    class="waveform-bar"
                    :style="{ transform: `scaleY(${barHeights[i - 1]})`, transitionDelay: `${(i - 1) * 12}ms` }"
                />
            </div>

            <!-- AwaitingSubcommand: ⌘ icon -->
            <svg
                v-else-if="isAwaiting"
                class="command-icon"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
            >
                <path
                    d="M18 3a3 3 0 0 0-3 3v12a3 3 0 0 0 3 3 3 3 0 0 0 3-3 3 3 0 0 0-3-3H6a3 3 0 0 0-3 3 3 3 0 0 0 3 3 3 3 0 0 0 3-3V6a3 3 0 0 0-3-3 3 3 0 0 0-3 3 3 3 0 0 0 3 3h12a3 3 0 0 0 3-3 3 3 0 0 0-3-3z"
                />
            </svg>

            <!-- Processing: conic-gradient ring -->
            <div v-else-if="isProcessing" class="processing-ring" />

            <!-- Transient: checkmark -->
            <svg v-if="transientState === 'success'" class="transient-icon success-icon" viewBox="0 0 24 24">
                <circle cx="12" cy="12" r="10" fill="none" stroke="currentColor" stroke-width="2" class="draw-circle" />
                <path
                    d="M8 12l3 3 5-5"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    class="draw-check"
                />
            </svg>

            <!-- Transient: error -->
            <svg v-if="transientState === 'error'" class="transient-icon error-icon" viewBox="0 0 24 24">
                <circle cx="12" cy="12" r="10" fill="none" stroke="currentColor" stroke-width="2" class="draw-circle" />
                <path
                    d="M15 9l-6 6M9 9l6 6"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    stroke-linecap="round"
                    class="draw-x"
                />
            </svg>

            <!-- Mini-badge for batch processing during dictation -->
            <div v-if="showMiniBadge" class="mini-badge" :class="miniBadgeClass">
                <div v-if="miniBadgeState === 'processing'" class="mini-spinner" />
                <svg v-else-if="miniBadgeState === 'done'" class="mini-check" viewBox="0 0 12 12">
                    <path
                        d="M3 6l2 2 4-4"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="1.5"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                    />
                </svg>
            </div>
        </div>
    </div>
</template>

<script setup lang="ts">
    import { listen } from '@tauri-apps/api/event';
    import { getCurrentWindow } from '@tauri-apps/api/window';

    import type { AudioStateEvent, SubcommandMatchEvent } from '~/entities/audio';
    import { AudioMode } from '~/entities/audio';

    definePageMeta({
        layout: 'overlay',
    });

    // --- State ---

    const mode = ref<AudioMode>(AudioMode.Standby);
    const amplitude = ref(0);
    const transientState = ref<'success' | 'error' | null>(null);
    const miniBadgeState = ref<'processing' | 'done' | null>(null);

    let transientTimer: ReturnType<typeof setTimeout> | null = null;
    let miniBadgeTimer: ReturnType<typeof setTimeout> | null = null;

    // --- Computed ---

    const isStandby = computed(() => mode.value === AudioMode.Standby);
    const isDictation = computed(() => mode.value === AudioMode.Dictation);
    const isAwaiting = computed(() => mode.value === AudioMode.AwaitingSubcommand);
    const isProcessing = computed(() => mode.value === AudioMode.Processing);

    const modeClass = computed(() => {
        if (transientState.value === 'error') return 'error shake';
        if (transientState.value === 'success') return 'success';
        switch (mode.value) {
            case AudioMode.Standby:
                return 'standby';
            case AudioMode.Dictation:
                return 'dictation';
            case AudioMode.Processing:
                return 'processing';
            case AudioMode.AwaitingSubcommand:
                return 'awaiting';
            default:
                return 'standby';
        }
    });

    const showMiniBadge = computed(() => miniBadgeState.value !== null && isDictation.value);
    const miniBadgeClass = computed(() => miniBadgeState.value);

    // --- Waveform bars ---

    const barHeights = computed(() => {
        const a = amplitude.value;
        // Generate 5 bar heights from amplitude with variation
        return [0.2 + a * 0.5, 0.15 + a * 0.8, 0.2 + a * 1.0, 0.15 + a * 0.7, 0.2 + a * 0.4];
    });

    // --- Transient state helpers ---

    function showTransient(state: 'success' | 'error', durationMs = 1500) {
        if (transientTimer) clearTimeout(transientTimer);
        transientState.value = state;
        transientTimer = setTimeout(() => {
            transientState.value = null;
        }, durationMs);
    }

    function showMiniBadgeBriefly(state: 'processing' | 'done', durationMs = 1000) {
        if (miniBadgeTimer) clearTimeout(miniBadgeTimer);
        miniBadgeState.value = state;
        if (state === 'done') {
            miniBadgeTimer = setTimeout(() => {
                miniBadgeState.value = null;
            }, durationMs);
        }
    }

    // --- Dev: suppress devtools/Vite overlay (controlled from Settings via Tauri event) ---

    let devOverlayObserver: MutationObserver | null = null;

    function startDevSuppression() {
        if (devOverlayObserver) return;

        // Remove existing dev elements
        removeDevElements();

        // Watch for new ones
        devOverlayObserver = new MutationObserver(mutations => {
            for (const mutation of mutations) {
                for (const node of mutation.addedNodes) {
                    if (node instanceof HTMLElement) {
                        const tag = node.tagName;
                        // Vite HMR error overlay (Shadow DOM, only .close() works)
                        if (tag === 'VITE-ERROR-OVERLAY') {
                            (node as HTMLElement & { close: () => void }).close();
                        }
                        // vite-plugin-checker error/warning counter
                        if (tag === 'VITE-PLUGIN-CHECKER-ERROR-OVERLAY') {
                            node.remove();
                        }
                        // Nuxt devtools container
                        if (node.id?.includes('nuxt-devtools') || tag === 'NUXT-DEVTOOLS') {
                            node.remove();
                        }
                    }
                }
            }
        });
        devOverlayObserver.observe(document.body, { childList: true, subtree: false });
    }

    function stopDevSuppression() {
        devOverlayObserver?.disconnect();
        devOverlayObserver = null;
    }

    function removeDevElements() {
        // Vite error overlay
        document.querySelectorAll('vite-error-overlay').forEach(el => {
            (el as HTMLElement & { close: () => void }).close();
        });
        // vite-plugin-checker overlay
        document.querySelectorAll('vite-plugin-checker-error-overlay').forEach(el => {
            el.remove();
        });
        // Nuxt devtools
        document.querySelectorAll('[id*="nuxt-devtools"], nuxt-devtools').forEach(el => {
            el.remove();
        });
    }

    // --- Event listeners ---

    onMounted(async () => {
        // Default: suppress dev overlays on indicator (can be toggled from Settings)
        if (import.meta.dev) {
            startDevSuppression();
        }

        await Promise.all([
            listen<AudioStateEvent>('audio-state-changed', event => {
                mode.value = event.payload.mode;
            }),

            // Transcription complete → checkmark
            listen('transcription', () => {
                showTransient('success');
            }),

            listen<number>('vad-amplitude', event => {
                amplitude.value = Math.min(event.payload, 1.0);
            }),

            listen<SubcommandMatchEvent>('subcommand-match', () => {
                showTransient('success');
            }),

            listen('subcommand-timeout', () => {
                // Timeout is not an error, just return to standby silently
            }),

            listen('audio-error', () => {
                showTransient('error', 2000);
            }),

            // Batch processing events during dictation
            listen('transcription-batch-start', () => {
                showMiniBadgeBriefly('processing');
            }),

            listen('transcription-batch-done', () => {
                showMiniBadgeBriefly('done');
            }),

            // Dev: toggle dev overlay suppression from Settings
            listen<boolean>('overlay-suppress-devtools', event => {
                if (event.payload) {
                    startDevSuppression();
                } else {
                    stopDevSuppression();
                }
            }),

            // Dev: toggle click-through from Settings
            listen<boolean>('overlay-set-click-through', async event => {
                try {
                    await getCurrentWindow().setIgnoreCursorEvents(event.payload);
                } catch (e) {
                    console.warn('Failed to set click-through:', e);
                }
            }),
        ]);
    });

    onBeforeUnmount(() => {
        stopDevSuppression();
    });
</script>

<style>
    html,
    body {
        background: transparent !important;
        margin: 0;
        padding: 0;
    }
</style>

<style scoped>
    /* --- Base indicator --- */

    .indicator {
        position: relative;
        width: 48px;
        height: 48px;
        border-radius: 50%;
        display: flex;
        align-items: center;
        justify-content: center;
        color: white;
        transition:
            background-color 300ms ease,
            transform 300ms ease;
        will-change: transform, opacity;
    }

    /* --- Mode colors --- */

    .indicator.standby {
        background-color: #9ca3af;
        animation: breathe 3s ease-in-out infinite;
    }

    .indicator.dictation {
        background-color: var(--color-creo-600, #3d6d85);
    }

    .indicator.awaiting {
        background-color: var(--color-creo-600, #3d6d85);
    }

    .indicator.processing {
        background-color: #d97706;
    }

    .indicator.success {
        background-color: #22c55e;
    }

    .indicator.error {
        background-color: #ef4444;
    }

    /* --- Standby: breathing glow --- */

    .standby-dot {
        width: 8px;
        height: 8px;
        border-radius: 50%;
        background-color: rgba(255, 255, 255, 0.6);
    }

    @keyframes breathe {
        0%,
        100% {
            transform: scale(1);
            opacity: 0.85;
        }
        50% {
            transform: scale(1.06);
            opacity: 1;
        }
    }

    /* --- Dictation: waveform bars --- */

    .waveform {
        display: flex;
        align-items: center;
        gap: 3px;
        height: 24px;
    }

    .waveform-bar {
        width: 3px;
        height: 100%;
        background-color: rgba(255, 255, 255, 0.9);
        border-radius: 1.5px;
        transform-origin: center;
        transition: transform 80ms ease-out;
        will-change: transform;
    }

    /* --- AwaitingSubcommand: ⌘ icon --- */

    .command-icon {
        width: 22px;
        height: 22px;
        color: rgba(255, 255, 255, 0.85);
        animation: gentle-pulse 2s ease-in-out infinite;
    }

    @keyframes gentle-pulse {
        0%,
        100% {
            opacity: 0.7;
        }
        50% {
            opacity: 1;
        }
    }

    /* --- Processing: conic-gradient ring --- */

    .processing-ring {
        width: 32px;
        height: 32px;
        border-radius: 50%;
        background: conic-gradient(
            rgba(255, 255, 255, 0) 0deg,
            rgba(255, 255, 255, 0.8) 120deg,
            rgba(255, 255, 255, 0) 240deg,
            rgba(255, 255, 255, 0.4) 360deg
        );
        mask: radial-gradient(circle, transparent 40%, black 42%);
        -webkit-mask: radial-gradient(circle, transparent 40%, black 42%);
        animation: spin 1.2s linear infinite;
        will-change: transform;
    }

    @keyframes spin {
        to {
            transform: rotate(360deg);
        }
    }

    /* --- Transient icons --- */

    .transient-icon {
        position: absolute;
        width: 28px;
        height: 28px;
    }

    .success-icon {
        color: white;
    }

    .error-icon {
        color: white;
    }

    .draw-circle {
        stroke-dasharray: 63;
        stroke-dashoffset: 63;
        animation: draw 0.5s ease forwards;
    }

    .draw-check {
        stroke-dasharray: 20;
        stroke-dashoffset: 20;
        animation: draw 0.3s ease forwards 0.4s;
    }

    .draw-x {
        stroke-dasharray: 20;
        stroke-dashoffset: 20;
        animation: draw 0.3s ease forwards 0.3s;
    }

    @keyframes draw {
        to {
            stroke-dashoffset: 0;
        }
    }

    /* --- Error shake --- */

    .shake {
        animation: shake 0.4s ease;
    }

    @keyframes shake {
        0%,
        100% {
            transform: translateX(0);
        }
        20% {
            transform: translateX(-4px);
        }
        40% {
            transform: translateX(4px);
        }
        60% {
            transform: translateX(-3px);
        }
        80% {
            transform: translateX(3px);
        }
    }

    /* --- Mini-badge --- */

    .mini-badge {
        position: absolute;
        bottom: -2px;
        right: -2px;
        width: 18px;
        height: 18px;
        border-radius: 50%;
        background-color: white;
        box-shadow: 0 0 0 2px var(--color-creo-600, #3d6d85);
        display: flex;
        align-items: center;
        justify-content: center;
    }

    .mini-badge.processing {
        background-color: white;
    }

    .mini-badge.done {
        background-color: #22c55e;
        box-shadow: 0 0 0 2px #22c55e;
    }

    .mini-spinner {
        width: 10px;
        height: 10px;
        border: 2px solid #e5e7eb;
        border-top-color: var(--color-creo-600, #3d6d85);
        border-radius: 50%;
        animation: spin 0.8s linear infinite;
    }

    .mini-check {
        width: 12px;
        height: 12px;
        color: white;
    }
</style>
