<template>
    <div class="flex grow flex-col items-center justify-center gap-6 p-6">
        <!-- Models missing banner -->
        <div
            v-if="audioStore.modelStatus && !audioStore.modelStatus.allPresent"
            class="w-full max-w-xs rounded-lg border border-amber-200 bg-amber-50 p-4 dark:border-amber-800 dark:bg-amber-900/20"
        >
            <p class="mb-2 text-sm font-medium text-amber-800 dark:text-amber-200">Models required</p>
            <p class="mb-2 text-xs text-amber-700 dark:text-amber-300">Place model files in:</p>
            <code class="mb-3 block rounded bg-amber-100 px-2 py-1 text-xs break-all dark:bg-amber-900/40">
                {{ audioStore.modelStatus.modelsDir }}
            </code>
            <ul class="space-y-1">
                <li
                    v-for="model in audioStore.modelStatus.models"
                    :key="model.filename"
                    class="flex items-center gap-2 text-xs"
                >
                    <span
                        :class="model.exists ? 'text-green-600 dark:text-green-400' : 'text-red-600 dark:text-red-400'"
                    >
                        {{ model.exists ? 'OK' : 'Missing' }}
                    </span>
                    <span class="text-amber-700 dark:text-amber-300">
                        {{ model.filename }}
                    </span>
                    <span class="text-amber-500 dark:text-amber-400"> ({{ model.sizeHint }}) </span>
                </li>
            </ul>
        </div>

        <!-- Status indicator -->
        <div class="relative flex items-center justify-center">
            <!-- Pulse ring (visible when not idle) -->
            <div
                v-if="!audioStore.isIdle"
                class="absolute size-24 animate-ping rounded-full opacity-20"
                :class="pulseColor"
            />
            <!-- VAD speech indicator -->
            <div
                v-if="audioStore.isSpeech && !audioStore.isIdle"
                class="absolute size-28 animate-pulse rounded-full bg-white/10"
            />
            <!-- Main circle -->
            <div
                class="relative flex size-20 items-center justify-center rounded-full transition-colors duration-300"
                :class="circleColor"
            >
                <u-icon :name="stateIcon" class="size-8 text-white" />
            </div>
        </div>

        <!-- State label -->
        <div class="text-center">
            <p class="text-lg font-medium">{{ stateLabel }}</p>
            <p class="mt-1 text-sm text-neutral-500">{{ stateDescription }}</p>
        </div>

        <!-- Transcription display -->
        <div
            v-if="audioStore.lastTranscription"
            class="w-full max-w-xs rounded-lg bg-neutral-100 p-3 dark:bg-neutral-800"
        >
            <p class="text-sm text-neutral-600 dark:text-neutral-300">
                {{ audioStore.lastTranscription }}
            </p>
        </div>

        <!-- Error display -->
        <div v-if="audioStore.error" class="w-full max-w-xs rounded-lg bg-red-50 p-3 dark:bg-red-900/20">
            <p class="text-sm text-red-600 dark:text-red-400">{{ audioStore.error }}</p>
        </div>

        <!-- Controls -->
        <div class="flex gap-3">
            <u-button
                v-if="audioStore.isIdle"
                color="primary"
                :disabled="audioStore.modelStatus && !audioStore.modelStatus.allPresent"
                @click="audioStore.startListening()"
            >
                Start
            </u-button>
            <u-button v-else color="neutral" variant="outline" @click="audioStore.stopListening()"> Stop </u-button>
        </div>

        <!-- Dev controls (only in dev mode) -->
        <div v-if="isDev" class="flex flex-col items-center gap-3">
            <p class="text-xs font-medium tracking-wide text-neutral-400 uppercase">Dev Controls</p>
            <div class="flex flex-wrap justify-center gap-2">
                <u-button
                    v-for="m in modes"
                    :key="m.value"
                    size="sm"
                    :variant="audioStore.mode === m.value ? 'solid' : 'outline'"
                    :color="audioStore.mode === m.value ? 'primary' : 'neutral'"
                    @click="audioStore._setMode(m.value)"
                >
                    {{ m.label }}
                </u-button>
            </div>
            <u-button size="sm" variant="soft" color="neutral" @click="runTestCapture"> Test Capture (3s) </u-button>
            <p v-if="testResult" class="max-w-xs text-xs whitespace-pre-wrap text-neutral-500">
                {{ testResult }}
            </p>
        </div>

        <!-- Platform info -->
        <div class="mt-auto text-center text-xs text-neutral-400">
            <p>{{ platformLabel }}</p>
        </div>
    </div>
</template>

<script setup lang="ts">
    import { AudioMode, useAudioStore } from '~/entities/audio';
    import { usePlatformStore } from '~/entities/platform';

    const audioStore = useAudioStore();
    const platformStore = usePlatformStore();

    const isDev = import.meta.dev;
    const testResult = ref<string | null>(null);

    const modes = [
        { value: AudioMode.Idle, label: 'Idle' },
        { value: AudioMode.Listening, label: 'Listening' },
        { value: AudioMode.Dictation, label: 'Dictation' },
        { value: AudioMode.Processing, label: 'Processing' },
    ] as const;

    const stateConfig = computed(() => {
        switch (audioStore.mode) {
            case AudioMode.Listening:
                return {
                    icon: 'i-heroicons-microphone',
                    label: 'Listening...',
                    description: 'Waiting for wake word',
                    circle: 'bg-blue-500',
                    pulse: 'bg-blue-500',
                };
            case AudioMode.Dictation:
                return {
                    icon: 'i-heroicons-pencil-square',
                    label: 'Dictation',
                    description: 'Say "Creo, gotovo" to finish',
                    circle: 'bg-green-500',
                    pulse: 'bg-green-500',
                };
            case AudioMode.Processing:
                return {
                    icon: 'i-heroicons-arrow-path',
                    label: 'Processing...',
                    description: 'Recognizing speech',
                    circle: 'bg-amber-500',
                    pulse: 'bg-amber-500',
                };
            default:
                return {
                    icon: 'i-heroicons-speaker-wave',
                    label: 'Creo',
                    description: 'Voice assistant',
                    circle: 'bg-neutral-700',
                    pulse: '',
                };
        }
    });

    const stateIcon = computed(() => stateConfig.value.icon);
    const stateLabel = computed(() => stateConfig.value.label);
    const stateDescription = computed(() => stateConfig.value.description);
    const circleColor = computed(() => stateConfig.value.circle);
    const pulseColor = computed(() => stateConfig.value.pulse);

    const platformLabel = computed(() => {
        if (platformStore.isNativePlatform) {
            return `Platform: ${platformStore.currentNativePlatform}`;
        }
        return 'Platform: Web Browser';
    });

    async function runTestCapture() {
        testResult.value = 'Capturing...';
        const result = await audioStore.testCapture();
        testResult.value = result;
    }

    onMounted(() => {
        if (platformStore.isNativePlatform) {
            audioStore.checkModels();
            audioStore.setupEventListeners();
        }
    });

    onUnmounted(() => {
        audioStore.cleanup();
    });
</script>
