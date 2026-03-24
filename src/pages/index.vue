<template>
    <div class="flex grow flex-col gap-8 overflow-y-auto p-4">
        <!-- Models missing alert -->
        <u-alert
            v-if="audioStore.modelStatus && !audioStore.modelStatus.allPresent"
            icon="i-lucide-alert-triangle"
            color="warning"
            variant="soft"
            title="Models required"
        >
            <template #description>
                <p class="mb-2">Place model files in:</p>
                <code class="bg-muted block rounded px-2 py-1 text-xs break-all">
                    {{ audioStore.modelStatus.modelsDir }}
                </code>
                <ul class="mt-2 space-y-1">
                    <li
                        v-for="model in audioStore.modelStatus.models"
                        :key="model.filename"
                        class="flex items-center gap-2 text-xs"
                    >
                        <u-badge
                            :color="model.exists ? 'success' : 'error'"
                            :variant="model.exists ? 'soft' : 'solid'"
                            :label="model.exists ? 'OK' : 'Missing'"
                            size="xs"
                        />
                        <span>{{ model.filename }}</span>
                        <span class="text-dimmed">({{ model.sizeHint }})</span>
                    </li>
                </ul>
            </template>
        </u-alert>

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
                <p class="text-highlighted text-sm font-medium">{{ stateLabel }}</p>
                <p class="text-muted text-xs">{{ stateDescription }}</p>
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

        <!-- Error alert -->
        <u-alert
            v-if="audioStore.error"
            icon="i-lucide-circle-x"
            color="error"
            variant="soft"
            :description="audioStore.error"
        />

        <!-- Two-column layout -->
        <div class="grid grid-cols-1 gap-8 md:grid-cols-2">
            <!-- Voice Commands -->
            <section class="shadow-card rounded-lg bg-white p-5 dark:bg-neutral-900">
                <div class="mb-4 flex items-center justify-between">
                    <h2 class="text-highlighted text-sm font-semibold">Voice Commands</h2>
                    <u-button v-if="!isCreating" size="xs" variant="soft" @click="startCreating">+ New</u-button>
                </div>

                <!-- Command list -->
                <div v-if="wakeStore.hasCommands" class="space-y-2">
                    <div
                        v-for="cmd in wakeStore.commands"
                        :key="cmd.name"
                        class="bg-muted flex items-center justify-between rounded-md px-3 py-2"
                    >
                        <div>
                            <p class="text-sm font-medium">{{ cmd.name }}</p>
                            <div class="mt-0.5">
                                <u-badge
                                    :color="cmd.sampleCount >= 3 ? 'success' : 'warning'"
                                    variant="soft"
                                    :label="`${cmd.sampleCount}/3 samples${cmd.sampleCount < 3 ? ' — needs more' : ''}`"
                                    size="xs"
                                />
                            </div>
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

                <p v-if="!wakeStore.hasCommands && !isCreating" class="text-muted py-4 text-center text-sm">
                    No commands yet. Create one to start.
                </p>

                <!-- Create / Edit command -->
                <div v-if="isCreating" class="bg-muted mt-4 space-y-4 rounded-md p-4">
                    <div v-if="!editingExisting">
                        <u-form-field label="Command name" class="mb-3">
                            <u-input v-model="newCommandName" size="sm" placeholder="e.g. Приём" />
                        </u-form-field>

                        <u-form-field label="Action">
                            <u-select v-model="selectedAction" :items="actionOptions" />
                        </u-form-field>
                    </div>
                    <p v-else class="text-highlighted text-sm font-medium">{{ newCommandName }}</p>

                    <!-- Recording section -->
                    <div v-if="newCommandName.trim()">
                        <p class="text-muted mb-2 text-xs">
                            <template v-if="currentSamples.length === 0">
                                Say "{{ newCommandName }}" clearly when ready.
                            </template>
                            <template v-else-if="currentSamples.length < 3">
                                {{ 3 - currentSamples.length }} more sample{{
                                    3 - currentSamples.length > 1 ? 's' : ''
                                }}
                                needed.
                            </template>
                            <template v-else>Enough samples recorded. You can add more for better accuracy.</template>
                        </p>

                        <!-- Sample list -->
                        <!-- TODO: add audio playback (inline waveform + play button) per sample -->
                        <div v-if="currentSamples.length > 0" class="mb-3 space-y-1">
                            <div
                                v-for="(sample, idx) in currentSamples"
                                :key="idx"
                                class="bg-elevated flex items-center gap-2 rounded px-2 py-1 text-xs"
                            >
                                <u-badge color="neutral" variant="subtle" :label="`#${idx + 1}`" size="xs" />
                                <span class="text-dimmed flex-1">{{ sample.embeddingCount }} embeddings</span>
                            </div>
                        </div>

                        <u-button
                            size="sm"
                            :color="wakeStore.isRecording ? 'error' : 'primary'"
                            :disabled="wakeStore.isRecording"
                            :icon="wakeStore.isRecording ? 'i-lucide-mic' : 'i-lucide-mic'"
                            @click="recordSample"
                        >
                            {{ wakeStore.isRecording ? 'Listening...' : 'Record Sample' }}
                        </u-button>
                    </div>

                    <u-alert
                        v-if="wakeStore.error"
                        icon="i-lucide-circle-x"
                        color="error"
                        variant="soft"
                        :description="wakeStore.error"
                        class="mt-2"
                    />

                    <div class="border-default flex justify-end gap-2 border-t pt-3">
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
            </section>

            <!-- Dictation History -->
            <section class="shadow-card rounded-lg bg-white p-5 dark:bg-neutral-900">
                <h2 class="text-highlighted mb-4 text-sm font-semibold">Dictation History</h2>

                <div v-if="audioStore.lastTranscription" class="bg-muted rounded-md p-3">
                    <p class="text-sm">{{ audioStore.lastTranscription }}</p>
                </div>

                <p v-else class="text-muted py-4 text-center text-sm">No dictations yet.</p>
            </section>
        </div>

        <!-- Dev controls -->
        <div v-if="isDev" class="border-accented rounded-lg border border-dashed p-3">
            <p class="text-dimmed mb-2 text-xs font-medium tracking-wide uppercase">Dev Controls</p>
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
                <u-button size="xs" variant="soft" color="neutral" @click="runTestCapture">Test Capture</u-button>
            </div>
            <p v-if="testResult" class="text-dimmed mt-2 text-xs whitespace-pre-wrap">{{ testResult }}</p>
        </div>

        <div class="text-dimmed text-center text-xs">{{ platformLabel }}</div>
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

    const actionOptions = WAKE_ACTION_OPTIONS.map(opt => ({
        label: `${opt.label} — ${opt.description}`,
        value: opt.value,
    }));

    const canStart = computed(() => !audioStore.modelStatus || audioStore.modelStatus.allPresent);

    const stateConfig = computed(() => {
        switch (audioStore.mode) {
            case AudioMode.Listening:
                return {
                    icon: 'i-lucide-mic',
                    label: 'Listening...',
                    description: 'Waiting for wake word',
                    circle: 'bg-blue-500',
                    pulse: 'bg-blue-500',
                };
            case AudioMode.Dictation:
                return {
                    icon: 'i-lucide-pencil',
                    label: 'Dictation',
                    description: 'Say stop command to finish',
                    circle: 'bg-green-500',
                    pulse: 'bg-green-500',
                };
            case AudioMode.Processing:
                return {
                    icon: 'i-lucide-loader',
                    label: 'Processing...',
                    description: 'Recognizing speech',
                    circle: 'bg-amber-500',
                    pulse: 'bg-amber-500',
                };
            default:
                return {
                    icon: 'i-lucide-audio-lines',
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
        const cmd = wakeStore.commands.find(c => c.name === commandName);
        currentSamples.value = Array.from({ length: cmd?.sampleCount ?? 0 }, (_, i) => ({
            commandName,
            embeddingCount: 0,
            totalSamples: cmd?.sampleCount ?? 0,
            path: `sample_${i}`,
        }));
    }

    async function recordSample() {
        const name = newCommandName.value.trim();
        if (!name) return;
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
