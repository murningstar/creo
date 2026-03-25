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
                <h2 class="text-highlighted mb-4 text-sm font-semibold">Voice Commands</h2>

                <ActionList label="New command" @add="openCreateModal">
                    <div
                        v-for="cmd in wakeStore.commands"
                        :key="cmd.name"
                        class="bg-muted flex items-center justify-between rounded-md px-3 py-2"
                    >
                        <div>
                            <p class="text-sm font-medium">{{ cmd.name }}</p>
                            <div class="mt-0.5">
                                <u-badge
                                    :color="cmd.sampleCount >= REQUIRED_SAMPLES ? 'success' : 'warning'"
                                    variant="soft"
                                    :label="`${cmd.sampleCount}/${REQUIRED_SAMPLES} samples${cmd.sampleCount < REQUIRED_SAMPLES ? ' — needs more' : ''}`"
                                    size="xs"
                                />
                            </div>
                        </div>
                        <div class="flex gap-1">
                            <u-button size="xs" variant="ghost" color="primary" @click="openEditModal(cmd.name)">
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
                </ActionList>
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

        <!-- Command create/edit modal -->
        <u-modal v-model:open="modalOpen" :title="modalTitle" :ui="{ footer: 'justify-end' }">
            <template #body>
                <!-- Step 1: Name + Action (create only) -->
                <div v-if="modalStep === 'setup'" class="space-y-4">
                    <u-form-field label="Command name">
                        <u-input v-model="commandName" size="sm" placeholder="e.g. Приём" autofocus />
                    </u-form-field>
                    <u-form-field label="Action">
                        <u-select v-model="commandAction" :items="actionOptions" />
                    </u-form-field>
                </div>

                <!-- Step 2: Recording -->
                <div v-else-if="modalStep === 'recording'">
                    <RecordingFlow :commands="recordingCommands" @complete="onRecordingDone" />
                </div>

                <!-- Step 3: Done -->
                <div v-else-if="modalStep === 'done'" class="space-y-2 text-center">
                    <u-icon name="i-lucide-check-circle" class="text-primary mx-auto size-10" />
                    <p class="text-muted text-sm">Command "{{ commandName }}" is ready.</p>
                </div>
            </template>

            <template #footer="{ close }">
                <template v-if="modalStep === 'setup'">
                    <u-button variant="outline" size="sm" @click="close">Cancel</u-button>
                    <u-button
                        color="primary"
                        size="sm"
                        :disabled="!commandName.trim()"
                        @click="modalStep = 'recording'"
                    >
                        Continue
                    </u-button>
                </template>
                <template v-else-if="modalStep === 'done'">
                    <u-button color="primary" size="sm" @click="finishModal(close)">Done</u-button>
                </template>
            </template>
        </u-modal>

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
    import ActionList from '~/shared/ui/action-list/action-list.vue';
    import { RecordingFlow, type RecordingCommand } from '~/features/recording-flow';
    import { AudioMode, useAudioStore } from '~/entities/audio';
    import { usePlatformStore } from '~/entities/platform';
    import {
        useWakeCommandsStore,
        WAKE_ACTION_OPTIONS,
        REQUIRED_SAMPLES,
        type WakeActionType,
    } from '~/entities/wake-commands';

    const audioStore = useAudioStore();
    const platformStore = usePlatformStore();
    const wakeStore = useWakeCommandsStore();

    const isDev = import.meta.dev;
    const testResult = ref<string | null>(null);

    // Modal state
    const modalOpen = ref(false);
    const modalStep = ref<'setup' | 'recording' | 'done'>('setup');
    const commandName = ref('');
    const commandAction = ref<WakeActionType>('command_mode');
    const isEditing = ref(false);

    const modalTitle = computed(() => (isEditing.value ? `Edit: ${commandName.value}` : 'New Command'));

    const actionOptions = WAKE_ACTION_OPTIONS.map(opt => ({
        label: `${opt.label} — ${opt.description}`,
        value: opt.value,
    }));

    const recordingCommands = computed<RecordingCommand[]>(() => [
        {
            name: commandName.value.trim(),
            label: commandName.value.trim(),
            action: commandAction.value,
            requiredSamples: REQUIRED_SAMPLES,
        },
    ]);

    const modes = [
        { value: AudioMode.Idle, label: 'Idle' },
        { value: AudioMode.Listening, label: 'Listening' },
        { value: AudioMode.Dictation, label: 'Dictation' },
        { value: AudioMode.Processing, label: 'Processing' },
    ] as const;

    const platformLabel = computed(() => {
        if (platformStore.isNativePlatform) return `Platform: ${platformStore.currentNativePlatform}`;
        return 'Platform: Web Browser';
    });

    function openCreateModal() {
        commandName.value = '';
        commandAction.value = 'command_mode';
        isEditing.value = false;
        modalStep.value = 'setup';
        modalOpen.value = true;
    }

    function openEditModal(name: string) {
        commandName.value = name;
        isEditing.value = true;
        modalStep.value = 'recording';
        modalOpen.value = true;
    }

    function onRecordingDone() {
        modalStep.value = 'done';
    }

    async function finishModal(close: () => void) {
        await wakeStore.loadCommands();
        close();
    }

    async function runTestCapture() {
        testResult.value = 'Capturing...';
        testResult.value = await audioStore.testCapture();
    }

    onMounted(() => {
        if (platformStore.isNativePlatform) {
            wakeStore.loadCommands();
        }
    });
</script>
