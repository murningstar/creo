<template>
    <div class="flex grow flex-col gap-4 overflow-y-auto p-4">
        <!-- Models missing banner -->
        <div
            v-if="audioStore.modelStatus && !audioStore.modelStatus.allPresent"
            class="rounded-lg border border-amber-200 bg-amber-50 p-4 dark:border-amber-800 dark:bg-amber-900/20"
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
                    <span class="text-amber-700 dark:text-amber-300">{{ model.filename }}</span>
                    <span class="text-amber-500 dark:text-amber-400">({{ model.sizeHint }})</span>
                </li>
            </ul>
        </div>

        <!-- Status bar -->
        <div class="flex items-center gap-4">
            <div class="relative">
                <div
                    v-if="!audioStore.isIdle"
                    class="absolute -inset-2 animate-ping rounded-full opacity-20"
                    :class="pulseColor"
                />
                <div
                    class="relative flex size-12 items-center justify-center rounded-full transition-colors duration-300"
                    :class="circleColor"
                >
                    <u-icon :name="stateIcon" class="size-5 text-white" />
                </div>
            </div>
            <div class="flex-1">
                <p class="text-sm font-medium">{{ stateLabel }}</p>
                <p class="text-xs text-neutral-500">{{ stateDescription }}</p>
            </div>
            <u-button
                v-if="audioStore.isIdle"
                size="sm"
                color="primary"
                :disabled="!canStart"
                @click="audioStore.startListening()"
            >
                Start
            </u-button>
            <u-button v-else size="sm" color="neutral" variant="outline" @click="audioStore.stopListening()">
                Stop
            </u-button>
        </div>

        <!-- Error display -->
        <div v-if="audioStore.error" class="rounded-lg bg-red-50 p-3 dark:bg-red-900/20">
            <p class="text-sm text-red-600 dark:text-red-400">{{ audioStore.error }}</p>
        </div>

        <!-- Two-column layout -->
        <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
            <!-- Voice Commands -->
            <div class="rounded-lg border border-neutral-200 p-4 dark:border-neutral-700">
                <div class="mb-3 flex items-center justify-between">
                    <p class="text-sm font-medium">Voice Commands</p>
                    <u-button v-if="!isCreating" size="xs" variant="soft" @click="startCreating"> + New </u-button>
                </div>

                <!-- Command list -->
                <div v-if="wakeStore.hasCommands" class="space-y-2">
                    <div
                        v-for="cmd in wakeStore.commands"
                        :key="cmd.name"
                        class="flex items-center justify-between rounded-md bg-neutral-50 px-3 py-2 dark:bg-neutral-800"
                    >
                        <div>
                            <p class="text-sm font-medium">{{ cmd.name }}</p>
                            <p
                                class="text-xs"
                                :class="
                                    cmd.sampleCount >= 3
                                        ? 'text-green-600 dark:text-green-400'
                                        : 'text-amber-600 dark:text-amber-400'
                                "
                            >
                                {{ cmd.sampleCount }}/3 samples
                                <span v-if="cmd.sampleCount < 3"> — needs more</span>
                            </p>
                        </div>
                        <div class="flex gap-1">
                            <u-button size="xs" variant="ghost" color="primary" @click="openEditor(cmd.name)">
                                Edit
                            </u-button>
                            <u-button
                                size="xs"
                                variant="ghost"
                                color="error"
                                @click="wakeStore.deleteCommand(cmd.name)"
                            >
                                Delete
                            </u-button>
                        </div>
                    </div>
                </div>

                <p v-if="!wakeStore.hasCommands && !isCreating" class="py-4 text-center text-sm text-neutral-500">
                    No commands yet. Create one to start.
                </p>

                <!-- Create / Edit command -->
                <div
                    v-if="isCreating"
                    class="mt-3 space-y-3 rounded-md border border-neutral-200 p-3 dark:border-neutral-600"
                >
                    <!-- Command name input (only for new) -->
                    <div v-if="!editingExisting">
                        <label class="mb-1 block text-xs font-medium text-neutral-600 dark:text-neutral-400">
                            Command name
                        </label>
                        <u-input v-model="newCommandName" size="sm" placeholder="e.g. Приём" />

                        <label class="mt-2 mb-1 block text-xs font-medium text-neutral-600 dark:text-neutral-400">
                            Action
                        </label>
                        <select
                            v-model="selectedAction"
                            class="w-full rounded-md border border-neutral-300 bg-white px-2 py-1.5 text-sm dark:border-neutral-600 dark:bg-neutral-800"
                        >
                            <option v-for="opt in WAKE_ACTION_OPTIONS" :key="opt.value" :value="opt.value">
                                {{ opt.label }} — {{ opt.description }}
                            </option>
                        </select>
                    </div>
                    <p v-else class="text-sm font-medium">{{ newCommandName }}</p>

                    <!-- Recording section -->
                    <div v-if="newCommandName.trim()">
                        <p class="mb-2 text-xs text-neutral-500">
                            <template v-if="currentSamples.length === 0">
                                Say "{{ newCommandName }}" clearly when ready.
                            </template>
                            <template v-else-if="currentSamples.length < 3">
                                {{ 3 - currentSamples.length }} more sample{{
                                    3 - currentSamples.length > 1 ? 's' : ''
                                }}
                                needed.
                            </template>
                            <template v-else> Enough samples recorded. You can add more for better accuracy. </template>
                        </p>

                        <!-- Sample list -->
                        <!-- TODO: add audio playback (inline waveform + play button) per sample -->
                        <div v-if="currentSamples.length > 0" class="mb-2 space-y-1">
                            <div
                                v-for="(sample, idx) in currentSamples"
                                :key="idx"
                                class="flex items-center gap-2 rounded bg-neutral-50 px-2 py-1 text-xs dark:bg-neutral-800"
                            >
                                <span class="font-medium text-neutral-600 dark:text-neutral-400"> #{{ idx + 1 }} </span>
                                <span class="flex-1 text-neutral-500"> {{ sample.embeddingCount }} embeddings </span>
                                <!-- TODO: play button + waveform here -->
                            </div>
                        </div>

                        <!-- Record button -->
                        <u-button
                            size="sm"
                            :color="wakeStore.isRecording ? 'error' : 'primary'"
                            :disabled="wakeStore.isRecording"
                            @click="recordSample"
                        >
                            <template v-if="wakeStore.isRecording">
                                <u-icon name="i-heroicons-microphone" class="mr-1 size-4 animate-pulse" />
                                Listening...
                            </template>
                            <template v-else>
                                <u-icon name="i-heroicons-microphone" class="mr-1 size-4" />
                                Record Sample
                            </template>
                        </u-button>
                    </div>

                    <p v-if="wakeStore.error" class="text-xs text-red-500">{{ wakeStore.error }}</p>

                    <!-- Actions -->
                    <div class="flex justify-end gap-2 border-t border-neutral-200 pt-3 dark:border-neutral-600">
                        <u-button size="xs" variant="ghost" @click="cancelCreating">Cancel</u-button>
                        <u-button
                            size="xs"
                            color="primary"
                            :disabled="currentSamples.length === 0"
                            @click="saveCommand"
                        >
                            Save
                        </u-button>
                    </div>
                </div>
            </div>

            <!-- Dictation History -->
            <div class="rounded-lg border border-neutral-200 p-4 dark:border-neutral-700">
                <p class="mb-3 text-sm font-medium">Dictation History</p>

                <div v-if="audioStore.lastTranscription" class="rounded-md bg-neutral-50 p-3 dark:bg-neutral-800">
                    <p class="text-sm text-neutral-600 dark:text-neutral-300">
                        {{ audioStore.lastTranscription }}
                    </p>
                </div>

                <p v-else class="py-4 text-center text-sm text-neutral-500">No dictations yet.</p>
            </div>
        </div>

        <!-- Dev controls -->
        <div v-if="isDev" class="rounded-lg border border-dashed border-neutral-300 p-3 dark:border-neutral-600">
            <p class="mb-2 text-xs font-medium tracking-wide text-neutral-400 uppercase">Dev Controls</p>
            <div class="flex flex-wrap gap-2">
                <u-button
                    v-for="m in modes"
                    :key="m.value"
                    size="xs"
                    :variant="audioStore.mode === m.value ? 'solid' : 'outline'"
                    :color="audioStore.mode === m.value ? 'primary' : 'neutral'"
                    @click="audioStore.__devSetMode(m.value)"
                >
                    {{ m.label }}
                </u-button>
                <u-button size="xs" variant="soft" color="neutral" @click="runTestCapture"> Test Capture </u-button>
            </div>
            <p v-if="testResult" class="mt-2 text-xs whitespace-pre-wrap text-neutral-500">{{ testResult }}</p>
        </div>

        <div class="text-center text-xs text-neutral-400">{{ platformLabel }}</div>
    </div>
</template>

<script setup lang="ts">
    import { AudioMode, useAudioStore } from '~/entities/audio';
    import { usePlatformStore } from '~/entities/platform';
    import { useSettingsStore } from '~/entities/settings';
    import {
        useWakeCommandsStore,
        WAKE_ACTION_OPTIONS,
        type RecordResult,
        type WakeActionType,
    } from '~/entities/wake-commands';

    const audioStore = useAudioStore();
    const platformStore = usePlatformStore();
    const settingsStore = useSettingsStore();
    const wakeStore = useWakeCommandsStore();

    const isDev = import.meta.dev;
    const testResult = ref<string | null>(null);

    // Command creation state
    const isCreating = ref(false);
    const editingExisting = ref(false);
    const newCommandName = ref('');
    const selectedAction = ref<WakeActionType>('command_mode');
    const currentSamples = ref<RecordResult[]>([]);

    const modes = [
        { value: AudioMode.Idle, label: 'Idle' },
        { value: AudioMode.Listening, label: 'Listening' },
        { value: AudioMode.Dictation, label: 'Dictation' },
        { value: AudioMode.Processing, label: 'Processing' },
    ] as const;

    const canStart = computed(() => {
        const modelsOk = !audioStore.modelStatus || audioStore.modelStatus.allPresent;
        return modelsOk;
    });

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
                    description: 'Say stop command to finish',
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
        if (platformStore.isNativePlatform) return `Platform: ${platformStore.currentNativePlatform}`;
        return 'Platform: Web Browser';
    });

    function startCreating() {
        isCreating.value = true;
        editingExisting.value = false;
        newCommandName.value = '';
        selectedAction.value = 'command_mode';
        currentSamples.value = [];
    }

    function openEditor(commandName: string) {
        isCreating.value = true;
        editingExisting.value = true;
        newCommandName.value = commandName;
        // Load existing sample count
        const cmd = wakeStore.commands.find(c => c.name === commandName);
        currentSamples.value = Array.from({ length: cmd?.sampleCount ?? 0 }, (_, i) => ({
            commandName,
            embeddingCount: 0, // Unknown for existing samples
            totalSamples: cmd?.sampleCount ?? 0,
            path: `sample_${i}`,
        }));
    }

    async function recordSample() {
        const name = newCommandName.value.trim();
        if (!name) return;
        // Pass action only on first sample (saves mapping once)
        const action = currentSamples.value.length === 0 ? selectedAction.value : undefined;
        const result = await wakeStore.recordSample(name, action);
        if (result) {
            currentSamples.value.push(result);
        }
    }

    function cancelCreating() {
        isCreating.value = false;
        editingExisting.value = false;
        newCommandName.value = '';
        currentSamples.value = [];
    }

    function saveCommand() {
        // Samples are already saved on disk by record_wake_sample.
        // Just close the editor and refresh.
        wakeStore.loadCommands();
        cancelCreating();
    }

    async function runTestCapture() {
        testResult.value = 'Capturing...';
        testResult.value = await audioStore.testCapture();
    }

    onMounted(async () => {
        if (platformStore.isNativePlatform) {
            await settingsStore.init();
            audioStore.checkModels();
            audioStore.setupEventListeners();
            wakeStore.loadCommands();
        }
    });

    onUnmounted(() => {
        audioStore.cleanup();
    });
</script>
