<template>
    <div class="space-y-6">
        <!-- Progress indicator -->
        <div class="flex gap-2">
            <div
                v-for="(cmd, idx) in commands"
                :key="cmd.name"
                class="h-1 flex-1 rounded-full transition-colors"
                :class="stepClass(idx)"
            />
        </div>

        <!-- Current command -->
        <div v-if="currentCommand">
            <div class="mb-1 flex items-baseline justify-between">
                <p class="text-highlighted text-sm font-semibold">{{ currentCommand.label }}</p>
                <span class="text-dimmed text-xs">{{ currentIndex + 1 }}/{{ commands.length }}</span>
            </div>
            <p class="text-muted mb-4 text-xs">
                Say "<span class="font-medium">{{ currentCommand.name }}</span
                >" clearly when ready.
            </p>

            <!-- Samples recorded -->
            <div class="mb-3 space-y-1">
                <div v-for="idx in currentCommand.requiredSamples" :key="idx" class="flex items-center gap-2">
                    <div
                        class="size-2 rounded-full transition-colors"
                        :class="idx <= samples.length ? 'bg-primary' : 'bg-neutral-200 dark:bg-neutral-700'"
                    />
                    <span class="text-xs" :class="idx <= samples.length ? 'text-highlighted' : 'text-dimmed'">
                        Sample {{ idx }}
                        <template v-if="idx <= samples.length">
                            — {{ samples[idx - 1]?.embeddingCount }} embeddings
                        </template>
                    </span>
                </div>
            </div>

            <u-button
                size="sm"
                :color="wakeStore.isRecording ? 'error' : canAdvance ? 'neutral' : 'primary'"
                :variant="canAdvance ? 'outline' : 'solid'"
                :disabled="wakeStore.isRecording || canAdvance"
                icon="i-lucide-mic"
                @click="record"
            >
                {{ wakeStore.isRecording ? 'Listening...' : canAdvance ? 'All recorded' : 'Record Sample' }}
            </u-button>

            <u-alert
                v-if="wakeStore.error"
                icon="i-lucide-circle-x"
                color="error"
                variant="soft"
                :description="wakeStore.error"
                class="mt-3"
            />
        </div>

        <!-- Navigation -->
        <div class="border-default flex justify-between border-t pt-4">
            <u-button v-if="currentIndex > 0" size="xs" variant="ghost" @click="prev">Back</u-button>
            <span v-else />
            <div class="flex gap-2">
                <u-button v-if="canSkip" size="xs" variant="ghost" @click="next">Skip</u-button>
                <u-button
                    v-if="canAdvance"
                    size="xs"
                    color="primary"
                    @click="currentIndex < commands.length - 1 ? next() : finish()"
                >
                    {{ currentIndex < commands.length - 1 ? 'Next' : 'Done' }}
                </u-button>
            </div>
        </div>
    </div>
</template>

<script setup lang="ts">
    import { useWakeCommandsStore, REQUIRED_SAMPLES, type RecordResult } from '~/entities/wake-commands';
    import type { RecordingCommand } from '../model/types';

    const props = defineProps<{
        commands: RecordingCommand[];
    }>();

    const emit = defineEmits<{
        complete: [];
    }>();

    const wakeStore = useWakeCommandsStore();

    const currentIndex = ref(0);
    const samplesPerCommand = ref<Map<number, RecordResult[]>>(new Map());

    const currentCommand = computed(() => props.commands[currentIndex.value]);

    const samples = computed(() => samplesPerCommand.value.get(currentIndex.value) ?? []);

    const canAdvance = computed(
        () => samples.value.length >= (currentCommand.value?.requiredSamples ?? REQUIRED_SAMPLES)
    );
    const canSkip = computed(() => !canAdvance.value && currentIndex.value < props.commands.length - 1);

    function stepClass(idx: number) {
        const recorded = samplesPerCommand.value.get(idx)?.length ?? 0;
        const required = props.commands[idx]?.requiredSamples ?? REQUIRED_SAMPLES;
        if (idx === currentIndex.value) return 'bg-primary';
        if (recorded >= required) return 'bg-primary/40';
        return 'bg-neutral-200 dark:bg-neutral-700';
    }

    async function record() {
        const cmd = currentCommand.value;
        if (!cmd) return;
        if (samples.value.length >= (cmd.requiredSamples ?? REQUIRED_SAMPLES)) return;

        const isFirst =
            !samplesPerCommand.value.has(currentIndex.value) ||
            samplesPerCommand.value.get(currentIndex.value)!.length === 0;
        const action = isFirst ? cmd.action : undefined;

        const result = await wakeStore.recordSample(cmd.name, action);
        if (result) {
            const existing = samplesPerCommand.value.get(currentIndex.value) ?? [];
            samplesPerCommand.value.set(currentIndex.value, [...existing, result]);
        }
    }

    function next() {
        if (currentIndex.value < props.commands.length - 1) {
            currentIndex.value++;
        }
    }

    function prev() {
        if (currentIndex.value > 0) {
            currentIndex.value--;
        }
    }

    function finish() {
        emit('complete');
    }
</script>
