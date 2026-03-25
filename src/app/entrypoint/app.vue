<template>
    <u-app>
        <nuxt-layout>
            <template #voice-status>
                <div v-if="platformStore.isNativePlatform" class="flex items-center gap-3">
                    <div class="relative">
                        <div
                            v-if="!audioStore.isIdle"
                            class="absolute -inset-1.5 animate-ping rounded-full opacity-20"
                            :class="stateConfig.pulse"
                        />
                        <CreoLogo
                            class="relative size-8 transition-colors duration-300"
                            :class="stateConfig.logoColor"
                        />
                    </div>
                    <div>
                        <p class="text-highlighted text-sm leading-tight font-semibold">{{ stateConfig.label }}</p>
                        <p class="text-muted text-xs leading-tight">{{ stateConfig.description }}</p>
                    </div>
                    <u-button
                        v-if="audioStore.isIdle"
                        size="xs"
                        color="primary"
                        :disabled="!canStart"
                        @click="audioStore.startListening()"
                    >
                        Start
                    </u-button>
                    <u-button v-else size="xs" color="neutral" variant="outline" @click="audioStore.stopListening()">
                        Stop
                    </u-button>
                </div>
                <h1 v-else class="text-lg font-semibold">{{ settingsStore.assistantName }}</h1>
            </template>

            <nuxt-page />
        </nuxt-layout>
    </u-app>
</template>

<script setup lang="ts">
    import CreoLogo from '~/shared/ui/icons/ui/i-creo-logo.vue';
    import { AudioMode, useAudioStore } from '~/entities/audio';
    import { usePlatformStore } from '~/entities/platform';
    import { useSettingsStore } from '~/entities/settings';

    useHead({ title: 'Creo' });

    const audioStore = useAudioStore();
    const platformStore = usePlatformStore();
    const settingsStore = useSettingsStore();

    const canStart = computed(() => !audioStore.modelStatus || audioStore.modelStatus.allPresent);

    const stateConfig = computed(() => {
        switch (audioStore.mode) {
            case AudioMode.Listening:
                return {
                    label: 'Listening...',
                    description: 'Waiting for wake word',
                    logoColor: 'text-blue-500',
                    pulse: 'bg-blue-500',
                };
            case AudioMode.Dictation:
                return {
                    label: 'Dictation',
                    description: 'Say stop command to finish',
                    logoColor: 'text-green-500',
                    pulse: 'bg-green-500',
                };
            case AudioMode.Processing:
                return {
                    label: 'Processing...',
                    description: 'Recognizing speech',
                    logoColor: 'text-amber-500',
                    pulse: 'bg-amber-500',
                };
            default:
                return {
                    label: settingsStore.assistantName,
                    description: 'Voice assistant',
                    logoColor: 'text-neutral-700',
                    pulse: '',
                };
        }
    });
</script>
