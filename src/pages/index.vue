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

                <!-- Categorized view: after Wizard/Rename setup -->
                <div v-if="hasSystemSetup" class="space-y-2">
                    <!-- System commands (вписывай, готово, отмена) -->
                    <div
                        v-for="cmd in systemCommands"
                        :key="cmd.name"
                        class="bg-muted flex items-center gap-2 rounded-md px-3 py-2"
                    >
                        <span class="grow text-sm font-medium">{{ cmd.name }}</span>
                        <u-badge
                            :color="cmd.sampleCount >= REQUIRED_SAMPLES ? 'success' : 'warning'"
                            variant="soft"
                            :label="`${cmd.sampleCount}/${REQUIRED_SAMPLES}`"
                            size="xs"
                        />
                        <u-button size="xs" variant="ghost" color="primary" @click="openEditModal(cmd.name)">
                            Edit
                        </u-button>
                    </div>

                    <!-- Приём container -->
                    <div v-if="priemCommand" class="rounded-md border border-neutral-200 dark:border-neutral-700">
                        <!-- Приём header (same style as system commands) -->
                        <div class="bg-muted flex items-center gap-2 rounded-t-md px-3 py-2">
                            <span class="grow text-sm font-medium">{{ priemCommand.name }}</span>
                            <u-badge
                                :color="priemCommand.sampleCount >= REQUIRED_SAMPLES ? 'success' : 'warning'"
                                variant="soft"
                                :label="`${priemCommand.sampleCount}/${REQUIRED_SAMPLES}`"
                                size="xs"
                            />
                            <u-button
                                size="xs"
                                variant="ghost"
                                color="primary"
                                @click="openEditModal(priemCommand.name)"
                            >
                                Edit
                            </u-button>
                        </div>

                        <!-- Subcommands -->
                        <div class="space-y-2 p-2">
                            <div
                                v-for="cmd in userCommands"
                                :key="cmd.name"
                                class="flex items-center gap-2 rounded-md border border-neutral-300 px-3 py-2 dark:border-neutral-600"
                            >
                                <span class="grow text-sm font-medium">{{ cmd.name }}</span>
                                <u-badge
                                    :color="cmd.sampleCount >= REQUIRED_SAMPLES ? 'success' : 'warning'"
                                    variant="soft"
                                    :label="`${cmd.sampleCount}/${REQUIRED_SAMPLES}`"
                                    size="xs"
                                />
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

                            <!-- New command -->
                            <button
                                class="flex w-full items-center justify-center gap-2 rounded-md border border-dashed border-neutral-300 px-3 py-2 transition-colors hover:border-neutral-400 dark:border-neutral-600 dark:hover:border-neutral-500"
                                @click="openCreateModal"
                            >
                                <u-icon name="i-lucide-plus" class="text-dimmed size-4" />
                                <span class="text-dimmed text-xs">New command</span>
                            </button>
                        </div>
                    </div>
                </div>

                <!-- Flat list: before system setup (legacy commands) -->
                <div v-else class="space-y-2">
                    <div
                        v-for="cmd in wakeStore.commands"
                        :key="cmd.name"
                        class="bg-muted flex items-center gap-2 rounded-md px-3 py-2"
                    >
                        <span class="grow text-sm font-medium">{{ cmd.name }}</span>
                        <u-badge
                            :color="cmd.sampleCount >= REQUIRED_SAMPLES ? 'success' : 'warning'"
                            variant="soft"
                            :label="`${cmd.sampleCount}/${REQUIRED_SAMPLES}`"
                            size="xs"
                        />
                        <u-button size="xs" variant="ghost" color="primary" @click="openEditModal(cmd.name)">
                            Edit
                        </u-button>
                        <u-button size="xs" variant="ghost" color="error" @click="wakeStore.deleteCommand(cmd.name)">
                            Delete
                        </u-button>
                    </div>

                    <!-- New command -->
                    <button
                        class="flex w-full items-center justify-center gap-2 rounded-md border border-dashed border-neutral-300 px-3 py-2 transition-colors hover:border-neutral-400 dark:border-neutral-600 dark:hover:border-neutral-500"
                        @click="openCreateModal"
                    >
                        <u-icon name="i-lucide-plus" class="text-dimmed size-4" />
                        <span class="text-dimmed text-xs">New command</span>
                    </button>
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
        <div
            v-if="isDev"
            class="rounded-lg border border-dashed border-amber-300 bg-amber-50 p-3 dark:border-amber-700 dark:bg-amber-950/30"
        >
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

        <div class="text-dimmed text-center text-xs">{{ platformStore.platformLabel }}</div>
    </div>
</template>

<script setup lang="ts">
    import { RecordingFlow, type RecordingCommand } from '~/features/recording-flow';
    import { AudioMode, useAudioStore } from '~/entities/audio';
    import { usePlatformStore } from '~/entities/platform';
    import { useSettingsStore } from '~/entities/settings';
    import {
        useWakeCommandsStore,
        WAKE_ACTION_OPTIONS,
        REQUIRED_SAMPLES,
        BASE_COMMANDS,
        buildBaseCommandName,
        type WakeAction,
    } from '~/entities/wake-commands';

    const audioStore = useAudioStore();
    const platformStore = usePlatformStore();
    const settingsStore = useSettingsStore();
    const wakeStore = useWakeCommandsStore();

    // --- Command categorization ---
    const baseCommandNames = computed(() => {
        const names = new Set<string>();
        for (const def of BASE_COMMANDS) {
            names.add(buildBaseCommandName(settingsStore.assistantName, def.suffix));
        }
        return names;
    });

    const priemDef = BASE_COMMANDS.find(c => c.action === 'await_subcommand')!;
    const priemCommandName = computed(() => buildBaseCommandName(settingsStore.assistantName, priemDef.suffix));

    const systemCommands = computed(() =>
        wakeStore.commands.filter(cmd => baseCommandNames.value.has(cmd.name) && cmd.name !== priemCommandName.value)
    );

    const priemCommand = computed(() => wakeStore.commands.find(cmd => cmd.name === priemCommandName.value) ?? null);

    const userCommands = computed(() => wakeStore.commands.filter(cmd => !baseCommandNames.value.has(cmd.name)));

    const hasSystemSetup = computed(() => systemCommands.value.length > 0 || priemCommand.value !== null);

    const isDev = import.meta.dev;
    const testResult = ref<string | null>(null);

    // Modal state
    const modalOpen = ref(false);
    const modalStep = ref<'setup' | 'recording' | 'done'>('setup');
    const commandName = ref('');
    const commandAction = ref<WakeAction>('await_subcommand');
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
            action: isEditing.value ? undefined : commandAction.value,
            requiredSamples: REQUIRED_SAMPLES,
        },
    ]);

    const modes = [
        { value: AudioMode.Off, label: 'Off' },
        { value: AudioMode.Standby, label: 'Standby' },
        { value: AudioMode.Dictation, label: 'Dictation' },
        { value: AudioMode.Processing, label: 'Processing' },
        { value: AudioMode.AwaitingSubcommand, label: 'Awaiting' },
    ] as const;

    function openCreateModal() {
        commandName.value = '';
        commandAction.value = 'await_subcommand';
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
