<template>
    <div class="flex h-screen w-screen items-center justify-center">
        <div class="indicator-circle" :class="modeClass">
            <span v-if="mode === AudioMode.Standby" class="standby-dot" />
        </div>
    </div>
</template>

<script setup lang="ts">
    import { listen } from '@tauri-apps/api/event';

    import type { AudioStateEvent } from '~/entities/audio';
    import { AudioMode } from '~/entities/audio';

    definePageMeta({
        layout: 'overlay',
    });

    const mode = ref<AudioMode>(AudioMode.Standby);

    const modeClass = computed(() => {
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

    onMounted(async () => {
        await listen<AudioStateEvent>('audio-state-changed', event => {
            mode.value = event.payload.mode;
        });
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
    .indicator-circle {
        width: 48px;
        height: 48px;
        border-radius: 50%;
        display: flex;
        align-items: center;
        justify-content: center;
        transition:
            background-color 300ms ease,
            box-shadow 300ms ease;
    }

    .indicator-circle.standby {
        background-color: #9ca3af;
        animation: breathe 3s ease-in-out infinite;
    }

    .indicator-circle.dictation {
        background-color: var(--color-primary-500, #3d6d85);
    }

    .indicator-circle.processing {
        background-color: var(--color-secondary-500, #6d5a85);
    }

    .indicator-circle.awaiting {
        background-color: var(--color-primary-500, #3d6d85);
    }

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
</style>
