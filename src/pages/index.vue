<template>
    <div class="flex grow flex-col items-center justify-center gap-4">
        <h1 class="text-4xl font-bold">Creo</h1>
        <p class="text-lg text-neutral-500">Voice Assistant</p>
        <p class="text-sm text-neutral-400">Platform: {{ platformLabel }}</p>
    </div>
</template>

<script setup lang="ts">
    import { usePlatformStore } from '~/entities/platform';

    const platformStore = usePlatformStore();

    const platformLabel = computed(() => {
        if (platformStore.isNativePlatform) {
            return `Native (${platformStore.currentNativePlatform})`;
        }
        return 'Web Browser';
    });
</script>
