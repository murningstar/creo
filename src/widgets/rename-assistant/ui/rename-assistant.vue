<template>
    <!-- Settings card -->
    <section class="shadow-card rounded-lg bg-white p-7 dark:bg-neutral-900">
        <div class="max-w-[26rem]">
            <div class="mb-1 flex items-center gap-3">
                <CreoLogo class="size-6 text-neutral-700 dark:text-neutral-300" />
                <h2 class="text-highlighted text-sm font-semibold">{{ settingsStore.assistantName }}</h2>
            </div>
            <p class="text-dimmed mb-4 text-xs">Voice commands use this name as prefix.</p>

            <div class="pl-7">
                <u-button size="xs" variant="soft" @click="openModal">Rename</u-button>
            </div>
        </div>
    </section>

    <!-- Rename modal -->
    <u-modal v-model:open="modalOpen" title="Rename Assistant" :ui="{ footer: 'justify-end' }">
        <!-- Intentionally no default slot trigger — opened programmatically -->
        <template #body>
            <!-- Step 1: New name -->
            <div v-if="step === 'name'" class="space-y-4">
                <p class="text-muted text-sm">
                    Choose a new name. All base voice commands will be re-recorded with the new name.
                </p>
                <u-form-field label="Assistant name">
                    <u-input v-model="newName" size="sm" placeholder="e.g. Джарвис" autofocus />
                </u-form-field>
                <div v-if="newName.trim()" class="space-y-1">
                    <p class="text-dimmed text-xs">Commands to record:</p>
                    <p v-for="cmd in previewCommands" :key="cmd" class="text-muted text-xs">"{{ cmd }}"</p>
                </div>
            </div>

            <!-- Step 2: Recording -->
            <div v-else-if="step === 'recording'">
                <RecordingFlow :commands="recordingCommands" @complete="onAllCommandsRecorded" />
            </div>

            <!-- Step 3: Done -->
            <div v-else-if="step === 'done'" class="space-y-3 text-center">
                <CreoLogo class="text-primary mx-auto size-12" />
                <p class="text-highlighted text-sm font-semibold">
                    {{ newName.trim() }}
                </p>
                <p class="text-muted text-sm">All base commands recorded successfully.</p>
            </div>
        </template>

        <template #footer="{ close }">
            <template v-if="step === 'name'">
                <u-button variant="outline" size="sm" @click="close">Cancel</u-button>
                <u-button color="primary" size="sm" :disabled="!canProceed" @click="startRecording">
                    Continue
                </u-button>
            </template>
            <template v-else-if="step === 'done'">
                <u-button color="primary" size="sm" @click="completeFlow(close)">Done</u-button>
            </template>
        </template>
    </u-modal>
</template>

<script setup lang="ts">
    import CreoLogo from '~/shared/ui/icons/ui/i-creo-logo.vue';
    import { RecordingFlow, type RecordingCommand } from '~/features/recording-flow';
    import { useSettingsStore } from '~/entities/settings';
    import {
        useWakeCommandsStore,
        BASE_COMMANDS,
        buildBaseCommandName,
        REQUIRED_SAMPLES,
    } from '~/entities/wake-commands';

    const settingsStore = useSettingsStore();
    const wakeStore = useWakeCommandsStore();

    const modalOpen = ref(false);
    const step = ref<'name' | 'recording' | 'done'>('name');
    const newName = ref('');

    const canProceed = computed(() => newName.value.trim().length > 0);

    const previewCommands = computed(() =>
        BASE_COMMANDS.map(cmd => buildBaseCommandName(newName.value.trim(), cmd.suffix))
    );

    const recordingCommands = computed<RecordingCommand[]>(() =>
        BASE_COMMANDS.map(cmd => ({
            name: buildBaseCommandName(newName.value.trim(), cmd.suffix),
            label: cmd.label,
            action: cmd.action,
            requiredSamples: REQUIRED_SAMPLES,
        }))
    );

    function openModal() {
        newName.value = '';
        step.value = 'name';
        modalOpen.value = true;
    }

    function startRecording() {
        if (!newName.value.trim()) return;
        step.value = 'recording';
    }

    function onAllCommandsRecorded() {
        step.value = 'done';
    }

    async function completeFlow(close: () => void) {
        const oldName = settingsStore.assistantName;
        const trimmed = newName.value.trim();

        try {
            await wakeStore.deleteBaseCommands(oldName);
            await settingsStore.setAssistantName(trimmed);
            await wakeStore.loadCommands();
        } finally {
            close();
        }
    }
</script>
